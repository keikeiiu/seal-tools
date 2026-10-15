using System;
using System.Collections.Generic;
using System.IO;
using YamlDotNet.Serialization;
using YamlDotNet.Serialization.NamingConventions;

namespace SealTools.Core.Config;

// Loads defaults.yaml + local.yaml (overlay) + attributes.yaml.
// Fails loudly (ConfigException) on missing files or invalid machine-specific values —
// no silent fallback defaults anywhere in code.

public sealed class ConfigLoader
{
    private readonly string _configDir;

    public ConfigLoader(string configDir) => _configDir = configDir;

    private static readonly IDeserializer Deserializer = new DeserializerBuilder()
        .WithNamingConvention(UnderscoredNamingConvention.Instance)
        .IgnoreUnmatchedProperties()
        .Build();

    private static readonly ISerializer Serializer = new SerializerBuilder()
        .WithNamingConvention(UnderscoredNamingConvention.Instance)
        .Build();

    private string PathOf(string name) => Path.Combine(_configDir, name);

    // Written above the serialized content on every save. YamlDotNet serializes an object graph, so
    // it cannot carry comments through: whatever prose a save destroys is gone for good, and the only
    // comments that survive are the ones the writer emits itself. That is what this is.
    //
    // This is now the ONLY file a save rewrites, which is why it carries both halves: the machine's own
    // measurements and the portable settings that used to live in defaults.yaml. defaults.yaml is a
    // shipped template that an update replaces, so a save target inside it was a setting an update
    // could cover.
    private const string LocalHeader =
        "# ── This machine's configuration ──────────────────────────────────────────────────\n" +
        "# Two halves, and they travel differently.\n" +
        "#\n" +
        "# The MEASURED half: coordinates, the display environment, the Arduino port override, and the\n" +
        "# calibration images beside this file. True only on THIS machine.\n" +
        "#\n" +
        "# The `settings:` block: the PORTABLE half — the same values on every machine. It lives here\n" +
        "# rather than in defaults.yaml because this file is YOURS. defaults.yaml is the template\n" +
        "# publish.bat ships, so anything saved there would be replaced by the next update. A hand-added\n" +
        "# spammer preset is personal too, and that is why presets live here as well.\n" +
        "#\n" +
        "# Gitignored, and excluded from the public zip by publish.bat — do not commit it or share it.\n" +
        "#\n" +
        "# READ THIS BEFORE EDITING. A save REWRITES this whole file's owned blocks — the ones named in\n" +
        "# docs/CONFIG.md — and leaves any other TOP-LEVEL block alone, so a block of your own survives.\n" +
        "# Inside an owned block, a key the launcher does not know about is DROPPED, and comments are\n" +
        "# lost everywhere: the writer serialises values and cannot carry prose. Keep a copy if you edit.\n" +
        "#\n" +
        "# Documented in docs/CONFIG.md; calibrating is docs/CALIBRATION.md.\n" +
        "\n";

    public AppConfig Load()
    {
        // THE BASE. An install that has taken an update has the shipped template — the zip carries the
        // defaults as config/defaults.yaml.example precisely so that extracting an update cannot cover
        // the player's own defaults.yaml — and reads that. A checkout has only the tracked file, so a
        // dev run reads exactly what it read before.
        var baseIsShipped = File.Exists(PathOf("defaults.yaml.example"));
        var defaults = Deserialize<AppConfig>(baseIsShipped ? "defaults.yaml.example" : "defaults.yaml");

        // First run on a new machine: seed the machine-specific config from the
        // example template (v1 coords as a starting point), then calibrate in-app.
        var localPath = PathOf("local.yaml");
        if (!File.Exists(localPath))
        {
            var examplePath = PathOf("local.yaml.example");
            if (File.Exists(examplePath))
                // Through the same writer as every other save, not File.Copy: a copy interrupted
                // halfway leaves a TRUNCATED local.yaml that loads as a valid empty config, and the
                // player then runs on v1 starting coordinates without an error anywhere. The atomic
                // write leaves either the old state or the whole file.
                WriteAtomically(localPath, File.ReadAllText(examplePath));
        }

        if (!File.Exists(localPath))
        {
            throw new ConfigException(
                "config/local.yaml not found and config/local.yaml.example is missing, so it cannot be created.");
        }

        var local = Deserialize<LocalOverrides>("local.yaml");

        // THE LEGACY FILE. On an install that has taken an update, this is the player's own old
        // defaults.yaml — the only copy of every setting they ever saved, because the old save path wrote
        // the whole portable projection into it. It is read ONCE: the fold below moves those values into
        // local.yaml, and from then on the marker stops this file being read at all.
        var alreadyFolded = local.Settings?.LegacyFolded == true;
        var legacy = baseIsShipped && !alreadyFolded && File.Exists(PathOf("defaults.yaml"))
            ? Deserialize<AppConfig>("defaults.yaml")
            : null;

        // Normalise both files the same way, because the fold compares like with like: a pre-presets
        // flat `spammer.keys` becomes a preset in memory first, so the comparison never meets two shapes
        // of the same setting — and a leaf with no destination has nowhere to land.
        if (legacy != null) MigrateSpammerPresets(legacy);
        MigrateSpammerPresets(defaults);

        // THE FOLD, once. It compares section by section and writes only what the player changed, so a
        // default a later release changes still arrives for every key they never touched.
        var folded = legacy != null && FoldIntoLocal(legacy, defaults);

        // A successful fold has just written local.yaml, so the copy read above is stale. Re-read it, or
        // the player's own values arrive only on the NEXT launch — which is exactly the launch after the
        // one that was supposed to bring them across.
        if (folded) local = Deserialize<LocalOverrides>("local.yaml");

        // The legacy file as a VALUES LAYER — the pre-fold behaviour, kept only as the fallback for a
        // fold that could not be written (read-only medium, full disk). It sits BELOW local.yaml, which
        // is why it is applied before ApplyOverrides.
        if (legacy != null && !folded)
            ApplySettings(defaults, FromConfig(legacy));

        // Order matters: legacy keys become a preset in memory, adoption copies whatever presets are
        // left into local.yaml, and only then does local.yaml override the result.
        //
        // The adoption reads whichever file actually holds the player's presets — the legacy file on an
        // install that has taken an update, since the shipped template has none — and its own guard makes
        // it once-only, so every later launch does nothing.
        AdoptSpammerPresetsIntoLocal(legacy ?? defaults, local);
        ApplyOverrides(defaults, local);

        // AFTER the merge, because the migration only fills gaps: a file that already carries
        // `slots:` must win over the flat keys it also happens to still hold.
        MigratePetSlots(defaults.Pet, Deserialize<LocalOverridesLegacy>("local.yaml").Pet);

        // THE VARIANT RESOLVES THE TITLE, once, here — and that placement is the whole reason this
        // change is small: `window.title` stays the single value every tool already reads, so none of
        // the call sites in OcrEngine, GemComposer, HidPointer or the launcher has to know a variant
        // exists. Before the validator, which requires a title to be present.
        //
        // An unset or unknown variant changes NOTHING, deliberately: a file written before variants
        // existed carries only `window.title`, and an unrecognised name must not blank the target.
        if (defaults.Game.Active is { Title.Length: > 0 } variant)
            defaults.Window.Title = variant.Title;

        ConfigValidator.Validate(defaults);
        return defaults;
    }

    // A pet block written before the breeding rows existed is one flat set of fields — see
    // LocalPetLegacy for why it is read separately, and for what goes wrong without this.
    //
    // Fills only what the new shape left empty, so a half-migrated file (rows written by the new build,
    // flat keys still present because nothing rewrote them) resolves to the rows.
    private static void MigratePetSlots(PetConfig pet, LocalPetLegacy? old)
    {
        if (old == null) return;

        // `pet_cell` and `food_cells` were renamed to `return_slot` and `food_slots`, so their VALUES
        // are stranded under keys nothing maps to — the same silent loss, one level down.
        // The empty-slot reference used to be ONE crop for the whole tool. It is per row now, because
        // one crop shared by four rows read three of them wrong — see PetSlotConfig.PetSlotEmptyPng.
        // A file written before that carries it at the top level, where it belongs to row 1.
        if (pet.Slots.Count > 0 && string.IsNullOrWhiteSpace(pet.Slots[0].PetSlotEmptyPng))
            pet.Slots[0].PetSlotEmptyPng = old.PetSlotEmptyPng;

        if (pet.ReturnSlot is not { Count: 2 } && old.PetCell is { Count: 2 }) pet.ReturnSlot = old.PetCell;
        if (pet.FoodSlots.Count == 0 && old.FoodCells is { Count: > 0 }) pet.FoodSlots = old.FoodCells;
        if (pet.FoodSlotsUsed == 0 && old.FoodCellsUsed is { } used and > 0) pet.FoodSlotsUsed = used;
        if (pet.Queue.Count == 0 && !string.IsNullOrWhiteSpace(old.PetIconPng))
            pet.Queue.Add(new PetQueueEntry { Rect = old.PetIconRect, Png = old.PetIconPng });

        if (pet.Slots.Count > 0) return;

        // The one row the old build could drive, and it is the free row — which is why 2 is the right
        // stack count to carry over rather than a guess.
        pet.Slots.Add(new PetSlotConfig
        {
            ToggleLabel = old.ToggleLabel,
            BoardingPetSlot = old.BoardingPetSlot,
            // The old flat config held two COUNT boxes — boxes around the numbers — not slot boxes, so
            // their span is an APPROXIMATION of the strip the new calibration asks for: it runs between
            // the two numbers rather than between the two slots. Carried anyway, because without it a
            // pre-rows file could not load food at all, and a strip that is a little narrow still puts
            // a click inside the slot. Redrawing it is the first thing to do on such a machine.
            FeederStrip = SpanOf(old.FeederSlotA, old.FeederSlotB),
            Stacks = 2,
            PetSlotEmptyPng = old.PetSlotEmptyPng,
            BoardingRunning = old.BoardingRunning ?? false,
        });
    }

    private static bool IsValidRect(List<int>? r) => BagGrid.IsValidRect(r);

    /// <summary>The smallest rect containing both, or null when neither is usable. Used only by the
    /// pre-rows migration, to squeeze a strip out of two boxes it was never really made of.</summary>
    private static List<int>? SpanOf(List<int>? a, List<int>? b)
    {
        if (!IsValidRect(a)) return IsValidRect(b) ? b : null;
        if (!IsValidRect(b)) return a;

        var left = Math.Min(a![0], b![0]);
        var top = Math.Min(a[1], b[1]);
        var right = Math.Max(a[0] + a[2], b[0] + b[2]);
        var bottom = Math.Max(a[1] + a[3], b[1] + b[3]);
        return new List<int> { left, top, right - left, bottom - top };
    }


    // A config written before presets existed has a flat `spammer.keys` list. Move it into a preset
    // named "default" so the rest of the app only deals with presets.
    //
    // This has to run before ApplyOverrides merges local.yaml in. It used to run after, and then a
    // local.yaml that supplied any preset at all made Presets.Count non-zero, so a pre-presets
    // config never converted and its keys were dropped silently.
    private static void MigrateSpammerPresets(AppConfig cfg)
    {
        var sp = cfg.Spammer;
        if (sp.Presets is not { Count: > 0 } && sp.Keys is { Count: > 0 } legacy)
        {
            sp.Presets ??= new();
            sp.Presets["default"] = legacy;
            sp.Active = "default";
        }
        sp.Keys = null;
    }

    // Presets used to be saved into defaults.yaml — the file publish.bat copies into the public zip.
    // The save path stopped writing them, so the first Save on ANY other tab (tuner, gem and hotkeys
    // all called it) rewrote defaults.yaml without a spammer block. On a machine that only ever ran
    // the older build those presets existed nowhere else, so that Save deleted them outright and the
    // next launch quietly created a blank "default". Adopt them into local.yaml before that can
    // happen.
    //
    // Runs at most once per install: local.Spammer is non-null afterwards, including on a first run
    // where local.yaml was just seeded from the example (which carries no spammer block).
    private void AdoptSpammerPresetsIntoLocal(AppConfig defaults, LocalOverrides local)
    {
        if (local.Spammer != null || defaults.Spammer.Presets is not { Count: > 0 }) return;

        // A hand-written LEGACY step folds into the object shape first, so the copy that is adopted
        // carries one representation rather than two — and the old key is then never written again.
        if (defaults.Spammer.StartupStep != null)
            foreach (var (name, step) in defaults.Spammer.StartupStep)
                StartupFor(defaults.Spammer, name).Step = step;

        var adopted = new LocalSpammer
        {
            Active = defaults.Spammer.Active,
            Presets = defaults.Spammer.Presets,
            // Carried for the same reason the presets are: this adoption is the one path that moves
            // a hand-written defaults.yaml block into local.yaml, and dropping it here would keep the
            // keys and silently lose their order.
            Priority = defaults.Spammer.Priority,
            Combos = defaults.Spammer.Combos,
            Startup = defaults.Spammer.Startup,
        };

        // Written through the gated path like every other writer, and the guard is re-checked against
        // the file as it stands NOW: another process can have adopted in the gap since this Load()
        // read local.yaml, and writing ours blind would replace the winner's presets with our copy.
        LocalSpammer? inFile = null;
        UpdateLocal(fresh =>
        {
            fresh.Spammer ??= adopted;
            inFile = fresh.Spammer;
        });

        // The session follows the file, so even the process that lost that race reads the presets
        // that are actually stored rather than a second set of its own.
        local.Spammer = inFile;
    }

    /// <summary>The startup block for a preset, created if it has none — so folding a LEGACY entry in
    /// does not have to ask whether the new shape already carries one.</summary>
    private static SpammerStartup StartupFor(SpammerConfig spammer, string name)
        => spammer.Startup.TryGetValue(name, out var s)
            ? s
            : spammer.Startup[name] = new SpammerStartup();

    /// <summary>Loads the OCR dictionary — the active game variant's, which the caller names.
    /// <paramref name="file"/> omitted means the TW dictionary, which is what a config with no variant
    /// uses.
    ///
    /// A missing file throws, through <see cref="Deserialize"/>, rather than falling back — and that is
    /// the deliberate direction: an absent dictionary matches NOTHING, so every attribute would read as
    /// "no attribute", silently, and look exactly like the game having changed.</summary>
    public AttributesConfig LoadAttributes(string? file = null)
    {
        var name = string.IsNullOrWhiteSpace(file) ? "attributes.yaml" : file;

        if (!File.Exists(PathOf(name)))
        {
            // A variant's dictionary ships as its TEMPLATE and nothing else (attributes.us.yaml.example),
            // because a filled one is something the player grows in place — and an update must not be
            // able to cover it. So a missing dictionary is SEEDED from the template, which is the
            // instruction the old error message gave by hand and the same seed shape local.yaml has.
            //
            // The template carries `attributes: []` rather than a bare `attributes:` key on purpose: the
            // consumers enumerate that list (the switch's status line, the attributes tab, the rules
            // editor) and a null one throws where an empty one means "nothing named yet".
            var example = name + ".example";
            if (File.Exists(PathOf(example)))
            {
                WriteAtomically(PathOf(name), File.ReadAllText(PathOf(example)));
            }
            else
            {
                // No template either. Still loud, and for the original reason: a dictionary that matches
                // nothing reads every attribute as "no attribute", silently, which looks exactly like
                // the game having changed.
                throw new ConfigException($"Missing config file: {PathOf(name)}");
            }
        }

        return Deserialize<AttributesConfig>(name);
    }

    // Load just the machine-specific overlay (returns null if local.yaml is absent).
    public LocalOverrides? LoadLocal()
    {
        var path = PathOf("local.yaml");
        if (!File.Exists(path)) return null;
        return Deserialize<LocalOverrides>("local.yaml");
    }

    private T Deserialize<T>(string name)
    {
        var path = PathOf(name);
        if (!File.Exists(path))
            throw new ConfigException($"Missing config file: {path}");
        string text;
        try
        {
            text = File.ReadAllText(path);
        }
        catch (IOException ex)
        {
            throw new ConfigException($"Cannot read {path}: {ex.Message}", ex);
        }

        try
        {
            return Deserializer.Deserialize<T>(text);
        }
        catch (Exception ex)
        {
            throw new ConfigException($"Failed to parse {path}: {ex.Message}", ex);
        }
    }

    /// <summary>Applies a settings block onto the config being loaded: what the fold writes into
    /// local.yaml, or — while a fold is still pending — the legacy file's own portable values.
    ///
    /// Every guard is "is it PRESENT", never "is it non-empty". A block written by the fold is partial by
    /// design, so a value it does not mention must leave the shipped one standing; and a value it states
    /// as empty is a choice, not an omission.
    ///
    /// Applied FIRST in <see cref="ApplyOverrides"/>, so the machine-specific half below keeps the last
    /// word wherever the two ever name the same field. Today they never do: LocalTuner, LocalGem and
    /// LocalArduino carry geometry, points and the port, and this block carries none of those.</summary>
    private static void ApplySettings(AppConfig defaults, LocalSettings? settings)
    {
        if (settings is null) return;

        if (settings.Window != null) defaults.Window = settings.Window;
        if (settings.ReferenceWindow != null) defaults.ReferenceWindow = settings.ReferenceWindow;
        if (settings.Game is { } game && defaults.Game != null)
        {
            // Per name, like every other dictionary in this file: a client a later release adds to the
            // shipped defaults must still arrive, and the player's own entry must still win.
            if (game.Variants is { Count: > 0 } variants)
                foreach (var (name, variant) in variants)
                    defaults.Game.Variants[name] = variant;
            // Unconditional when present: an empty variant is the real choice "use window.title", and
            // skipping it on emptiness would make going back impossible — the same reasoning as
            // quest.active_sequence below.
            if (game.Variant != null) defaults.Game.Variant = game.Variant;
        }
        if (settings.Arduino is { } arduino)
        {
            if (arduino.Vid is { } vid) defaults.Arduino.Vid = vid;
            if (arduino.Pid is { } pid) defaults.Arduino.Pid = pid;
            if (arduino.Baud is { } baud) defaults.Arduino.Baud = baud;
        }
        if (settings.Hotkeys != null) defaults.Hotkeys = settings.Hotkeys;
        // Guarded like the local tuner half below, and for the same recorded reason: a bare `tuner:` key
        // in the base makes it null, and dereferencing that threw before Validate could report the empty
        // section.
        if (settings.Tuner is { } tuner && defaults.Tuner != null)
        {
            if (tuner.GradeOrder is { } gradeOrder) defaults.Tuner.GradeOrder = gradeOrder;
            if (tuner.TargetGrade != null) defaults.Tuner.TargetGrade = tuner.TargetGrade;
            if (tuner.MaxRetries is { } maxRetries) defaults.Tuner.MaxRetries = maxRetries;
            if (tuner.SaveCaptures is { } saveCaptures) defaults.Tuner.SaveCaptures = saveCaptures;
            if (tuner.OcrRetries is { } ocrRetries) defaults.Tuner.OcrRetries = ocrRetries;
            if (tuner.SpringMode != null) defaults.Tuner.SpringMode = tuner.SpringMode;
            if (tuner.MouseGuard != null) defaults.Tuner.MouseGuard = tuner.MouseGuard;
            if (tuner.GuardPx is { } guardPx) defaults.Tuner.GuardPx = guardPx;
            if (tuner.RecenterMax is { } recenterMax) defaults.Tuner.RecenterMax = recenterMax;
            if (tuner.Models != null) defaults.Tuner.Models = tuner.Models;
            if (tuner.Timing != null) defaults.Tuner.Timing = tuner.Timing;
            if (tuner.GradeColors != null) defaults.Tuner.GradeColors = tuner.GradeColors;
            if (tuner.Filter != null) defaults.Tuner.Filter = tuner.Filter;
        }
        if (settings.Gem is { } gem)
        {
            // `is { Count: > 0 }`, unlike the scalars above: an empty grade list is not a state the
            // composer can run, so writing one must not replace a shipped list with nothing.
            if (gem.Grades is { Count: > 0 } grades) defaults.Gem.Grades = grades;
            if (gem.StartGrade != null) defaults.Gem.StartGrade = gem.StartGrade;
            if (gem.EmptyMode != null) defaults.Gem.EmptyMode = gem.EmptyMode;
            if (gem.EmptyStreak is { } emptyStreak) defaults.Gem.EmptyStreak = emptyStreak;
            if (gem.ColoredGapMin is { } coloredGapMin) defaults.Gem.ColoredGapMin = coloredGapMin;
            if (gem.SaveEmptyCaptures is { } saveEmptyCaptures) defaults.Gem.SaveEmptyCaptures = saveEmptyCaptures;
            if (gem.MoveMode != null) defaults.Gem.MoveMode = gem.MoveMode;
        }
        if (settings.Pet is { } petSettings)
        {
            if (petSettings.ItemsPerMinute is { } itemsPerMinute) defaults.Pet.ItemsPerMinute = itemsPerMinute;
            if (petSettings.PetSlotOccupiedAbove is { } petSlotOccupiedAbove)
                defaults.Pet.PetSlotOccupiedAbove = petSlotOccupiedAbove;
        }
    }

    private static void ApplyOverrides(AppConfig defaults, LocalOverrides local)
    {
        // The portable layer first — the layer defaults.yaml used to be — then this machine's own
        // measurements on top of it.
        ApplySettings(defaults, local.Settings);

        if (local.Calibration != null) defaults.Calibration = local.Calibration;
        // `defaults.Tuner` can itself be null — a bare `tuner:` key in defaults.yaml makes it so — and
        // dereferencing it here threw before Validate could report the empty section. The local half
        // was already null-checked; this half was not, which is the whole bug.
        if (defaults.Tuner != null)
        {
            if (local.Tuner?.Ocr != null) defaults.Tuner.Ocr = local.Tuner.Ocr;
            if (local.Tuner?.SpringPoint != null) defaults.Tuner.SpringPoint = local.Tuner.SpringPoint;
        }
        if (local.Gem?.GradePositions is { Count: > 0 }) defaults.Gem.GradePositions = local.Gem.GradePositions;
        if (local.Gem?.Movements != null) defaults.Gem.Movements = local.Gem.Movements;
        if (local.Gem?.ResourceGems != null) defaults.Gem.ResourceGems = local.Gem.ResourceGems;
        if (local.Gem?.ResultGemArea != null) defaults.Gem.ResultGemArea = local.Gem.ResultGemArea;
        if (local.Gem?.EmptySignature != null) defaults.Gem.EmptySignature = local.Gem.EmptySignature;
        if (local.Gem?.EmptyDistance != null) defaults.Gem.EmptyDistance = local.Gem.EmptyDistance.Value;
        if (!string.IsNullOrEmpty(local.Arduino?.Port)) defaults.Arduino.Port = local.Arduino.Port;
        if (local.BuySell is { } bs)
        {
            if (BagGrid.IsValidRect(bs.BagGrid)) defaults.BuySell.BagGrid = bs.BagGrid;
            if (BagGrid.IsValidRect(bs.BagSlot)) defaults.BuySell.BagSlot = bs.BagSlot;
            if (bs.ShopRows is > 0) defaults.BuySell.ShopRows = bs.ShopRows.Value;
            if (BagGrid.IsValidRect(bs.ShopRegion)) defaults.BuySell.ShopRegion = bs.ShopRegion;
            if (IsPoint(bs.ScrollPoint)) defaults.BuySell.ScrollPoint = bs.ScrollPoint;
            if (IsPoint(bs.MaxButton)) defaults.BuySell.MaxButton = bs.MaxButton;
            if (bs.SellCap is > 0) defaults.BuySell.SellCap = bs.SellCap.Value;

            // Merged per name, like the spammer presets, so a preset added by hand in defaults.yaml
            // would survive beside the player's own.
            if (bs.Presets is { Count: > 0 })
                foreach (var (name, preset) in bs.Presets)
                    defaults.BuySell.Presets[name] = preset;
        }

        if (local.Pet is { } pet)
        {
            if (IsPoint(pet.MenuButton)) defaults.Pet.MenuButton = pet.MenuButton;
            if (IsPoint(pet.FeedIcon)) defaults.Pet.FeedIcon = pet.FeedIcon;
            if (IsPoint(pet.CloseButton)) defaults.Pet.CloseButton = pet.CloseButton;
            if (!string.IsNullOrWhiteSpace(pet.CloseButtonPng)) defaults.Pet.CloseButtonPng = pet.CloseButtonPng;
            // A LOAD-side line with no reflection fence over it (the fences cover the save direction),
            // which is exactly where this block has silently dropped fields before. False means "no
            // belief", which is also the default, so only the true case is worth assigning.
            if (pet.BoardingWindowMayBeOpen is true) defaults.Pet.BoardingWindowMayBeOpen = true;
            if (pet.PageTabs is { Count: > 0 }) defaults.Pet.PageTabs = pet.PageTabs;
            // The boarding bag's grid, deliberately separate from BuySell's — the bag sits somewhere
            // else in this flow, so borrowing those numbers would aim every cell at the wrong item.
            if (BagGrid.IsValidRect(pet.BagGrid)) defaults.Pet.BagGrid = pet.BagGrid;
            if (BagGrid.IsValidRect(pet.BagSlot)) defaults.Pet.BagSlot = pet.BagSlot;
            if (pet.FoodSlots is { Count: > 0 }) defaults.Pet.FoodSlots = pet.FoodSlots;
            if (pet.FoodSlotsUsed is { } used and >= 0) defaults.Pet.FoodSlotsUsed = used;
            if (!string.IsNullOrWhiteSpace(pet.FoodIconPng)) defaults.Pet.FoodIconPng = pet.FoodIconPng;
            if (pet.ActionWaitMs is { } wait and > 0) defaults.Pet.ActionWaitMs = wait;
            if (pet.WaitAfterEmptyMinutes is { } wait2) defaults.Pet.WaitAfterEmptyMinutes = wait2;
            if (pet.ReloadOnStart is { } onStart) defaults.Pet.ReloadOnStart = onStart;
            if (!string.IsNullOrWhiteSpace(pet.FoodLoadMode)) defaults.Pet.FoodLoadMode = pet.FoodLoadMode;
            // IsValidRect, NOT IsPoint. These are RECTANGLES, and IsPoint means "exactly two values" —
            // so the point predicate is false for them, every time, and both were written to local.yaml
            // on save and silently DISCARDED on every load. The neighbouring MaxButton line is a
            // genuine point, and copying its predicate without reading it is how this happened.
            if (pet.FeederCountLeftFraction is { } fcl) defaults.Pet.FeederCountLeftFraction = fcl;
            if (pet.FeederCountMinScore is { } fms) defaults.Pet.FeederCountMinScore = fms;
            if (pet.ReturnSlot is { Count: 2 }) defaults.Pet.ReturnSlot = pet.ReturnSlot;
            if (IsPoint(pet.MaxButton)) defaults.Pet.MaxButton = pet.MaxButton;

            // Wholesale rather than merged per row: these are the player's own calibration for the
            // machine in front of them, and half of one file's rows merged into another's would be a
            // configuration nobody wrote.
            if (pet.Slots is { Count: > 0 } slots)
                defaults.Pet.Slots = slots.Select(s => s.ToConfig()).ToList();
            if (pet.Queue is { Count: > 0 } queued)
                defaults.Pet.Queue = queued.Select(q => q.ToConfig()).ToList();
        }

        // The hover panel's offset is measured in this machine's pixels, so it lives here with
        // the other calibrated geometry rather than in the portable defaults.
        if (local.Tooltip is { IsSet: true } tip) defaults.Tooltip = tip;

        // Spammer presets are the player's own key rotations, so they belong in local.yaml with the
        // calibration — not in the portable defaults.yaml that publish.bat ships. Merged per preset
        // name rather than wholesale, so a preset added by hand to defaults.yaml would survive.
        if (local.Spammer != null)
        {
            if (!string.IsNullOrEmpty(local.Spammer.Active)) defaults.Spammer.Active = local.Spammer.Active;
            if (local.Spammer.Presets != null)
                foreach (var (name, keys) in local.Spammer.Presets)
                    defaults.Spammer.Presets[name] = keys;
            // Merged the same way and for the same reason as the presets above. It is the same
            // projection hazard that has dropped a field four times in this repo, so it is on the
            // guard test's list — a copy that is missing here loads as "no order given", which is a
            // silent fall back to the old behaviour rather than an error.
            if (local.Spammer.Priority != null)
                foreach (var (name, order) in local.Spammer.Priority)
                    defaults.Spammer.Priority[name] = order;
            if (local.Spammer.Combos != null)
                foreach (var (name, combos) in local.Spammer.Combos)
                    defaults.Spammer.Combos[name] = combos;
            // The legacy bare step folds into the object shape FIRST, so a config written before the
            // lead-in existed keeps its stagger without the player retyping it; a new-style entry for
            // the same preset then overwrites it, which is what makes the fold a migration rather
            // than a merge of two live sources.
            if (local.Spammer.StartupStep != null)
                foreach (var (name, step) in local.Spammer.StartupStep)
                    StartupFor(defaults.Spammer, name).Step = step;
            if (local.Spammer.Startup != null)
                foreach (var (name, s) in local.Spammer.Startup)
                    defaults.Spammer.Startup[name] = s;
        }

        // Quest flows are personal in exactly the same way, and for the same reason they must not live
        // in defaults.yaml: that file is what publish.bat copies into a public zip. Merged per flow
        // name, so a flow added by hand to defaults.yaml would survive.
        if (local.Quest != null)
        {
            if (!string.IsNullOrEmpty(local.Quest.Active)) defaults.Quest.Active = local.Quest.Active;
            if (local.Quest.Presets != null)
                foreach (var (name, preset) in local.Quest.Presets)
                    defaults.Quest.Presets[name] = preset;

            // The sequence to run, merged per name like the flows. Note the assignment is UNCONDITIONAL
            // when the key is present: an empty `active_sequence` is a real choice — "run the single
            // flow" — and skipping it on emptiness would make it impossible to turn a sequence off.
            if (local.Quest.ActiveSequence != null)
                defaults.Quest.ActiveSequence = local.Quest.ActiveSequence;
            if (local.Quest.Sequences != null)
                foreach (var (name, sequence) in local.Quest.Sequences)
                    defaults.Quest.Sequences[name] = sequence;

            if (local.Quest.GuardPx is { } guardPx and >= 0) defaults.Quest.GuardPx = guardPx;
        }
    }

    // One writer at a time, and a temp name nobody else can be holding. Both save paths used to write
    // `<path>.tmp` with no mutual exclusion — so the WPF thread saving a calibration and the pet
    // tool's PersistPetState, which are independent, could both be writing that ONE name. The lock
    // serializes them; the unique name means a writer that died mid-save cannot leave a name for the
    // next one to collide with either. Static, because a second ConfigLoader instance must not be able
    // to interleave with the first.
    private static readonly object SaveGate = new();

    /// <summary>Replaces a config file with a fully written temp file, one writer at a time. The temp
    /// name is unique per call and is removed if the write fails, so a failure cannot leave litter in
    /// the config folder or a name for the next writer to trip over.</summary>
    private static void WriteAtomically(string path, string yaml)
    {
        lock (SaveGate)
        {
            var tmp = path + "." + System.Guid.NewGuid().ToString("N") + ".tmp";
            try
            {
                File.WriteAllText(tmp, yaml);
                File.Move(tmp, path, overwrite: true);
            }
            catch
            {
                // The first failure is the one worth reporting; a failed cleanup must not replace it.
                try { if (File.Exists(tmp)) File.Delete(tmp); } catch { /* keep the original error */ }
                throw;
            }
        }
    }

    /// <summary>Reads local.yaml, applies <paramref name="mutate"/> to it, and writes it back — all
    /// under <see cref="SaveGate"/>, which is the entire point of the method.
    ///
    /// The gate used to cover only the file publish, so every caller loaded a snapshot and saved it
    /// later with nothing held in between. The launcher's threads and the pet tool's PersistPetState
    /// are independent, and whichever of two writers saved second wrote its own older snapshot back:
    /// the other's block was silently DELETED from the file, and neither caller could tell. Reading
    /// inside the lock makes that unrepresentable — a writer says WHAT to change, never what the whole
    /// file should contain.
    ///
    /// The callback may therefore run more than once in a process's lifetime but never concurrently,
    /// and it sees the file as it is now rather than as it was when some earlier Load() ran. A file
    /// that cannot be PARSED fails the save rather than being overwritten — the read comes first, so
    /// an unreadable local.yaml says so instead of being replaced by whatever was in memory.</summary>
    public void UpdateLocal(Action<LocalOverrides> mutate)
    {
        lock (SaveGate)
        {
            var path = PathOf("local.yaml");
            // Whatever is in the file that this app does not own is carried through the save verbatim.
            // A save re-serialises an object graph, so before this it DELETED any top-level block the
            // class does not model — a hand-added one, or one a future release added that this build
            // does not know about. That is the same silent-deletion shape this file has paid for four
            // times, and the reason the header can no longer say "any key is dropped".
            var foreign = ForeignBlocks(File.Exists(path) ? File.ReadAllText(path) : "");
            var local = LoadLocal() ?? new LocalOverrides();
            mutate(local);
            WriteAtomically(path, LocalHeader + Serializer.Serialize(local) + foreign);
        }
    }

    /// <summary>The top-level blocks of local.yaml this app owns and rewrites. Every property of
    /// <see cref="LocalOverrides"/> is one of these, and a test walks the class to keep that true — a
    /// block added to the class but forgotten here would be preserved by accident and never updated.</summary>
    public static readonly string[] OwnedLocalBlocks =
    {
        "calibration", "tuner", "gem", "arduino", "ui", "spammer", "buy_sell", "pet", "tooltip",
        "quest", "settings",
    };

    /// <summary>The top-level blocks in <paramref name="text"/> that this app does NOT own, as raw text to
    /// append after a save — verbatim, comments included, so a hand-added or future block survives
    /// untouched. Empty when there is nothing foreign, which is the normal case.</summary>
    private static string ForeignBlocks(string text)
    {
        if (text.Length == 0) return "";

        var lines = text.Replace("\r\n", "\n").Split('\n');
        var kept = new List<string>();
        for (var i = 0; i < lines.Length; i++)
        {
            if (!TryTopLevelKey(lines[i], out var key) || IsOwned(key)) continue;

            // Walk back over the comment lines that introduce the block, so a hand-written explanation
            // travels with it. A blank line stops the walk, which keeps the file's own header out.
            var from = i;
            while (from > 0 && lines[from - 1].StartsWith('#')) from--;

            var to = i + 1;
            while (to < lines.Length && !TryTopLevelKey(lines[to], out _)) to++;

            kept.Add(string.Join("\n", lines, from, to - from).TrimEnd('\n'));
            i = to - 1;
        }

        return kept.Count == 0 ? "" : "\n" + string.Join("\n", kept) + "\n";
    }

    private static bool IsOwned(string key)
    {
        foreach (var owned in OwnedLocalBlocks)
        {
            if (owned == key) return true;
        }

        return false;
    }

    /// <summary>True when the line opens a top-level block: column zero, not a comment, and a bare
    /// `name:` — which is how the writer emits every block it owns.</summary>
    private static bool TryTopLevelKey(string line, out string key)
    {
        key = "";
        if (line.Length == 0 || line[0] == '#' || char.IsWhiteSpace(line[0])) return false;

        var colon = line.IndexOf(':');
        if (colon <= 0) return false;

        for (var i = 0; i < colon; i++)
        {
            if (!char.IsLetterOrDigit(line[i]) && line[i] != '_') return false;
        }

        key = line[..colon];
        return true;
    }

    /// <summary>Writes the portable settings into local.yaml's `settings:` block — the same sections and
    /// the same fields the shipped template carries, so that moving the save target moved the VALUES
    /// rather than re-deriving them.
    ///
    /// This is what SaveDefaults used to do, into the file publish.bat ships. That file is a template an
    /// update replaces, so a save target inside it was a setting an update could cover — which is the
    /// whole reason this block exists. Nothing writes defaults.yaml any more.
    ///
    /// The block is MUTATED rather than replaced, and that is the point of it being a block at all: a
    /// fresh object would drop the fold's marker and anything else this release does not know about —
    /// the silent-deletion shape this file and its tests have paid for four times.</summary>
    public void SaveSettings(AppConfig cfg) =>
        // Merged rather than replaced: a fresh block would drop the fold's marker, and the marker is what
        // makes "fold once" true.
        UpdateLocal(local => MergeSettings(FromConfig(cfg), local.Settings ??= new LocalSettings()));

    /// <summary>The portable settings of <paramref name="cfg"/>, as the mirror. This is the ONE place the
    /// projection's section list is written down for READING — SaveSettings writes it into local.yaml and
    /// the fold compares two of them — so neither can drift from the other.
    ///
    /// `game` is named, never the resolved object: GameConfig.Active is a reference INTO Variants, and
    /// serialising that graph writes the variant twice, the second time as a YAML alias
    /// (`us: &amp;o0 … active: *o0`) — the stray key a save left in defaults.yaml on 2026-10-13. And
    /// arduino carries no port: that one is this machine's and stays in LocalArduino, which is why the
    /// projection writes it as "".</summary>
    private static LocalSettings FromConfig(AppConfig cfg) => new()
    {
        Window = cfg.Window,
        ReferenceWindow = cfg.ReferenceWindow,
        Game = new LocalSettingsGame { Variant = cfg.Game.Variant, Variants = cfg.Game.Variants },
        Arduino = new LocalSettingsArduino { Vid = cfg.Arduino.Vid, Pid = cfg.Arduino.Pid, Baud = cfg.Arduino.Baud },
        Hotkeys = cfg.Hotkeys,
        Tuner = new LocalSettingsTuner
        {
            GradeOrder = cfg.Tuner.GradeOrder,
            TargetGrade = cfg.Tuner.TargetGrade,
            MaxRetries = cfg.Tuner.MaxRetries,
            SaveCaptures = cfg.Tuner.SaveCaptures,
            OcrRetries = cfg.Tuner.OcrRetries,
            SpringMode = cfg.Tuner.SpringMode,
            MouseGuard = cfg.Tuner.MouseGuard,
            GuardPx = cfg.Tuner.GuardPx,
            RecenterMax = cfg.Tuner.RecenterMax,
            Models = cfg.Tuner.Models,
            Timing = cfg.Tuner.Timing,
            GradeColors = cfg.Tuner.GradeColors,
            Filter = cfg.Tuner.Filter,
        },
        Gem = new LocalSettingsGem
        {
            Grades = cfg.Gem.Grades,
            StartGrade = cfg.Gem.StartGrade,
            EmptyMode = cfg.Gem.EmptyMode,
            EmptyStreak = cfg.Gem.EmptyStreak,
            ColoredGapMin = cfg.Gem.ColoredGapMin,
            SaveEmptyCaptures = cfg.Gem.SaveEmptyCaptures,
            MoveMode = cfg.Gem.MoveMode,
        },
        Pet = new LocalSettingsPet
        {
            ItemsPerMinute = cfg.Pet.ItemsPerMinute,
            PetSlotOccupiedAbove = cfg.Pet.PetSlotOccupiedAbove,
        },
    };

    /// <summary>Carries the sections <paramref name="from"/> holds onto <paramref name="to"/>, leaving
    /// every section it does not hold alone. With <paramref name="differFrom"/> given, a section is
    /// carried only when that one's WRITTEN form differs — the fold's rule, and what keeps a value the
    /// player never touched out of the block so it goes on tracking the shipped template.
    ///
    /// Sections are carried WHOLE, never field by field: the apply side assigns the object, so half a
    /// section would drag the other half's defaults in with it.
    ///
    /// <see cref="LocalSettings.LegacyFolded"/> is deliberately not in this list. The marker is not a
    /// setting, and a merge that carried it would be able to clear it.</summary>
    private static void MergeSettings(LocalSettings from, LocalSettings to, LocalSettings? differFrom = null)
    {
        if (Carry(from.Window, differFrom?.Window)) to.Window = from.Window;
        if (Carry(from.ReferenceWindow, differFrom?.ReferenceWindow)) to.ReferenceWindow = from.ReferenceWindow;
        if (Carry(from.Game, differFrom?.Game)) to.Game = from.Game;
        if (Carry(from.Arduino, differFrom?.Arduino)) to.Arduino = from.Arduino;
        if (Carry(from.Hotkeys, differFrom?.Hotkeys)) to.Hotkeys = from.Hotkeys;
        if (Carry(from.Tuner, differFrom?.Tuner)) to.Tuner = from.Tuner;
        if (Carry(from.Gem, differFrom?.Gem)) to.Gem = from.Gem;
        if (Carry(from.Pet, differFrom?.Pet)) to.Pet = from.Pet;
    }

    /// <summary>True when a section should be carried: it must be present, and — when there is something
    /// to compare against — it must differ from that in the form it would be WRITTEN in. Comparing the
    /// serialised form rather than the objects makes "differs" mean "the file would say something
    /// different", which is the only sense the fold cares about.</summary>
    private static bool Carry<T>(T? from, T? compare) where T : class
        => from != null && (compare == null || Serializer.Serialize(from) != Serializer.Serialize(compare));

    /// <summary>Moves the player's own portable values out of the legacy defaults file and into
    /// local.yaml's settings block, ONCE, and reports whether it succeeded.
    ///
    /// Reading is per leaf, writing is per SECTION: a section is carried whole as soon as any of its
    /// leaves differs from the shipped template, because the apply side assigns objects — `hotkeys.start`
    /// on its own would carry the default 0 for the other three keys.
    ///
    /// 'Differs from the shipped template' is how it reads 'the player changed it', and the direction it
    /// errs in is the safe one: a default a release changed under a key nobody touched is preserved
    /// rather than silently applied, and the release note naming that key is the habit that covers it.
    ///
    /// The source is never written, moved or deleted — it is the rollback copy and the record of what the
    /// player had. A write that cannot happen returns false rather than throwing: the marker stays unset,
    /// so the caller keeps reading the legacy file for the session and the fold retries next launch.</summary>
    private bool FoldIntoLocal(AppConfig legacy, AppConfig shipped)
    {
        var differing = new LocalSettings();
        MergeSettings(FromConfig(legacy), differing, differFrom: FromConfig(shipped));

        try
        {
            UpdateLocal(local =>
            {
                var s = local.Settings ??= new LocalSettings();
                MergeSettings(differing, s);
                // Unconditional, and in the SAME write. A machine that never changed a value must still
                // stop folding, or the next launch would fold the old template against a newer release's
                // and pin that release's defaults as the player's own.
                s.LegacyFolded = true;
            });
            return true;
        }
        catch (IOException)
        {
            return false;
        }
        catch (UnauthorizedAccessException)
        {
            return false;
        }
    }

    // Machine-specific overlay shape (mirrors local.yaml).
    public sealed class LocalOverrides
    {
        public CalibrationInfo? Calibration { get; set; }
        public LocalTuner? Tuner { get; set; }
        public LocalGem? Gem { get; set; }
        public LocalArduino? Arduino { get; set; }
        public LocalUi? Ui { get; set; }
        public LocalSpammer? Spammer { get; set; }
        public LocalBuySell? BuySell { get; set; }
        public LocalPet? Pet { get; set; }
        public TooltipConfig? Tooltip { get; set; }
        public LocalQuest? Quest { get; set; }

        /// <summary>The PORTABLE settings, in the machine's own file. Mirrors the shipped template's
        /// sections and fields exactly — that is what the fence compares — so that the values a save
        /// carries here are the ones defaults.yaml holds a template for.
        ///
        /// They live here rather than in defaults.yaml because that file is what publish.bat copies into
        /// the release zip: a save target that is also a shipped file is a file an update can cover.
        /// Written by <see cref="SaveSettings"/>; applied on load by <see cref="ApplyOverrides"/>.</summary>
        public LocalSettings? Settings { get; set; }
    }

    /// <summary>See <see cref="LocalOverrides.Settings"/>. Every section is nullable and so is every
    /// VALUE inside them, and that is load-bearing rather than defensive: a block the fold writes is
    /// partial by design — it carries only the values the player changed — so a field the block does not
    /// mention has to mean "not stated" and leave the shipped value standing. A non-nullable field would
    /// make silence indistinguishable from zero, which is how a partial write blanks a setting.
    ///
    /// The nested whole objects (window, reference_window, hotkeys, models, timing, grade_colors,
    /// filter) are the exception: they are copied and applied whole, so they keep the shape of the type
    /// they mirror and reuse it directly.
    ///
    /// Note what an unset section looks like on disk: this writer emits a null as a BARE key, so a folded
    /// block shows the sections it does not carry as `tuner:` with nothing under them. That is the same
    /// shape the file already gives its other unset values (`spring_point:`, `startup_step:`), and a
    /// reader must treat it as absent rather than as an empty object — which is exactly what the nullable
    /// fields and the presence guards are for.</summary>
    public sealed class LocalSettings
    {
        public WindowConfig? Window { get; set; }
        public ReferenceWindowConfig? ReferenceWindow { get; set; }
        public LocalSettingsGame? Game { get; set; }
        public LocalSettingsArduino? Arduino { get; set; }
        public HotkeysConfig? Hotkeys { get; set; }
        public LocalSettingsTuner? Tuner { get; set; }
        public LocalSettingsGem? Gem { get; set; }
        public LocalSettingsPet? Pet { get; set; }

        /// <summary>The once-only fold's marker (see PLAN-DEPLOY.md §4). A MODELLED field rather than a
        /// comment or a bare key, because every writer re-serialises from the object graph and would
        /// strip anything it does not know about — the marker is what makes "fold once" true, so it has
        /// to survive every save. Written by the fold in step 2; carried through untouched by
        /// <see cref="SaveSettings"/>.
        ///
        /// Until then it is null, and this writer emits a null as a BARE key — `legacy_folded:` with no
        /// value — the same shape the file already gives its unset optionals (`startup_step:`,
        /// `sell_slots:`). So the line shows up from the first settings save, meaning nothing yet.</summary>
        public bool? LegacyFolded { get; set; }
    }

    /// <summary>game, without GameConfig's computed <c>Active</c>. That property is a reference INTO
    /// <c>Variants</c>, and serialising the graph emits it twice — the second time as a YAML alias
    /// (<c>us: &amp;o0 … active: *o0</c>), which is the stray key a save left in defaults.yaml. The mirror
    /// names the variant and the table, and never the resolved object.</summary>
    public sealed class LocalSettingsGame
    {
        /// <summary>Nullable so that "not stated" and "deliberately empty" stay different: an empty
        /// variant is a real choice — it means the plain `window.title` is the target — so a block that
        /// says nothing must not be able to make it.</summary>
        public string? Variant { get; set; }

        /// <summary>Merged per name rather than replaced, so a client a later release adds still
        /// arrives: the same reasoning as the spammer's presets.</summary>
        public Dictionary<string, GameVariant>? Variants { get; set; }
    }

    /// <summary>arduino, without the port: that one is this machine's and lives in LocalArduino, which
    /// is why the projection writes <c>port = ""</c>.</summary>
    public sealed class LocalSettingsArduino
    {
        public int? Vid { get; set; }
        public List<int>? Pid { get; set; }
        public int? Baud { get; set; }
    }

    /// <summary>The 13 portable tuner fields. The calibrated geometry (ocr, spring_point) is machine
    /// state and stays in LocalTuner, so the two blocks never name the same field.</summary>
    public sealed class LocalSettingsTuner
    {
        public List<string>? GradeOrder { get; set; }
        public string? TargetGrade { get; set; }
        public int? MaxRetries { get; set; }
        public bool? SaveCaptures { get; set; }
        public int? OcrRetries { get; set; }
        public string? SpringMode { get; set; }
        public string? MouseGuard { get; set; }
        public int? GuardPx { get; set; }
        public int? RecenterMax { get; set; }
        public ModelsConfig? Models { get; set; }
        public TimingConfig? Timing { get; set; }
        public GradeColorsConfig? GradeColors { get; set; }
        public FilterConfig? Filter { get; set; }
    }

    /// <summary>The 7 portable gem fields. The calibrated points, the move counts and the empty-slot
    /// signature are machine state and stay in LocalGem.</summary>
    public sealed class LocalSettingsGem
    {
        public List<string>? Grades { get; set; }
        public string? StartGrade { get; set; }
        public string? EmptyMode { get; set; }
        public int? EmptyStreak { get; set; }
        public int? ColoredGapMin { get; set; }
        public bool? SaveEmptyCaptures { get; set; }
        public string? MoveMode { get; set; }
    }

    /// <summary>The two pet settings the feeder reads, which no config file could carry before.</summary>
    public sealed class LocalSettingsPet
    {
        public int? ItemsPerMinute { get; set; }
        public double? PetSlotOccupiedAbove { get; set; }
    }

    /// <summary>The buy/sell tool's geometry and presets. Machine-specific like the gem positions —
    /// written by the calibrator, read by the tool.</summary>
    public sealed class LocalBuySell
    {
        /// <summary>Whole bag grid, client-relative physical [x, y, w, h].</summary>
        public List<int>? BagGrid { get; set; }

        /// <summary>One bag slot in the same space, kept as the uniformity check.</summary>
        public List<int>? BagSlot { get; set; }

        /// <summary>Rows of the shop list visible at once.</summary>
        public int? ShopRows { get; set; }

        /// <summary>The visible shop list region [x, y, w, h].</summary>
        public List<int>? ShopRegion { get; set; }

        /// <summary>Where the cursor parks so the wheel scrolls the list.</summary>
        public List<int>? ScrollPoint { get; set; }

        /// <summary>MAX button centre in the count dialog.</summary>
        public List<int>? MaxButton { get; set; }

        /// <summary>Named buy items. Personal, so they stay out of the shipped defaults.yaml.</summary>
        public Dictionary<string, BuyPreset>? Presets { get; set; }

        /// <summary>Bag slots selected for selling (0 = top-left).</summary>
        public List<int>? SellSlots { get; set; }

        /// <summary>Hard per-run ceiling on slots sold.</summary>
        public int? SellCap { get; set; }
    }

    /// <summary>One breeding row, as it is written to local.yaml.</summary>
    public sealed class LocalPetSlot
    {
        public bool? Enabled { get; set; }
        public List<int>? ToggleLabel { get; set; }
        public List<int>? BoardingPetSlot { get; set; }
        public List<int>? FeederStrip { get; set; }
        public int? Stacks { get; set; }
        public string? PetSlotEmptyPng { get; set; }
        public bool? BoardingRunning { get; set; }

        /// <summary>The live shape, from the stored one. A missing `stacks` means TWO — the free
        /// row's count, which is the only row a file written before the rows existed can describe.
        ///
        /// EVERY field, and `PetSlotEmptyPng` was not — which meant every load silently dropped each
        /// row's empty-slot reference, and the next save wrote the blank back. Row 1 survived only
        /// because the migration re-supplies it from the old top-level value, so the loss looked like
        /// "rows 2-4 have no reference" rather than like a projection bug. The live run that followed
        /// read three empty rows as occupied and started an empty boarding on one of them.
        ///
        /// `EveryNestedPetFieldIsCopied` walks this type by reflection now, so the omission cannot
        /// happen a third time.</summary>
        public PetSlotConfig ToConfig() => new()
        {
            // Missing means ON, so a file written before this existed keeps driving its rows.
            Enabled = Enabled ?? true,
            ToggleLabel = ToggleLabel,
            BoardingPetSlot = BoardingPetSlot,
            FeederStrip = FeederStrip,
            Stacks = Stacks ?? 2,
            PetSlotEmptyPng = PetSlotEmptyPng,
            BoardingRunning = BoardingRunning ?? false,
        };
    }

    /// <summary>One queued pet, as it is written to local.yaml.</summary>
    public sealed class LocalPetQueueEntry
    {
        public string? Label { get; set; }
        public List<int>? Rect { get; set; }
        public string? Png { get; set; }

        /// <summary>The pet LINE, for the feeding table — see <see cref="PetQueueEntry.Species"/>.
        /// EVERY field must be listed in BOTH directions; this projection has silently dropped one
        /// before, on load, three times across two sessions.</summary>
        public string? Species { get; set; }

        public PetQueueEntry ToConfig() =>
            new() { Label = Label, Rect = Rect, Png = Png, Species = Species };
    }

    /// <summary>The pet food auto-replacement geometry (see <see cref="PetConfig"/>). Machine-specific
    /// in full — including the bag grid, which is NOT the same one the shop opens beside.</summary>
    public sealed class LocalPet
    {
        public List<int>? MenuButton { get; set; }
        public List<int>? FeedIcon { get; set; }
        public List<int>? CloseButton { get; set; }
        /// <summary>A crop of the boarding X as it looks with the window OPEN — the reference the
        /// already-open probe compares against. Session half: the RUN learns it from a window whose
        /// contents it just read, so there is no calibration button for it and the calibration half
        /// must not be expected to carry it. See <see cref="PetConfig.CloseButtonPng"/>.</summary>
        public string? CloseButtonPng { get; set; }
        /// <summary>Whether the window may still be on screen — set on an open, cleared on a SENT
        /// close, left set when the close could not be reached. Session half for the same reason:
        /// it describes the last operation this run performed.</summary>
        public bool? BoardingWindowMayBeOpen { get; set; }
        public List<List<int>>? PageTabs { get; set; }
        public List<int>? BagGrid { get; set; }
        public List<int>? BagSlot { get; set; }
        public List<List<int>>? FoodSlots { get; set; }
        public int? FoodSlotsUsed { get; set; }
        /// <summary>A crop of one food item — the reference the food cells are found with. Session
        /// half: it is captured on the Pet tab, so that tab's Save is what must persist it.</summary>
        public string? FoodIconPng { get; set; }
        public int? ActionWaitMs { get; set; }
        public int? WaitAfterEmptyMinutes { get; set; }
        /// <summary>right_click | drag — see <see cref="Core.FoodLoadMode"/>. Session half: it is set
        /// on the Pet tab, beside the timing boxes, because it belongs to running a feed rather than
        /// to calibrating one.</summary>
        public string? FoodLoadMode { get; set; }
        /// <summary>How sure the reader must be for a line to count as the count. The only part of the
        /// feeder-count reading that is a setting: where the count is read from is derived from each
        /// slot by <see cref="Core.FeederLayout"/>.</summary>
        public double? FeederCountLeftFraction { get; set; }
        public double? FeederCountMinScore { get; set; }
        public bool? ReloadOnStart { get; set; }
        public List<int>? ReturnSlot { get; set; }
        public List<int>? MaxButton { get; set; }
        public List<LocalPetSlot>? Slots { get; set; }
        public List<LocalPetQueueEntry>? Queue { get; set; }

        /// <summary>THE one place a <see cref="PetConfig"/> becomes a LocalPet, and it exists because
        /// there were two.
        ///
        /// The launcher had a Save on each of the two pet tabs, and each built its own explicit field
        /// list. Three things went wrong with that, all of them silent:
        ///
        ///   * `ActionWaitMs` and `WaitAfterEmptyMinutes` were in NEITHER list. The Timing boxes on the
        ///     Pet tab wrote them to the in-memory config, the loader read them back from here, and
        ///     nothing in between ever wrote them — so the values reverted on the next launch, which is
        ///     the same shape as `gem.move_mode` (PROGRESS.md, 2026-09-13).
        ///   * `PetIconRect`, `PetIconPng` and `MaxButton` were missing from Calibrate Pet's list, and
        ///     that Save REPLACED the object — so saving a calibration wiped the queue crops and the
        ///     pet's own MAX.
        ///   * Nothing failed. A missing field is indistinguishable from a field that was never set.
        ///
        /// A single projection is what makes the omission impossible rather than merely fixed:
        /// `EveryLocalPetFieldIsCopiedFromTheConfig` walks these properties by reflection, so adding a
        /// field to this class and forgetting it here fails the build's tests rather than the player's
        /// next session. Both save buttons now call this, so neither can drop what the other wrote.
        /// </summary>
        /// <summary>THE one place a <see cref="PetConfig"/> becomes a LocalPet — the union of the two
        /// scoped writers below.</summary>
        public static LocalPet From(PetConfig p)
        {
            var local = new LocalPet();
            ApplyCalibration(local, p);
            ApplySession(local, p);
            return local;
        }

        /// <summary>The CALIBRATION half — everything marked on Calibrate Pet: the way into the
        /// window, the bag's grid, and each row's geometry.
        ///
        /// Writes ONTO an existing LocalPet rather than replacing it, and that distinction is the
        /// whole safety of the thing. The launcher saves from two tabs, and each is scoped to its own
        /// — so a save has to leave the other half exactly as the file had it. Assigning a fresh
        /// object would blank the half it is not about, which is the bug that cost the player their
        /// food cells twice already.
        ///
        /// The rows merge BY INDEX: geometry is overwritten, but <see cref="LocalPetSlot.BoardingRunning"/>
        /// is left alone, because that belongs to the session half and a calibration save must not
        /// clear it.</summary>
        public static void ApplyCalibration(LocalPet t, PetConfig p)
        {
            t.MenuButton = p.MenuButton;
            t.FeedIcon = p.FeedIcon;
            t.CloseButton = p.CloseButton;
            t.PageTabs = p.PageTabs;
            t.BagGrid = p.BagGrid;
            t.BagSlot = p.BagSlot;
            t.MaxButton = p.MaxButton;

            var merged = new List<LocalPetSlot>();
            for (int i = 0; i < p.Slots.Count; i++)
            {
                var src = p.Slots[i];
                var running = i < (t.Slots?.Count ?? 0) ? t.Slots![i].BoardingRunning : src.BoardingRunning;
                merged.Add(new LocalPetSlot
                {
                    // Carried even though the tick is on the Pet tab: this list is REBUILT from the
                    // fields named here, so anything left out is dropped by a Calibrate save. See
                    // ApplySession for the other half of why this appears twice.
                    Enabled = src.Enabled,
                    ToggleLabel = src.ToggleLabel,
                    BoardingPetSlot = src.BoardingPetSlot,
                    FeederStrip = src.FeederStrip,
                    Stacks = src.Stacks,
                    PetSlotEmptyPng = src.PetSlotEmptyPng,
                    BoardingRunning = running,
                });
            }
            t.Slots = merged;
        }

        /// <summary>The SESSION half — everything the Pet tab owns: the bag marks, the queue, the
        /// timing, and whether each row is boarding. All of it describes THIS run rather than the
        /// machine, which is why it is the half that changes between runs.
        ///
        /// Writes onto an existing LocalPet for the same reason as above, and touches only
        /// <see cref="LocalPetSlot.BoardingRunning"/> within the rows — walking by index and doing
        /// nothing when the counts disagree, because a row added on the other tab is not this half's
        /// to introduce.</summary>
        public static void ApplySession(LocalPet t, PetConfig p)
        {
            t.FoodSlots = p.FoodSlots;
            t.FoodSlotsUsed = p.FoodSlotsUsed;
            t.FoodIconPng = p.FoodIconPng;
            // The close probe's pair: learned and updated by the run itself rather than marked by the
            // player, which is what puts them on THIS half — the session save is what a tool's own
            // state write calls (LauncherService.PersistPetState).
            t.CloseButtonPng = p.CloseButtonPng;
            t.BoardingWindowMayBeOpen = p.BoardingWindowMayBeOpen;
            t.ReturnSlot = p.ReturnSlot;
            t.WaitAfterEmptyMinutes = p.WaitAfterEmptyMinutes;
            t.ActionWaitMs = p.ActionWaitMs;
            t.FoodLoadMode = p.FoodLoadMode;
            // Both of these are edited on the PET TAB, so the session half is what must write them —
            // a scalar carried by the wrong half is a setting that saves fine from one screen and
            // silently reverts from the other, which is the ActionWaitMs bug exactly. The pair above
            // is drawn on Calibrate Pet, so it belongs to that half instead.
            t.FeederCountLeftFraction = p.FeederCountLeftFraction;
            t.FeederCountMinScore = p.FeederCountMinScore;
            t.ReloadOnStart = p.ReloadOnStart;

            t.Queue = p.Queue.Select(q => new LocalPetQueueEntry
            {
                Label = q.Label,
                Rect = q.Rect,
                Png = q.Png,
                Species = q.Species,
            }).ToList();

            if (t.Slots == null) return;
            for (int i = 0; i < p.Slots.Count && i < t.Slots.Count; i++)
            {
                t.Slots[i].BoardingRunning = p.Slots[i].BoardingRunning;
                // Enabled is here AND in ApplyCalibration, and both are needed for different reasons.
                // The tick lives on the Pet tab, so THIS is the projection that persists it — the
                // session save is what a toggle calls. But ApplyCalibration REBUILDS the row list from
                // the fields it names, so a field it does not carry is dropped by the next Calibrate
                // save, which is how a setting goes missing without anything failing.
                t.Slots[i].Enabled = p.Slots[i].Enabled;
            }
        }
    }

    /// <summary>The pet block AS IT WAS WRITTEN BEFORE the breeding rows existed — one flat set of
    /// fields with no `slots:` list, and `pet_cell` / `food_cells` under their old names.
    ///
    /// It is a separate class, and read as a SECOND pass over local.yaml, for one reason: the fields
    /// below have no counterpart on the live <see cref="PetConfig"/> any more, and
    /// `EveryLocalPetFieldIsCopiedFromTheConfig` requires every LocalPet property to have one. Keeping
    /// them on LocalPet would have meant teaching that test to ignore things, which is the beginning of
    /// the guard not working. Here they are clearly marked, read once, and deletable the day no
    /// pre-rows local.yaml exists.
    ///
    /// The values themselves matter: `IgnoreUnmatchedProperties` means an old file does not fail, it
    /// just silently loses the keys nothing maps to. Without this pass, upgrading would have thrown
    /// away the player's whole pet calibration — the shape of loss the spammer migration exists for.
    /// </summary>
    public sealed class LocalPetLegacy
    {
        public string? PetSlotEmptyPng { get; set; }
        public List<int>? ToggleLabel { get; set; }
        public List<int>? BoardingPetSlot { get; set; }
        public List<int>? FeederSlotA { get; set; }
        public List<int>? FeederSlotB { get; set; }
        public bool? BoardingRunning { get; set; }
        public List<int>? PetCell { get; set; }
        public List<List<int>>? FoodCells { get; set; }
        public int? FoodCellsUsed { get; set; }
        public List<int>? PetIconRect { get; set; }
        public string? PetIconPng { get; set; }
    }

    /// <summary>Just enough of local.yaml to reach the old pet block. Deserialized alongside the real
    /// one, never saved.</summary>
    private sealed class LocalOverridesLegacy
    {
        public LocalPetLegacy? Pet { get; set; }
    }

    private static bool IsPoint(List<int>? p) => p is { Count: 2 };

    /// <summary>The player's own spammer rotations. Personal, not machine-specific, but it lives here
    /// for the same reason: defaults.yaml is the template that ships, so anything a player edits
    /// there would be published with the next release.</summary>
    /// <summary>The quest hand-in flows, in local.yaml.
    ///
    /// **The preset VALUES are the config types themselves** — <see cref="QuestPreset"/> and
    /// <see cref="QuestStep"/>, not mirrors of them. That is the same choice LocalBuySell makes, and it
    /// is worth stating why: this projection has silently dropped a field three times, and every one of
    /// those was a hand-written field list. Handing the whole object across leaves no list to forget.
    /// Only the container is declared here.</summary>
    public sealed class LocalQuest
    {
        public string? Active { get; set; }
        public Dictionary<string, QuestPreset>? Presets { get; set; }

        /// <summary>Sequences, carried the same way and for the same reason as the flows: personal, so
        /// they live in local.yaml and not in the file a public build ships. The VALUES are the config
        /// types themselves — no mirror, so there is no field list to forget.</summary>
        public string? ActiveSequence { get; set; }

        public Dictionary<string, QuestSequence>? Sequences { get; set; }

        /// <summary>Carried HERE and not in defaults.yaml, for the reason the tool-level scalars on the
        /// Pet tab had to be: the old save path rewrote defaults.yaml from an explicit field list, so a
        /// setting with no line in that list was written once and silently dropped by the next Save from
        /// any tab. The flows are the personal half and this is the behaviour half, but both are the
        /// Quest tab's to keep.</summary>
        public int? GuardPx { get; set; }
    }

    public sealed class LocalSpammer
    {
        public string? Active { get; set; }
        public Dictionary<string, Dictionary<string, double>>? Presets { get; set; }

        /// <summary>Preset name → the keys that outrank the rest, in order. Personal like the presets
        /// beside it, and carried here for the same reason: it describes a rotation the player tuned,
        /// not something a published build should ship.</summary>
        public Dictionary<string, List<string>>? Priority { get; set; }

        /// <summary>Preset name → that preset's combos, in the order they are tried. Personal, and
        /// carried here for the same reason as the two above.</summary>
        public Dictionary<string, List<SpammerCombo>>? Combos { get; set; }

        /// <summary>Preset name → that preset's opening: lead-in and step. Personal, and carried here
        /// for the same reason as the three above.</summary>
        public Dictionary<string, SpammerStartup>? Startup { get; set; }

        /// <summary>LEGACY, read on load only — the opening step as a bare map, before the lead-in
        /// existed. Folded into <see cref="Startup"/> by the merge below and never written, so it
        /// disappears from the file on the next save.</summary>
        public Dictionary<string, double>? StartupStep { get; set; }
    }

    /// <summary>Windows-specific launcher state: where the window sits, how big it is when expanded,
    /// and whether it floats above other windows. Machine-specific, so it lives here rather than in
    /// defaults.yaml — a second PC wants its own placement.</summary>
    public sealed class LocalUi
    {
        public double? Left { get; set; }
        public double? Top { get; set; }
        public double? Width { get; set; }
        /// <summary>Height to restore when the config region is expanded. The collapsed and mini
        /// heights are computed by the window, not stored.</summary>
        public double? ExpandedHeight { get; set; }
        public bool Pinned { get; set; }
    }

    public sealed class LocalTuner
    {
        public OcrGeometry? Ocr { get; set; }
        /// <summary>[x, y] of the 發條 button, client-relative physical pixels. Machine-specific, so it
        /// lives in local.yaml like the OCR geometry and gem click points.</summary>
        public List<int>? SpringPoint { get; set; }
    }

    public sealed class LocalGem
    {
        public Dictionary<string, List<int>>? GradePositions { get; set; }
        public MovementsConfig? Movements { get; set; }
        public List<int>? ResultGemArea { get; set; }
        public List<List<int>>? ResourceGems { get; set; }
        public ColorComposition? EmptySignature { get; set; }
        public double? EmptyDistance { get; set; }
    }

    public sealed class LocalArduino
    {
        public string? Port { get; set; }
    }
}

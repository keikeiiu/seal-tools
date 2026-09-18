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
    // It says the two things a reader has to know before editing the file, because both have already
    // cost real data. Comments vanish on the next save from any tab. And a key the serializer's
    // anonymous object does not mention is DROPPED - that is not a theory, it is how a machine's
    // spammer presets were deleted outright (the block lived here, a Save from the Tuner tab rewrote
    // the file without it, and nothing said so). Presets live in local.yaml now and the adoption path
    // catches strays, but the underlying behaviour has not changed and will not without a rework.
    private const string DefaultsHeader =
        "# ── Portable settings ─────────────────────────────────────────────────────────────\n" +
        "# The same on every machine. Anything measured on ONE machine belongs in local.yaml, not here.\n" +
        "#\n" +
        "# READ THIS BEFORE EDITING. The launcher REWRITES this whole file when you press Save on the\n" +
        "# Tuner, Gem or Hotkeys tab. It serialises a fixed list of keys, which means:\n" +
        "#   * every comment in here is lost on the next save, including any you add;\n" +
        "#   * any key the launcher does not know about is DROPPED, silently — this is how a spammer\n" +
        "#     block once disappeared, taking a player's key rotations with it. Presets are personal\n" +
        "#     and live in local.yaml, which is why there is no spammer section below.\n" +
        "#\n" +
        "# Documented in docs/CONFIG.md. Keys and their meaning: docs/USER_GUIDE.md.\n" +
        "\n";

    // The same warning for local.yaml. It loses comments the same way, and its comments are the more
    // valuable ones - local.yaml.example ships a page of guidance explaining that the seeded values
    // are v1 starting points and will be wrong until the calibrators run, all of which the first
    // save deletes.
    private const string LocalHeader =
        "# ── Machine-specific settings ─────────────────────────────────────────────────────\n" +
        "# Everything measured or tuned on THIS machine: coordinates, the display environment, the\n" +
        "# Arduino port override, and your own spammer presets. Gitignored, and excluded from the\n" +
        "# public zip by publish.bat — do not commit it or share it.\n" +
        "#\n" +
        "# READ THIS BEFORE EDITING. The launcher REWRITES this whole file when you calibrate or save\n" +
        "# a preset. It serialises a fixed list of keys, so every comment here is lost on the next\n" +
        "# save and any key the launcher does not know about is DROPPED. Keep a copy if you hand-edit.\n" +
        "#\n" +
        "# Documented in docs/CONFIG.md; calibrating is docs/CALIBRATION.md.\n" +
        "\n";

    public AppConfig Load()
    {
        var defaults = Deserialize<AppConfig>("defaults.yaml");

        // First run on a new machine: seed the machine-specific config from the
        // example template (v1 coords as a starting point), then calibrate in-app.
        var localPath = PathOf("local.yaml");
        if (!File.Exists(localPath))
        {
            var examplePath = PathOf("local.yaml.example");
            if (File.Exists(examplePath))
                File.Copy(examplePath, localPath);
        }

        if (!File.Exists(localPath))
        {
            throw new ConfigException(
                "config/local.yaml not found and config/local.yaml.example is missing, so it cannot be created.");
        }

        var local = Deserialize<LocalOverrides>("local.yaml");

        // Order matters: legacy keys become a preset in memory, adoption copies whatever presets are
        // left into local.yaml, and only then does local.yaml override the result.
        MigrateSpammerPresets(defaults);
        AdoptSpammerPresetsIntoLocal(defaults, local);
        ApplyOverrides(defaults, local);

        // AFTER the merge, because the migration only fills gaps: a file that already carries
        // `slots:` must win over the flat keys it also happens to still hold.
        MigratePetSlots(defaults.Pet, Deserialize<LocalOverridesLegacy>("local.yaml").Pet);

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
            FeederSlots = new List<List<int>?> { old.FeederSlotA, old.FeederSlotB }
                .Where(IsValidRect).Select(r => r!).ToList(),
            Stacks = 2,
            BoardingRunning = old.BoardingRunning ?? false,
        });
    }

    private static bool IsValidRect(List<int>? r) => BagGrid.IsValidRect(r);


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
    // SaveDefaults no longer writes them, so the first Save on ANY other tab (tuner, gem and hotkeys
    // all call it) rewrites defaults.yaml without a spammer block. On a machine that only ever ran
    // the older build those presets existed nowhere else, so that Save deleted them outright and the
    // next launch quietly created a blank "default". Adopt them into local.yaml before that can
    // happen.
    //
    // Runs at most once per install: local.Spammer is non-null afterwards, including on a first run
    // where local.yaml was just seeded from the example (which carries no spammer block).
    private void AdoptSpammerPresetsIntoLocal(AppConfig defaults, LocalOverrides local)
    {
        if (local.Spammer != null || defaults.Spammer.Presets is not { Count: > 0 }) return;

        local.Spammer = new LocalSpammer
        {
            Active = defaults.Spammer.Active,
            Presets = defaults.Spammer.Presets,
        };
        SaveLocal(local);
    }

    public AttributesConfig LoadAttributes() => Deserialize<AttributesConfig>("attributes.yaml");

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

    private static void ApplyOverrides(AppConfig defaults, LocalOverrides local)
    {
        if (local.Calibration != null) defaults.Calibration = local.Calibration;
        if (local.Tuner?.Ocr != null) defaults.Tuner.Ocr = local.Tuner.Ocr;
        if (local.Tuner?.SpringPoint != null) defaults.Tuner.SpringPoint = local.Tuner.SpringPoint;
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
            if (pet.PageTabs is { Count: > 0 }) defaults.Pet.PageTabs = pet.PageTabs;
            if (!string.IsNullOrWhiteSpace(pet.PetSlotEmptyPng)) defaults.Pet.PetSlotEmptyPng = pet.PetSlotEmptyPng;
            // The boarding bag's grid, deliberately separate from BuySell's — the bag sits somewhere
            // else in this flow, so borrowing those numbers would aim every cell at the wrong item.
            if (BagGrid.IsValidRect(pet.BagGrid)) defaults.Pet.BagGrid = pet.BagGrid;
            if (BagGrid.IsValidRect(pet.BagSlot)) defaults.Pet.BagSlot = pet.BagSlot;
            if (pet.FoodSlots is { Count: > 0 }) defaults.Pet.FoodSlots = pet.FoodSlots;
            if (pet.FoodSlotsUsed is { } used and >= 0) defaults.Pet.FoodSlotsUsed = used;
            if (pet.ActionWaitMs is { } wait and > 0) defaults.Pet.ActionWaitMs = wait;
            if (pet.WaitAfterEmptyMinutes is { } wait2) defaults.Pet.WaitAfterEmptyMinutes = wait2;
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
        }
    }

    // Atomic write so the launcher can save config while tools re-read it.
    public void SaveLocal(LocalOverrides local)
    {
        var path = PathOf("local.yaml");
        var yaml = LocalHeader + Serializer.Serialize(local);
        var tmp = path + ".tmp";
        File.WriteAllText(tmp, yaml);
        File.Move(tmp, path, overwrite: true);
    }

    // Re-serialize the portable sections to defaults.yaml. Machine-specific parts
    // (ocr geometry, gem positions/movements, port, dpi) live in local.yaml and are excluded.
    public void SaveDefaults(AppConfig cfg)
    {
        var portable = new
        {
            window = cfg.Window,
            reference_window = cfg.ReferenceWindow,
            arduino = new { vid = cfg.Arduino.Vid, pid = cfg.Arduino.Pid, baud = cfg.Arduino.Baud, port = "" },
            hotkeys = cfg.Hotkeys,
            tuner = new
            {
                grade_order = cfg.Tuner.GradeOrder,
                target_grade = cfg.Tuner.TargetGrade,
                max_retries = cfg.Tuner.MaxRetries,
                save_captures = cfg.Tuner.SaveCaptures,
                ocr_retries = cfg.Tuner.OcrRetries,
                spring_mode = cfg.Tuner.SpringMode,
                mouse_guard = cfg.Tuner.MouseGuard,
                guard_px = cfg.Tuner.GuardPx,
                recenter_max = cfg.Tuner.RecenterMax,
                models = cfg.Tuner.Models,
                timing = cfg.Tuner.Timing,
                grade_colors = cfg.Tuner.GradeColors,
                filter = cfg.Tuner.Filter,
            },
            // move_mode belongs here (docs/MOVE-SETS.md says the composer's choice is written to
            // defaults.yaml) but was missing from this list, so every Save silently reset it to the
            // property default. Same shape as the ocr_retries regression; the round-trip test now
            // covers it with a non-default value.
            gem = new { grades = cfg.Gem.Grades, start_grade = cfg.Gem.StartGrade, empty_mode = cfg.Gem.EmptyMode, empty_streak = cfg.Gem.EmptyStreak, colored_gap_min = cfg.Gem.ColoredGapMin, save_empty_captures = cfg.Gem.SaveEmptyCaptures, move_mode = cfg.Gem.MoveMode },
            // spammer is deliberately absent: presets are personal and live in local.yaml. Leaving
            // it here would write the merged in-memory presets back out on every Save Config
            // (tuner, gem, hotkeys all call this), pushing a player's rotations into the file
            // publish.bat ships. The spammer block in defaults.yaml is a static seed, not saved.
        };

        var path = PathOf("defaults.yaml");
        var yaml = DefaultsHeader + Serializer.Serialize(portable);
        var tmp = path + ".tmp";
        File.WriteAllText(tmp, yaml);
        File.Move(tmp, path, overwrite: true);
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
        public List<int>? ToggleLabel { get; set; }
        public List<int>? BoardingPetSlot { get; set; }
        public List<List<int>>? FeederSlots { get; set; }
        public int? Stacks { get; set; }
        public bool? BoardingRunning { get; set; }

        /// <summary>The live shape, from the stored one. A missing `stacks` means TWO — the free
        /// row's count, which is the only row a file written before the rows existed can describe.</summary>
        public PetSlotConfig ToConfig() => new()
        {
            ToggleLabel = ToggleLabel,
            BoardingPetSlot = BoardingPetSlot,
            FeederSlots = FeederSlots ?? new(),
            Stacks = Stacks ?? 2,
            BoardingRunning = BoardingRunning ?? false,
        };
    }

    /// <summary>One queued pet, as it is written to local.yaml.</summary>
    public sealed class LocalPetQueueEntry
    {
        public string? Label { get; set; }
        public List<int>? Rect { get; set; }
        public string? Png { get; set; }

        public PetQueueEntry ToConfig() => new() { Label = Label, Rect = Rect, Png = Png };
    }

    /// <summary>The pet food auto-replacement geometry (see <see cref="PetConfig"/>). Machine-specific
    /// in full — including the bag grid, which is NOT the same one the shop opens beside.</summary>
    public sealed class LocalPet
    {
        public List<int>? MenuButton { get; set; }
        public List<int>? FeedIcon { get; set; }
        public List<int>? CloseButton { get; set; }
        public List<List<int>>? PageTabs { get; set; }
        public string? PetSlotEmptyPng { get; set; }
        public List<int>? BagGrid { get; set; }
        public List<int>? BagSlot { get; set; }
        public List<List<int>>? FoodSlots { get; set; }
        public int? FoodSlotsUsed { get; set; }
        public int? ActionWaitMs { get; set; }
        public int? WaitAfterEmptyMinutes { get; set; }
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
        public static LocalPet From(PetConfig p) => new()
        {
            MenuButton = p.MenuButton,
            FeedIcon = p.FeedIcon,
            CloseButton = p.CloseButton,
            PageTabs = p.PageTabs,
            PetSlotEmptyPng = p.PetSlotEmptyPng,
            BagGrid = p.BagGrid,
            BagSlot = p.BagSlot,
            FoodSlots = p.FoodSlots,
            FoodSlotsUsed = p.FoodSlotsUsed,
            ReturnSlot = p.ReturnSlot,
            MaxButton = p.MaxButton,
            WaitAfterEmptyMinutes = p.WaitAfterEmptyMinutes,
            ActionWaitMs = p.ActionWaitMs,
            Slots = p.Slots.Select(s => new LocalPetSlot
            {
                ToggleLabel = s.ToggleLabel,
                BoardingPetSlot = s.BoardingPetSlot,
                FeederSlots = s.FeederSlots,
                Stacks = s.Stacks,
                BoardingRunning = s.BoardingRunning,
            }).ToList(),
            Queue = p.Queue.Select(q => new LocalPetQueueEntry
            {
                Label = q.Label,
                Rect = q.Rect,
                Png = q.Png,
            }).ToList(),
        };
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
    public sealed class LocalSpammer
    {
        public string? Active { get; set; }
        public Dictionary<string, Dictionary<string, double>>? Presets { get; set; }
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

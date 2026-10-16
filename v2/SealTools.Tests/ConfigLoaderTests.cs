using System;
using System.IO;
using SealTools.Core;
using SealTools.Core.Config;
using Xunit;
using YamlDotNet.RepresentationModel;

namespace SealTools.Tests;

public class ConfigLoaderTests
{
    // Walk up from the test output dir to find the v2/config directory in the repo.
    private static string FindConfigDir()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null)
        {
            var candidate = Path.Combine(dir.FullName, "config");
            if (File.Exists(Path.Combine(candidate, "defaults.yaml")))
                return candidate;
            dir = dir.Parent;
        }
        throw new InvalidOperationException("Could not locate the v2/config directory.");
    }

    private static string MakeTempConfigDir(bool includeLocal)
    {
        var src = FindConfigDir();
        var dst = Path.Combine(Path.GetTempPath(), "sealtools_test_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dst);
        File.Copy(Path.Combine(src, "defaults.yaml"), Path.Combine(dst, "defaults.yaml"));
        File.Copy(Path.Combine(src, "attributes.yaml"), Path.Combine(dst, "attributes.yaml"));
        if (includeLocal)
            File.Copy(Path.Combine(src, "local.yaml.example"), Path.Combine(dst, "local.yaml"));
        return dst;
    }

    [Fact]
    public void LoadMergesLocalOverridesIntoDefaults()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            var cfg = new ConfigLoader(dir).Load();

            // portable defaults
            Assert.Equal("TW_LIVE", cfg.Window.Title);
            Assert.Equal(0x2341, cfg.Arduino.Vid);
            Assert.Equal("DG", cfg.Tuner.TargetGrade);
            Assert.Equal(999999, cfg.Tuner.MaxRetries);
            Assert.Equal(0.4, cfg.Tuner.Timing.ClickEnterDelay);

            // machine-specific from local.yaml
            Assert.Equal(1140, cfg.Tuner.Ocr.Region.Left);
            Assert.Equal(320, cfg.Tuner.Ocr.Region.Height);
            Assert.Equal(25, cfg.Tuner.Ocr.RowHeight);
            Assert.Equal(727, cfg.Gem.GradePositions["N"][0]);
            Assert.Equal(696, cfg.Gem.GradePositions["N"][1]);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // ── The portable settings block (PLAN-DEPLOY.md step 1) ─────────────────────────────────────────
    //
    // The settings block is the same portable data in the machine's own file, so that one day the save
    // target can move there and an update cannot cover it. Nothing calls SaveSettings yet; these exist
    // first because the failure they guard is a value written to a file and then silently discarded on
    // the way back — which is how ocr_retries, gem.move_mode, a hand-added pet: block and a whole
    // spammer block were each lost once.

    /// <summary>Every leaf path in a YAML file, dotted — enough to compare what two writers emit without
    /// depending on their types, which is the only way to fence an anonymous projection.</summary>
    private static List<string> YamlPaths(string file, string? onlyUnder = null)
    {
        var stream = new YamlStream();
        using (var reader = new StreamReader(file))
            stream.Load(reader);

        var paths = new List<string>();
        Walk(stream.Documents[0].RootNode, "");
        return onlyUnder == null
            ? paths
            : paths.Where(p => p.StartsWith(onlyUnder + ".", StringComparison.Ordinal))
                   .Select(p => p[(onlyUnder.Length + 1)..]).ToList();

        void Walk(YamlNode node, string prefix)
        {
            switch (node)
            {
                case YamlMappingNode map when map.Children.Count > 0:
                    foreach (var (key, value) in map.Children)
                        Walk(value, prefix.Length == 0 ? key.ToString() : $"{prefix}.{key}");
                    break;
                case YamlMappingNode:
                    paths.Add(prefix + " {}");
                    break;
                case YamlSequenceNode seq when seq.Children.Count > 0:
                    Walk(seq.Children[0], prefix + "[]");
                    break;
                case YamlSequenceNode:
                    paths.Add(prefix + " []");
                    break;
                default:
                    paths.Add(prefix);
                    break;
            }
        }
    }

    private static YamlNode? Child(YamlNode? node, string key)
    {
        if (node is not YamlMappingNode map) return null;
        foreach (var (k, v) in map.Children)
            if (k.ToString() == key) return v;
        return null;
    }

    /// <summary>True when a section carries no value at all. This writer emits a null as a BARE key, so
    /// an unset section is present in the file as an empty scalar — the same shape the real local.yaml
    /// already gives its `spring_point:` and `startup_step:`. "Carries nothing" is therefore the right
    /// test for "the fold did not bring this section across"; "absent" would be wrong.</summary>
    private static bool CarriesNothing(YamlNode? node) => node switch
    {
        null => true,
        YamlScalarNode scalar => string.IsNullOrWhiteSpace(scalar.Value),
        YamlMappingNode map => map.Children.Count == 0,
        _ => false,
    };

    /// <summary>The settings subtree of a local.yaml, or null when the file carries none.</summary>
    private static YamlNode? SettingsNode(string file) => Child(RootNode(file), "settings");

    private static YamlNode RootNode(string file)
    {
        var stream = new YamlStream();
        using (var reader = new StreamReader(file))
            stream.Load(reader);
        return stream.Documents[0].RootNode;
    }

    // ── Step 2: the base rule, the fold, and the root marker (PLAN-DEPLOY.md §4, §6) ─────────────────
    //
    // An install that takes an update gets config/defaults.yaml.example — the shipped template — beside
    // its own config/defaults.yaml, which holds every setting the player ever saved. The first launch
    // after that moves their own values into local.yaml once and stops reading the old file; the tests
    // below are the whole of that contract.

    /// <summary>The shipped defaults as this release writes them — the file the zip carries under
    /// config/defaults.yaml.example. Using the repo's own file rather than a hand-made stub is what makes
    /// these tests exercise the real key set, comments and all.</summary>
    private static string ShippedDefaults() => File.ReadAllText(Path.Combine(FindConfigDir(), "defaults.yaml"));

    /// <summary>A temp install AFTER an update has landed: the shipped template beside the player's own
    /// legacy defaults.yaml, and a seeded local.yaml. `legacy` defaults to the shipped text, so a test
    /// names only the values it wants the player to have changed.</summary>
    private static string MakeTempInstall(string shipped, string? legacy = null)
    {
        var dir = Path.Combine(Path.GetTempPath(), "sealtools_install_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dir);
        var src = FindConfigDir();
        File.Copy(Path.Combine(src, "attributes.yaml"), Path.Combine(dir, "attributes.yaml"));
        File.WriteAllText(Path.Combine(dir, "defaults.yaml.example"), shipped);
        File.WriteAllText(Path.Combine(dir, "defaults.yaml"), legacy ?? shipped);
        File.Copy(Path.Combine(src, "local.yaml.example"), Path.Combine(dir, "local.yaml"));
        return dir;
    }

    [Fact]
    public void TheFoldCarriesOnlyWhatThePlayerChanged()
    {
        var shipped = ShippedDefaults();
        var dir = MakeTempInstall(shipped, shipped.Replace("target_grade: DG", "target_grade: S"));
        try
        {
            var legacyBytes = File.ReadAllBytes(Path.Combine(dir, "defaults.yaml"));

            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal("S", cfg.Tuner.TargetGrade);

            var settings = SettingsNode(Path.Combine(dir, "local.yaml"));
            Assert.NotNull(settings);
            // The section holding the change was carried whole — the apply side assigns the object, so a
            // lone `target_grade` leaf would have dragged the other twelve tuner defaults in with it.
            Assert.False(CarriesNothing(Child(settings, "tuner")));
            Assert.Equal("S", Child(Child(settings, "tuner"), "target_grade")?.ToString());
            Assert.Equal("3", Child(Child(settings, "tuner"), "ocr_retries")?.ToString());
            // And the sections that do NOT differ were not carried at all, which is the point of the
            // whole design: they go on tracking the shipped template, so a default a later release
            // changes still reaches this machine.
            Assert.True(CarriesNothing(Child(settings, "window")));
            Assert.True(CarriesNothing(Child(settings, "hotkeys")));
            Assert.True(CarriesNothing(Child(settings, "gem")));
            Assert.True(CarriesNothing(Child(settings, "pet")));
            Assert.True(CarriesNothing(Child(settings, "arduino")));

            // The legacy file is the rollback copy: never written, never moved, never deleted.
            Assert.Equal(legacyBytes, File.ReadAllBytes(Path.Combine(dir, "defaults.yaml")));
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void TheFoldMarksItselfDoneEvenWhenNothingDiffers()
    {
        // A player who never changed a value: nothing to carry, and the marker still has to be set — or
        // the next release's launch would fold this template against that release's example and pin
        // every default it changed as the player's own.
        var shipped = ShippedDefaults();
        var dir = MakeTempInstall(shipped);
        try
        {
            new ConfigLoader(dir).Load();

            var settings = SettingsNode(Path.Combine(dir, "local.yaml"));
            Assert.Equal("true", Child(settings, "legacy_folded")?.ToString());
            Assert.True(CarriesNothing(Child(settings, "tuner")));
            Assert.True(CarriesNothing(Child(settings, "window")));
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void TheFoldRunsOnceAndTheLegacyFileIsNeverReadAgain()
    {
        var shipped = ShippedDefaults();
        var dir = MakeTempInstall(shipped, shipped.Replace("target_grade: DG", "target_grade: S"));
        try
        {
            var first = new ConfigLoader(dir).Load();
            Assert.Equal("S", first.Tuner.TargetGrade);

            var afterFirstLoad = File.ReadAllBytes(Path.Combine(dir, "local.yaml"));

            // A second launch writes nothing at all: the marker means the legacy file is not read.
            var second = new ConfigLoader(dir).Load();
            Assert.Equal("S", second.Tuner.TargetGrade);
            Assert.Equal(afterFirstLoad, File.ReadAllBytes(Path.Combine(dir, "local.yaml")));

            // Which is decisive with the legacy file GONE: the value cannot have come from it.
            File.Delete(Path.Combine(dir, "defaults.yaml"));
            Assert.Equal("S", new ConfigLoader(dir).Load().Tuner.TargetGrade);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void AChangedShippedDefaultIsCarriedAcrossAsThePlayersOwn()
    {
        // The fold's approximation, pinned so nobody later mistakes it for a bug: it reads "the player
        // changed it" as "it differs from the shipped template", so a default a RELEASE changed, under a
        // key this player never touched, is carried across as if they had chosen it. The direction is the
        // safe one — a saved value beats a silent behaviour change — and the release note that names the
        // key is the habit that covers it.
        var shippedNow = ShippedDefaults().Replace("target_grade: DG", "target_grade: S");
        var legacyFromTheOldRelease = ShippedDefaults();
        var dir = MakeTempInstall(shippedNow, legacyFromTheOldRelease);
        try
        {
            Assert.Equal("DG", new ConfigLoader(dir).Load().Tuner.TargetGrade);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void TheFoldLeavesTheMachineHalfOfLocalYamlAlone()
    {
        var shipped = ShippedDefaults();
        var dir = MakeTempInstall(shipped, shipped.Replace("target_grade: DG", "target_grade: S"));
        try
        {
            var before = new ConfigLoader(dir).LoadLocal()!;
            Assert.NotNull(before.Tuner?.Ocr);

            new ConfigLoader(dir).Load();

            // The fold writes local.yaml, which is also where this machine's calibration lives — the one
            // thing in the install that cannot be regenerated from the repo.
            var after = new ConfigLoader(dir).LoadLocal()!;
            Assert.Equal(before.Tuner!.Ocr!.Region.Left, after.Tuner!.Ocr!.Region.Left);
            Assert.Equal(before.Gem!.GradePositions!.Count, after.Gem!.GradePositions!.Count);
            Assert.NotNull(after.Settings);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void AFoldThatCannotBeWrittenStillLoadsTheLegacyValues()
    {
        var shipped = ShippedDefaults();
        var dir = MakeTempInstall(shipped, shipped.Replace("target_grade: DG", "target_grade: S"));
        var localPath = Path.Combine(dir, "local.yaml");
        try
        {
            // A read-only local.yaml stands in for the medium the plan names: the atomic writer's Move
            // cannot replace it, so the fold fails the way it would on a read-only stick or a full disk.
            File.SetAttributes(localPath, FileAttributes.ReadOnly);
            var untouched = File.ReadAllBytes(localPath);

            // Nothing throws...
            var cfg = new ConfigLoader(dir).Load();

            // ...the legacy values are still in force for this session, from the fallback layer...
            Assert.Equal("S", cfg.Tuner.TargetGrade);
            // ...the marker was not set, so the fold retries on the next launch...
            Assert.Equal(untouched, File.ReadAllBytes(localPath));
            // ...and the failed write left no litter.
            Assert.Empty(Directory.GetFiles(dir, "local.yaml.*.tmp"));
        }
        finally
        {
            File.SetAttributes(localPath, FileAttributes.Normal);
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void ACheckoutWithNoShippedTemplateReadsExactlyWhatItDidBefore()
    {
        // No defaults.yaml.example: a repo checkout, and every install that has not taken an update.
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal("TW_LIVE", cfg.Window.Title);
            Assert.Equal("DG", cfg.Tuner.TargetGrade);

            // And no fold ran: without a shipped template there are not two files to compare.
            Assert.Null(new ConfigLoader(dir).LoadLocal()!.Settings);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void AFreshInstallWithOnlyTheShippedTemplateLoads()
    {
        var dir = Path.Combine(Path.GetTempPath(), "sealtools_fresh_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dir);
        var src = FindConfigDir();
        File.Copy(Path.Combine(src, "attributes.yaml"), Path.Combine(dir, "attributes.yaml"));
        File.Copy(Path.Combine(src, "defaults.yaml"), Path.Combine(dir, "defaults.yaml.example"));
        File.Copy(Path.Combine(src, "local.yaml.example"), Path.Combine(dir, "local.yaml.example"));
        try
        {
            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal("TW_LIVE", cfg.Window.Title);
            // local.yaml was seeded, and no fold ran: there is no legacy file to fold.
            Assert.True(File.Exists(Path.Combine(dir, "local.yaml")));
            Assert.Null(new ConfigLoader(dir).LoadLocal()!.Settings);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void TheRootMarkerAcceptsEitherDefaultsName()
    {
        var dir = Path.Combine(Path.GetTempPath(), "sealtools_root_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path.Combine(dir, "config"));
        try
        {
            Assert.False(RootDir.IsRoot(dir));

            File.WriteAllText(Path.Combine(dir, "config", "defaults.yaml"), "window:\n");
            Assert.True(RootDir.IsRoot(dir));
            File.Delete(Path.Combine(dir, "config", "defaults.yaml"));

            // The name an install has after an update, and the reason the probe had to widen: without it
            // a folder that has taken one is no longer recognisable as an install at all.
            File.WriteAllText(Path.Combine(dir, "config", "defaults.yaml.example"), "window:\n");
            Assert.True(RootDir.IsRoot(dir));

            // And the walk finds it from below, which is what a dev run needs.
            var nested = Path.Combine(dir, "bin", "Debug", "net8.0-windows");
            Directory.CreateDirectory(nested);
            Assert.Equal(dir, RootDir.Find(nested));
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>The packaging rule, checked from the suite as well as from the script: the public
    /// branch's explicit copy list is the enforcement seam, so a file the app writes must never be named
    /// there. Precedent for reading a real repo file from a test: FirmwareSketchTests reads the .ino.</summary>
    [Fact]
    public void ThePublicZipListCarriesNoFileThePlayerOwns()
    {
        var path = "";
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            var candidate = Path.Combine(dir.FullName, "v2", "publish.bat");
            if (File.Exists(candidate)) { path = candidate; break; }
        }

        Assert.False(string.IsNullOrEmpty(path), "v2/publish.bat was not found by walking up from " + AppContext.BaseDirectory);
        var lines = File.ReadAllLines(path);

        var fromConfig = lines
            .Select(l => l.Trim())
            .Where(l => l.StartsWith("copy /Y config\\", StringComparison.Ordinal))
            .Select(l => l["copy /Y config\\".Length..].Split(' ')[0])
            .ToList();

        // Exactly the four a fresh install needs and nothing it writes. A fifth line for a file the app
        // owns fails here — the same rule the script's own allow-list check enforces against the zip.
        var expected = new List<string>
        {
            "attributes.yaml", "defaults.yaml", "local.yaml.example", "attributes.us.yaml.example",
        };
        Assert.Equal(expected, fromConfig);

        // The shipped defaults go out under the TEMPLATE name, so an extract cannot cover the player's
        // own defaults.yaml — which is where every setting they ever saved lives.
        Assert.Contains(lines, l => l.Contains(@"config\defaults.yaml      %PUB%\config\defaults.yaml.example", StringComparison.Ordinal));

        // And the feeding table, which was in no zip until this release.
        Assert.Contains(lines, l => l.TrimStart().StartsWith("copy /Y docs\\pet-data.csv", StringComparison.Ordinal));
    }

    /// <summary>The fence for the mirror, and it compares what the two WRITERS actually emit rather than
    /// two lists of names kept in step by hand: the projection into defaults.yaml, the mirror into
    /// local.yaml's settings block. A field added to one and forgotten in the other fails here — which is
    /// the whole reason the mirror exists rather than a second hand-written list.
    ///
    /// Two differences are deliberate and named:
    ///   arduino.port — the projection writes it (as "") because a SHIPPED file must clear whatever port
    ///     the builder's machine had; the mirror must not carry it, because a port is not portable and
    ///     local.yaml keeps this machine's own in the arduino block beside it.
    ///   legacy_folded — the fold's marker, which has no counterpart in a shipped file.</summary>
    /// The reference is the TEMPLATE the repo ships — the file publish.bat packages as
    /// config/defaults.yaml.example and the app reads as its base — rather than a second writer's output.
    /// Nothing writes it any more, so it is a stable reference instead of two things to keep in step.</summary>
    [Fact]
    public void TheSettingsMirrorCarriesExactlyTheShippedTemplatesFields()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            loader.SaveSettings(loader.Load());

            var shipped = YamlPaths(Path.Combine(FindConfigDir(), "defaults.yaml"));
            var mirrored = YamlPaths(Path.Combine(dir, "local.yaml"), onlyUnder: "settings");

            var missingFromMirror = shipped.Except(mirrored).Where(p => p != "arduino.port").ToList();
            var extraInMirror = mirrored.Except(shipped).Where(p => p != "legacy_folded").ToList();

            Assert.Empty(missingFromMirror);
            Assert.Empty(extraInMirror);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>The same list the test above walks, pointed at the new path and READ BACK: every portable
    /// section, set to a value that differs from its default so a missing link cannot pass. The existing
    /// trip through defaults.yaml is <see cref="SaveDefaultsPreservesEveryPortableField"/>; this is the
    /// trip through local.yaml, which is the one the update cannot cover.</summary>
    [Fact]
    public void SaveSettingsPreservesEveryPortableField()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            var before = loader.Load();

            before.Window.Title = "MY_WINDOW";
            before.ReferenceWindow.Width = 111;
            before.ReferenceWindow.Height = 222;
            before.Game.Variant = "us";
            before.Arduino.Vid = 0x1234;
            before.Arduino.Pid = new List<int> { 5, 6 };
            before.Arduino.Baud = 57600;
            before.Hotkeys.Start = 65;
            before.Hotkeys.Quit = 66;
            before.Hotkeys.AdvanceGrade = 67;
            before.Hotkeys.Pause = 68;
            before.Tuner.GradeOrder = new List<string> { "S", "A" };
            before.Tuner.TargetGrade = "S";
            before.Tuner.MaxRetries = 4;
            before.Tuner.SaveCaptures = true;
            before.Tuner.OcrRetries = 5;
            before.Tuner.SpringMode = "hid";
            before.Tuner.MouseGuard = "stop";
            before.Tuner.GuardPx = 11;
            before.Tuner.RecenterMax = 7;
            before.Tuner.Models.Detector = "det.onnx";
            before.Tuner.Timing.OcrDelay = 0.75;
            before.Tuner.GradeColors.Yellow.RMin = 77;
            before.Tuner.Filter.Enabled = true;
            before.Gem.Grades = new List<string> { "A", "B" };
            before.Gem.StartGrade = "A";
            before.Gem.EmptyMode = "advance_grade";
            before.Gem.EmptyStreak = 4;
            before.Gem.ColoredGapMin = 55;
            before.Gem.SaveEmptyCaptures = true;
            before.Gem.MoveMode = "tuned";
            before.Pet.ItemsPerMinute = 7;
            before.Pet.PetSlotOccupiedAbove = 0.25;

            loader.SaveSettings(before);
            var after = new ConfigLoader(dir).Load();

            Assert.Equal("MY_WINDOW", Child(Child(SettingsNode(Path.Combine(dir, "local.yaml")), "window"), "title")?.ToString());
            Assert.Equal(111, after.ReferenceWindow.Width);
            Assert.Equal(222, after.ReferenceWindow.Height);
            // window.title is the RESOLVED copy when a variant is set — the variant is the authority, as
            // the defaults.yaml test above records — so the title to compare is the one it resolves to.
            Assert.Equal("US_LIVE", after.Window.Title);
            Assert.Equal("us", after.Game.Variant);
            Assert.Equal("attributes.us.yaml", after.Game.Variants["us"].Attributes);
            Assert.Equal(0x1234, after.Arduino.Vid);
            Assert.Equal(new List<int> { 5, 6 }, after.Arduino.Pid);
            Assert.Equal(57600, after.Arduino.Baud);
            Assert.Equal(65, after.Hotkeys.Start);
            Assert.Equal(66, after.Hotkeys.Quit);
            Assert.Equal(67, after.Hotkeys.AdvanceGrade);
            Assert.Equal(68, after.Hotkeys.Pause);
            Assert.Equal(new List<string> { "S", "A" }, after.Tuner.GradeOrder);
            Assert.Equal("S", after.Tuner.TargetGrade);
            Assert.Equal(4, after.Tuner.MaxRetries);
            Assert.True(after.Tuner.SaveCaptures);
            Assert.Equal(5, after.Tuner.OcrRetries);
            Assert.Equal("hid", after.Tuner.SpringMode);
            Assert.Equal("stop", after.Tuner.MouseGuard);
            Assert.Equal(11, after.Tuner.GuardPx);
            Assert.Equal(7, after.Tuner.RecenterMax);
            Assert.Equal("det.onnx", after.Tuner.Models.Detector);
            Assert.Equal(0.75, after.Tuner.Timing.OcrDelay);
            Assert.Equal(77, after.Tuner.GradeColors.Yellow.RMin);
            Assert.True(after.Tuner.Filter.Enabled);
            Assert.Equal(new List<string> { "A", "B" }, after.Gem.Grades);
            Assert.Equal("A", after.Gem.StartGrade);
            Assert.Equal("advance_grade", after.Gem.EmptyMode);
            Assert.Equal(4, after.Gem.EmptyStreak);
            Assert.Equal(55, after.Gem.ColoredGapMin);
            Assert.True(after.Gem.SaveEmptyCaptures);
            Assert.Equal("tuned", after.Gem.MoveMode);
            Assert.Equal(7, after.Pet.ItemsPerMinute);
            Assert.Equal(0.25, after.Pet.PetSlotOccupiedAbove);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>With NO variant the title stands on its own through the new path too — the case the
    /// variant would otherwise hide.</summary>
    [Fact]
    public void WithNoVariantTheSettingsTitleStillRoundTrips()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            var cfg = loader.Load();
            cfg.Game.Variant = "";
            cfg.Window.Title = "MY_OWN_TITLE";

            loader.SaveSettings(cfg);

            Assert.Equal("MY_OWN_TITLE", new ConfigLoader(dir).Load().Window.Title);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>The block goes to the machine's file and nowhere else. defaults.yaml is what publish.bat
    /// copies into a release zip, so a save that touched it would be publishing one machine's settings —
    /// and until step 4 of the plan lands it is still the file the launcher reads its values from, so a
    /// write here would also be a behaviour change in a step that promises none.</summary>
    [Fact]
    public void SaveSettingsWritesLocalYamlAndLeavesDefaultsAlone()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            var cfg = loader.Load();
            var defaultsPath = Path.Combine(dir, "defaults.yaml");
            var untouched = File.ReadAllBytes(defaultsPath);

            cfg.Tuner.GuardPx = 13;
            loader.SaveSettings(cfg);

            Assert.Equal(untouched, File.ReadAllBytes(defaultsPath));
            Assert.Equal("13", Child(Child(SettingsNode(Path.Combine(dir, "local.yaml")), "tuner"), "guard_px")?.ToString());
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>2026-10-13: the client switch rewrote defaults.yaml and left a stray key in it. The cause
    /// is GameConfig.Active — a reference INTO Variants — which the serialiser writes twice, the second
    /// time as a YAML anchor/alias pair (`us: &amp;o0 … active: *o0`). The mirror names the variant and the
    /// table and never the resolved object, and this pins that with a variant actually set, because with
    /// none the alias cannot appear at all.
    ///
    /// Scoped to the settings subtree on purpose: `active:` is a legitimate key elsewhere in local.yaml
    /// (the spammer's active preset), so the whole-file check would fail for the wrong reason.</summary>
    [Fact]
    public void TheSettingsBlockNamesTheVariantAndNeverAliasesIt()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            var cfg = loader.Load();
            cfg.Game.Variant = "us";

            loader.SaveSettings(cfg);

            var game = Child(SettingsNode(Path.Combine(dir, "local.yaml")), "game");
            Assert.NotNull(game);
            Assert.Null(Child(game, "active"));
            Assert.Equal("us", Child(game, "variant")?.ToString());
            var written = File.ReadAllText(Path.Combine(dir, "local.yaml"));
            Assert.DoesNotContain("&o", written);
            Assert.DoesNotContain("*o", written);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>A PARTIAL block — which is what the fold writes, since it carries only the values the
    /// player changed — must leave everything it does not mention standing, and must still apply a value
    /// it DOES state when that value is zero or empty. This is the rule the whole mirror is shaped
    /// around: with non-nullable fields, "not stated" and "zero" are the same thing, which is how a
    /// projection has blanked a setting four times in this repo (ocr_retries 3 → 0 among them).</summary>
    [Fact]
    public void APartialSettingsBlockLeavesTheRestStanding()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            var before = loader.Load();

            File.AppendAllText(Path.Combine(dir, "local.yaml"),
                "\nsettings:\n  tuner:\n    guard_px: 33\n  gem:\n    colored_gap_min: 0\n");

            var after = new ConfigLoader(dir).Load();

            Assert.Equal(33, after.Tuner.GuardPx);
            // Stated as ZERO on purpose: 0 is a legal colored_gap_min, so a `> 0` guard would have read
            // a deliberate value as silence and quietly left the shipped one in place.
            Assert.Equal(0, after.Gem.ColoredGapMin);
            // And every value the block does not mention is untouched.
            Assert.Equal(before.Tuner.MaxRetries, after.Tuner.MaxRetries);
            Assert.Equal(before.Tuner.SpringMode, after.Tuner.SpringMode);
            Assert.Equal(before.Tuner.Timing.OcrDelay, after.Tuner.Timing.OcrDelay);
            Assert.Equal(before.Gem.MoveMode, after.Gem.MoveMode);
            Assert.Equal(before.Window.Title, after.Window.Title);
            Assert.Equal(before.Hotkeys.Start, after.Hotkeys.Start);
            Assert.Equal(before.Pet.ItemsPerMinute, after.Pet.ItemsPerMinute);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>A top-level block the app does not own survives a save, verbatim and with its comment.
    /// A save re-serialises an object graph, so before this it DELETED anything the class does not model
    /// — the same silent-deletion shape as the four recorded field losses, one level up.</summary>
    [Fact]
    public void ATopLevelBlockTheAppDoesNotOwnSurvivesASave()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var localPath = Path.Combine(dir, "local.yaml");
            File.AppendAllText(localPath,
                "\n# My own notes about this machine, which nothing should delete.\nmy_notes:\n  monitor: \"left\"\n  notes:\n  - and a list\n");

            var loader = new ConfigLoader(dir);
            var cfg = loader.Load();
            cfg.Tuner.GuardPx = 17;
            loader.SaveSettings(cfg);

            var written = File.ReadAllText(localPath);
            Assert.Contains("my_notes:", written);
            // Quoted in the input and quoted in the output, which is the real assertion: the block is
            // carried through as TEXT, not parsed and re-emitted — a re-serialisation would drop the
            // quotes and any other styling the author chose.
            Assert.Contains("monitor: \"left\"", written);
            Assert.Contains("- and a list", written);
            Assert.Contains("# My own notes about this machine", written);      // comment included
            Assert.Equal("17", Child(Child(SettingsNode(localPath), "tuner"), "guard_px")?.ToString());
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>The other half of that rule: a block the app DOES own is still rewritten, so the safety
    /// above is not bought by making saves do nothing.</summary>
    [Fact]
    public void ABlockTheAppOwnsIsStillRewritten()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var localPath = Path.Combine(dir, "local.yaml");
            File.AppendAllText(localPath, "\nui:\n  left: 111\n  pinned: false\n");

            var loader = new ConfigLoader(dir);
            loader.UpdateLocal(l => l.Ui = new ConfigLoader.LocalUi { Left = 222, Pinned = true });

            var local = new ConfigLoader(dir).LoadLocal()!;
            Assert.Equal(222, local.Ui!.Left);
            Assert.True(local.Ui.Pinned);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>The owned-block list is a fixed list too, and this is its fence: every property of
    /// LocalOverrides must be named in it, or a block added to the class would be treated as foreign —
    /// preserved by accident and never updated by any save.</summary>
    [Fact]
    public void EveryLocalOverridesBlockIsInTheOwnedList()
    {
        foreach (var prop in typeof(ConfigLoader.LocalOverrides).GetProperties())
        {
            // The same underscoring the YAML writer applies: BuySell -> buy_sell, Ui -> ui.
            var name = "";
            for (var i = 0; i < prop.Name.Length; i++)
            {
                var c = prop.Name[i];
                if (char.IsUpper(c) && i > 0 && char.IsLower(prop.Name[i - 1])) name += "_";
                name += char.ToLowerInvariant(c);
            }

            Assert.Contains(name, ConfigLoader.OwnedLocalBlocks);
        }
    }

    /// <summary>The reason the writer shape changed, as a test. Two writers that each change a different
    /// part of local.yaml must both survive; before the gated read-modify-write the second one saved the
    /// snapshot it had loaded earlier and deleted the first one's block — silently, from two threads that
    /// both reported success (the WPF thread saving a calibration, and the pet tool's PersistPetState).</summary>
    [Fact]
    public void TwoWritersEachKeepTheOthersBlock()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            var cfg = loader.Load();
            cfg.Tuner.GuardPx = 13;
            loader.SaveSettings(cfg);

            // The other writer, which loaded nothing and knows only its own block.
            loader.UpdateLocal(l => l.Ui = new ConfigLoader.LocalUi { Left = 456, Pinned = true });

            var local = new ConfigLoader(dir).LoadLocal()!;
            Assert.Equal(456, local.Ui!.Left);
            Assert.Equal(13, local.Settings!.Tuner!.GuardPx);
            Assert.Equal(13, new ConfigLoader(dir).Load().Tuner.GuardPx);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // Writes a custom local.yaml so a broken calibration can be exercised directly.
    private static string MakeTempConfigDirWithLocal(string localYaml)
    {
        var dir = MakeTempConfigDir(includeLocal: false);
        File.WriteAllText(Path.Combine(dir, "local.yaml"), localYaml);
        return dir;
    }

    private const string ValidOcrLocal =
        "tuner:\n  ocr:\n    region: {left: 0, top: 0, width: 300, height: 320}\n" +
        "    grade_area: {x1: 10, y1: 10, x2: 100, y2: 40}\n" +
        "    grade_y: [10, 40]\n    attr_y: [42, 140]\n    remaining_y: [190, 235]\n" +
        "    row_height: 25\n" +
        "gem:\n  grade_positions:\n    N: [730, 698]\n";

    [Fact]
    public void LoadRejectsTruncatedOcrBand()
    {
        // grade_y has one entry; OcrEngine indexes [0] and [1].
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal.Replace("grade_y: [10, 40]", "grade_y: [10]"));
        try
        {
            var ex = Assert.Throws<ConfigException>(() => new ConfigLoader(dir).Load());
            Assert.Contains("grade_y", ex.Message);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void LoadRejectsGradeAreaOutsideRegion()
    {
        // grade_area x2 (400) exceeds the 300 px region width.
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal.Replace("x2: 100", "x2: 400"));
        try
        {
            var ex = Assert.Throws<ConfigException>(() => new ConfigLoader(dir).Load());
            Assert.Contains("grade_area", ex.Message);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void LoadAcceptsValidOcrGeometry()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var cfg = new ConfigLoader(dir).Load();
            Assert.Equal(25, cfg.Tuner.Ocr.RowHeight);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void UpdateLocalPersistsTheCalibrationEnvironmentBlock()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            var loader = new ConfigLoader(dir);
            loader.UpdateLocal(local => local.Calibration = new CalibrationInfo
            {
                DpiScale = 1.5,
                MonitorDpi = 144,
                Screen = new System.Collections.Generic.List<int> { 3840, 2160 },
                ClientSize = new System.Collections.Generic.List<int> { 2865, 1789 },
                MeasuredAt = "2026-09-09 21:00:00",
            });

            var reloaded = new ConfigLoader(dir).Load();
            Assert.Equal(1.5, reloaded.Calibration.DpiScale);
            Assert.Equal(144u, reloaded.Calibration.MonitorDpi);
            Assert.Equal(new System.Collections.Generic.List<int> { 3840, 2160 }, reloaded.Calibration.Screen);
            Assert.Equal(new System.Collections.Generic.List<int> { 2865, 1789 }, reloaded.Calibration.ClientSize);
            Assert.Equal("2026-09-09 21:00:00", reloaded.Calibration.MeasuredAt);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // A config written before presets existed has a flat `spammer.keys` list; it must migrate into
    // a preset named "default" so the rest of the app only deals with presets.
    [Fact]
    public void LegacyFlatSpammerKeysMigrateIntoADefaultPreset()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var path = Path.Combine(dir, "defaults.yaml");
            // defaults.yaml has no spammer block any more, so the legacy one is simply appended.
            File.AppendAllText(path, "spammer:\n  keys:\n    '*0': 0.25\n    'F1': 5.0\n");

            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal("default", cfg.Spammer.Active);
            Assert.Equal(0.25, cfg.Spammer.ActiveKeys["*0"]);
            Assert.Equal(5.0, cfg.Spammer.ActiveKeys["F1"]);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void LoadMissingLocalYamlThrowsConfigException()
    {
        var dir = MakeTempConfigDir(includeLocal: false);
        try
        {
            Assert.Throws<ConfigException>(() => new ConfigLoader(dir).Load());
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // Spammer presets are the player's own rotations, so they live in local.yaml rather than the
    // template defaults.yaml that publish.bat ships. Every preset comes from there, and `active`
    // picks which one runs.
    [Fact]
    public void LocalYamlSuppliesTheSpammerPresets()
    {
        var dir = MakeTempConfigDirWithLocal(
            ValidOcrLocal +
            "spammer:\n  active: Knight0-9\n  presets:\n    Knight0-9:\n      '*0': 0.2\n      F1: 1.5\n" +
            "    Boss:\n      F2: 9.0\n");
        try
        {
            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal("Knight0-9", cfg.Spammer.Active);
            Assert.Equal(0.2, cfg.Spammer.ActiveKeys["*0"]);
            Assert.Equal(1.5, cfg.Spammer.ActiveKeys["F1"]);
            Assert.Equal(2, cfg.Spammer.Presets.Count);
            Assert.True(cfg.Spammer.Presets.ContainsKey("Boss"));
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // The priority ORDER rides through the same local.yaml projection as the presets above — the one
    // that has silently dropped a field four times in this repo. So it gets its own guard rather than
    // leaning on that test: a missing copy here throws nothing and reports nothing, it loads as "no
    // order given" and the preempt rule switches itself off, which reads as the feature not existing.
    [Fact]
    public void LocalYamlCarriesTheSpammerPriorityOrder()
    {
        var dir = MakeTempConfigDirWithLocal(
            ValidOcrLocal +
            "spammer:\n  active: Knight0-9\n  presets:\n    Knight0-9:\n      '*0': 0.2\n      F1: 1.5\n" +
            "    Boss:\n      F2: 9.0\n" +
            "  priority:\n    Knight0-9:\n      - F1\n      - '*0'\n");
        try
        {
            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal(new List<string> { "F1", "*0" }, cfg.Spammer.ActivePriority);
            // The other half, and the one that keeps this additive: a preset with no order must come
            // back with NO entry at all, so ActivePriority is empty and every rotation saved before
            // this existed keeps its old send-everything-due behaviour.
            Assert.False(cfg.Spammer.Priority.ContainsKey("Boss"));
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // The combos ride the same local.yaml projection as the presets and the order above — the one
    // that has silently dropped a field four times in this repo. A missing copy here throws nothing
    // and reports nothing: the combos come back empty and the pairs are never cast together, which
    // looks exactly like the feature not existing.
    [Fact]
    public void LocalYamlCarriesTheSpammerCombos()
    {
        var dir = MakeTempConfigDirWithLocal(
            ValidOcrLocal +
            "spammer:\n  active: Knight0-9\n  presets:\n    Knight0-9:\n      '1': 6.0\n      '2': 6.0\n" +
            "  combos:\n    Knight0-9:\n      - keys: ['1', '2']\n        gap: 0.75\n");
        try
        {
            var cfg = new ConfigLoader(dir).Load();

            var combo = Assert.Single(cfg.Spammer.ActiveCombos);
            Assert.Equal(new List<string> { "1", "2" }, combo.Keys);
            Assert.Equal(0.75, combo.Gap);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // The opening settings ride the same local.yaml projection as the presets, the order and the
    // combos. Same hazard, same guard — and here the failure is especially quiet, because an opening
    // that comes back as zeroes is not an error: it is "no lead-in, no stagger", so the feature just
    // looks like it was never built.
    [Fact]
    public void LocalYamlCarriesTheSpammerStartup()
    {
        var dir = MakeTempConfigDirWithLocal(
            ValidOcrLocal +
            "spammer:\n  active: Knight0-9\n  presets:\n    Knight0-9:\n      '1': 6.0\n" +
            "  startup:\n    Knight0-9:\n      lead_in: 5\n      step: 0.15\n");
        try
        {
            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal(5, cfg.Spammer.ActiveStartup.LeadIn);
            Assert.Equal(0.15, cfg.Spammer.ActiveStartup.Step);
            // The additive half, and the one the player asked for by name — "some need a lead-in, some
            // don't". A preset with no entry must read all-zeroes, so a rotation that wants none keeps
            // opening on the click with everything due at once.
            Assert.False(cfg.Spammer.Startup.ContainsKey("Boss"));
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // THE FOLD-IN. `startup_step` was a bare map before the lead-in existed, and a config written
    // then must keep its stagger rather than lose it to a shape change — silently, since a lost step
    // is indistinguishable from a preset that never had one. This is the test that fails if the
    // migration is ever dropped.
    [Fact]
    public void LegacyStartupStepFoldsIntoTheStartupBlock()
    {
        var dir = MakeTempConfigDirWithLocal(
            ValidOcrLocal +
            "spammer:\n  active: Knight0-9\n  presets:\n    Knight0-9:\n      '1': 6.0\n" +
            "  startup_step:\n    Knight0-9: 2\n");
        try
        {
            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal(2, cfg.Spammer.ActiveStartup.Step);
            Assert.Equal(0, cfg.Spammer.ActiveStartup.LeadIn);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // And the other direction: a config carrying BOTH must read the new one, or a player who edited
    // the new box would find the old number winning.
    [Fact]
    public void TheNewStartupBlockWinsOverTheLegacyStep()
    {
        var dir = MakeTempConfigDirWithLocal(
            ValidOcrLocal +
            "spammer:\n  active: Knight0-9\n  presets:\n    Knight0-9:\n      '1': 6.0\n" +
            "  startup_step:\n    Knight0-9: 2\n" +
            "  startup:\n    Knight0-9:\n      lead_in: 5\n      step: 0.15\n");
        try
        {
            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal(0.15, cfg.Spammer.ActiveStartup.Step);
            Assert.Equal(5, cfg.Spammer.ActiveStartup.LeadIn);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // ── the game variant — docs/plans/PLAN-US-CLIENT.md ────────────────────────────────────────────────

    /// <summary>The active variant RESOLVES the window title at load, and that is the whole reason the
    /// US-client change is small: every tool already reads `window.title`, so none of them — the OCR
    /// engine, capture, the cursor, the launcher — has to know a variant exists.</summary>
    [Fact]
    public void TheActiveVariantResolvesTheWindowTitle()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            // As shipped: tw, resolving to the title every tool reads.
            Assert.Equal("TW_LIVE", new ConfigLoader(dir).Load().Window.Title);

            // Point it at the US client and the SAME field follows — there is no second title to keep
            // in step.
            var defaults = Path.Combine(dir, "defaults.yaml");
            File.WriteAllText(defaults,
                File.ReadAllText(defaults).Replace("  variant: tw", "  variant: us"));

            Assert.Equal("US_LIVE", new ConfigLoader(dir).Load().Window.Title);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>An unset or UNKNOWN variant must change nothing. Unset is the shape of every config
    /// written before variants existed; unknown is a typo. In both, the configured title has to stand —
    /// blanking it would send every tool hunting a window called "".</summary>
    [Fact]
    public void AnUnsetOrUnknownVariantLeavesTheConfiguredTitleAlone()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            var defaults = Path.Combine(dir, "defaults.yaml");
            var text = File.ReadAllText(defaults);

            foreach (var replacement in new List<string> { "  variant: nope", "  variant: \"\"" })
            {
                File.WriteAllText(defaults, text.Replace("  variant: tw", replacement));
                Assert.Equal("TW_LIVE", new ConfigLoader(dir).Load().Window.Title);
            }
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>The Setup tab's switch, as the config sees it: a known name sets the variant and the
    /// resolved title and hands back the dictionary to load; an unknown name changes NOTHING and says
    /// what the known ones are. The caller (LauncherService) loads that dictionary between the two, so
    /// an install can never be left pointing at a client whose dictionary is missing — which would be a
    /// launcher that throws in its constructor rather than one that starts degraded.</summary>
    [Fact]
    public void SelectingAVariantResolvesItAndHandsBackItsDictionary()
    {
        var cfg = new AppConfig();

        var dictionary = cfg.ApplyVariant("us", out var error);

        Assert.Null(error);
        Assert.Equal("attributes.us.yaml", dictionary);
        Assert.Equal("us", cfg.Game.Variant);
        Assert.Equal("US_LIVE", cfg.Window.Title);

        // Case is the variants map's business, not the player's — it is case-insensitive, so a
        // hand-typed "TW" must work.
        Assert.Equal("attributes.yaml", cfg.ApplyVariant("TW", out var error2));
        Assert.Null(error2);
        Assert.Equal("TW_LIVE", cfg.Window.Title);

        // An unknown name leaves BOTH fields alone: nothing half-switched.
        var title = cfg.Window.Title;
        var variant = cfg.Game.Variant;
        Assert.Null(cfg.ApplyVariant("nope", out var error3));
        Assert.NotNull(error3);
        Assert.Contains("nope", error3);
        Assert.Contains("tw", error3);
        Assert.Equal(title, cfg.Window.Title);
        Assert.Equal(variant, cfg.Game.Variant);
    }

    /// <summary>The US dictionary ships EMPTY ON PURPOSE, and this pins it. The loader refuses a
    /// MISSING dictionary (the test below), and the text-free tools — gem, buy, sell, spammer, quest,
    /// and the pet reads — need the suite to be pointable at the US client today. It must not quietly
    /// gain content, because content here is words the Tuner matches, and words written from anywhere
    /// but that client are the thing the repo's own rule forbids.</summary>
    [Fact]
    public void TheShippedUsTemplateExistsAndIsEmptyOnPurpose()
    {
        var path = "";
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            var candidate = Path.Combine(dir.FullName, "v2", "config", "attributes.us.yaml.example");
            if (File.Exists(candidate)) { path = candidate; break; }
        }

        Assert.False(string.IsNullOrEmpty(path),
            "v2/config/attributes.us.yaml.example was not found by walking up from " + AppContext.BaseDirectory);

        // Loaded THROUGH the loader, so the assertion covers what it parses to rather than what the file
        // text looks like — `attributes: []` is an empty LIST, and a bare `attributes:` would be null,
        // which every consumer enumerates and throws on.
        var attrs = new ConfigLoader(Path.GetDirectoryName(path)!).LoadAttributes("attributes.us.yaml.example");
        Assert.Empty(attrs.Attributes);
    }

    /// <summary>A variant's dictionary that is MISSING is SEEDED from its template — the instruction the
    /// old error message gave by hand, and the same shape local.yaml has always had. That is what lets
    /// the Setup tab's client switch work on a fresh install without the zip shipping a file the player
    /// grows in place. And an existing one is never overwritten: it is the player's.</summary>
    [Fact]
    public void AMissingVariantDictionaryIsSeededFromItsTemplate()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            File.WriteAllText(Path.Combine(dir, "attributes.us.yaml.example"),
                "attributes: []\nper_level_stats: []\nnegative_attr_markers: []\ntext_fixes:\n  whole: []\n");

            var attrs = new ConfigLoader(dir).LoadAttributes("attributes.us.yaml");

            Assert.Empty(attrs.Attributes);
            // Seeded, not merely read: the file is there afterwards, and it is the one that gets filled.
            Assert.True(File.Exists(Path.Combine(dir, "attributes.us.yaml")));

            // Second direction: once it exists it is the player's file, and a seed must never undo it.
            File.WriteAllText(Path.Combine(dir, "attributes.us.yaml"),
                "attributes:\n  - name: \"Mine\"\n    category: \"atk\"\n    variants: [\"Mine\"]\n");
            Assert.Single(new ConfigLoader(dir).LoadAttributes("attributes.us.yaml").Attributes);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>With no template either, the failure stays LOUD — and for the original reason: a
    /// dictionary that matches nothing reads every attribute as "no attribute", silently, which is
    /// indistinguishable from the game having changed.</summary>
    [Fact]
    public void AMissingVariantDictionaryWithNoTemplateIsStillLoud()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            var ex = Assert.Throws<ConfigException>(
                () => new ConfigLoader(dir).LoadAttributes("attributes.us.yaml"));

            Assert.Contains("attributes.us.yaml", ex.Message);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // A save writes the `settings:` block and nothing else. The player's spammer presets live in their
    // own block of the same file, so a save must leave them alone — and the portable half has no
    // spammer field at all, which is what keeps a personal rotation out of the release template.
    [Fact]
    public void SaveSettingsLeavesTheSpammerBlockAlone()
    {
        var dir = MakeTempConfigDirWithLocal(
            ValidOcrLocal +
            "spammer:\n  active: Personal\n  presets:\n    Personal:\n      '*0': 0.2\n");
        try
        {
            var loader = new ConfigLoader(dir);
            var cfg = loader.Load();
            Assert.Equal("Personal", cfg.Spammer.Active);

            loader.SaveSettings(cfg);

            // The preset is still in its own block...
            var reloaded = new ConfigLoader(dir).LoadLocal()!;
            Assert.Equal(0.2, reloaded.Spammer!.Presets!["Personal"]["*0"]);

            // ...and the settings block carries no spammer key, so a personal rotation can never reach
            // the portable half and from there a release.
            var written = File.ReadAllText(Path.Combine(dir, "local.yaml"));
            Assert.Contains("\nspammer:", written);
            Assert.Null(Child(SettingsNode(Path.Combine(dir, "local.yaml")), "spammer"));
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // Saving rewrites these files from a fixed key list, so every comment in them is destroyed -
    // including the page of guidance local.yaml.example ships explaining that the seeded values are
    // v1 starting points and will be wrong until the calibrators run. The writer therefore emits its
    // own header, which is the only prose that CAN survive a save, and it has to say the two things
    // that have already cost data: comments are not preserved, and unknown keys are dropped.
    [Fact]
    public void BothSavesWriteAHeaderSayingWhatTheyDestroy()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            var cfg = loader.Load();

            // A no-op change: the point is the header the write emits, not what it carries.
            loader.UpdateLocal(_ => { });

            var written = File.ReadAllText(Path.Combine(dir, "local.yaml"));
            Assert.StartsWith("#", written);              // a header, not bare serialised content
            Assert.Contains("REWRITES this whole file", written);
            Assert.Contains("DROPPED", written);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // The header must not be parsed as content, and must not disturb the round-trip the other tests
    // rely on: a save followed by a load has to return what was saved.
    [Fact]
    public void HeaderDoesNotDisturbTheRoundTrip()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            var cfg = loader.Load();
            cfg.Tuner.TargetGrade = "XG";

            loader.SaveSettings(cfg);

            Assert.Equal("XG", new ConfigLoader(dir).Load().Tuner.TargetGrade);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // The pet calibrator writes its own local.yaml block. The load-bearing assertion is the last
    // one: the bag the boarding window opens is at a DIFFERENT place from the one the shop opens
    // beside, so the pet grid and the buy/sell grid are separate calibrations. If they ever shared a
    // field, every click in the pet flow would land on the wrong bag cell — and unlike a mis-aimed
    // sale there is no undo for feeding the pet the wrong item.
    /// <summary>A quest flow is the one config shape with a LIST OF OBJECTS inside it — and each of those
    /// objects carries three fields of its own — so this pins the whole thing through a real save and
    /// reload rather than trusting the projection.
    ///
    /// A dropped field here is a flow that sends nothing, or waits the wrong amount, 500 times, and this
    /// is the tool with NO other eyes on it: it reads nothing from the screen, so there is nothing that
    /// could notice a step arriving wrong. That makes the config the only place this can be caught.</summary>
    [Fact]
    public void QuestFlowsRoundTripWithTheirSteps()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            loader.UpdateLocal(local => local.Quest = new ConfigLoader.LocalQuest
            {
                Active = "red1",
                GuardPx = 12,
                Presets = new Dictionary<string, QuestPreset>
                {
                    ["red1"] = new()
                    {
                        InitialWaitSeconds = 7.5,
                        Loops = 500,
                        Steps = new List<QuestStep>
                        {
                            new() { Action = SealTools.Core.QuestAction.Click, DelaySeconds = 0.3 },
                            new() { Action = SealTools.Core.QuestAction.Key, Value = "1", DelaySeconds = 0.2 },
                            new() { Action = SealTools.Core.QuestAction.Enter, DelaySeconds = 0.25 },
                            new() { Action = SealTools.Core.QuestAction.Wait, DelaySeconds = 1.5 },
                        },
                    },
                },
            });

            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal("red1", cfg.Quest.Active);
            Assert.Equal(12, cfg.Quest.GuardPx);

            var flow = cfg.Quest.ActivePreset;
            Assert.Equal(7.5, flow.InitialWaitSeconds);
            Assert.Equal(500, flow.Loops);
            Assert.Equal(4, flow.Steps.Count);

            // The steps survive IN ORDER, with each of the three fields — and the no-value case stays
            // null rather than becoming an empty string, because a key step with "" is the shape that
            // sends nothing.
            Assert.Equal(SealTools.Core.QuestAction.Click, flow.Steps[0].Action);
            Assert.Null(flow.Steps[0].Value);
            Assert.Equal(0.3, flow.Steps[0].DelaySeconds);

            Assert.Equal(SealTools.Core.QuestAction.Key, flow.Steps[1].Action);
            Assert.Equal("1", flow.Steps[1].Value);
            Assert.Equal(0.2, flow.Steps[1].DelaySeconds);

            Assert.Equal(SealTools.Core.QuestAction.Enter, flow.Steps[2].Action);
            Assert.Null(flow.Steps[2].Value);

            Assert.Equal(SealTools.Core.QuestAction.Wait, flow.Steps[3].Action);
            Assert.Equal(1.5, flow.Steps[3].DelaySeconds);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>A SEQUENCE — several flows in one run — through a real save and reload, because it is
    /// another list of objects inside the one config shape that already had one, and the projection has
    /// silently dropped a field three times.
    ///
    /// The entries matter as much as the sequence: an entry is a flow's NAME plus a count, and a dropped
    /// one is a step of the run that silently never happens.</summary>
    [Fact]
    public void QuestSequencesRoundTripWithTheirEntries()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            loader.UpdateLocal(local => local.Quest = new ConfigLoader.LocalQuest
            {
                Active = "a",
                ActiveSequence = "nightly",
                GuardPx = 9,
                Presets = new Dictionary<string, QuestPreset>
                {
                    ["a"] = new() { Loops = 3, Steps = new List<QuestStep> { new() { Action = SealTools.Core.QuestAction.Click } } },
                    ["b"] = new() { Loops = 5, Steps = new List<QuestStep> { new() { Action = SealTools.Core.QuestAction.Enter } } },
                },
                Sequences = new Dictionary<string, QuestSequence>
                {
                    ["nightly"] = new()
                    {
                        InitialWaitSeconds = 12.5,
                        Loops = 5,
                        Entries = new List<QuestSequenceEntry>
                        {
                            new() { Preset = "a", Times = 3 },
                            new() { Preset = "b", Times = 2 },
                        },
                    },
                },
            });

            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal("nightly", cfg.Quest.ActiveSequence);
            Assert.Equal(9, cfg.Quest.GuardPx);

            var sequence = cfg.Quest.ActiveSequenceOrNull;
            Assert.NotNull(sequence);
            Assert.Equal(12.5, sequence!.InitialWaitSeconds);
            Assert.Equal(5, sequence.Loops);

            // In ORDER, with both fields of each entry — the A-then-B rhythm is the whole point.
            Assert.Equal(2, sequence.Entries.Count);
            Assert.Equal("a", sequence.Entries[0].Preset);
            Assert.Equal(3, sequence.Entries[0].Times);
            Assert.Equal("b", sequence.Entries[1].Preset);
            Assert.Equal(2, sequence.Entries[1].Times);

            // And the flows it names are still there — the sequence refers to them by name.
            Assert.Equal(2, cfg.Quest.Presets.Count);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>A DUPLICATED flow and sequence through a real save and reload. The button itself cannot
    /// be tested here (the Launcher is out of this project), so what is pinned is everything the button
    /// hands to the config: the generated name has to survive as a KEY, and the copy's steps have to come
    /// back as the copy's own steps rather than the original's.
    ///
    /// A duplicate that reloads as the preset it was copied from would look right on screen and be wrong
    /// on the next launch, which is the same class of failure as a dropped step field.</summary>
    [Fact]
    public void DuplicatedFlowsAndSequencesSurviveASaveAndReload()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            loader.UpdateLocal(local =>
            {
                var source = new QuestPreset
                {
                    InitialWaitSeconds = 3,
                    Loops = 2,
                    Steps = new List<QuestStep>
                    {
                        new() { Action = SealTools.Core.QuestAction.Click, DelaySeconds = 0.3 },
                    },
                };
                var sourceSequence = new QuestSequence
                {
                    Loops = 4,
                    Entries = new List<QuestSequenceEntry> { new() { Preset = "red", Times = 2 } },
                };

                var presets = new Dictionary<string, QuestPreset> { ["red"] = source };
                var sequences = new Dictionary<string, QuestSequence> { ["nightly"] = sourceSequence };

                // What the two Duplicate buttons do, in the order they do it.
                var copyName = PresetCopy.CopyName("red", presets.Keys);
                presets[copyName] = PresetCopy.CopyQuestPreset(source);
                var sequenceCopyName = PresetCopy.CopyName("nightly", sequences.Keys);
                sequences[sequenceCopyName] = PresetCopy.CopyQuestSequence(sourceSequence);

                local.Quest = new ConfigLoader.LocalQuest
                {
                    Active = copyName,
                    ActiveSequence = sequenceCopyName,
                    Presets = presets,
                    Sequences = sequences,
                };
            });

            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal("red (copy)", cfg.Quest.Active);
            Assert.Equal("nightly (copy)", cfg.Quest.ActiveSequence);
            Assert.Equal(2, cfg.Quest.Presets.Count);
            Assert.Equal(2, cfg.Quest.Sequences.Count);

            var copy = cfg.Quest.ActivePreset;
            Assert.Equal(3, copy.InitialWaitSeconds);
            Assert.Equal(2, copy.Loops);
            Assert.Single(copy.Steps);
            Assert.Equal(SealTools.Core.QuestAction.Click, copy.Steps[0].Action);

            // The sequence's copy still names the flow it named before: a duplicate re-points nothing.
            var sequenceCopy = cfg.Quest.ActiveSequenceOrNull;
            Assert.NotNull(sequenceCopy);
            Assert.Equal(4, sequenceCopy!.Loops);
            Assert.Equal("red", sequenceCopy.Entries[0].Preset);
            Assert.Equal(2, sequenceCopy.Entries[0].Times);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>An empty `active_sequence` means "run the single flow" — the shape of every config written
    /// before sequences existed — and an UNKNOWN name must fall back the same way rather than leaving the
    /// tool with nothing to run.</summary>
    [Fact]
    public void NoSequenceChosenFallsBackToTheSingleFlow()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            // One block, written three times with a different active sequence each time — through the
            // loader's read-modify-write, the way the Quest tab writes it.
            var questBlock = new ConfigLoader.LocalQuest
            {
                Active = "a",
                Presets = new Dictionary<string, QuestPreset>
                {
                    ["a"] = new() { Steps = new List<QuestStep> { new() { Action = SealTools.Core.QuestAction.Click } } },
                },
                Sequences = new Dictionary<string, QuestSequence>
                {
                    ["nightly"] = new() { Entries = new List<QuestSequenceEntry> { new() { Preset = "a", Times = 1 } } },
                },
            };

            // Nothing chosen at all.
            loader.UpdateLocal(l => l.Quest = questBlock);
            Assert.Null(new ConfigLoader(dir).Load().Quest.ActiveSequenceOrNull);

            // A name nobody recognises.
            questBlock.ActiveSequence = "typo";
            loader.UpdateLocal(l => l.Quest = questBlock);
            Assert.Null(new ConfigLoader(dir).Load().Quest.ActiveSequenceOrNull);

            // And the real one resolves.
            questBlock.ActiveSequence = "nightly";
            loader.UpdateLocal(l => l.Quest = questBlock);
            Assert.NotNull(new ConfigLoader(dir).Load().Quest.ActiveSequenceOrNull);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void PetCalibrationRoundTripsAndKeepsItsOwnBagGrid()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            loader.UpdateLocal(local => local.Pet = new ConfigLoader.LocalPet
            {
                MenuButton = new List<int> { 815, 1735 },
                FeedIcon = new List<int> { 900, 1400 },
                CloseButton = new List<int> { 880, 140 },
                CloseButtonPng = "close-ref-png",
                BoardingWindowMayBeOpen = true,
                PageTabs = new List<List<int>> { new() { 700, 180 }, new() { 745, 180 }, new() { 790, 180 } },
                BagGrid = new List<int> { 660, 150, 408, 408 },
                BagSlot = new List<int> { 660, 150, 51, 51 },
                FoodSlots = new List<List<int>> { new() { 0, 5 }, new() { 1, 12 } },
                ReturnSlot = new List<int> { 1, 2 },
                Slots = new List<ConfigLoader.LocalPetSlot>
                {
                    new()
                    {
                        ToggleLabel = new List<int> { 590, 200, 120, 30 },
                        BoardingPetSlot = new List<int> { 190, 210, 60, 60 },
                        FeederStrip = new List<int> { 250, 210, 130, 60 },
                        PetSlotEmptyPng = "row-slot-png",
                        Stacks = 5,
                        BoardingRunning = true,
                    },
                },
            });

            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal(new List<int> { 815, 1735 }, cfg.Pet.MenuButton);
            Assert.Equal(new List<int> { 900, 1400 }, cfg.Pet.FeedIcon);
            Assert.Equal(new List<int> { 1, 2 }, cfg.Pet.ReturnSlot);
            Assert.Equal(3, cfg.Pet.PageTabs.Count);
            Assert.Equal(2, cfg.Pet.FoodSlots.Count);
            Assert.Equal(new List<int> { 660, 150, 408, 408 }, cfg.Pet.BagGrid);

            // The rows survive as rows, with the per-row fields intact — including Stacks, which is
            // the whole reason a row is a thing rather than a set of loose fields.
            var slot = Assert.Single(cfg.Pet.Slots);
            Assert.Equal(new List<int> { 590, 200, 120, 30 }, slot.ToggleLabel);
            Assert.Equal(new List<int> { 190, 210, 60, 60 }, slot.BoardingPetSlot);
            Assert.Equal(new List<int> { 250, 210, 130, 60 }, slot.FeederStrip);

            // The count REFERENCE, which is the one that got away. It is a RECTANGLE, and the loader
            // applied the "is this a point" predicate to it — Count == 2, which a rect never is — so
            // both boxes were written to local.yaml on save and silently DISCARDED on every load. The
            // player's symptom was the Test read telling them to draw boxes they had already drawn,
            // and it looked like the tool forgetting a setting rather than a load predicate being
            // wrong for the shape.
            //
            // Asserted here rather than only in a unit test of the predicate, because the direction
            // that matters is the ROUND TRIP: written by UpdateLocal, read back by Load.
            Assert.Equal(5, slot.Stacks);
            Assert.True(slot.BoardingRunning);

            // Derived, per row: 5 stacks = 1,500 items at 3/min = 500 minutes.
            Assert.Equal(1500, PetConfig.LoadItemsFor(slot));
            Assert.Equal(500, cfg.Pet.LoadMinutesFor(slot));

            // The close probe's pair — SESSION state the run writes about itself. The save direction
            // has a reflection fence over it; the LOAD merge (ApplyOverrides) has none, and this is
            // where fields have silently gone missing twice before, so the round trip is the test.
            // The belief in particular must survive a restart: the window a killed run leaves open is
            // exactly the case only the next run can notice.
            Assert.Equal("close-ref-png", cfg.Pet.CloseButtonPng);
            Assert.True(cfg.Pet.BoardingWindowMayBeOpen);

            // The two grids are independent calibrations, and this is the assertion that keeps them so.
            Assert.Null(cfg.BuySell.BagGrid);

            // A zero-sized rectangle is not a calibration and must not be adopted — the same guard the
            // buy/sell grid gets. Without it all 64 derived centres land on the same pixel.
            // Through a SECOND loader instance, so nothing in memory carries the block: the
            // read-modify-write is the only thing putting it in the file, which is the shape the
            // launcher's own threads have.
            new ConfigLoader(dir).UpdateLocal(l =>
            {
                l.Pet ??= new ConfigLoader.LocalPet();
                l.Pet.BagGrid = new List<int> { 0, 0, 0, 0 };
            });
            Assert.Null(new ConfigLoader(dir).Load().Pet.BagGrid);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // The pet flow persists through LocalPet, and the launcher used to build that object from TWO
    // hand-written field lists — one per pet tab. A field missing from either was silently never
    // saved, and because Calibrate Pet's list REPLACED the object rather than mutating it, a save
    // there also wiped whatever the other list knew about. That is how the Timing boxes came to
    // revert on every launch and how a calibration save dropped the queue crops.
    //
    // There is one projection now (`LocalPet.From`) and this is what keeps it complete: it walks
    // LocalPet's properties by reflection, so ADDING A FIELD AND FORGETTING IT THERE FAILS HERE
    // rather than in a player's next session. The values are all non-default on purpose — a copy
    // that was never made is then a null where a value was expected, not a coincidence.
    [Fact]
    public void EveryLocalPetFieldIsCopiedFromTheConfig()
    {
        var cfg = new PetConfig
        {
            MenuButton = new List<int> { 1, 2 },
            FeedIcon = new List<int> { 3, 4 },
            CloseButton = new List<int> { 5, 6 },
            PageTabs = new List<List<int>> { new() { 7, 8 } },
            BagGrid = new List<int> { 25, 26, 27, 28 },
            BagSlot = new List<int> { 29, 30, 31, 32 },
            FoodSlots = new List<List<int>> { new() { 0, 33 } },
            FoodSlotsUsed = 7,
            // MUST be set here, or the loop below cannot see a missing copy: it compares
            // LocalPet against the config, so a field this fixture omits is null on BOTH
            // sides and passes whether or not the projection carries it.
            FoodIconPng = "food-png",
            CloseButtonPng = "close-png",
            BoardingWindowMayBeOpen = true,
            ReturnSlot = new List<int> { 34, 35 },
            MaxButton = new List<int> { 40, 41 },
            WaitAfterEmptyMinutes = 3,
            ActionWaitMs = 1234,
            FoodLoadMode = SealTools.Core.FoodLoadMode.Drag,
            // Non-default on purpose: against the defaults this would pass whether or not either
            // half copied the field, which is the trap this guard's own comment warns about.
            FeederCountMinScore = 0.55,
            Slots = new List<PetSlotConfig>
            {
                new()
                {
                    ToggleLabel = new List<int> { 9, 10, 11, 12 },
                    BoardingPetSlot = new List<int> { 13, 14, 15, 16 },
                    FeederStrip = new List<int> { 17, 18, 19, 20 },
                    Stacks = 5,
                    PetSlotEmptyPng = "empty-slot-png",
                    BoardingRunning = true,
                },
            },
            Queue = new List<PetQueueEntry>
            {
                // A REAL species, in Chinese, deliberately: this is a value the player picks from the
                // feeding table, so the test proves the YAML round-trip carries non-ASCII as well as
                // proving the field survives the projection.
                new() { Label = "p", Rect = new List<int> { 36, 37, 38, 39 }, Png = "icon-png",
                        Species = "黑龙类" },
            },
        };

        var local = ConfigLoader.LocalPet.From(cfg);

        // The nested shapes are MAPPED rather than assigned — a LocalPetSlot is a different type from
        // a PetSlotConfig — so reference equality is the wrong check for them and they are asserted
        // field by field below instead. Everything else is a straight assignment and is compared here.
        var mapped = new List<string> { nameof(ConfigLoader.LocalPet.Slots), nameof(ConfigLoader.LocalPet.Queue) };

        foreach (var prop in typeof(ConfigLoader.LocalPet).GetProperties())
        {
            var source = typeof(PetConfig).GetProperty(prop.Name);
            Assert.True(source != null,
                $"LocalPet.{prop.Name} has no PetConfig counterpart — the two have drifted apart");
            if (mapped.Contains(prop.Name)) continue;
            Assert.True(Equals(prop.GetValue(local), source!.GetValue(cfg)),
                $"LocalPet.{prop.Name} was not copied from PetConfig, so a Save silently drops it");
        }

        // The nested shapes are lists, so the loop above only proves the LIST was copied. These are
        // the fields inside a row and inside a queue entry — a new one of those is exactly the kind of
        // thing that gets added to PetSlotConfig and forgotten in the projection.
        var slot = Assert.Single(local.Slots!);
        Assert.Equal(new List<int> { 9, 10, 11, 12 }, slot.ToggleLabel);
        Assert.Equal(new List<int> { 13, 14, 15, 16 }, slot.BoardingPetSlot);
        Assert.Equal(5, slot.Stacks);
        Assert.True(slot.BoardingRunning);

        var queued = Assert.Single(local.Queue!);
        Assert.Equal("p", queued.Label);
        Assert.Equal("icon-png", queued.Png);
        // SAVE direction. `Species` decides the feeding estimate, and a projection that drops it on
        // save would look exactly like a pet nobody had named — the schedule would fall back to the
        // configured cycle with nothing saying why.
        Assert.Equal("黑龙类", queued.Species);
    }

    // The launcher has a Save on each pet tab and the two are SCOPED: Calibrate Pet writes the
    // machine's half, the Pet tab writes the run's. That is only safe if the two halves together
    // cover every field — a field in neither is one nothing ever saves, which is precisely the bug
    // the single projection was introduced to end.
    //
    // So the assertion is a UNION, not a list: apply both to an empty block and the result must be
    // identical to the whole-block projection. It fails the moment a field is added and put in
    // neither half, and it does not care which half a field lands in.
    [Fact]
    public void TheTwoScopedSavesTogetherCoverTheWholeBlock()
    {
        var cfg = new PetConfig
        {
            MenuButton = new List<int> { 1, 2 },
            FeedIcon = new List<int> { 3, 4 },
            CloseButton = new List<int> { 5, 6 },
            PageTabs = new List<List<int>> { new() { 7, 8 } },
            BagGrid = new List<int> { 25, 26, 27, 28 },
            BagSlot = new List<int> { 29, 30, 31, 32 },
            FoodSlots = new List<List<int>> { new() { 0, 33 } },
            FoodSlotsUsed = 7,
            ReturnSlot = new List<int> { 34, 35 },
            MaxButton = new List<int> { 40, 41 },
            WaitAfterEmptyMinutes = 3,
            ActionWaitMs = 1234,
            FoodLoadMode = SealTools.Core.FoodLoadMode.Drag,
            // Non-default on purpose: against the defaults this would pass whether or not either
            // half copied the field, which is the trap this guard's own comment warns about.
            FeederCountMinScore = 0.55,
            Slots = new List<PetSlotConfig>
            {
                new()
                {
                    ToggleLabel = new List<int> { 9, 10, 11, 12 },
                    BoardingPetSlot = new List<int> { 13, 14, 15, 16 },
                    FeederStrip = new List<int> { 17, 18, 19, 20 },
                    Stacks = 5,
                    BoardingRunning = true,
                },
            },
            Queue = new List<PetQueueEntry>
            {
                // A REAL species, in Chinese, deliberately: this is a value the player picks from the
                // feeding table, so the test proves the YAML round-trip carries non-ASCII as well as
                // proving the field survives the projection.
                new() { Label = "p", Rect = new List<int> { 36, 37, 38, 39 }, Png = "icon-png",
                        Species = "黑龙类" },
            },
        };

        // Each half on its OWN block, then the question per field: did either of them write the value
        // the config actually holds?
        //
        // COMPARED AGAINST THE CONFIG, not against From — and that is the whole test. Comparing
        // against From looks equivalent and is worthless: From is defined as the two halves, so it
        // loses whatever they lose and agrees with itself. Compared against the config, a field in
        // neither half leaves its block at the CLR default while the config holds a non-default, and
        // the assertion fires. `cfg` below therefore fills every field on purpose.
        var calibrationOnly = new ConfigLoader.LocalPet();
        ConfigLoader.LocalPet.ApplyCalibration(calibrationOnly, cfg);

        var sessionOnly = new ConfigLoader.LocalPet();
        ConfigLoader.LocalPet.ApplySession(sessionOnly, cfg);

        // The nested shapes are MAPPED, so reference equality is the wrong check and they are compared
        // field by field below. Slots is also the one property BOTH halves touch — calibration owns the
        // geometry, the session owns BoardingRunning.
        var mapped = new List<string> { nameof(ConfigLoader.LocalPet.Slots), nameof(ConfigLoader.LocalPet.Queue) };

        foreach (var prop in typeof(ConfigLoader.LocalPet).GetProperties())
        {
            if (mapped.Contains(prop.Name)) continue;
            var source = typeof(PetConfig).GetProperty(prop.Name)!.GetValue(cfg);
            var byCalibration = Equals(prop.GetValue(calibrationOnly), source);
            var bySession = Equals(prop.GetValue(sessionOnly), source);

            // LIMIT OF THIS GUARD, and it cost a bug: "either half" is the wrong question for a field
            // whose UI lives on ONE tab. A feeder-count field was written by the CALIBRATION half
            // while its box sat on the Pet tab, so it saved from Calibrate Pet and silently reverted
            // from the Pet tab — and this test passed both before and after the fix. The pairing that
            // matters is "the half whose tab edits it", which reflection cannot see. Worth a comment
            // because the guard reads as though it covers more than it does.
            Assert.True(byCalibration || bySession,
                $"LocalPet.{prop.Name} is in NEITHER scoped save, so no button ever writes it");
        }

        // Slots: calibration supplies the geometry, and the rows have to exist before the session can
        // say anything about them.
        var cal = Assert.Single(calibrationOnly.Slots!);
        Assert.Equal(new List<int> { 9, 10, 11, 12 }, cal.ToggleLabel);
        Assert.Equal(5, cal.Stacks);
        Assert.True(cal.BoardingRunning);

        // Queue: the session's, and the calibration half must not touch it.
        var q = Assert.Single(sessionOnly.Queue!);
        Assert.Equal("p", q.Label);
        Assert.Equal("icon-png", q.Png);
        // Null on a fresh block: the calibration half sets the queue to nothing at all, which is what
        // "leaves it alone" means when there was nothing there to leave.
        Assert.True(calibrationOnly.Queue is null or { Count: 0 });
    }

    // The guard above reaches the OUTER LocalPet and stops there, and the split test above only ever
    // asserts the CALIBRATION half's rows. So nothing covered the per-row fields the SESSION half
    // owns — and the mutation check proved it: deleting `t.Slots[i].Enabled = p.Slots[i].Enabled`
    // from ApplySession left all 126 tests green.
    //
    // That is the `ActionWaitMs` shape exactly — a field some save never wrote, so it moves in the UI,
    // appears to work, and is gone by the next launcher start. Worth a test rather than a note,
    // because the tick lives on the Pet tab, which saves through ApplySession, so this projection is
    // the ONLY thing that persists it.
    //
    // The rows must already EXIST: ApplySession deliberately does not invent one, because introducing
    // a row is not a run's business.
    [Fact]
    public void TheSessionHalfCarriesThePerRowRunFlags()
    {
        var existing = new ConfigLoader.LocalPet
        {
            // Both start NULL (they are bool?), so writing false/true is distinguishable from the
            // field never being touched — a test that used the CLR defaults would pass either way.
            Slots = new List<ConfigLoader.LocalPetSlot> { new() },
        };
        var cfg = new PetConfig
        {
            Slots = new List<PetSlotConfig>
            {
                new() { Enabled = false, BoardingRunning = true },
            },
        };

        ConfigLoader.LocalPet.ApplySession(existing, cfg);

        var row = Assert.Single(existing.Slots!);
        Assert.False(row.Enabled);
        Assert.True(row.BoardingRunning);
    }

    // The guard above walks LocalPet's properties, and that was NOT ENOUGH.
    //
    // LocalPetSlot is the row — a nested type with its own hand-written projection — and
    // PetSlotEmptyPng was missing from it. So every load dropped each row's empty-slot reference and
    // the next save wrote the blank back. Row 1 survived only because the migration re-supplies it
    // from the old top-level value, which made the loss look like "rows 2-4 have no reference"
    // rather than like a projection bug — and the run that followed read three empty rows as
    // occupied and started an empty boarding on one of them.
    //
    // A field list is only as good as the widest thing it is checked against. This is the nested
    // types, by reflection, so a field added to a ROW or a QUEUE ENTRY and forgotten in its
    // projection fails here rather than in a live run.
    // COMPARED AGAINST THE STORED OBJECT, not against From() — and the first version of this test got
    // that wrong. It went through LocalPet.From, which builds the stored rows directly and never
    // calls ToConfig at all, so deleting the offending field from ToConfig left the test green. The
    // direction that matters is the LOAD: what the file holds, against what the app ends up with.
    [Fact]
    public void EveryNestedPetFieldIsCopied()
    {
        var stored = new ConfigLoader.LocalPetSlot
        {
            // false, not the default: a fixture that used the default would pass whether or not the
            // projection copied it — the trap the outer guard's own round-trip test fell into.
            Enabled = false,
            ToggleLabel = new List<int> { 1, 2, 3, 4 },
            BoardingPetSlot = new List<int> { 5, 6, 7, 8 },
            FeederStrip = new List<int> { 9, 10, 11, 12 },
            Stacks = 5,
            PetSlotEmptyPng = "row-empty-png",
            BoardingRunning = true,
        };
        var live = stored.ToConfig();

        foreach (var prop in typeof(ConfigLoader.LocalPetSlot).GetProperties())
        {
            var target = typeof(PetSlotConfig).GetProperty(prop.Name);
            Assert.True(target != null, $"LocalPetSlot.{prop.Name} has no PetSlotConfig counterpart");
            Assert.True(Equals(prop.GetValue(stored), target!.GetValue(live)),
                $"LocalPetSlot.{prop.Name} is not carried into the config, so every LOAD drops it " +
                "and the next save writes the blank back");
        }

        var storedEntry = new ConfigLoader.LocalPetQueueEntry
        {
            Label = "a pet",
            Rect = new List<int> { 11, 12, 13, 14 },
            Png = "icon-png",
            // Non-null on purpose: the loop below compares the two objects field by field, so a null
            // here would pass whether or not the projection carried it.
            Species = "黑龙类",
        };
        var liveEntry = storedEntry.ToConfig();

        foreach (var prop in typeof(ConfigLoader.LocalPetQueueEntry).GetProperties())
        {
            var target = typeof(PetQueueEntry).GetProperty(prop.Name);
            Assert.True(target != null, $"LocalPetQueueEntry.{prop.Name} has no counterpart");
            Assert.True(Equals(prop.GetValue(storedEntry), target!.GetValue(liveEntry)),
                $"LocalPetQueueEntry.{prop.Name} is not carried into the config");
        }
    }

    /// <summary>And the scoping has to be REAL, or it is just a union with extra steps: a calibration
    /// save must not disturb the run's half, because that is the whole reason for splitting them.</summary>
    [Fact]
    public void ACalibrationSaveLeavesTheSessionHalfAlone()
    {
        var cfg = new PetConfig
        {
            ReturnSlot = new List<int> { 1, 2 },
            FoodSlots = new List<List<int>> { new() { 0, 33 } },
            FoodSlotsUsed = 4,
            Queue = new List<PetQueueEntry> { new() { Label = "kept", Png = "icon" } },
            Slots = new List<PetSlotConfig> { new() { ToggleLabel = new List<int> { 9, 10, 11, 12 } } },
        };
        cfg.Slots[0].BoardingRunning = true;

        // Calibration first, because the rows have to EXIST before either half can write anything
        // into them — ApplySession deliberately does not invent a row, since introducing one is not
        // the session's to do. That ordering is the real one: a machine is calibrated before it runs.
        var onDisk = new ConfigLoader.LocalPet();
        ConfigLoader.LocalPet.ApplyCalibration(onDisk, cfg);
        ConfigLoader.LocalPet.ApplySession(onDisk, cfg);

        // A calibration save from an app that does NOT know the session state — the shape of a
        // restart where only the machine half was re-read.
        var afterCalibration = onDisk;
        ConfigLoader.LocalPet.ApplyCalibration(afterCalibration, new PetConfig
        {
            Slots = new List<PetSlotConfig> { new() { ToggleLabel = new List<int> { 1, 1, 1, 1 } } },
        });

        Assert.Equal(new List<int> { 1, 2 }, afterCalibration.ReturnSlot);
        Assert.Equal(4, afterCalibration.FoodSlotsUsed);
        Assert.Equal("kept", Assert.Single(afterCalibration.Queue!).Label);
        Assert.True(Assert.Single(afterCalibration.Slots!).BoardingRunning);
        Assert.Equal(new List<int> { 1, 1, 1, 1 }, Assert.Single(afterCalibration.Slots!).ToggleLabel);
    }

    // The pet block used to be one flat set of fields. It is now `slots:` plus `return_slot` and
    // `food_slots`, and the loader sets IgnoreUnmatchedProperties — so an old file does not fail, it
    // just silently loses every key nothing maps to any more. That is a player's whole pet
    // calibration, and it is the same shape of loss the spammer migration exists to prevent.
    //
    // This is the test that says the upgrade keeps it. Every value below is one a real local.yaml
    // carries today, written under its OLD name, and every one has to come back under the new one.
    private const string PreRowsPetLocal =
        ValidOcrLocal +
        "pet:\n" +
        "  menu_button: [815, 1735]\n" +
        "  toggle_label: [682, 260, 127, 46]\n" +
        "  boarding_pet_slot: [240, 247, 64, 70]\n" +
        "  feeder_slot_a: [344, 262, 67, 56]\n" +
        "  feeder_slot_b: [412, 259, 63, 60]\n" +
        "  boarding_running: true\n" +
        "  pet_cell: [1, 2]\n" +
        "  food_cells: [[1, 63], [1, 62]]\n" +
        "  food_cells_used: 11\n" +
        "  pet_icon_rect: [36, 37, 38, 39]\n" +
        "  pet_icon_png: 'iVBORw0KGgo='\n" +
        "  pet_slot_empty_png: 'row-one-empty'\n";

    // The player's ACTUAL pre-rows pet block, copied field for field from their local.yaml on
    // 2026-09-19 — including the empty values, which is the part a hand-written fixture gets wrong.
    // Written after a live run showed `food_slots: []` and an empty `return_slot` where a migrated
    // file should have had both, so this is the regression that says whether the migration is at
    // fault or something downstream wiped them.
    private const string PlayersPreRowsPetLocal =
        ValidOcrLocal +
        "pet:\n" +
        "  menu_button:\n  - 830\n  - 1740\n" +
        "  feed_icon:\n  - 874\n  - 1665\n" +
        "  close_button:\n  - 866\n  - 177\n" +
        "  page_tabs:\n  - - 976\n    - 221\n  - - 1064\n    - 217\n  - - 1155\n    - 221\n" +
        "  toggle_label:\n  - 682\n  - 260\n  - 127\n  - 46\n" +
        "  boarding_pet_slot:\n  - 240\n  - 247\n  - 64\n  - 70\n" +
        "  feeder_slot_a:\n  - 344\n  - 262\n  - 67\n  - 56\n" +
        "  feeder_slot_b:\n  - 412\n  - 259\n  - 63\n  - 60\n" +
        "  bag_grid:\n  - 933\n  - 242\n  - 405\n  - 406\n" +
        "  bag_slot:\n  - 933\n  - 243\n  - 47\n  - 48\n" +
        "  food_cells:\n  - - 1\n    - 63\n  - - 1\n    - 62\n  - - 1\n    - 61\n" +
        "  food_cells_used: 11\n" +
        "  action_wait_ms: \n" +
        "  wait_after_empty_minutes: \n" +
        "  boarding_running: true\n" +
        "  pet_cell:\n  - 1\n  - 2\n" +
        "  pet_icon_rect: \n" +
        "  pet_icon_png: \n" +
        "  max_button: \n";

    [Fact]
    public void ThePlayersRealPreRowsFileSurvivesTheMigration()
    {
        var dir = MakeTempConfigDirWithLocal(PlayersPreRowsPetLocal);
        try
        {
            var pet = new ConfigLoader(dir).Load().Pet;

            Assert.Equal(new List<int> { 1, 2 }, pet.ReturnSlot);
            Assert.Equal(3, pet.FoodSlots.Count);
            Assert.Equal(11, pet.FoodSlotsUsed);

            var slot = Assert.Single(pet.Slots);
            Assert.Equal(new List<int> { 682, 260, 127, 46 }, slot.ToggleLabel);
            Assert.Equal(new List<int> { 240, 247, 64, 70 }, slot.BoardingPetSlot);
            Assert.True(slot.BoardingRunning);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void PetBlockWrittenBeforeTheRowsLoadsAsOneRow()
    {
        var dir = MakeTempConfigDirWithLocal(PreRowsPetLocal);
        try
        {
            var pet = new ConfigLoader(dir).Load().Pet;

            // The rows: one, built from the flat fields, and it is the FREE row — which is why two
            // stacks is the right count to carry over rather than a guess.
            var slot = Assert.Single(pet.Slots);
            Assert.Equal(new List<int> { 682, 260, 127, 46 }, slot.ToggleLabel);
            Assert.Equal(new List<int> { 240, 247, 64, 70 }, slot.BoardingPetSlot);
            Assert.Equal(2, slot.Stacks);
            Assert.True(slot.BoardingRunning);

            // The strip is the SPAN of the two old boxes — `feeder_slot_a` [344,262,67,56] and
            // `feeder_slot_b` [412,259,63,60] — which is an APPROXIMATION and deliberately so: those
            // two framed the numbers, not the slots, so the span runs between the digits and comes out
            // a little narrow. It is carried because a pre-rows file could not load food at all
            // without it, and a narrow strip still puts a click inside the slot. Redrawing it is the
            // first thing to do on such a machine, and the migrator says so.
            Assert.Equal(new List<int> { 344, 259, 131, 60 }, slot.FeederStrip);

            // The empty-slot reference used to be ONE crop for the whole tool, and it is per row now.
            // A file written before that carries it at the top level, and it belongs to row 1 —
            // which is the row it was taken from, which is why row 1 was the only one that read
            // correctly on the live run that found this.
            Assert.Equal("row-one-empty", slot.PetSlotEmptyPng);

            // The renames. `pet_cell` and `food_cells` are also stranded keys, one level down.
            Assert.Equal(new List<int> { 1, 2 }, pet.ReturnSlot);
            Assert.Equal(2, pet.FoodSlots.Count);
            Assert.Equal(11, pet.FoodSlotsUsed);

            // The staged icon becomes the first queued pet — the field was never read by anything, and
            // this is the migration that finally gives it a consumer rather than dropping it.
            var queued = Assert.Single(pet.Queue);
            Assert.Equal(new List<int> { 36, 37, 38, 39 }, queued.Rect);
            Assert.Equal("iVBORw0KGgo=", queued.Png);

            // Shared fields are untouched by the migration.
            Assert.Equal(new List<int> { 815, 1735 }, pet.MenuButton);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>The other half of the migration: once the new build has saved, the file carries
    /// `slots:` and the flat keys are gone — and loading THAT must not re-run the migration over the
    /// top and produce a second row or overwrite a value the player has since changed.</summary>
    [Fact]
    public void AMigratedFileRoundTripsWithoutMigratingTwice()
    {
        var dir = MakeTempConfigDirWithLocal(PreRowsPetLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            var migrated = loader.Load();

            // Save it back the way the UI would, then load again.
            loader.UpdateLocal(local => local.Pet = ConfigLoader.LocalPet.From(migrated.Pet));

            var again = new ConfigLoader(dir).Load().Pet;
            var slot = Assert.Single(again.Slots);
            Assert.Equal(new List<int> { 682, 260, 127, 46 }, slot.ToggleLabel);
            Assert.Equal(new List<int> { 1, 2 }, again.ReturnSlot);
            Assert.Single(again.Queue);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    /// <summary>The Timing fields are the pair that was written by nothing at all. Pinned through the
    /// loader as well as the projection, because either half failing loses the value in the same
    /// silent way: the numbers look set on the tab and are gone after a restart.</summary>
    [Fact]
    public void PetTimingRoundTripsThroughLocalYaml()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            loader.UpdateLocal(local => local.Pet = ConfigLoader.LocalPet.From(new PetConfig
            {
                WaitAfterEmptyMinutes = 2,
                ActionWaitMs = 1234,
            }));

            var cfg = new ConfigLoader(dir).Load();
            Assert.Equal(2, cfg.Pet.WaitAfterEmptyMinutes);
            Assert.Equal(1234, cfg.Pet.ActionWaitMs);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // Presets used to live in defaults.yaml. SaveDefaults no longer writes them, so the first Save
    // on any other tab rewrites that file without a spammer block — and on a machine that only ever
    // ran the older build, that deleted the player's only copy. Load() adopts them into local.yaml
    // first. The SaveDefaults + reload at the end is the assertion that actually catches the loss.
    [Fact]
    public void LoadAdoptsSpammerPresetsLeftInDefaultsIntoLocalYaml()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            File.AppendAllText(Path.Combine(dir, "defaults.yaml"),
                "spammer:\n  active: Knight0-9\n  presets:\n    Knight0-9:\n      '*0': 0.2\n");

            var cfg = new ConfigLoader(dir).Load();
            Assert.Equal("Knight0-9", cfg.Spammer.Active);
            Assert.Equal(0.2, cfg.Spammer.ActiveKeys["*0"]);

            var adopted = new ConfigLoader(dir).LoadLocal()!;
            Assert.NotNull(adopted.Spammer);
            Assert.Equal("Knight0-9", adopted.Spammer!.Active);
            Assert.Equal(0.2, adopted.Spammer.Presets!["Knight0-9"]["*0"]);

            new ConfigLoader(dir).SaveSettings(cfg);
            var reloaded = new ConfigLoader(dir).Load();
            Assert.Equal("Knight0-9", reloaded.Spammer.Active);
            Assert.Equal(0.2, reloaded.Spammer.ActiveKeys["*0"]);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // The legacy flat-keys migration has to run before adoption reads the preset set. After the
    // merge it used to see Count != 0 whenever local.yaml supplied a preset and skip, dropping the
    // keys silently; before it, they become a preset and reach local.yaml.
    [Fact]
    public void LegacyFlatKeysReachLocalYamlThroughAdoption()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            File.AppendAllText(Path.Combine(dir, "defaults.yaml"), "spammer:\n  keys:\n    '*0': 0.25\n");

            var cfg = new ConfigLoader(dir).Load();
            Assert.Equal(0.25, cfg.Spammer.ActiveKeys["*0"]);

            var adopted = new ConfigLoader(dir).LoadLocal()!;
            Assert.Equal(0.25, adopted.Spammer!.Presets!["default"]["*0"]);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public void UpdateLocalPersistsSpammerPresets()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            var loader = new ConfigLoader(dir);
            loader.UpdateLocal(local => local.Spammer = new ConfigLoader.LocalSpammer
            {
                Active = "Knight0-9",
                Presets = new System.Collections.Generic.Dictionary<string, System.Collections.Generic.Dictionary<string, double>>
                {
                    ["Knight0-9"] = new System.Collections.Generic.Dictionary<string, double>
                    {
                        ["*0"] = 0.2,
                        ["F1"] = 1.5,
                    },
                },
            });

            var reloaded = new ConfigLoader(dir).Load();
            Assert.Equal("Knight0-9", reloaded.Spammer.Active);
            Assert.Equal(1.5, reloaded.Spammer.ActiveKeys["F1"]);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // ActiveKeys used to fall back to another preset when the active name was missing, so the
    // spammer would press a different rotation with nothing indicating which. Empty is the safe
    // answer, and SkillSpammer reports it on the card.
    [Fact]
    public void ActiveKeysIsEmptyWhenTheActivePresetIsMissing()
    {
        var dir = MakeTempConfigDirWithLocal(
            ValidOcrLocal +
            "spammer:\n  active: DoesNotExist\n  presets:\n    Real:\n      '*0': 0.2\n");
        try
        {
            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal("DoesNotExist", cfg.Spammer.Active);
            Assert.Empty(cfg.Spammer.ActiveKeys);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // YamlDotNet sets a property to null when its key is present with no value, which beats the
    // field initializer. The validator promises a descriptive ConfigException first, so it must not
    // dereference that null and die with a NullReferenceException instead.
    [Fact]
    public void LoadRejectsAValueLessBandWithAMessageNotANullReference()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal.Replace("grade_y: [10, 40]", "grade_y:"));
        try
        {
            var ex = Assert.Throws<ConfigException>(() => new ConfigLoader(dir).Load());
            Assert.Contains("grade_y", ex.Message);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // The same trap as the value-less band above, one level up: a bare `tuner:` line makes the whole
    // SECTION null, and the validator read `c.Tuner.Ocr` before any guard could report it — so a
    // hand-edited file died with a NullReferenceException instead of the descriptive ConfigException
    // the validator exists to produce. Found by the 2026-09-22 audit.
    [Fact]
    public void LoadRejectsAnEmptyTunerSectionWithAMessageNotANullReference()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            var path = Path.Combine(dir, "defaults.yaml");
            // Collapse the whole tuner block to a value-less key: everything from `tuner:` up to the
            // next top-level line goes away, which is what a hand-edit that empties the section does.
            var collapsed = System.Text.RegularExpressions.Regex.Replace(
                File.ReadAllText(path), @"(?ms)^tuner:.*?(?=^[^\s#])", "tuner:\n");

            // If this fails the collapse did not happen and the test below would prove nothing.
            Assert.DoesNotContain("grade_order", collapsed);
            File.WriteAllText(path, collapsed);

            var ex = Assert.Throws<ConfigException>(() => new ConfigLoader(dir).Load());
            Assert.Contains("tuner", ex.Message);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }

    // The audit's finding, as a fence. `EveryLocalPetFieldIsCopiedFromTheConfig` walks LocalPet ->
    // PetConfig, so a PetConfig property with NO file counterpart is invisible to it — which is how
    // four of them accumulated without anything noticing. This walks the other way: every settable
    // property on PetConfig must be accounted for, either as one LocalPet carries, as one the portable
    // projection writes, or with a stated reason for neither.
    //
    // Like its twin, this can only fail when someone ADDS a property — that is the point: the failure
    // is the point at which the decision should be made, rather than a player discovering that the
    // setting they hand-edited was deleted by the next Save.
    [Fact]
    public void EveryPetConfigFieldIsCarriedByAFileOrDeliberatelyExcluded()
    {
        var portable = new[] { nameof(PetConfig.ItemsPerMinute), nameof(PetConfig.PetSlotOccupiedAbove) };

        var excluded = new Dictionary<string, string>
        {
            [nameof(PetConfig.FeederSlotA)] =
                "legacy food-count box. Read by nothing since FeederLayout derived the strip from the " +
                "row geometry; kept rather than deleted so a hand-edited feeder_slot_a: does not become " +
                "an unknown key in someone's defaults.yaml",
            [nameof(PetConfig.FeederSlotB)] = "same as FeederSlotA",
        };

        var uncovered = typeof(PetConfig).GetProperties()
            .Where(p => p.CanRead && p.CanWrite)
            .Select(p => p.Name)
            .Where(n => System.Array.IndexOf(portable, n) < 0
                        && !excluded.ContainsKey(n)
                        && typeof(ConfigLoader.LocalPet).GetProperty(n) == null)
            .ToList();

        Assert.True(uncovered.Count == 0,
            "These PetConfig properties have no home in any config file, so nothing can set them and a " +
            "hand-added block is deleted by the next Save. Carry them, or list them with a reason:\n  " +
            string.Join("\n  ", uncovered));
    }

    [Fact]
    public void LoadAttributesParsesDictionary()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            var attrs = new ConfigLoader(dir).LoadAttributes();
            Assert.NotEmpty(attrs.Attributes);
            Assert.Equal("攻擊力", attrs.Attributes[0].Name);
            Assert.Contains("力量", attrs.PerLevelStats);
            Assert.NotEmpty(attrs.TextFixes.Whole);
        }
        finally
        {
            Directory.Delete(dir, recursive: true);
        }
    }
}

using System;
using System.IO;
using SealTools.Core.Config;
using Xunit;

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

    // SaveDefaults rewrites defaults.yaml from an explicit field list, so a field that exists in
    // defaults.yaml but is missing from that list is silently dropped on every launcher Save.
    // (ocr_retries was: 3 -> 0, which quietly disabled the OCR re-scan.)
    [Fact]
    public void SaveDefaultsPreservesEveryPortableField()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            var loader = new ConfigLoader(dir);
            var before = loader.Load();
            Assert.Equal(3, before.Tuner.OcrRetries);

            // Set a value that differs from the property default, or the assertions below pass even
            // when the field is missing from the projection — which is how move_mode was lost.
            before.Gem.MoveMode = "tuned";

            loader.SaveDefaults(before);

            var after = new ConfigLoader(dir).Load();
            Assert.Equal(before.Tuner.OcrRetries, after.Tuner.OcrRetries);
            Assert.Equal(before.Tuner.SaveCaptures, after.Tuner.SaveCaptures);
            Assert.Equal(before.Tuner.MaxRetries, after.Tuner.MaxRetries);
            Assert.Equal(before.Tuner.TargetGrade, after.Tuner.TargetGrade);
            Assert.Equal(before.Tuner.Timing.OcrDelay, after.Tuner.Timing.OcrDelay);
            Assert.Equal(before.Tuner.SpringMode, after.Tuner.SpringMode);
            Assert.Equal(before.Tuner.MouseGuard, after.Tuner.MouseGuard);
            Assert.Equal(before.Tuner.GuardPx, after.Tuner.GuardPx);
            Assert.Equal(before.Tuner.RecenterMax, after.Tuner.RecenterMax);
            Assert.Equal(before.Window.Title, after.Window.Title);
            Assert.Equal(before.Arduino.Baud, after.Arduino.Baud);
            // spammer is deliberately NOT in this list — see SaveDefaultsLeavesSpammerPresetsAlone.
            Assert.Equal("tuned", after.Gem.MoveMode);
            Assert.Equal(before.Gem.StartGrade, after.Gem.StartGrade);
            Assert.Equal(before.Gem.EmptyMode, after.Gem.EmptyMode);
            Assert.Equal(before.Gem.ColoredGapMin, after.Gem.ColoredGapMin);
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
    public void SaveLocalPersistsTheCalibrationEnvironmentBlock()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            var loader = new ConfigLoader(dir);
            var local = loader.LoadLocal()!;
            local.Calibration = new CalibrationInfo
            {
                DpiScale = 1.5,
                MonitorDpi = 144,
                Screen = new System.Collections.Generic.List<int> { 3840, 2160 },
                ClientSize = new System.Collections.Generic.List<int> { 2865, 1789 },
                MeasuredAt = "2026-09-09 21:00:00",
            };
            loader.SaveLocal(local);

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

    // SaveDefaults serialises the MERGED in-memory config and is called by the tuner, gem and
    // hotkeys tabs. If spammer were in its field list, saving any of those would push the player's
    // personal rotations into the defaults.yaml that ships.
    [Fact]
    public void SaveDefaultsLeavesSpammerPresetsAlone()
    {
        var dir = MakeTempConfigDirWithLocal(
            ValidOcrLocal +
            "spammer:\n  active: Personal\n  presets:\n    Personal:\n      '*0': 0.2\n");
        try
        {
            var loader = new ConfigLoader(dir);
            var cfg = loader.Load();
            Assert.Equal("Personal", cfg.Spammer.Active);

            loader.SaveDefaults(cfg);

            // Assert on the file itself, not a reload: loading merges local.yaml back on top, so a
            // reloaded Active is "Personal" again by design. The leak this guards against is
            // "Personal" reaching the shipped defaults.yaml on disk.
            var written = File.ReadAllText(Path.Combine(dir, "defaults.yaml"));
            Assert.DoesNotContain("Personal", written);

            // No spammer KEY, which is what this actually means. It used to assert the word never
            // appears at all, and that stopped being the same thing once the generated header began
            // explaining why there is no spammer section — prose that contains the word while
            // writing no key. Matching the parsed key is both narrower and truer than matching a
            // substring, and it cannot be satisfied by adding a comment.
            Assert.DoesNotContain("\nspammer:", written);
            Assert.False(written.StartsWith("spammer:", StringComparison.Ordinal),
                "defaults.yaml must not open with a spammer key");
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

            loader.SaveDefaults(cfg);
            loader.SaveLocal(loader.LoadLocal()!);

            foreach (var name in new[] { "defaults.yaml", "local.yaml" })
            {
                var written = File.ReadAllText(Path.Combine(dir, name));
                Assert.StartsWith("#", written);              // a header, not bare serialised content
                Assert.Contains("REWRITES this whole file", written);
                Assert.Contains("DROPPED", written);
            }
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

            loader.SaveDefaults(cfg);

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
    [Fact]
    public void PetCalibrationRoundTripsAndKeepsItsOwnBagGrid()
    {
        var dir = MakeTempConfigDirWithLocal(ValidOcrLocal);
        try
        {
            var loader = new ConfigLoader(dir);
            var local = loader.LoadLocal() ?? new ConfigLoader.LocalOverrides();
            local.Pet = new ConfigLoader.LocalPet
            {
                MenuButton = new List<int> { 815, 1735 },
                FeedIcon = new List<int> { 900, 1400 },
                CloseButton = new List<int> { 880, 140 },
                PageTabs = new List<List<int>> { new() { 700, 180 }, new() { 745, 180 }, new() { 790, 180 } },
                ToggleLabel = new List<int> { 590, 200, 120, 30 },
                BoardingPetSlot = new List<int> { 190, 210, 60, 60 },
                FeederSlotA = new List<int> { 250, 210, 60, 60 },
                FeederSlotB = new List<int> { 320, 210, 60, 60 },
                BagGrid = new List<int> { 660, 150, 408, 408 },
                BagSlot = new List<int> { 660, 150, 51, 51 },
                FoodCells = new List<List<int>> { new() { 0, 5 }, new() { 1, 12 } },
            };
            loader.SaveLocal(local);

            var cfg = new ConfigLoader(dir).Load();

            Assert.Equal(new List<int> { 815, 1735 }, cfg.Pet.MenuButton);
            Assert.Equal(new List<int> { 900, 1400 }, cfg.Pet.FeedIcon);
            Assert.Equal(new List<int> { 590, 200, 120, 30 }, cfg.Pet.ToggleLabel);
            Assert.Equal(new List<int> { 190, 210, 60, 60 }, cfg.Pet.BoardingPetSlot);
            Assert.Equal(new List<int> { 250, 210, 60, 60 }, cfg.Pet.FeederSlotA);
            Assert.Equal(new List<int> { 320, 210, 60, 60 }, cfg.Pet.FeederSlotB);
            Assert.Equal(3, cfg.Pet.PageTabs.Count);
            Assert.Equal(2, cfg.Pet.FoodCells.Count);
            Assert.Equal(new List<int> { 660, 150, 408, 408 }, cfg.Pet.BagGrid);
            Assert.Equal(200, cfg.Pet.LoadMinutes);   // 600 items at 3/min

            // The two grids are independent calibrations, and this is the assertion that keeps them so.
            Assert.Null(cfg.BuySell.BagGrid);

            // A zero-sized rectangle is not a calibration and must not be adopted — the same guard the
            // buy/sell grid gets. Without it all 64 derived centres land on the same pixel.
            local.Pet.BagGrid = new List<int> { 0, 0, 0, 0 };
            new ConfigLoader(dir).SaveLocal(local);
            Assert.Null(new ConfigLoader(dir).Load().Pet.BagGrid);
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

            new ConfigLoader(dir).SaveDefaults(cfg);
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
    public void SaveLocalPersistsSpammerPresets()
    {
        var dir = MakeTempConfigDir(includeLocal: true);
        try
        {
            var loader = new ConfigLoader(dir);
            var local = loader.LoadLocal()!;
            local.Spammer = new ConfigLoader.LocalSpammer
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
            };
            loader.SaveLocal(local);

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

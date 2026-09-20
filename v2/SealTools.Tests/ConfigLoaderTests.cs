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
                BagGrid = new List<int> { 660, 150, 408, 408 },
                BagSlot = new List<int> { 660, 150, 51, 51 },
                FoodSlots = new List<List<int>> { new() { 0, 5 }, new() { 1, 12 } },
                ReturnSlot = new List<int> { 1, 2 },
                // The count reference. It is a RECTANGLE, and the loader applied the "is this a point"
                // predicate to it — Count == 2, which a rect never is — so both boxes were written on
                // save and silently DISCARDED on every load. The player's symptom was the Test read
                // telling them to draw boxes they had already drawn.
                // Arbitrary on purpose. Borrowing the player's real coordinates made the test read
                // as though it depended on their calibration; all it needs is a rectangle that is not
                // two values long, which is exactly what the old predicate got wrong.
                FeederCountSlot = new List<int> { 11, 12, 13, 14 },
                FeederCountText = new List<int> { 21, 22, 23, 24 },
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
            };
            loader.SaveLocal(local);

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
            // that matters is the ROUND TRIP: written by SaveLocal, read back by Load.
            Assert.Equal(new List<int> { 11, 12, 13, 14 }, cfg.Pet.FeederCountSlot);
            Assert.Equal(new List<int> { 21, 22, 23, 24 }, cfg.Pet.FeederCountText);
            Assert.Equal(5, slot.Stacks);
            Assert.True(slot.BoardingRunning);

            // Derived, per row: 5 stacks = 1,500 items at 3/min = 500 minutes.
            Assert.Equal(1500, PetConfig.LoadItemsFor(slot));
            Assert.Equal(500, cfg.Pet.LoadMinutesFor(slot));

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
            ReturnSlot = new List<int> { 34, 35 },
            MaxButton = new List<int> { 40, 41 },
            WaitAfterEmptyMinutes = 3,
            ActionWaitMs = 1234,
            FoodLoadMode = SealTools.Core.FoodLoadMode.Drag,
            // Non-default on purpose: against the defaults this would pass whether or not either
            // half copied the field, which is the trap this guard's own comment warns about.
            FeederCountSlot = new List<int> { 21, 22, 23, 24 },
            FeederCountText = new List<int> { 25, 26, 27, 28 },
            FeederCountShiftPx = 4,
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
                new() { Label = "p", Rect = new List<int> { 36, 37, 38, 39 }, Png = "icon-png" },
            },
        };

        var local = ConfigLoader.LocalPet.From(cfg);

        // The nested shapes are MAPPED rather than assigned — a LocalPetSlot is a different type from
        // a PetSlotConfig — so reference equality is the wrong check for them and they are asserted
        // field by field below instead. Everything else is a straight assignment and is compared here.
        var mapped = new[] { nameof(ConfigLoader.LocalPet.Slots), nameof(ConfigLoader.LocalPet.Queue) };

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
            FeederCountSlot = new List<int> { 21, 22, 23, 24 },
            FeederCountText = new List<int> { 25, 26, 27, 28 },
            FeederCountShiftPx = 4,
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
                new() { Label = "p", Rect = new List<int> { 36, 37, 38, 39 }, Png = "icon-png" },
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
        var mapped = new[] { nameof(ConfigLoader.LocalPet.Slots), nameof(ConfigLoader.LocalPet.Queue) };

        foreach (var prop in typeof(ConfigLoader.LocalPet).GetProperties())
        {
            if (mapped.Contains(prop.Name)) continue;
            var source = typeof(PetConfig).GetProperty(prop.Name)!.GetValue(cfg);
            var byCalibration = Equals(prop.GetValue(calibrationOnly), source);
            var bySession = Equals(prop.GetValue(sessionOnly), source);

            // LIMIT OF THIS GUARD, and it cost a bug: "either half" is the wrong question for a field
            // whose UI lives on ONE tab. FeederCountShiftPx was written by the CALIBRATION half while
            // its box sits on the Pet tab, so it saved from Calibrate Pet and silently reverted from
            // the Pet tab — and this test passed both before and after the fix. The pairing that
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
            var local = loader.LoadLocal() ?? new ConfigLoader.LocalOverrides();
            local.Pet = ConfigLoader.LocalPet.From(migrated.Pet);
            loader.SaveLocal(local);

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
            var local = loader.LoadLocal() ?? new ConfigLoader.LocalOverrides();
            local.Pet = ConfigLoader.LocalPet.From(new PetConfig
            {
                WaitAfterEmptyMinutes = 2,
                ActionWaitMs = 1234,
            });
            loader.SaveLocal(local);

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

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

            loader.SaveDefaults(before);

            var after = new ConfigLoader(dir).Load();
            Assert.Equal(before.Tuner.OcrRetries, after.Tuner.OcrRetries);
            Assert.Equal(before.Tuner.SaveCaptures, after.Tuner.SaveCaptures);
            Assert.Equal(before.Tuner.MaxRetries, after.Tuner.MaxRetries);
            Assert.Equal(before.Tuner.TargetGrade, after.Tuner.TargetGrade);
            Assert.Equal(before.Tuner.Timing.OcrDelay, after.Tuner.Timing.OcrDelay);
            Assert.Equal(before.Window.Title, after.Window.Title);
            Assert.Equal(before.Arduino.Baud, after.Arduino.Baud);
            Assert.Equal(before.Spammer.Active, after.Spammer.Active);
            Assert.Equal(before.Spammer.ActiveKeys, after.Spammer.ActiveKeys);
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
            var text = File.ReadAllText(path);
            var spammerAt = text.IndexOf("spammer:", StringComparison.Ordinal);
            Assert.True(spammerAt > 0, "defaults.yaml should end with the spammer block");
            File.WriteAllText(path, text[..spammerAt] + "spammer:\n  keys:\n    '*0': 0.25\n    'F1': 5.0\n");

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

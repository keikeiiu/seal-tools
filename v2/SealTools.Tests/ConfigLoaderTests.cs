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
            Assert.Equal(-30, cfg.Gem.Movements.RegisterCombine[0]);
            Assert.Equal(80, cfg.Gem.Movements.RegisterCombine[1]);
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

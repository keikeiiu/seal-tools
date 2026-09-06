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

        if (File.Exists(localPath))
        {
            var local = Deserialize<LocalOverrides>("local.yaml");
            ApplyOverrides(defaults, local);
        }
        else
        {
            throw new ConfigException(
                "config/local.yaml not found and config/local.yaml.example is missing, so it cannot be created.");
        }

        ConfigValidator.Validate(defaults);
        return defaults;
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
        if (local.Tuner?.Ocr != null) defaults.Tuner.Ocr = local.Tuner.Ocr;
        if (local.Gem?.GradePositions is { Count: > 0 }) defaults.Gem.GradePositions = local.Gem.GradePositions;
        if (local.Gem?.Movements != null) defaults.Gem.Movements = local.Gem.Movements;
        if (!string.IsNullOrEmpty(local.Arduino?.Port)) defaults.Arduino.Port = local.Arduino.Port;
        if (local.Display?.DpiScale != null) defaults.Display.DpiScale = local.Display.DpiScale;
    }

    // Atomic write so the launcher can save config while tools re-read it.
    public void SaveLocal(LocalOverrides local)
    {
        var path = PathOf("local.yaml");
        var yaml = Serializer.Serialize(local);
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
                models = cfg.Tuner.Models,
                timing = cfg.Tuner.Timing,
                grade_colors = cfg.Tuner.GradeColors,
                filter = cfg.Tuner.Filter,
            },
            gem = new { grades = cfg.Gem.Grades, start_grade = cfg.Gem.StartGrade },
            spammer = new { keys = cfg.Spammer.Keys },
        };

        var path = PathOf("defaults.yaml");
        var yaml = Serializer.Serialize(portable);
        var tmp = path + ".tmp";
        File.WriteAllText(tmp, yaml);
        File.Move(tmp, path, overwrite: true);
    }

    // Machine-specific overlay shape (mirrors local.yaml).
    public sealed class LocalOverrides
    {
        public LocalTuner? Tuner { get; set; }
        public LocalGem? Gem { get; set; }
        public LocalArduino? Arduino { get; set; }
        public DisplayOverrides? Display { get; set; }
    }

    public sealed class LocalTuner
    {
        public OcrGeometry? Ocr { get; set; }
    }

    public sealed class LocalGem
    {
        public Dictionary<string, List<int>>? GradePositions { get; set; }
        public MovementsConfig? Movements { get; set; }
    }

    public sealed class LocalArduino
    {
        public string? Port { get; set; }
    }

    public sealed class DisplayOverrides
    {
        public double? DpiScale { get; set; }
    }
}

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

        ConfigValidator.Validate(defaults);
        return defaults;
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
        var yaml = Serializer.Serialize(portable);
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
    }

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

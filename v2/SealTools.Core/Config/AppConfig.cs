using System.Collections.Generic;
using System.ComponentModel.DataAnnotations;

namespace SealTools.Core.Config;

// Strongly-typed config graph. Mirrors v2/config/defaults.yaml (+ local.yaml overlay).
// No hardcoded defaults: every value originates from the YAML files.

public sealed class AppConfig
{
    public WindowConfig Window { get; set; } = new();
    /// <summary>Environment the machine-specific calibration was measured in (local.yaml only).</summary>
    public CalibrationInfo Calibration { get; set; } = new();
    public ReferenceWindowConfig ReferenceWindow { get; set; } = new();
    public ArduinoConfig Arduino { get; set; } = new();
    public HotkeysConfig Hotkeys { get; set; } = new();
    public TunerConfig Tuner { get; set; } = new();
    public GemConfig Gem { get; set; } = new();
    public SpammerConfig Spammer { get; set; } = new();
}

public sealed class WindowConfig
{
    [Required(ErrorMessage = "window.title is required")]
    public string Title { get; set; } = "";
}

public sealed class ReferenceWindowConfig
{
    public int Width { get; set; }
    public int Height { get; set; }
}

/// <summary>The display environment a calibration was measured in — written by the calibrators,
/// read by the Setup tab. All sizes are PHYSICAL pixels. Storing the scale makes the coordinates
/// portable: another machine can tell it must convert or recalibrate (see docs/COORDINATES.md).</summary>
public sealed class CalibrationInfo
{
    /// <summary>Physical px per logical px at calibration time (e.g. 1.5).</summary>
    public double? DpiScale { get; set; }
    /// <summary>Effective monitor DPI at calibration time (96 * dpi_scale, e.g. 144).</summary>
    public uint? MonitorDpi { get; set; }
    /// <summary>Primary screen size in physical pixels [width, height].</summary>
    public List<int>? Screen { get; set; }
    /// <summary>Game client size in physical pixels [width, height].</summary>
    public List<int>? ClientSize { get; set; }
    /// <summary>When the calibration was saved (informational).</summary>
    public string? MeasuredAt { get; set; }
}

public sealed class ArduinoConfig
{
    [Range(1, int.MaxValue, ErrorMessage = "arduino.vid must be > 0")]
    public int Vid { get; set; }
    public List<int> Pid { get; set; } = new();
    public int Baud { get; set; }
    public string Port { get; set; } = "";   // empty = auto-detect
}

public sealed class HotkeysConfig
{
    public int Start { get; set; }
    public int Quit { get; set; }
    public int AdvanceGrade { get; set; }
    public int Pause { get; set; }
}

public sealed class TunerConfig
{
    public List<string> GradeOrder { get; set; } = new();
    public string TargetGrade { get; set; } = "";
    public int MaxRetries { get; set; }
    public bool SaveCaptures { get; set; }
    /// <summary>Re-scan up to this many times if a scan looks unconfirmed, then force-capture + log.</summary>
    public int OcrRetries { get; set; }
    public TimingConfig Timing { get; set; } = new();
    public GradeColorsConfig GradeColors { get; set; } = new();
    public ModelsConfig Models { get; set; } = new();
    public FilterConfig Filter { get; set; } = new();
    public OcrGeometry Ocr { get; set; } = new();   // machine-specific; overridden by local.yaml
}

public sealed class TimingConfig
{
    public double ClickEnterDelay { get; set; }
    public double OcrDelay { get; set; }
}

public sealed class OcrGeometry
{
    public RegionConfig Region { get; set; } = new();
    public BoxConfig GradeArea { get; set; } = new();
    public List<int> GradeY { get; set; } = new();
    public List<int> AttrY { get; set; } = new();
    public List<int> RemainingY { get; set; } = new();
    public List<int> AttrX { get; set; } = new();
    public List<int> RemainingX { get; set; } = new();
    public int RowHeight { get; set; }
}

public sealed class RegionConfig
{
    public int Left { get; set; }
    public int Top { get; set; }
    [Range(1, int.MaxValue, ErrorMessage = "tuner.ocr.region width/height must be > 0")]
    public int Width { get; set; }
    [Range(1, int.MaxValue, ErrorMessage = "tuner.ocr.region width/height must be > 0")]
    public int Height { get; set; }
}

public sealed class BoxConfig
{
    public int X1 { get; set; }
    public int Y1 { get; set; }
    public int X2 { get; set; }
    public int Y2 { get; set; }
}

public sealed class GradeColorsConfig
{
    public YellowMaskConfig Yellow { get; set; } = new();
    public BlueMaskConfig Blue { get; set; } = new();
    public RedMaskConfig Red { get; set; } = new();
    public PurpleMaskConfig Purple { get; set; } = new();
    public WhiteMaskConfig White { get; set; } = new();
    public GradeDecisionConfig Decision { get; set; } = new();
}

public sealed class YellowMaskConfig { public int RgGap { get; set; } public int GbGap { get; set; } public int RMin { get; set; } public int GMin { get; set; } }
public sealed class BlueMaskConfig { public int BgGap { get; set; } public int BrGap { get; set; } public int BMin { get; set; } }
public sealed class RedMaskConfig { public int RgGap { get; set; } public int RbGap { get; set; } public int RMin { get; set; } }
public sealed class PurpleMaskConfig { public int RbGap { get; set; } public int BgGap { get; set; } public int RMin { get; set; } public int BMin { get; set; } }
public sealed class WhiteMaskConfig { public int RMin { get; set; } public int GMin { get; set; } public int BMin { get; set; } }

public sealed class GradeDecisionConfig
{
    public int TotalMin { get; set; }
    public int DgYellow { get; set; }
    public int GBlue { get; set; }
    public int XgRed { get; set; }
    public int SgPurple { get; set; }
    public int NWhite { get; set; }
    public int NYellowMax { get; set; }
    public int NBlueMax { get; set; }
}

public sealed class FilterRule
{
    public string Name { get; set; } = "";
    public int Count { get; set; } = 1;
    public int? Min { get; set; }
    public int? Max { get; set; }
}

public sealed class FilterConfig
{
    public bool Enabled { get; set; }
    [AllowedValues("any", "all", "per_attr", ErrorMessage = "tuner.filter.match_mode must be any|all|per_attr")]
    public string MatchMode { get; set; } = "any";
    public string? RequireGrade { get; set; }
    public List<FilterRule> Rules { get; set; } = new();
    public List<FilterRule> OverrideRules { get; set; } = new();
}

public sealed class GemConfig
{
    public List<string> Grades { get; set; } = new();
    public string StartGrade { get; set; } = "";
    [MinLength(1, ErrorMessage = "gem.grade_positions must not be empty")]
    public Dictionary<string, List<int>> GradePositions { get; set; } = new();
    public MovementsConfig Movements { get; set; } = new();
    /// <summary>Which move set the composer uses between calibrated points (see
    /// <see cref="SealTools.Core.GemRoutes"/>): "arduino" (default since 2.2) places the cursor on
    /// the destination point with the Arduino, closed loop, and re-aims every move; "tuned" sends
    /// the hand-tuned gem.movements counts instead.</summary>
    [AllowedValues("tuned", "arduino", ErrorMessage = "gem.move_mode must be tuned|arduino")]
    public string MoveMode { get; set; } = "arduino";
    /// <summary>Composed-result gem area as [x, y, width, height] (client-relative). Used for
    /// OCR (read the result) and as a derived centre click point.</summary>
    public List<int>? ResultGemArea { get; set; }
    /// <summary>Centre of the result-gem area, used as the click point. Null if the area
    /// isn't valid (missing or not exactly [x, y, w, h]).</summary>
    public (int X, int Y)? ResultGemCenter =>
        ResultGemArea is { Count: 4 }
            ? (ResultGemArea[0] + ResultGemArea[2] / 2, ResultGemArea[1] + ResultGemArea[3] / 2)
            : null;
    /// <summary>Three resource-gem click points [[x, y], ...] used to click a stuck resource
    /// gem to remove it. One point per composer slot.</summary>
    public List<List<int>> ResourceGems { get; set; } = new();
    /// <summary>Action when the auto-check sees the result gem box empty. "stop" halts the
    /// composer; "advance_grade" moves to the next grade and keeps composing;
    /// "advance_grade_clear" right-clicks all 3 resource slots (clears a stuck gem) then
    /// advances to the next grade.</summary>
    [AllowedValues("stop", "advance_grade", "advance_grade_clear", ErrorMessage = "gem.empty_mode must be stop|advance_grade|advance_grade_clear")]
    public string EmptyMode { get; set; } = "stop";
    /// <summary>How different the result box may look from the saved empty-box crop before it
    /// counts as holding a gem (0..1). With the pixel-difference test this is the FRACTION of
    /// pixels allowed to differ; with the colour-signature fallback it is the colour distance.
    /// Measured on the reference machine: empty 0.000 (pixel-identical) and 0.001 (signature),
    /// gem 0.357 / 0.293 — so 0.01 is ~35x below the gem signal and 10x above the floor: a gem
    /// can never read as empty, while a few dozen stray pixels don't stall the composer.
    /// Lower = stricter (quicker to say "has a gem").</summary>
    public double EmptyDistance { get; set; } = 0.01;
    /// <summary>Channel max-min gap (0..255) above which a pixel counts as "coloured" in the
    /// result-box colour fingerprint. Portable: the empty reference self-calibrates, this is
    /// just the pixel-classification threshold.</summary>
    [Range(0, 255, ErrorMessage = "gem.colored_gap_min must be 0-255")]
    public int ColoredGapMin { get; set; } = 40;
    /// <summary>Consecutive empty reads required before the EmptyMode action fires — guards
    /// against a single transient false empty.</summary>
    public int EmptyStreak { get; set; } = 2;
    /// <summary>Write the empty-check capture PNG + distance log line for debugging. Default
    /// off — the per-cycle disk I/O slows the compose loop.</summary>
    public bool SaveEmptyCaptures { get; set; }
    /// <summary>Sampled empty-result-gem colour reference (machine-specific). Null = empty
    /// detection is off until calibrated.</summary>
    public ColorComposition? EmptySignature { get; set; }
}

public sealed class MovementsConfig
{
    public Dictionary<string, List<int>> RadioToRegister { get; set; } = new();
    public List<int> RegisterCombine { get; set; } = new();
    public List<int> CombineRegister { get; set; } = new();
    public List<int> GradeNext { get; set; } = new();
    public List<int> GradePrev { get; set; } = new();
    public List<int> RegisterSlot1 { get; set; } = new();
    public List<int> Slot1Slot2 { get; set; } = new();
    public List<int> Slot2Slot3 { get; set; } = new();
    public List<int> Slot3ToN { get; set; } = new();
    public List<int> Slot3ToG { get; set; } = new();
    public List<int> Slot3ToDg { get; set; } = new();
}

public sealed class SpammerConfig
{
    /// <summary>Name of the preset the spammer presses.</summary>
    public string Active { get; set; } = "default";

    /// <summary>Named key sets — preset name → (key → cooldown seconds). Switch presets in the UI
    /// to run a different skill rotation.</summary>
    public Dictionary<string, Dictionary<string, double>> Presets { get; set; } = new();

    /// <summary>Legacy flat key list (pre-presets). Read on load for migration only — SpammerConfig
    /// is never serialized (SaveDefaults writes an explicit shape), so this is not written back.</summary>
    public Dictionary<string, double>? Keys { get; set; }

    /// <summary>The keys of the active preset (empty when it is missing).</summary>
    public Dictionary<string, double> ActiveKeys =>
        Presets.TryGetValue(Active, out var keys) ? keys
        : Presets.Count > 0 ? Presets.Values.First() : new Dictionary<string, double>();
}

public sealed class ModelsConfig
{
    public string Detector { get; set; } = "";
    public string Recognizer { get; set; } = "";
    public string Classifier { get; set; } = "";
}

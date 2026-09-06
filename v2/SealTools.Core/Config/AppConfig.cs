using System.Collections.Generic;
using System.ComponentModel.DataAnnotations;

namespace SealTools.Core.Config;

// Strongly-typed config graph. Mirrors v2/config/defaults.yaml (+ local.yaml overlay).
// No hardcoded defaults: every value originates from the YAML files.

public sealed class AppConfig
{
    public WindowConfig Window { get; set; } = new();
    public ReferenceWindowConfig ReferenceWindow { get; set; } = new();
    public ArduinoConfig Arduino { get; set; } = new();
    public HotkeysConfig Hotkeys { get; set; } = new();
    public TunerConfig Tuner { get; set; } = new();
    public GemConfig Gem { get; set; } = new();
    public SpammerConfig Spammer { get; set; } = new();
    public DisplayConfig Display { get; set; } = new();
}

public sealed class DisplayConfig
{
    public double? DpiScale { get; set; }   // optional manual override; null = auto-detect via GetDpiForWindow
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
    public double DgWhiteRatio { get; set; }
    public int GBlue { get; set; }
    public double GWhiteRatio { get; set; }
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
    public List<int> Slot2Dg { get; set; } = new();
}

public sealed class SpammerConfig
{
    public Dictionary<string, double> Keys { get; set; } = new();
}

public sealed class ModelsConfig
{
    public string Detector { get; set; } = "";
    public string Recognizer { get; set; } = "";
    public string Classifier { get; set; } = "";
}

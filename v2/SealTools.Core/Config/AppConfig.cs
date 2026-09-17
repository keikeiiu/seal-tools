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
    public BuySellConfig BuySell { get; set; } = new();
    public PetConfig Pet { get; set; } = new();
    public TooltipConfig Tooltip { get; set; } = new();
}

/// <summary>Where the game's hover panel sits relative to the cursor — see
/// docs/PLAN-HOVER-INFO.md.
///
/// A capability, not a setting of any one tool: the panel is anchored to the pointer, so its position
/// relative to the pointer is the same everywhere in the game even though its SIZE varies by what is
/// under it. Calibrating that offset once serves every hover-read this suite ever does — a pet's name
/// and growth, a food stack's count, whatever wants one next.
///
/// All four values are client-relative PHYSICAL pixels, like every other calibrated coordinate here,
/// which is why they belong in local.yaml: a box measured at one machine's scale means nothing at
/// another's.</summary>
public sealed class TooltipConfig
{
    /// <summary>Panel's left edge relative to the cursor. Negative when the panel sits left of the
    /// pointer, which is what happens near a screen edge if the game flips it.</summary>
    public int OffsetX { get; set; }

    /// <summary>Panel's top edge relative to the cursor.</summary>
    public int OffsetY { get; set; }

    /// <summary>The captured box's size. Deliberately sized for the LARGEST panel rather than one per
    /// item type: a smaller panel then leaves background behind it, which OCR ignores, where a
    /// per-type box would stop the offset being universal — the entire point of calibrating it.</summary>
    public int Width { get; set; }
    public int Height { get; set; }

    /// <summary>How long the panel takes to appear after the cursor settles on an item. Too short and
    /// the capture finds an empty region, which reads exactly like "nothing here" — the same class of
    /// silent failure as a click that outruns an animation.</summary>
    public int HoverDelayMs { get; set; } = 700;

    /// <summary>True once a box has been dragged. The offset alone is meaningless without a size to
    /// read, and a size without an offset would read the wrong place.</summary>
    public bool IsSet => Width > 0 && Height > 0;
}

/// <summary>Geometry for the pet food auto-replacement tool — the boarding (代養) flow. Every value
/// here is machine-specific, so all of it lives in local.yaml beside the buy/sell grid; nothing
/// belongs in the portable defaults.yaml.
///
/// See docs/PLAN-PET-AUTOFEED.md. Two things about this shape are worth knowing before reading it:
/// the bag opened by the boarding window is at a DIFFERENT place from the one the shop opens beside,
/// so <see cref="BagGrid"/> is its own calibration and must not be shared with
/// <see cref="BuySellConfig.BagGrid"/>; and the points below are the click path, which is
/// 目錄 → pet feed icon → boarding window, not the pet cartoon image (that opens a different window
/// entirely, about the manual feeding system).</summary>
public sealed class PetConfig
{
    // ── The click path into the boarding window ─────────────────────────────

    /// <summary>The 目錄 button in the bottom-left icon cluster. Opens the secondary icon panel.</summary>
    public List<int>? MenuButton { get; set; }

    /// <summary>The pet feed icon in that panel — the chick holding a bottle. Opens the boarding
    /// window, and the bag comes up with it. A POINT rather than a crop: it sits at a fixed position
    /// in a static grid, so nothing has to be found.</summary>
    public List<int>? FeedIcon { get; set; }

    // NOTE: there is deliberately no MAX point here. The boarding count dialog is the SAME dialog the
    // buy/sell tools use, so <see cref="BuySellConfig.MaxButton"/> is reused verbatim (player,
    // 2026-09-17). A second mark for one button would be a second thing to drift, and a wrong MAX
    // here means feeding the pet the wrong quantity.
    //
    // The sequence is not identical though: the boarding dialog takes MAX then ONE Enter. There is no
    // confirmation behind it, so `MaxEnterEnter`'s second Enter must not be sent (see
    // PLAN-PET-AUTOFEED.md §4).

    /// <summary>The boarding window's X. There are two close buttons on screen at once (this one and
    /// the bag's), so the flow has to know which it is pressing.</summary>
    public List<int>? CloseButton { get; set; }

    /// <summary>The bag's ITEM1 / ITEM2 / ITEM3 tab points, in page order. Absolute tabs, not
    /// next/prev: clicking a tab lands on that page whatever page you were on, so nothing has to be
    /// read back to know where the tool is.</summary>
    public List<List<int>> PageTabs { get; set; } = new();

    // ── What the boarding window says ───────────────────────────────────────

    /// <summary>The 開始代養 / 結束代養 button, as a box — the tool CLICKS its centre to start
    /// boarding once the food is loaded. Its label is not read.
    ///
    /// It was briefly planned as a state read: one button both starts and ends boarding, so its label
    /// would say whether the pet is being fed. Dropped, because the schedule already decides when to
    /// reload and there is nothing left for the label to answer.</summary>
    public List<int>? ToggleLabel { get; set; }

    /// <summary>The two feeder slots in the boarding window, as boxes.
    ///
    /// These are the empty-check crops: the same saved-crop-and-differing-pixels test the composer
    /// uses on its result box, which is how a loaded slot is told from an empty one. The current flow
    /// does not read them yet — it reloads on a schedule rather than looking first — so they are
    /// calibrated ahead of the check that will use them rather than because something needs them
    /// today.</summary>
    public List<int>? FeederSlotA { get; set; }
    public List<int>? FeederSlotB { get; set; }

    // ── The bag, as THIS flow shows it ──────────────────────────────────────

    /// <summary>Whole bag grid region client-relative physical [x, y, w, h], for the bag the boarding
    /// window opens. NOT interchangeable with <see cref="BuySellConfig.BagGrid"/>: the bag sits
    /// somewhere else here, and sharing the numbers would aim every cell at the wrong item.</summary>
    public List<int>? BagGrid { get; set; }

    /// <summary>One bag slot, the uniformity check on <see cref="BagGrid"/>.</summary>
    public List<int>? BagSlot { get; set; }

    /// <summary>The bag cells holding pet food, each as [page, cell] with page 0-based. Consumed
    /// highest cell index first, the same rule SellPass uses and for the same reason.</summary>
    public List<List<int>> FoodCells { get; set; } = new();

    /// <summary>The bag cell the pet is put back into, as [page, cell].
    ///
    /// This is the SIMPLE path and it is what the tool uses today: mark where the pet goes and
    /// right-click it. It is honest about its own limitation — when farming, loot takes the first free
    /// slot and the pet does not necessarily land here, so this works while the bag is stable and the
    /// icon matching below is what replaces it. Testing the flow end to end is worth more than getting
    /// the hard case right first.</summary>
    public List<int>? PetCell { get; set; }

    /// <summary>Where the pet item was dragged on the calibration capture. Kept so the box can be
    /// redrawn and re-cropped, rather than being a one-shot. Unused by the current flow — see
    /// <see cref="PetCell"/> for why — and kept because it is the direction the farming case goes.</summary>
    public List<int>? PetIconRect { get; set; }

    /// <summary>The pet's bag icon, cropped from the capture at <see cref="PetIconRect"/> and stored
    /// as a base64 PNG. This is what the tool matches across the 64 cells to find the pet after
    /// boarding ends and drops it into the first free slot.
    ///
    /// A crop PER PET, because the icon is the pet's own portrait (see PLAN-PET-AUTOFEED.md §5). The
    /// multi-pet case is therefore more entries, not a different mechanism.</summary>
    public string? PetIconPng { get; set; }

    /// <summary>Optional override for the count dialog's MAX.
    ///
    /// Normally null and <see cref="BuySellConfig.MaxButton"/> is used — the boarding count dialog is
    /// the same dialog, so a second mark for one button would be a second thing to drift. It exists
    /// because that sameness is the mechanism, which the player confirmed, and NOT the position,
    /// which they could not: the dialog may sit elsewhere in this flow. If a demo run shows it does,
    /// this is the mark to fill rather than a rebuild.</summary>
    public List<int>? MaxButton { get; set; }

    // ── Behaviour ───────────────────────────────────────────────────────────

    /// <summary>How many items one boarding load is: two stacks of the game's 300 cap.</summary>
    public int LoadItems { get; set; } = 600;

    /// <summary>Items the game consumes per minute while boarding runs. Stage 6 is 3; the boarding
    /// window states this itself (`每1分 讀取3個`), so it is a measured game constant rather than a
    /// guess.</summary>
    public int ItemsPerMinute { get; set; } = 3;

    /// <summary>Minutes a full load lasts, derived rather than stored — so the two numbers above can
    /// never disagree with it.</summary>
    public int LoadMinutes => ItemsPerMinute > 0 ? LoadItems / ItemsPerMinute : 0;
}

/// <summary>Geometry and presets for the buy/sell tool. The rectangles are machine-specific — a
/// different PC, resolution or UI skin moves them — so they live in local.yaml beside the gem
/// positions. Two grid rectangles rather than 64 points: see <see cref="SealTools.Core.BagGrid"/>
/// for why that is enough and what the second one is for.</summary>
public sealed class BuySellConfig
{
    /// <summary>Whole bag grid region, client-relative physical [x, y, w, h].</summary>
    public List<int>? BagGrid { get; set; }

    /// <summary>One bag slot in the same space. The uniformity check on <see cref="BagGrid"/>, not a
    /// second source of truth.</summary>
    public List<int>? BagSlot { get; set; }

    /// <summary>Rows the filter list shows at once. A property of the GAME rather than of anyone's
    /// setup, which is why it is not exposed in the UI — but a config value rather than a constant so
    /// a game update that changes it does not need a code change.</summary>
    public int ShopRows { get; set; } = ShopGeometry.DefaultRows;

    /// <summary>The visible part of the shop list, client-relative physical [x, y, w, h]. Dragged
    /// around exactly the rows the list shows. Every row centre is derived from this — see
    /// <see cref="SealTools.Core.ShopGeometry"/> for why one region beats two clicked rows.</summary>
    public List<int>? ShopRegion { get; set; }

    /// <summary>Where the cursor is parked so the wheel scrolls the LIST. The wheel acts on whatever
    /// is under the cursor, so this is not optional.</summary>
    public List<int>? ScrollPoint { get; set; }

    /// <summary>Centre of the MAX button in the count dialog. The only click in a transaction with no
    /// keyboard equivalent — everything else is Enter.</summary>
    public List<int>? MaxButton { get; set; }


    /// <summary>Named buy items (springs, pet food, …). Personal rather than machine-specific, but
    /// they describe positions, so they live in local.yaml with the geometry.</summary>
    public Dictionary<string, BuyPreset> Presets { get; set; } = new();

    /// <summary>Bag slot indices (0 = top-left) selected for selling. Deliberately NOT persisted:
    /// what you are selling is decided fresh each time, and a selection carried over from a previous
    /// session is a selection nobody re-checked. Selling is the one irreversible thing here, so it
    /// starts from an empty grid every launch and reads only what is ticked now.</summary>
    public List<int> SellSlots { get; set; } = new();

    /// <summary>Hard ceiling on slots sold in one run. Enforced in the loop, not advisory — selling
    /// is the one irreversible thing this suite does.</summary>
    public int SellCap { get; set; } = 16;
}

/// <summary>One named thing to buy. Position is (scroll, then row), because the list scrolls and a
/// row index alone would mean nothing.</summary>
public sealed class BuyPreset
{
    /// <summary>Wheel notches down from the top of the list before clicking. 0 = no scrolling.
    /// Tuned by watching a dry run rather than computed, because a scroll position cannot be
    /// observed after the fact.</summary>
    public int Scroll { get; set; }

    /// <summary>Row index within the visible list after scrolling. 0 = the top row.</summary>
    public int Row { get; set; }

    /// <summary>How many to buy per run.</summary>
    public int Count { get; set; } = 1;
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

    /// <summary>Where the tuner puts the cursor at run start: "manual" (today — you position the
    /// mouse on the 發條 button yourself; the tuner only sends C+E) or "hid" (the tuner places it
    /// with the Arduino closed loop and, if the guard is on, keeps it there). Mirrors gem.move_mode.</summary>
    [AllowedValues("manual", "hid", ErrorMessage = "tuner.spring_mode must be manual|hid")]
    public string SpringMode { get; set; } = "manual";

    /// <summary>Client-relative physical pixel point [x, y] of the 發條 button (machine-specific,
    /// written by the calibrate tab). Only read in "hid" spring_mode.</summary>
    public List<int>? SpringPoint { get; set; }

    /// <summary>Cursor guard, "hid" spring_mode only: "off" (never look at the cursor), "stop" (halt
    /// the run when the mouse drifts off the spring — the human-takes-over case), or "recenter" (put
    /// it back on the spring and carry on toward the target grade).</summary>
    [AllowedValues("off", "stop", "recenter", ErrorMessage = "tuner.mouse_guard must be off|stop|recenter")]
    public string MouseGuard { get; set; } = "off";

    /// <summary>How far (px) the cursor may sit from the spring point before the guard acts.</summary>
    public int GuardPx { get; set; } = 8;

    /// <summary>Runaway guard for "recenter": if the cursor has to be re-centred more than this many
    /// times in a row, stop rather than fight a mouse that keeps moving.</summary>
    public int RecenterMax { get; set; } = 20;
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

    /// <summary>The keys of the active preset, empty when that preset is missing. The empty result
    /// is deliberate: falling back to another preset would press a different rotation, silently and
    /// with no indication which one. SkillSpammer reports the empty case on the tool card instead.</summary>
    public Dictionary<string, double> ActiveKeys =>
        Presets.TryGetValue(Active, out var keys) ? keys : new Dictionary<string, double>();
}

public sealed class ModelsConfig
{
    public string Detector { get; set; } = "";
    public string Recognizer { get; set; } = "";
    public string Classifier { get; set; } = "";
}

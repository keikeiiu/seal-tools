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
    public int HoverDelayMs { get; set; } = 1200;

    /// <summary>True once a box has been dragged. The offset alone is meaningless without a size to
    /// read, and a size without an offset would read the wrong place.</summary>
    public bool IsSet => Width > 0 && Height > 0;
}

/// <summary>ONE BREEDING ROW of the `PET BREED` window.
///
/// The window holds four rows — one free, three paid — and they are independent: each has its own
/// start/end button, its own food boxes and its own boarding slot (measured from the capture,
/// 2026-09-19, which also showed all four boarding at once). So every field here is per row, and the
/// list of these is <see cref="PetConfig.Slots"/>.
///
/// The one field that is NOT just geometry: <see cref="Stacks"/>. The free row holds TWO food stacks
/// and a paid row holds FIVE (player, 2026-09-19), which is why the load size is a property of the row
/// rather than of the tool — and why a row's reload interval differs from its neighbour's.</summary>
public sealed class PetSlotConfig
{
    /// <summary>The 開始代養 / 結束代養 button, as a box — the tool CLICKS its centre to start or end
    /// boarding on THIS row. Its label is not read.</summary>
    public List<int>? ToggleLabel { get; set; }

    /// <summary>The pet's slot in the boarding window, as a box — the slot a successfully-placed pet
    /// lands in, and the crop the "did it actually go in?" check reads. NOT to be confused with
    /// <see cref="PetConfig.ReturnSlot"/>, which is where the pet sits in the BAG.</summary>
    public List<int>? BoardingPetSlot { get; set; }

    /// <summary>The FOOD COUNT boxes in this row, as boxes — one per food slot, each framing the
    /// number the game draws on it. Normally two, and NOT <see cref="Stacks"/> wide: these are the
    /// counts a row displays, which is two whether the row holds two stacks or five.
    ///
    /// Boxed TIGHTLY around the digits rather than the slot: the number sits at the slot's
    /// bottom-right and spills past its frame, so a box that frames the slot clips it.
    ///
    /// Nothing reads them yet — the reload still schedules by arithmetic. They are the reading that
    /// would replace it (PLAN-PET-AUTOFEED.md §2), which is why they are calibrated and unread.</summary>
    public List<List<int>> FeederSlots { get; set; } = new();

    /// <summary>How many food stacks this row holds — 2 on the free row, 5 on a paid one. Different
    /// per row means different reload intervals, so the schedule is per row too.</summary>
    public int Stacks { get; set; } = 2;

    /// <summary>Whether boarding is running on THIS row right now — i.e. the pet is in the loader
    /// rather than in the bag.
    ///
    /// The reload needs it because the 開始代養 / 結束代養 control is ONE button per row: pressing it
    /// while boarding runs ENDS it, and while stopped STARTS it. So the tool cannot press it blindly —
    /// it would do the opposite of what the step needs.
    ///
    /// Read from the row's own boarding slot once a reference crop exists; until then it is stated by
    /// the player at the start and set by the tool afterwards, because a successful reload always
    /// leaves boarding running.</summary>
    public bool BoardingRunning { get; set; }
}

/// <summary>One pet to breed, identified by its ICON rather than by a bag position.
///
/// The position is the thing that cannot be trusted: a pet returned by the boarding window lands in
/// the FIRST FREE SLOT, and the character farms throughout, so the first free slot is not the slot the
/// pet came from and not the slot that was marked. The icon is the pet's own portrait, so matching it
/// across the 64 cells finds the pet wherever it went — which is what turns a queue of positions into
/// a queue of pets.
///
/// <see cref="Png"/> is a crop stored as base64 in local.yaml, like the empty-slot reference and for
/// the same reason: a second file is a second thing to lose. <see cref="Rect"/> is kept so the box can
/// be redrawn and re-cropped rather than being a one-shot.</summary>
public sealed class PetQueueEntry
{
    /// <summary>For the log and the UI only. NOTHING matches on it — the recogniser returned
    /// 真蔚蓝尽凰 for 真蔚藍鳳凰 on a live read (2026-09-19), and the name never needs to be read at all
    /// because the crop is the identity.</summary>
    public string? Label { get; set; }

    /// <summary>Where the icon was dragged on the calibration capture.</summary>
    public List<int>? Rect { get; set; }

    /// <summary>The icon itself, cropped at <see cref="Rect"/>.</summary>
    public string? Png { get; set; }
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
/// entirely, about the manual feeding system).
///
/// What is SHARED and what is PER ROW is the split that matters. The way in (目錄, the feed icon, the
/// X), the bag and its grid, the food pool, and the queue are shared; the toggle, the boarding slot
/// and the stack count belong to a row and live in <see cref="Slots"/>.</summary>
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

    /// <summary>The breeding rows, top to bottom — ONE ENTRY PER ROW. Index 0 is the free row.
    ///
    /// A list rather than four fields because the rows are the same thing repeated: the capture shows
    /// four identical layouts, each with its own button, food boxes and pet slot. Adding a row is an
    /// entry, not a mechanism.
    ///
    /// The tool drives as many as are configured, one at a time — the player's own ordering constraint
    /// (2026-09-19): each row is offloaded and re-boarded before the next is touched.</summary>
    public List<PetSlotConfig> Slots { get; set; } = new();

    /// <summary>The pet slot as it looks EMPTY, cropped from a capture and stored as a base64 PNG.
    ///
    /// The reference for "did the pet actually go in?". A right-click can fail to register — measured
    /// on a live 12-hour run, where roughly half the boarded time was lost to reloads that loaded food
    /// into an empty slot and reported success — and the placement itself is verified, so the click
    /// was on target. Which means the only way to know is to look at the result.
    ///
    /// Capture it with the breeder empty, before a pet goes in.</summary>
    public string? PetSlotEmptyPng { get; set; }

    /// <summary>How unlike the empty reference a slot must look before it counts as occupied, as a
    /// fraction of differing pixels. Deliberately low: the composer measured an empty box against
    /// itself at 0.000 and against a gem at ~0.3, so anything above a few percent is content. The cost
    /// of being wrong in the strict direction is a retry; in the loose direction it is not noticing a
    /// pet that never went in, which is what this exists to catch.</summary>
    public double PetSlotOccupiedAbove { get; set; } = 0.05;

    /// <summary>The two FOOD COUNTS in the boarding window, as boxes.
    ///
    /// Each is the number the game draws on a food slot — how many items are left in that stack,
    /// which is the reading the reload decision is meant to hang on rather than the 185-minute
    /// arithmetic it uses today.
    ///
    /// Boxed TIGHTLY around the digits, not around the slot: the number sits at the slot's
    /// bottom-right and spills past its frame, so a box that frames the slot clips it. And not so
    /// wide that it reaches the next slot, because then the two numbers read as one string.
    ///
    /// Named FeederSlotA/B for the slot they belong to; an earlier revision had them framing the
    /// slots themselves as empty-check crops, and the name outlived the purpose — the boxes were
    /// re-dragged onto the numbers because a clipped read was worth less than a slot check that was
    /// never built.</summary>
    public List<int>? FeederSlotA { get; set; }
    public List<int>? FeederSlotB { get; set; }

    // ── The bag, as THIS flow shows it ──────────────────────────────────────

    /// <summary>Whole bag grid region client-relative physical [x, y, w, h], for the bag the boarding
    /// window opens. NOT interchangeable with <see cref="BuySellConfig.BagGrid"/>: the bag sits
    /// somewhere else here, and sharing the numbers would aim every cell at the wrong item.</summary>
    public List<int>? BagGrid { get; set; }

    /// <summary>One bag slot, the uniformity check on <see cref="BagGrid"/>.</summary>
    public List<int>? BagSlot { get; set; }

    /// <summary>The bag cells holding pet food, each as [page, cell] with page 0-based.
    ///
    /// ONE POOL FOR ALL ROWS: every row's reload draws its stacks from this list in order, which is
    /// right while the pets share a food type (the player's stated case — "assuming the pets are the
    /// same stage"). With four rows the pool is drawn down far faster: 2+5+5+5 = 17 cells per full
    /// round, against 2 before.
    ///
    /// Consumed in the order they are listed, tracked by <see cref="FoodSlotsUsed"/>. NOT
    /// highest-index-first, which is what SellPass does and what an earlier revision of this copied —
    /// that rule exists because a sold slot leaves a hole the bag may compact into, and **the food
    /// here is locked**, so nothing shifts and the order buys nothing. What does matter is not
    /// re-clicking a cell this run has already emptied, which is what the count is for.</summary>
    public List<List<int>> FoodSlots { get; set; } = new();

    /// <summary>How many of <see cref="FoodSlots"/> have been used up. Persisted, because a restart
    /// that reset it would silently right-click cells the previous run had already emptied — and the
    /// only thing that notices is a pet that stops being fed.</summary>
    public int FoodSlotsUsed { get; set; }

    /// <summary>The bag cell a returned pet lands in, as [page, cell] — the ROW-INDEPENDENT landing
    /// place, because the game puts it in the first free slot rather than where it came from.
    ///
    /// The player's own name for it, and the reason the bag is kept with one slot free: a returned pet
    /// lands there, so the mark stays true. It is the SIMPLE path and it is what the tool uses today.
    /// It is honest about its own limitation — when farming, loot takes the first free slot and the pet
    /// does not necessarily land here — which is what <see cref="Queue"/> replaces it with.</summary>
    public List<int>? ReturnSlot { get; set; }

    /// <summary>The pets to breed, identified by ICON. The tool scans the bag for the best match and
    /// boards the first one it finds — see <see cref="PetQueueEntry"/> for why position cannot be
    /// trusted, and PLAN-PET-AUTOFEED.md §13 for why any match will do rather than a particular one:
    /// an idle breeder is wasted time, so a substitute of the right kind is harmless.
    ///
    /// Empty means the tool falls back to <see cref="ReturnSlot"/> and the fixed-cell behaviour.</summary>
    public List<PetQueueEntry> Queue { get; set; } = new();

    /// <summary>Optional override for the count dialog's MAX.
    ///
    /// Normally null and <see cref="BuySellConfig.MaxButton"/> is used — the boarding count dialog is
    /// the same dialog, so a second mark for one button would be a second thing to drift. It exists
    /// because that sameness is the mechanism, which the player confirmed, and NOT the position,
    /// which they could not: the dialog may sit elsewhere in this flow. If a demo run shows it does,
    /// this is the mark to fill rather than a rebuild.</summary>
    public List<int>? MaxButton { get; set; }

    // ── Behaviour ───────────────────────────────────────────────────────────

    /// <summary>How many minutes to wait AFTER the feeder should be empty before reloading.
    ///
    /// The cycle is `load_minutes + this`. On the free row the default is 2 stacks = 600 items at
    /// 3/min = 200 minutes, so the default of 5 reloads every 205; a paid row's 5 stacks make it 505.
    ///
    /// **Positive, and that is the point** (player, 2026-09-19): reloading *after* the feeder empties
    /// guarantees the feeder IS empty when the stacks go in. Reloading early leaves food in a slot
    /// that takes several stacks, and what the game does with a top-up onto a partial is unknown —
    /// refuses, swaps, or swallows it. A five-minute gap with nothing fed is the price of never
    /// finding out, and it is 1% of the cycle.
    ///
    /// Negative means reload early, which discards food: at 3/min, arriving 10 minutes early sets
    /// aside 30 items every cycle. Available because the trade may change once we know what a
    /// partially-full feeder does.</summary>
    public int WaitAfterEmptyMinutes { get; set; } = 5;

    /// <summary>How long to wait after EACH action on the pet before the next one — ending boarding,
    /// placing the pet, a stack, the start.
    ///
    /// Separate from the tool's own click delays because it is the player's observation (2026-09-19)
    /// that the game needs a buffer after each pet-changing action, not just after a click. Every step
    /// here changes the breeder's state, and the next click is aimed at a window that is still
    /// absorbing the last one. A field rather than a constant because it is tuned by watching, like
    /// the hover delay.
    ///
    /// The cost of too long is a slower reload; the cost of too short is a click that does not
    /// register, which is the failure this tool cannot see without the pet-slot check.</summary>
    public int ActionWaitMs { get; set; } = 1200;

    /// <summary>The game's per-stack cap, and a game constant rather than a setting.</summary>
    public const int StackSize = 300;

    /// <summary>Items the game consumes per minute while boarding runs. Stage 6 is 3; the boarding
    /// window states this itself (`每1分 攝取3個`), so it is a measured game constant rather than a
    /// guess — and each row states its own, so a stage-7 row saying 4 is readable rather than a number
    /// this config is simply wrong about.</summary>
    public int ItemsPerMinute { get; set; } = 3;

    /// <summary>How many items one reload puts into the given row, DERIVED from that row's stack count
    /// so the two can never disagree. It was a settable tool-wide 600 before, which is how "two
    /// stacks" became invisible — and it is per row now because the free row holds two and a paid row
    /// five, so the two empty at different times and need different timers.</summary>
    public static int LoadItemsFor(PetSlotConfig slot) => Math.Max(1, slot.Stacks) * StackSize;

    /// <summary>Minutes that row's load lasts, derived rather than stored.</summary>
    public int LoadMinutesFor(PetSlotConfig slot) =>
        ItemsPerMinute > 0 ? LoadItemsFor(slot) / ItemsPerMinute : 0;

    /// <summary>Minutes between reloads for that row: the load, plus the wait past empty.</summary>
    public int CycleMinutesFor(PetSlotConfig slot) =>
        LoadMinutesFor(slot) + WaitAfterEmptyMinutes;
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

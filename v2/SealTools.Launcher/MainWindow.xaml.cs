using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.IO.Ports;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using Rectangle = System.Windows.Shapes.Rectangle;
using Ellipse = System.Windows.Shapes.Ellipse;
using SealTools.Core;
using SealTools.Core.Config;
using SealTools.Tuner;
using FluentWindow = Wpf.Ui.Controls.FluentWindow;
using ControlAppearance = Wpf.Ui.Controls.ControlAppearance;
using UiButton = Wpf.Ui.Controls.Button;
using CardControl = Wpf.Ui.Controls.CardControl;
using Card = Wpf.Ui.Controls.Card;
using InfoBar = Wpf.Ui.Controls.InfoBar;
using InfoBarSeverity = Wpf.Ui.Controls.InfoBarSeverity;
using Mat = OpenCvSharp.Mat;

namespace SealTools.Launcher;

/// <summary>
/// Main window: tool cards (start/stop + live status) and a tabbed config editor,
/// all driven by <see cref="LauncherService"/>.
/// </summary>
public partial class MainWindow : FluentWindow, IDisposable
{
    private static readonly (string Id, string Name)[] Tools =
    {
        ("tuner", "Magic Tuner"),
        ("gem", "Gem Composer"),
        ("spammer", "Skill Spammer"),
        ("buy", "Buy Items"),
        ("sell", "Sell Items"),
        ("pet", "Pet Feeder"),
    };

    private static readonly string[] Grades = { "N", "G", "DG", "XG", "SG" };
    private static readonly string[] MatchModes = { "any", "all", "per_attr" };
    private static readonly string[] GemGrades = { "N", "G", "DG" };
    // What the gem composer does when the result window is empty (a grade ran out of resources).
    // Label is shown in the config tab; Value is the empty_mode value written to config.
    private static readonly (string Label, string Value)[] GemEmptyModes =
    {
        ("Stop", "stop"),
        ("Advance to next grade", "advance_grade"),
        ("Clear resources, then advance", "advance_grade_clear"),
    };
    private static readonly string[] RequireGradeOptions = { "None", "N", "G", "DG", "XG", "SG" };

    private readonly LauncherService _service;
    // id -> its card Border, so mini mode can show only the running tool's card.
    private readonly Dictionary<string, Border> _toolCards = new();
    private readonly Dictionary<string, TextBlock> _statusBlocks = new();
    private readonly DispatcherTimer _timer;
    // Debounces the placement save while the window is being dragged or resized.
    private DispatcherTimer? _uiSaveTimer;

    private static readonly string[] CalibGemSteps = { "N", "G", "DG", "Register", "Combine" };
    // The two composer move sets (Core/GemRoutes.cs): tuned counts vs closed-loop point placement.
    private static readonly string[] GemMoveModes = { "tuned", "arduino" };
    // The tuner's cursor placement + guard modes (Core/SealTuner.cs).
    private static readonly string[] TunerSpringModes = { "manual", "hid" };
    private static readonly string[] TunerGuardModes = { "off", "stop", "recenter" };
    private static readonly string[] CalibResourceSteps = { "Resource1", "Resource2", "Resource3" };
    // Points offered by the gem "Test Click" move-cursor check.
    private static readonly string[] GemTestPoints =
        CalibGemSteps.Concat(CalibResourceSteps).Concat(new[] { "ResultCenter" }).ToArray();
    // Click steps come first (grade buttons + resource slots), then one drag step for the
    // result-gem area. The result gem box is an AREA (not a point) because it's both OCR-read
    // and clicked (centre).
    private static readonly int GemResultBoxStep = CalibGemSteps.Length + CalibResourceSteps.Length;

    // Composer move routes shown as editable dx/dy rows in the Calibrate Gem tab. Each maps to a
    // raw `movements` value the composer sends for that step. Key = config field + grade for the
    // radio->register moves; From/To drive the test button; Dx/Dy are the user's fine-tuned boxes.
    private sealed class GemMoveRow
    {
        public string Key = "";
        public string From = "";
        public string To = "";
        public TextBox Dx = null!;
        public TextBox Dy = null!;
    }
    private readonly List<GemMoveRow> _gemMoveRows = new();

    // Editable coordinate rows in the Calibrate Gem tab (N/G/DG/Register/Combine + Resource1-3 +
    // result area). Key is the config key; X/Y (and W/H for the result area) are editable boxes.
    private sealed class GemCoordRow
    {
        public string Key = "";
        public TextBox X = null!;
        public TextBox Y = null!;
        public TextBox? W;
        public TextBox? H;
    }
    private readonly List<GemCoordRow> _gemCoordRows = new();
    private static readonly int GemTotalSteps = GemResultBoxStep + 1;
    private bool _disposed;

    // Tuner calibrator (drag three boxes: grade / attributes / remaining, then click the spring) state.
    private Image? _tunerImage;
    private Canvas? _tunerCanvas;
    private TextBlock? _tunerHint;
    private BitmapSource? _tunerScreenshot;
    private int _tunerStep;
    private Rect? _tunerGradeBox;
    private Rect? _tunerAttrBox;
    private Rect? _tunerRemainingBox;
    // 發條 button [x, y] (client-relative physical), set by the 4th calibrate step. Only used in
    // spring_mode: hid.
    private Point? _tunerSpringPoint;
    // Measured attribute line pitch from the last "Check OCR", used by BuildOcrGeometry
    // to persist an accurate row_height instead of the loose attr.Height/3 guess.
    private int? _tunerAttrPitch;
    // Display measurement from the last capture of each tab (scale + physical client rect), saved
    // with the calibration so another machine can convert (docs/COORDINATES.md).
    private DisplayInfo? _tunerDisplay;
    private DisplayInfo? _gemDisplay;
    private Point? _tunerDragStart;
    private Rectangle? _tunerMarquee;

    // Gem calibrator (click buttons + drag the result-gem area) state.
    private Image? _gemImage;
    private Canvas? _gemCanvas;
    private TextBlock? _gemHint;
    private BitmapSource? _gemScreenshot;
    private Grid? _gemGrid;
    private readonly Dictionary<string, Point> _gemPoints = new();
    private Rect? _gemResultBox;
    private Point? _gemDragStart;
    private Rectangle? _gemMarquee;
    private int _gemStep;
    private System.Windows.Controls.ComboBox? _gemTestPoint;
    private System.Windows.Controls.ComboBox? _gemMoveMode;
    private bool _cycleRunning;
    private System.Windows.Controls.Image? _gemResultPreview;

    // Buy/Sell calibrator state.
    private TextBlock? _buySellHint;
    private Image? _bsImage;
    private Canvas? _bsCanvas;
    private BitmapSource? _bsScreenshot;
    private DisplayInfo? _bsDisplay;
    /// <summary>Dragging a box on the capture: which of the two rectangles the next drag fills.</summary>
    private string? _bsDragTarget;
    private Point? _bsDragStart;
    private Rectangle? _bsMarquee;
    /// <summary>Which of the single points the calibrator is waiting for, if any: "first", "second",
    /// "scroll" or "max". Set by the matching button, cleared by the click that fills it.</summary>
    private string? _bsPointTarget;
    /// <summary>The "what is set so far" list on the Buy/Sell calibrate tab.</summary>
    private TextBlock? _bsChecklist;

    // Hover-panel calibrator state.
    private TextBlock? _tooltipHint;
    private Image? _tooltipImage;
    private Canvas? _tooltipCanvas;
    private BitmapSource? _tooltipScreenshot;
    /// <summary>The point the tool will park the cursor on so a panel comes up — marked on the capture,
    /// then used by the hover button. Deliberately not persisted: it is a working position for this
    /// calibration, not a fact about the machine.</summary>
    private List<int>? _tooltipHoverPoint;
    private bool _tooltipArmingPoint;
    private bool _tooltipArmed;
    private Point? _tooltipDragStart;
    private Rectangle? _tooltipMarquee;

    // Pet calibrator state. Deliberately its own capture and canvas rather than sharing the buy/sell
    // one: the two are marked against different screens (the shop vs the boarding window), and a
    // shared capture would let a mark be taken against the wrong one without anything saying so.
    private TextBlock? _petHint;
    private Image? _petImage;
    private Canvas? _petCanvas;
    private BitmapSource? _petScreenshot;
    private string? _petDragTarget;
    private Point? _petDragStart;
    private Rectangle? _petMarquee;
    /// <summary>Which single point the calibrator is waiting for — see <see cref="PetArmPoint"/>.</summary>
    private string? _petPointTarget;
    private TextBlock? _petChecklist;
    /// <summary>Which bag page the cell picker edits, and whether a click marks food or the pet.</summary>
    private int _petPickPage;

    /// <summary>What a click on the bag marks. Three now, and the third is the point: the RETURN slot
    /// is where a pet comes back to — one cell you keep EMPTY — while the pet you want to queue is
    /// somewhere else entirely, in a slot that holds a pet. Cropping a queue icon from the return slot
    /// could only ever capture an empty cell, which is what made the two look like one thing.</summary>
    private enum PetPick { Food, ReturnSlot, QueueSource }
    private PetPick _petPick = PetPick.Food;

    /// <summary>The bag cell holding a pet whose icon is about to be captured. Transient — it is a
    /// source for one crop, not a saved mark, because where the pet sits now is not where it will be
    /// found later. The whole point of the icon is that it does not need the position.</summary>
    private List<int>? _petQueueSource;
    private Dictionary<int, Border> _petCellBoxes = new();
    /// <summary>The page buttons, and the two mode buttons — kept so the active one can be shown as
    /// active. Without that the picker silently edits a page nobody can see it is editing, and marks
    /// appear on a page the user is not looking at.</summary>
    private readonly List<UiButton> _petPageButtons = new();
    private UiButton? _petFoodModeButton;
    private UiButton? _petPetModeButton;
    private UiButton? _petQueueModeButton;
    private TextBlock? _petCellInfo;
    /// <summary>The "what is missing before Start" line on the Pet tab.</summary>
    private TextBlock? _petReadyText;
    /// <summary>Which breeding row the Calibrate Pet tab is marking. See <see cref="EditRow"/>.</summary>
    private int _petEditRow;

    /// <summary>Rebuilt whenever a row is added, so the strip always shows exactly the rows that exist.</summary>
    private StackPanel? _petRowStrip;

    /// <summary>This row's food-slot count — 2 on the free row, 5 on a paid one.</summary>
    private Wpf.Ui.Controls.TextBox? _petStacksBox;

    /// <summary>One boarding tick per row, on the Pet tab — the tool's one piece of state it cannot
    /// read for itself, and a per-run fact rather than a calibration.</summary>
    private StackPanel? _petBoardingTicks;

    /// <summary>The Pet tab's own hint block. Distinct from <see cref="_petHint"/>, which belongs to
    /// Calibrate Pet — reporting a Pet-tab save into the other tab's message box says nothing useful.</summary>
    private TextBlock? _petTabHint;

    /// <summary>The Pet tab's checklist — its own features only, like Calibrate Pet's.</summary>
    private TextBlock? _petSessionChecklist;

    /// <summary>Suppresses the tick handlers while the panel is being rebuilt, or repopulating it
    /// would write every row's value straight back.</summary>
    private bool _syncingPetRow;

    /// <summary>The queue on the Pet tab: the label field, the preview of the last crop, and the list.</summary>
    private Wpf.Ui.Controls.TextBox? _petQueueLabel;
    private Image? _petQueuePreview;
    private TextBlock? _petQueueList;

    // Buy tab / Sell tab state.
    private System.Windows.Controls.ComboBox? _buyPreset;
    /// <summary>The row picker on the Buy tab, kept as a field because RefreshBuyPresets fills it —
    /// including when no preset is selected, which is the state a brand-new install starts in and
    /// cannot create its first item without.</summary>
    private ComboBox? _buyRowBox;
    /// <summary>The same preset picker on the Buy card, so a run can be set up without opening
    /// Configuration. Both mirrors of one choice — see OnBuyPresetChanged.</summary>
    private ComboBox? _buyPresetCard;
    private Wpf.Ui.Controls.TextBox? _buyCountCard;
    /// <summary>The item currently chosen on either picker. One value behind two controls.</summary>
    private string? _activeBuyPreset;
    /// <summary>True while the two pickers are being synchronised, so their events don't recurse.</summary>
    private bool _syncingBuy;
    private TextBlock? _buyHint;
    private readonly Dictionary<int, Border> _sellSlotBoxes = new();
    private TextBlock? _sellHint;

    public MainWindow()
    {
        InitializeComponent();

        _service = new LauncherService(FindRootDir());
        BuildToolCards();
        BuildConfigTabs();
        LoadCalibrationImages();

        // Window layout: placement and "on top" are remembered (local.yaml), and the config region
        // starts collapsed so the window opens as just the tool cards.
        RestoreUiState();
        HoldSpaceToggle.Click += async (_, _) =>
        {
            if (_service.CurrentId == "holdspace" && _service.CurrentState?.Running == true)
            {
                _service.ReleaseSpace();
                _ = _service.StopTool();
            }
            else if (!await _service.StartToolAsync("holdspace"))
                MessageBox.Show(_service.LastArduinoError ?? "Arduino not found.", "Cannot start", MessageBoxButton.OK, MessageBoxImage.Warning);
        };
        ConfigToggle.Click += (_, _) => SetConfigExpanded(!_configExpanded);
        ToolsToggle.Click += (_, _) => SetToolsCollapsed(!_toolsCollapsed);
        PinToggle.Click += (_, _) => SetPinned(!Topmost);
        SetConfigExpanded(false);

        // Commit a move or a resize shortly after the user stops dragging. Without this the
        // placement is only written on a graceful close, so a session that ends any other way
        // (killed process, crash) silently loses it — which is exactly how a resize went missing.
        _uiSaveTimer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(700) };
        _uiSaveTimer.Tick += (_, _) => { _uiSaveTimer.Stop(); SaveUiState(); };
        LocationChanged += (_, _) => QueueUiStateSave();
        SizeChanged += (_, _) => QueueUiStateSave();

        _timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(750) };
        _timer.Tick += (_, _) => RefreshStatus();
        _timer.Start();
        RefreshStatus();

        Closed += (_, _) => Dispose();
    }

    /// <inheritdoc />
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _timer.Stop();
        _uiSaveTimer?.Stop();
        // Before the service goes: remember where the window was so the next launch opens there.
        SaveUiState();
        _service.Dispose();
        GC.SuppressFinalize(this);
    }

    // ── Tool cards ──────────────────────────────────────────────────────────

    private void BuildToolCards()
    {
        foreach (var (id, name) in Tools)
        {
            var nameText = new TextBlock
            {
                Text = name,
                Foreground = Res("TextFillColorPrimaryBrush"),
                FontSize = 18,
                FontWeight = FontWeights.SemiBold,
            };

            var statusText = new TextBlock
            {
                Text = "stopped",
                Foreground = Res("SystemFillColorCriticalBrush"),
                FontSize = 13,
                TextWrapping = TextWrapping.Wrap,
                Margin = new Thickness(0, 6, 0, 0),
            };
            _statusBlocks[id] = statusText;

            var startButton = MakeButton("Start", ControlAppearance.Primary);
            startButton.Click += async (_, _) =>
            {
                // Which item to buy is chosen on the Buy tab, but a tool start only carries an id —
                // so the choice is handed over here, just before the run that will read it.
                if (id == "buy")
                {
                    _service.PendingBuyPreset = _activeBuyPreset;
                    _service.PendingBuyCount = BuyRunCountFromCard();
                }

                if (!await _service.StartToolAsync(id))
                {
                    // Without this the click just does nothing: the tools' "Arduino not found"
                    // message goes to a console the published WinExe doesn't have.
                    MessageBox.Show(
                        _service.LastArduinoError ?? "Arduino not found.",
                        "Cannot start", MessageBoxButton.OK, MessageBoxImage.Warning);
                }
            };

            var stopButton = MakeButton("Stop", ControlAppearance.Danger);
            stopButton.Click += (_, _) => _service.StopTool();

            var buttons = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                // Right-aligned explicitly. A vertical StackPanel stretches its children to the
                // column's width, and the column is as wide as its widest row — so on the Buy card,
                // where the preset and count sit above, Start/Stop were stretched to that width and
                // then laid out from ITS left edge, pulling them away from every other card's buttons.
                HorizontalAlignment = HorizontalAlignment.Right,
            };
            buttons.Children.Add(startButton);
            buttons.Children.Add(stopButton);

            var left = new StackPanel();
            left.Children.Add(nameText);
            left.Children.Add(statusText);

            // Everything the Buy tool needs to be started without opening Configuration: which item,
            // and how many. The preset picker appears on the tab as well — it is the same choice, so
            // the two are kept in step rather than being one-looking control with two states.
            var right = new StackPanel
            {
                Orientation = Orientation.Vertical,
                HorizontalAlignment = HorizontalAlignment.Right,
            };
            if (id == "buy")
            {
                // One height for every control in this row, and gaps the same either side of the
                // box. Measured before this: the combo and the "-" were TOUCHING (no gap at all),
                // the other gaps were 4, and the three elements had three different heights — 34,
                // 32 and 25. Nothing lined up because nothing was actually set.
                const double RowHeight = 32;      // matches the Start/Stop buttons beside it
                const double CountWidth = 44;
                const double PresetWidth = 150;   // fixed, so a long preset name does not clip
                const double GapRow = 8;          // preset -> stepper
                const double GapInner = 4;        // stepper -> box -> stepper
                const double InlineFontSize = 11;  // the control's default is too large for this row height:
                                       // descenders (the "g" in Springs) were clipped by the box

                _buyPresetCard = new ComboBox
                {
                    Width = PresetWidth,
                    Height = RowHeight,
                    // Smaller than the control's default: at the inherited size the text did not fit
                    // the row height and its descenders were clipped by the box's lower edge. Height
                    // cannot grow to suit it, because the row is tied to the Start/Stop buttons
                    // beside it — so the text shrinks instead.
                    FontSize = InlineFontSize,
                    VerticalAlignment = VerticalAlignment.Center,
                    Margin = new Thickness(0, 0, GapRow, 0),
                };
                _buyPresetCard.SelectionChanged += (_, _) => OnBuyPresetChanged(fromCard: true);

                _buyCountCard = UiText("1");
                _buyCountCard.FontSize = InlineFontSize;
                _buyCountCard.Width = CountWidth;
                _buyCountCard.Height = RowHeight;
                _buyCountCard.Margin = new Thickness(GapInner, 0, GapInner, 0);
                _buyCountCard.VerticalAlignment = VerticalAlignment.Center;
                _buyCountCard.HorizontalContentAlignment = HorizontalAlignment.Center;

                // NOT MakeButton: that one is sized for "Start"/"Save" and sets MinWidth 84, so "−"
                // and "+" came out 84 pixels wide each, flanking a 48px box. Its 10px top margin also
                // sat them lower than the box they flank.
                var minus = MakeStepperButton("−");
                minus.Click += (_, _) => BumpBuyCount(-1);
                var plus = MakeStepperButton("+");
                plus.Click += (_, _) => BumpBuyCount(+1);

                var countRow = new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    VerticalAlignment = VerticalAlignment.Center,
                    // MakeButton carries a 6px right margin, so a row of them ends 6px short of the
                    // column edge. Without the same inset here the "+" overhangs "Stop" by exactly
                    // that — which is what made the two rows look out of line even though the
                    // buttons below matched every other card to the pixel.
                    Margin = new Thickness(0, 0, 6, 0),
                };
                countRow.Children.Add(minus);
                countRow.Children.Add(_buyCountCard);
                countRow.Children.Add(plus);

                // Item on the left, count on the right, Start/Stop below both — one row of controls
                // rather than three ragged ones, so the edges line up down the card.
                var runnerGrid = new Grid();
                runnerGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
                runnerGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
                Grid.SetColumn(_buyPresetCard, 0);
                Grid.SetColumn(countRow, 1);
                runnerGrid.Children.Add(_buyPresetCard);
                runnerGrid.Children.Add(countRow);

                right.Children.Add(runnerGrid);
            }
            right.Children.Add(buttons);

            var cardGrid = new Grid();
            cardGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            cardGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            Grid.SetColumn(left, 0);
            Grid.SetColumn(right, 1);
            cardGrid.Children.Add(left);
            cardGrid.Children.Add(right);

            var card = new Border
            {
                Background = Res("CardBackgroundFillColorDefaultBrush"),
                CornerRadius = new CornerRadius(12),
                BorderBrush = new SolidColorBrush(Color.FromArgb(0x20, 0xFF, 0xFF, 0xFF)),
                BorderThickness = new Thickness(1),
                Padding = new Thickness(20),
                Margin = new Thickness(0, 0, 0, 12),
                Child = cardGrid,
            };

            ToolsPanel.Children.Add(card);
            _toolCards[id] = card;
        }
    }

    private void RefreshStatus()
    {
        var state = _service.CurrentState;

        // Mini mode: while a tool runs, the window shows only that tool's card so it takes a corner
        // rather than the whole left edge. Only one tool can run at a time, so nothing is hidden
        // that could be used anyway.
        // Hold Space has no card (it's a top-row button), so it doesn't shrink the window to a card.
        _miniToolId = state is { Running: true } && _service.CurrentId != "holdspace"
            ? _service.CurrentId : null;
        ApplyWindowLayout();

        foreach (var (id, _) in Tools)
        {
            if (!_statusBlocks.TryGetValue(id, out var block))
            {
                continue;
            }

            if (_service.CurrentId == id && state != null)
            {
                block.Text = FormatStatus(state);
                block.Foreground = state.Running
                    ? Res("SystemFillColorSuccessBrush")
                    : Res("SystemFillColorCriticalBrush");
            }
            else
            {
                block.Text = "stopped";
                block.Foreground = Res("SystemFillColorCriticalBrush");
            }
        }

        // Hold Space's small card (top-right) flips between Hold Space and Stop Space as it runs.
        bool holding = _service.CurrentId == "holdspace" && state?.Running == true;
        HoldSpaceStatus.Text = holding ? "● holding" : "● idle";
        HoldSpaceStatus.Foreground = holding ? Res("SystemFillColorSuccessBrush") : Res("TextFillColorSecondaryBrush");
        HoldSpaceToggle.Content = holding ? "Stop Space" : "Hold Space";
        HoldSpaceToggle.Appearance = holding ? ControlAppearance.Danger : ControlAppearance.Secondary;
    }

    private static string FormatStatus(ToolState state)
    {
        var lines = new List<string> { state.Running ? "● RUNNING" : "● paused" };
        if (!string.IsNullOrEmpty(state.Grade)) lines.Add($"Grade: {state.Grade}");
        if (state.Remaining.HasValue) lines.Add($"Remaining: {state.Remaining}");
        if (state.Attempt > 0) lines.Add($"Attempt: {state.Attempt}");
        if (state.Cycle > 0) lines.Add($"Cycle: {state.Cycle}");
        if (!string.IsNullOrEmpty(state.Current)) lines.Add($"Current: {state.Current}");
        if (state.Attributes.Count > 0) lines.AddRange(state.Attributes.Select(a => "· " + a));
        if (!string.IsNullOrEmpty(state.FilterStatus)) lines.Add($"Filter: {state.FilterStatus}");
        if (!string.IsNullOrEmpty(state.Message)) lines.Add("⚠ " + state.Message);
        return string.Join(Environment.NewLine, lines);
    }

    // ── Config tabs ─────────────────────────────────────────────────────────

    // Fallback height for the cards-only window, used only before the first layout pass has run.
    // The real value is measured from the cards — see MeasureCardsHeight. This constant was 430 and
    // documented as "three cards": adding Buy and Sell made it five, and because the cards sit in an
    // Auto row with no ScrollViewer they were CLIPPED rather than scrolled. A hand-measured number
    // that silently rots when a tool is added is not worth keeping.
    private const double ConfigCollapsedHeight = 430;

    /// <summary>The firmware's per-command scroll ceiling. Shared with the tool rather than copied:
    /// this and ShopTool.WheelMax were separate constants, and the preset validation had its own
    /// literal 30 that stayed behind when the ceiling moved. One place now.</summary>
    private const int WheelMaxNotches = ShopGeometry.MaxScrollNotches;

    /// <summary>What the cards-only height actually measured last layout pass. The "remember the
    /// user's expanded height" checks compare against this rather than the constant above, so they
    /// still mean "bigger than the collapsed window" now that the collapsed window is taller.</summary>
    private double _collapsedHeight = ConfigCollapsedHeight;

    // Everything in the collapsed window that is NOT the cards: title bar, window margins, and the
    // row holding the config chevron and the Hold Space toggle. Fixed — it does not change with the
    // number of tools, which is why only the card half is measured.
    private const double WindowChrome = 136;

    // Mini mode — the window shrinking to the running tool's card alone — used to have its own
    // hand-measured constant here. It does not any more: mini mode sizes from the same measurement
    // as the normal view, because the cards are not all the same height (a long status line wraps)
    // and a constant was wrong for whichever tool was not the one it was measured on.

    // What to restore when the region is expanded again. Zero until something shrinks the window, so
    // a fresh launch expands to the window's designed height.
    private double _configExpandedHeight;

    // Layout state. The config collapse, the card collapse and the mini mode all want to set Height,
    // so one function owns it — otherwise they fight, each undoing the other's resize.
    private bool _configExpanded;
    /// <summary>Tool cards hidden so the tabs get the height. Deliberately NOT persisted: it exists
    // for one task — dragging a capture canvas on a screen too small to show the cards and the canvas
    // at once — and a launcher that opened with its status cards missing would read as broken.</summary>
    private bool _toolsCollapsed;
    private string? _miniToolId;
    private bool _layoutApplied;
    private bool _appliedExpanded;
    private bool _appliedMini;
    private bool _appliedToolsCollapsed;

    private void SetConfigExpanded(bool expanded)
    {
        _configExpanded = expanded;
        // Collapsing the tabs brings the cards back. Otherwise the window could be left showing
        // neither, which is not a state worth being able to reach — and this is the pair the two
        // toggles are: hiding the cards opens the tabs, closing the tabs restores the cards.
        if (!expanded) _toolsCollapsed = false;
        ApplyWindowLayout();
    }

    /// <summary>Hides the tool cards, giving the tabs the whole window.</summary>
    private void SetToolsCollapsed(bool collapsed)
    {
        _toolsCollapsed = collapsed;
        // Only meaningful alongside the tabs — you hide the cards to use them. Opening Configuration
        // here is also what keeps the window from ever being empty.
        if (collapsed) _configExpanded = true;
        ApplyWindowLayout();
    }

    /// <summary>Floats the window above every other window, so the tool status stays readable while
    /// you play. Remembered, so it survives a restart.</summary>
    private void SetPinned(bool pinned, bool persist = true)
    {
        Topmost = pinned;
        PinToggle.Content = pinned ? "Pinned on top" : "Pin on top";
        PinToggle.Appearance = pinned ? ControlAppearance.Primary : ControlAppearance.Secondary;
        if (persist) SaveUiState();
    }

    // Restore placement and pinning from local.yaml. Geometry is machine-specific, so it lives there
    // rather than in defaults.yaml.
    private void RestoreUiState()
    {
        ConfigLoader.LocalUi? ui = null;
        try
        {
            ui = _service.LoadLocal()?.Ui;
            if (ui?.Left is { } left) Left = left;
            if (ui?.Top is { } top) Top = top;
            if (ui?.Width is { } width) Width = width;
            if (ui?.ExpandedHeight is { } h) _configExpandedHeight = h;
        }
        catch { /* a bad ui block must not stop the launcher opening */ }

        // Always, even with nothing saved: this is what labels the button. Not persisted here, so a
        // first launch doesn't write a local.yaml just for opening.
        SetPinned(ui?.Pinned ?? false, persist: false);
    }

    /// <summary>Ask for the placement to be written shortly — restarted on every move/resize event,
    /// so a drag writes once, at the end of it, rather than continuously.</summary>
    private void QueueUiStateSave()
    {
        _uiSaveTimer?.Stop();
        _uiSaveTimer?.Start();
    }

    /// <summary>Persist placement, expanded height and pinning. Called on a pin change, on close, and
    /// after a move or resize settles; never throws — window state must not be able to break
    /// shutdown.</summary>
    private void SaveUiState()
    {
        try
        {
            if (WindowState != WindowState.Normal) return; // maximized/minimized: keep the last real size
            if (_configExpanded && _miniToolId == null && Height > _collapsedHeight)
                _configExpandedHeight = Height;

            var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
            local.Ui = new ConfigLoader.LocalUi
            {
                Left = double.IsNaN(Left) ? null : Left,
                Top = double.IsNaN(Top) ? null : Top,
                Width = double.IsNaN(Width) ? null : Width,
                ExpandedHeight = _configExpandedHeight > 0 ? _configExpandedHeight : null,
                Pinned = Topmost,
            };
            _service.SaveLocal(local);
        }
        catch { /* never let this break shutdown */ }
    }

    /// <summary>Applies the window's layout for the current state: the config region, whether only
    /// the running tool's card is shown, and the height that follows from those.</summary>
    private void ApplyWindowLayout()
    {
        // Cards-hidden counts as "not mini": mini mode shows ONE card and sizes the window to it, so
        // the two cannot both apply — collapsing the cards would otherwise shrink the window to a
        // height for a card that is not on screen.
        bool mini = _miniToolId != null && !_toolsCollapsed;
        if (_layoutApplied && mini == _appliedMini && _configExpanded == _appliedExpanded
            && _toolsCollapsed == _appliedToolsCollapsed) return;

        // Remember the height the user was working with before anything shrinks it.
        if (_layoutApplied && _appliedExpanded && !_appliedMini && Height > _collapsedHeight)
            _configExpandedHeight = Height;

        ConfigTabs.Visibility = _configExpanded ? Visibility.Visible : Visibility.Collapsed;
        ConfigToggle.Content = (_configExpanded ? "▾  " : "▸  ") + "Configuration";
        ToolsPanel.Visibility = _toolsCollapsed ? Visibility.Collapsed : Visibility.Visible;
        ToolsToggle.Content = (_toolsCollapsed ? "▸  " : "▾  ") + "Tools";

        foreach (var (id, card) in _toolCards)
            card.Visibility = !mini || id == _miniToolId ? Visibility.Visible : Visibility.Collapsed;

        // Measured AFTER the visibility change, so this is the height of the cards actually on
        // screen — all of them normally, one of them in mini mode.
        double collapsed = MeasureCardsHeight();
        _collapsedHeight = collapsed;

        Height = mini ? collapsed
            : _configExpanded ? (_configExpandedHeight > 0 ? _configExpandedHeight : 720)
            : collapsed;

        _layoutApplied = true;
        _appliedExpanded = _configExpanded;
        _appliedMini = mini;
        _appliedToolsCollapsed = _toolsCollapsed;
    }

    /// <summary>Height the window needs for the tool cards that are currently visible, plus the fixed
    /// chrome. Measured with an unbounded height so the answer does not depend on the window's present
    /// size — working out how tall it must BE is the whole point. WPF ignores collapsed children, so
    /// the same call sizes both the normal view (every card) and mini mode (the running tool's only).</summary>
    private double MeasureCardsHeight()
    {
        ToolsPanel.Measure(new Size(Math.Max(320, Width - 40), double.PositiveInfinity));
        var cards = ToolsPanel.DesiredSize.Height;
        return cards <= 0 ? ConfigCollapsedHeight : cards + WindowChrome;
    }

    private void BuildConfigTabs()
    {
        ConfigTabs.Items.Add(BuildTunerTab());
        ConfigTabs.Items.Add(BuildGemTab());
        ConfigTabs.Items.Add(BuildSpammerTab());
        ConfigTabs.Items.Add(BuildBuyTab());
        ConfigTabs.Items.Add(BuildSellTab());
        ConfigTabs.Items.Add(BuildPetTab());
        ConfigTabs.Items.Add(BuildAttributesTab());
        ConfigTabs.Items.Add(BuildTunerCalibrateTab());
        ConfigTabs.Items.Add(BuildGemCalibrateTab());
        ConfigTabs.Items.Add(BuildBuySellCalibrateTab());
        ConfigTabs.Items.Add(BuildPetCalibrateTab());
        ConfigTabs.Items.Add(BuildTooltipCalibrateTab());
        ConfigTabs.Items.Add(BuildArduinoTab());
        ConfigTabs.Items.Add(BuildSetupTab());
        ConfigTabs.Items.Add(BuildHotkeysTab());
    }

    // Arduino connection status: a green/red light, the expected VID/PID, every serial port the
    // OS sees (with the matching one flagged), and a test click. Used to diagnose "Arduino not
    // found" without re-reading the code.
    // One place that names a theme brush. WPF-UI's semantic brushes are used directly (no second
    // palette beside the theme), so the app follows the theme rather than hard-coded hex values.
    // If a theme switch should repaint live later, this becomes SetResourceReference and every
    // call site follows.
    private Brush Res(string key) => (Brush)FindResource(key);

    // The one "grey explanation" paragraph, so hints read the same in every tab.
    private TextBlock Hint(string text) => new()
    {
        Text = text,
        Foreground = Res("TextFillColorSecondaryBrush"),
        TextWrapping = TextWrapping.Wrap,
        Margin = new Thickness(0, 0, 0, 10),
    };

    // A fixed-width line for machine-read values (rects, ports, ids), used by several tabs.
    private TextBlock Mono() => new()
    {
        Foreground = Res("TextFillColorPrimaryBrush"),
        FontFamily = new FontFamily("Consolas"),
        TextWrapping = TextWrapping.Wrap,
        Margin = new Thickness(0, 0, 0, 2),
    };

    // A WPF-UI text box, so every text field gets the same Fluent chrome (placeholder, clear button)
    // instead of the plain WPF one. ComboBox/CheckBox are already restyled by WPF-UI's dictionary.
    private static Wpf.Ui.Controls.TextBox UiText(string text, string? placeholder = null, bool clearButton = true) => new()
    {
        Text = text,
        PlaceholderText = placeholder ?? "",
        // The clear button needs room to sit inside the field; off for the narrow ones (key/delay).
        ClearButtonEnabled = clearButton,
        VerticalContentAlignment = VerticalAlignment.Center,
    };

    // A titled section: a bordered Card with a header, which is how every tab groups its controls.
    // Card rather than CardControl on purpose — CardControl measures its content with unbounded
    // width, so wrapping text runs past the border (see docs/PLAN-UI-CLEANUP.md).
    private static Card Section(string title, params UIElement[] children)
    {
        var body = new StackPanel();
        body.Children.Add(new TextBlock
        {
            Text = title,
            FontWeight = FontWeights.SemiBold,
            Margin = new Thickness(0, 0, 0, 8),
        });
        foreach (var child in children) body.Children.Add(child);
        return new Card { Content = body, Margin = new Thickness(0, 0, 0, 12) };
    }

    private TabItem BuildArduinoTab()
    {
        // No horizontal margin: the cards should line up with the tab strip's left border, not sit
        // 8px inside it. Vertical margin keeps a little breathing room at the top.
        var panel = new StackPanel { Margin = new Thickness(0, 8, 0, 8) };

        panel.Children.Add(Hint("Is the Arduino there, and does its click reach the game?"));

        // ── Connection ──────────────────────────────────────────────────────
        var light = new Ellipse
        {
            Width = 14,
            Height = 14,
            Fill = Res("SystemFillColorCriticalBrush"),
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 8, 0),
        };
        var status = new TextBlock
        {
            Foreground = Res("TextFillColorPrimaryBrush"),
            FontWeight = FontWeights.SemiBold,
            TextWrapping = TextWrapping.Wrap,
        };
        var statusRow = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 0, 0, 6) };
        statusRow.Children.Add(light);
        statusRow.Children.Add(status);

        var detail = new TextBlock
        {
            Foreground = Res("TextFillColorPrimaryBrush"),
            FontFamily = new FontFamily("Consolas"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 0, 0, 4),
        };

        void Refresh()
        {
            var devices = _service.ArduinoDevices();
            var match = devices.FirstOrDefault(d => d.IsMatch);
            if (match != null)
            {
                light.Fill = Res("SystemFillColorSuccessBrush");
                // The friendly name usually already ends in "(COM5)" — don't repeat the port.
                status.Text = match.Name.Contains($"({match.Port})", StringComparison.OrdinalIgnoreCase)
                    ? $"Connected — {match.Name}"
                    : $"Connected — {match.Name} ({match.Port})";
            }
            else
            {
                light.Fill = Res("SystemFillColorCriticalBrush");
                status.Text = "Not found";
            }

            var lines = new List<string>
            {
                $"Expected: VID 0x{_service.Config.Arduino.Vid:X4}  PID {string.Join("/", _service.Config.Arduino.Pid.Select(p => $"0x{p:X4}"))}",
            };
            if (devices.Count == 0) lines.Add("(no serial ports detected)");
            foreach (var d in devices)
                lines.Add($"{(d.IsMatch ? ">>" : "  ")} {d.Port,-8} {d.Name}  [{d.PnpId}]");
            detail.Text = string.Join(Environment.NewLine, lines);
        }

        var refreshBtn = MakeButton("Refresh", ControlAppearance.Secondary);
        refreshBtn.Click += (_, _) => Refresh();
        var refreshRow = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 4, 0, 0) };
        refreshRow.Children.Add(refreshBtn);

        panel.Children.Add(Section("Connection", statusRow, detail, refreshRow));

        // ── Input test ──────────────────────────────────────────────────────

        // Its own result line, so a click test can't overwrite the connection status above.
        var testResult = new TextBlock
        {
            Foreground = Res("TextFillColorPrimaryBrush"),
            VerticalAlignment = VerticalAlignment.Center,
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(10, 0, 0, 0),
        };
        var testBtn = MakeButton("Send a test click", ControlAppearance.Primary);
        testBtn.Click += async (_, _) =>
        {
            var ser = await _service.ArduinoPortAsync();
            if (ser == null)
            {
                testResult.Text = "No Arduino — see Connection above.";
                return;
            }
            try
            {
                HidPointer.Click(ser);
                testResult.Text = $"Sent (clicked via {ser.PortName}).";
            }
            catch (Exception ex)
            {
                testResult.Text = "Click failed: " + ex.Message;
            }
        };

        var testRow = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 2, 0, 0) };
        testRow.Children.Add(testBtn);
        testRow.Children.Add(testResult);

        panel.Children.Add(Section("Input test",
            Hint("Sends one left click through the Arduino, at wherever the cursor already is — it does " +
                 "not move the cursor. Use it to prove the HID path works, or Calibrate Gem → Test to " +
                 "place the cursor as well."),
            testRow));

        Refresh();

        return MakeTab("Arduino", panel);
    }

    // Display environment: shows what the tools measure for this machine and stores the reference
    // a calibration was made against. Detect fills the fields; the user may override the scale and
    // the client size if detection is wrong (e.g. an unusual monitor arrangement).
    private TabItem BuildSetupTab()
    {
        // No horizontal margin: the cards should line up with the tab strip's left border, not sit
        // 8px inside it. Vertical margin keeps a little breathing room at the top.
        var panel = new StackPanel { Margin = new Thickness(0, 8, 0, 8) };

        panel.Children.Add(Hint(
            "Display environment. Coordinates are stored in PHYSICAL pixels relative to the game window's " +
            "client area. Detect reads the current setup; the scale and reference client size stay editable " +
            "if detection is wrong. If another machine's values differ from the stored calibration, " +
            "recalibrate there (see docs/COORDINATES.md)."));

        var monitor = Mono();
        var window = Mono();
        window.Margin = new Thickness(0, 0, 0, 8);
        var stored = new TextBlock { Foreground = Res("TextFillColorSecondaryBrush"), TextWrapping = TextWrapping.Wrap };
        var warning = new TextBlock { Foreground = Res("SystemFillColorCautionBrush"), TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 4, 0, 0) };

        var scaleBox = UiText("", "1.5");
        scaleBox.Width = 80;
        var clientWBox = UiText("", "width");
        clientWBox.Width = 76;
        var clientHBox = UiText("", "height");
        clientHBox.Width = 76;

        var clientRow = new StackPanel { Orientation = Orientation.Horizontal };
        clientRow.Children.Add(clientWBox);
        clientRow.Children.Add(new TextBlock { Text = " × ", VerticalAlignment = VerticalAlignment.Center, Foreground = Res("TextFillColorSecondaryBrush") });
        clientRow.Children.Add(clientHBox);

        DisplayInfo? detected = null;

        void ShowStored()
        {
            var c = _service.Config.Calibration;
            stored.Text = c.DpiScale == null
                ? "Stored calibration: none yet — run Calibrate Tuner / Calibrate Gem."
                : $"Stored calibration: scale {c.DpiScale} · client {string.Join("×", c.ClientSize ?? new List<int>())} · {c.MeasuredAt}";

            warning.Text = detected is { } d && c.ClientSize is { Count: 2 } cs &&
                           (cs[0] != d.PhysicalClient.Width || cs[1] != d.PhysicalClient.Height)
                ? $"⚠ The calibration was measured on a {cs[0]}×{cs[1]} client; yours is " +
                  $"{d.PhysicalClient.Width}×{d.PhysicalClient.Height} — recalibrate."
                : "";
        }

        var detect = MakeButton("Detect", ControlAppearance.Primary);
        detect.Click += (_, _) =>
        {
            var hwnd = WindowFinder.FindByTitle(_service.Config.Window.Title);
            if (hwnd == IntPtr.Zero) { monitor.Text = "Game window not found — open the game first."; return; }
            var info = Dpi.Measure(hwnd);
            if (info == null) { monitor.Text = "Could not measure the display."; return; }

            detected = info;
            monitor.Text = $"Monitor: {info.ScreenWidth}×{info.ScreenHeight} physical, {info.MonitorDpi} dpi " +
                           $"({info.Scale * 100:0.#}%)";
            window.Text = $"Game window: frame {info.PhysicalFrame.Width}×{info.PhysicalFrame.Height}, " +
                          $"client {info.PhysicalClient.Width}×{info.PhysicalClient.Height} @ " +
                          $"({info.PhysicalClient.Left},{info.PhysicalClient.Top})";

            scaleBox.Text = info.Scale.ToString("0.###", CultureInfo.InvariantCulture);
            clientWBox.Text = info.PhysicalClient.Width.ToString(CultureInfo.InvariantCulture);
            clientHBox.Text = info.PhysicalClient.Height.ToString(CultureInfo.InvariantCulture);
            ShowStored();
        };

        var fields = new StackPanel();
        fields.Children.Add(LabeledField("Scale (physical px per logical px)", scaleBox));
        fields.Children.Add(LabeledField("Reference client size (physical)", clientRow));

        var detectRow = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 8, 0, 0) };
        detectRow.Children.Add(detect);

        panel.Children.Add(Section("Display environment",
            Hint("Detect reads the live window. The fields below stay editable in case detection is wrong."),
            monitor, window, fields, detectRow));

        panel.Children.Add(Section("Stored calibration", stored, warning));

        var result = new InfoBar { IsOpen = false, IsClosable = true, Margin = new Thickness(0, 12, 0, 0) };
        var save = MakeButton("Save Setup", ControlAppearance.Primary);
        save.Margin = new Thickness(0, 0, 0, 0);
        save.Click += (_, _) =>
        {
            double scale = double.TryParse(scaleBox.Text.Trim(), NumberStyles.Float, CultureInfo.InvariantCulture, out var s) && s > 0
                ? s : 1.0;
            int w = int.TryParse(clientWBox.Text.Trim(), out var ww) ? ww : 0;
            int h = int.TryParse(clientHBox.Text.Trim(), out var hh) ? hh : 0;

            var cal = _service.Config.Calibration;
            cal.DpiScale = Math.Round(scale, 4);
            cal.MonitorDpi = (uint)Math.Round(96 * scale);
            cal.ClientSize = new List<int> { w, h };
            if (detected is { } d) cal.Screen = new List<int> { d.ScreenWidth, d.ScreenHeight };
            cal.MeasuredAt = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture);

            var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
            local.Calibration = cal;
            SaveReport(() => _service.SaveLocal(local), result,
                $"Written to local.yaml — scale {cal.DpiScale}, client {w}×{h}.");
            ShowStored();
        };

        panel.Children.Add(save);
        panel.Children.Add(result);

        // Prefill from what is already stored, so the tab is informative before Detect is pressed.
        if (_service.Config.Calibration.DpiScale is { } saved)
        {
            scaleBox.Text = saved.ToString("0.###", CultureInfo.InvariantCulture);
            if (_service.Config.Calibration.ClientSize is { Count: 2 } cs)
            {
                clientWBox.Text = cs[0].ToString(CultureInfo.InvariantCulture);
                clientHBox.Text = cs[1].ToString(CultureInfo.InvariantCulture);
            }
        }
        ShowStored();

        return MakeTab("Setup", panel);
    }

    private TabItem BuildTunerTab()
    {
        // No horizontal margin: the cards should line up with the tab strip's left border, not sit
        // 8px inside it. Vertical margin keeps a little breathing room at the top.
        var panel = new StackPanel { Margin = new Thickness(0, 8, 0, 8) };

        panel.Children.Add(Hint(
            "What the tuner rolls for and how it judges each result. Rerolling stops at the target grade, " +
            "or earlier if an override rule matches."));

        var targetGrade = MakeComboBox(Grades, _service.Config.Tuner.TargetGrade);
        var requireGrade = MakeComboBox(RequireGradeOptions, GradeOrNone(_service.Config.Tuner.Filter.RequireGrade));
        var maxRetries = UiText(_service.Config.Tuner.MaxRetries.ToString(CultureInfo.InvariantCulture));

        var goalFields = new StackPanel();
        goalFields.Children.Add(LabeledField("Target grade", targetGrade));
        goalFields.Children.Add(LabeledField("Require grade", requireGrade));
        goalFields.Children.Add(LabeledField("Max retries", maxRetries));
        panel.Children.Add(Section("Goal",
            Hint("The run stops the moment the target grade is reached — that outranks everything below."),
            goalFields));

        var springMode = MakeComboBox(TunerSpringModes, _service.Config.Tuner.SpringMode);
        var mouseGuard = MakeComboBox(TunerGuardModes, _service.Config.Tuner.MouseGuard);
        var springFields = new StackPanel();
        springFields.Children.Add(LabeledField("Spring mode", springMode));
        springFields.Children.Add(LabeledField("Mouse guard", mouseGuard));
        panel.Children.Add(Section("Spring (發條)",
            Hint("manual — you put the mouse on the 發條 button yourself (the tuner only clicks + Enter). " +
                 "hid — the tuner places the cursor on the calibrated spring point at run start. The mouse " +
                 "guard (hid mode only) stops the run when the mouse drifts off the button (stop) or " +
                 "re-centres it and carries on (recenter)."),
            springFields));

        var clickDelay = UiText(_service.Config.Tuner.Timing.ClickEnterDelay.ToString(CultureInfo.InvariantCulture));
        var ocrDelay = UiText(_service.Config.Tuner.Timing.OcrDelay.ToString(CultureInfo.InvariantCulture));

        var timingFields = new StackPanel();
        timingFields.Children.Add(LabeledField("Click delay (s)", clickDelay));
        timingFields.Children.Add(LabeledField("OCR delay (s)", ocrDelay));
        panel.Children.Add(Section("Timing",
            Hint("Wait after clicking before pressing Enter, then after Enter before reading the screen. " +
                 "Too short and the read catches the previous frame."),
            timingFields));

        var filterEnabled = new CheckBox
        {
            IsChecked = _service.Config.Tuner.Filter.Enabled,
            Content = "Filter enabled",
            Foreground = Res("TextFillColorPrimaryBrush"),
        };
        var matchMode = MakeComboBox(MatchModes, _service.Config.Tuner.Filter.MatchMode);
        var ruleRows = new List<RuleRow>();
        var rulesEditor = BuildRulesEditor(_service.Config.Tuner.Filter.Rules, ruleRows, "+ Add Rule");

        var filterFields = new StackPanel();
        filterFields.Children.Add(LabeledField("Match mode", matchMode));
        filterFields.Children.Add(Hint(
            "any — one rule matching is enough. all — every rule must match. per_attr — every rule " +
            "must reach its own Count, counting matching attributes (so Count only matters here). " +
            "Overrides short-circuit all three: a hit stops the run immediately."));
        panel.Children.Add(Section("Filter — rules (the main goal)",
            Hint("Keep rolling until the result matches these rules."),
            filterEnabled, filterFields, rulesEditor));

        var overrideRows = new List<RuleRow>();
        var overrideEditor = BuildRulesEditor(_service.Config.Tuner.Filter.OverrideRules, overrideRows, "+ Add Override");
        panel.Children.Add(Section("Filter — overrides (stop immediately)",
            Hint("Stop the moment a result matches one of these, whatever the rules above say."),
            overrideEditor));

        void SetFilterFieldsEnabled(bool on)
        {
            matchMode.IsEnabled = on;
            requireGrade.IsEnabled = on;
            rulesEditor.IsEnabled = on;
            overrideEditor.IsEnabled = on;
        }
        filterEnabled.Checked += (_, _) => SetFilterFieldsEnabled(true);
        filterEnabled.Unchecked += (_, _) => SetFilterFieldsEnabled(false);
        SetFilterFieldsEnabled(filterEnabled.IsChecked ?? false);

        var saveCaptures = new CheckBox
        {
            IsChecked = _service.Config.Tuner.SaveCaptures,
            Content = "Save OCR captures",
            Foreground = Res("TextFillColorPrimaryBrush"),
        };
        var cleanupNote = new TextBlock
        {
            Foreground = Res("TextFillColorSecondaryBrush"),
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(10, 0, 0, 0),
        };
        var cleanup = MakeButton("Clean up capture images", ControlAppearance.Secondary);
        cleanup.Click += (_, _) => cleanupNote.Text = $"Deleted {_service.CleanupCaptures()} capture image(s).";
        var cleanupRow = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 2, 0, 0) };
        cleanupRow.Children.Add(cleanup);
        cleanupRow.Children.Add(cleanupNote);
        panel.Children.Add(Section("Advanced",
            Hint("Writes every OCR frame to logs/captures as it is read — useful when a read looks wrong, " +
                 "at the cost of disk writes on every attempt."),
            saveCaptures, cleanupRow));

        var result = new InfoBar { IsOpen = false, IsClosable = true };
        var save = MakeButton("Save Tuner Config", ControlAppearance.Primary);
        save.Click += (_, _) =>
        {
            var cfg = _service.Config;
            cfg.Tuner.TargetGrade = targetGrade.SelectedItem?.ToString() ?? "DG";
            if (int.TryParse(maxRetries.Text, out var mr)) cfg.Tuner.MaxRetries = mr;
            if (double.TryParse(clickDelay.Text, out var cd)) cfg.Tuner.Timing.ClickEnterDelay = cd;
            if (double.TryParse(ocrDelay.Text, out var od)) cfg.Tuner.Timing.OcrDelay = od;
            cfg.Tuner.Filter.MatchMode = matchMode.SelectedItem?.ToString() ?? "any";
            var rg = requireGrade.SelectedItem?.ToString();
            cfg.Tuner.Filter.RequireGrade = (rg == null || rg == "None") ? null : rg;
            cfg.Tuner.Filter.Enabled = filterEnabled.IsChecked ?? false;
            cfg.Tuner.Filter.Rules = ruleRows.Select(r => r.ToRule()).ToList();
            cfg.Tuner.Filter.OverrideRules = overrideRows.Select(r => r.ToRule()).ToList();
            cfg.Tuner.SaveCaptures = saveCaptures.IsChecked ?? false;
            cfg.Tuner.SpringMode = springMode.SelectedItem?.ToString() ?? "manual";
            cfg.Tuner.MouseGuard = mouseGuard.SelectedItem?.ToString() ?? "off";
            SaveReport(() => _service.SaveConfig(), result,
                $"Written to defaults.yaml — target {cfg.Tuner.TargetGrade}, " +
                $"{cfg.Tuner.Filter.Rules.Count} rule(s), {cfg.Tuner.Filter.OverrideRules.Count} override(s).");
        };
        panel.Children.Add(save);
        panel.Children.Add(result);

        return MakeTab("Tuner", panel);
    }

    private TabItem BuildGemTab()
    {
        // No horizontal margin: the cards should line up with the tab strip's left border, not sit
        // 8px inside it. Vertical margin keeps a little breathing room at the top.
        var panel = new StackPanel { Margin = new Thickness(0, 8, 0, 8) };

        panel.Children.Add(Hint(
            "How the Gem Composer starts and what it does when a grade runs out of resources. " +
            "The click points and move set live in Calibrate Gem."));

        var startGrade = MakeComboBox(GemGrades, _service.Config.Gem.StartGrade);
        // What the composer does when the result window is empty (grade ran out of resources).
        var emptyMode = MakeComboBox(
            GemEmptyModes.Select(m => m.Label).ToList(),
            GemEmptyModes.First(m => m.Value == _service.Config.Gem.EmptyMode).Label);

        var runFields = new StackPanel();
        runFields.Children.Add(LabeledField("Start grade", startGrade));
        runFields.Children.Add(LabeledField("On empty result", emptyMode));
        panel.Children.Add(Section("Run", runFields));

        var saveEmptyCaptures = new CheckBox
        {
            IsChecked = _service.Config.Gem.SaveEmptyCaptures,
            Content = "Save empty-check captures",
            Foreground = Res("TextFillColorPrimaryBrush"),
        };
        panel.Children.Add(Section("Advanced",
            Hint("Writes the sampled result-box crop plus a diff line per cycle; useful when the " +
                 "empty check misbehaves, at the cost of disk I/O every cycle."),
            saveEmptyCaptures));

        var result = new InfoBar { IsOpen = false, IsClosable = true, Margin = new Thickness(0, 0, 0, 0) };
        var save = MakeButton("Save Gem Config", ControlAppearance.Primary);
        save.Click += (_, _) =>
        {
            _service.Config.Gem.StartGrade = startGrade.SelectedItem?.ToString() ?? "N";
            var mode = GemEmptyModes.First(m => m.Label == emptyMode.SelectedItem?.ToString());
            _service.Config.Gem.EmptyMode = mode.Value;
            _service.Config.Gem.SaveEmptyCaptures = saveEmptyCaptures.IsChecked ?? false;
            SaveReport(() => _service.SaveConfig(), result,
                $"Written to defaults.yaml — start at {_service.Config.Gem.StartGrade}, {mode.Value} on empty.");
        };
        panel.Children.Add(save);
        panel.Children.Add(result);

        return MakeTab("Gem", panel);
    }

    // One key + cooldown row in the spammer editor.
    private sealed class SpamKeyRow
    {
        // Fluent text boxes, so their height matches the preset ComboBox beside them — plain WPF
        // boxes are shorter and sat on a different baseline. No clear button: these are 88px wide.
        public Wpf.Ui.Controls.TextBox Key { get; } = UiText("", null, clearButton: false);
        public Wpf.Ui.Controls.TextBox Delay { get; } = UiText("", null, clearButton: false);
        // "fast" is the friendly face of the leading '*' on the key; the config still stores '*key'.
        //
        // Left to size itself on purpose. Do NOT pin Width here: the drawn glyph needs more than the
        // ~20px it looks like, and a Width of 20 clips it away COMPLETELY (measured — an empty gap
        // where the tick box should be). The column is what gives it room; see FastColumnWidth.
        public CheckBox Fast { get; } = new() { VerticalAlignment = VerticalAlignment.Center };
    }

    private TabItem BuildSpammerTab()
    {
        // No horizontal margin: the cards should line up with the tab strip's left border, not sit
        // 8px inside it. Vertical margin keeps a little breathing room at the top.
        var panel = new StackPanel { Margin = new Thickness(0, 8, 0, 8) };

        panel.Children.Add(Hint(
            "Keys the spammer presses, each on its own cooldown. Keys the Arduino can't send are " +
            "ignored, with a warning on the tool card."));

        // Named key sets — switching presets changes which rotation the spammer presses.
        var presets = _service.Config.Spammer.Presets;
        if (presets.Count == 0) presets["default"] = new Dictionary<string, double>();
        string current = presets.ContainsKey(_service.Config.Spammer.Active)
            ? _service.Config.Spammer.Active
            : presets.Keys.First();

        // Read-only summary of the active preset's keys, shown on the Active card so the editor
        // only has to appear while you're actually changing something. Each key is a small pill.
        var keysSummary = new WrapPanel { Margin = new Thickness(0, 14, 0, 6) };

        void UpdateSummary()
        {
            keysSummary.Children.Clear();
            if (!presets.TryGetValue(current, out var keys) || keys.Count == 0)
            {
                keysSummary.Children.Add(new TextBlock { Text = "(no keys)", Foreground = Res("TextFillColorSecondaryBrush") });
                return;
            }
            foreach (var kv in keys)
            {
                bool fast = kv.Key.StartsWith('*');
                var key = kv.Key.TrimStart('*');
                var keycap = new Border
                {
                    CornerRadius = new CornerRadius(6),
                    BorderBrush = Res("TextFillColorSecondaryBrush"),
                    BorderThickness = new Thickness(1),
                    MinWidth = 34,
                    MinHeight = 30,
                    Padding = new Thickness(6, 2, 6, 2),
                    Child = new TextBlock
                    {
                        Text = key,
                        Foreground = Res("TextFillColorPrimaryBrush"),
                        HorizontalAlignment = HorizontalAlignment.Center,
                        VerticalAlignment = VerticalAlignment.Center,
                        FontSize = 14,
                    },
                };
                var cd = new TextBlock
                {
                    Text = $"{kv.Value:g}s" + (fast ? " ⚡" : ""),
                    Foreground = Res("TextFillColorSecondaryBrush"),
                    HorizontalAlignment = HorizontalAlignment.Center,
                    FontSize = 11,
                    Margin = new Thickness(0, 2, 0, 0),
                };
                var tile = new StackPanel { Margin = new Thickness(0, 0, 10, 6) };
                tile.Children.Add(keycap);
                tile.Children.Add(cd);
                keysSummary.Children.Add(tile);
            }
        }

        var editButton = MakeButton("Edit", ControlAppearance.Secondary);
        var editor = new StackPanel { Visibility = Visibility.Collapsed };
        editButton.Click += (_, _) =>
        {
            bool nowEditing = editor.Visibility != Visibility.Visible;
            editor.Visibility = nowEditing ? Visibility.Visible : Visibility.Collapsed;
            editButton.Content = nowEditing ? "Done" : "Edit";
            if (!nowEditing) UpdateSummary();
        };

        var rows = new List<SpamKeyRow>();
        // The Fast column. 48 was too narrow and clipped the tick box's RIGHT BORDER away, leaving a
        // "C" — left border and top/bottom stubs, no right edge. The glyph asks for more than it
        // appears to: at 48 less the cell's 8px margin the control got 40, and the border fell about
        // 2px outside. Pinching it the other way is not a fix either — a Width of 20 clips it away
        // entirely. Measured both ways; 60 leaves ~52 for a ~42px glyph.
        const double FastColumnWidth = 60;

        // A grid, not a stack of labelled rows: ten rows each repeating "Key" / "Delay (s)" was
        // noise. One header, then bare boxes lined up underneath it.
        var rowsPanel = new Grid();
        rowsPanel.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(96) });
        rowsPanel.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(96) });
        rowsPanel.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(FastColumnWidth) });
        rowsPanel.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var keyHeader = new Grid { Margin = new Thickness(0, 0, 0, 2) };
        keyHeader.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(96) });
        keyHeader.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(96) });
        keyHeader.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(FastColumnWidth) });
        keyHeader.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var keyHeaderLabel = new TextBlock { Text = "Key", Foreground = Res("TextFillColorSecondaryBrush"), FontSize = 12 };
        var delayHeaderLabel = new TextBlock { Text = "Delay (s)", Foreground = Res("TextFillColorSecondaryBrush"), FontSize = 12 };
        var fastHeaderLabel = new TextBlock { Text = "Fast", Foreground = Res("TextFillColorSecondaryBrush"), FontSize = 12 };
        Grid.SetColumn(keyHeaderLabel, 0);
        Grid.SetColumn(delayHeaderLabel, 1);
        Grid.SetColumn(fastHeaderLabel, 2);
        keyHeader.Children.Add(keyHeaderLabel);
        keyHeader.Children.Add(delayHeaderLabel);
        keyHeader.Children.Add(fastHeaderLabel);
        // MinWidth is a floor for a very narrow window, not the width it takes: it sits in a star
        // column now, so it stretches into whatever the Rename and Delete buttons leave.
        var presetBox = new ComboBox { MinWidth = 110, VerticalAlignment = VerticalAlignment.Center };
        var presetName = UiText("");
        presetName.Width = 160;
        presetName.VerticalAlignment = VerticalAlignment.Center;

        // Contextual name prompt — appears only while creating or renaming a preset.
        var promptLabel = new TextBlock { VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 8, 0) };
        var confirmBtn = MakeButton("Create", ControlAppearance.Primary);
        var cancelBtn = MakeButton("Cancel", ControlAppearance.Secondary);
        cancelBtn.Margin = new Thickness(8, 0, 0, 0);
        var namePrompt = new StackPanel { Orientation = Orientation.Horizontal, Visibility = Visibility.Collapsed, Margin = new Thickness(0, 4, 0, 8) };
        namePrompt.Children.Add(promptLabel);
        namePrompt.Children.Add(presetName);
        namePrompt.Children.Add(confirmBtn);
        namePrompt.Children.Add(cancelBtn);
        string? promptMode = null; // "create" | "rename" | null

        void ShowPrompt(string mode, string label, string button)
        {
            promptMode = mode;
            promptLabel.Text = label;
            confirmBtn.Content = button;
            namePrompt.Visibility = Visibility.Visible;
            presetName.Focus();
        }

        void AddRow(string key, string delay)
        {
            var row = new SpamKeyRow();
            bool fast = key.StartsWith('*');
            if (fast) key = key[1..];
            row.Key.Text = key;
            row.Key.Width = 88;
            row.Delay.Text = delay;
            row.Delay.Width = 88;
            row.Fast.IsChecked = fast;
            row.Key.Margin = new Thickness(0, 2, 8, 2);
            row.Delay.Margin = new Thickness(0, 2, 8, 2);
            row.Fast.Margin = new Thickness(0, 2, 8, 2);

            int r = rowsPanel.RowDefinitions.Count;
            rowsPanel.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

            var del = new UiButton { Content = "✕", Appearance = ControlAppearance.Secondary, MinWidth = 28, Margin = new Thickness(0, 2, 0, 2) };
            del.Click += (_, _) =>
            {
                rowsPanel.Children.Remove(row.Key);
                rowsPanel.Children.Remove(row.Delay);
                rowsPanel.Children.Remove(row.Fast);
                rowsPanel.Children.Remove(del);
                rows.Remove(row);
            };

            Grid.SetRow(row.Key, r); Grid.SetColumn(row.Key, 0);
            Grid.SetRow(row.Delay, r); Grid.SetColumn(row.Delay, 1);
            Grid.SetRow(row.Fast, r); Grid.SetColumn(row.Fast, 2);
            Grid.SetRow(del, r); Grid.SetColumn(del, 3);
            rowsPanel.Children.Add(row.Key);
            rowsPanel.Children.Add(row.Delay);
            rowsPanel.Children.Add(row.Fast);
            rowsPanel.Children.Add(del);
            rows.Add(row);
        }

        void LoadRows(string name)
        {
            rowsPanel.Children.Clear();
            rowsPanel.RowDefinitions.Clear();
            rows.Clear();
            if (!presets.TryGetValue(name, out var keys)) return;
            foreach (var kv in keys)
                AddRow(kv.Key, kv.Value.ToString(CultureInfo.InvariantCulture));
        }

        const string AddNewMarker = "＋ Add new…";

        void RefreshPresetList(string select)
        {
            var names = presets.Keys.OrderBy(k => k, StringComparer.OrdinalIgnoreCase).ToList();
            names.Add(AddNewMarker);
            presetBox.ItemsSource = names;
            presetBox.SelectedItem = select;
        }

        var status = new TextBlock
        {
            Foreground = Res("TextFillColorSecondaryBrush"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 0, 0, 8),
        };

        bool loading = false;
        presetBox.SelectionChanged += (_, _) =>
        {
            if (loading || presetBox.SelectedItem is not string name) return;
            if (name == AddNewMarker)
            {
                // Snap the picker back to the real preset, then open the editor for the new name.
                loading = true; presetBox.SelectedItem = current; loading = false;
                editor.Visibility = Visibility.Visible;
                editButton.Content = "Done";
                presetName.Text = "";
                ShowPrompt("create", "Create preset:", "Create");
                return;
            }
            if (name == current) return;
            presets[current] = RowsToKeys(); // keep unsaved edits when switching
            current = name;
            LoadRows(name);
            UpdateSummary();
        };

        // Closing the prompt also restores the Edit/Done button. Rename hides it while the prompt is
        // open, and nothing put it back — so after a single rename the editor could never be
        // collapsed again, and if it was already collapsed it could not be reopened at all.
        void ClosePrompt()
        {
            namePrompt.Visibility = Visibility.Collapsed;
            promptMode = null;
            editButton.Visibility = Visibility.Visible;
        }

        cancelBtn.Click += (_, _) => ClosePrompt();

        confirmBtn.Click += (_, _) =>
        {
            var name = presetName.Text.Trim();
            if (name.Length == 0) { status.Text = "Type a name first."; return; }
            if (promptMode == "create")
            {
                if (presets.ContainsKey(name)) { status.Text = $"A preset named '{name}' already exists."; return; }
                presets[current] = RowsToKeys();
                presets[name] = new Dictionary<string, double>();
                current = name;
                loading = true; RefreshPresetList(name); loading = false;
                LoadRows(name);
                status.Text = $"Added preset '{name}'.";
            }
            else if (promptMode == "rename")
            {
                if (name == current) { status.Text = "That is already the name."; return; }
                if (presets.ContainsKey(name)) { status.Text = $"A preset named '{name}' already exists."; return; }
                presets[current] = RowsToKeys();
                var keys = presets[current];
                presets.Remove(current);
                presets[name] = keys;
                var old = current;
                current = name;
                loading = true; RefreshPresetList(name); loading = false;
                status.Text = $"Renamed '{old}' to '{name}'.";
            }
            presetName.Text = "";
            ClosePrompt();
            UpdateSummary();
        };

        var renamePreset = MakeButton("Rename", ControlAppearance.Secondary);
        renamePreset.Click += (_, _) =>
        {
            editor.Visibility = Visibility.Visible;
            // The editor is open, so the button reads Done — matching the "＋ Add new…" path. It is
            // hidden only while the name prompt is up, and ClosePrompt brings it back.
            editButton.Content = "Done";
            editButton.Visibility = Visibility.Collapsed;
            presetName.Text = current;
            ShowPrompt("rename", "Rename to:", "Rename");
        };
        var delPreset = MakeButton("Delete", ControlAppearance.Secondary);
        delPreset.Click += (_, _) =>
        {
            if (presets.Count <= 1) { status.Text = "The last preset cannot be deleted."; return; }
            var confirm = MessageBox.Show(
                $"Delete preset '{current}' and its keys?",
                "Delete preset",
                MessageBoxButton.YesNo,
                MessageBoxImage.Warning);
            if (confirm != MessageBoxResult.Yes) return;
            var gone = current;
            presets.Remove(current);
            current = presets.Keys.First();
            loading = true; RefreshPresetList(current); loading = false;
            LoadRows(current);
            UpdateSummary();
            status.Text = $"Deleted preset '{gone}'.";
        };
        foreach (var kv in presets[current])
            AddRow(kv.Key, kv.Value.ToString(CultureInfo.InvariantCulture));

        var addButton = MakeButton("+ Add Key", ControlAppearance.Secondary);
        addButton.Click += (_, _) => AddRow("", "0.2");

        // Two aligned rows rather than one long run of labels and buttons: Delete acts on the picked
        // preset, so it sits with the picker; + New and Rename act on the typed name, so they sit with
        // the name field. LabeledField gives both rows the same label column.
        foreach (var b in new[] { renamePreset, delPreset })
            b.Margin = new Thickness(8, 0, 0, 0);

        // A Grid, not a horizontal StackPanel. A StackPanel measures its children with unbounded width,
        // so the picker took its whole MinWidth and the two buttons their natural size, and the total
        // ran past the card — at a 619-logical-px window the Delete button was clipped mid-word
        // ("Delet"). With the picker in a star column it takes what is left rather than demanding it,
        // so the row fits at any width instead of only at the width it was built at.
        var pickRow = new Grid();
        pickRow.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        pickRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        pickRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        Grid.SetColumn(presetBox, 0);
        Grid.SetColumn(renamePreset, 1);
        Grid.SetColumn(delPreset, 2);
        pickRow.Children.Add(presetBox);
        pickRow.Children.Add(renamePreset);
        pickRow.Children.Add(delPreset);

        panel.Children.Add(Section("Active",
            Hint("Which key set the spammer presses. The keys below are a read-only summary; click Edit " +
                 "to change them. Switching the preset is not saved until Save Preset."),
            LabeledField("Preset", pickRow),
            keysSummary,
            editButton,
            status));
        editor.Children.Add(namePrompt);
        editor.Children.Add(Section("Keys",
            // Said here as well as in the tab intro: the default preset is all "*0, *1, …" rows, and
            // the one thing a reader needs to know about them is what that star means.
            Hint("* is a fast tap — the key is held about 10 ms instead of the normal 30–80 ms. " +
                 "Without it the press is longer, which is what most games want for a held skill. " +
                 "Any single letter or digit works, plus F1–F12."),
            keyHeader, rowsPanel, addButton));

        Dictionary<string, double> RowsToKeys()
        {
            var result = new Dictionary<string, double>();
            foreach (var r in rows)
            {
                var k = r.Key.Text.Trim();
                if (k.Length == 0) continue;
                if (r.Fast.IsChecked == true) k = "*" + k;
                if (double.TryParse(r.Delay.Text.Trim(), NumberStyles.Float, CultureInfo.InvariantCulture, out var d))
                    result[k] = d;
            }
            return result;
        }

        void RebuildRowsFromRaw(string text)
        {
            var parsed = ParseKeys(text);
            if (parsed.Count == 0) return; // empty/garbage — leave the rows alone
            rowsPanel.Children.Clear();
            rowsPanel.RowDefinitions.Clear();
            rows.Clear();
            foreach (var kv in parsed)
                AddRow(kv.Key, kv.Value.ToString(CultureInfo.InvariantCulture));
        }

        // Advanced: the same data as raw "key:seconds" lines, for setups the rows can't express.
        var advanced = new CheckBox
        {
            Content = "Advanced — edit the raw key:seconds list",
            Foreground = Res("TextFillColorPrimaryBrush"),
            Margin = new Thickness(0, 12, 0, 4),
        };
        var raw = new TextBox
        {
            AcceptsReturn = true,
            Height = 110,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        };
        var rawPanel = new StackPanel { Visibility = Visibility.Collapsed };
        rawPanel.Children.Add(LabeledField("Keys (key:seconds)", raw));
        editor.Children.Add(Section("Advanced",
            Hint("The same data as raw key:seconds lines, for setups the rows above can't express. " +
                 "Tick the box to edit it; unticking rebuilds the rows from your text."),
            advanced, rawPanel));

        advanced.Checked += (_, _) =>
        {
            raw.Text = SerializeKeys(RowsToKeys()); // reflect the rows before editing raw
            rawPanel.Visibility = Visibility.Visible;
        };
        advanced.Unchecked += (_, _) =>
        {
            RebuildRowsFromRaw(raw.Text);
            rawPanel.Visibility = Visibility.Collapsed;
        };

        var result = new InfoBar { IsOpen = false, IsClosable = true };
        var save = MakeButton("Save Preset", ControlAppearance.Primary);
        save.Click += (_, _) =>
        {
            presets[current] = advanced.IsChecked == true ? ParseKeys(raw.Text) : RowsToKeys();
            _service.Config.Spammer.Active = current;
            // Presets are the player's own rotations, so they go to local.yaml with the calibration.
            // defaults.yaml is the template publish.bat ships, so a preset saved there would be
            // published with the next release. The in-memory config is already correct: `presets`
            // is the same dictionary the spammer tool reads.
            var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
            local.Spammer = new ConfigLoader.LocalSpammer
            {
                Active = current,
                Presets = presets.ToDictionary(kv => kv.Key, kv => kv.Value),
            };
            SaveReport(() => _service.SaveLocal(local), result,
                $"Preset '{current}' written to local.yaml ({presets[current].Count} key(s)).");
            UpdateSummary();
        };
        // The editor goes in before the save button, not after. It used to be added last, which put
        // Save Preset ABOVE the prompt and the two cards it saves — and with the editor collapsed,
        // which is how the tab opens, a lone Save button with nothing above it. The calibrator tabs
        // had this same fault ("the save used to sit between two cards, belonging to neither"); the
        // rule is status card, then the action, last.
        panel.Children.Add(editor);
        panel.Children.Add(save);
        panel.Children.Add(result);

        loading = true;
        RefreshPresetList(current);
        loading = false;
        UpdateSummary();

        return MakeTab("Spammer", panel);
    }

    private TabItem BuildAttributesTab()
    {
        // No horizontal margin: the cards should line up with the tab strip's left border, not sit
        // 8px inside it. Vertical margin keeps a little breathing room at the top.
        var panel = new StackPanel { Margin = new Thickness(0, 8, 0, 8) };

        panel.Children.Add(Hint(
            "OCR attribute dictionary — the item attributes the tuner can recognize and match against " +
            "your filter rules. \"Name\" is what a filter rule matches on; \"OCR variants\" are the " +
            "garbled forms OCR actually produces and auto-corrects to that name. Read-only: edit " +
            "attributes.yaml and restart to change it."));

        var grid = new DataGrid
        {
            AutoGenerateColumns = false,
            IsReadOnly = true,
            CanUserAddRows = false,
            CanUserDeleteRows = false,
            HeadersVisibility = DataGridHeadersVisibility.Column,
            GridLinesVisibility = DataGridGridLinesVisibility.Horizontal,
            // Transparent so the table sits inside its card rather than painting a second surface.
            Background = Brushes.Transparent,
            Foreground = Res("TextFillColorPrimaryBrush"),
            RowBackground = Brushes.Transparent,
            BorderThickness = new Thickness(0),
        };

        grid.Columns.Add(new DataGridTextColumn { Header = "Name", Binding = new Binding("Name"), Width = new DataGridLength(160) });
        grid.Columns.Add(new DataGridTextColumn { Header = "Category", Binding = new Binding("Category"), Width = new DataGridLength(100) });
        grid.Columns.Add(new DataGridTextColumn { Header = "OCR variants", Binding = new Binding("Variants"), Width = new DataGridLength(1, DataGridLengthUnitType.Star) });

        grid.ItemsSource = _service.Attributes.Attributes
            .Select(a => new { Name = a.Name, Category = a.Category, Variants = string.Join(" / ", a.Variants) })
            .ToList();

        panel.Children.Add(Section($"Dictionary · {_service.Attributes.Attributes.Count} attributes", grid));

        return MakeTab("Attributes", panel);
    }

    private TabItem BuildTunerCalibrateTab()
    {
        // The status/output line of this tab: capture result, OCR dump, save confirmation. Mono, not
        // a warning colour — it prints multi-line diagnostics rather than warnings.
        var hint = Mono();
        hint.Text = "Capture the 發條 window to begin.";
        _tunerHint = hint;

        var image = new Image { Stretch = Stretch.Uniform };
        var canvas = new Canvas { Background = Brushes.Transparent, MinHeight = 300 };
        _tunerImage = image;
        _tunerCanvas = canvas;

        canvas.MouseDown += (_, e) => TunerMouseDown(canvas, e.GetPosition(canvas));
        canvas.MouseMove += (_, e) => TunerMouseMove(canvas, e.GetPosition(canvas));
        canvas.MouseUp += (_, e) => TunerMouseUp(canvas, e.GetPosition(canvas));

        var grid = new Grid { Margin = new Thickness(0, 8, 0, 8) };
        grid.Children.Add(image);
        grid.Children.Add(canvas);

        var capture = MakeButton("Capture 發條 window", ControlAppearance.Primary);
        capture.Click += (_, _) => TunerCapture();

        var save = MakeButton("Save Tuner", ControlAppearance.Primary);
        save.Click += (_, _) => TunerSave();

        var check = MakeButton("Check OCR", ControlAppearance.Secondary);
        check.Click += (_, _) => CheckTunerOcr();

        var springTest = MakeButton("Test Click (發條)", ControlAppearance.Secondary);
        springTest.Click += (_, _) => TunerSpringTestClick();

        var top = new StackPanel { Orientation = Orientation.Horizontal };
        top.Children.Add(capture);
        top.Children.Add(check);

        // No horizontal margin: the cards should line up with the tab strip's left border, not sit
        // 8px inside it. Vertical margin keeps a little breathing room at the top.
        var panel = new StackPanel { Margin = new Thickness(0, 8, 0, 8) };
        panel.Children.Add(Section("Capture",
            Hint("Open the 發條 (tuning) window first; the launcher hides itself for the grab so it " +
                 "cannot cover the game. The image must show the whole window."),
            top));
        panel.Children.Add(Section("Boxes",
            Hint("Drag three boxes on the capture: the grade letter, the three attribute lines, and " +
                 "the spring count. They are colour-coded, and a too-small drag is ignored."),
            grid));
        panel.Children.Add(Section("Spring (發條)",
            Hint("Click the 發條 button in the capture to record its point, then Test Click to confirm " +
                 "the cursor lands on it. Only used when spring_mode is \"hid\" (see the Tuner tab)."),
            springTest));
        panel.Children.Add(Section("Result", hint));
        panel.Children.Add(save);

        return MakeTab("Calibrate Tuner", panel);
    }

    private TabItem BuildGemCalibrateTab()
    {
        // The status/output line of this tab: what a test just did, where the cursor landed, whether
        // a save worked. Mono, because most of it is coordinates and machine output.
        var hint = Mono();
        hint.Text = "Capture the gem window to begin.";
        _gemHint = hint;

        var image = new Image { Stretch = Stretch.Uniform };
        var canvas = new Canvas { Background = Brushes.Transparent, MinHeight = 300 };
        _gemImage = image;
        _gemCanvas = canvas;

        canvas.MouseDown += (_, e) => GemMouseDown(canvas, e.GetPosition(canvas));
        canvas.MouseMove += (_, e) => GemMouseMove(canvas, e.GetPosition(canvas));
        canvas.MouseUp += (_, e) => GemMouseUp(canvas, e.GetPosition(canvas));

        var grid = new Grid { Margin = new Thickness(0, 8, 0, 8) };
        grid.Children.Add(image);
        grid.Children.Add(canvas);
        _gemGrid = grid;

        var capture = MakeButton("Capture gem window", ControlAppearance.Primary);
        capture.Click += (_, _) => GemCapture();

        // Diagnostics: report the window rects and save a sample capture. Advanced — it is a
        // one-off "why is the capture wrong" tool, not part of calibrating.
        var diag = MakeButton("Diagnose capture", ControlAppearance.Secondary);
        diag.Click += (_, _) => DiagnoseCapture();
        var captureRow = new StackPanel { Orientation = Orientation.Horizontal };
        captureRow.Children.Add(capture);

        var save = MakeButton("Save Gem Composer", ControlAppearance.Primary);
        save.Click += (_, _) => GemSave();

        // Point/move tests and result tests as two simple horizontal rows (same clean look as the
        // Tuner tab). "from"/"to" drive the two move tests; the result-gem checks sit on their own
        // row below. Buttons keep their natural width but drop the top-10 margin so they line up
        // with the dropdowns.
        var testBox = new ComboBox { ItemsSource = GemTestPoints, SelectedIndex = 0, MinWidth = 130 };
        var testFromBox = new ComboBox { ItemsSource = GemTestPoints, SelectedIndex = 0, MinWidth = 130 };
        var testBtn = MakeButton("Place cursor + click", ControlAppearance.Secondary);
        testBtn.Click += (_, _) => GemTestClick(testBox);
        var testMoveBtn = MakeButton("Test tuned move", ControlAppearance.Secondary);
        testMoveBtn.Click += (_, _) => GemTestRelativeMove(testFromBox, testBox);
        var checkColBtn = MakeButton("Check Result Colour", ControlAppearance.Secondary);
        checkColBtn.Click += (_, _) => GemCheckResultColor();
        var testGemBtn = MakeButton("Sample result gem", ControlAppearance.Secondary);
        testGemBtn.Click += (_, _) => GemTestResult();
        _gemTestPoint = testBox;

        foreach (var b in new UiButton[] { testBtn, testMoveBtn, checkColBtn, testGemBtn })
            b.Margin = new Thickness(6, 0, 6, 0); // left+right gap, drop the top-10 so buttons align with the dropdowns

        var pointRow = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 8, 0, 0) };
        pointRow.Children.Add(new TextBlock { Text = "from", VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 6, 0) });
        pointRow.Children.Add(testFromBox);
        pointRow.Children.Add(new TextBlock { Text = "to", VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(12, 0, 6, 0) });
        pointRow.Children.Add(testBox);
        pointRow.Children.Add(testBtn);
        pointRow.Children.Add(testMoveBtn);

        var resultRow = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 8, 0, 0) };
        resultRow.Children.Add(checkColBtn);
        resultRow.Children.Add(testGemBtn);

        // The two cursor APIs, tested separately on the SELECTED point with the real computed
        // coordinates, so their behaviour can be compared directly.
        var debugCursorBtn = MakeButton("Debug Cursor (logical)", ControlAppearance.Secondary);
        debugCursorBtn.Margin = new Thickness(6, 0, 6, 0);
        debugCursorBtn.Click += (_, _) => DebugCursorPath(testBox, physical: false);

        var debugPhysicalBtn = MakeButton("Debug Physical", ControlAppearance.Secondary);
        debugPhysicalBtn.Margin = new Thickness(6, 0, 6, 0);
        debugPhysicalBtn.Click += (_, _) => DebugCursorPath(testBox, physical: true);

        // Advanced: the one-off diagnostics, out of the calibration path. Same gap on every button —
        // they used to have mixed margins, so the spacing read as misalignment.
        var advancedRow = new StackPanel { Orientation = Orientation.Horizontal };
        foreach (var b in new[] { diag, debugCursorBtn, debugPhysicalBtn })
        {
            b.Margin = new Thickness(0, 0, 8, 0);
            advancedRow.Children.Add(b);
        }

        // Result-gem box crop preview: once the result box is dragged, show the exact region
        // being sampled for empty-detection, so the user can visually confirm it's over the
        // empty slot (and not an offset/mis-sized area).
        var resultPreview = new Image { Stretch = Stretch.Uniform, MaxWidth = 260, MaxHeight = 140, Margin = new Thickness(0, 4, 0, 0) };
        _gemResultPreview = resultPreview;
        var resultPreviewLabel = new TextBlock
        {
            Text = "Result gem box crop (empty-detection region):",
            Foreground = Res("TextFillColorSecondaryBrush"),
            Margin = new Thickness(0, 10, 0, 0),
            TextWrapping = TextWrapping.Wrap,
        };
        var resultPreviewPanel = new StackPanel { Margin = new Thickness(0) };
        resultPreviewPanel.Children.Add(resultPreviewLabel);
        resultPreviewPanel.Children.Add(resultPreview);

        // No horizontal margin: the cards should line up with the tab strip's left border, not sit
        // 8px inside it. Vertical margin keeps a little breathing room at the top.
        var panel = new StackPanel { Margin = new Thickness(0, 8, 0, 8) };

        panel.Children.Add(Section("Capture",
            Hint("Open the gem combine window first; the launcher hides itself for the grab so it " +
                 "cannot cover the game. The image must show the whole window."),
            captureRow));

        panel.Children.Add(Section("Points",
            Hint("Click each button in the captured image in order — N, G, DG, Register, Combine, " +
                 "then the three resource slots — and finally drag a box around the composed result gem."),
            grid));

        // Editable coordinate fields (positions). Prefilled from the saved config so the user can
        // fix a point directly (e.g. N/G/DG all on the same vertical level) without re-capturing.

        var cinv = System.Globalization.CultureInfo.InvariantCulture;
        var coordGrid = new Grid { Margin = new Thickness(0, 0, 0, 6) };
        coordGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(96) });  // label
        coordGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(64) });  // X
        coordGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(64) });  // Y
        coordGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(64) });  // W
        coordGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(64) });  // H

        var coordBold = FontWeights.SemiBold;
        int cr = 0;
        coordGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Place(coordGrid, new TextBlock { Text = "Point", FontWeight = coordBold, Margin = new Thickness(0, 0, 8, 4) }, cr, 0);
        Place(coordGrid, new TextBlock { Text = "X", FontWeight = coordBold, Margin = new Thickness(0, 0, 8, 4) }, cr, 1);
        Place(coordGrid, new TextBlock { Text = "Y", FontWeight = coordBold, Margin = new Thickness(0, 0, 8, 4) }, cr, 2);
        Place(coordGrid, new TextBlock { Text = "W", FontWeight = coordBold, Margin = new Thickness(0, 0, 8, 4) }, cr, 3);
        Place(coordGrid, new TextBlock { Text = "H", FontWeight = coordBold, Margin = new Thickness(0, 0, 8, 4) }, cr, 4);
        cr++;

        // Fluent, like every other field in the app — a plain WPF box is a different height, which
        // showed up as the coordinate grid not matching the rest of the tab.
        Wpf.Ui.Controls.TextBox MakeCoordBox(string val)
        {
            var box = UiText(val, null, clearButton: false);
            box.Width = 56;
            box.VerticalAlignment = VerticalAlignment.Center;
            box.Margin = new Thickness(0, 0, 8, 6);
            return box;
        }

        void AddCoordRow(string label, string key, int x, int y, int? w = null, int? h = null)
        {
            var row = new GemCoordRow { Key = key, X = MakeCoordBox(x.ToString(cinv)), Y = MakeCoordBox(y.ToString(cinv)) };
            if (w.HasValue) row.W = MakeCoordBox(w.Value.ToString(cinv));
            if (h.HasValue) row.H = MakeCoordBox(h.Value.ToString(cinv));
            _gemCoordRows.Add(row);
            coordGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            Place(coordGrid, new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 8, 6) }, cr, 0);
            Place(coordGrid, row.X, cr, 1);
            Place(coordGrid, row.Y, cr, 2);
            if (row.W != null) Place(coordGrid, row.W, cr, 3);
            if (row.H != null) Place(coordGrid, row.H, cr, 4);
            cr++;
        }

        var gem = _service.Config.Gem;
        foreach (var key in CalibGemSteps)
            if (gem.GradePositions.TryGetValue(key, out var pos) && pos.Count > 1)
                AddCoordRow(key, key, pos[0], pos[1]);
        for (int i = 0; i < gem.ResourceGems.Count; i++)
        {
            var r = gem.ResourceGems[i];
            if (r.Count > 1) AddCoordRow($"Resource{i + 1}", $"Resource{i + 1}", r[0], r[1]);
        }
        if (gem.ResultGemArea is { Count: 4 } a)
            AddCoordRow("Result area", "ResultArea", a[0], a[1], a[2], a[3]);

        var saveCoords = MakeButton("Save Coordinates", ControlAppearance.Primary);
        saveCoords.Click += (_, _) => SaveCoordinates();
        panel.Children.Add(Section("Coordinates",
            Hint("The points the clicks land on, in client-relative physical pixels. Edit a value " +
                 "directly to nudge a point (e.g. N/G/DG on the same level) without re-capturing."),
            coordGrid, saveCoords));

        // Read-only checks: nothing here writes config or clicks in the game except the two move
        // tests, which only click the buttons you picked.
        panel.Children.Add(Section("Tests",
            Hint("Place the cursor on a point, run a single route, or sample the result box. " +
                 "Nothing here changes the config."),
            pointRow, resultRow, resultPreviewPanel));

        // Tuned move editor: one row per route on a shared Grid so the label / dx / dy / Test columns
        // line up. dx/dy are the RAW counts the composer sends (no computation) — they prefill from
        // the config, the per-row button sends that exact move, and Save writes them back.
        // One shared definition of the routes (Core/GemRoutes.cs), so the tuned editor, the arduino
        // editor below and the composer can never drift apart.
        var routes = GemRoutes.All;

        // Grid with margin-based spacing: a right margin on each cell provides the column gap, and a
        // bottom margin provides the row gap, so rows/columns never touch. Controls size naturally
        // (no fixed heights) and are centred vertically — the same clean look as the Tuner tab.
        const double ColGap = 8, RowGap = 6;
        var moveGrid = new Grid { Margin = new Thickness(0, 0, 0, 6) };
        moveGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(200) });  // route label (fits "Resource1 → Resource2")
        moveGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(76) });   // dx box
        moveGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(76) });   // dy box
        moveGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });      // Test btn
        for (int r = 0; r <= routes.Count; r++)
            moveGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        // Header row, one cell per column, with the same column gap so it lines up below.
        var headBold = FontWeights.SemiBold;
        Place(moveGrid, new TextBlock { Text = "Move", FontWeight = headBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, 0, 0);
        Place(moveGrid, new TextBlock { Text = "dx", FontWeight = headBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, 0, 1);
        Place(moveGrid, new TextBlock { Text = "dy", FontWeight = headBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, 0, 2);
        Place(moveGrid, new TextBlock { Text = "Send", FontWeight = headBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 0, RowGap) }, 0, 3);

        var inv = System.Globalization.CultureInfo.InvariantCulture;
        int moveRow = 1;
        foreach (var route in routes)
        {
            var cur = GetGemMovement(route.Key);
            // Width = column (76) - right gap (ColGap), so the box fits flush inside its cell and
            // the WPF-UI rounded corners aren't clipped on the right edge.
            var dxBox = new TextBox
            {
                Text = cur is { Count: >= 1 } ? cur[0].ToString(inv) : "",
                Width = 76 - ColGap,
                VerticalContentAlignment = VerticalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
                Margin = new Thickness(0, 0, ColGap, RowGap),
            };
            var dyBox = new TextBox
            {
                Text = cur is { Count: >= 2 } ? cur[1].ToString(inv) : "",
                Width = 76 - ColGap,
                VerticalContentAlignment = VerticalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
                Margin = new Thickness(0, 0, ColGap, RowGap),
            };
            var row = new GemMoveRow { Key = route.Key, From = route.From, To = route.To, Dx = dxBox, Dy = dyBox };
            _gemMoveRows.Add(row);

            var testB = MakeButton("Send", ControlAppearance.Secondary);
            testB.Margin = new Thickness(0, 0, 0, RowGap); // align with the boxes, add only the row gap
            System.Windows.Automation.AutomationProperties.SetName(testB, $"Send tuned move {route.Label}");
            testB.Click += (_, _) => GemTestMovement(row.From, row.To, ParseMove(row.Dx), ParseMove(row.Dy));

            Place(moveGrid, new TextBlock { Text = route.Label, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, moveRow, 0);
            Place(moveGrid, dxBox, moveRow, 1);
            Place(moveGrid, dyBox, moveRow, 2);
            Place(moveGrid, testB, moveRow, 3);
            moveRow++;
        }
        // Save ONLY the tuned counts, independent of the full click-point calibration that
        // "Save Gem Composer" requires.
        var saveMoves = MakeButton("Save tuned counts", ControlAppearance.Primary);
        saveMoves.Click += (_, _) => SaveComposerMoves();

        // The mode selector gets its OWN card, deliberately not the arduino card: that card is
        // disabled whenever the other set is active, which disabled the selector with it and left the
        // mode impossible to switch back from.
        _gemMoveMode = MakeComboBox(GemMoveModes, _service.Config.Gem.MoveMode);
        var modeRow = new StackPanel { Orientation = Orientation.Horizontal };
        modeRow.Children.Add(new TextBlock
        {
            Text = "Composer move mode",
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 8, 0),
        });
        modeRow.Children.Add(_gemMoveMode);
        panel.Children.Add(Section("Move set",
            Hint("Which set the composer uses. The set that is NOT active is dimmed below; switching " +
                 "here wakes it up again."),
            modeRow));

        var tunedCard = Section("Moves — tuned (hand-tuned counts)",
            Hint("What the composer sends when move mode is \"tuned\": one raw D dx dy per route, " +
                 "tuned by hand on this PC. The per-row button sends that exact move."),
            moveGrid, saveMoves);
        panel.Children.Add(tunedCard);

        // The other move set: the same routes, each ending on a calibrated POINT that the Arduino
        // drives the cursor to (closed loop). Nothing above is replaced — the composer picks with
        // gem.move_mode, and this card is how the set is tried on a live run first.
        var arduinoGrid = new Grid { Margin = new Thickness(0, 0, 0, 6) };
        arduinoGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(200) }); // route label
        arduinoGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(130) }); // destination
        arduinoGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });     // Test btn
        for (int r = 0; r <= routes.Count; r++)
            arduinoGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        Place(arduinoGrid, new TextBlock { Text = "Move", FontWeight = headBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, 0, 0);
        Place(arduinoGrid, new TextBlock { Text = "Goes to", FontWeight = headBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, 0, 1);
        Place(arduinoGrid, new TextBlock { Text = "Run", FontWeight = headBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 0, RowGap) }, 0, 2);

        int arduinoRow = 1;
        foreach (var route in routes)
        {
            var testB = MakeButton("Run", ControlAppearance.Secondary);
            testB.Margin = new Thickness(0, 0, 0, RowGap);
            System.Windows.Automation.AutomationProperties.SetName(testB, $"Run arduino route {route.Label}");
            testB.Click += (_, _) => GemTestArduinoRoute(route.From, route.To);

            Place(arduinoGrid, new TextBlock { Text = route.Label, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, arduinoRow, 0);
            Place(arduinoGrid, new TextBlock { Text = route.To, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, arduinoRow, 1);
            Place(arduinoGrid, testB, arduinoRow, 2);
            arduinoRow++;
        }
        var arduinoCard = Section("Moves — arduino (cursor placed on the point)",
            Hint("What the composer sends when move mode is \"arduino\": it places the cursor on the " +
                 "route's destination point and re-aims every move, so nothing needs tuning. The " +
                 "per-row button runs the route for real: click the source, place, click the target."),
            arduinoGrid);
        panel.Children.Add(arduinoCard);

        // Dim whichever set the composer is not using, so the two grids can't be confused. The mode
        // selector is inside the arduino card, so switching is one click away.
        void ApplyMoveMode()
        {
            bool arduino = (_gemMoveMode?.SelectedItem as string) == "arduino";
            arduinoCard.Opacity = arduino ? 1.0 : 0.45;
            tunedCard.Opacity = arduino ? 0.45 : 1.0;
            arduinoCard.IsEnabled = arduino;
            tunedCard.IsEnabled = !arduino;
        }
        _gemMoveMode.SelectionChanged += (_, _) => ApplyMoveMode();
        ApplyMoveMode();

        // One complete cycle, driven entirely by the arduino moves — the fastest way to see whether
        // the new set survives a real run before switching the composer over to it.
        var cycleBtn = MakeButton("Run one full cycle", ControlAppearance.Primary);
        cycleBtn.Click += (_, _) => GemTestFullCycle();
        panel.Children.Add(Section("Full-run test",
            Hint("Plays one whole cycle with the arduino moves — N, G and DG combined, resource slots " +
                 "cleared in between, stopping after DG. This one really clicks in the game: 21 clicks."),
            cycleBtn));

        panel.Children.Add(Section("Advanced",
            Hint("Input tests that compare the two cursor APIs, plus a capture diagnostic. None of " +
                 "these calibrate anything on their own."),
            advancedRow));

        // Same shape as Calibrate Tuner: the status card, then the primary action last — the save
        // used to sit between two cards, belonging to neither.
        panel.Children.Add(Section("Result", hint));
        panel.Children.Add(save);

        return MakeTab("Calibrate Gem", panel);
    }

    // Resolve a calibrated point by name, preferring the just-dragged/clicked in-session point
    // then falling back to the saved config (so a test works right after a restart, before
    // re-capturing the canvas). Returns null when the name isn't calibrated.
    private Point? ResolveGemPoint(string name)
    {
        var cfg = _service.Config.Gem;
        Point? pt = name == "ResultCenter"
            ? (_gemResultBox is { } rb ? new Point(rb.X + rb.Width / 2, rb.Y + rb.Height / 2) : (Point?)null)
            : _gemPoints.TryGetValue(name, out var p) ? (Point?)p : null;
        pt ??= name switch
        {
            _ when name == "ResultCenter" => cfg.ResultGemCenter is { } c ? new Point(c.X, c.Y) : (Point?)null,
            _ when name.StartsWith("Resource", StringComparison.Ordinal) =>
                int.TryParse(name.AsSpan(8), out var i) && cfg.ResourceGems.Count > i - 1
                    ? new Point(cfg.ResourceGems[i - 1][0], cfg.ResourceGems[i - 1][1]) : (Point?)null,
            _ when cfg.GradePositions.TryGetValue(name, out var pos) && pos.Count > 1
                => new Point(pos[0], pos[1]),
            _ => null,
        };
        return pt;
    }

    // RAW hand-tuned movement counts for a composer pair (grade -> Register, Register -> Combine,
    // Combine -> Register). These are exactly what the composer sends for its "D" moves. The Test
    // Move feature sends THESE counts (the same value the composer sends) — never a computed
    // pixel delta, which is what broke the composer before. Returns null if the pair has no
    // movement defined.
    private List<int>? ResolveGemMovement(string fromName, string toName)
    {
        var m = _service.Config.Gem.Movements;
        if (toName == "Register" && _service.Config.Gem.GradePositions.ContainsKey(fromName))
            return m.RadioToRegister.TryGetValue(fromName, out var r) ? r : null;
        if (fromName == "Register" && toName == "Combine") return m.RegisterCombine.Count == 2 ? m.RegisterCombine : null;
        if (fromName == "Combine" && toName == "Register") return m.CombineRegister.Count == 2 ? m.CombineRegister : null;
        // Adjacent grade-to-grade (e.g. N -> G): the composer's "next/prev grade" moves.
        var grades = _service.Config.Gem.Grades;
        int fi = grades.IndexOf(fromName), ti = grades.IndexOf(toName);
        if (fi >= 0 && ti >= 0 && Math.Abs(fi - ti) == 1)
            return ti > fi ? m.GradeNext : m.GradePrev;
        return null;
    }

    // Current raw [dx, dy] for a composer-move route key ("radio_N"/"radio_G"/"radio_DG",
    // "register_combine", "combine_register"). Used to prefill the Calibrate rows.
    private List<int>? GetGemMovement(string key)
    {
        var mv = _service.Config.Gem.Movements;
        if (key.StartsWith("radio_", StringComparison.Ordinal))
            return mv.RadioToRegister.TryGetValue(key[6..], out var r) ? r : null;
        return key switch
        {
            "register_combine" => mv.RegisterCombine.Count == 2 ? mv.RegisterCombine : null,
            "combine_register" => mv.CombineRegister.Count == 2 ? mv.CombineRegister : null,
            "register_slot1" => mv.RegisterSlot1.Count == 2 ? mv.RegisterSlot1 : null,
            "slot1_slot2" => mv.Slot1Slot2.Count == 2 ? mv.Slot1Slot2 : null,
            "slot2_slot3" => mv.Slot2Slot3.Count == 2 ? mv.Slot2Slot3 : null,
            "slot3_n" => mv.Slot3ToN.Count == 2 ? mv.Slot3ToN : null,
            "slot3_g" => mv.Slot3ToG.Count == 2 ? mv.Slot3ToG : null,
            "slot3_dg" => mv.Slot3ToDg.Count == 2 ? mv.Slot3ToDg : null,
            _ => null,
        };
    }

    // Write the user's fine-tuned [dx, dy] back into the in-memory movement config.
    private void SetGemMovement(string key, int dx, int dy)
    {
        var v = new List<int> { dx, dy };
        var mv = _service.Config.Gem.Movements;
        if (key.StartsWith("radio_", StringComparison.Ordinal)) { mv.RadioToRegister[key[6..]] = v; return; }
        switch (key)
        {
            case "register_combine": mv.RegisterCombine = v; break;
            case "combine_register": mv.CombineRegister = v; break;
            case "register_slot1": mv.RegisterSlot1 = v; break;
            case "slot1_slot2": mv.Slot1Slot2 = v; break;
            case "slot2_slot3": mv.Slot2Slot3 = v; break;
            case "slot3_n": mv.Slot3ToN = v; break;
            case "slot3_g": mv.Slot3ToG = v; break;
            case "slot3_dg": mv.Slot3ToDg = v; break;
        }
    }

    // Test a raw composer move WITHOUT computing anything: focus-click the from point, send the
    // user's exact D dx dy, then confirm-click the to point. The same value the composer sends.
    private async void GemTestMovement(string fromName, string toName, int dx, int dy)
    {
        var from = ResolveGemPoint(fromName);
        if (from == null) { _gemHint!.Text = $"Start \"{fromName}\" isn't calibrated yet."; return; }
        var target = ResolveGemPoint(toName);
        if (target == null) { _gemHint!.Text = $"Target \"{toName}\" isn't calibrated yet."; return; }

        var display = HidPointer.Display(_service.Config.Window.Title);
        if (display == null) { _gemHint!.Text = "Game window not found (or minimized) — open and restore the game first."; return; }
        var ser = await _service.ArduinoPortAsync();
        if (ser == null) { _gemHint!.Text = "Arduino not found — plug it in and retry."; return; }

        try
        {
            // v1 single-click core: place(from) -> C -> D dx dy -> C (no focus-click).
            var placed = HidPointer.To(ser, WindowFinder.ComputeCursorTarget(display, (int)from.Value.X, (int)from.Value.Y));
            if (!placed.Ok)
            {
                _gemHint!.Text = $"Couldn't place the cursor on \"{fromName}\" — {placed.Error}. Nothing was clicked.";
                return;
            }
            System.Threading.Thread.Sleep(300);
            HidPointer.Click(ser);
            System.Threading.Thread.Sleep(500);
            HidPointer.Move(ser, dx, dy);
            System.Threading.Thread.Sleep(300);
            HidPointer.Click(ser);
            System.Threading.Thread.Sleep(300);

            _gemHint!.Text = $"Test move: click \"{fromName}\" then D {dx} {dy} then click \"{toName}\". Does it land?";
        }
        catch (Exception ex) { _gemHint!.Text = $"Test move failed: {ex.Message}"; }
    }

    // Parse a TextBox int; fall back to 0 on empty/garbage.
    private static int ParseMove(TextBox box) => int.TryParse(box.Text.Trim(), out var n) ? n : 0;

    // Lay an element into a grid cell. Keeps the Calibrate-Gem move editor aligned.
    private static void Place(Grid grid, UIElement el, int row, int col)
    {
        Grid.SetRow(el, row);
        Grid.SetColumn(el, col);
        grid.Children.Add(el);
    }

    // Move the cursor to the selected calibrated point (client-relative -> screen) without
    // clicking, so the user can verify each calibration position visually.
    private async void GemTestClick(ComboBox box)
    {
        if (box.SelectedItem is not string name)
        {
            _gemHint!.Text = "Pick a point to test.";
            return;
        }

        var pt = ResolveGemPoint(name);
        if (pt == null)
        {
            _gemHint!.Text = $"\"{name}\" isn't calibrated yet.";
            return;
        }

        var display = HidPointer.Display(_service.Config.Window.Title);
        if (display == null)
        {
            _gemHint!.Text = "Game window not found (or minimized) — open and restore the game first.";
            return;
        }

        var ser = await _service.ArduinoPortAsync();
        if (ser == null)
        {
            _gemHint!.Text = "Arduino not found — plug it in and retry.";
            return;
        }

        // v1 does a single cursor-place + C — no focus-click, no double click. Mirror that: put the
        // cursor on the point with the Arduino and click once. A cursor that can't be placed is
        // reported, never clicked through (docs/CURSOR-INVESTIGATION.md).
        try
        {
            var target = WindowFinder.ComputeCursorTarget(display, (int)pt.Value.X, (int)pt.Value.Y);
            var placed = HidPointer.To(ser, target);

            if (!placed.Ok)
            {
                _gemHint!.Text = $"Test click \"{name}\": couldn't place the cursor — {placed.Error}. Nothing was clicked.";
                LogCursorMove(name, target, placed);
                return;
            }

            System.Threading.Thread.Sleep(300);
            HidPointer.Click(ser);
            System.Threading.Thread.Sleep(200);

            _gemHint!.Text = $"Test click \"{name}\": cursor placed at ({placed.X},{placed.Y}) " +
                $"in {placed.Steps} Arduino move(s), clicked via {ser.PortName}.";
            LogCursorMove(name, target, placed);
        }
        catch (Exception ex)
        {
            _gemHint!.Text = $"Test click failed: {ex.Message}";
        }
    }

    // One line per Test Click, so a mis-placed cursor can be inspected after the fact. Replaces the
    // per-probe logging (accepted/retry/phys/bg/dpiCtx/clip) that found the SetCursorPos refusal.
    private static void LogCursorMove(string name, WindowFinder.CursorTarget target, CursorPlacement placed)
    {
        try
        {
            var final = placed.X is { } px && placed.Y is { } py ? $"({px},{py})" : "(?)";
            File.AppendAllText(LogPath("arduino_debug.txt"),
                $"{DateTime.Now:HH:mm:ss} test-click name={name} target=({target.LogicalX},{target.LogicalY}) " +
                $"ok={placed.Ok} steps={placed.Steps} final={final} fg=\"{WindowFinder.ForegroundTitle()}\"" +
                $"{(placed.Error == null ? "" : $" error=\"{placed.Error}\"")}\n");
        }
        catch { /* diagnostics must never break the click */ }
    }

    // Test a RELATIVE "D" move at 1:1 between two user-chosen points, mirroring the composer's
    // real anchor: focus-click the SOURCE (SetCursorPos -> C), then a relative move D(source->target)
    // at 1:1, then confirm-click the TARGET. This confirms the mouse scale is 1:1: if every step
    // lands on a button, the scale is correct. With a separated in-game cursor the pixel distance
    // can't be measured, so this is judged visually (does each click land?).
    private async void GemTestRelativeMove(ComboBox fromBox, ComboBox toBox)
    {
        if (fromBox.SelectedItem is not string fromName)
        {
            _gemHint!.Text = "Pick a START point (where the test focus-clicks).";
            return;
        }
        if (toBox.SelectedItem is not string toName)
        {
            _gemHint!.Text = "Pick a TARGET point to move to.";
            return;
        }
        var from = ResolveGemPoint(fromName);
        if (from == null)
        {
            _gemHint!.Text = $"Start \"{fromName}\" isn't calibrated yet.";
            return;
        }
        var target = ResolveGemPoint(toName);
        if (target == null)
        {
            _gemHint!.Text = $"Target \"{toName}\" isn't calibrated yet.";
            return;
        }

        var display = HidPointer.Display(_service.Config.Window.Title);
        if (display == null)
        {
            _gemHint!.Text = "Game window not found (or minimized) — open and restore the game first.";
            return;
        }
        var ser = await _service.ArduinoPortAsync();
        if (ser == null)
        {
            _gemHint!.Text = "Arduino not found — plug it in and retry.";
            return;
        }
        // The composer sends the RAW hand-tuned movement for a pair, not a computed pixel delta
        // (computing px/100 is what broke it before). So this test must send the SAME raw movement
        // the composer would send — then a move that lands here means the composer's is correct.
        var mv = ResolveGemMovement(fromName, toName);
        if (mv == null)
        {
            _gemHint!.Text = $"No raw movement for \"{fromName}\" → \"{toName}\". Pick a composer pair (e.g. N → Register, Register → Combine).";
            return;
        }
        try
        {

            // v1 single-click core: place(from) -> C -> D raw -> C (no focus-click, no double
            // click). Same raw movement the composer sends.
            var placed = HidPointer.To(ser, WindowFinder.ComputeCursorTarget(display, (int)from.Value.X, (int)from.Value.Y));
            if (!placed.Ok)
            {
                _gemHint!.Text = $"Couldn't place the cursor on \"{fromName}\" — {placed.Error}. Nothing was clicked.";
                return;
            }
            System.Threading.Thread.Sleep(300);
            HidPointer.Click(ser);
            System.Threading.Thread.Sleep(500);
            HidPointer.Move(ser, mv[0], mv[1]);
            System.Threading.Thread.Sleep(300);
            HidPointer.Click(ser);
            System.Threading.Thread.Sleep(300);

            _gemHint!.Text = $"Test move: click \"{fromName}\" then D {mv[0]} {mv[1]} then click \"{toName}\". Does it land?";
        }
        catch (Exception ex)
        {
            _gemHint!.Text = $"Test move failed: {ex.Message}";
        }
    }

    // Test a route with the NEW move set: place on the SOURCE point (Arduino closed loop), click,
    // then place on the DESTINATION point and click. These are the exact two placements the composer
    // makes when gem.move_mode is "arduino" — no tuned counts involved, so a route that lands here
    // lands in the composer. (GemTestRelativeMove above tests the tuned set.)
    private async void GemTestArduinoRoute(string fromName, string toName)
    {
        var from = ResolveGemPoint(fromName);
        if (from == null) { _gemHint!.Text = $"Start \"{fromName}\" isn't calibrated yet."; return; }
        var target = ResolveGemPoint(toName);
        if (target == null) { _gemHint!.Text = $"Target \"{toName}\" isn't calibrated yet."; return; }

        var display = HidPointer.Display(_service.Config.Window.Title);
        if (display == null) { _gemHint!.Text = "Game window not found (or minimized) — open and restore the game first."; return; }
        var ser = await _service.ArduinoPortAsync();
        if (ser == null) { _gemHint!.Text = "Arduino not found — plug it in and retry."; return; }

        try
        {
            var placedFrom = HidPointer.To(ser, WindowFinder.ComputeCursorTarget(display, (int)from.Value.X, (int)from.Value.Y));
            if (!placedFrom.Ok)
            {
                _gemHint!.Text = $"Couldn't place the cursor on \"{fromName}\" — {placedFrom.Error}. Nothing was clicked.";
                LogRouteMove(fromName, toName, placedFrom, null);
                return;
            }
            System.Threading.Thread.Sleep(300);
            HidPointer.Click(ser);
            System.Threading.Thread.Sleep(500);

            var placedTo = HidPointer.To(ser, WindowFinder.ComputeCursorTarget(display, (int)target.Value.X, (int)target.Value.Y));
            if (!placedTo.Ok)
            {
                _gemHint!.Text = $"Couldn't place the cursor on \"{toName}\" — {placedTo.Error}. Stopped instead of clicking blind.";
                LogRouteMove(fromName, toName, placedFrom, placedTo);
                return;
            }
            System.Threading.Thread.Sleep(300);
            HidPointer.Click(ser);
            System.Threading.Thread.Sleep(300);

            _gemHint!.Text = $"New move: placed on \"{fromName}\", clicked, then placed on \"{toName}\", clicked. Does it land?";
            LogRouteMove(fromName, toName, placedFrom, placedTo);
        }
        catch (Exception ex)
        {
            _gemHint!.Text = $"Test move failed: {ex.Message}";
        }
    }

    // One complete composer cycle driven by the ARDUINO move set (closed-loop placement, no tuned
    // counts) — the fastest way to validate the new set on a live run before switching the composer
    // to it. Mirrors the composer's own loop body, so the click order is the one the composer uses:
    //
    //   select grade → Register (register) → Combine (combine #1)
    //     → Register, Register (deregister + register, as the composer's normal path does)
    //     → Combine (combine #2)
    //     → right-click Resource1/2/3 to clear the slots
    //
    // N and G run that loop twice; DG runs it once and the cycle stops there (no trailing clear).
    private async void GemTestFullCycle()
    {
        if (_cycleRunning)
        {
            _gemHint!.Text = "A full-cycle test is already running.";
            return;
        }

        // action = which HID click; Point = a calibrated point name (Core/GemRoutes.Resolve knows
        // the same names). Two Register clicks in a row are deliberate: deregister the produced gem,
        // then register the next batch — without them a second combine does nothing.
        var steps = new (string Action, string Point)[]
        {
            ("click", "N"), ("click", "Register"), ("click", "Combine"),
            ("click", "Register"), ("click", "Register"), ("click", "Combine"),
            ("rclick", "Resource1"), ("rclick", "Resource2"), ("rclick", "Resource3"),
            ("click", "G"), ("click", "Register"), ("click", "Combine"),
            ("click", "Register"), ("click", "Register"), ("click", "Combine"),
            ("rclick", "Resource1"), ("rclick", "Resource2"), ("rclick", "Resource3"),
            ("click", "DG"), ("click", "Register"), ("click", "Combine"),
        };

        // Resolve every point BEFORE clicking anything: a cycle that half-runs because one point
        // wasn't calibrated would leave the game in a confusing state.
        var targets = new (string Action, string Point, Point At)[steps.Length];
        for (int i = 0; i < steps.Length; i++)
        {
            var pt = ResolveGemPoint(steps[i].Point);
            if (pt == null)
            {
                _gemHint!.Text = $"Full cycle: \"{steps[i].Point}\" isn't calibrated yet — nothing was clicked.";
                return;
            }
            targets[i] = (steps[i].Action, steps[i].Point, pt.Value);
        }

        // Claim the guard BEFORE the first await. ArduinoPortAsync waits out a 2 s boot delay on a
        // cold start, and a second click used to pass the check at the top of this method while the
        // first was still suspended — two cycles then drove the game at the same time.
        _cycleRunning = true;
        try
        {
            var ser = await _service.ArduinoPortAsync();
            if (ser == null)
            {
                _gemHint!.Text = "Arduino not found — plug it in and retry.";
                return;
            }

            for (int i = 0; i < targets.Length; i++)
            {
                var (action, point, at) = targets[i];
                _gemHint!.Text = $"Full cycle {i + 1}/{targets.Length}: {action} \"{point}\"…";

                var display = HidPointer.Display(_service.Config.Window.Title);
                if (display == null)
                {
                    _gemHint!.Text = $"Full cycle stopped at {i + 1}/{targets.Length} ({action} \"{point}\") — game window not found (or minimized).";
                    LogCycle($"stopped at {i + 1}/{targets.Length} {action} {point}: game window gone");
                    return;
                }

                var placed = HidPointer.To(ser, WindowFinder.ComputeCursorTarget(display, (int)at.X, (int)at.Y));
                if (!placed.Ok)
                {
                    _gemHint!.Text = $"Full cycle stopped at {i + 1}/{targets.Length} ({action} \"{point}\") — {placed.Error}.";
                    LogCycle($"stopped at {i + 1}/{targets.Length} {action} {point}: {placed.Error}");
                    return;
                }

                System.Threading.Thread.Sleep(300);
                if (action == "click") HidPointer.Click(ser);
                else HidPointer.RightClick(ser);
                // A combine click gets longer: the game animates the result before the next step.
                System.Threading.Thread.Sleep(point == "Combine" ? 800 : 500);
            }

            _gemHint!.Text = $"Full cycle done ({targets.Length} steps): N and G each combined twice " +
                "(with the composer's deregister+register between them) and had their resource slots cleared, " +
                "then DG combined once. Did every step land?";
            LogCycle($"done steps={targets.Length}");
        }
        catch (Exception ex)
        {
            _gemHint!.Text = $"Full cycle failed: {ex.Message}";
            LogCycle($"failed: {ex.Message}");
        }
        finally
        {
            _cycleRunning = false;
        }
    }

    // One line per full-cycle run, so a cycle that stopped halfway can be inspected afterwards.
    private static void LogCycle(string outcome)
    {
        try
        {
            File.AppendAllText(LogPath("arduino_debug.txt"),
                $"{DateTime.Now:HH:mm:ss} test-cycle-arduino {outcome} fg=\"{WindowFinder.ForegroundTitle()}\"\n");
        }
        catch { /* diagnostics must never break the cycle */ }
    }

    // One line per New-Gem-Composer-Moves test, mirroring the Test Click line so a route that
    // didn't land can be inspected after the fact.
    private static void LogRouteMove(string fromName, string toName, CursorPlacement from, CursorPlacement? to)
    {
        try
        {
            static string Pos(CursorPlacement p) => p.X is { } x && p.Y is { } y ? $"({x},{y})" : "(?)";
            File.AppendAllText(LogPath("arduino_debug.txt"),
                $"{DateTime.Now:HH:mm:ss} test-route-arduino {fromName} → {toName} " +
                $"from ok={from.Ok} at {Pos(from)}, to ok={to?.Ok} at {(to == null ? "-" : Pos(to))} " +
                $"fg=\"{WindowFinder.ForegroundTitle()}\"\n");
        }
        catch { /* diagnostics must never break the move */ }
    }

    // Sample the live result box and report its colour composition ("colour code"), plus the
    // distance to the sampled empty reference and the empty/has-gem verdict.
    private void GemCheckResultColor()
    {
        if (!SampleResultBox(out var frame, out var sig, out var rb)) return;
        var verdict = sig == null
            ? " — no empty reference yet (save with the box empty to capture it)"
            : $" — distance {GemColorAnalyzer.Distance(frame, sig):0.000} => " +
              (GemColorAnalyzer.IsEmpty(frame, sig, _service.Config.Gem.EmptyDistance) ? "EMPTY" : "has gem");
        _gemHint!.Text = $"Result colour: {DescribeColor(frame)}{verdict}";
    }

    // Test empty-vs-gem detection: sample the CURRENT result box (manually put a gem in the box,
    // or leave it empty) and report the colour + channel spread + verdict, so the user can
    // validate the discriminator against a real gem by hand. No in-game clicking.
    private void GemTestResult()
    {
        if (!SampleResultBox(out var frame, out var sig, out var rb)) return;
        var spread = Spread(frame);
        // Rough expected signs: empty slot is near-white/pale (spread small, RGB means close);
        // a red/blue/green gem dominates ONE channel (spread large, one RGB mean far from others).
        var dominant = DominantTone(frame);
        var baseVerdict = sig == null
            ? "no empty reference yet"
            : (GemColorAnalyzer.IsEmpty(frame, sig, _service.Config.Gem.EmptyDistance) ? "EMPTY" : "has gem");
        var hint = $"Test object: {DescribeColor(frame)}  channel spread {spread:0.00}  tone {dominant}  => {baseVerdict}" +
                   (sig != null ? $" (distance {GemColorAnalyzer.Distance(frame, sig):0.000})" : "");
        if (sig != null)
            hint += $"\n  vs empty ref: dV {Math.Abs(frame.MeanV - sig.MeanV):0.000}  dS {Math.Abs(frame.MeanSat - sig.MeanSat):0.000}  dC {Math.Abs(frame.ColoredFraction - sig.ColoredFraction):0.000}  dB {Math.Abs(frame.MeanB - sig.MeanB):0.000}";
        _gemHint!.Text = hint;
    }

    // Resolve the result box (in-session drag or saved config) and sample it. Sets a hint and
    // returns false on failure.
    private bool SampleResultBox(out ColorComposition frame, out ColorComposition? sig, out Rect rb)
    {
        frame = null!; sig = null; rb = default;
        var a = _service.Config.Gem.ResultGemArea;
        var box = _gemResultBox ?? (a is { Count: 4 }
            ? new Rect(a[0], a[1], a[2], a[3]) : (Rect?)null);
        if (box == null)
        {
            _gemHint!.Text = "No result box yet — drag it during gem calibration (or it's not saved).";
            return false;
        }
        rb = box.Value;
        var hwnd = WindowFinder.FindByTitle(_service.Config.Window.Title);
        var analyzed = GemColorAnalyzer.Analyze(hwnd, (int)rb.X, (int)rb.Y, (int)rb.Width, (int)rb.Height,
            _service.Config.Gem.ColoredGapMin);
        if (analyzed == null)
        {
            _gemHint!.Text = "Game window not found (or minimized) — open and restore the game first.";
            return false;
        }
        frame = analyzed;
        sig = _service.Config.Gem.EmptySignature;
        SaveResultCrop(hwnd, rb);
        return true;
    }

    // Channel dominance of the mean colour: max(r,g,b) - min(r,g,b). Pale-yellow empty slots are
    // near-neutral (small spread); a red/blue/green gem dominates one channel (large spread).
    private static double Spread(ColorComposition c) =>
        Math.Max(c.MeanR, Math.Max(c.MeanG, c.MeanB)) - Math.Min(c.MeanR, Math.Min(c.MeanG, c.MeanB));

    // Rough reading of the dominant channel, for the Test Result Gem hint: an empty slot is
    // near-white/pale (no dominant channel => "neutral"); a gem is a single hue.
    private static string DominantTone(ColorComposition c)
    {
        if (Spread(c) < 0.08) return "neutral / pale";
        var r = c.MeanR; var g = c.MeanG; var b = c.MeanB;
        return r >= g && r >= b ? "reddom"
             : g >= r && g >= b ? "greendom"
             : "bluedom";
    }

    // Save a crop-out PNG of the result-gem box so its exact pixels can be compared later
    // (e.g. against the empty reference or a previous run). Writes logs/captures/result_box.png.
    private static void SaveResultCrop(IntPtr hwnd, Rect rb)
    {
        try
        {
            var cap = ScreenCapture.CaptureClientRegion(hwnd, new SealTools.Core.Config.RegionConfig
            { Left = (int)rb.X, Top = (int)rb.Y, Width = (int)rb.Width, Height = (int)rb.Height });
            if (cap == null) return;
            using var mat = cap.Image;
            var dir = LogPath("captures");
            Directory.CreateDirectory(dir);
            mat.ImWrite(Path.Combine(dir, "result_box.png"));
        }
        catch { /* diagnostics must never break the check */ }
    }

    // Render the captured screenshot at its NATURAL size with the calibration dots + result box
    // overlaid at their natural (whole-window) coordinates — so the saved PNG has the overlays
    // in the exact right spots, independent of the on-screen Stretch.Uniform scaling.
    private void SaveGemCalibrationImage(string fileName)
    {
        var shot = _gemScreenshot;
        if (shot == null) return;
        var visual = new DrawingVisual();
        using (var dc = visual.RenderOpen())
        {
            dc.DrawImage(shot, new Rect(0, 0, shot.PixelWidth, shot.PixelHeight));
            foreach (var pt in _gemPoints.Values)
                dc.DrawEllipse(Brushes.Yellow, new Pen(Brushes.White, 1), new Point(pt.X, pt.Y), 5, 5);
            if (_gemResultBox is { } box)
                dc.DrawRectangle(null, new Pen(Brushes.Magenta, 2), box);
        }
        var bmp = new RenderTargetBitmap(shot.PixelWidth, shot.PixelHeight, 96, 96, PixelFormats.Pbgra32);
        bmp.Render(visual);
        var encoder = new PngBitmapEncoder();
        encoder.Frames.Add(BitmapFrame.Create(bmp));
        var path = CalibrationImagePath(fileName);
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        using var fs = File.Create(path);
        encoder.Save(fs);
    }

    // Render the tuner screenshot at NATURAL size with the three calibration bands overlaid, so the
    // saved reference shows exactly which bands were selected — same approach as the Gem tab.
    // (The old version rendered the on-screen grid, so the PNG came out at display size.)
    private void SaveTunerCalibrationImage(string fileName)
    {
        var shot = _tunerScreenshot;
        if (shot == null) return;
        var visual = new DrawingVisual();
        using (var dc = visual.RenderOpen())
        {
            dc.DrawImage(shot, new Rect(0, 0, shot.PixelWidth, shot.PixelHeight));
            if (_tunerGradeBox is { } g)
                dc.DrawRectangle(null, new Pen(Brushes.LimeGreen, 3), g);
            if (_tunerAttrBox is { } a)
                dc.DrawRectangle(null, new Pen(Brushes.DodgerBlue, 3), a);
            if (_tunerRemainingBox is { } r)
                dc.DrawRectangle(null, new Pen(Brushes.Orange, 3), r);
        }
        var bmp = new RenderTargetBitmap(shot.PixelWidth, shot.PixelHeight, 96, 96, PixelFormats.Pbgra32);
        bmp.Render(visual);
        var encoder = new PngBitmapEncoder();
        encoder.Frames.Add(BitmapFrame.Create(bmp));
        var path = CalibrationImagePath(fileName);
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        using var fs = File.Create(path);
        encoder.Save(fs);
    }

    // Reload a saved calibration reference image into a tab (boxes are shown baked in, NOT
    // re-activated). Returns true when the file was loaded.
    private static bool LoadCalibrationImage(Image? target, string fileName)
    {
        if (target == null) return false;
        var path = CalibrationImagePath(fileName);
        if (!File.Exists(path)) return false;
        try
        {
            var bmp = new BitmapImage();
            bmp.BeginInit();
            bmp.CacheOption = BitmapCacheOption.OnLoad;
            bmp.UriSource = new Uri(path, UriKind.Absolute);
            bmp.EndInit();
            bmp.Freeze();
            target.Source = bmp;
            return true;
        }
        catch { return false; }
    }

    // Absolute path of a calibration reference image (co-located with config/local.yaml).
    private static string CalibrationImagePath(string fileName)
        => Path.Combine(FindRootDir(), "config", fileName);

    // Environment block recorded with a calibration (physical pixels). See docs/COORDINATES.md.
    private static CalibrationInfo ToCalibration(DisplayInfo d) => new()
    {
        DpiScale = Math.Round(d.Scale, 4),
        MonitorDpi = d.MonitorDpi,
        Screen = new List<int> { d.ScreenWidth, d.ScreenHeight },
        ClientSize = new List<int> { d.PhysicalClient.Width, d.PhysicalClient.Height },
        MeasuredAt = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture),
    };

    // Runtime logs live under the app root (next to config/), matching the tools' FileLogger paths.
    // They must NOT go under AppContext.BaseDirectory: in a dev run that is the build output folder,
    // so the files land somewhere different from logs/ and are easy to lose.
    private static string LogPath(params string[] parts)
        => Path.Combine(new[] { FindRootDir(), "logs" }.Concat(parts).ToArray());

    // Called once at launch: if a Save Tuner / Save Gem wrote a calibration reference image,
    // show it in its tab. Boxes are baked into the PNG and NOT re-activated — the drag handlers
    // stay inert because _tunerScreenshot/_gemScreenshot remain null, so the reloaded picture is
    // a static reference until the user Captures + Saves a new calibration.
    private void LoadCalibrationImages()
    {
        if (LoadCalibrationImage(_tunerImage, "calib_tuner.png"))
            _tunerHint!.Text = "Showing last saved Tuner calibration (reference). Capture + Save to update.";
        if (LoadCalibrationImage(_gemImage, "calib_gem.png"))
            _gemHint!.Text = "Showing last saved Gem calibration (reference). Capture + Save to update.";
        LoadCalibrationImage(_gemResultPreview, "calib_gem_result.png");
    }

    // Compact human-readable colour composition.
    private static string DescribeColor(ColorComposition c) =>
        $"V {c.MeanV * 100:0}% S {c.MeanSat * 100:0}% colour {c.ColoredFraction * 100:0}% " +
        $"RGB({c.MeanR * 255:0},{c.MeanG * 255:0},{c.MeanB * 255:0})";

    // ── Tuner calibrator (drag an OCR box) ──────────────────────────────────

    private async void TunerCapture()
    {
        var shot = await CaptureScreenshotAsync();
        if (shot == null)
        {
            _tunerHint!.Text = "Game window not found — open the game first.";
            return;
        }

        _tunerDisplay = shot.Value.Display;
        _tunerScreenshot = shot.Value.Image;
        _tunerImage!.Source = shot.Value.Image;
        _tunerStep = 0;
        _tunerGradeBox = null;
        _tunerAttrBox = null;
        _tunerRemainingBox = null;
        _tunerSpringPoint = null;
        _tunerDragStart = null;
        _tunerMarquee = null;
        _tunerCanvas!.Children.Clear();
        _tunerHint!.Text = "Step 1/4 — drag a box around the grade letter (e.g. DG / G / N).";
    }

    private void TunerMouseDown(Canvas canvas, Point p)
    {
        if (_tunerScreenshot == null) return;

        // Step 3 records the 發條 button with a single click (a press, not a drag).
        if (_tunerStep == 3)
        {
            _tunerSpringPoint = CanvasToNatural(p, _tunerScreenshot, canvas);
            AddDot(canvas, p);
            _tunerStep = 4;
            _tunerHint!.Text = "Spring point set — click Check OCR to verify, then Save Tuner.";
            return;
        }

        if (_tunerStep >= 4) return;
        _tunerDragStart = p;
        _tunerMarquee = new Rectangle { Stroke = Brushes.LimeGreen, StrokeThickness = 2, StrokeDashArray = new DoubleCollection { 4, 2 } };
        Canvas.SetLeft(_tunerMarquee, p.X);
        Canvas.SetTop(_tunerMarquee, p.Y);
        canvas.Children.Add(_tunerMarquee);
    }

    private void TunerMouseMove(Canvas canvas, Point p)
    {
        if (_tunerMarquee == null || _tunerDragStart == null) return;
        var x = Math.Min(_tunerDragStart.Value.X, p.X);
        var y = Math.Min(_tunerDragStart.Value.Y, p.Y);
        Canvas.SetLeft(_tunerMarquee, x);
        Canvas.SetTop(_tunerMarquee, y);
        _tunerMarquee.Width = Math.Abs(p.X - _tunerDragStart.Value.X);
        _tunerMarquee.Height = Math.Abs(p.Y - _tunerDragStart.Value.Y);
    }

    private void TunerMouseUp(Canvas canvas, Point p)
    {
        if (_tunerMarquee == null || _tunerDragStart == null || _tunerScreenshot == null || _tunerCanvas == null) return;
        var a = CanvasToNatural(_tunerDragStart.Value, _tunerScreenshot, _tunerCanvas);
        var b = CanvasToNatural(p, _tunerScreenshot, _tunerCanvas);
        var box = new Rect(
            new Point(Math.Min(a.X, b.X), Math.Min(a.Y, b.Y)),
            new Point(Math.Max(a.X, b.X), Math.Max(a.Y, b.Y)));

        // Reject accidental clicks/tiny drags — a zero-height band would crash OCR later.
        if (box.Width < 4 || box.Height < 4)
        {
            canvas.Children.Remove(_tunerMarquee);
            _tunerMarquee = null;
            _tunerDragStart = null;
            return;
        }

        // Solidify the marquee so the completed box stays visible (colour-coded per band).
        _tunerMarquee.Stroke = _tunerStep switch { 0 => Brushes.LimeGreen, 1 => Brushes.DodgerBlue, _ => Brushes.Orange };
        _tunerMarquee.StrokeDashArray = null;
        _tunerMarquee = null;
        _tunerDragStart = null;

        switch (_tunerStep)
        {
            case 0:
                _tunerGradeBox = box;
                _tunerStep = 1;
                _tunerHint!.Text = "Step 2/4 — drag a box around the 3 attribute lines.";
                break;
            case 1:
                _tunerAttrBox = box;
                _tunerStep = 2;
                _tunerHint!.Text = "Step 3/4 — drag a box around the spring count (remaining).";
                break;
            default:
                _tunerRemainingBox = box;
                _tunerStep = 3;
                _tunerHint!.Text = "Step 4/4 — click the 發條 button to record its point.";
                break;
        }
    }

    private void CheckTunerOcr()
    {
        bool dragged = _tunerGradeBox != null && _tunerAttrBox != null && _tunerRemainingBox != null;

        // No in-session drag? Check the SAVED geometry from local.yaml, so an existing calibration
        // can be verified against the live game without capturing again.
        var ocr = dragged
            ? BuildOcrGeometry(_tunerGradeBox!.Value, _tunerAttrBox!.Value, _tunerRemainingBox!.Value)
            : _service.Config.Tuner.Ocr;

        if (ocr.Region.Width < 20 || ocr.Region.Height < 20)
        {
            _tunerHint!.Text = dragged
                ? "The boxes are too small — drag real rectangles."
                : "No saved calibration yet — press Capture and drag the three boxes.";
            return;
        }

        _tunerHint!.Text = "Running OCR…";
        var result = _service.CheckOcr(ocr);
        _tunerAttrPitch = result?.AttrLineSpacing;
        if (result == null)
        {
            _tunerHint!.Text = "OCR failed — game window not found?";
            return;
        }

        var lines = new List<string>();
        if (!dragged) lines.Add("(checking the saved calibration from local.yaml)");
        lines.Add($"Grade: {result.Grade ?? "?"}");
        lines.Add(
            $"Remaining: {(result.Remaining.HasValue ? result.Remaining.Value.ToString(CultureInfo.InvariantCulture) : "?")}");
        for (var i = 0; i < result.Attributes.Count; i++)
        {
            lines.Add($"Attr {i + 1}: {string.Join(" ", result.Attributes[i])}");
        }

        // Show what the tuner's matcher makes of those raw lines, and the filter verdict — the same
        // processing a run performs, so the whole pipeline can be verified from here.
        var matched = new AttrMatcher(_service.Attributes).MatchAttributes(result.Attributes);
        lines.Add("");
        lines.Add("Matched:");
        foreach (var m in matched)
        {
            lines.Add($"  {m.Name} = {(m.Value.HasValue ? m.Value.Value.ToString(CultureInfo.InvariantCulture) : "?")}");
        }

        var filter = _service.Config.Tuner.Filter;
        if (filter.Enabled)
        {
            var fr = AttrMatcher.CheckFilter(matched, filter);
            lines.Add($"Filter: {(fr.Passed ? "MATCH" : "no match")} ({fr.Reason})");
        }
        _tunerHint!.Text = string.Join(Environment.NewLine, lines);
    }

    private void TunerSave()
    {
        if (_tunerGradeBox == null || _tunerAttrBox == null || _tunerRemainingBox == null)
        {
            _tunerHint!.Text = "Drag all three boxes first (grade, attributes, remaining).";
            return;
        }

        var ocr = BuildOcrGeometry(_tunerGradeBox.Value, _tunerAttrBox.Value, _tunerRemainingBox.Value, _tunerAttrPitch);
        if (ocr.Region.Width < 20 || ocr.Region.Height < 20)
        {
            _tunerHint!.Text = "The boxes are too small — drag real rectangles.";
            return;
        }

        var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
        local.Tuner = new ConfigLoader.LocalTuner { Ocr = ocr };
        if (_tunerSpringPoint is { } sp)
            local.Tuner.SpringPoint = new List<int> { (int)Math.Round(sp.X), (int)Math.Round(sp.Y) };
        if (_tunerDisplay is { } d)
        {
            local.Calibration = ToCalibration(d);
            _service.Config.Calibration = local.Calibration;
        }
        if (!TrySaveCalibration(() => _service.SaveLocal(local), _tunerHint!,
                "Tuner saved to config\\local.yaml."))
            return;
        // Refresh the in-memory config too, or the next tuner run still uses the
        // startup geometry (seeded from local.yaml.example) instead of the boxes
        // just calibrated — the cause of the "Check OCR is fine, run drifts" bug.
        _service.Config.Tuner.Ocr = ocr;
        if (_tunerSpringPoint is { } sp2)
            _service.Config.Tuner.SpringPoint = new List<int> { (int)Math.Round(sp2.X), (int)Math.Round(sp2.Y) };
        SaveTunerCalibrationImage("calib_tuner.png");
        _tunerHint!.Text = "Tuner saved to config\\local.yaml.";
        MessageBox.Show("Tuner calibration saved.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
    }

    private async void TunerSpringTestClick()
    {
        if (_tunerSpringPoint is not { } sp)
        {
            _tunerHint!.Text = "No spring point recorded — click the 發條 button in the capture first.";
            return;
        }

        var display = HidPointer.Display(_service.Config.Window.Title);
        if (display == null)
        {
            _tunerHint!.Text = "Game window not found (or minimized) — open and restore the game first.";
            return;
        }

        var ser = await _service.ArduinoPortAsync();
        if (ser == null)
        {
            _tunerHint!.Text = "Arduino not found — plug it in and retry.";
            return;
        }

        // Same place-and-click as the gem tab's Test Click: the Arduino closed loop, one click,
        // never click blind.
        try
        {
            var target = WindowFinder.ComputeCursorTarget(display, (int)Math.Round(sp.X), (int)Math.Round(sp.Y));
            var placed = HidPointer.To(ser, target);

            if (!placed.Ok)
            {
                _tunerHint!.Text = $"Test click (發條): couldn't place the cursor — {placed.Error}. Nothing was clicked.";
                LogCursorMove("spring", target, placed);
                return;
            }

            System.Threading.Thread.Sleep(300);
            HidPointer.Click(ser);
            System.Threading.Thread.Sleep(200);

            _tunerHint!.Text = $"Test click (發條): cursor placed at ({placed.X},{placed.Y}) in {placed.Steps} Arduino move(s), clicked via {ser.PortName}.";
            LogCursorMove("spring", target, placed);
        }
        catch (Exception ex)
        {
            _tunerHint!.Text = $"Test click failed: {ex.Message}";
        }
    }

    private static OcrGeometry BuildOcrGeometry(Rect grade, Rect attr, Rect remaining, int? rowHeight = null)
    {
        // Region = bounding box of the three measured bands; sub-bands are their
        // offsets relative to it. Everything is measured, so it adapts to any window.
        var left = (int)Math.Min(grade.Left, Math.Min(attr.Left, remaining.Left));
        var top = (int)Math.Min(grade.Top, Math.Min(attr.Top, remaining.Top));
        var right = (int)Math.Max(grade.Right, Math.Max(attr.Right, remaining.Right));
        var bottom = (int)Math.Max(grade.Bottom, Math.Max(attr.Bottom, remaining.Bottom));

        // row_height is the real line pitch when "Check OCR" measured it (preferred), else
        // fall back to attr.Height/3 — which is only exact if the box is drawn tight around
        // the three lines; a loose box (with padding) overestimates it and merges rows.
        var measured = rowHeight is > 0 ? rowHeight.Value : Math.Max(1, (int)Math.Round(attr.Height / 3.0));

        return new OcrGeometry
        {
            Region = new RegionConfig { Left = left, Top = top, Width = right - left, Height = bottom - top },
            GradeArea = new BoxConfig
            {
                X1 = (int)(grade.Left - left),
                Y1 = (int)(grade.Top - top),
                X2 = (int)(grade.Right - left),
                Y2 = (int)(grade.Bottom - top),
            },
            GradeY = new List<int> { (int)(grade.Top - top), (int)(grade.Bottom - top) },
            AttrY = new List<int> { (int)(attr.Top - top), (int)(attr.Bottom - top) },
            RemainingY = new List<int> { (int)(remaining.Top - top), (int)(remaining.Bottom - top) },
            AttrX = new List<int> { (int)(attr.Left - left), (int)(attr.Right - left) },
            RemainingX = new List<int> { (int)(remaining.Left - left), (int)(remaining.Right - left) },
            RowHeight = measured,
        };
    }

    // ── Gem calibrator (click buttons) ─────────────────────────────────────

    private async void GemCapture()
    {
        var shot = await CaptureScreenshotAsync();
        if (shot == null)
        {
            _gemHint!.Text = "Game window not found (or minimized) — open and restore the game first.";
            return;
        }

        _gemDisplay = shot.Value.Display;
        _gemScreenshot = shot.Value.Image;
        _gemImage!.Source = shot.Value.Image;
        _gemPoints.Clear();
        _gemResultBox = null;
        if (_gemResultPreview != null) _gemResultPreview.Source = null;
        _gemDragStart = null;
        _gemMarquee = null;
        _gemStep = 0;
        _gemCanvas!.Children.Clear();
        _gemHint!.Text = GemStepHint(_gemStep);
    }

    // Human-readable prompt for the current calibration step.
    private static string GemStepHint(int step)
    {
        if (step < CalibGemSteps.Length) return $"Click the \"{CalibGemSteps[step]}\" button.";
        if (step < GemResultBoxStep) return $"Click resource gem slot {step - CalibGemSteps.Length + 1}.";
        return "Drag a box around the composed result gem.";
    }

    private void GemMouseDown(Canvas canvas, Point p)
    {
        if (_gemScreenshot == null || _gemCanvas == null || _gemStep >= GemTotalSteps) return;

        // The final step is the result-gem AREA: it needs a drag, not a click.
        if (_gemStep == GemResultBoxStep)
        {
            _gemDragStart = p;
            _gemMarquee = new Rectangle { Stroke = Brushes.Magenta, StrokeThickness = 2, StrokeDashArray = new DoubleCollection { 4, 2 } };
            Canvas.SetLeft(_gemMarquee, p.X);
            Canvas.SetTop(_gemMarquee, p.Y);
            canvas.Children.Add(_gemMarquee);
            return;
        }

        var label = _gemStep < CalibGemSteps.Length
            ? CalibGemSteps[_gemStep]
            : CalibResourceSteps[_gemStep - CalibGemSteps.Length];
        _gemPoints[label] = CanvasToNatural(p, _gemScreenshot, _gemCanvas);
        AddDot(canvas, p);
        _gemStep++;
        _gemHint!.Text = _gemStep < GemTotalSteps ? GemStepHint(_gemStep) : "All set — click Save.";
    }

    private void GemMouseMove(Canvas canvas, Point p)
    {
        if (_gemMarquee == null || _gemDragStart == null) return;
        var x = Math.Min(_gemDragStart.Value.X, p.X);
        var y = Math.Min(_gemDragStart.Value.Y, p.Y);
        Canvas.SetLeft(_gemMarquee, x);
        Canvas.SetTop(_gemMarquee, y);
        _gemMarquee.Width = Math.Abs(p.X - _gemDragStart.Value.X);
        _gemMarquee.Height = Math.Abs(p.Y - _gemDragStart.Value.Y);
    }

    private void GemMouseUp(Canvas canvas, Point p)
    {
        if (_gemMarquee == null || _gemDragStart == null || _gemScreenshot == null || _gemCanvas == null) return;
        var a = CanvasToNatural(_gemDragStart.Value, _gemScreenshot, _gemCanvas);
        var b = CanvasToNatural(p, _gemScreenshot, _gemCanvas);
        var box = new Rect(
            new Point(Math.Min(a.X, b.X), Math.Min(a.Y, b.Y)),
            new Point(Math.Max(a.X, b.X), Math.Max(a.Y, b.Y)));

        // Reject accidental clicks/tiny drags — a zero-size area would break OCR downstream.
        if (box.Width < 4 || box.Height < 4)
        {
            canvas.Children.Remove(_gemMarquee);
            _gemMarquee = null;
            _gemDragStart = null;
            return;
        }

        _gemMarquee.StrokeDashArray = null;
        _gemMarquee = null;
        _gemDragStart = null;
        _gemResultBox = box;
        _gemStep++;
        _gemHint!.Text = "All set — click Save.";
        ShowResultPreview(box);
    }

    // Show the cropped-out result gem box (the region empty-detection samples) so the user can
    // confirm it lines up with the empty slot. The box is in natural/client coords of the capture.
    private void ShowResultPreview(Rect box)
    {
        try
        {
            var shot = _gemScreenshot;
            var img = _gemResultPreview;
            if (shot == null || img == null) return;
            var w = (int)Math.Clamp(box.Width, 1, shot.PixelWidth);
            var h = (int)Math.Clamp(box.Height, 1, shot.PixelHeight);
            var x = (int)Math.Clamp(box.X, 0, Math.Max(0, shot.PixelWidth - w));
            var y = (int)Math.Clamp(box.Y, 0, Math.Max(0, shot.PixelHeight - h));
            img.Source = new CroppedBitmap(shot, new Int32Rect(x, y, w, h));
        }
        catch { /* preview must never break calibration */ }
    }

    // The given box from the CALIBRATION SCREENSHOT (launcher hidden) as an OpenCV Mat, or null
    // when there is no screenshot yet. Same pixels SaveResultGemCrop writes as the reference image,
    // so the empty signature and the reference crop can never disagree.
    private Mat? CropScreenshot(Rect box)
    {
        var shot = _gemScreenshot;
        if (shot == null) return null;
        int x = (int)Math.Clamp(box.X, 0, Math.Max(0, shot.PixelWidth - 1));
        int y = (int)Math.Clamp(box.Y, 0, Math.Max(0, shot.PixelHeight - 1));
        int w = (int)Math.Clamp(box.Width, 1, shot.PixelWidth - x);
        int h = (int)Math.Clamp(box.Height, 1, shot.PixelHeight - y);
        var bgr = new FormatConvertedBitmap(shot, PixelFormats.Bgr24, null, 0);
        var mat = new Mat(h, w, OpenCvSharp.MatType.CV_8UC3);
        bgr.CopyPixels(new Int32Rect(x, y, w, h), mat.Data, h * w * 3, w * 3);
        return mat;
    }

    // Save the result-gem box crop as a reference image so it reappears on next launch.
    private void SaveResultGemCrop(Rect box)
    {
        try
        {
            var shot = _gemScreenshot;
            if (shot == null) return;
            var w = (int)Math.Clamp(box.Width, 1, shot.PixelWidth);
            var h = (int)Math.Clamp(box.Height, 1, shot.PixelHeight);
            var x = (int)Math.Clamp(box.X, 0, Math.Max(0, shot.PixelWidth - w));
            var y = (int)Math.Clamp(box.Y, 0, Math.Max(0, shot.PixelHeight - h));
            var crop = new CroppedBitmap(shot, new Int32Rect(x, y, w, h));
            var encoder = new PngBitmapEncoder();
            encoder.Frames.Add(BitmapFrame.Create(crop));
            var path = CalibrationImagePath("calib_gem_result.png");
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            using var fs = File.Create(path);
            encoder.Save(fs);
        }
        catch { /* save must never break calibration */ }
    }

    private void GemSave()
    {
        if (_gemStep < GemTotalSteps || _gemResultBox == null)
        {
            _gemHint!.Text = "Not done yet — finish all buttons and the result-gem box first.";
            return;
        }

        // Grade buttons (N/G/DG/Register/Combine).
        var positions = new Dictionary<string, List<int>>();
        foreach (var key in CalibGemSteps)
        {
            var pt = _gemPoints[key];
            positions[key] = new List<int> { (int)pt.X, (int)pt.Y };
        }

        // Resource gems: 3 click points.
        var resources = new List<List<int>>();
        foreach (var key in CalibResourceSteps)
        {
            var pt = _gemPoints[key];
            resources.Add(new List<int> { (int)pt.X, (int)pt.Y });
        }

        // Result-gem area [x, y, width, height] (client-relative), used for OCR + centre click.
        var rb = _gemResultBox.Value;
        var resultArea = new List<int> { (int)rb.X, (int)rb.Y, (int)rb.Width, (int)rb.Height };
        SaveResultGemCrop(rb);

        // Empty-result colour reference. We don't decide here whether the box holds a gem —
        // empty detection is only as good as the reference, so we ASK the user to guarantee the
        // result box is EMPTY on screen right now. Only then do we sample the empty signature.
        // If they say the box still has a gem, we skip it entirely so empty-detection stays off
        // (rather than capture a polluted reference that always misreports "has gem").
        ColorComposition? emptySig = null;
        var confirmEmpty = MessageBox.Show(
            "Empty-check setup: make sure the RESULT box is currently EMPTY (no gem in it).\n\n" +
            "Click Yes only if the result box is truly empty — the empty colour is captured from it.\n" +
            "Click No if the box still has a gem (empty-check stays off until you re-save with it empty).",
            "Empty reference", MessageBoxButton.YesNo, MessageBoxImage.Question);
        if (confirmEmpty == MessageBoxResult.Yes)
        {
            // Sample the CALIBRATION SCREENSHOT (taken with the launcher hidden), not a live screen
            // grab. A live grab is CopyFromScreen, so whatever covers the box at that instant — the
            // launcher itself, a tooltip — becomes part of the "empty" reference. That is how the
            // stored signature ended up 0.17 (mean) / 0.53 (norm) away from the reference crop the
            // same save wrote, which made every later check meaningless.
            using var crop = CropScreenshot(rb);
            emptySig = crop == null ? null : GemColorAnalyzer.Analyze(crop, _service.Config.Gem.ColoredGapMin);
        }

        // Positions only — movements are saved separately by "Save Composer Moves", so don't
        // touch gem.Movements here.
        var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
        // Carry over existing machine-specific gem settings (Movements etc.) — don't clobber
        // them by rebuilding LocalGem from scratch.
        var gem = local.Gem ?? new ConfigLoader.LocalGem();
        gem.GradePositions = positions;
        gem.ResourceGems = resources;
        gem.ResultGemArea = resultArea;
        gem.EmptySignature = emptySig;
        gem.EmptyDistance = _service.Config.Gem.EmptyDistance;
        local.Gem = gem;
        if (_gemDisplay is { } d)
        {
            local.Calibration = ToCalibration(d);
            _service.Config.Calibration = local.Calibration;
        }
        // Which move set the composer uses is a preference, not a coordinate — it lives in the
        // portable config (defaults.yaml) like gem.start_grade/empty_mode. Both files are written
        // here, so a failure in either must stop before the "saved" dialog below claims success.
        _service.Config.Gem.MoveMode = _gemMoveMode?.SelectedItem as string ?? "tuned";
        if (!TrySaveCalibration(() => { _service.SaveLocal(local); _service.SaveConfig(); }, _gemHint!,
                "Gem Composer saved to config\\local.yaml."))
            return;
        // Refresh in-memory config so the next gem run uses the just-calibrated
        // click points rather than the startup (example-seeded) ones.
        _service.Config.Gem.GradePositions = positions;
        _service.Config.Gem.ResourceGems = resources;
        _service.Config.Gem.ResultGemArea = resultArea;
        _service.Config.Gem.EmptySignature = emptySig;
        SaveGemCalibrationImage("calib_gem.png");
        var emptyInfo = emptySig == null
            ? "\n\n(no empty colour reference captured — result box wasn't available)"
            : $"\n\nEmpty-result colour code: {DescribeColor(emptySig)}";
        _gemHint!.Text = "Gem Composer saved to config\\local.yaml.";
        RefreshGemCoords();
        MessageBox.Show("Gem calibration saved." + emptyInfo, "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
    }

    // Repopulate the coordinate boxes from the current config (e.g. after a capture-based save).
    private void RefreshGemCoords()
    {
        var inv = System.Globalization.CultureInfo.InvariantCulture;
        var gem = _service.Config.Gem;
        foreach (var row in _gemCoordRows)
        {
            if (row.Key == "ResultArea")
            {
                if (gem.ResultGemArea is { Count: 4 } a)
                {
                    row.X.Text = a[0].ToString(inv);
                    row.Y.Text = a[1].ToString(inv);
                    if (row.W != null) row.W.Text = a[2].ToString(inv);
                    if (row.H != null) row.H.Text = a[3].ToString(inv);
                }
            }
            else if (row.Key.StartsWith("Resource", StringComparison.Ordinal)
                     && int.TryParse(row.Key.AsSpan(8), out var ri) && ri >= 1 && gem.ResourceGems.Count >= ri)
            {
                var r = gem.ResourceGems[ri - 1];
                if (r.Count > 1)
                {
                    row.X.Text = r[0].ToString(inv);
                    row.Y.Text = r[1].ToString(inv);
                }
            }
            else if (gem.GradePositions.TryGetValue(row.Key, out var pos) && pos.Count > 1)
            {
                row.X.Text = pos[0].ToString(inv);
                row.Y.Text = pos[1].ToString(inv);
            }
        }
    }

    // Save the directly-edited coordinate boxes to local.yaml (no capture guard).
    private void SaveCoordinates()
    {
        var gem = _service.Config.Gem;
        foreach (var row in _gemCoordRows)
        {
            int x = int.TryParse(row.X.Text.Trim(), out var px) ? px : 0;
            int y = int.TryParse(row.Y.Text.Trim(), out var py) ? py : 0;
            if (row.Key == "ResultArea")
            {
                int w = row.W != null && int.TryParse(row.W.Text.Trim(), out var pw) ? pw : 0;
                int h = row.H != null && int.TryParse(row.H.Text.Trim(), out var ph) ? ph : 0;
                gem.ResultGemArea = new List<int> { x, y, w, h };
            }
            else if (row.Key.StartsWith("Resource", StringComparison.Ordinal)
                     && int.TryParse(row.Key.AsSpan(8), out var ri) && ri >= 1 && gem.ResourceGems.Count >= ri)
            {
                gem.ResourceGems[ri - 1] = new List<int> { x, y };
            }
            else
            {
                gem.GradePositions[row.Key] = new List<int> { x, y };
            }
        }

        var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
        var lg = local.Gem ?? new ConfigLoader.LocalGem();
        lg.GradePositions = gem.GradePositions;
        lg.ResourceGems = gem.ResourceGems;
        lg.ResultGemArea = gem.ResultGemArea;
        local.Gem = lg;
        TrySaveCalibration(() => _service.SaveLocal(local), _gemHint!,
            "Coordinates saved to config\\local.yaml.");
    }

    // Save ONLY the composer moves (raw dx/dy) from the move-editor boxes. Unlike GemSave, this
    // does NOT require the full click-point calibration — it just persists Movements to local.yaml.
    private void SaveComposerMoves()
    {
        foreach (var row in _gemMoveRows)
            SetGemMovement(row.Key, ParseMove(row.Dx), ParseMove(row.Dy));

        var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
        var gem = local.Gem ?? new ConfigLoader.LocalGem();
        gem.Movements = _service.Config.Gem.Movements;
        local.Gem = gem;
        TrySaveCalibration(() => _service.SaveLocal(local), _gemHint!,
            "Composer moves saved to config\\local.yaml.");
    }

    // Diagnostic: move the cursor to the SELECTED point using one specific API, so the two paths can
    // be compared directly on the same target.
    //   physical = SetPhysicalCursorPos(physical client origin + offset)          — no conversion
    //   logical  = SetCursorPos(logical client origin + offset / scale)           — v1's call
    // Reports the computed coordinates, whether the API accepted them, and where the cursor ended up.
    private void DebugCursorPath(ComboBox? box, bool physical)
    {
        if (box?.SelectedItem is not string name) { _gemHint!.Text = "Pick a point first."; return; }
        var pt = ResolveGemPoint(name);
        if (pt == null) { _gemHint!.Text = $"\"{name}\" isn't calibrated yet."; return; }

        var display = HidPointer.Display(_service.Config.Window.Title);
        if (display == null) { _gemHint!.Text = "Game window not found (or minimized) — open and restore the game first."; return; }

        int x = (int)pt.Value.X, y = (int)pt.Value.Y;

        // Calculation first (pure), then the API call — the same split the tools use.
        var target = WindowFinder.ComputeCursorTarget(display, x, y);
        bool ok = physical
            ? WindowFinder.SetPhysicalCursorPosition(target)
            : WindowFinder.SetLogicalCursorPosition(target);

        var after = WindowFinder.LogicalCursorPosition();
        var afterText = after is { } a ? $"({a.X},{a.Y})" : "?";
        _gemHint!.Text =
            $"{name}: offset ({x},{y}), scale {display.Scale:0.###}\n" +
            $"  computed logical  = ({target.LogicalX},{target.LogicalY})\n" +
            $"  computed physical = ({target.PhysicalX},{target.PhysicalY})\n" +
            $"  called {(physical ? "SetPhysicalCursorPos" : "SetCursorPos")} → ok={ok}; cursor now {afterText}.";
    }

    // Diagnostic: report the window rects and save a sample capture, so "is the game being captured
    // correctly, and does the launcher cover it?" can be checked without guessing. Also shows the
    // non-client offset (frame vs client), which is what a whole-window capture would have been
    // shifted by — the capture path itself is client-origin CopyFromScreen.
    private async void DiagnoseCapture()
    {
        var hwnd = WindowFinder.FindByTitle(_service.Config.Window.Title);
        if (hwnd == IntPtr.Zero)
        {
            _gemHint!.Text = "Game window not found (or minimized) — open and restore the game first.";
            return;
        }

        var client = WindowFinder.GetClientRectInScreen(hwnd);
        var frame = WindowFinder.GetFrameRect(hwnd);
        if (client == null || frame == null)
        {
            _gemHint!.Text = "Could not read the window rects.";
            return;
        }

        int border = client.Left - frame.Left;
        int title = client.Top - frame.Top;
        try
        {
            var dir = LogPath("captures");
            Directory.CreateDirectory(dir);
            await WithLauncherHiddenAsync(() =>
            {
                using var shot = ScreenCapture.CaptureScreen(client);
                if (shot == null)
                    throw new InvalidOperationException(
                        "the screen grab returned nothing (locked session, or a client area with no size)");
                shot.ImWrite(Path.Combine(dir, "diag_capture.png"));
                return true;
            });

            _gemHint!.Text =
                $"frame=({frame.Left},{frame.Top}) {frame.Width}x{frame.Height}   " +
                $"client=({client.Left},{client.Top}) {client.Width}x{client.Height}   " +
                $"non-client offset=({border},{title})\n" +
                "Saved logs\\captures\\diag_capture.png (launcher hidden for the grab).";
        }
        catch (Exception ex)
        {
            _gemHint!.Text = "Capture diagnostic failed: " + ex.Message;
        }
    }

    // ── Shared calibrator helpers ───────────────────────────────────────────

    // CopyFromScreen reads the actual screen, so anything covering the game — this launcher window
    // in particular — ends up in the capture. Hide the launcher for the grab, then restore it.
    private async Task<T> WithLauncherHiddenAsync<T>(Func<T> capture)
    {
        var wasVisible = Visibility == Visibility.Visible;
        if (wasVisible)
        {
            Visibility = Visibility.Hidden;
            await Task.Delay(300); // let the compositor take the window off the screen
        }
        try
        {
            return capture();
        }
        finally
        {
            if (wasVisible) Visibility = Visibility.Visible;
        }
    }

    // Physical-pixel capture of the whole game client, plus the display measurement taken in the
    // same peek. Null when the game window isn't open.
    private async Task<(BitmapSource Image, DisplayInfo Display)?> CaptureScreenshotAsync()
    {
        var cap = await WithLauncherHiddenAsync(() =>
        {
            var hwnd = WindowFinder.FindByTitle(_service.Config.Window.Title);
            return ScreenCapture.CaptureClient(hwnd);
        });
        if (cap == null) return null;

        using (cap.Image) return (MatToBitmapSource(cap.Image), cap.Display);
    }

    private static Point CanvasToNatural(Point p, BitmapSource screenshot, Canvas canvas)
    {
        var w = screenshot.PixelWidth;
        var h = screenshot.PixelHeight;
        var availW = canvas.ActualWidth;
        var availH = canvas.ActualHeight;
        if (w == 0 || h == 0 || availW <= 0 || availH <= 0) return p;

        var scale = Math.Min(availW / w, availH / h);
        var dispW = w * scale;
        var dispH = h * scale;
        var offX = (availW - dispW) / 2;
        var offY = (availH - dispH) / 2;
        return new Point((p.X - offX) / scale, (p.Y - offY) / scale);
    }

    /// <summary>The inverse of <see cref="CanvasToNatural"/>: an image (natural) pixel to where the
    /// canvas shows it. Needed to draw a marker AT a computed coordinate rather than read one back
    /// from a click — the bag-grid overlay is the one place a coordinate travels that way.</summary>
    private static Point NaturalToCanvas(Point p, BitmapSource screenshot, Canvas canvas)
    {
        var w = screenshot.PixelWidth;
        var h = screenshot.PixelHeight;
        var availW = canvas.ActualWidth;
        var availH = canvas.ActualHeight;
        if (w == 0 || h == 0 || availW <= 0 || availH <= 0) return p;

        var scale = Math.Min(availW / w, availH / h);
        return new Point(p.X * scale + (availW - w * scale) / 2,
                         p.Y * scale + (availH - h * scale) / 2);
    }

    private static void AddDot(Canvas canvas, Point p)
    {
        var dot = new Ellipse
        {
            Width = 10,
            Height = 10,
            Fill = Brushes.Yellow,
            Stroke = Brushes.White,
            StrokeThickness = 1,
        };
        Canvas.SetLeft(dot, p.X - 5);
        Canvas.SetTop(dot, p.Y - 5);
        canvas.Children.Add(dot);
    }

    /// <summary>BitmapSource -> Mat. The reverse of <see cref="MatToBitmapSource"/>, which is all the
    /// launcher needed until the empty-pet-slot reference wanted a crop of what the calibrator is
    /// showing. Pixels are copied through a byte array because the two types share no memory model.
    /// </summary>
    private static Mat BitmapSourceToMat(BitmapSource source)
    {
        var bgr = source.Format == PixelFormats.Bgr24
            ? source
            : new FormatConvertedBitmap(source, PixelFormats.Bgr24, null, 0);

        int stride = bgr.PixelWidth * 3;
        var bytes = new byte[stride * bgr.PixelHeight];
        bgr.CopyPixels(bytes, stride, 0);

        var mat = new Mat(bgr.PixelHeight, bgr.PixelWidth, OpenCvSharp.MatType.CV_8UC3);
        System.Runtime.InteropServices.Marshal.Copy(bytes, 0, mat.Data, bytes.Length);
        return mat;
    }

    private static BitmapSource MatToBitmapSource(Mat mat)
    {
        var width = mat.Width;
        var height = mat.Height;
        var srcStride = (int)mat.Step();
        var dstStride = width * 3;
        var data = new byte[height * dstStride];
        var src = mat.Data;
        for (var y = 0; y < height; y++)
        {
            Marshal.Copy(IntPtr.Add(src, y * srcStride), data, y * dstStride, dstStride);
        }

        var source = BitmapSource.Create(width, height, 96, 96, PixelFormats.Bgr24, null, data, dstStride);
        source.Freeze();
        return source;
    }

    // ── Helpers ─────────────────────────────────────────────────────────────

    private TabItem BuildHotkeysTab()
    {
        // No horizontal margin: the cards should line up with the tab strip's left border, not sit
        // 8px inside it. Vertical margin keeps a little breathing room at the top.
        var panel = new StackPanel { Margin = new Thickness(0, 8, 0, 8) };

        var startBox = UiText(VkName(_service.Config.Hotkeys.Start));
        var quitBox = UiText(VkName(_service.Config.Hotkeys.Quit));
        var gradeBox = UiText(VkName(_service.Config.Hotkeys.AdvanceGrade));
        var pauseBox = UiText(VkName(_service.Config.Hotkeys.Pause));

        var fields = new StackPanel();
        fields.Children.Add(LabeledField("Start / stop rolling", startBox));
        fields.Children.Add(LabeledField("Quit (immediate)", quitBox));
        fields.Children.Add(LabeledField("Advance grade (gem)", gradeBox));
        fields.Children.Add(LabeledField("Pause (graceful stop)", pauseBox));

        // Card (bordered content), not CardControl: CardControl's template measures its content with
        // unbounded width, so the hint paragraph could not wrap and was clipped at the border.
        var cardBody = new StackPanel();
        cardBody.Children.Add(new TextBlock
        {
            Text = "Keys",
            FontWeight = FontWeights.SemiBold,
            Margin = new Thickness(0, 0, 0, 8),
        });
        cardBody.Children.Add(Hint(
            "Type a key name: F1–F24, Esc, CapsLock, Space, Tab, Enter, or a single letter/digit.\n" +
            "They only reach the tool while the LAUNCHER has focus — the game's anti-cheat blocks " +
            "background key reads. Click the launcher first, then press the key."));
        cardBody.Children.Add(fields);
        panel.Children.Add(new Card { Content = cardBody });

        // Result of a save shown inline instead of a modal "Saved." dialog.
        var result = new InfoBar { IsOpen = false, IsClosable = true, Margin = new Thickness(0, 12, 0, 0) };

        var save = MakeButton("Save Hotkeys", ControlAppearance.Primary);
        save.Margin = new Thickness(0, 12, 0, 0);
        save.Click += (_, _) =>
        {
            var s = ParseVk(startBox.Text);
            var q = ParseVk(quitBox.Text);
            var g = ParseVk(gradeBox.Text);
            var p = ParseVk(pauseBox.Text);
            if (s == 0 || q == 0 || g == 0 || p == 0)
            {
                result.Severity = InfoBarSeverity.Error;
                result.Title = "Invalid hotkey";
                result.Message = "One of the names isn't a key. Use F1–F24, Esc, CapsLock, Space, Tab, Enter, or a single letter/digit.";
                result.IsOpen = true;
                return;
            }
            _service.Config.Hotkeys.Start = s;
            _service.Config.Hotkeys.Quit = q;
            _service.Config.Hotkeys.AdvanceGrade = g;
            _service.Config.Hotkeys.Pause = p;
            SaveReport(() => _service.SaveConfig(), result, "Hotkeys written to defaults.yaml.");
        };
        panel.Children.Add(save);
        panel.Children.Add(result);

        return MakeTab("Hotkeys", panel);
    }

    private static int ParseVk(string name)
    {
        var n = name.Trim();
        if (n.StartsWith("F", StringComparison.OrdinalIgnoreCase) && int.TryParse(n.AsSpan(1), System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out var f) && f is >= 1 and <= 24)
        {
            return 0x70 + f - 1;
        }

        return n.ToUpperInvariant() switch
        {
            "ESC" or "ESCAPE" => 0x1B,
            "CAPSLOCK" or "CAPS" => 0x14,
            "SPACE" => 0x20,
            "TAB" => 0x09,
            "ENTER" or "RETURN" => 0x0D,
            "BACKSPACE" => 0x08,
            _ when n.Length == 1 => char.ToUpperInvariant(n[0]),
            _ => 0,
        };
    }

    private static string VkName(int vk)
    {
        if (vk is >= 0x70 and <= 0x87) return $"F{vk - 0x70 + 1}";
        return vk switch
        {
            0x1B => "Esc",
            0x14 => "CapsLock",
            0x20 => "Space",
            0x09 => "Tab",
            0x0D => "Enter",
            0x08 => "Backspace",
            _ when vk is >= 0x21 and <= 0x7E => ((char)vk).ToString(),
            _ => $"0x{vk:X}",
        };
    }

    // Label column width: wide enough that the longest label in the app ("Scale (physical px per
    // logical px)") sits on one line — at 180 it wrapped, which looked accidental.
    private const double LabelColumnWidth = 230;

    private Grid LabeledField(string label, FrameworkElement control)
    {
        var grid = new Grid { Margin = new Thickness(0, 4, 0, 4) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(LabelColumnWidth) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });

        var labelText = new TextBlock
        {
            Text = label,
            Foreground = Res("TextFillColorSecondaryBrush"),
            VerticalAlignment = VerticalAlignment.Center,
            TextWrapping = TextWrapping.Wrap,
        };
        Grid.SetColumn(labelText, 0);
        Grid.SetColumn(control, 1);
        grid.Children.Add(labelText);
        grid.Children.Add(control);
        return grid;
    }

    private static ComboBox MakeComboBox(IEnumerable<string> items, string? selected)
    {
        var combo = new ComboBox();
        foreach (var item in items)
        {
            combo.Items.Add(item);
        }
        combo.SelectedItem = selected;
        return combo;
    }

    // "None" means no grade floor; the code also treats null/"false" as unset.
    private static string GradeOrNone(string? g) =>
        string.IsNullOrWhiteSpace(g) || g.Equals("false", StringComparison.OrdinalIgnoreCase) ? "None" : g;

    /// <summary>The calibrator tabs' equivalent of <see cref="SaveReport"/>: they report through a
    /// hint line and a confirmation dialog rather than an InfoBar, and the dialog used to be raised
    /// after the write — so a failed save both left the click looking inert and, had it got that far,
    /// claimed success. False means the write failed and the caller must not announce success.</summary>
    private static bool TrySaveCalibration(Action save, TextBlock hint, string successMessage)
    {
        try
        {
            save();
            hint.Text = successMessage;
            return true;
        }
        catch (Exception ex)
        {
            hint.Text = $"Save failed: {ex.Message} — nothing was written.";
            return false;
        }
    }

    /// <summary>Runs a config save and reports the outcome on a tab's InfoBar. SaveLocal/SaveDefaults
    /// write files and throw on I/O failure, and the success bar used to be raised after the call —
    /// so a read-only config directory or a full disk made Save look like it did nothing at all,
    /// with the exception absorbed by the app-level handler and only a log line to show for it.</summary>
    private static void SaveReport(Action save, InfoBar bar, string successMessage)
    {
        try
        {
            save();
            bar.Severity = InfoBarSeverity.Success;
            bar.Title = "Saved";
            bar.Message = successMessage;
        }
        catch (Exception ex)
        {
            bar.Severity = InfoBarSeverity.Error;
            bar.Title = "Save failed";
            bar.Message = $"{ex.Message} — nothing was written.";
        }
        bar.IsOpen = true;
    }

    /// <summary>Every config tab is the same shell — a header and a vertically-scrolling panel. It was
    /// copied verbatim into nine Build*Tab methods, which is why the horizontal-scroll fix below had
    /// to be made nine times.</summary>
    /// <summary>The Sell tab. All this needs is WHICH slots — the geometry is the calibrated grid and
    /// the shared transaction, so there is nothing else to configure here.</summary>
    private TabItem BuildSellTab()
    {
        var panel = new StackPanel();
        var hint = Mono();
        _sellHint = hint;

        // The per-row buttons are Sell's own — see MakeBagPicker.
        var (slots, sellCells) = MakeBagPicker(ToggleSellSlot, ToggleSellRow);
        _sellSlotBoxes.Clear();
        foreach (var (index, cell) in sellCells) _sellSlotBoxes[index] = cell;

        var all = MakeButton("Select all", ControlAppearance.Secondary);
        all.Click += (_, _) => { _service.Config.BuySell.SellSlots = Enumerable.Range(0, BagGrid.SlotCount).ToList(); PaintSellSlots(); };
        var none = MakeButton("Clear", ControlAppearance.Secondary);
        none.Click += (_, _) => { _service.Config.BuySell.SellSlots.Clear(); PaintSellSlots(); };

        var cap = UiText(_service.Config.BuySell.SellCap.ToString(CultureInfo.InvariantCulture));
        cap.Width = 60;
        var saveCap = MakeInlineButton("Save", ControlAppearance.Secondary);
        saveCap.Click += (_, _) =>
        {
            if (!int.TryParse(cap.Text.Trim(), out var n) || n < 1)
            {
                hint.Text = "The cap must be a positive number.";
                return;
            }
            _service.Config.BuySell.SellCap = n;
            SaveBuySell(hint, "Cap saved to config\\local.yaml. (The slot selection is never saved — " +
                "it is chosen fresh each time, on purpose.)");
        };

        var capRow = new StackPanel { Orientation = Orientation.Horizontal };
        capRow.Children.Add(cap);
        capRow.Children.Add(saveCap);

        var sellDry = MakeButton("Dry run", ControlAppearance.Secondary);
        sellDry.Click += async (_, _) => await SellDryRun();

        var pickRow = new StackPanel { Orientation = Orientation.Horizontal };
        pickRow.Children.Add(all);
        pickRow.Children.Add(none);
        pickRow.Children.Add(sellDry);

        panel.Children.Add(Section("Slots to sell",
            Hint("Click the slots to sell. The grid mirrors the bag — slot 1 top-left, slot 64 " +
                 "bottom-right — so lighting them up shows exactly what is about to go. Selling runs " +
                 "from the highest slot number down, which stays correct whether or not the bag " +
                 "closes the gap after each sale."),
            pickRow));

        panel.Children.Add(Section("Grid", slots));

        panel.Children.Add(Section("Safety",
            Hint("A hard ceiling on how many slots one run may sell. It is enforced in the loop, not " +
                 "advice — selling is the one thing here that cannot be undone."),
            LabeledField("Max slots per run", capRow)));

        panel.Children.Add(Section("Status", hint));

        PaintSellSlots();
        return MakeTab("Sell", panel);
    }

    /// <summary>The Buy tab: pick one of your named items, set how many, and the card's Start runs it.
    /// Presets live here rather than in the calibrator because the geometry — rows, scroll point, MAX
    /// — is shared by every item, while only the row and the scroll distance differ between them.</summary>
    private TabItem BuildBuyTab()
    {
        var panel = new StackPanel();
        var hint = Mono();
        hint.Text = "Calibrate Buy / Sell first, then add the items you re-buy here.";
        _buyHint = hint;

        _buyPreset = new ComboBox { MinWidth = 160, VerticalAlignment = VerticalAlignment.Center };
        var reload = MakeInlineButton("Reload", ControlAppearance.Secondary);
        reload.Click += (_, _) => RefreshBuyPresets();
        var pickRow = new StackPanel { Orientation = Orientation.Horizontal };
        pickRow.Children.Add(_buyPreset);
        pickRow.Children.Add(reload);

        var nameBox = UiText("", "e.g. Springs");
        _buyRowBox = BuyRowPicker();
        var rowBox = _buyRowBox;
        var scrollBox = UiText("0");
        var countBox = UiText("1");
        foreach (var b in new[] { nameBox, scrollBox, countBox }) b.Width = 90;
        nameBox.Width = 160;

        var save = MakeButton("Save item", ControlAppearance.Primary);
        save.Click += (_, _) => SaveBuyPreset(nameBox, rowBox, scrollBox, countBox);
        var del = MakeButton("Delete item", ControlAppearance.Secondary);
        del.Click += (_, _) => DeleteBuyPreset();
        var dryRun = MakeButton("Dry run", ControlAppearance.Secondary);
        dryRun.Click += async (_, _) => await BuyDryRun();

        var actRow = new StackPanel { Orientation = Orientation.Horizontal };
        actRow.Children.Add(save);
        actRow.Children.Add(del);
        actRow.Children.Add(dryRun);

        _buyPreset.SelectionChanged += (_, _) =>
        {
            OnBuyPresetChanged(fromCard: false);
            if (_buyPreset.SelectedItem is not string name) return;
            if (!_service.Config.BuySell.Presets.TryGetValue(name, out var p)) return;
            nameBox.Text = name;
            SyncBuyRowPicker(rowBox, p.Row);
            scrollBox.Text = p.Scroll.ToString(CultureInfo.InvariantCulture);
            countBox.Text = p.Count.ToString(CultureInfo.InvariantCulture);
        };

        var fields = new StackPanel();
        fields.Children.Add(LabeledField("Name", nameBox));
        fields.Children.Add(LabeledField("Row", rowBox));
        fields.Children.Add(LabeledField("Scroll notches", scrollBox));
        fields.Children.Add(LabeledField("Usual count", countBox));

        panel.Children.Add(Section("Item",
            Hint("Pick the item the Buy card's Start will buy. The dropdown is what the tool reads — " +
                 "editing the fields below does nothing until you save them onto a name."),
            LabeledField("Preset", pickRow)));

        panel.Children.Add(Section("Position",
            Hint("Row 1 is the top visible row after scrolling; the list drops down from there. Scroll " +
                 "notches are wheel-downs FROM THE TOP of the list, so the number only means anything " +
                 "while the list is actually at the top — scroll it there yourself before a run or a dry " +
                 "run; the tool does not do it for you. Set the number by trial with Dry run below: it " +
                 "scrolls and moves the cursor but never clicks, so a wrong guess costs nothing."),
            fields,
            actRow));

        panel.Children.Add(Section("Status", hint));

        RefreshBuyPresets();
        return MakeTab("Buy", panel);
    }

    /// <summary>The Buy tab's row field, as a picker rather than a text box. It used to be a free-text
    /// 0-based index with "0 = top row" in the label, which is invisible at the point of use: a 1 there
    /// bought the second item down, and nothing on screen said so. A labelled entry cannot be off by
    /// one, because there is no number to get wrong — and the value stored is still the 0-based index
    /// the geometry works in.
    ///
    /// Its own helper rather than an inline ComboBox so the one place that knows how a row becomes a
    /// label is also the one place that turns it back.</summary>
    private static ComboBox BuyRowPicker() => new()
    {
        Width = 160,
        VerticalAlignment = VerticalAlignment.Center,
    };

    /// <summary>Points the row picker at <paramref name="row"/> (0-based), rebuilding its entries from
    /// the configured visible-row count first — the count is a config value, so a picker built once at
    /// startup could offer a row this shop's list does not have.
    ///
    /// A preset saved while the count was larger is still representable: its row gets an entry of its
    /// own, marked rather than dropped. Dropping it would blank the field, and the next Save would
    /// write the wrong row — trading a clear message for a silent one. Selecting that entry leaves
    /// <c>SelectedIndex</c> past the bottom of the list, which is exactly what the save-time check
    /// reports.</summary>
    private void SyncBuyRowPicker(ComboBox picker, int row)
    {
        var rows = Math.Max(1, _service.Config.BuySell.ShopRows);
        var items = new List<string>(rows + 1);
        for (int i = 0; i < rows; i++) items.Add($"Row {i + 1}");
        if (row >= rows) items.Add($"Row {row + 1} — past the bottom of the list");
        picker.ItemsSource = items;
        picker.SelectedIndex = row >= rows ? rows : Math.Max(0, row);
    }

    /// <summary>Repopulates both preset pickers and leaves them on the same item. There are two
    /// because the choice genuinely belongs in both places — on the tab, where the items are defined,
    /// and on the card, where a run is started — but it is ONE choice, so they mirror rather than
    /// drift.</summary>
    private void RefreshBuyPresets()
    {
        var names = _service.Config.BuySell.Presets.Keys
            .OrderBy(k => k, StringComparer.OrdinalIgnoreCase).ToList();
        if (_activeBuyPreset == null || !names.Contains(_activeBuyPreset))
            _activeBuyPreset = names.FirstOrDefault();

        _syncingBuy = true;
        try
        {
            if (_buyPreset != null) { _buyPreset.ItemsSource = names; _buyPreset.SelectedItem = _activeBuyPreset; }
            if (_buyPresetCard != null) { _buyPresetCard.ItemsSource = names; _buyPresetCard.SelectedItem = _activeBuyPreset; }
        }
        finally { _syncingBuy = false; }

        SyncBuyCountFromPreset();
        // Filled here as well as from the picker's own SelectionChanged, because that event does not
        // fire when nothing is selected — and "nothing selected" is exactly the fresh install that has
        // to pick a row to create its first item.
        if (_buyRowBox is { } picker)
        {
            var row = _activeBuyPreset != null &&
                      _service.Config.BuySell.Presets.TryGetValue(_activeBuyPreset, out var active)
                ? active.Row
                // No preset yet, so there is no stored row to show. Top row is the default the old
                // text box started on, and the picker has to start somewhere to be usable at all.
                : 0;
            SyncBuyRowPicker(picker, row);
        }
    }

    private void OnBuyPresetChanged(bool fromCard)
    {
        if (_syncingBuy) return;

        var chosen = (fromCard ? _buyPresetCard?.SelectedItem : _buyPreset?.SelectedItem) as string;
        _activeBuyPreset = chosen;

        _syncingBuy = true;
        try
        {
            if (fromCard && _buyPreset != null) _buyPreset.SelectedItem = chosen;
            if (!fromCard && _buyPresetCard != null) _buyPresetCard.SelectedItem = chosen;
        }
        finally { _syncingBuy = false; }

        SyncBuyCountFromPreset();
    }

    /// <summary>Seeds the run count from the preset's own count — its usual amount — which the card
    /// then lets you change for this run without editing the item.</summary>
    private void SyncBuyCountFromPreset()
    {
        if (_buyCountCard == null || _activeBuyPreset == null) return;
        if (!_service.Config.BuySell.Presets.TryGetValue(_activeBuyPreset, out var p)) return;
        _buyCountCard.Text = p.Count.ToString(CultureInfo.InvariantCulture);
        _service.PendingBuyCount = p.Count;
    }

    private void BumpBuyCount(int delta)
    {
        if (_buyCountCard == null) return;
        var n = Math.Max(1, BuyRunCountFromCard() + delta);
        _buyCountCard.Text = n.ToString(CultureInfo.InvariantCulture);
    }

    /// <summary>The count as the card currently reads it. Free text with +/- beside it, so anything
    /// unparseable falls back to 1 rather than refusing to start.</summary>
    private int BuyRunCountFromCard()
        => _buyCountCard != null && int.TryParse(_buyCountCard.Text.Trim(), out var n) && n >= 1 ? n : 1;

    private void SaveBuyPreset(Wpf.Ui.Controls.TextBox name, ComboBox row,
                               Wpf.Ui.Controls.TextBox scroll, Wpf.Ui.Controls.TextBox count)
    {
        var key = name.Text.Trim();
        if (key.Length == 0) { _buyHint!.Text = "Give the item a name first."; return; }
        // The picker's index IS the 0-based row the geometry wants, so there is no number to parse
        // and no off-by-one to make — see BuyRowPicker.
        if (row.SelectedIndex < 0) { _buyHint!.Text = "Pick the row this item sits on."; return; }
        var r = row.SelectedIndex;
        if (!int.TryParse(scroll.Text.Trim(), out var sc) || sc < 0) { _buyHint!.Text = "Scroll notches must be 0 or more."; return; }
        // Against the SAME constant the firmware is built from, not a copy. This said "30" for
        // several cap changes after the firmware moved on, which is the whole reason the number is
        // interpolated everywhere else.
        if (sc > WheelMaxNotches)
        {
            _buyHint!.Text = $"The firmware caps one scroll at {WheelMaxNotches} notches.";
            return;
        }
        if (!int.TryParse(count.Text.Trim(), out var n) || n < 1) { _buyHint!.Text = "Buy count must be 1 or more."; return; }
        if (!ShopRowOk(r)) return;

        _service.Config.BuySell.Presets[key] = new BuyPreset { Row = r, Scroll = sc, Count = n };
        RefreshBuyPresets();
        _buyPreset!.SelectedItem = key;
        SaveBuySell(_buyHint!, $"Item '{key}' saved (row {r + 1}, scroll {sc}, count {n}).");
    }

    private void DeleteBuyPreset()
    {
        if (_buyPreset?.SelectedItem is not string key) { _buyHint!.Text = "Pick an item to delete."; return; }
        if (!_service.Config.BuySell.Presets.Remove(key)) { _buyHint!.Text = $"'{key}' isn't there any more."; return; }
        RefreshBuyPresets();
        SaveBuySell(_buyHint!, $"Item '{key}' deleted.");
    }

    /// <summary>Warns when a row index would land outside the calibrated list, rather than letting the
    /// run click past the bottom of it.</summary>
    private bool ShopRowOk(int row)
    {
        var bs = _service.Config.BuySell;
        if (ShopGeometry.Problem(bs.ShopRegion, bs.ShopRows) is { } problem)
        {
            _buyHint!.Text = "Shop rows aren't calibrated — " + problem + ".";
            return false;
        }
        // A row past the bottom of the visible list would click whatever is underneath the shop
        // window, which is not a thing anyone wants a buying tool to do.
        if (row >= bs.ShopRows)
        {
            // Numbered as the picker numbers it, not as the index it stores: this message is read
            // next to a control that says "Row 4", and "Row 3" there would be the off-by-one again.
            _buyHint!.Text = $"Row {row + 1} is past the bottom of the list — only {bs.ShopRows} rows " +
                "are visible at once. Scroll further and pick a lower row.";
            return false;
        }
        return true;
    }

    /// <summary>Persists the whole buy/sell section — geometry and presets together, so a save from
    /// any of the three tabs can't drop what another one set.</summary>
    private void SaveBuySell(TextBlock hint, string success)
    {
        var bs = _service.Config.BuySell;
        var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
        local.BuySell = new ConfigLoader.LocalBuySell
        {
            BagGrid = bs.BagGrid,
            BagSlot = bs.BagSlot,
            ShopRows = bs.ShopRows,
            ShopRegion = bs.ShopRegion,
            ScrollPoint = bs.ScrollPoint,
            MaxButton = bs.MaxButton,
            Presets = bs.Presets,
            SellCap = bs.SellCap,
        };
        TrySaveCalibration(() => _service.SaveLocal(local), hint, success);
    }

    /// <summary>Where a run would put the cursor, without clicking. The buy dry run's whole point:
    /// it scrolls and parks the pointer so the stored scroll amount can be judged by eye and adjusted
    /// — which is the only way to set it, since a scroll position can't be measured after the fact.</summary>
    private bool TryPlace(SerialPort ser, int x, int y, out string error)
    {
        error = "";
        var display = HidPointer.Display(_service.Config.Window.Title);
        if (display == null) { error = "Game window not found (or minimized)."; return false; }
        var placed = HidPointer.To(ser, WindowFinder.ComputeCursorTarget(display, x, y));
        if (!placed.Ok) { error = placed.Error ?? $"couldn't place the cursor at ({x},{y})"; return false; }
        return true;
    }

    /// <summary>How long the firmware needs to walk a scroll out: one gap per notch, plus slack for
    /// the serial round trip. Waiting less means reading a list that is still moving — which shows up
    /// as a cursor placed on the wrong row, not as an obvious error.</summary>
    private static int ScrollSettleMs(int notches) => notches * 30 + 400;

    private async Task BuyDryRun()
    {
        var bs = _service.Config.BuySell;
        if (_buyPreset?.SelectedItem is not string name || !bs.Presets.TryGetValue(name, out var preset))
        {
            _buyHint!.Text = "Pick an item first.";
            return;
        }
        if (ShopGeometry.Problem(bs.ShopRegion, bs.ShopRows) is { } problem)
        {
            _buyHint!.Text = "Shop rows aren't calibrated — " + problem + ".";
            return;
        }
        if (bs.ScrollPoint is not { Count: 2 } scroll)
        {
            _buyHint!.Text = "The scroll point isn't calibrated.";
            return;
        }

        var ser = await _service.ArduinoPortAsync();
        if (ser == null) { _buyHint!.Text = _service.LastArduinoError ?? "Arduino not found."; return; }
        // Click the scroll point FIRST. The dry run is a calibration action — you have just been
        // clicking this window — so the game is unfocused and the wheel goes nowhere. A real run
        // needs no such click: there the mouse is already in the game.
        if (!TryPlace(ser, scroll[0], scroll[1], out var err)) { _buyHint!.Text = "Dry run stopped: " + err; return; }
        HidPointer.Click(ser);
        await Task.Delay(500);

        try
        {
            // Same origin the real run uses: wheel-up to the maximum and let it clamp, then down by
            // the stored amount. If this lands wrong, so would the run.
            // No scroll-to-top. The list is expected to already BE at the top, which is your setup
            // rather than the tool's job — scrolling it there would be the length of the whole list
            // on every attempt. The wait is the firmware's walking time rather than a guess: it sends
            // the notches out one at a time with a gap, and reading mid-scroll puts the cursor on a
            // stale row.
            if (preset.Scroll > 0)
            {
                ser.Write($"Z {preset.Scroll}\n");
                await Task.Delay(ScrollSettleMs(preset.Scroll));
            }
        }
        catch (Exception ex) { _buyHint!.Text = "Scroll failed: " + ex.Message; return; }

        var row = ShopGeometry.RowCentre(bs.ShopRegion, bs.ShopRows, preset.Row);
        if (row is not { } target) { _buyHint!.Text = "Couldn't work out the row."; return; }
        if (!TryPlace(ser, target.X, target.Y, out err)) { _buyHint!.Text = "Dry run stopped: " + err; return; }

        _buyHint!.Text = $"Dry run: scrolled {preset.Scroll} notch(es) down from where the list was and " +
                         $"parked the cursor on row {preset.Row}. Nothing was clicked. It assumes the " +
                         "list is already at the TOP — scroll it there yourself first. If the pointer " +
                         $"isn't on '{name}', change the scroll amount and run this again.";
    }

    private async Task SellDryRun()
    {
        var bs = _service.Config.BuySell;
        if (!BagGrid.IsValidRect(bs.BagGrid)) { _sellHint!.Text = "Calibrate the bag grid first."; return; }
        if (bs.SellSlots.Count == 0) { _sellHint!.Text = "Click some slots first."; return; }

        var ser = await _service.ArduinoPortAsync();
        if (ser == null) { _sellHint!.Text = _service.LastArduinoError ?? "Arduino not found."; return; }

        // Same descending order the real run uses, so the dry run shows the actual sequence.
        var centres = BagGrid.Centres(bs.BagGrid!);
        var ordered = bs.SellSlots.Where(i => i >= 0 && i < centres.Count)
                                  .Distinct().OrderByDescending(i => i).ToList();

        for (int n = 0; n < ordered.Count; n++)
        {
            var c = centres[ordered[n]];
            if (!TryPlace(ser, c.X, c.Y, out var err)) { _sellHint!.Text = "Dry run stopped: " + err; return; }
            _sellHint!.Text = $"Would sell slot {ordered[n] + 1} ({n + 1}/{ordered.Count}) — no click.";
            await Task.Delay(220);
        }

        _sellHint!.Text = $"Dry run done: {ordered.Count} slot(s) would be sold, highest number first. " +
                          "Nothing was clicked.";
    }

    /// <summary>Selects or clears a whole bag row: all eight if any are off, none if all are on. One
    /// click instead of eight, and it can't leave a row half-selected by mistake.</summary>
    private void ToggleSellRow(int row)
    {
        var slots = _service.Config.BuySell.SellSlots;
        var indices = Enumerable.Range(row * BagGrid.Cols, BagGrid.Cols).ToList();
        bool allOn = indices.All(slots.Contains);

        foreach (var i in indices)
        {
            if (allOn) slots.Remove(i);
            else if (!slots.Contains(i)) slots.Add(i);
        }
        PaintSellSlots();
    }

    /// <summary>The 8x8 bag picker, shared by the Sell screen and the Pet screen.
    ///
    /// They want the same WIDGET over different selections — Sell's is the per-run list of what to
    /// sell, Pet's is where the food and the pet live — so the grid is built once here and told what
    /// a click means. The selection itself stays with the caller, which is the point: sharing the
    /// widget must not share the choice, or un-marking a food cell would become a sale.
    ///
    /// <paramref name="onRow"/> adds the per-row shortcut column, and only selling passes it: "all
    /// eight slots of row 3" is a meaningful sale and is not a meaningful thing to mark as food.</summary>
    private static (Grid Grid, Dictionary<int, Border> Cells) MakeBagPicker(
        Action<int> onClick, Action<int>? onRow = null)
    {
        var grid = new Grid { Margin = new Thickness(0, 8, 0, 8), HorizontalAlignment = HorizontalAlignment.Left };
        for (int c = 0; c < BagGrid.Cols; c++)
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(34) });
        for (int r = 0; r < BagGrid.Rows; r++)
            grid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(34) });

        if (onRow != null)
        {
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            for (int r = 0; r < BagGrid.Rows; r++)
            {
                int row = r;
                var rowButton = MakeRowButton($"row {r + 1}");
                rowButton.ToolTip = $"Select or clear all 8 slots of bag row {r + 1}";
                rowButton.Click += (_, _) => onRow(row);
                Grid.SetRow(rowButton, r);
                Grid.SetColumn(rowButton, BagGrid.Cols);
                grid.Children.Add(rowButton);
            }
        }

        var cells = new Dictionary<int, Border>();
        for (int i = 0; i < BagGrid.SlotCount; i++)
        {
            int index = i;
            var cell = new Border
            {
                Margin = new Thickness(1),
                CornerRadius = new CornerRadius(4),
                BorderThickness = new Thickness(1),
                ToolTip = $"Bag slot {index + 1}",
            };
            cell.MouseLeftButtonDown += (_, _) => onClick(index);
            Grid.SetRow(cell, index / BagGrid.Cols);
            Grid.SetColumn(cell, index % BagGrid.Cols);
            grid.Children.Add(cell);
            cells[index] = cell;
        }

        return (grid, cells);
    }

    private void ToggleSellSlot(int index)
    {
        var slots = _service.Config.BuySell.SellSlots;
        if (!slots.Remove(index)) slots.Add(index);
        PaintSellSlots();
    }

    private void PaintSellSlots()
    {
        if (_sellHint == null) return;
        var bs = _service.Config.BuySell;
        foreach (var (index, cell) in _sellSlotBoxes)
        {
            bool on = bs.SellSlots.Contains(index);
            cell.Background = on ? Res("SystemFillColorSuccessBrush") : Res("CardBackgroundFillColorDefaultBrush");
            cell.BorderBrush = on ? Res("SystemFillColorSuccessBrush") : Res("TextFillColorSecondaryBrush");
        }

        _sellHint.Text = bs.SellSlots.Count == 0
            ? "No slots selected — click the grid to choose what to sell."
            : bs.SellSlots.Count > bs.SellCap
                ? $"{bs.SellSlots.Count} slots selected, which is over the cap of {bs.SellCap}. " +
                  "Raise the cap on purpose, or narrow the selection — the run refuses either way."
                : $"{bs.SellSlots.Count} slot(s) selected (cap {bs.SellCap}).";
    }

    /// <summary>Buy / Sell calibration: the bag grid, and the scroll test that proves the firmware's
    /// wheel works. The shop-row picker and the MAX point land with the tool itself — what is here is
    /// everything that can be built and checked without the buy flow existing yet.</summary>
    private TabItem BuildBuySellCalibrateTab()
    {
        var panel = new StackPanel();

        var hint = Mono();
        hint.Text = "Capture with the SHOP open, the BAG open, and the count dialog showing — one " +
                    "frame then covers every mark on this tab.";
        _buySellHint = hint;

        var image = new Image { Stretch = Stretch.Uniform };
        var canvas = new Canvas { Background = Brushes.Transparent, MinHeight = 300 };
        _bsImage = image;
        _bsCanvas = canvas;
        canvas.MouseDown += (_, e) => BsMouseDown(canvas, e.GetPosition(canvas));
        canvas.MouseMove += (_, e) => BsMouseMove(canvas, e.GetPosition(canvas));
        canvas.MouseUp += (_, e) => BsMouseUp(canvas, e.GetPosition(canvas));
        // The markers are positioned absolutely, while the Image rescales to fit — so anything that
        // changes the canvas size (a scrollbar appearing, the window resizing) moves the image out
        // from under them. Redrawing on resize is what keeps the overlay on the thing it marks.
        canvas.SizeChanged += (_, _) => BsRedrawOverlay();

        var grid = new Grid { Margin = new Thickness(0, 8, 0, 8) };
        grid.Children.Add(image);
        grid.Children.Add(canvas);

        var capture = MakeButton("Capture bag window", ControlAppearance.Primary);
        capture.Click += (_, _) => BsCapture();

        var drawGrid = MakeButton("Draw grid area", ControlAppearance.Secondary);
        drawGrid.Click += (_, _) => ArmDrag("grid");
        var drawSlot = MakeButton("Draw one slot", ControlAppearance.Secondary);
        drawSlot.Click += (_, _) => ArmDrag("slot");
        var showCentres = MakeButton("Show 64 centres", ControlAppearance.Secondary);
        showCentres.Click += (_, _) => BsShowCentres();
        var drawRow = new StackPanel { Orientation = Orientation.Horizontal };
        foreach (var b in new UiButton[] { drawGrid, drawSlot, showCentres })
        {
            b.Margin = new Thickness(0, 0, 6, 0);
            drawRow.Children.Add(b);
        }
        var captureRow = new StackPanel { Orientation = Orientation.Horizontal };
        captureRow.Children.Add(capture);

        panel.Children.Add(Section("Bag grid",
            Hint("Drag a box around the WHOLE 8x8 bag, then a second box around ONE slot. Both are " +
                 "needed: the whole grid gives the pitch, and the single slot is the check that the " +
                 "grid really is uniform. If the two disagree the tab says so rather than averaging " +
                 "them. \"Show 64 centres\" draws where the tool would click — if the dots don't sit " +
                 "on the slots, the drag is wrong."),
            captureRow,
            LabeledField("Draw", drawRow),
            grid));

        var drawList = MakeButton("Draw list region", ControlAppearance.Secondary);
        drawList.Click += (_, _) => ArmDrag("list");
        var markScroll = MakeButton("Mark focus point", ControlAppearance.Secondary);
        markScroll.Click += (_, _) => ArmPoint("scroll");
        var markMax = MakeButton("Mark MAX button", ControlAppearance.Secondary);
        markMax.Click += (_, _) => ArmPoint("max");

        var markRow = new StackPanel { Orientation = Orientation.Horizontal };
        foreach (var b in new UiButton[] { drawList, markScroll, markMax })
        {
            b.Margin = new Thickness(0, 0, 6, 0);
            markRow.Children.Add(b);
        }

        var saveShop = MakeButton("Save Calibration", ControlAppearance.Primary);
        saveShop.Click += (_, _) => BsSave();

        panel.Children.Add(Section("Shop (buying)",
            Hint("Drag a box around EXACTLY the visible rows of the list — the first row's top to the " +
                 "last row's bottom. Every row is derived from that one region, so the rows are only " +
                 "as good as this drag; there is a real boundary to aim at. The focus point only has " +
                 "to be SOMEWHERE inside the game: the wheel works anywhere in the focused window, so " +
                 "its exact position does not matter. It must be somewhere INERT — both tools " +
                 "left-click it at the start of a run to give the game focus, so a point over a list " +
                 "row would select or buy that row. MAX is the count dialog's MAX button."),
            LabeledField("Mark", markRow),
            saveShop));

        // What is set and what is not, so a save with a gap doesn't leave the tool refusing to run
        // for a reason this tab never showed.
        _bsChecklist = Mono();
        _bsChecklist.Text = "nothing captured yet";
        panel.Children.Add(Section("Setup so far",
            Hint("Filled in as you mark things. Saving with a gap is allowed — the tool says what is " +
                 "missing when you press Start, rather than clicking into empty screen."),
            _bsChecklist));

        panel.Children.Add(Section("Result", hint));

        var notches = UiText("3");
        notches.Width = 60;
        notches.VerticalAlignment = VerticalAlignment.Center;
        var up = MakeInlineButton("Scroll up", ControlAppearance.Secondary);
        var down = MakeInlineButton("Scroll down", ControlAppearance.Secondary);
        up.Click += async (_, _) => await BuySellTestScroll(notches, upwards: true);
        down.Click += async (_, _) => await BuySellTestScroll(notches, upwards: false);

        var scrollRow = new StackPanel { Orientation = Orientation.Horizontal };
        scrollRow.Children.Add(notches);
        scrollRow.Children.Add(up);
        scrollRow.Children.Add(down);

        panel.Children.Add(Section("Test scroll (wheel)",
            Hint($"Sends real wheel notches to the game through the Arduino, so you can see how far one " +
                 $"notch moves the shop list — the number that sets every buy item's scroll amount. One " +
                 $"command carries at most {WheelMaxNotches} notches; that is a guard against a malformed " +
                 "value wedging the board, not a limit you should meet in normal use. The game must be " +
                 "FOCUSED and the cursor over the list, which is why this clicks the scroll point first " +
                 "— you have just been clicking the launcher. Needs the Q/Z commands flashed."),
            LabeledField("Notches", scrollRow)));

        RefreshCalibChecklist();
        return MakeTab("Buy / Sell", panel);
    }

    // ── Hover panel calibrator ──────────────────────────────────────────────
    //
    // Universal by design, which is why it is its own tab rather than a section of a tool's: the panel
    // is anchored to the POINTER, so its offset from the pointer is the same everywhere in the game
    // even though its SIZE varies by what is under it. One calibration therefore serves every
    // hover-read this suite ever does. See docs/PLAN-HOVER-INFO.md.
    //
    // The tool PLACES the cursor rather than reading where it happens to be. That is what makes the
    // offset exact: the pointer is at a coordinate the tool chose, so nothing has to be inferred from
    // a cursor read taken at an uncertain moment — and the hover itself is reproducible instead of
    // depending on the user holding still.
    private TabItem BuildTooltipCalibrateTab()
    {
        var panel = new StackPanel();

        var hint = Mono();
        hint.Text = "Capture the game, mark the point to hover, then let the tool hover and capture again.";
        _tooltipHint = hint;

        var image = new Image { Stretch = Stretch.Uniform };
        var canvas = new Canvas { Background = Brushes.Transparent, MinHeight = 300 };
        _tooltipImage = image;
        _tooltipCanvas = canvas;
        canvas.MouseDown += (_, e) => TooltipMouseDown(canvas, e.GetPosition(canvas));
        canvas.MouseMove += (_, e) => TooltipMouseMove(canvas, e.GetPosition(canvas));
        canvas.MouseUp += (_, e) => TooltipMouseUp(canvas, e.GetPosition(canvas));
        canvas.SizeChanged += (_, _) => TooltipRedraw();

        var grid = new Grid { Margin = new Thickness(0, 8, 0, 8) };
        grid.Children.Add(image);
        grid.Children.Add(canvas);

        var capture = MakeButton("Capture game", ControlAppearance.Primary);
        capture.Click += (_, _) => TooltipCapture();
        var mark = MakeButton("Mark hover point", ControlAppearance.Secondary);
        mark.Click += (_, _) => TooltipMarkHoverPoint();
        var hover = MakeButton("Hover and capture", ControlAppearance.Primary);
        hover.Click += (_, _) => TooltipHoverAndCapture();

        var row = new StackPanel { Orientation = Orientation.Horizontal };
        foreach (var b in new UiButton[] { capture, mark, hover })
        {
            b.Margin = new Thickness(0, 0, 6, 0);
            row.Children.Add(b);
        }

        panel.Children.Add(Section("Measure the hover panel",
            Hint("1. Capture game — with the bag or whatever holds the item open.\n" +
                 "2. Mark hover point — click the item whose panel you want, in the capture. The tool " +
                 "will park the cursor there, so pick one that reliably shows a panel.\n" +
                 "3. Hover and capture — the tool moves the cursor there with the Arduino, clicks to " +
                 "give the game focus, waits for the panel, and captures again. Then drag a box around " +
                 "the panel.\n" +
                 "The offset is worked out from the point the tool placed the cursor on, so nothing is " +
                 "typed and nothing is eyeballed. Size the box for the LARGEST panel you care about — " +
                 "a pet's is bigger than a food item's — because a smaller panel then just leaves " +
                 "background behind it, which the read ignores. A box per item type would be tighter " +
                 "and would stop the offset being universal, which is the whole point of it."),
            LabeledField("Do", row),
            grid));

        var save = MakeButton("Save Calibration", ControlAppearance.Primary);
        save.Click += (_, _) => TooltipSave();
        var testRead = MakeButton("Test read", ControlAppearance.Secondary);
        testRead.Click += async (_, _) => await TooltipTestRead(hint);
        // A field rather than a constant, because it was measured and then immediately found wanting:
        // 700ms read the bag instead of the panel on the first attempt, and only worked once the
        // cursor had been parked a while. That is a number to tune by watching, which means a control.
        var hoverBox = UiText(_service.Config.Tooltip.HoverDelayMs.ToString(CultureInfo.InvariantCulture));
        hoverBox.Width = 70;
        hoverBox.VerticalAlignment = VerticalAlignment.Center;
        hoverBox.TextChanged += (_, _) =>
        {
            if (int.TryParse(hoverBox.Text.Trim(), out var ms) && ms >= 100)
                _service.Config.Tooltip.HoverDelayMs = ms;
        };

        var buttons = new StackPanel { Orientation = Orientation.Horizontal };
        save.Margin = new Thickness(0, 0, 6, 0);
        testRead.Margin = new Thickness(0, 0, 6, 0);
        buttons.Children.Add(save);
        buttons.Children.Add(testRead);
        panel.Children.Add(Section("Save and check",
            Hint("Hover delay — how long the cursor sits on the item before the panel is read. 700ms " +
                 "was not always enough: one read caught the bag with the cursor on it and no panel " +
                 "up yet, which looks exactly like a wrong offset. Raise it until Test read is " +
                 "reliable, then press Save to keep it."),
            LabeledField("Hover delay (ms)", hoverBox),
            buttons));

        panel.Children.Add(Section("Result", hint));

        return MakeTab("Calibrate Tooltip", panel);
    }

    private async void TooltipCapture()
    {
        // A plain capture, for marking the hover point on. The offset is NOT measured from this one.
        var shot = await CaptureScreenshotAsync();
        if (shot == null)
        {
            _tooltipHint!.Text = "Game window not found (or minimized) — open and restore the game first.";
            return;
        }

        _tooltipScreenshot = shot.Value.Image;
        _tooltipImage!.Source = shot.Value.Image;
        _tooltipArmed = false;
        _tooltipArmingPoint = false;
        _tooltipDragStart = null;
        _tooltipMarquee = null;
        _tooltipCanvas!.Children.Clear();
        TooltipRedraw();
        _tooltipHint!.Text = "Captured. Now press Mark hover point and click the item whose panel you " +
            "want to measure.";
    }

    /// <summary>Focus on an inert point, THEN move to the hover point — without clicking it.
    ///
    /// The click is only there to give the game focus, and the HID click focuses as it presses. Making
    /// it the hover point too means pressing whatever is being hovered, and on a pet in the bag that
    /// SWITCHES THE EQUIPPED PET (player, 2026-09-18) — so the thing being measured is changed by the
    /// act of measuring it. A hover needs the cursor OVER the item, not a press on it.
    ///
    /// The focus point is the buy/sell one, reused rather than re-marked: it is already calibrated and
    /// already documented as a place nothing responds to, and a second one would be a second thing to
    /// drift.
    /// </summary>
    private async Task<bool> FocusThenHover(SerialPort ser, List<int> hoverPoint, TextBlock hint)
    {
        var focus = _service.Config.BuySell.ScrollPoint;
        if (focus is not { Count: 2 })
        {
            hint.Text = "No focus point is calibrated. Both tools left-click one at the start of a run " +
                        "to give the game focus — mark it on Calibrate Buy/Sell, somewhere inert. It " +
                        "must NOT be the item being hovered: the click would act on it.";
            return false;
        }

        if (!TryPlace(ser, focus[0], focus[1], out var err))
        {
            hint.Text = "Couldn't reach the focus point: " + err;
            return false;
        }
        HidPointer.Click(ser);
        await Task.Delay(400);

        // MOVE only. This is the whole point of the split.
        if (!TryPlace(ser, hoverPoint[0], hoverPoint[1], out err))
        {
            hint.Text = "Couldn't move onto the item: " + err;
            return false;
        }
        await Task.Delay(Math.Max(200, _service.Config.Tooltip.HoverDelayMs));
        return true;
    }

    private void TooltipMarkHoverPoint()
    {
        if (_tooltipScreenshot == null)
        {
            _tooltipHint!.Text = "Capture the game first.";
            return;
        }
        _tooltipArmingPoint = true;
        _tooltipHint!.Text = "Click the item to hover over — one that reliably shows a panel.";
    }

    /// <summary>Move, click to focus, wait for the panel, and capture. The cursor is placed by the tool,
    /// so the coordinate the offset is measured against is one it chose rather than one it read.</summary>
    private async void TooltipHoverAndCapture()
    {
        if (_tooltipHoverPoint is not { Count: 2 } point)
        {
            _tooltipHint!.Text = "Mark the hover point first.";
            return;
        }

        var ser = await _service.ArduinoPortAsync();
        if (ser == null)
        {
            _tooltipHint!.Text = _service.LastArduinoError ?? "Arduino not found.";
            return;
        }

        // One click does both jobs here and elsewhere: the HID click focuses the game as it presses,
        // so there is no separate focus step. It lands on the item, which is the point — the panel is
        // what is being measured.
        if (!await FocusThenHover(ser, point, _tooltipHint!)) return;

        if (_tooltipScreenshot is not null && _tooltipCanvas is not null)
        {
            var shot = await CaptureScreenshotAsync();
            if (shot == null)
            {
                _tooltipHint!.Text = "Couldn't capture after the hover — is the game still up?";
                return;
            }
            _tooltipScreenshot = shot.Value.Image;
            _tooltipImage!.Source = shot.Value.Image;
        }

        _tooltipArmed = true;
        _tooltipArmingPoint = false;
        _tooltipCanvas!.Children.Clear();
        TooltipRedraw();
        _tooltipHint!.Text = "Hovered and captured. Drag a box around the panel now — if no panel is " +
            "showing, the item was a poor choice; mark another and try again.";
    }

    private void TooltipMouseDown(Canvas canvas, Point p)
    {
        if (_tooltipScreenshot == null) return;

        if (_tooltipArmingPoint)
        {
            var natural = CanvasToNatural(p, _tooltipScreenshot, _tooltipCanvas!);
            _tooltipHoverPoint = new List<int>
            {
                (int)Math.Round(natural.X), (int)Math.Round(natural.Y),
            };
            _tooltipArmingPoint = false;
            TooltipRedraw();
            _tooltipHint!.Text = $"Hover point ({_tooltipHoverPoint[0]},{_tooltipHoverPoint[1]}). Now " +
                "press Hover and capture — the tool will move there and take the picture for you.";
            return;
        }

        if (!_tooltipArmed) return;
        _tooltipDragStart = p;
        _tooltipMarquee = new Rectangle
        {
            Stroke = Brushes.Magenta,
            StrokeThickness = 2,
            StrokeDashArray = new DoubleCollection { 4, 2 },
        };
        Canvas.SetLeft(_tooltipMarquee, p.X);
        Canvas.SetTop(_tooltipMarquee, p.Y);
        canvas.Children.Add(_tooltipMarquee);
    }

    private void TooltipMouseMove(Canvas canvas, Point p)
    {
        if (_tooltipMarquee == null || _tooltipDragStart == null) return;
        Canvas.SetLeft(_tooltipMarquee, Math.Min(_tooltipDragStart.Value.X, p.X));
        Canvas.SetTop(_tooltipMarquee, Math.Min(_tooltipDragStart.Value.Y, p.Y));
        _tooltipMarquee.Width = Math.Abs(p.X - _tooltipDragStart.Value.X);
        _tooltipMarquee.Height = Math.Abs(p.Y - _tooltipDragStart.Value.Y);
    }

    private void TooltipMouseUp(Canvas canvas, Point p)
    {
        if (_tooltipMarquee == null || _tooltipDragStart == null || _tooltipScreenshot == null ||
            _tooltipCanvas == null) return;

        var a = CanvasToNatural(_tooltipDragStart.Value, _tooltipScreenshot, _tooltipCanvas);
        var b = CanvasToNatural(p, _tooltipScreenshot, _tooltipCanvas);
        var rect = new List<int>
        {
            (int)Math.Round(Math.Min(a.X, b.X)),
            (int)Math.Round(Math.Min(a.Y, b.Y)),
            (int)Math.Round(Math.Abs(b.X - a.X)),
            (int)Math.Round(Math.Abs(b.Y - a.Y)),
        };

        canvas.Children.Remove(_tooltipMarquee);
        _tooltipMarquee = null;
        _tooltipDragStart = null;
        _tooltipArmed = false;

        if (rect[2] < 20 || rect[3] < 20)
        {
            _tooltipHint!.Text = $"That box is only {rect[2]}x{rect[3]} px — too small, drag again.";
            return;
        }

        // The offset is against the point the TOOL placed the cursor on, not against anything read off
        // the screen. That is the whole reason the hover is a button rather than a user action.
        if (_tooltipHoverPoint is not { Count: 2 } origin)
        {
            _tooltipHint!.Text = "No hover point — press Mark hover point, then Hover and capture again.";
            return;
        }

        var cfg = _service.Config.Tooltip;
        cfg.OffsetX = rect[0] - origin[0];
        cfg.OffsetY = rect[1] - origin[1];
        cfg.Width = rect[2];
        cfg.Height = rect[3];

        TooltipRedraw();
        TooltipSay();
    }

    private void TooltipRedraw()
    {
        if (_tooltipScreenshot == null || _tooltipCanvas == null) return;
        _tooltipCanvas.Children.Clear();
        var canvas = _tooltipCanvas;
        var shot = _tooltipScreenshot;

        if (_tooltipHoverPoint is { Count: 2 } p)
            Dot(canvas, shot, new Point(p[0], p[1]), Brushes.Gold, 18);

        var cfg = _service.Config.Tooltip;
        if (cfg.IsSet && _tooltipHoverPoint is { Count: 2 } origin)
            Box(canvas, shot,
                new List<int> { origin[0] + cfg.OffsetX, origin[1] + cfg.OffsetY, cfg.Width, cfg.Height },
                Brushes.Magenta);
    }

    private void TooltipSay()
    {
        var cfg = _service.Config.Tooltip;
        _tooltipHint!.Text =
            $"Panel offset from the cursor: ({cfg.OffsetX}, {cfg.OffsetY}), reading {cfg.Width}x" +
            $"{cfg.Height} px. Every hover-read in the game uses this — a pet's panel and a food " +
            "item's sit at the same offset even though their sizes differ. Check the magenta box lands " +
            "on the panel rather than beside it, then Save.";
    }

    /// <summary>Hovers the marked point and reads the panel, so we find out whether the OCR can make
    /// sense of a tooltip BEFORE anything is built on it.
    ///
    /// The answer decides more than it looks: the plan is to resolve a pet's stage from its NAME, via
    /// the 387-pet table in PET-DATA.md, and a read that gets the numbers but garbles the name is
    /// nearly useless for that. Chinese text in a stylised game font is a harder read than the
    /// numerals the feeder counts use, and nothing here can tell us which way it goes except trying.
    /// </summary>
    private async Task TooltipTestRead(TextBlock hint)
    {
        var cfg = _service.Config.Tooltip;
        if (!cfg.IsSet)
        {
            hint.Text = "Calibrate the panel first — hover the point, capture, drag the box, Save.";
            return;
        }
        if (_tooltipHoverPoint is not { Count: 2 } point)
        {
            hint.Text = "Mark the hover point first: press Mark hover point and click the item in the " +
                        "capture. The panel is measured from where the cursor ended up, so the read " +
                        "needs to know where that was.";
            return;
        }

        var ser = await _service.ArduinoPortAsync();
        if (ser == null)
        {
            hint.Text = _service.LastArduinoError ?? "Arduino not found.";
            return;
        }

        if (!await FocusThenHover(ser, point, hint)) return;

        // The panel region is the point the cursor was parked on, plus the offset the calibration
        // measured — which is the whole reason the offset is stored rather than an absolute box.
        var region = new RegionConfig
        {
            Left = point[0] + cfg.OffsetX,
            Top = point[1] + cfg.OffsetY,
            Width = cfg.Width,
            Height = cfg.Height,
        };

        var stamp = DateTime.Now.ToString("yyyyMMdd_HHmmss", CultureInfo.InvariantCulture);
        var debugPath = System.IO.Path.Combine(AppContext.BaseDirectory, "logs", "reads", $"tooltip_{stamp}.png");

        try
        {
            var lines = await WithLauncherHiddenAsync(() => _service.ReadText(region, 3, debugPath));
            hint.Text = lines.Count == 0
                ? "Read nothing. See logs\reads\tooltip_*.png — it shows the region the OCR was " +
                  "given, so a wrong offset and unreadable text can be told apart."
                : "Read: " + string.Join(" | ", lines) +
                  "  (also saved to logs\reads\tooltip_*.png)";
        }
        catch (Exception ex)
        {
            hint.Text = "Read failed: " + ex.Message;
        }
    }

    private void TooltipSave()
    {
        var cfg = _service.Config.Tooltip;
        if (!cfg.IsSet)
        {
            _tooltipHint!.Text = "Measure it first — an offset with no size reads the wrong place.";
            return;
        }

        var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
        local.Tooltip = cfg;
        TrySaveCalibration(() => _service.SaveLocal(local), _tooltipHint!,
            $"Saved to config\\local.yaml — offset ({cfg.OffsetX}, {cfg.OffsetY}) at " +
            $"{cfg.Width}x{cfg.Height}. Measured in this machine's pixels, so it does not travel.");
    }

    // ── Pet food auto-replacement — calibrator ──────────────────────────────
    //
    // Marks the boarding (代養) flow; see docs/PLAN-PET-AUTOFEED.md. Two things make this tab unlike
    // the others:
    //
    //   * The marks live on TWO screens that cannot both be up. 目錄 and the pet feed icon are on the
    //     main screen with the icon panel open; everything else needs the boarding window, which
    //     covers that panel. So the tab expects a capture per screen. Marks are written to config as
    //     they are placed, so re-capturing only swaps the background and never loses one.
    //   * The bag here is NOT the bag the shop opens beside. Same 8x8 game grid, different place on
    //     screen — so it gets its own two-corner drag. Sharing the buy/sell numbers would aim every
    //     cell at the wrong item, which is the one failure in this feature with no undo.
    private TabItem BuildPetCalibrateTab()
    {
        var panel = new StackPanel();

        var hint = Mono();
        hint.Text = "Capture with the icon panel open first (click 目錄), mark those two points, then " +
                    "capture again with the boarding window up and mark the rest.";
        _petHint = hint;

        var image = new Image { Stretch = Stretch.Uniform };
        var canvas = new Canvas { Background = Brushes.Transparent, MinHeight = 300 };
        _petImage = image;
        _petCanvas = canvas;
        canvas.MouseDown += (_, e) => PetMouseDown(canvas, e.GetPosition(canvas));
        canvas.MouseMove += (_, e) => PetMouseMove(canvas, e.GetPosition(canvas));
        canvas.MouseUp += (_, e) => PetMouseUp(canvas, e.GetPosition(canvas));
        canvas.SizeChanged += (_, _) => PetRedrawOverlay();

        var grid = new Grid { Margin = new Thickness(0, 8, 0, 8) };
        grid.Children.Add(image);
        grid.Children.Add(canvas);

        var capture = MakeButton("Capture game", ControlAppearance.Primary);
        capture.Click += (_, _) => PetCapture();
        var captureRow = new StackPanel { Orientation = Orientation.Horizontal };
        captureRow.Children.Add(capture);

        panel.Children.Add(Section("Capture",
            Hint("Expect two captures, because the marks live on screens that cannot both be up. " +
                 "FIRST: open the icon panel (click 目錄) and mark those two points. THEN: open the " +
                 "boarding window and press Capture again. Marks already placed are kept — each is " +
                 "written as you make it — so re-capturing only swaps the background."),
            captureRow,
            grid));

        var markMenu = MakeButton("Mark 目錄", ControlAppearance.Secondary);
        markMenu.Click += (_, _) => PetArmPoint("menu");
        var markFeed = MakeButton("Mark pet feed icon", ControlAppearance.Secondary);
        markFeed.Click += (_, _) => PetArmPoint("feed");
        var markClose = MakeButton("Mark boarding X", ControlAppearance.Secondary);
        markClose.Click += (_, _) => PetArmPoint("close");
        var pointRow = new StackPanel { Orientation = Orientation.Horizontal };
        foreach (var b in new UiButton[] { markMenu, markFeed, markClose })
        {
            b.Margin = new Thickness(0, 0, 6, 0);
            pointRow.Children.Add(b);
        }

        panel.Children.Add(Section("How it gets to the feeder",
            Hint("目錄 is the button in the bottom-left icon cluster; it opens a panel of eight round " +
                 "icons, and the pet feed icon in that panel is the chick holding a bottle. Both are " +
                 "POINTS — they never move, so nothing has to be found. Do NOT mark the pet cartoon " +
                 "image on the main screen: that opens the manual feeding window, a different system " +
                 "holding a different food."),
            LabeledField("Mark", pointRow)));

        var tabRow = new StackPanel { Orientation = Orientation.Horizontal };
        for (int i = 1; i <= 3; i++)
        {
            var page = i;
            var b = MakeButton($"Mark ITEM{page}", ControlAppearance.Secondary);
            b.Click += (_, _) => PetArmPoint($"tab{page}");
            b.Margin = new Thickness(0, 0, 6, 0);
            tabRow.Children.Add(b);
        }

        panel.Children.Add(Section("Bag pages",
            Hint("ITEM1 / ITEM2 / ITEM3 are ABSOLUTE tabs: clicking one lands on that page whatever " +
                 "page you were on. That is why they are three marks rather than a next/previous " +
                 "pair — there is nothing to read back and nothing to lose count of."),
            LabeledField("Mark", tabRow)));

        // Only the start button. The feeder slots and the hunger readout used to be here and are
        // gone: the tool never inspects the feeder — it reloads on a schedule rather than asking the
        // game anything — and the hunger readout at the bottom right belongs to the CARRIED pet, not
        // the boarded one. Marks nothing reads are marks that go stale unnoticed.
        // ── WHICH ROW THE MARKS BELOW BELONG TO ────────────────────────────
        //
        // The window has four breeding rows, and the capture (2026-09-19) settled that they are
        // genuinely independent: each carries its own start/end button, its own pet slot and its own
        // food boxes. So every mark in the sections below is a mark for ONE row, and this picks which.
        //
        // Same reasoning as the bag picker's page buttons: a mark written against the wrong row is one
        // the tool will act on, and the tab would give no sign of it. The count matters here too — the
        // free row holds two food stacks and a paid row five, so the tool reloads them on different
        // clocks and a wrong number here is a row reloaded early or left dry.
        _petRowStrip = new StackPanel { Orientation = Orientation.Horizontal };

        var addRow = MakeButton("+ Add row", ControlAppearance.Secondary);
        addRow.Click += (_, _) =>
        {
            var pet = _service.Config.Pet;
            if (pet.Slots.Count >= 4)
            {
                _petHint!.Text = "Four rows is what the window has — one free and three paid.";
                return;
            }
            _petEditRow = pet.Slots.Count;
            EditRow(pet);
            RefreshPetRows();
            PetRedrawOverlay();
        };

        _petStacksBox = UiText("");
        _petStacksBox.Width = 46;
        _petStacksBox.VerticalAlignment = VerticalAlignment.Center;
        _petStacksBox.TextChanged += PetStacksChanged;


        var rowRow = new StackPanel { Orientation = Orientation.Horizontal };
        addRow.Margin = new Thickness(8, 0, 0, 0);
        rowRow.Children.Add(_petRowStrip);
        rowRow.Children.Add(addRow);

        panel.Children.Add(Section("Breeding rows",
            Hint("The window holds four rows — one free, three behind the paid expansion — and each " +
                 "has its OWN start button, pet slot and food boxes. Pick the row first: everything " +
                 "you drag below is recorded against it.\n" +
                 "Food slots per row — 2 on the free row, 5 on a paid one. It is the row's capacity, " +
                 "not a preference, and the tool reloads each row on its own clock because of it. " +
                 "Getting it wrong reloads a row early or leaves it dry."),
            rowRow,
            LabeledField("Food slots in this row", _petStacksBox)));

        var drawToggle = MakeButton("Draw start button", ControlAppearance.Secondary);
        drawToggle.Click += (_, _) => PetArmDrag("toggle");
        var drawPetSlot = MakeButton("Draw pet slot", ControlAppearance.Secondary);
        drawPetSlot.Click += (_, _) => PetArmDrag("petslot");
        var drawFeeder = MakeButton("Draw food counts", ControlAppearance.Secondary);
        // Starts at the first slot that is NOT marked yet, not always at slot 1. Pressing it again
        // would otherwise re-arm slot 1 and overwrite it, so a row with four of five done could never
        // be finished without redoing all four — which is what a half-marked row looked like.
        drawFeeder.Click += (_, _) =>
        {
            var row = EditRow(_service.Config.Pet);
            var next = 0;
            while (next < row.Stacks && BagGrid.IsValidRect(FeederAt(row, next))) next++;
            PetArmDrag($"feeder:{Math.Min(next, Math.Max(0, row.Stacks - 1))}");
        };
        var boxRow = new StackPanel { Orientation = Orientation.Horizontal };
        foreach (var b in new UiButton[] { drawToggle, drawPetSlot, drawFeeder })
        {
            b.Margin = new Thickness(0, 0, 6, 0);
            boxRow.Children.Add(b);
        }

        panel.Children.Add(Section("Starting boarding",
            Hint("Start button — drag a box around the one reading 開始代養 or 結束代養. The tool " +
                 "clicks its centre once the food is loaded. It is one button that both starts and " +
                 "ends boarding, but the label is not read: the schedule already decides when to " +
                 "reload, so there is nothing to ask.\n" +
                 "Pet slot — the square the pet lands in when it is placed. An empty-check crop, " +
                 "and the one that answers \"did the pet actually go in?\" before any food is loaded: " +
                 "a right-click that missed leaves an empty slot and a window that otherwise looks " +
                 "perfectly normal.\n" +
                 "Food counts — drag the first slot's number, then it asks for the second's. These " +
                 "are the READ regions: the number that says how much food is left, which the game " +
                 "draws at each slot's bottom-right and which spills past the slot's frame. Box the " +
                 "digits tightly and stop short of the neighbouring slot, or the two numbers come " +
                 "back as one string."),
            LabeledField("Draw", boxRow)));

        var drawGrid = MakeButton("Draw grid area", ControlAppearance.Secondary);
        drawGrid.Click += (_, _) => PetArmDrag("grid");
        var drawSlot = MakeButton("Draw one slot", ControlAppearance.Secondary);
        drawSlot.Click += (_, _) => PetArmDrag("slot");
        var showCentres = MakeButton("Show 64 centres", ControlAppearance.Secondary);
        showCentres.Click += (_, _) => PetShowCentres();
        var gridRow = new StackPanel { Orientation = Orientation.Horizontal };
        foreach (var b in new UiButton[] { drawGrid, drawSlot, showCentres })
        {
            b.Margin = new Thickness(0, 0, 6, 0);
            gridRow.Children.Add(b);
        }

        panel.Children.Add(Section("Bag grid (this flow's own)",
            Hint("The bag the boarding window opens is at a DIFFERENT place from the one the shop " +
                 "opens beside, so this is its own calibration — the Buy/Sell numbers are NOT reused. " +
                 "Same method though: a box around the whole 8x8, then a second around one slot as the " +
                 "uniformity check. \"Show 64 centres\" draws where the tool would right-click; if the " +
                 "dots miss the slots, re-drag the grid area."),
            LabeledField("Draw", gridRow)));

        _petChecklist = Mono();
        _petChecklist.Text = "nothing captured yet";
        panel.Children.Add(Section("Setup so far",
            Hint("Filled in as you mark things. Saving with a gap is allowed — the tool says what is " +
                 "missing rather than clicking into empty screen."),
            _petChecklist));

        var emptySlot = MakeButton("Capture empty pet slot", ControlAppearance.Secondary);
        emptySlot.Click += (_, _) => PetCaptureEmptySlot();

        var save = MakeButton("Save Calibration", ControlAppearance.Primary);
        save.Click += (_, _) => PetSave();
        var saveRow = new StackPanel { Orientation = Orientation.Horizontal };
        emptySlot.Margin = new Thickness(0, 0, 6, 0);
        saveRow.Children.Add(emptySlot);
        saveRow.Children.Add(save);
        panel.Children.Add(Section("Save",
            Hint("Capture empty pet slot — with the breeder OPEN and NO pet in it. This is the " +
                 "reference the tool compares against after every placement, so it can tell a pet that " +
                 "went in from a right-click that did nothing. Without it the check is skipped and a " +
                 "failed placement is invisible, which is how a 12-hour run lost half its boarding time " +
                 "while reporting success."),
            saveRow));

        panel.Children.Add(Section("Result", hint));

        RefreshPetRows();
        RefreshPetCells();
        RefreshPetChecklist();
        LoadPetCapture();
        return MakeTab("Calibrate Pet", panel);
    }

    // ── Pet tab — the bag setup a run reads ────────────────────────────────
    //
    // This is NOT calibration and does not live on the Calibrate Pet tab, for the reason the Sell
    // screen already gives: what is in the bag changes with what the character has been doing, and a
    // selection carried over from last time is a selection nobody re-checked. The geometry that does
    // NOT change — the bag grid, the windows, the points — stays on Calibrate Pet.
    private TabItem BuildPetTab()
    {
        var panel = new StackPanel();
        var hint = Mono();
        hint.Text = "Mark where the pet food is and where the pet goes, then press Start on the " +
                    "Pet Feeder card.";
        _petTabHint = hint;

        // Open on the page the marks are actually on. It used to open on page 1 whatever the config
        // said, and because the marks live on ONE page the grid came up looking blank — which reads as
        // "my marks are gone" and invites re-clicking them. The picker TOGGLES, so re-clicking a marked
        // cell removes it: the display talking the player into deleting their own calibration.
        _petPickPage = InitialPetPage();

        var pageRow = new StackPanel { Orientation = Orientation.Horizontal };
        for (int i = 1; i <= 3; i++)
        {
            var page = i;
            var b = MakeButton($"Page {page}", ControlAppearance.Secondary);
            b.Click += (_, _) => { _petPickPage = page - 1; RefreshPetCells(); };
            b.Margin = new Thickness(0, 0, 6, 0);
            _petPageButtons.Add(b);
            pageRow.Children.Add(b);
        }

        var modeFood = MakeButton("Mark FOOD cells", ControlAppearance.Secondary);
        modeFood.Click += (_, _) => { _petPick = PetPick.Food; RefreshPetCells(); };

        // "RETURN slot", not "the PET cell". They are different places and the old label said they
        // were the same: this one is where a pet comes BACK to — a cell you keep empty — while a pet
        // you want to breed is sitting in a cell that holds a pet.
        var modePet = MakeButton("Mark the RETURN slot", ControlAppearance.Secondary);
        modePet.Click += (_, _) => { _petPick = PetPick.ReturnSlot; RefreshPetCells(); };

        var modeQueue = MakeButton("Mark a pet to queue", ControlAppearance.Secondary);
        modeQueue.Click += (_, _) => { _petPick = PetPick.QueueSource; RefreshPetCells(); };

        _petFoodModeButton = modeFood;
        _petPetModeButton = modePet;
        _petQueueModeButton = modeQueue;
        modeFood.Margin = new Thickness(0, 0, 6, 0);
        modePet.Margin = new Thickness(0, 0, 6, 0);
        modeQueue.Margin = new Thickness(0, 0, 6, 0);
        var modeRow = new StackPanel { Orientation = Orientation.Horizontal };
        modeRow.Children.Add(modeFood);
        modeRow.Children.Add(modePet);
        modeRow.Children.Add(modeQueue);

        // The same widget the Sell screen uses, over this screen's own selection.
        var (cellGrid, petCells) = MakeBagPicker(TogglePetCell);
        _petCellBoxes = petCells;

        var cellsPanel = new StackPanel();
        cellsPanel.Children.Add(LabeledField("Page to edit", pageRow));
        cellsPanel.Children.Add(LabeledField("A click marks", modeRow));
        cellsPanel.Children.Add(cellGrid);
        _petCellInfo = Mono();
        cellsPanel.Children.Add(_petCellInfo);


        panel.Children.Add(Section("The bag, right now",
            Hint("Where the food is and where the pet goes, as the bag looks BEFORE you start. " +
                 "Re-mark them whenever the bag changes — the tool acts on what is marked here, and " +
                 "a stale mark means it right-clicks whatever has taken that slot since."),
            cellsPanel));

        // This tab's OWN checklist, the counterpart of Calibrate Pet's "Setup so far" — same shape,
        // same idea, and only this tab's items. What the two tabs hold is different in kind: that one
        // is what is true of the machine, this one is what is true of this run.
        _petSessionChecklist = Mono();
        panel.Children.Add(Section("Setup so far — this tab",
            Hint("The run's own state, as opposed to the machine's. The bag marks change whenever " +
                 "the bag does, and the boarding flags change every reload — none of it is a " +
                 "calibration and none of it is reported on Calibrate Pet."),
            _petSessionChecklist));

        var ready = Mono();
        ready.Text = "";
        _petReadyText = ready;
        panel.Children.Add(Section("Ready to run", ready));

        // The two numbers that decide WHEN the tool acts and how long it waits between its own steps.
        // Both are judgements rather than measurements — one trades wasted food against slack, the
        // other trades a slower reload against clicks that do not register — so both are fields.
        var marginBox = UiText(_service.Config.Pet.WaitAfterEmptyMinutes.ToString(CultureInfo.InvariantCulture));
        marginBox.Width = 60;
        marginBox.VerticalAlignment = VerticalAlignment.Center;
        marginBox.TextChanged += (_, _) =>
        {
            if (int.TryParse(marginBox.Text.Trim(), out var m) && m >= 0)
                _service.Config.Pet.WaitAfterEmptyMinutes = m;
        };

        var waitBox = UiText(_service.Config.Pet.ActionWaitMs.ToString(CultureInfo.InvariantCulture));
        waitBox.Width = 70;
        waitBox.VerticalAlignment = VerticalAlignment.Center;
        waitBox.TextChanged += (_, _) =>
        {
            if (int.TryParse(waitBox.Text.Trim(), out var ms) && ms >= 100)
                _service.Config.Pet.ActionWaitMs = ms;
        };

        panel.Children.Add(Section("Timing",
            Hint($"Wait after empty — how many minutes PAST the feeder emptying to reload. The load " +
                 $"is {_service.Config.Pet.LoadMinutesFor(EditRow(_service.Config.Pet))} minutes, so a " +
                 $"wait of {_service.Config.Pet.WaitAfterEmptyMinutes} reloads every " +
                 $"{_service.Config.Pet.CycleMinutesFor(EditRow(_service.Config.Pet))}. " +
                 "It is POSITIVE on purpose: reloading after the feeder empties guarantees it IS " +
                 "empty when the stacks go in, and what the game does with a top-up onto a partial " +
                 "stack is unknown. The cost is that many minutes with nothing fed; 1-2 covers any " +
                 "drift, and a negative value reloads early and discards food." + Environment.NewLine +
                 "Action wait — the pause after EACH step of a reload before the next one. Too short " +
                 "and a click does not register, which costs a whole cycle."),
            LabeledField("Wait after empty (min)", marginBox),
            LabeledField("Action wait (ms)", waitBox)));

        // Boarding state belongs HERE, not under Calibrate, and it is the same argument §13 makes for
        // the queue: whether a pet is in the loader RIGHT NOW is a per-RUN fact, and Calibrate is a
        // once-a-machine screen. It had been on this tab all along, but it edited row 1 whatever you
        // were looking at, so rows 2-4 could never be set — and a wrong flag does the OPPOSITE of the
        // step, since the 結束代養 / 開始代養 control is one button per row.
        //
        // One tick per configured row, rebuilt when the rows change.
        _petBoardingTicks = new StackPanel();

        panel.Children.Add(Section("Boarding state",
            Hint("Tick each row that is boarding RIGHT NOW — the pet is in the loader rather than in " +
                 "the bag. The reload has to END boarding to get the pet back before it can put it in " +
                 "again, and the start/end control is one button per row: pressing it with the wrong " +
                 "idea of the state does the opposite of what the step needs, and the pet ends up in " +
                 "the wrong place. The tool ticks each one for you after a successful reload, since a " +
                 "finished reload always leaves boarding running."),
            _petBoardingTicks));

        // Without this the marks live only in memory and vanish on the next launcher start, which is
        // exactly what happened: they were marked, a run used them, and local.yaml still read
        // food_cells: [] because nothing on this tab had ever written it.
        // "Save", not "Save Calibration" — this tab holds the run's state rather than the machine's,
        // and the two Saves are scoped to their own halves now. Calling both by the same name would
        // say they are the same kind of act, which is the confusion the scoping exists to remove.
        var save = MakeButton("Save", ControlAppearance.Primary);
        save.Click += (_, _) =>
        {
            var pet = _service.Config.Pet;
            if (pet.ReturnSlot is not { Count: 2 } && pet.FoodSlots.Count == 0)
            {
                hint.Text = "Nothing to save — mark the return slot and the food cells first.";
                return;
            }
            PetSessionSave(hint);
        };
        panel.Children.Add(Section("Save", save));

        // Shows what the OCR makes of the feeder slots before anything acts on it. The counts are the
        // reading the reload decision will hang on, and a reading nobody has looked at is a number
        // nobody should trust — the same reason every other calibrator has a Test button.
        var testRead = MakeButton("Test read", ControlAppearance.Secondary);
        testRead.Click += async (_, _) => await PetTestRead(hint);
        panel.Children.Add(Section("Read the feeder",
            Hint("Open the boarding window with the food loaded, then press Test read. The tool reads " +
                 "each feeder slot and reports the text it found — the stack counts the reload " +
                 "decision will use. The slots are the boxes drawn on Calibrate Pet, so if this reads " +
                 "nothing, check those boxes cover the numbers."),
            testRead));

        // ── THE QUEUE ────────────────────────────────────────────────────────
        //
        // A pet returned by the boarding window lands in the FIRST FREE bag slot, and the character
        // farms throughout — so by the time a reload needs to find it, the position it was taken from
        // is meaningless. The icon is not: it is the pet's own portrait, so matching it across the 64
        // cells finds the pet wherever it went.
        //
        // Captured from the bag cell you have marked rather than by dragging a box, because the grid is
        // already calibrated — the cell centre and the pitch give the exact crop, so there is nothing
        // to aim.
        var queueLabel = UiText("", "name this pet");
        queueLabel.Width = 160;
        queueLabel.VerticalAlignment = VerticalAlignment.Center;
        _petQueueLabel = queueLabel;

        var addIcon = MakeButton("Capture the marked pet's icon", ControlAppearance.Secondary);
        addIcon.Click += async (_, _) => await PetCaptureIcon(hint);
        var removeIcon = MakeButton("Remove last", ControlAppearance.Secondary);
        removeIcon.Click += (_, _) =>
        {
            var pet = _service.Config.Pet;
            if (pet.Queue.Count == 0) { hint.Text = "The queue is empty."; return; }
            pet.Queue.RemoveAt(pet.Queue.Count - 1);
            PetSessionSave(hint);
            RefreshPetQueue();
        };

        var queueButtons = new StackPanel { Orientation = Orientation.Horizontal };
        addIcon.Margin = new Thickness(0, 0, 6, 0);
        queueButtons.Children.Add(addIcon);
        queueButtons.Children.Add(removeIcon);

        _petQueuePreview = new Image
        {
            Stretch = Stretch.None,
            HorizontalAlignment = HorizontalAlignment.Left,
            Margin = new Thickness(0, 6, 0, 0),
        };

        _petQueueList = Mono();
        _petQueueList.Text = "no pet icons captured yet";

        panel.Children.Add(Section("The queue — which pets to breed",
            Hint("These are the pets WAITING to be boarded, and there can be as many as you like. " +
                 "To add one: press \"Mark a pet to queue\", click the bag cell the pet is sitting " +
                 "in, put that page up, then press Capture icon. The crop comes from the marked " +
                 "cell, so there is nothing to aim.\n" +
                 "A pet you are queuing is NOT the same as the return slot below. The return slot is " +
                 "where a boarded pet comes BACK to — one cell, kept empty. A queued pet sits in a " +
                 "cell that holds a pet, and each one is photographed separately.\n" +
                 "When a pet finishes it is MAILED and leaves the bag, so a queued pet that can be " +
                 "found is by definition not finished: the tool boards the first match and needs no " +
                 "other test. Any pet of the right kind is a harmless substitute, because an idle " +
                 "breeder is wasted time.\n" +
                 "What is captured is the pet's OWN PORTRAIT, matched wherever it has moved to — so " +
                 "the bag can be rearranged and the queue still works. With no icons captured the " +
                 "tool falls back to the return slot."),
            LabeledField("Label", queueLabel),
            queueButtons,
            _petQueuePreview,
            _petQueueList));

        // The hover read has been a capability with no consumer since 2026-09-18. Before anything is
        // built on it — the finished-pet guard and the queue both are — it has to be shown working on
        // a live pet, and the report has to be wide enough to answer what we do NOT know yet: whether
        // the panel carries the current level's 所需喂养值. If it does, the pet's wyz falls out of one
        // read and the remaining time needs no table, no identification and no second reading.
        var testPanel = MakeButton("Test read the pet panel", ControlAppearance.Secondary);
        testPanel.Click += async (_, _) => await PetTestPanelRead(hint);
        panel.Children.Add(Section("Read a pet's panel",
            Hint("Hovers the pet you marked with \"Mark a pet to queue\", and reports what the OCR " +
                 "makes of the hover panel. The pet has to BE in that cell — one that is in the loader " +
                 "instead leaves the cell empty and the read finds nothing. Needs the panel calibrated " +
                 "on Calibrate Tooltip. The dump of every number is deliberate: it is how we find out " +
                 "what else the panel states besides the three values the parser wants."),
            testPanel));

        panel.Children.Add(Section("Result", hint));

        RefreshPetBoardingTicks();
        RefreshPetQueue();
        RefreshPetCells();
        RefreshPetSessionChecklist();
        RefreshPetReady();
        return MakeTab("Pet", panel);
    }

    /// <summary>Arms one of the single-point marks — they are one click each, and the next click on
    /// the capture fills whichever is armed.</summary>
    private void PetArmPoint(string which)
    {
        if (_petScreenshot == null)
        {
            _petHint!.Text = "Capture the game first.";
            return;
        }
        _petPointTarget = which;
        _petHint!.Text = which switch
        {
            "menu" => "Click 目錄 — the button in the bottom-left icon cluster.",
            "feed" => "Click the pet feed icon in the panel that opens: the chick holding a bottle.",
            "close" => "Click the BOARDING window's X. There are two X buttons on screen at once " +
                       "(this one and the bag's), so make sure it is the boarding window's.",
            _ => $"Click the {which.ToUpperInvariant().Replace("TAB", "-ITEM")} tab in the bag.",
        };
    }

    private void PetSetPoint(string which, List<int> point)
    {
        var pet = _service.Config.Pet;
        switch (which)
        {
            case "menu": pet.MenuButton = point; break;
            case "feed": pet.FeedIcon = point; break;
            case "close": pet.CloseButton = point; break;
            default:
                var page = which[^1] - '1';
                while (pet.PageTabs.Count <= page) pet.PageTabs.Add(new List<int>());
                pet.PageTabs[page] = point;
                break;
        }

        PetRedrawOverlay();
        _petHint!.Text = which switch
        {
            "menu" => $"目錄 at ({point[0]},{point[1]}).",
            "feed" => $"Pet feed icon at ({point[0]},{point[1]}).",
            "close" => $"Boarding X at ({point[0]},{point[1]}).",
            _ => $"ITEM{which[^1]} tab at ({point[0]},{point[1]}).",
        };
    }

    private void PetArmDrag(string target)
    {
        if (_petScreenshot == null)
        {
            _petHint!.Text = "Capture the game first.";
            return;
        }
        _petDragTarget = target;

        // The food counts are a CHAIN, one drag per slot, and how many there are is the ROW's stack
        // count — two on the free row, five on a paid one. It used to stop at two, which is the free
        // row's number generalised to every row: the same mistake the load size made, in the markup.
        if (target.StartsWith("feeder:", StringComparison.Ordinal))
        {
            var i = int.Parse(target["feeder:".Length..], CultureInfo.InvariantCulture);
            var stacks = EditRow(_service.Config.Pet).Stacks;
            _petHint!.Text = $"Drag a box around the COUNT on food slot {i + 1} of {stacks} — the " +
                             "number at its bottom-right, which spills past the slot's frame. Tight " +
                             "around the digits, and not reaching the next slot, or two numbers come " +
                             "back as one string.";
            return;
        }

        _petHint!.Text = target switch
        {
            "toggle" => "Drag a box around the 開始代養 / 結束代養 button. The tool clicks its centre " +
                        "to start boarding once the food is loaded.",
            "petslot" => "Drag a box around the PET SLOT in the boarding window — the square the " +
                        "pet sits in, next to the food.",
            "grid" => "Drag a box around the WHOLE 8x8 bag grid.",
            _ => "Drag a box around ONE slot.",
        };
    }

    private async void PetCapture()
    {
        var shot = await CaptureScreenshotAsync();
        if (shot == null)
        {
            _petHint!.Text = "Game window not found (or minimized) — open and restore the game first.";
            return;
        }

        _petScreenshot = shot.Value.Image;
        _petImage!.Source = shot.Value.Image;
        _petDragTarget = null;
        _petDragStart = null;
        _petMarquee = null;
        _petCanvas!.Children.Clear();
        PetRedrawOverlay();
        _petHint!.Text = "Captured. Marks already placed are kept — if what you need next is not on " +
            "this screen, open it in the game and capture again.";
    }

    private void PetMouseDown(Canvas canvas, Point p)
    {
        if (_petScreenshot == null) return;

        // A single-point mark is finished by the click itself, unlike a box which needs the drag to
        // end before it means anything.
        if (_petPointTarget != null)
        {
            var natural = CanvasToNatural(p, _petScreenshot, _petCanvas!);
            PetSetPoint(_petPointTarget, new List<int>
            {
                (int)Math.Round(natural.X), (int)Math.Round(natural.Y),
            });
            _petPointTarget = null;
            return;
        }

        if (_petDragTarget == null) return;
        _petDragStart = p;
        _petMarquee = new Rectangle
        {
            Stroke = Brushes.Magenta,
            StrokeThickness = 2,
            StrokeDashArray = new DoubleCollection { 4, 2 },
        };
        Canvas.SetLeft(_petMarquee, p.X);
        Canvas.SetTop(_petMarquee, p.Y);
        canvas.Children.Add(_petMarquee);
    }

    private void PetMouseMove(Canvas canvas, Point p)
    {
        if (_petMarquee == null || _petDragStart == null) return;
        Canvas.SetLeft(_petMarquee, Math.Min(_petDragStart.Value.X, p.X));
        Canvas.SetTop(_petMarquee, Math.Min(_petDragStart.Value.Y, p.Y));
        _petMarquee.Width = Math.Abs(p.X - _petDragStart.Value.X);
        _petMarquee.Height = Math.Abs(p.Y - _petDragStart.Value.Y);
    }

    private void PetMouseUp(Canvas canvas, Point p)
    {
        if (_petMarquee == null || _petDragStart == null || _petScreenshot == null || _petCanvas == null) return;

        var a = CanvasToNatural(_petDragStart.Value, _petScreenshot, _petCanvas);
        var b = CanvasToNatural(p, _petScreenshot, _petCanvas);
        var rect = new List<int>
        {
            (int)Math.Round(Math.Min(a.X, b.X)),
            (int)Math.Round(Math.Min(a.Y, b.Y)),
            (int)Math.Round(Math.Abs(b.X - a.X)),
            (int)Math.Round(Math.Abs(b.Y - a.Y)),
        };

        canvas.Children.Remove(_petMarquee);
        _petMarquee = null;
        _petDragStart = null;

        // Smaller floor than the buy/sell tab's 20 px: the hunger readout and the toggle label are
        // text-sized boxes, and 20 would reject legitimate ones. The 6 px inset the empty check uses
        // needs room, so this still refuses a slip of the mouse.
        if (rect[2] < 10 || rect[3] < 10)
        {
            _petHint!.Text = $"That box is only {rect[2]}x{rect[3]} px — too small, drag again.";
            return;
        }

        var pet = _service.Config.Pet;
        switch (_petDragTarget)
        {
            case "toggle":
                EditRow(pet).ToggleLabel = rect;
                _petDragTarget = null;
                _petHint!.Text = $"Start button {rect[2]}x{rect[3]} — the tool clicks its centre once " +
                    "the food is loaded.";
                break;
            case "petslot":
                EditRow(pet).BoardingPetSlot = rect;
                _petDragTarget = null;
                _petHint!.Text = $"Pet slot {rect[2]}x{rect[3]} — the tool checks this is occupied " +
                    "after placing the pet, so a missed right-click stops the run instead of loading " +
                    "food into an empty boarding.";
                break;
            case { } t when t.StartsWith("feeder:", StringComparison.Ordinal):
            {
                var i = int.Parse(t["feeder:".Length..], CultureInfo.InvariantCulture);
                var row = EditRow(pet);
                SetFeederAt(row, i, rect);
                if (i + 1 < row.Stacks)
                {
                    _petDragTarget = $"feeder:{i + 1}";
                    _petHint!.Text = $"Slot {i + 1}'s count {rect[2]}x{rect[3]}. Now slot " +
                                     $"{i + 2} of {row.Stacks}.";
                }
                else
                {
                    _petDragTarget = null;
                    _petHint!.Text = $"Slot {i + 1}'s count {rect[2]}x{rect[3]}. " +
                                     $"All {row.Stacks} marked for this row.";
                }
                break;
            }
            case "grid":
                pet.BagGrid = rect;
                _petDragTarget = "slot";
                _petHint!.Text = $"Grid area {rect[2]}x{rect[3]}. Now drag a box around ONE slot.";
                break;
            default:
                pet.BagSlot = rect;
                _petDragTarget = null;
                var gap = BagGrid.ImpliedGap(pet.BagGrid ?? new List<int>(), rect);
                _petHint!.Text = $"Slot {rect[2]}x{rect[3]}, implying a {Math.Round(gap)} px gap. " +
                    (BagGrid.Disagreement(pet.BagGrid ?? new List<int>(), rect)
                     ?? "Grid and slot box agree.");
                break;
        }

        PetRedrawOverlay();
        RefreshPetChecklist();
    }

    /// <summary>Redraws every pet mark from the current config. Colour-coded so it is obvious which
    /// mark is which on a capture that carries two windows' worth of UI.</summary>
    private void PetRedrawOverlay()
    {
        if (_petScreenshot == null || _petCanvas == null) return;

        _petCanvas.Children.Clear();
        var pet = _service.Config.Pet;
        var canvas = _petCanvas;
        var shot = _petScreenshot;

        if (BagGrid.IsValidRect(pet.BagGrid))
        {
            foreach (var (x, y) in BagGrid.Centres(pet.BagGrid!))
                Dot(canvas, shot, new Point(x, y), Brushes.Magenta, 8);
        }

        if (pet.MenuButton is { Count: 2 } mb) Dot(canvas, shot, new Point(mb[0], mb[1]), Brushes.Gold, 16);
        if (pet.FeedIcon is { Count: 2 } fi) Dot(canvas, shot, new Point(fi[0], fi[1]), Brushes.Orange, 16);
        if (pet.CloseButton is { Count: 2 } cb) Dot(canvas, shot, new Point(cb[0], cb[1]), Brushes.Crimson, 14);

        // Page tabs drawn at growing sizes, so a swapped pair reads as wrong rather than silently
        // sending the tool to the wrong page.
        for (int i = 0; i < pet.PageTabs.Count; i++)
            if (pet.PageTabs[i] is { Count: 2 } tab)
                Dot(canvas, shot, new Point(tab[0], tab[1]), Brushes.DeepSkyBlue, 12 + i * 3);

        // EVERY row, not just the one being edited. Drawing only the selected one made switching rows
        // look like the previous row had been erased — the marks were all still in the config, but the
        // picture said otherwise, which is a display that talks the player into re-marking. The row
        // being edited keeps its solid colours; the others are drawn faint so their position is
        // visible without competing with the one you are working on.
        for (int i = 0; i < pet.Slots.Count; i++)
        {
            var row = pet.Slots[i];
            var editing = i == _petEditRow;

            var toggleBrush = editing ? Brushes.LimeGreen : Faint(Brushes.LimeGreen);
            var slotBrush = editing ? Brushes.DeepSkyBlue : Faint(Brushes.DeepSkyBlue);
            var countBrush = editing ? Brushes.Yellow : Faint(Brushes.Yellow);

            if (BagGrid.IsValidRect(row.ToggleLabel)) Box(canvas, shot, row.ToggleLabel!, toggleBrush);
            if (BagGrid.IsValidRect(row.BoardingPetSlot)) Box(canvas, shot, row.BoardingPetSlot!, slotBrush);
            for (int f = 0; f < Math.Max(1, row.Stacks); f++)
                if (BagGrid.IsValidRect(FeederAt(row, f)))
                    Box(canvas, shot, FeederAt(row, f)!, countBrush);
        }

        if (BagGrid.IsValidRect(pet.BagSlot)) Box(canvas, shot, pet.BagSlot!, Brushes.HotPink);

        RefreshPetChecklist();
    }

    /// <summary>Draws every slot centre the grid implies — the check that the bag really is uniform,
    /// and the only way to see it is.</summary>
    private void PetShowCentres()
    {
        if (_petScreenshot == null)
        {
            _petHint!.Text = "Capture the game first.";
            return;
        }
        if (!BagGrid.IsValidRect(_service.Config.Pet.BagGrid))
        {
            _petHint!.Text = "Draw the grid area first.";
            return;
        }

        PetRedrawOverlay();
        _petHint!.Text = "Drew the 64 slot centres. If the magenta dots don't sit on the bag slots, " +
            "re-drag the grid area. " +
            (BagGrid.Disagreement(_service.Config.Pet.BagGrid!, _service.Config.Pet.BagSlot ?? new List<int>())
             ?? "Grid and slot box agree.");
    }

    private void RefreshPetChecklist()
    {
        if (_petChecklist == null) return;
        var pet = _service.Config.Pet;

        string Mark(bool ok, string label) => (ok ? "  ok   " : "  --   ") + label;

        var tabs = 0;
        foreach (var t in pet.PageTabs) if (t is { Count: 2 }) tabs++;

        var lines = new List<string>
        {
            "SHARED — marked once, used by every row",
            Mark(pet.MenuButton is { Count: 2 }, "目錄 button           (click)"),
            Mark(pet.FeedIcon is { Count: 2 }, "pet feed icon       (click)"),
            Mark(pet.CloseButton is { Count: 2 }, "boarding X          (click)"),
            Mark(tabs == 3, $"bag page tabs       (click) {tabs}/3"),
            Mark(BagGrid.IsValidRect(pet.BagGrid), "bag grid area        (drag)"),
            Mark(BagGrid.IsValidRect(pet.BagSlot), "one bag slot         (drag)"),
            Mark(!string.IsNullOrEmpty(pet.PetSlotEmptyPng), "empty-slot reference (capture)"),
        };
        // THIS TAB'S ITEMS ONLY. The return slot, the food cells and the queue are the Pet tab's, and
        // reporting them here made a per-run gap look like a calibration gap — the two tabs are meant
        // to be answerable separately, and a checklist that spans both is how they stopped being.

        // EVERY row, not just the one being edited. The whole point of the rows is that they differ —
        // a checklist that showed only the selected one would report "ready" for a row 3 that had
        // never been drawn, and the tool would drive it into empty screen. It also makes the rows
        // comparable: three paid rows should read identically apart from their boxes.
        if (pet.Slots.Count == 0)
        {
            lines.Add("");
            lines.Add("ROWS — none yet. Press \"+ Add row\" below.");
        }

        for (int i = 0; i < pet.Slots.Count; i++)
        {
            var row = pet.Slots[i];
            var counts = Enumerable.Range(0, Math.Max(1, row.Stacks))
                .Count(f => BagGrid.IsValidRect(FeederAt(row, f)));

            lines.Add("");
            lines.Add($"ROW {i + 1} — {(i == 0 ? "free, 2 food slots" : "paid, 5 food slots")}" +
                      $"{(i == _petEditRow ? "   ← editing" : "")}");
            lines.Add(Mark(BagGrid.IsValidRect(row.ToggleLabel), "start/end button     (drag)"));
            lines.Add(Mark(BagGrid.IsValidRect(row.BoardingPetSlot), "pet slot             (drag)"));
            lines.Add(Mark(counts >= row.Stacks, $"food counts          (drag) {counts}/{row.Stacks}"));
        }

        _petChecklist.Text = string.Join("\n", lines);
    }

    /// <summary>Marks or unmarks one cell on the page being edited. Food cells toggle; the pet cell
    /// is exclusive — clicking a new one moves it rather than adding a second, because a pet cannot
    /// be in two slots and two marks would be a contradiction the tool would have to resolve.</summary>
    /// <summary>The breeding row the calibrator is editing, created on demand.
    ///
    /// Which row is picked by the strip above the marks, and it matters for the same reason the bag
    /// picker's page buttons matter: a mark written against the wrong row is a mark the tool will act
    /// on, and nothing else on the tab would show it. The tool itself drives every configured row —
    /// this is only about where a drag lands.
    ///
    /// A row ADDED here defaults to five stacks if it is not the first, because rows 2-4 only exist
    /// behind the paid expansion and a paid row holds five. The free row holds two.</summary>
    private PetSlotConfig EditRow(PetConfig pet)
    {
        while (pet.Slots.Count <= _petEditRow)
            pet.Slots.Add(new PetSlotConfig { Stacks = pet.Slots.Count == 0 ? 2 : 5 });
        return pet.Slots[_petEditRow];
    }

    /// <summary>Read/write one of a row's food-count boxes by position. They are a list because a paid
    /// row shows five, but the calibrator has two drag targets and that is all it needs to name.</summary>
    /// <summary>Fills the Pet tab's checklist. Its own items only — see where the section is built for
    /// why the two tabs are kept apart.</summary>
    private void RefreshPetSessionChecklist()
    {
        if (_petSessionChecklist == null) return;
        var pet = _service.Config.Pet;

        string Mark(bool ok, string label) => (ok ? "  ok   " : "  --   ") + label;

        var round = pet.Slots.Sum(r => Math.Max(1, r.Stacks));
        var lines = new List<string>
        {
            Mark(pet.ReturnSlot is { Count: 2 },
                 pet.ReturnSlot is { Count: 2 } rs
                     ? $"return slot          page {rs[0] + 1}, cell {rs[1]}"
                     : "return slot          not marked"),
            Mark(pet.FoodSlots.Count > 0,
                 $"food cells           {pet.FoodSlots.Count} marked, {pet.FoodSlotsUsed} used" +
                 (round > 0 ? $"  (one round loads {round})" : "")),
            Mark(pet.Queue.Count > 0,
                 $"queued pet icons     {pet.Queue.Count}" +
                 (pet.Queue.Count == 0 ? "  (the return slot is used instead)" : "")),
        };

        for (int i = 0; i < pet.Slots.Count; i++)
            lines.Add(Mark(true, $"row {i + 1} boarding       " +
                                 (pet.Slots[i].BoardingRunning ? "yes" : "no — tick it if it is feeding")));

        _petSessionChecklist.Text = string.Join("\n", lines);
    }

    /// <summary>Rebuilds the Pet tab's boarding ticks — one per configured row.</summary>
    ///
    /// Rebuilt rather than built once because the row count changes: a row added on Calibrate Pet has
    /// to appear here as a tick, and a tick with no row behind it must not survive a row's removal.</summary>
    private void RefreshPetBoardingTicks()
    {
        if (_petBoardingTicks == null) return;
        var pet = _service.Config.Pet;

        _petBoardingTicks.Children.Clear();

        if (pet.Slots.Count == 0)
        {
            _petBoardingTicks.Children.Add(new TextBlock
            {
                Text = "No breeding rows are set up — add them on Calibrate Pet.",
                Foreground = Res("TextFillColorSecondaryBrush"),
            });
            return;
        }

        _syncingPetRow = true;
        for (int i = 0; i < pet.Slots.Count; i++)
        {
            var row = pet.Slots[i];
            var tick = new CheckBox
            {
                Content = $"Row {i + 1} is boarding now" +
                          (i == 0 ? "  (the free row, 2 stacks)" : "  (paid, 5 stacks)"),
                IsChecked = row.BoardingRunning,
                Margin = new Thickness(0, 2, 0, 2),
            };
            tick.Checked += (_, _) => { if (!_syncingPetRow) { row.BoardingRunning = true; PetSessionSave(_petTabHint!); } };
            tick.Unchecked += (_, _) => { if (!_syncingPetRow) { row.BoardingRunning = false; PetSessionSave(_petTabHint!); } };
            _petBoardingTicks.Children.Add(tick);
        }
        _syncingPetRow = false;
        RefreshPetSessionChecklist();
    }

    /// <summary>Which bag page the picker opens on: the return slot's page, or failing that the page
    /// holding the most food cells. Falls back to the first page when nothing is marked at all.</summary>
    private int InitialPetPage()
    {
        var pet = _service.Config.Pet;
        if (pet.ReturnSlot is { Count: 2 } back && back[0] >= 0 && back[0] < pet.PageTabs.Count)
            return back[0];

        var most = pet.FoodSlots
            .Where(c => c is { Count: 2 })
            .GroupBy(c => c[0])
            .OrderByDescending(g => g.Count())
            .FirstOrDefault();

        return most != null && most.Key >= 0 && most.Key < Math.Max(1, pet.PageTabs.Count)
            ? most.Key
            : 0;
    }

    private static List<int>? FeederAt(PetSlotConfig row, int i)
        => i < row.FeederSlots.Count ? row.FeederSlots[i] : null;

    private static void SetFeederAt(PetSlotConfig row, int i, List<int> rect)
    {
        while (row.FeederSlots.Count <= i) row.FeederSlots.Add(new List<int>());
        row.FeederSlots[i] = rect;
    }

    /// <summary>Rebuilds the row strip and syncs the food-slot field to the selected row.
    ///
    /// Rebuilt rather than appended to because the row count changes: "+ Add row" has to produce a new
    /// button, and a strip built once at startup could not show a row that was added afterwards. The
    /// same mistake the Buy tab's row picker made, and the same fix.</summary>
    private void RefreshPetRows()
    {
        var pet = _service.Config.Pet;

        if (_petRowStrip != null)
        {
            _petRowStrip.Children.Clear();
            for (int i = 0; i < pet.Slots.Count; i++)
            {
                var index = i;
                var b = MakeButton($"Row {index + 1}", index == _petEditRow
                    ? ControlAppearance.Primary
                    : ControlAppearance.Secondary);
                b.Margin = new Thickness(0, 0, 6, 0);
                b.Click += (_, _) =>
                {
                    _petEditRow = index;
                    RefreshPetRows();
                    PetRedrawOverlay();
                };
                _petRowStrip.Children.Add(b);
            }
        }

        // Written rather than bound, so the field shows the row you just switched to instead of the
        // previous row's count — with the handler suppressed, or setting it would write back.
        if (_petStacksBox != null && pet.Slots.Count > _petEditRow)
        {
            _petStacksBox.TextChanged -= PetStacksChanged;
            _petStacksBox.Text = pet.Slots[_petEditRow].Stacks.ToString(CultureInfo.InvariantCulture);
            _petStacksBox.TextChanged += PetStacksChanged;
        }

        // The boarding ticks are on the Pet tab and there is one per row, so adding a row has to
        // produce another tick — a panel built once at startup could not show a row that did not exist
        // then, which is the same mistake the row strip and the Buy tab's picker both made.
        RefreshPetBoardingTicks();

        // The checklist marks which row is being edited and lists them all, so it has to follow the
        // selection and the row count rather than only the marks.
        RefreshPetChecklist();
    }

    private void PetStacksChanged(object sender, TextChangedEventArgs e)
    {
        if (_petStacksBox == null) return;
        if (int.TryParse(_petStacksBox.Text.Trim(), out var n) && n is > 0 and <= 5)
        {
            EditRow(_service.Config.Pet).Stacks = n;
            RefreshPetChecklist();
        }
    }

    private void TogglePetCell(int cell)
    {
        var pet = _service.Config.Pet;

        switch (_petPick)
        {
            case PetPick.ReturnSlot:
            {
                // ONE cell, and clicking it again clears it. It is where a returned pet lands, so it
                // is a cell you keep free — not a pet's home, and naming it "the PET cell" is what
                // made that ambiguous.
                var same = pet.ReturnSlot is { Count: 2 } p && p[0] == _petPickPage && p[1] == cell;
                pet.ReturnSlot = same ? null : new List<int> { _petPickPage, cell };
                PetSessionSave(_petTabHint!);
                break;
            }

            case PetPick.QueueSource:
                // Transient, and not saved: it says "crop from here", not "a pet lives here".
                _petQueueSource = new List<int> { _petPickPage, cell };
                break;

            default:
            {
                var existing = pet.FoodSlots.FirstOrDefault(c => c is { Count: 2 } && c[0] == _petPickPage && c[1] == cell);
                if (existing != null) pet.FoodSlots.Remove(existing);
                else pet.FoodSlots.Add(new List<int> { _petPickPage, cell });

                // Any change to the set invalidates how far the last run got through it: cells have
                // moved or been added, so a count carried over would skip or repeat. Re-marking means
                // "start from the top of this list", which is the only assumption that is safe either
                // way.
                pet.FoodSlotsUsed = 0;
                break;
            }
        }

        RefreshPetCells();
    }

    private void RefreshPetCells()
    {
        // The controls first, and before the empty-boxes guard: which page and which mode are being
        // edited has to be visible even when nothing is marked yet — that is exactly the state where
        // a wrong page silently swallows the first click.
        for (int i = 0; i < _petPageButtons.Count; i++)
            _petPageButtons[i].Appearance = i == _petPickPage
                ? ControlAppearance.Primary
                : ControlAppearance.Secondary;
        if (_petFoodModeButton != null)
            _petFoodModeButton.Appearance = _petPick == PetPick.Food ? ControlAppearance.Primary : ControlAppearance.Secondary;
        if (_petPetModeButton != null)
            _petPetModeButton.Appearance = _petPick == PetPick.ReturnSlot ? ControlAppearance.Primary : ControlAppearance.Secondary;
        if (_petQueueModeButton != null)
            _petQueueModeButton.Appearance = _petPick == PetPick.QueueSource ? ControlAppearance.Primary : ControlAppearance.Secondary;

        if (_petCellBoxes.Count == 0) return;
        var pet = _service.Config.Pet;

        foreach (var (i, cell) in _petCellBoxes)
        {
            var isFood = pet.FoodSlots.Any(c => c is { Count: 2 } && c[0] == _petPickPage && c[1] == i);
            var isReturn = pet.ReturnSlot is { Count: 2 } p && p[0] == _petPickPage && p[1] == i;
            var isQueue = _petQueueSource is { Count: 2 } qs && qs[0] == _petPickPage && qs[1] == i;

            // Three marks in one grid, and they are different KINDS of thing: food is an inventory,
            // the return slot is a place you keep free, and the queue source is a pet you are about to
            // photograph. Coloured apart so a glance says which is which.
            var brush = isReturn ? Res("SystemFillColorCautionBrush")
                : isQueue ? Res("SystemFillColorAttentionBrush")
                : isFood ? Res("SystemFillColorSuccessBrush")
                : Res("CardBackgroundFillColorDefaultBrush");
            cell.Background = brush;
            cell.BorderBrush = isReturn || isFood || isQueue ? brush : Res("TextFillColorSecondaryBrush");
            cell.ToolTip = isReturn ? $"Bag slot {i + 1} — the RETURN slot (where a pet comes back to)"
                : isQueue ? $"Bag slot {i + 1} — the pet being queued"
                : isFood ? $"Bag slot {i + 1} — food"
                : $"Bag slot {i + 1}";
        }

        if (_petCellInfo == null) return;
        var onPage = pet.FoodSlots.Count(c => c is { Count: 2 } && c[0] == _petPickPage);
        var returnWhere = pet.ReturnSlot is { Count: 2 } rq
            ? $"page {rq[0] + 1}, cell {rq[1]}"
            : "not marked";
        var queueWhere = _petQueueSource is { Count: 2 } qq
            ? $"page {qq[0] + 1}, cell {qq[1]}"
            : "not marked";

        _petCellInfo.Text =
            $"Editing page {_petPickPage + 1} — {onPage} food cell(s) here, {pet.FoodSlots.Count} in " +
            $"total. Return slot: {returnWhere}. Pet to queue: {queueWhere}. " +
            $"A click marks {_petPick switch
            {
                PetPick.ReturnSlot => "the RETURN slot",
                PetPick.QueueSource => "a pet to queue",
                _ => "a FOOD cell",
            }}.";
        RefreshPetSessionChecklist();
        RefreshPetReady();
    }

    /// <summary>What the tool would say if you pressed Start now. Mirrors PetTool.Ready, so a gap
    /// shows on the tab rather than as a card message after the click.</summary>
    private void RefreshPetReady()
    {
        if (_petReadyText == null) return;
        var pet = _service.Config.Pet;

        // THIS TAB'S ITEMS ONLY — the session half. The geometry, the bag grid and the rows are the
        // Calibrate tab's and are reported there; listing them here too made each tab answer for the
        // other, which is the mixing the two are meant to avoid. The card names the real reason if you
        // press Start with a calibration gap, and Calibrate Pet's checklist names it before that.
        var lines = new List<string>();
        if (pet.ReturnSlot is not { Count: 2 } && pet.Queue.Count == 0)
            lines.Add("neither the return slot nor a pet icon is marked — nothing says where the pet is");
        if (pet.FoodSlots.Count == 0) lines.Add("no food cells are marked");

        var round = pet.Slots.Sum(r => Math.Max(1, r.Stacks));
        if (round > 0 && pet.FoodSlots.Count > 0 && pet.FoodSlots.Count < round)
            lines.Add($"{pet.FoodSlots.Count} food cell(s) marked but one round of all " +
                      $"{pet.Slots.Count} row(s) loads {round}");
        if (pet.Queue.Count == 0)
            lines.Add("no pet icon queued — the tool will use the return slot instead of looking " +
                      "for the pet by icon");

        _petReadyText.Text = lines.Count == 0
            ? $"This tab is ready. {pet.Slots.Count} row(s), reloading every " +
              string.Join(" / ", pet.Slots.Select(r => $"{pet.CycleMinutesFor(r)} min")) + "."
            : "Not ready:" + Environment.NewLine + "  - " +
              string.Join(Environment.NewLine + "  - ", lines);
    }

    /// <summary>Writes the bag marks. They live in the same local.yaml block as the geometry because
    /// they are the same kind of thing — coordinates measured on this machine — even though one is set
    /// once and the other re-marked as the bag changes.</summary>
    /// <summary>Reads both feeder slots and reports what the OCR found, without acting on it.
    ///
    /// The launcher is hidden for the read because the capture reads the screen: whatever is on top is
    /// what lands in the image, and a count read off the launcher would be a plausible-looking number
    /// that means nothing.</summary>
    private async Task PetTestRead(TextBlock hint)
    {
        var pet = _service.Config.Pet;

        // EVERY count on the row being edited, not the first two. A paid row has five, so this used
        // to read two of them and quietly imply the row was empty of the other three.
        var row = EditRow(pet);
        var boxes = new List<(string What, List<int> Box)>();
        for (int f = 0; f < Math.Max(1, row.Stacks); f++)
            if (BagGrid.IsValidRect(FeederAt(row, f)))
                boxes.Add(($"slot {f + 1}", FeederAt(row, f)!));

        if (boxes.Count == 0)
        {
            hint.Text = "Draw the food counts on Calibrate Pet first — the counts are read from them.";
            return;
        }

        var report = new List<string>();
        var total = 0;
        var counted = 0;
        try
        {
            foreach (var (what, box) in boxes)
            {
                var region = new RegionConfig { Left = box[0], Top = box[1], Width = box[2], Height = box[3] };
                // Each read leaves its upscaled region behind, so a blank result can be told apart
                // from a wrong region by looking at the file rather than by arguing about it.
                var stamp = DateTime.Now.ToString("yyyyMMdd_HHmmss", CultureInfo.InvariantCulture);
                var debugPath = System.IO.Path.Combine(
                    AppContext.BaseDirectory, "logs", "reads", $"{what.Replace(" ", "")}_{stamp}.png");
                var lines = await WithLauncherHiddenAsync(() => _service.ReadText(region, 3, debugPath));
                var joined = string.Join(" | ", lines);

                // The number is what matters, so it is parsed rather than echoed: a slot reading "270"
                // and one reading "27O" look identical in a report and behave completely differently.
                var digits = new string(joined.Where(char.IsDigit).ToArray());
                var parsed = int.TryParse(digits, out var n) ? n : (int?)null;
                if (parsed is { } value) { total += value; counted++; }

                report.Add($"{what}: {(joined.Length == 0 ? "(nothing)" : joined)}" +
                           (parsed is { } v2 ? $" -> {v2}" : " -> not a number"));
            }
        }
        catch (Exception ex)
        {
            hint.Text = "Read failed: " + ex.Message;
            return;
        }

        hint.Text = string.Join("; ", report) +
            (counted > 0
                ? $". Total {total} items."
                : ". Nothing parsed. Each read saved what the OCR saw to logs\reads — open one to see " +
                  "whether the box is in the wrong place or the number is too small to read.");
    }

    /// <summary>Hovers the marked pet cell and reports the whole panel — the read the feeder's guard
    /// and the queue will both rest on, shown working before anything is built on it.
    ///
    /// It reports EVERY number it saw, not only the three the parser wants, and that is the reason it
    /// exists. The open question is whether the panel carries the current level's 所需喂养值: if it
    /// does, then wyz = that ÷ (1 + growth/10) and the remaining time needs no table, no pet
    /// identification and no second read. A report that echoed only the parse could not answer that.
    ///
    /// The hover is a MOVE, never a click — see FocusThenHover: a click on a pet in the bag SWITCHES
    /// THE EQUIPPED PET, so clicking to measure would change the thing being measured.</summary>
    private async Task PetTestPanelRead(TextBlock hint)
    {
        var pet = _service.Config.Pet;
        var tip = _service.Config.Tooltip;

        if (!tip.IsSet)
        {
            hint.Text = "No hover panel is calibrated. Calibrate Tooltip first — hover a pet, capture, " +
                        "drag a box around the panel, Save. Without the offset this would OCR a " +
                        "rectangle of whatever happens to sit beside the cursor.";
            return;
        }
        if (!BagGrid.IsValidRect(pet.BagGrid))
        {
            hint.Text = "The boarding bag grid isn't calibrated — Calibrate Pet.";
            return;
        }
        // A cell HOLDING A PET, which is the queue source — not the return slot, which is kept empty
        // and would have nothing to hover. Reading an empty cell would report "not a pet panel" and
        // look like the parser failing rather than the wrong cell being read.
        if (_petQueueSource is not { Count: 2 } marked)
        {
            hint.Text = "Mark a pet to read first — press \"Mark a pet to queue\" and click the " +
                        "bag cell the pet is sitting in. NOT the return slot: that one is kept empty.";
            return;
        }

        var centres = BagGrid.Centres(pet.BagGrid!);
        var (page, cell) = (marked[0], marked[1]);
        if (cell < 0 || cell >= centres.Count)
        {
            hint.Text = $"The marked pet ({cell}) is outside the bag grid — mark it again.";
            return;
        }

        // The mark carries the page it lives on, so bring that page up first. Hovering the right
        // COORDINATE over whatever page happens to be showing is the same silent miss the tool itself
        // guards against, and it would read as "the panel is empty" rather than as a wrong page.
        if (page < 0 || page >= pet.PageTabs.Count || pet.PageTabs[page] is not { Count: 2 } tab)
        {
            hint.Text = $"The marked pet is on bag page {page + 1}, and that page's tab isn't " +
                        "calibrated — mark it on Calibrate Pet.";
            return;
        }

        var ser = await _service.ArduinoPortAsync();
        if (ser == null)
        {
            hint.Text = _service.LastArduinoError ?? "Arduino not found.";
            return;
        }

        if (!TryPlace(ser, tab[0], tab[1], out var tabError))
        {
            hint.Text = $"Couldn't reach the ITEM{page + 1} tab: {tabError}";
            return;
        }
        HidPointer.Click(ser);
        await Task.Delay(900);

        var (cx, cy) = centres[cell];
        if (!await FocusThenHover(ser, new List<int> { cx, cy }, hint)) return;

        // The panel region is the cell's centre plus the offset the calibration measured — which is
        // the whole reason the offset is stored rather than an absolute box.
        var region = new RegionConfig
        {
            Left = cx + tip.OffsetX,
            Top = cy + tip.OffsetY,
            Width = tip.Width,
            Height = tip.Height,
        };

        var stamp = DateTime.Now.ToString("yyyyMMdd_HHmmss", CultureInfo.InvariantCulture);
        var debugPath = Path.Combine(AppContext.BaseDirectory, "logs", "reads", $"petpanel_{stamp}.png");

        try
        {
            var lines = await WithLauncherHiddenAsync(() => _service.ReadText(region, 3, debugPath));
            var panel = PetPanel.Parse(lines);

            var report = new List<string>
            {
                $"Page {page + 1}, cell {cell} (bag slot {cell + 1}) — panel read at " +
                $"({region.Left},{region.Top}) {region.Width}x{region.Height}.",
                lines.Count == 0
                    ? "Read NOTHING. The image saved to logs\\reads is the exact region the OCR was " +
                      "given, so a wrong region and a pet that is not in that cell can be told apart " +
                      "by looking at it."
                    : "Lines: " + string.Join(" | ", lines),
                panel == null
                    ? "Not a pet panel — no growth + EXP pair in it. (The parser wants both: a +N and " +
                      "a %.)"
                    : "Parsed: " + panel.Describe(),
                "Numbers seen: " + NumbersWithContext(lines),
            };

            hint.Text = string.Join(Environment.NewLine, report);
        }
        catch (Exception ex)
        {
            hint.Text = "Read failed: " + ex.Message;
        }
    }

    /// <summary>Every number in a read, with a few characters either side of it.
    ///
    /// A diagnostic, NOT a parser — and the difference is the point. The parser deliberately reads
    /// only the three values it wants; this deliberately reads all of them, because the question it
    /// exists to answer cannot be answered by a report that echoes only what we already expected. The
    /// surrounding text is what makes a bare number identifiable in a read whose Chinese is mangled.</summary>
    private static string NumbersWithContext(IReadOnlyList<string> lines)
    {
        var text = string.Join(" ", lines);
        if (text.Length == 0) return "(nothing read)";

        var found = Regex.Matches(text, @"\d+(?:[.,]\d+)?")
            .Select(m =>
            {
                var from = Math.Max(0, m.Index - 6);
                var to = Math.Min(text.Length, m.Index + m.Length + 6);
                return text.Substring(from, to - from);
            })
            .ToList();

        return found.Count == 0 ? "(no numbers)" : string.Join("  |  ", found);
    }

    /// <summary>Writes THIS TAB'S HALF — the run's state — and nothing else.
    ///
    /// The counterpart of Calibrate Pet's Save, scoped for the same reason the tabs are: the return
    /// slot, the food cells, the queue, the timing and the boarding flags describe this run, and
    /// writing them must not disturb a bag grid or a row's geometry. Through ApplySession, which
    /// mutates the loaded block rather than replacing it — replacing is what blanked the other half
    /// twice.
    ///
    /// One writer for all of it: the Save button, the boarding ticks and the icon capture all call
    /// this, because they are all changes to the same half. They used to be two near-identical
    /// methods, which is one field away from disagreeing about what the half contains.</summary>
    private void PetSessionSave(TextBlock hint)
    {
        var pet = _service.Config.Pet;
        var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
        local.Pet ??= new ConfigLoader.LocalPet();
        ConfigLoader.LocalPet.ApplySession(local.Pet, pet);

        TrySaveCalibration(() => _service.SaveLocal(local), hint,
            $"Saved {pet.FoodSlots.Count} food cell(s), the return slot, the queue and the boarding " +
            "flags. The geometry on Calibrate Pet was left alone.");
    }

    /// <summary>Writes the capture the marks were placed against, so the next session opens with the
    /// overlay VISIBLE instead of only listed in the checklist.
    ///
    /// RAW, with no boxes baked in — unlike the tuner's and the gem's reference PNGs. Those are static
    /// records of a decision; this one is redrawn from the config every time the row changes, so a
    /// baked image would show the marks as they were when it was saved rather than as they are now.
    ///
    /// A convenience, so it never fails a save: a lost reference image costs a re-capture, which is
    /// cheaper than a Save that refuses.
    /// </summary>
    private void SavePetCapture()
    {
        if (_petScreenshot == null) return;
        try
        {
            var path = CalibrationImagePath("calib_pet.png");
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            var encoder = new PngBitmapEncoder();
            encoder.Frames.Add(BitmapFrame.Create(_petScreenshot));
            using var fs = File.Create(path);
            encoder.Save(fs);
        }
        catch
        {
            // See above — never the reason a save fails.
        }
    }

    /// <summary>Restores the last session's capture into the Calibrate Pet tab and redraws the marks
    /// on it. Called after the tab is built, not from LoadCalibrationImages, because the image and the
    /// canvas have to exist first.
    ///
    /// This is what the player noticed was missing: the marks were saved, the checklist said so, and
    /// the picture showed nothing, because the overlay is drawn on a capture that only existed in the
    /// session that took it.</summary>
    private void LoadPetCapture()
    {
        if (_petImage == null) return;
        var path = CalibrationImagePath("calib_pet.png");
        if (!File.Exists(path)) return;

        try
        {
            var bmp = new BitmapImage();
            bmp.BeginInit();
            bmp.CacheOption = BitmapCacheOption.OnLoad;
            bmp.UriSource = new Uri(path, UriKind.Absolute);
            bmp.EndInit();
            bmp.Freeze();

            _petScreenshot = bmp;
            _petImage.Source = bmp;
            PetRedrawOverlay();
            if (_petHint != null)
                _petHint.Text = "Showing the capture your saved marks were placed on. Re-capture only " +
                                "if the game window has moved or been resized.";
        }
        catch
        {
            // A reference image that will not load is not worth a message; the marks are all still
            // there, and Capture game is one press away.
        }
    }

    /// <summary>Crops a queued pet's icon out of a fresh capture and adds it to the queue.
    ///
    /// Taken from the MARKED CELL rather than from a dragged box, because the bag grid is already
    /// calibrated: the cell centre and the pitch give the exact crop, so there is nothing to aim. What
    /// it does require is the bag up on the marked page with the pet in that cell — the crop is
    /// whatever is really there, which is why the preview is shown rather than the entry being taken
    /// on trust. A wrong crop is not dangerous (it matches nothing, and the scan then finds no pet and
    /// says so) but it is a queue entry that will never work, and silence about that is worse.
    ///
    /// The box is the same one <see cref="IconMatch.FindBestCell"/> scores against, from the same
    /// centre and pitch — so what is captured here is literally what the scan will be looking for.</summary>
    private async Task PetCaptureIcon(TextBlock hint)
    {
        var pet = _service.Config.Pet;
        if (_petQueueSource is not { Count: 2 } marked)
        {
            hint.Text = "Mark the bag cell HOLDING THE PET first — press \"Mark a pet to queue\", " +
                        "click the cell with the pet in it, then press this. NOT the return slot: " +
                        "that one is kept empty, so there is no pet there to photograph.";
            return;
        }
        if (!BagGrid.IsValidRect(pet.BagGrid))
        {
            hint.Text = "The boarding bag grid isn't calibrated — Calibrate Pet.";
            return;
        }

        var centres = BagGrid.Centres(pet.BagGrid!);
        var cell = marked[1];
        if (cell < 0 || cell >= centres.Count)
        {
            hint.Text = $"The marked pet ({cell}) is outside the bag grid.";
            return;
        }

        var shot = await CaptureScreenshotAsync();
        if (shot == null)
        {
            hint.Text = "Couldn't capture — is the game open and not minimized?";
            return;
        }

        var (cx, cy) = centres[cell];
        var box = new OpenCvSharp.Rect(
            cx - (int)Math.Round(BagGrid.PitchX(pet.BagGrid!) / 2),
            cy - (int)Math.Round(BagGrid.PitchY(pet.BagGrid!) / 2),
            (int)Math.Round(BagGrid.PitchX(pet.BagGrid!)),
            (int)Math.Round(BagGrid.PitchY(pet.BagGrid!)));

        if (box.X < 0 || box.Y < 0 ||
            box.Right > shot.Value.Image.PixelWidth || box.Bottom > shot.Value.Image.PixelHeight)
        {
            hint.Text = "The marked pet falls outside the capture — re-check the bag grid on Calibrate Pet.";
            return;
        }

        try
        {
            using var full = BitmapSourceToMat(shot.Value.Image);
            using var crop = new Mat(full, box);

            pet.Queue.Add(new PetQueueEntry
            {
                Label = string.IsNullOrWhiteSpace(_petQueueLabel?.Text) ? null : _petQueueLabel!.Text.Trim(),
                Rect = new List<int> { box.X, box.Y, box.Width, box.Height },
                Png = IconMatch.ToBase64(crop),
            });

            PetSessionSave(hint);
            RefreshPetQueue();
            if (_petQueuePreview != null) _petQueuePreview.Source = MatToBitmapSource(crop);

            hint.Text = $"Added {box.Width}x{box.Height} from page {marked[0] + 1}, cell {cell}. " +
                        $"{pet.Queue.Count} pet icon(s) queued. Check the preview matches the pet.";
        }
        catch (Exception ex)
        {
            hint.Text = "Couldn't crop the icon: " + ex.Message;
        }
    }

    private void RefreshPetQueue()
    {
        var queue = _service.Config.Pet.Queue;
        if (_petQueueList == null) return;

        if (queue.Count == 0)
        {
            _petQueueList.Text = "no pet icons captured yet — the tool will use the marked return slot";
            RefreshPetSessionChecklist();
            return;
        }

        _petQueueList.Text = string.Join("\n", queue.Select((q, i) =>
            $"  {i + 1}. {(string.IsNullOrWhiteSpace(q.Label) ? "(no label)" : q.Label)}" +
            $"  {q.Rect?[2]}x{q.Rect?[3]} at ({q.Rect?[0]},{q.Rect?[1]})" +
            $"  {q.Png?.Length ?? 0} chars"));
    }

    /// <summary>Crops the boarding window's pet slot out of the current capture and stores it as the
    /// EMPTY reference. Taken from the capture rather than a fresh grab so what is stored is exactly
    /// what was on screen when you looked at it.</summary>
    private void PetCaptureEmptySlot()
    {
        var pet = _service.Config.Pet;
        if (_petScreenshot == null)
        {
            _petHint!.Text = "Capture the game first — with the breeder open and no pet in it.";
            return;
        }
        if (!BagGrid.IsValidRect(EditRow(pet).BoardingPetSlot))
        {
            _petHint!.Text = "Draw the pet slot first — the crop is taken from that box.";
            return;
        }

        var box = EditRow(pet).BoardingPetSlot!;
        var rect = new OpenCvSharp.Rect(box[0], box[1], box[2], box[3]);
        if (rect.Right > _petScreenshot.PixelWidth || rect.Bottom > _petScreenshot.PixelHeight)
        {
            _petHint!.Text = "The pet slot box falls outside the capture — re-draw it and capture again.";
            return;
        }

        try
        {
            using var full = BitmapSourceToMat(_petScreenshot);
            using var crop = new OpenCvSharp.Mat(full, rect);
            pet.PetSlotEmptyPng = IconMatch.ToBase64(crop);
            _petHint!.Text = $"Empty pet slot stored ({rect.Width}x{rect.Height}). The tool will now " +
                "check after every placement that a pet actually went in. Save to keep it.";
            RefreshPetChecklist();
        }
        catch (Exception ex)
        {
            _petHint!.Text = "Couldn't crop the slot: " + ex.Message;
        }
    }

    private void PetSave()
    {
        var pet = _service.Config.Pet;

        // Only the grid pair gets a blocking check: a mis-dragged grid aims every click at the wrong
        // cell, and unlike a mis-aimed sale there is no undo. The rest save with gaps on purpose, so
        // the tool can say what is missing rather than refusing to open.
        if (BagGrid.IsValidRect(pet.BagGrid) && BagGrid.IsValidRect(pet.BagSlot) &&
            BagGrid.Disagreement(pet.BagGrid!, pet.BagSlot!) is { } problem)
        {
            _petHint!.Text = "Not saved: " + problem;
            return;
        }

        // CALIBRATION ONLY. Both Save buttons write through the same projection, but each is scoped to
        // the half its tab owns — this one leaves the food cells, the queue and the boarding flags
        // exactly as the file had them, because they are the Pet tab's business and this screen has no
        // opinion about them. It writes ONTO the loaded block rather than replacing it; assigning a
        // fresh object is what blanked the other half twice before.
        var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
        local.Pet ??= new ConfigLoader.LocalPet();
        ConfigLoader.LocalPet.ApplyCalibration(local.Pet, pet);

        TrySaveCalibration(() => _service.SaveLocal(local), _petHint!,
            "Saved the calibration to config\\local.yaml. The bag marks, the queue and the boarding " +
            "flags are the Pet tab's and were left alone.");

        // Alongside the marks, the picture they were placed on — so reopening the tab shows where
        // they landed rather than only telling you they exist.
        SavePetCapture();
    }

    /// <summary>Arms one of the four single-point marks — they are one click each, and the next click
    /// on the capture fills whichever is armed.</summary>
    private void ArmPoint(string which)
    {
        if (_bsScreenshot == null)
        {
            _buySellHint!.Text = "Capture the game first.";
            return;
        }
        _bsPointTarget = which;
        _buySellHint!.Text = which switch
        {
            "scroll" => "Click a spot in the game that is SAFE TO LEFT-CLICK. Both tools click here " +
                        "first, because starting a run means clicking the launcher and that leaves the " +
                        "game unfocused — a state in which it ignores both the wheel and a right-click. " +
                        "So NOT over a shop row or a bag slot: clicking it must do nothing. Empty panel " +
                        "space or the shop window's title bar. Buying also scrolls from here, which is " +
                        "fine because the wheel acts anywhere in the focused window.",
            _ => "Click the MAX button in the count dialog.",
        };
    }

    private void SetShopPoint(string which, List<int> point)
    {
        var bs = _service.Config.BuySell;
        switch (which)
        {
            case "scroll": bs.ScrollPoint = point; break;
            default: bs.MaxButton = point; break;
        }

        BsRedrawOverlay();
        _buySellHint!.Text = which switch
        {
            "scroll" => $"Focus point at ({point[0]},{point[1]}) — both tools left-click here first to " +
                        "focus the game.",
            _ => $"MAX button at ({point[0]},{point[1]}).",
        };
    }

    private void ArmDrag(string target)
    {
        if (_bsScreenshot == null)
        {
            _buySellHint!.Text = "Capture the bag window first.";
            return;
        }
        _bsDragTarget = target;
        _buySellHint!.Text = target switch
        {
            "grid" => "Drag a box around the WHOLE 8x8 bag grid.",
            "list" => $"Drag a box around EXACTLY the {_service.Config.BuySell.ShopRows} visible rows of " +
                      "the shop list — row 1's top to the last row's bottom. There is a boundary to " +
                      "aim at; the derived rows are only as good as this drag.",
            _ => "Drag a box around ONE slot.",
        };
    }

    private async void BsCapture()
    {
        var shot = await CaptureScreenshotAsync();
        if (shot == null)
        {
            _buySellHint!.Text = "Game window not found (or minimized) — open and restore the game first.";
            return;
        }

        _bsDisplay = shot.Value.Display;
        _bsScreenshot = shot.Value.Image;
        _bsImage!.Source = shot.Value.Image;
        _bsDragTarget = null;
        _bsDragStart = null;
        _bsMarquee = null;
        _bsCanvas!.Children.Clear();
        RefreshCalibChecklist();
        _buySellHint!.Text = "Captured. Draw the grid area and one slot, then mark the shop rows, " +
            "the focus point and MAX. (If the shop or bag isn't open in the capture, capture again.) " +
            "BOTH TOOLS LEFT-CLICK THE FOCUS POINT at the start of a run — that is what gives the game " +
            "focus — so put it somewhere inert, never over a list row or a bag slot.";
    }

    private void BsMouseDown(Canvas canvas, Point p)
    {
        if (_bsScreenshot == null) return;

        // A single-point mark is finished by the click itself, unlike a box which needs the drag to
        // end before it means anything.
        if (_bsPointTarget != null)
        {
            var natural = CanvasToNatural(p, _bsScreenshot, _bsCanvas!);
            SetShopPoint(_bsPointTarget, new List<int>
            {
                (int)Math.Round(natural.X), (int)Math.Round(natural.Y),
            });
            _bsPointTarget = null;
            return;
        }

        if (_bsDragTarget == null) return;
        _bsDragStart = p;
        _bsMarquee = new Rectangle
        {
            Stroke = Brushes.Magenta,
            StrokeThickness = 2,
            StrokeDashArray = new DoubleCollection { 4, 2 },
        };
        Canvas.SetLeft(_bsMarquee, p.X);
        Canvas.SetTop(_bsMarquee, p.Y);
        canvas.Children.Add(_bsMarquee);
    }

    private void BsMouseMove(Canvas canvas, Point p)
    {
        if (_bsMarquee == null || _bsDragStart == null) return;
        Canvas.SetLeft(_bsMarquee, Math.Min(_bsDragStart.Value.X, p.X));
        Canvas.SetTop(_bsMarquee, Math.Min(_bsDragStart.Value.Y, p.Y));
        _bsMarquee.Width = Math.Abs(p.X - _bsDragStart.Value.X);
        _bsMarquee.Height = Math.Abs(p.Y - _bsDragStart.Value.Y);
    }

    private void BsMouseUp(Canvas canvas, Point p)
    {
        if (_bsMarquee == null || _bsDragStart == null || _bsScreenshot == null || _bsCanvas == null) return;

        var a = CanvasToNatural(_bsDragStart.Value, _bsScreenshot, _bsCanvas);
        var b = CanvasToNatural(p, _bsScreenshot, _bsCanvas);
        var rect = new List<int>
        {
            (int)Math.Round(Math.Min(a.X, b.X)),
            (int)Math.Round(Math.Min(a.Y, b.Y)),
            (int)Math.Round(Math.Abs(b.X - a.X)),
            (int)Math.Round(Math.Abs(b.Y - a.Y)),
        };

        canvas.Children.Remove(_bsMarquee);
        _bsMarquee = null;
        _bsDragStart = null;

        // A slip of the mouse shouldn't silently become a 3-pixel grid. Same guard the tuner and gem
        // calibrators use, for the same reason.
        if (rect[2] < 20 || rect[3] < 20)
        {
            _buySellHint!.Text = $"That box is only {rect[2]}x{rect[3]} px — too small, drag again.";
            return;
        }

        var cfg = _service.Config.BuySell;
        if (_bsDragTarget == "grid")
        {
            cfg.BagGrid = rect;
            _bsDragTarget = "slot";
            _buySellHint!.Text = $"Grid area {rect[2]}x{rect[3]}. Now drag a box around ONE slot.";
        }
        else if (_bsDragTarget == "list")
        {
            cfg.ShopRegion = rect;
            _bsDragTarget = null;
            BsRedrawOverlay();
            _buySellHint!.Text = $"List region {rect[2]}x{rect[3]} — row pitch works out at " +
                $"{Math.Round(ShopGeometry.RowPitch(rect, cfg.ShopRows), 1)} px over {cfg.ShopRows} rows. " +
                "Press Show centres to check the rows land where they should.";
        }
        else
        {
            cfg.BagSlot = rect;
            _bsDragTarget = null;
            BsRedrawOverlay();
            var gap = BagGrid.ImpliedGap(cfg.BagGrid ?? new List<int>(), rect);
            _buySellHint!.Text = $"Slot {rect[2]}x{rect[3]}, implying a {Math.Round(gap)} px gap between " +
                "slots. " + (_service.GridCheck() ?? "Press Show 64 centres to check.");
        }
        RefreshCalibChecklist();
    }

    /// <summary>Redraws every marker on the capture from the current config: the 64 slot centres the
    /// grid implies, and the four shop marks. Called after each mark, so a click that registered is
    /// visible immediately — without it a click looks like it did nothing, which is exactly how the
    /// shop marks felt.</summary>
    /// <summary>A calibrated rectangle as an outline on the capture.</summary>
    /// <summary>The same colour at low opacity — for a mark that is NOT the one being edited.
    ///
    /// Faded at full colour rather than switched to grey, so the four kinds of box stay recognisable
    /// across rows: green is always a start button, blue always a pet slot, yellow always a count.
    /// A grey version would make every row look like the same kind of thing.</summary>
    private static Brush Faint(Brush b) =>
        b is SolidColorBrush s ? new SolidColorBrush(Color.FromArgb(70, s.Color.R, s.Color.G, s.Color.B)) : b;

    private static void Box(Canvas? canvas, BitmapSource? shot, List<int> rect, Brush stroke)
    {
        if (!CanvasReady(canvas, shot)) return;
        var a = NaturalToCanvas(new Point(rect[0], rect[1]), shot!, canvas!);
        var b = NaturalToCanvas(new Point(rect[0] + rect[2], rect[1] + rect[3]), shot!, canvas!);
        var box = new Rectangle
        {
            Width = Math.Max(1, b.X - a.X),
            Height = Math.Max(1, b.Y - a.Y),
            Stroke = stroke,
            StrokeThickness = 2,
            Fill = Brushes.Transparent,
        };
        Canvas.SetLeft(box, a.X);
        Canvas.SetTop(box, a.Y);
        canvas!.Children.Add(box);
    }

    private void BsRedrawOverlay()
    {
        if (_bsScreenshot == null || _bsCanvas == null) return;

        _bsCanvas.Children.Clear();
        var bs = _service.Config.BuySell;
        var canvas = _bsCanvas;
        var shot = _bsScreenshot;

        if (BagGrid.IsValidRect(bs.BagGrid))
        {
            foreach (var (x, y) in BagGrid.Centres(bs.BagGrid!))
                Dot(canvas, shot, new Point(x, y), Brushes.Magenta, 8);
        }

        // Distinct colours so it is obvious which mark is which on a busy capture.
        if (bs.ScrollPoint is { Count: 2 } sp) Dot(canvas, shot, new Point(sp[0], sp[1]), Brushes.Yellow, 14);
        // The list region and the rows derived from it — the same idea as the 64 bag centres, and
        // the check that matters: if these don't sit on the rows, the region drag is wrong.
        if (BagGrid.IsValidRect(bs.ShopRegion))
        {
            Box(canvas, shot, bs.ShopRegion!, Brushes.DeepSkyBlue);
            foreach (var (x, y) in ShopGeometry.RowCentres(bs.ShopRegion, bs.ShopRows))
                Dot(canvas, shot, new Point(x, y), Brushes.LimeGreen, 10);
        }

        if (bs.MaxButton is { Count: 2 } mb) Dot(canvas, shot, new Point(mb[0], mb[1]), Brushes.OrangeRed, 14);

        RefreshCalibChecklist();
    }

    /// <summary>One marker, at a natural (image) coordinate. Local because the canvas shows the
    /// screenshot scaled to fit, so a computed coordinate has to be mapped forward first.</summary>
    /// <summary>False until the canvas has been laid out. Drawing before that would place markers as
    /// if the scale were 1, i.e. silently in the wrong spot — better to draw nothing and let the
    /// resize handler do it once the size is known.</summary>
    private static bool CanvasReady(Canvas? canvas, BitmapSource? shot) =>
        shot != null && canvas is { ActualWidth: > 0, ActualHeight: > 0 };

    /// <summary>One marker at a natural (image) coordinate. Takes its canvas explicitly rather than
    /// reading the buy/sell fields, because there is more than one calibrator on this window and each
    /// draws on its own capture.</summary>
    private static void Dot(Canvas? canvas, BitmapSource? shot, Point natural, Brush stroke, double size)
    {
        if (!CanvasReady(canvas, shot)) return;
        var dot = new Ellipse
        {
            Width = size,
            Height = size,
            Stroke = stroke,
            StrokeThickness = 2,
            Fill = Brushes.Transparent,
        };
        var p = NaturalToCanvas(natural, shot!, canvas!);
        Canvas.SetLeft(dot, p.X - size / 2);
        Canvas.SetTop(dot, p.Y - size / 2);
        canvas!.Children.Add(dot);
    }

    /// <summary>Which marks are set and which are missing. Saving with a gap leaves the tool refusing
    /// to run for a reason the tab never showed, so the gap is listed here instead.</summary>
    private void RefreshCalibChecklist()
    {
        if (_bsChecklist == null) return;
        var bs = _service.Config.BuySell;

        string Mark(bool ok, string label) => (ok ? "  ok   " : "  --   ") + label;

        var lines = new List<string>
        {
            Mark(BagGrid.IsValidRect(bs.BagGrid), "bag grid area      (drag)"),
            Mark(BagGrid.IsValidRect(bs.BagSlot), "one bag slot       (drag)"),
            Mark(ShopGeometry.Problem(bs.ShopRegion, bs.ShopRows) == null, "shop list region   (drag)"),
            Mark(bs.ScrollPoint is { Count: 2 }, "focus point        (click)"),
            Mark(bs.MaxButton is { Count: 2 }, "MAX button         (click)"),
        };
        if (bs.Presets.Count > 0) lines.Add("  " + bs.Presets.Count + " buy item(s)      (Buy tab)");
        if (bs.SellSlots.Count > 0) lines.Add("  " + bs.SellSlots.Count + " slot(s) to sell (Sell tab)");

        _bsChecklist.Text = string.Join("\n", lines);
    }

    /// <summary>Draws every slot centre the grid implies. The whole verification step: the derivation
    /// assumes the grid is uniform, and 64 dots at once is the only way to see whether it is.</summary>
    private void BsShowCentres()
    {
        if (_bsScreenshot == null)
        {
            _buySellHint!.Text = "Capture the game first.";
            return;
        }
        if (!BagGrid.IsValidRect(_service.Config.BuySell.BagGrid))
        {
            _buySellHint!.Text = "Draw the grid area first.";
            return;
        }

        BsRedrawOverlay();
        var rows = ShopGeometry.RowCentres(_service.Config.BuySell.ShopRegion, _service.Config.BuySell.ShopRows).Count;
        _buySellHint!.Text = $"Drew the slot centres and {rows} row centres. If the magenta dots don't " +
            "sit on the bag slots, re-drag the grid area; if the green ones don't sit on the shop rows, " +
            "re-drag the list region. " + (_service.GridCheck() ?? "Grid and slot box agree.");
    }

    private void BsSave()
    {
        var cfg = _service.Config.BuySell;

        if (!BagGrid.IsValidRect(cfg.BagGrid) || !BagGrid.IsValidRect(cfg.BagSlot))
        {
            _buySellHint!.Text = "Draw both boxes first — the grid area and one slot.";
            return;
        }
        if (_service.GridCheck() is { } problem)
        {
            _buySellHint!.Text = "Not saved: " + problem;
            return;
        }

        SaveBuySell(_buySellHint!,
            $"Saved to config\\local.yaml — slot pitch {Math.Round(BagGrid.PitchX(cfg.BagGrid!), 1)} px.");
    }

    /// <summary>One wheel nudge, for calibrating how far a notch moves the list.</summary>
    private async Task BuySellTestScroll(Wpf.Ui.Controls.TextBox notches, bool upwards)
    {
        if (!int.TryParse(notches.Text.Trim(), out var n) || n < 1)
        {
            _buySellHint!.Text = "Enter how many notches to send (1 or more).";
            return;
        }
        if (n > WheelMaxNotches)
        {
            // It is clamped, not queued — say so rather than let the number silently shrink.
            _buySellHint!.Text = $"The firmware caps a single scroll at {WheelMaxNotches} notches. " +
                "Send it twice if the list is longer.";
            return;
        }

        if (_service.Config.BuySell.ScrollPoint is not { Count: 2 } point)
        {
            _buySellHint!.Text = "Mark the scroll point first. The wheel acts on whatever is under the " +
                "cursor, so the cursor has to be put over the shop list before scrolling means anything.";
            return;
        }

        var ser = await _service.ArduinoPortAsync();
        if (ser == null)
        {
            _buySellHint!.Text = _service.LastArduinoError ?? "Arduino not found.";
            return;
        }

        // One click does both jobs. The cursor has to be OVER the list because that is where the
        // wheel goes, and the game has to be FOCUSED because it ignores the wheel otherwise — unlike
        // the HID clicks, which work unfocused because the first one focuses as it presses. Clicking
        // the scroll point achieves both, which is why there is no separate focus mark to calibrate.
        if (!TryPlace(ser, point[0], point[1], out var err))
        {
            _buySellHint!.Text = "Couldn't move the cursor to the scroll point: " + err;
            return;
        }
        HidPointer.Click(ser);
        await Task.Delay(500);

        try
        {
            ser.Write((upwards ? "Q " : "Z ") + n + "\n");
            _buySellHint!.Text = $"Clicked the list to focus and park the cursor, then sent {n} " +
                $"notch(es) {(upwards ? "up" : "down")} — did the list move?";
        }
        catch (Exception ex)
        {
            _buySellHint!.Text = $"Send failed: {ex.Message}";
        }
    }

    private static TabItem MakeTab(string header, Panel panel) =>
        new()
        {
            Header = header,
            Content = new ScrollViewer
            {
                Content = panel,
                VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                // Disabled, not Auto: with horizontal scrolling available the content is measured
                // with infinite width, so hint paragraphs never wrap and get clipped instead.
                HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            },
        };

    /// <summary>A button made to sit in a row WITH a text field. MakeButton carries a 10px top margin
    /// — right for a button stacking under a card's heading, wrong beside a field, where it drops the
    /// button 10px below the box it belongs to. This keeps the left gap and drops the top. The gem
    /// calibrator was already working around this by hand; now it has a name.</summary>
    private static UiButton MakeInlineButton(string text, ControlAppearance appearance)
    {
        var button = MakeButton(text, appearance);
        button.Margin = new Thickness(6, 0, 0, 0);
        return button;
    }

    /// <summary>A per-row button in the sell grid. Small, but sized for a word rather than a symbol.</summary>
    private static UiButton MakeRowButton(string text) => new()
    {
        Content = text,
        Appearance = ControlAppearance.Secondary,
        Height = 32,
        Padding = new Thickness(8, 0, 8, 0),
        Margin = new Thickness(6, 1, 0, 1),
    };

    /// <summary>A small +/- button for a stepper. Separate from <see cref="MakeButton"/> because that
    /// one is sized for "Start"/"Save" and sets MinWidth 84 — which made a "−" 84 pixels wide.</summary>
    private static UiButton MakeStepperButton(string text) => new()
    {
        Content = text,
        Appearance = ControlAppearance.Secondary,
        Width = 32,
        Height = 32,
        Padding = new Thickness(0),
        Margin = new Thickness(0),
    };

    private static UiButton MakeButton(string text, ControlAppearance appearance) =>
        new()
        {
            Content = text,
            Appearance = appearance,
            MinWidth = 84,
            Margin = new Thickness(0, 10, 6, 0),
        };

    private static string SerializeKeys(Dictionary<string, double> keys) =>
        string.Join("\n", keys.Select(kv => $"{kv.Key}:{kv.Value}"));

    private static Dictionary<string, double> ParseKeys(string text)
    {
        var result = new Dictionary<string, double>();
        foreach (var line in text.Split('\n'))
        {
            var t = line.Trim();
            if (t.Length == 0) continue;

            var idx = t.LastIndexOf(':');
            if (idx <= 0) continue;

            var key = t[..idx].Trim();
            if (double.TryParse(t[(idx + 1)..].Trim(), out var cd)) result[key] = cd;
        }
        return result;
    }

    // A single filter-rule editor row: attribute dropdown + count/min/max number boxes.
    private sealed class RuleRow
    {
        public ComboBox Name { get; } = new();
        public TextBox Count { get; } = new();
        public TextBox Min { get; } = new();
        public TextBox Max { get; } = new();

        public FilterRule ToRule()
        {
            var r = new FilterRule { Name = Name.SelectedItem?.ToString() ?? "" };
            if (int.TryParse(Count.Text, out var c)) r.Count = c;
            if (int.TryParse(Min.Text, out var m)) r.Min = m;
            if (int.TryParse(Max.Text, out var mx)) r.Max = mx;
            return r;
        }
    }

    // Builds an editable rule list: attribute dropdown + count/min/max boxes + delete,
    // with an "Add" button at the bottom. Rows are appended to `rows` as they are created.
    private StackPanel BuildRulesEditor(List<FilterRule> initial, List<RuleRow> rows, string addLabel)
    {
        var panel = new StackPanel();
        var names = _service.Attributes.Attributes.Select(a => a.Name).ToList();

        void AddRow(FilterRule? rule)
        {
            var row = new RuleRow();
            foreach (var n in names) row.Name.Items.Add(n);
            row.Name.SelectedItem = rule != null && names.Contains(rule.Name) ? rule.Name : (names.Count > 0 ? names[0] : "");
            row.Name.Width = 160;
            row.Count.Text = (rule?.Count ?? 1).ToString(CultureInfo.InvariantCulture);
            row.Min.Text = rule?.Min?.ToString(CultureInfo.InvariantCulture) ?? "";
            row.Max.Text = rule?.Max?.ToString(CultureInfo.InvariantCulture) ?? "";
            row.Count.Width = 36;
            row.Min.Width = 44;
            row.Max.Width = 44;

            var line = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 2, 0, 2) };
            line.Children.Add(row.Name);
            line.Children.Add(FieldLabel("Count"));
            line.Children.Add(row.Count);
            line.Children.Add(FieldLabel("Min"));
            line.Children.Add(row.Min);
            line.Children.Add(FieldLabel("Max"));
            line.Children.Add(row.Max);

            var del = new UiButton { Content = "✕", Appearance = ControlAppearance.Secondary, MinWidth = 28, Margin = new Thickness(8, 0, 0, 0) };
            del.Click += (_, _) => { panel.Children.Remove(line); rows.Remove(row); };
            line.Children.Add(del);

            panel.Children.Add(line);
            rows.Add(row);
        }

        foreach (var r in initial) AddRow(r);

        var add = MakeButton(addLabel, ControlAppearance.Secondary);
        add.Click += (_, _) => AddRow(null);
        panel.Children.Add(add);

        return panel;
    }

    private TextBlock FieldLabel(string text) => new()
    {
        Text = " " + text + " ",
        VerticalAlignment = VerticalAlignment.Center,
        Foreground = Res("TextFillColorSecondaryBrush"),
        FontSize = 12,
    };

    private static string FindRootDir()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null)
        {
            if (File.Exists(Path.Combine(dir.FullName, "config", "defaults.yaml")))
            {
                return dir.FullName;
            }

            dir = dir.Parent;
        }

        throw new InvalidOperationException("Could not locate the v2 root (config/defaults.yaml).");
    }
}

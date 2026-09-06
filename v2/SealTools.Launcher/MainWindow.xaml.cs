using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
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
using FluentWindow = Wpf.Ui.Controls.FluentWindow;
using ControlAppearance = Wpf.Ui.Controls.ControlAppearance;
using UiButton = Wpf.Ui.Controls.Button;
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
    };

    private static readonly string[] Grades = { "N", "G", "DG", "XG", "SG" };
    private static readonly string[] MatchModes = { "any", "all", "per_attr" };
    private static readonly string[] GemGrades = { "N", "G", "DG" };
    private static readonly string[] RequireGradeOptions = { "None", "N", "G", "DG", "XG", "SG" };

    private readonly LauncherService _service;
    private readonly Dictionary<string, TextBlock> _statusBlocks = new();
    private readonly DispatcherTimer _timer;

    private static readonly string[] CalibGemSteps = { "N", "G", "DG", "Register", "Combine" };
    private bool _disposed;

    // Tuner calibrator (drag three boxes: grade / attributes / remaining) state.
    private Image? _tunerImage;
    private Canvas? _tunerCanvas;
    private TextBlock? _tunerHint;
    private BitmapSource? _tunerScreenshot;
    private int _tunerStep;
    private Rect? _tunerGradeBox;
    private Rect? _tunerAttrBox;
    private Rect? _tunerRemainingBox;
    // Measured attribute line pitch from the last "Check OCR", used by BuildOcrGeometry
    // to persist an accurate row_height instead of the loose attr.Height/3 guess.
    private int? _tunerAttrPitch;
    private Point? _tunerDragStart;
    private Rectangle? _tunerMarquee;

    // Gem calibrator (click buttons) state.
    private Image? _gemImage;
    private Canvas? _gemCanvas;
    private TextBlock? _gemHint;
    private BitmapSource? _gemScreenshot;
    private readonly Dictionary<string, Point> _gemPoints = new();
    private int _gemStep;

    public MainWindow()
    {
        InitializeComponent();

        _service = new LauncherService(FindRootDir());
        BuildToolCards();
        BuildConfigTabs();

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
                Foreground = (Brush)FindResource("FgBrush"),
                FontSize = 18,
                FontWeight = FontWeights.SemiBold,
            };

            var statusText = new TextBlock
            {
                Text = "stopped",
                Foreground = (Brush)FindResource("BadBrush"),
                FontSize = 13,
                TextWrapping = TextWrapping.Wrap,
                Margin = new Thickness(0, 6, 0, 0),
            };
            _statusBlocks[id] = statusText;

            var startButton = MakeButton("Start", ControlAppearance.Primary);
            startButton.Click += (_, _) => _service.StartTool(id);

            var stopButton = MakeButton("Stop", ControlAppearance.Danger);
            stopButton.Click += (_, _) => _service.StopTool();

            var buttons = new StackPanel { Orientation = Orientation.Horizontal };
            buttons.Children.Add(startButton);
            buttons.Children.Add(stopButton);

            var left = new StackPanel();
            left.Children.Add(nameText);
            left.Children.Add(statusText);

            var cardGrid = new Grid();
            cardGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            cardGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            Grid.SetColumn(left, 0);
            Grid.SetColumn(buttons, 1);
            cardGrid.Children.Add(left);
            cardGrid.Children.Add(buttons);

            var card = new Border
            {
                Background = (Brush)FindResource("CardBrush"),
                CornerRadius = new CornerRadius(12),
                BorderBrush = new SolidColorBrush(Color.FromArgb(0x20, 0xFF, 0xFF, 0xFF)),
                BorderThickness = new Thickness(1),
                Padding = new Thickness(20),
                Margin = new Thickness(0, 0, 0, 12),
                Child = cardGrid,
            };

            ToolsPanel.Children.Add(card);
        }
    }

    private void RefreshStatus()
    {
        var state = _service.CurrentState;
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
                    ? (Brush)FindResource("GoodBrush")
                    : (Brush)FindResource("BadBrush");
            }
            else
            {
                block.Text = "stopped";
                block.Foreground = (Brush)FindResource("BadBrush");
            }
        }
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
        return string.Join(Environment.NewLine, lines);
    }

    // ── Config tabs ─────────────────────────────────────────────────────────

    private void BuildConfigTabs()
    {
        ConfigTabs.Items.Add(BuildTunerTab());
        ConfigTabs.Items.Add(BuildGemTab());
        ConfigTabs.Items.Add(BuildSpammerTab());
        ConfigTabs.Items.Add(BuildAttributesTab());
        ConfigTabs.Items.Add(BuildTunerCalibrateTab());
        ConfigTabs.Items.Add(BuildGemCalibrateTab());
        ConfigTabs.Items.Add(BuildSettingsTab());
    }

    private TabItem BuildTunerTab()
    {
        var panel = new StackPanel { Margin = new Thickness(8) };

        var targetGrade = MakeComboBox(Grades, _service.Config.Tuner.TargetGrade);
        panel.Children.Add(LabeledField("Target grade", targetGrade));

        var maxRetries = new TextBox { Text = _service.Config.Tuner.MaxRetries.ToString(CultureInfo.InvariantCulture) };
        panel.Children.Add(LabeledField("Max retries", maxRetries));

        var clickDelay = new TextBox { Text = _service.Config.Tuner.Timing.ClickEnterDelay.ToString(CultureInfo.InvariantCulture) };
        panel.Children.Add(LabeledField("Click delay (s)", clickDelay));

        var ocrDelay = new TextBox { Text = _service.Config.Tuner.Timing.OcrDelay.ToString(CultureInfo.InvariantCulture) };
        panel.Children.Add(LabeledField("OCR delay (s)", ocrDelay));

        var filterEnabled = new CheckBox
        {
            IsChecked = _service.Config.Tuner.Filter.Enabled,
            Content = "Filter enabled",
            Foreground = (Brush)FindResource("FgBrush"),
        };
        panel.Children.Add(filterEnabled);

        var matchMode = MakeComboBox(MatchModes, _service.Config.Tuner.Filter.MatchMode);
        panel.Children.Add(LabeledField("Match mode", matchMode));

        var requireGrade = MakeComboBox(RequireGradeOptions, GradeOrNone(_service.Config.Tuner.Filter.RequireGrade));
        panel.Children.Add(LabeledField("Require grade", requireGrade));

        var saveCaptures = new CheckBox
        {
            IsChecked = _service.Config.Tuner.SaveCaptures,
            Content = "Save OCR captures (debug only)",
            Foreground = (Brush)FindResource("FgBrush"),
        };
        panel.Children.Add(saveCaptures);

        var ruleRows = new List<RuleRow>();
        panel.Children.Add(new TextBlock { Text = "Rules (main goal)", FontWeight = FontWeights.SemiBold, Foreground = (Brush)FindResource("FgBrush"), Margin = new Thickness(0, 8, 0, 2) });
        var rulesEditor = BuildRulesEditor(_service.Config.Tuner.Filter.Rules, ruleRows, "+ Add Rule");
        panel.Children.Add(rulesEditor);

        var overrideRows = new List<RuleRow>();
        panel.Children.Add(new TextBlock { Text = "Override rules (stop immediately if matched)", FontWeight = FontWeights.SemiBold, Foreground = (Brush)FindResource("FgBrush"), Margin = new Thickness(0, 8, 0, 2) });
        var overrideEditor = BuildRulesEditor(_service.Config.Tuner.Filter.OverrideRules, overrideRows, "+ Add Override");
        panel.Children.Add(overrideEditor);

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
            _service.SaveConfig();
            MessageBox.Show("Tuner config saved.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
        };
        panel.Children.Add(save);

        var cleanup = MakeButton("Clean up captures", ControlAppearance.Secondary);
        cleanup.Click += (_, _) =>
        {
            var n = _service.CleanupCaptures();
            MessageBox.Show($"Deleted {n} capture image(s).", "Clean up", MessageBoxButton.OK, MessageBoxImage.Information);
        };
        panel.Children.Add(cleanup);

        return new TabItem { Header = "Tuner", Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto } };
    }

    private TabItem BuildGemTab()
    {
        var panel = new StackPanel { Margin = new Thickness(8) };

        var startGrade = MakeComboBox(GemGrades, _service.Config.Gem.StartGrade);
        panel.Children.Add(LabeledField("Start grade", startGrade));

        var save = MakeButton("Save Gem Config", ControlAppearance.Primary);
        save.Click += (_, _) =>
        {
            _service.Config.Gem.StartGrade = startGrade.SelectedItem?.ToString() ?? "N";
            _service.SaveConfig();
            MessageBox.Show("Gem config saved.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
        };
        panel.Children.Add(save);

        return new TabItem { Header = "Gem", Content = panel };
    }

    private TabItem BuildSpammerTab()
    {
        var panel = new StackPanel { Margin = new Thickness(8) };

        var keys = new TextBox
        {
            Text = SerializeKeys(_service.Config.Spammer.Keys),
            AcceptsReturn = true,
            Height = 110,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        };
        panel.Children.Add(LabeledField("Keys (key:seconds)", keys));

        var save = MakeButton("Save Spammer Config", ControlAppearance.Primary);
        save.Click += (_, _) =>
        {
            _service.Config.Spammer.Keys = ParseKeys(keys.Text);
            _service.SaveConfig();
            MessageBox.Show("Spammer config saved.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
        };
        panel.Children.Add(save);

        return new TabItem { Header = "Spammer", Content = panel };
    }

    private TabItem BuildAttributesTab()
    {
        var panel = new StackPanel { Margin = new Thickness(8) };

        panel.Children.Add(new TextBlock
        {
            Text = "OCR attribute dictionary — the item attributes the tuner can recognize and match against your filter rules. \"Name\" is what a filter rule matches on; \"OCR variants\" are the garbled forms OCR actually produces and auto-corrects to that name.",
            Foreground = (Brush)FindResource("MutedBrush"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 0, 0, 10),
        });

        var grid = new DataGrid
        {
            AutoGenerateColumns = false,
            IsReadOnly = true,
            CanUserAddRows = false,
            CanUserDeleteRows = false,
            HeadersVisibility = DataGridHeadersVisibility.Column,
            GridLinesVisibility = DataGridGridLinesVisibility.Horizontal,
            Background = (Brush)FindResource("CardBrush"),
            Foreground = (Brush)FindResource("FgBrush"),
            RowBackground = (Brush)FindResource("CardBrush"),
            BorderThickness = new Thickness(0),
        };

        grid.Columns.Add(new DataGridTextColumn { Header = "Name", Binding = new Binding("Name"), Width = new DataGridLength(160) });
        grid.Columns.Add(new DataGridTextColumn { Header = "Category", Binding = new Binding("Category"), Width = new DataGridLength(100) });
        grid.Columns.Add(new DataGridTextColumn { Header = "OCR variants", Binding = new Binding("Variants"), Width = new DataGridLength(1, DataGridLengthUnitType.Star) });

        grid.ItemsSource = _service.Attributes.Attributes
            .Select(a => new { Name = a.Name, Category = a.Category, Variants = string.Join(" / ", a.Variants) })
            .ToList();

        panel.Children.Add(grid);

        return new TabItem { Header = "Attributes", Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto } };
    }

    private TabItem BuildTunerCalibrateTab()
    {
        var hint = new TextBlock
        {
            Text = "Open the 發條 (tuning) window, capture, then drag three boxes: the grade letter, the 3 attribute lines, and the spring count.",
            Foreground = (Brush)FindResource("HighlightBrush"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 4, 0, 4),
        };
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

        var top = new StackPanel { Orientation = Orientation.Horizontal };
        top.Children.Add(capture);
        top.Children.Add(check);

        var panel = new StackPanel { Margin = new Thickness(8) };
        panel.Children.Add(hint);
        panel.Children.Add(top);
        panel.Children.Add(grid);
        panel.Children.Add(save);

        return new TabItem { Header = "Calibrate Tuner", Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto } };
    }

    private TabItem BuildGemCalibrateTab()
    {
        var hint = new TextBlock
        {
            Text = "Open the gem combine window, capture, then click each button in order.",
            Foreground = (Brush)FindResource("HighlightBrush"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 4, 0, 4),
        };
        _gemHint = hint;

        var image = new Image { Stretch = Stretch.Uniform };
        var canvas = new Canvas { Background = Brushes.Transparent, MinHeight = 300 };
        _gemImage = image;
        _gemCanvas = canvas;

        canvas.MouseDown += (_, e) => GemMouseDown(canvas, e.GetPosition(canvas));

        var grid = new Grid { Margin = new Thickness(0, 8, 0, 8) };
        grid.Children.Add(image);
        grid.Children.Add(canvas);

        var capture = MakeButton("Capture gem window", ControlAppearance.Primary);
        capture.Click += (_, _) => GemCapture();

        var save = MakeButton("Save Gem Composer", ControlAppearance.Primary);
        save.Click += (_, _) => GemSave();

        var panel = new StackPanel { Margin = new Thickness(8) };
        panel.Children.Add(hint);
        panel.Children.Add(capture);
        panel.Children.Add(grid);
        panel.Children.Add(save);

        return new TabItem { Header = "Calibrate Gem", Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto } };
    }

    // ── Tuner calibrator (drag an OCR box) ──────────────────────────────────

    private void TunerCapture()
    {
        var shot = CaptureScreenshot();
        if (shot == null)
        {
            _tunerHint!.Text = "Game window not found — open the game first.";
            return;
        }

        _tunerScreenshot = shot;
        _tunerImage!.Source = shot;
        _tunerStep = 0;
        _tunerGradeBox = null;
        _tunerAttrBox = null;
        _tunerRemainingBox = null;
        _tunerDragStart = null;
        _tunerMarquee = null;
        _tunerCanvas!.Children.Clear();
        _tunerHint!.Text = "Step 1/3 — drag a box around the grade letter (e.g. DG / G / N).";
    }

    private void TunerMouseDown(Canvas canvas, Point p)
    {
        if (_tunerScreenshot == null) return;
        if (_tunerStep >= 3) return;
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
                _tunerHint!.Text = "Step 2/3 — drag a box around the 3 attribute lines.";
                break;
            case 1:
                _tunerAttrBox = box;
                _tunerStep = 2;
                _tunerHint!.Text = "Step 3/3 — drag a box around the spring count (remaining).";
                break;
            default:
                _tunerRemainingBox = box;
                _tunerStep = 3;
                _tunerHint!.Text = "All bands set — click Check OCR to verify, then Save Tuner.";
                break;
        }
    }

    private void CheckTunerOcr()
    {
        if (_tunerGradeBox == null || _tunerAttrBox == null || _tunerRemainingBox == null)
        {
            _tunerHint!.Text = "Drag all three boxes first (grade, attributes, remaining).";
            return;
        }

        var ocr = BuildOcrGeometry(_tunerGradeBox.Value, _tunerAttrBox.Value, _tunerRemainingBox.Value);
        if (ocr.Region.Width < 20 || ocr.Region.Height < 20)
        {
            _tunerHint!.Text = "The boxes are too small — drag real rectangles.";
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

        var lines = new List<string> { $"Grade: {result.Grade ?? "?"}" };
        for (var i = 0; i < result.Attributes.Count; i++)
        {
            lines.Add($"Attr {i + 1}: {string.Join(" ", result.Attributes[i])}");
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
        _service.SaveLocal(local);
        // Refresh the in-memory config too, or the next tuner run still uses the
        // startup geometry (seeded from local.yaml.example) instead of the boxes
        // just calibrated — the cause of the "Check OCR is fine, run drifts" bug.
        _service.Config.Tuner.Ocr = ocr;
        _tunerHint!.Text = "Tuner saved to config\\local.yaml.";
        MessageBox.Show("Tuner calibration saved.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
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

    private void GemCapture()
    {
        var shot = CaptureScreenshot();
        if (shot == null)
        {
            _gemHint!.Text = "Game window not found — open the game first.";
            return;
        }

        _gemScreenshot = shot;
        _gemImage!.Source = shot;
        _gemPoints.Clear();
        _gemStep = 0;
        _gemCanvas!.Children.Clear();
        _gemHint!.Text = $"Click the \"{CalibGemSteps[0]}\" button.";
    }

    private void GemMouseDown(Canvas canvas, Point p)
    {
        if (_gemScreenshot == null || _gemCanvas == null || _gemStep >= CalibGemSteps.Length) return;
        _gemPoints[CalibGemSteps[_gemStep]] = CanvasToNatural(p, _gemScreenshot, _gemCanvas);
        AddDot(canvas, p);
        _gemStep++;
        _gemHint!.Text = _gemStep < CalibGemSteps.Length
            ? $"Click the \"{CalibGemSteps[_gemStep]}\" button."
            : "All points set — click Save.";
    }

    private void GemSave()
    {
        if (_gemStep < CalibGemSteps.Length)
        {
            _gemHint!.Text = "Click all five buttons first.";
            return;
        }

        var positions = new Dictionary<string, List<int>>();
        foreach (var key in CalibGemSteps)
        {
            var pt = _gemPoints[key];
            positions[key] = new List<int> { (int)pt.X, (int)pt.Y };
        }

        var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
        local.Gem = new ConfigLoader.LocalGem { GradePositions = positions };
        _service.SaveLocal(local);
        // Refresh in-memory config so the next gem run uses the just-calibrated
        // click points rather than the startup (example-seeded) ones.
        _service.Config.Gem.GradePositions = positions;
        _gemHint!.Text = "Gem Composer saved to config\\local.yaml.";
        MessageBox.Show("Gem calibration saved.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
    }

    // ── Shared calibrator helpers ───────────────────────────────────────────

    private BitmapSource? CaptureScreenshot()
    {
        var hwnd = WindowFinder.FindByTitle(_service.Config.Window.Title);
        var client = WindowFinder.GetClientRectInScreen(hwnd);
        if (client == null) return null;

        using var mat = ScreenCapture.Capture(client);
        return MatToBitmapSource(mat);
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

    private TabItem BuildSettingsTab()
    {
        var panel = new StackPanel { Margin = new Thickness(8) };

        panel.Children.Add(new TextBlock
        {
            Text = "Hotkeys. Type a key name: F1–F24, Esc, CapsLock, Space, Tab, Enter, or a single letter/digit.",
            Foreground = (Brush)FindResource("MutedBrush"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 0, 0, 10),
        });

        var startBox = new TextBox { Text = VkName(_service.Config.Hotkeys.Start) };
        var quitBox = new TextBox { Text = VkName(_service.Config.Hotkeys.Quit) };
        var gradeBox = new TextBox { Text = VkName(_service.Config.Hotkeys.AdvanceGrade) };
        var pauseBox = new TextBox { Text = VkName(_service.Config.Hotkeys.Pause) };

        panel.Children.Add(LabeledField("Start / stop rolling", startBox));
        panel.Children.Add(LabeledField("Quit (immediate)", quitBox));
        panel.Children.Add(LabeledField("Advance grade (gem)", gradeBox));
        panel.Children.Add(LabeledField("Pause (graceful stop)", pauseBox));

        var save = MakeButton("Save Hotkeys", ControlAppearance.Primary);
        save.Click += (_, _) =>
        {
            var s = ParseVk(startBox.Text);
            var q = ParseVk(quitBox.Text);
            var g = ParseVk(gradeBox.Text);
            var p = ParseVk(pauseBox.Text);
            if (s == 0 || q == 0 || g == 0 || p == 0)
            {
                MessageBox.Show("Invalid hotkey name.", "Error", MessageBoxButton.OK, MessageBoxImage.Warning);
                return;
            }
            _service.Config.Hotkeys.Start = s;
            _service.Config.Hotkeys.Quit = q;
            _service.Config.Hotkeys.AdvanceGrade = g;
            _service.Config.Hotkeys.Pause = p;
            _service.SaveConfig();
            MessageBox.Show("Hotkeys saved.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
        };
        panel.Children.Add(save);

        return new TabItem { Header = "Settings", Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto } };
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

    private Grid LabeledField(string label, FrameworkElement control)
    {
        var grid = new Grid { Margin = new Thickness(0, 4, 0, 4) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(180) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });

        var labelText = new TextBlock
        {
            Text = label,
            Foreground = (Brush)FindResource("MutedBrush"),
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
        Foreground = (Brush)FindResource("MutedBrush"),
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

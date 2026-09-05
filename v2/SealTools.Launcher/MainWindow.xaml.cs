using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
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

    private readonly LauncherService _service;
    private readonly Dictionary<string, TextBlock> _statusBlocks = new();
    private readonly DispatcherTimer _timer;

    private static readonly string[] CalibGemSteps = { "N", "G", "DG", "Register", "Combine" };
    private bool _disposed;

    // Tuner calibrator (drag an OCR box) state.
    private Image? _tunerImage;
    private Canvas? _tunerCanvas;
    private TextBlock? _tunerHint;
    private BitmapSource? _tunerScreenshot;
    private Rect? _tunerOcrBox;
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
                FontSize = 16,
                FontWeight = FontWeights.Bold,
            };

            var statusText = new TextBlock
            {
                Text = "stopped",
                Foreground = (Brush)FindResource("BadBrush"),
                FontSize = 12,
                TextWrapping = TextWrapping.Wrap,
                Margin = new Thickness(0, 4, 0, 0),
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
                CornerRadius = new CornerRadius(10),
                Padding = new Thickness(16),
                Margin = new Thickness(0, 0, 0, 10),
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
    }

    private TabItem BuildTunerTab()
    {
        var panel = new StackPanel();

        var targetGrade = MakeComboBox(Grades, _service.Config.Tuner.TargetGrade);
        panel.Children.Add(LabeledField("Target grade", targetGrade));

        var maxRetries = new TextBox { Text = _service.Config.Tuner.MaxRetries.ToString(CultureInfo.InvariantCulture) };
        panel.Children.Add(LabeledField("Max retries", maxRetries));

        var clickDelay = new TextBox { Text = _service.Config.Tuner.Timing.ClickEnterDelay.ToString(CultureInfo.InvariantCulture) };
        panel.Children.Add(LabeledField("Click delay (s)", clickDelay));

        var ocrDelay = new TextBox { Text = _service.Config.Tuner.Timing.OcrDelay.ToString(CultureInfo.InvariantCulture) };
        panel.Children.Add(LabeledField("OCR delay (s)", ocrDelay));

        var matchMode = MakeComboBox(MatchModes, _service.Config.Tuner.Filter.MatchMode);
        panel.Children.Add(LabeledField("Match mode", matchMode));

        var requireGrade = MakeComboBox(Grades, _service.Config.Tuner.Filter.RequireGrade);
        panel.Children.Add(LabeledField("Require grade", requireGrade));

        var filterEnabled = new CheckBox
        {
            IsChecked = _service.Config.Tuner.Filter.Enabled,
            Content = "Filter enabled",
            Foreground = (Brush)FindResource("FgBrush"),
        };
        panel.Children.Add(filterEnabled);

        var rules = new TextBox
        {
            Text = SerializeRules(_service.Config.Tuner.Filter.Rules),
            AcceptsReturn = true,
            Height = 70,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        };
        panel.Children.Add(LabeledField("Rules (name,count,min,max)", rules));

        var overrideRules = new TextBox
        {
            Text = SerializeRules(_service.Config.Tuner.Filter.OverrideRules),
            AcceptsReturn = true,
            Height = 70,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        };
        panel.Children.Add(LabeledField("Override rules", overrideRules));

        var save = MakeButton("Save Tuner Config", ControlAppearance.Primary);
        save.Click += (_, _) =>
        {
            var cfg = _service.Config;
            cfg.Tuner.TargetGrade = targetGrade.SelectedItem?.ToString() ?? "DG";
            if (int.TryParse(maxRetries.Text, out var mr)) cfg.Tuner.MaxRetries = mr;
            if (double.TryParse(clickDelay.Text, out var cd)) cfg.Tuner.Timing.ClickEnterDelay = cd;
            if (double.TryParse(ocrDelay.Text, out var od)) cfg.Tuner.Timing.OcrDelay = od;
            cfg.Tuner.Filter.MatchMode = matchMode.SelectedItem?.ToString() ?? "any";
            cfg.Tuner.Filter.RequireGrade = requireGrade.SelectedItem?.ToString();
            cfg.Tuner.Filter.Enabled = filterEnabled.IsChecked ?? false;
            cfg.Tuner.Filter.Rules = ParseRules(rules.Text);
            cfg.Tuner.Filter.OverrideRules = ParseRules(overrideRules.Text);
            _service.SaveConfig();
            MessageBox.Show("Tuner config saved.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
        };
        panel.Children.Add(save);

        return new TabItem { Header = "Tuner", Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto } };
    }

    private TabItem BuildGemTab()
    {
        var panel = new StackPanel();

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
        var panel = new StackPanel();

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
        var list = new ListBox { Foreground = (Brush)FindResource("FgBrush"), Background = (Brush)FindResource("CardBrush") };
        foreach (var attr in _service.Attributes.Attributes)
        {
            list.Items.Add($"{attr.Name}   ({attr.Category})");
        }

        return new TabItem { Header = "Attributes", Content = list };
    }

    private TabItem BuildTunerCalibrateTab()
    {
        var hint = new TextBlock
        {
            Text = "Open the 發條 (tuning) window, capture, then drag a box around the grade letter + 3 attribute lines.",
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

        var auto = MakeButton("Auto-anchor", ControlAppearance.Primary);
        auto.Click += (_, _) =>
        {
            try
            {
                _service.AutoAnchor();
                _tunerHint!.Text = "Auto-anchored — capture to verify.";
            }
            catch (Exception ex)
            {
                _tunerHint!.Text = ex.Message;
            }
        };

        var save = MakeButton("Save Tuner", ControlAppearance.Primary);
        save.Click += (_, _) => TunerSave();

        var check = MakeButton("Check OCR", ControlAppearance.Secondary);
        check.Click += (_, _) => CheckTunerOcr();

        var top = new StackPanel { Orientation = Orientation.Horizontal };
        top.Children.Add(capture);
        top.Children.Add(auto);
        top.Children.Add(check);

        var panel = new StackPanel();
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

        var panel = new StackPanel();
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
        _tunerOcrBox = null;
        _tunerDragStart = null;
        _tunerMarquee = null;
        _tunerCanvas!.Children.Clear();
        _tunerHint!.Text = "Drag a rectangle around the grade letter + 3 attribute lines.";
    }

    private void TunerMouseDown(Canvas canvas, Point p)
    {
        if (_tunerScreenshot == null) return;
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
        _tunerOcrBox = new Rect(
            new Point(Math.Min(a.X, b.X), Math.Min(a.Y, b.Y)),
            new Point(Math.Max(a.X, b.X), Math.Max(a.Y, b.Y)));
        _tunerMarquee = null;
        _tunerDragStart = null;
        _tunerHint!.Text = "OCR box selected — click Save Tuner.";
    }

    private void CheckTunerOcr()
    {
        if (_tunerOcrBox == null)
        {
            _tunerHint!.Text = "Drag the OCR box first.";
            return;
        }

        var ocr = BuildOcrGeometry(_tunerOcrBox.Value);
        if (ocr.Region.Width < 20 || ocr.Region.Height < 20)
        {
            _tunerHint!.Text = "The OCR box is too small — drag a real rectangle.";
            return;
        }

        _tunerHint!.Text = "Running OCR…";
        var result = _service.CheckOcr(ocr);
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
        if (_tunerOcrBox == null)
        {
            _tunerHint!.Text = "Drag the OCR box first.";
            return;
        }

        var ocr = BuildOcrGeometry(_tunerOcrBox.Value);
        if (ocr.Region.Width < 20 || ocr.Region.Height < 20)
        {
            _tunerHint!.Text = "The OCR box is too small — drag a real rectangle.";
            return;
        }

        var local = _service.LoadLocal() ?? new ConfigLoader.LocalOverrides();
        local.Tuner = new ConfigLoader.LocalTuner { Ocr = ocr };
        _service.SaveLocal(local);
        _tunerHint!.Text = "Tuner saved to config\\local.yaml.";
        MessageBox.Show("Tuner calibration saved.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
    }

    private static OcrGeometry BuildOcrGeometry(Rect box)
    {
        var w = box.Width;
        var h = box.Height;
        double R(double v) => Math.Round(v / 320.0 * h);
        double Cx(double v) => Math.Round(v / 300.0 * w);

        return new OcrGeometry
        {
            Region = new RegionConfig { Left = (int)box.Left, Top = (int)box.Top, Width = (int)w, Height = (int)h },
            GradeArea = new BoxConfig { X1 = (int)Cx(149), Y1 = (int)R(1), X2 = (int)Cx(232), Y2 = (int)R(44) },
            GradeY = new List<int> { (int)R(1), (int)R(44) },
            AttrY = new List<int> { (int)R(42), (int)R(140) },
            RemainingY = new List<int> { (int)R(190), (int)R(235) },
            RowHeight = Math.Max(1, (int)R(25)),
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

    private static UiButton MakeButton(string text, ControlAppearance appearance) =>
        new()
        {
            Content = text,
            Appearance = appearance,
            Margin = new Thickness(0, 10, 0, 0),
        };

    private static string SerializeRules(List<FilterRule> rules) =>
        string.Join("\n", rules.Select(r => FormattableString.Invariant($"{r.Name},{r.Count},{r.Min},{r.Max}")));

    private static List<FilterRule> ParseRules(string text)
    {
        var result = new List<FilterRule>();
        foreach (var line in text.Split('\n'))
        {
            var t = line.Trim();
            if (t.Length == 0) continue;

            var parts = t.Split(',');
            if (parts.Length < 2) continue;

            var rule = new FilterRule { Name = parts[0].Trim() };
            if (int.TryParse(parts[1].Trim(), out var count)) rule.Count = count;
            if (parts.Length > 2 && int.TryParse(parts[2].Trim(), out var min)) rule.Min = min;
            if (parts.Length > 3 && int.TryParse(parts[3].Trim(), out var max)) rule.Max = max;
            result.Add(rule);
        }
        return result;
    }

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

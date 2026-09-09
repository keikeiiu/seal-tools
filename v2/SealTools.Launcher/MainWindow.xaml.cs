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
using SealTools.Tuner;
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
    private readonly Dictionary<string, TextBlock> _statusBlocks = new();
    private readonly DispatcherTimer _timer;

    private static readonly string[] CalibGemSteps = { "N", "G", "DG", "Register", "Combine" };
    // The two composer move sets (Core/GemRoutes.cs): tuned counts vs closed-loop point placement.
    private static readonly string[] GemMoveModes = { "tuned", "arduino" };
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
    private System.Windows.Controls.Image? _gemResultPreview;

    public MainWindow()
    {
        InitializeComponent();

        _service = new LauncherService(FindRootDir());
        BuildToolCards();
        BuildConfigTabs();
        LoadCalibrationImages();

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
            startButton.Click += async (_, _) =>
            {
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
        if (!string.IsNullOrEmpty(state.Message)) lines.Add("⚠ " + state.Message);
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
        ConfigTabs.Items.Add(BuildArduinoTab());
        ConfigTabs.Items.Add(BuildSetupTab());
        ConfigTabs.Items.Add(BuildSettingsTab());
    }

    // Arduino connection status: a green/red light, the expected VID/PID, every serial port the
    // OS sees (with the matching one flagged), and a test click. Used to diagnose "Arduino not
    // found" without re-reading the code.
    private TabItem BuildArduinoTab()
    {
        var panel = new StackPanel { Margin = new Thickness(8) };

        var light = new Ellipse
        {
            Width = 14,
            Height = 14,
            Fill = (Brush)FindResource("BadBrush"),
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 8, 0),
        };
        var status = new TextBlock
        {
            Foreground = (Brush)FindResource("FgBrush"),
            FontWeight = FontWeights.SemiBold,
            TextWrapping = TextWrapping.Wrap,
        };
        var statusRow = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 0, 0, 6) };
        statusRow.Children.Add(light);
        statusRow.Children.Add(status);
        panel.Children.Add(statusRow);

        var detail = new TextBlock
        {
            Foreground = (Brush)FindResource("FgBrush"),
            FontFamily = new FontFamily("Consolas"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 0, 0, 4),
        };
        panel.Children.Add(detail);

        void Refresh()
        {
            var devices = _service.ArduinoDevices();
            var match = devices.FirstOrDefault(d => d.IsMatch);
            if (match != null)
            {
                light.Fill = (Brush)FindResource("GoodBrush");
                status.Text = $"Connected — {match.Name} ({match.Port})";
            }
            else
            {
                light.Fill = (Brush)FindResource("BadBrush");
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

        var testBtn = MakeButton("Test Click (C)", ControlAppearance.Primary);
        testBtn.Click += async (_, _) =>
        {
            var ser = await _service.ArduinoPortAsync();
            if (ser == null)
            {
                light.Fill = (Brush)FindResource("BadBrush");
                status.Text = "Not found — cannot send click";
                return;
            }
            try
            {
                GemPointer.Click(ser);
                light.Fill = (Brush)FindResource("GoodBrush");
                status.Text = "Sent click (C)";
            }
            catch (Exception ex)
            {
                light.Fill = (Brush)FindResource("BadBrush");
                status.Text = "Click failed: " + ex.Message;
            }
        };

        var buttonRow = new StackPanel { Orientation = Orientation.Horizontal };
        buttonRow.Children.Add(refreshBtn);
        buttonRow.Children.Add(testBtn);
        panel.Children.Add(buttonRow);

        Refresh();

        return new TabItem { Header = "Arduino", Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto } };
    }

    // Display environment: shows what the tools measure for this machine and stores the reference
    // a calibration was made against. Detect fills the fields; the user may override the scale and
    // the client size if detection is wrong (e.g. an unusual monitor arrangement).
    private TabItem BuildSetupTab()
    {
        var panel = new StackPanel { Margin = new Thickness(8) };

        panel.Children.Add(new TextBlock
        {
            Text = "Display environment. Coordinates are stored in PHYSICAL pixels relative to the game " +
                   "window's client area. Detect reads the current setup; the scale and reference client " +
                   "size stay editable if detection is wrong. If another machine's values differ from the " +
                   "stored calibration, recalibrate there (see docs/COORDINATES.md).",
            Foreground = (Brush)FindResource("MutedBrush"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 0, 0, 10),
        });

        var monitor = new TextBlock { Foreground = (Brush)FindResource("FgBrush"), FontFamily = new FontFamily("Consolas"), TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 0, 0, 2) };
        var window = new TextBlock { Foreground = (Brush)FindResource("FgBrush"), FontFamily = new FontFamily("Consolas"), TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 0, 0, 8) };
        var stored = new TextBlock { Foreground = (Brush)FindResource("MutedBrush"), TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 8, 0, 0) };
        var warning = new TextBlock { Foreground = (Brush)FindResource("HighlightBrush"), TextWrapping = TextWrapping.Wrap, Margin = new Thickness(0, 4, 0, 0) };
        panel.Children.Add(monitor);
        panel.Children.Add(window);

        var scaleBox = new TextBox { Width = 80, VerticalContentAlignment = VerticalAlignment.Center };
        var clientWBox = new TextBox { Width = 76, VerticalContentAlignment = VerticalAlignment.Center };
        var clientHBox = new TextBox { Width = 76, VerticalContentAlignment = VerticalAlignment.Center };
        panel.Children.Add(LabeledField("Scale (physical px per logical px)", scaleBox));

        var clientRow = new StackPanel { Orientation = Orientation.Horizontal };
        clientRow.Children.Add(clientWBox);
        clientRow.Children.Add(new TextBlock { Text = " × ", VerticalAlignment = VerticalAlignment.Center, Foreground = (Brush)FindResource("MutedBrush") });
        clientRow.Children.Add(clientHBox);
        panel.Children.Add(LabeledField("Reference client size (physical)", clientRow));

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

        var save = MakeButton("Save Setup", ControlAppearance.Primary);
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
            _service.SaveLocal(local);
            ShowStored();
            MessageBox.Show("Setup saved to config\\local.yaml.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
        };

        var buttons = new StackPanel { Orientation = Orientation.Horizontal };
        buttons.Children.Add(detect);
        buttons.Children.Add(save);
        panel.Children.Add(buttons);
        panel.Children.Add(stored);
        panel.Children.Add(warning);

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

        return new TabItem { Header = "Setup", Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto } };
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

        // What the composer does when the result window is empty (grade ran out of resources).
        var emptyMode = MakeComboBox(
            GemEmptyModes.Select(m => m.Label).ToList(),
            GemEmptyModes.First(m => m.Value == _service.Config.Gem.EmptyMode).Label);
        panel.Children.Add(LabeledField("On empty result", emptyMode));

        var saveEmptyCaptures = new CheckBox
        {
            IsChecked = _service.Config.Gem.SaveEmptyCaptures,
            Content = "Save empty-check captures (debug only)",
            Foreground = (Brush)FindResource("FgBrush"),
        };
        panel.Children.Add(saveEmptyCaptures);

        var save = MakeButton("Save Gem Config", ControlAppearance.Primary);
        save.Click += (_, _) =>
        {
            _service.Config.Gem.StartGrade = startGrade.SelectedItem?.ToString() ?? "N";
            var mode = GemEmptyModes.First(m => m.Label == emptyMode.SelectedItem?.ToString());
            _service.Config.Gem.EmptyMode = mode.Value;
            _service.Config.Gem.SaveEmptyCaptures = saveEmptyCaptures.IsChecked ?? false;
            _service.SaveConfig();
            MessageBox.Show("Gem config saved.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
        };
        panel.Children.Add(save);

        return new TabItem { Header = "Gem", Content = panel };
    }

    // One key + cooldown row in the spammer editor.
    private sealed class SpamKeyRow
    {
        public TextBox Key { get; } = new();
        public TextBox Delay { get; } = new();
    }

    private TabItem BuildSpammerTab()
    {
        var panel = new StackPanel { Margin = new Thickness(8) };

        panel.Children.Add(new TextBlock
        {
            Text = "Keys the spammer presses, each on its own cooldown. The Arduino supports digits 0–9 and " +
                   "F1–F10; prefix a key with * for the fast hold. Other keys are ignored (a warning appears " +
                   "on the tool card).",
            Foreground = (Brush)FindResource("MutedBrush"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 0, 0, 8),
        });

        // Named key sets — switching presets changes which rotation the spammer presses.
        var presets = _service.Config.Spammer.Presets;
        if (presets.Count == 0) presets["default"] = new Dictionary<string, double>();
        string current = presets.ContainsKey(_service.Config.Spammer.Active)
            ? _service.Config.Spammer.Active
            : presets.Keys.First();

        var rows = new List<SpamKeyRow>();
        var rowsPanel = new StackPanel();
        var presetBox = new ComboBox { MinWidth = 160, VerticalAlignment = VerticalAlignment.Center };
        var presetName = new TextBox { Width = 110, VerticalContentAlignment = VerticalAlignment.Center };

        void AddRow(string key, string delay)
        {
            var row = new SpamKeyRow();
            row.Key.Text = key;
            row.Key.Width = 80;
            row.Delay.Text = delay;
            row.Delay.Width = 70;

            var line = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 2, 0, 2) };
            line.Children.Add(FieldLabel("Key"));
            line.Children.Add(row.Key);
            line.Children.Add(FieldLabel("Delay (s)"));
            line.Children.Add(row.Delay);

            var del = new UiButton { Content = "✕", Appearance = ControlAppearance.Secondary, MinWidth = 28, Margin = new Thickness(8, 0, 0, 0) };
            del.Click += (_, _) => { rowsPanel.Children.Remove(line); rows.Remove(row); };
            line.Children.Add(del);

            rowsPanel.Children.Add(line);
            rows.Add(row);
        }

        void LoadRows(string name)
        {
            rowsPanel.Children.Clear();
            rows.Clear();
            if (!presets.TryGetValue(name, out var keys)) return;
            foreach (var kv in keys)
                AddRow(kv.Key, kv.Value.ToString(CultureInfo.InvariantCulture));
        }

        void RefreshPresetList(string select)
        {
            presetBox.ItemsSource = presets.Keys.OrderBy(k => k, StringComparer.OrdinalIgnoreCase).ToList();
            presetBox.SelectedItem = select;
        }

        bool loading = false;
        presetBox.SelectionChanged += (_, _) =>
        {
            if (loading || presetBox.SelectedItem is not string name || name == current) return;
            presets[current] = RowsToKeys(); // keep unsaved edits when switching
            current = name;
            LoadRows(name);
        };

        var status = new TextBlock
        {
            Foreground = (Brush)FindResource("MutedBrush"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 0, 0, 8),
        };

        var presetRow = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 0, 0, 4) };
        presetRow.Children.Add(new TextBlock { Text = "Preset ", VerticalAlignment = VerticalAlignment.Center, Foreground = (Brush)FindResource("MutedBrush") });
        presetRow.Children.Add(presetBox);
        presetRow.Children.Add(new TextBlock { Text = " name ", VerticalAlignment = VerticalAlignment.Center, Foreground = (Brush)FindResource("MutedBrush") });
        presetRow.Children.Add(presetName);
        var addPreset = MakeButton("+ New", ControlAppearance.Secondary);
        addPreset.Click += (_, _) =>
        {
            var name = presetName.Text.Trim();
            if (name.Length == 0) { status.Text = "Type a name first."; return; }
            if (presets.ContainsKey(name)) { status.Text = $"A preset named '{name}' already exists."; return; }
            presets[current] = RowsToKeys();
            presets[name] = new Dictionary<string, double>();
            current = name;
            loading = true; RefreshPresetList(name); loading = false;
            LoadRows(name);
            presetName.Text = "";
            status.Text = $"Added preset '{name}'.";
        };
        presetRow.Children.Add(addPreset);
        var renamePreset = MakeButton("Rename", ControlAppearance.Secondary);
        renamePreset.Click += (_, _) =>
        {
            var name = presetName.Text.Trim();
            if (name.Length == 0) { status.Text = "Type the new name first."; return; }
            if (name == current) { status.Text = "That is already the name."; return; }
            if (presets.ContainsKey(name)) { status.Text = $"A preset named '{name}' already exists."; return; }
            presets[current] = RowsToKeys();
            var keys = presets[current];
            presets.Remove(current);
            presets[name] = keys;
            var old = current;
            current = name;
            loading = true; RefreshPresetList(name); loading = false;
            presetName.Text = "";
            status.Text = $"Renamed '{old}' to '{name}'.";
        };
        presetRow.Children.Add(renamePreset);
        var delPreset = MakeButton("Delete", ControlAppearance.Secondary);
        delPreset.Click += (_, _) =>
        {
            if (presets.Count <= 1) { status.Text = "The last preset cannot be deleted."; return; }
            var gone = current;
            presets.Remove(current);
            current = presets.Keys.First();
            loading = true; RefreshPresetList(current); loading = false;
            LoadRows(current);
            status.Text = $"Deleted preset '{gone}'.";
        };
        presetRow.Children.Add(delPreset);
        panel.Children.Add(presetRow);
        panel.Children.Add(status);

        panel.Children.Add(rowsPanel);

        foreach (var kv in presets[current])
            AddRow(kv.Key, kv.Value.ToString(CultureInfo.InvariantCulture));

        var addButton = MakeButton("+ Add Key", ControlAppearance.Secondary);
        addButton.Click += (_, _) => AddRow("", "0.2");
        panel.Children.Add(addButton);

        Dictionary<string, double> RowsToKeys()
        {
            var result = new Dictionary<string, double>();
            foreach (var r in rows)
            {
                var k = r.Key.Text.Trim();
                if (k.Length == 0) continue;
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
            rows.Clear();
            foreach (var kv in parsed)
                AddRow(kv.Key, kv.Value.ToString(CultureInfo.InvariantCulture));
        }

        // Advanced: the same data as raw "key:seconds" lines, for setups the rows can't express.
        var advanced = new CheckBox
        {
            Content = "Advanced — edit the raw key:seconds list",
            Foreground = (Brush)FindResource("FgBrush"),
            Margin = new Thickness(0, 12, 0, 4),
        };
        panel.Children.Add(advanced);

        var raw = new TextBox
        {
            AcceptsReturn = true,
            Height = 110,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        };
        var rawPanel = new StackPanel { Visibility = Visibility.Collapsed };
        rawPanel.Children.Add(LabeledField("Keys (key:seconds)", raw));
        panel.Children.Add(rawPanel);

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

        var save = MakeButton("Save Spammer Config", ControlAppearance.Primary);
        save.Click += (_, _) =>
        {
            presets[current] = advanced.IsChecked == true ? ParseKeys(raw.Text) : RowsToKeys();
            _service.Config.Spammer.Active = current;
            _service.SaveConfig();
            MessageBox.Show($"Preset '{current}' saved.", "Saved", MessageBoxButton.OK, MessageBoxImage.Information);
        };
        panel.Children.Add(save);

        loading = true;
        RefreshPresetList(current);
        loading = false;

        return new TabItem { Header = "Spammer", Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto } };
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
            Text = "Open the gem combine window, capture, then click each button in order, and drag a box around the composed result gem.",
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
        canvas.MouseMove += (_, e) => GemMouseMove(canvas, e.GetPosition(canvas));
        canvas.MouseUp += (_, e) => GemMouseUp(canvas, e.GetPosition(canvas));

        var grid = new Grid { Margin = new Thickness(0, 8, 0, 8) };
        grid.Children.Add(image);
        grid.Children.Add(canvas);
        _gemGrid = grid;

        var capture = MakeButton("Capture gem window", ControlAppearance.Primary);
        capture.Click += (_, _) => GemCapture();

        // Diagnostics: report the window rects and save a sample capture.
        var diag = MakeButton("Diagnose capture", ControlAppearance.Secondary);
        diag.Click += (_, _) => DiagnoseCapture();
        var captureRow = new StackPanel { Orientation = Orientation.Horizontal };
        captureRow.Children.Add(capture);
        captureRow.Children.Add(diag);

        var save = MakeButton("Save Gem Composer", ControlAppearance.Primary);
        save.Click += (_, _) => GemSave();

        // Point/move tests and result tests as two simple horizontal rows (same clean look as the
        // Tuner tab). "from"/"to" drive the two move tests; the result-gem checks sit on their own
        // row below. Buttons keep their natural width but drop the top-10 margin so they line up
        // with the dropdowns.
        var testBox = new ComboBox { ItemsSource = GemTestPoints, SelectedIndex = 0, MinWidth = 130 };
        var testFromBox = new ComboBox { ItemsSource = GemTestPoints, SelectedIndex = 0, MinWidth = 130 };
        var testBtn = MakeButton("Test Click", ControlAppearance.Secondary);
        testBtn.Click += (_, _) => GemTestClick(testBox);
        var testMoveBtn = MakeButton("Test Move (rel)", ControlAppearance.Secondary);
        testMoveBtn.Click += (_, _) => GemTestRelativeMove(testFromBox, testBox);
        var checkColBtn = MakeButton("Check Result Colour", ControlAppearance.Secondary);
        checkColBtn.Click += (_, _) => GemCheckResultColor();
        var testGemBtn = MakeButton("Test Result Gem", ControlAppearance.Secondary);
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
        resultRow.Children.Add(debugCursorBtn);

        var debugPhysicalBtn = MakeButton("Debug Physical", ControlAppearance.Secondary);
        debugPhysicalBtn.Margin = new Thickness(6, 0, 6, 0);
        debugPhysicalBtn.Click += (_, _) => DebugCursorPath(testBox, physical: true);
        resultRow.Children.Add(debugPhysicalBtn);

        // Result-gem box crop preview: once the result box is dragged, show the exact region
        // being sampled for empty-detection, so the user can visually confirm it's over the
        // empty slot (and not an offset/mis-sized area).
        var resultPreview = new Image { Stretch = Stretch.Uniform, MaxWidth = 260, MaxHeight = 140, Margin = new Thickness(0, 4, 0, 0) };
        _gemResultPreview = resultPreview;
        var resultPreviewLabel = new TextBlock
        {
            Text = "Result gem box crop (empty-detection region):",
            Foreground = (Brush)FindResource("MutedBrush"),
            Margin = new Thickness(0, 10, 0, 0),
            TextWrapping = TextWrapping.Wrap,
        };
        var resultPreviewPanel = new StackPanel { Margin = new Thickness(0) };
        resultPreviewPanel.Children.Add(resultPreviewLabel);
        resultPreviewPanel.Children.Add(resultPreview);

        var panel = new StackPanel { Margin = new Thickness(8) };
        panel.Children.Add(hint);
        panel.Children.Add(captureRow);
        panel.Children.Add(grid);
        panel.Children.Add(save);

        // Editable coordinate fields (positions). Prefilled from the saved config so the user can
        // fix a point directly (e.g. N/G/DG all on the same vertical level) without re-capturing.
        panel.Children.Add(new TextBlock
        {
            Text = "Coordinates (edit directly, then Save Coordinates):",
            Foreground = (Brush)FindResource("HighlightBrush"),
            FontWeight = FontWeights.SemiBold,
            Margin = new Thickness(0, 14, 0, 4),
        });

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

        TextBox MakeCoordBox(string val) => new()
        {
            Width = 56,
            Text = val,
            VerticalContentAlignment = VerticalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 8, 6),
        };

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

        panel.Children.Add(coordGrid);

        var saveCoords = MakeButton("Save Coordinates", ControlAppearance.Primary);
        saveCoords.Click += (_, _) => SaveCoordinates();
        panel.Children.Add(saveCoords);

        // Position/result tests belong to the "Save Gem Composer" part.
        panel.Children.Add(pointRow);
        panel.Children.Add(resultRow);
        panel.Children.Add(resultPreviewPanel);

        // Divider: positions part (above) vs composer-moves part (below).
        panel.Children.Add(new Separator { Margin = new Thickness(0, 16, 0, 8) });

        // Composer move editor: one row per mandatory route on a shared Grid so the label / dx / dy /
        // Test columns all line up. dx/dy are the RAW counts the composer sends (no computation) —
        // they prefill from the current config, "Test" runs that exact move, and Save writes them back.
        panel.Children.Add(new TextBlock
        {
            Text = "Composer moves (raw dx dy — the composer sends these exact counts):",
            Foreground = (Brush)FindResource("HighlightBrush"),
            FontWeight = FontWeights.SemiBold,
            Margin = new Thickness(0, 4, 0, 4),
        });

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
        Place(moveGrid, new TextBlock { Text = "Test", FontWeight = headBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 0, RowGap) }, 0, 3);

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

            var testB = MakeButton("Test", ControlAppearance.Secondary);
            testB.Margin = new Thickness(0, 0, 0, RowGap); // align with the boxes, add only the row gap
            testB.Click += (_, _) => GemTestMovement(row.From, row.To, ParseMove(row.Dx), ParseMove(row.Dy));

            Place(moveGrid, new TextBlock { Text = route.Label, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, moveRow, 0);
            Place(moveGrid, dxBox, moveRow, 1);
            Place(moveGrid, dyBox, moveRow, 2);
            Place(moveGrid, testB, moveRow, 3);
            moveRow++;
        }
        panel.Children.Add(moveGrid);

        // Save ONLY the composer moves (raw dx/dy), independent of the full click-point
        // calibration that "Save Gem Composer" requires.
        var saveMoves = MakeButton("Save Composer Moves", ControlAppearance.Primary);
        saveMoves.Click += (_, _) => SaveComposerMoves();
        panel.Children.Add(saveMoves);

        // The NEW move set: the same routes, but each move places the cursor on the route's
        // destination POINT with the Arduino (closed loop) instead of sending tuned counts — the
        // same mechanism as Test Click. Nothing above is replaced; the composer picks the set with
        // gem.move_mode, and this section is how you try the new one on a live run first.
        panel.Children.Add(new Separator { Margin = new Thickness(0, 16, 0, 8) });
        panel.Children.Add(new TextBlock
        {
            Text = "New Gem Composer Moves (cursor placed on the destination point — no tuned counts):",
            Foreground = (Brush)FindResource("HighlightBrush"),
            FontWeight = FontWeights.SemiBold,
            Margin = new Thickness(0, 4, 0, 4),
        });

        _gemMoveMode = MakeComboBox(GemMoveModes, _service.Config.Gem.MoveMode);
        var modeRow = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(0, 0, 0, 6) };
        modeRow.Children.Add(new TextBlock
        {
            Text = "Composer move mode",
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(0, 0, 8, 0),
        });
        modeRow.Children.Add(_gemMoveMode);
        panel.Children.Add(modeRow);

        var arduinoGrid = new Grid { Margin = new Thickness(0, 0, 0, 6) };
        arduinoGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(200) }); // route label
        arduinoGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(130) }); // destination
        arduinoGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });     // Test btn
        for (int r = 0; r <= routes.Count; r++)
            arduinoGrid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        Place(arduinoGrid, new TextBlock { Text = "Move", FontWeight = headBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, 0, 0);
        Place(arduinoGrid, new TextBlock { Text = "Goes to", FontWeight = headBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, 0, 1);
        Place(arduinoGrid, new TextBlock { Text = "Test", FontWeight = headBold, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 0, RowGap) }, 0, 2);

        int arduinoRow = 1;
        foreach (var route in routes)
        {
            var testB = MakeButton("Test", ControlAppearance.Secondary);
            testB.Margin = new Thickness(0, 0, 0, RowGap);
            // Distinguishable in the accessibility tree (every row's visible label is "Test").
            System.Windows.Automation.AutomationProperties.SetName(testB, $"Test new move {route.Label}");
            testB.Click += (_, _) => GemTestArduinoRoute(route.From, route.To);

            Place(arduinoGrid, new TextBlock { Text = route.Label, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, arduinoRow, 0);
            Place(arduinoGrid, new TextBlock { Text = route.To, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, ColGap, RowGap) }, arduinoRow, 1);
            Place(arduinoGrid, testB, arduinoRow, 2);
            arduinoRow++;
        }
        panel.Children.Add(arduinoGrid);
        panel.Children.Add(new TextBlock
        {
            Text = "Test clicks the source point, then places the cursor on the destination point and clicks — " +
                   "the same closed-loop move the composer makes when move mode is \"arduino\".",
            TextWrapping = TextWrapping.Wrap,
            Foreground = (Brush)FindResource("TextFillColorSecondaryBrush"),
            Margin = new Thickness(0, 0, 0, 6),
        });

        return new TabItem { Header = "Calibrate Gem", Content = new ScrollViewer { Content = panel, VerticalScrollBarVisibility = ScrollBarVisibility.Auto } };
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

        var display = GemPointer.Display(_service.Config.Window.Title);
        if (display == null) { _gemHint!.Text = "Game window not found (or minimized) — open and restore the game first."; return; }
        var ser = await _service.ArduinoPortAsync();
        if (ser == null) { _gemHint!.Text = "Arduino not found — plug it in and retry."; return; }

        try
        {
            // v1 single-click core: place(from) -> C -> D dx dy -> C (no focus-click).
            var placed = GemPointer.To(ser, WindowFinder.ComputeCursorTarget(display, (int)from.Value.X, (int)from.Value.Y));
            if (!placed.Ok)
            {
                _gemHint!.Text = $"Couldn't place the cursor on \"{fromName}\" — {placed.Error}. Nothing was clicked.";
                return;
            }
            System.Threading.Thread.Sleep(300);
            GemPointer.Click(ser);
            System.Threading.Thread.Sleep(500);
            GemPointer.Move(ser, dx, dy);
            System.Threading.Thread.Sleep(300);
            GemPointer.Click(ser);
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

        var display = GemPointer.Display(_service.Config.Window.Title);
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
            var placed = GemPointer.To(ser, target);

            if (!placed.Ok)
            {
                _gemHint!.Text = $"Test click \"{name}\": couldn't place the cursor — {placed.Error}. Nothing was clicked.";
                LogCursorMove(name, target, placed);
                return;
            }

            System.Threading.Thread.Sleep(300);
            GemPointer.Click(ser);
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

        var display = GemPointer.Display(_service.Config.Window.Title);
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
            var placed = GemPointer.To(ser, WindowFinder.ComputeCursorTarget(display, (int)from.Value.X, (int)from.Value.Y));
            if (!placed.Ok)
            {
                _gemHint!.Text = $"Couldn't place the cursor on \"{fromName}\" — {placed.Error}. Nothing was clicked.";
                return;
            }
            System.Threading.Thread.Sleep(300);
            GemPointer.Click(ser);
            System.Threading.Thread.Sleep(500);
            GemPointer.Move(ser, mv[0], mv[1]);
            System.Threading.Thread.Sleep(300);
            GemPointer.Click(ser);
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

        var display = GemPointer.Display(_service.Config.Window.Title);
        if (display == null) { _gemHint!.Text = "Game window not found (or minimized) — open and restore the game first."; return; }
        var ser = await _service.ArduinoPortAsync();
        if (ser == null) { _gemHint!.Text = "Arduino not found — plug it in and retry."; return; }

        try
        {
            var placedFrom = GemPointer.To(ser, WindowFinder.ComputeCursorTarget(display, (int)from.Value.X, (int)from.Value.Y));
            if (!placedFrom.Ok)
            {
                _gemHint!.Text = $"Couldn't place the cursor on \"{fromName}\" — {placedFrom.Error}. Nothing was clicked.";
                LogRouteMove(fromName, toName, placedFrom, null);
                return;
            }
            System.Threading.Thread.Sleep(300);
            GemPointer.Click(ser);
            System.Threading.Thread.Sleep(500);

            var placedTo = GemPointer.To(ser, WindowFinder.ComputeCursorTarget(display, (int)target.Value.X, (int)target.Value.Y));
            if (!placedTo.Ok)
            {
                _gemHint!.Text = $"Couldn't place the cursor on \"{toName}\" — {placedTo.Error}. Stopped instead of clicking blind.";
                LogRouteMove(fromName, toName, placedFrom, placedTo);
                return;
            }
            System.Threading.Thread.Sleep(300);
            GemPointer.Click(ser);
            System.Threading.Thread.Sleep(300);

            _gemHint!.Text = $"New move: placed on \"{fromName}\", clicked, then placed on \"{toName}\", clicked. Does it land?";
            LogRouteMove(fromName, toName, placedFrom, placedTo);
        }
        catch (Exception ex)
        {
            _gemHint!.Text = $"Test move failed: {ex.Message}";
        }
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
        if (_tunerDisplay is { } d)
        {
            local.Calibration = ToCalibration(d);
            _service.Config.Calibration = local.Calibration;
        }
        _service.SaveLocal(local);
        // Refresh the in-memory config too, or the next tuner run still uses the
        // startup geometry (seeded from local.yaml.example) instead of the boxes
        // just calibrated — the cause of the "Check OCR is fine, run drifts" bug.
        _service.Config.Tuner.Ocr = ocr;
        SaveTunerCalibrationImage("calib_tuner.png");
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
            var hwnd = WindowFinder.FindByTitle(_service.Config.Window.Title);
            emptySig = GemColorAnalyzer.Analyze(hwnd, (int)rb.X, (int)rb.Y, (int)rb.Width, (int)rb.Height,
                _service.Config.Gem.ColoredGapMin);
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
        _service.SaveLocal(local);
        // Which move set the composer uses is a preference, not a coordinate — it lives in the
        // portable config (defaults.yaml) like gem.start_grade/empty_mode.
        _service.Config.Gem.MoveMode = _gemMoveMode?.SelectedItem as string ?? "tuned";
        _service.SaveConfig();
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
        _service.SaveLocal(local);

        _gemHint!.Text = "Coordinates saved to config\\local.yaml.";
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
        _service.SaveLocal(local);

        _gemHint!.Text = "Composer moves saved to config\\local.yaml.";
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

        var display = GemPointer.Display(_service.Config.Window.Title);
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

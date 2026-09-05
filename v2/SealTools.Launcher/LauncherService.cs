using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using SealTools.Core;
using SealTools.Core.Config;
using SealTools.Spammer;
using SealTools.Tuner;
using GemComposerTool = SealTools.GemComposer.GemComposer;

namespace SealTools.Launcher;

/// <summary>
/// Owns the config, the attribute dictionary, and the single running tool (one Arduino COM port).
/// Control is in-memory (CancellationToken), state is a shared <see cref="ToolState"/>.
/// </summary>
public sealed class LauncherService : IDisposable
{
    private readonly string _rootDir;
    private readonly ConfigLoader _loader;
    private CancellationTokenSource? _cts;
    private ToolState? _state;
    private string? _currentId;

    public LauncherService(string rootDir)
    {
        _rootDir = rootDir;
        _loader = new ConfigLoader(Path.Combine(rootDir, "config"));
        Config = _loader.Load();
        Attributes = _loader.LoadAttributes();
    }

    /// <summary>The merged config (portable + local). Mutable so edits are reflected live by the tools.</summary>
    public AppConfig Config { get; }

    /// <summary>The OCR attribute dictionary from attributes.yaml.</summary>
    public AttributesConfig Attributes { get; }

    /// <summary>The id of the currently-launched tool, or null when idle.</summary>
    public string? CurrentId => _currentId;

    /// <summary>The live state of the currently-launched tool, or null when idle.</summary>
    public ToolState? CurrentState => _state;

    /// <summary>Launches a tool (stopping the current one first) and starts it rolling.</summary>
    public void StartTool(string id)
    {
        if (id is not ("tuner" or "gem" or "spammer"))
        {
            throw new ArgumentException($"Unknown tool id: {id}", nameof(id));
        }

        StopTool();
        _currentId = id;
        _cts = new CancellationTokenSource();
        _state = new ToolState { Running = true };
        var ct = _cts.Token;
        var state = _state;

        _ = Task.Run(() =>
        {
            try
            {
                RunTool(id, state, ct);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"[!] {id} crashed: {ex.Message}");
                state.Running = false;
            }
        });
    }

    /// <summary>Stops the current tool and releases the Arduino COM port.</summary>
    public void StopTool()
    {
        if (_cts == null)
        {
            return;
        }

        _cts.Cancel();
        _cts.Dispose();
        _cts = null;
        _state = null;
        _currentId = null;
        Thread.Sleep(300); // let the COM port release
    }

    /// <summary>Persists the portable config (defaults.yaml). Machine coords are written separately by the calibrator.</summary>
    public void SaveConfig() => _loader.SaveDefaults(Config);

    /// <summary>Persists machine-specific coordinates (local.yaml) written by the calibrator.</summary>
    public void SaveLocal(ConfigLoader.LocalOverrides local) => _loader.SaveLocal(local);

    /// <summary>Loads the machine-specific overlay, or null when local.yaml is absent.</summary>
    public ConfigLoader.LocalOverrides? LoadLocal() => _loader.LoadLocal();

    /// <summary>Scales the machine-specific coords to the current window size (auto-anchor first guess).</summary>
    public void AutoAnchor()
    {
        var hwnd = WindowFinder.FindByTitle(Config.Window.Title);
        var client = WindowFinder.GetClientRectInScreen(hwnd);
        if (client == null)
        {
            throw new InvalidOperationException("Game window not found.");
        }

        var scaleX = (double)client.Width / Config.ReferenceWindow.Width;
        var scaleY = (double)client.Height / Config.ReferenceWindow.Height;
        var local = LoadLocal() ?? new ConfigLoader.LocalOverrides();
        if (local.Tuner?.Ocr != null) Calibration.ScaleOcr(local.Tuner.Ocr, scaleX, scaleY);
        if (local.Gem?.GradePositions != null) Calibration.ScalePositions(local.Gem.GradePositions, scaleX, scaleY);
        SaveLocal(local);
    }

    private int RunTool(string id, ToolState state, CancellationToken ct) => id switch
    {
        "tuner" => new SealTuner(Config, Attributes, _rootDir).Run(state, ct),
        "gem" => new GemComposerTool(Config).Run(state, ct),
        "spammer" => new SkillSpammer(Config).Run(state, ct),
        _ => 1,
    };

    public void Dispose() => StopTool();
}

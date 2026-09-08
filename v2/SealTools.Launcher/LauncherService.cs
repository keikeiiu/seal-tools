using System;
using System.IO;
using System.IO.Ports;
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
    private Task? _toolTask;
    private ToolState? _state;
    private string? _currentId;
    private OcrEngine? _diagnosticOcr;
    private SerialPort? _arduino;

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

    /// <summary>Enumerates the serial ports the OS sees, flagging any matching the configured
    /// Arduino VID/PID. Used by the "Arduino" status tab for connection diagnostics.</summary>
    public List<ArduinoDevice> ArduinoDevices() => Arduino.Diagnose(Config.Arduino.Vid, Config.Arduino.Pid);

    /// <summary>Gets the shared Arduino serial port, opening it once (with a boot delay) if needed.
    /// Returns null when the Arduino isn't found. Tools and the calibrate test buttons both use this
    /// single open port, so it is never opened twice (which was causing "COM port denied").</summary>
    public SerialPort? ArduinoPort()
    {
        if (_arduino is { IsOpen: true }) return _arduino;
        var port = Arduino.Find(Config.Arduino.Vid, Config.Arduino.Pid);
        if (port == null) return null;
        _arduino = Arduino.Open(port, Config.Arduino.Baud);
        Thread.Sleep(2000); // one-time boot delay after the serial open
        return _arduino;
    }

    /// <summary>Launches a tool (stopping the current one first) and starts it rolling.</summary>
    public void StartTool(string id)
    {
        if (id is not ("tuner" or "gem" or "spammer"))
        {
            throw new ArgumentException($"Unknown tool id: {id}", nameof(id));
        }

        StopTool();
        var ser = ArduinoPort();
        if (ser == null)
        {
            Console.WriteLine("[!] Arduino not found");
            return;
        }
        _currentId = id;
        _cts = new CancellationTokenSource();
        _state = new ToolState { Running = true };
        var ct = _cts.Token;
        var state = _state;

        _toolTask = Task.Run(() =>
        {
            try
            {
                RunTool(id, ser, state, ct);
            }
            catch (Exception ex)
            {
                Console.WriteLine($"[!] {id} crashed: {ex.Message}");
                try { File.AppendAllText(Path.Combine(_rootDir, "logs", "error.log"), $"{DateTime.Now:yyyy-MM-dd HH:mm:ss}: {ex}\n\n"); } catch { }
                state.Running = false;
            }
        });
    }

    /// <summary>Stops the current tool and releases the Arduino COM port.</summary>
    public void StopTool()
    {
        var cts = _cts;
        var task = _toolTask;
        if (cts == null)
        {
            return;
        }

        // Null the fields first so a re-entrant call (e.g. StartTool -> StopTool) sees no
        // current tool and doesn't double-cancel.
        _cts = null;
        _toolTask = null;
        _state = null;
        _currentId = null;

        cts.Cancel();

        // Dispose the CTS only after the tool thread has fully exited — the tool loop reads
        // ct.IsCancellationRequested, which throws ObjectDisposedException on a disposed CTS.
        task?.ContinueWith(
            _ => cts.Dispose(),
            CancellationToken.None,
            TaskContinuationOptions.ExecuteSynchronously,
            TaskScheduler.Default);
    }

    /// <summary>Persists the portable config (defaults.yaml). Machine coords are written separately by the calibrator.</summary>
    public void SaveConfig() => _loader.SaveDefaults(Config);

    /// <summary>Persists machine-specific coordinates (local.yaml) written by the calibrator.</summary>
    public void SaveLocal(ConfigLoader.LocalOverrides local) => _loader.SaveLocal(local);

    /// <summary>Loads the machine-specific overlay, or null when local.yaml is absent.</summary>
    public ConfigLoader.LocalOverrides? LoadLocal() => _loader.LoadLocal();

    /// <summary>Runs the OCR engine against the given geometry (used by the calibrator's "Check OCR" preview).</summary>
    public ScanResult? CheckOcr(OcrGeometry ocr)
    {
        _diagnosticOcr ??= new OcrEngine(Config, Attributes, _rootDir);
        return _diagnosticOcr.Scan(ocr);
    }

    /// <summary>Deletes the debug OCR capture images (logs/captures/*.png). Returns the number removed.</summary>
    public int CleanupCaptures()
    {
        var dir = Path.Combine(_rootDir, "logs", "captures");
        if (!Directory.Exists(dir)) return 0;
        var n = 0;
        foreach (var f in Directory.EnumerateFiles(dir, "*.png", SearchOption.TopDirectoryOnly))
        {
            try { File.Delete(f); n++; } catch { /* file locked — skip */ }
        }
        return n;
    }

    private int RunTool(string id, SerialPort ser, ToolState state, CancellationToken ct) => id switch
    {
        "tuner" => new SealTuner(Config, Attributes, _rootDir).Run(ser, state, ct),
        "gem" => new GemComposerTool(Config).Run(ser, state, ct),
        "spammer" => new SkillSpammer(Config).Run(ser, state, ct),
        _ => 1,
    };

    public void Dispose()
    {
        try
        {
            _diagnosticOcr?.Dispose();
            StopTool();
        }
        finally
        {
            // Always release the COM port, even if an earlier dispose step throws — otherwise a
            // half-closed process lingers holding the Arduino and the next launch can't open it.
            try { _arduino?.Dispose(); }
            catch { /* never let port teardown throw */ }
            _arduino = null;
        }
    }
}

using System;
using System.IO;
using System.IO.Ports;
using System.Threading;
using System.Threading.Tasks;
using SealTools.Core;
using SealTools.Core.Config;
using SealTools.Pet;
using SealTools.Shop;
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
    /// <summary>True while StartToolAsync is between its first line and the tool actually running.</summary>
    private bool _startInProgress;
    /// <summary>Set by StopTool when nothing is running yet but a start is in flight, so the start
    /// bails when it resumes rather than launching a tool the user already asked to stop.</summary>
    private bool _startCancelled;

    /// <summary>The buy preset the next buy run uses. The Buy tab sets it when Start is pressed;
    /// RunTool only receives a tool id, so the choice has to arrive some other way.</summary>
    public string? PendingBuyPreset { get; set; }

    /// <summary>How many to buy this run. Separate from the preset because the preset's own count is
    /// only its usual amount — sometimes you want a different number, and that should not mean editing
    /// the item. Set from the card at Start.</summary>
    public int PendingBuyCount { get; set; } = 1;
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

    /// <summary>Why the last <see cref="ArduinoPortAsync"/> call failed, or null on success.
    /// Surfaced to the user because the launcher has no console in the published WinExe.</summary>
    public string? LastArduinoError { get; private set; }

    /// <summary>Gets the shared Arduino serial port, opening it once (with a boot delay) if needed.
    /// Returns null when the Arduino isn't found or can't be opened (see <see cref="LastArduinoError"/>).
    /// Tools and the calibrate test buttons both use this single open port, so it is never opened twice
    /// (which was causing "COM port denied").</summary>
    public async Task<SerialPort?> ArduinoPortAsync()
    {
        LastArduinoError = null;
        if (_arduino is { IsOpen: true }) return _arduino;
        var port = Arduino.Find(Config.Arduino.Vid, Config.Arduino.Pid);
        if (port == null)
        {
            LastArduinoError = $"Arduino not found (VID 0x{Config.Arduino.Vid:X4}, " +
                $"PID {string.Join("/", Config.Arduino.Pid.Select(p => $"0x{p:X4}"))}). " +
                "Check the Arduino tab.";
            return null;
        }

        try
        {
            // A stale handle from a previous open is never reused.
            _arduino?.Dispose();
            _arduino = Arduino.Open(port, Config.Arduino.Baud);
        }
        catch (Exception ex)
        {
            _arduino = null;
            LastArduinoError = $"Could not open {port}: {ex.Message}";
            return null;
        }

        await Task.Delay(2000); // one-time boot delay after the serial open (non-blocking)
        return _arduino;
    }

    /// <summary>Launches a tool (stopping the current one first) and starts it rolling.
    /// Returns false when the Arduino can't be opened, so the caller can tell the user
    /// instead of the click silently doing nothing. A second call while a start is already in
    /// flight returns true without starting anything — see the guard below.</summary>
    public async Task<bool> StartToolAsync(string id)
    {
        if (id is not ("tuner" or "gem" or "spammer" or "holdspace" or "buy" or "sell" or "pet"))
        {
            throw new ArgumentException($"Unknown tool id: {id}", nameof(id));
        }

        // One start at a time. ArduinoPortAsync waits out a 2 s boot delay on a cold start, and its
        // fast path returns an already-open port with no delay at all — so two Start clicks could
        // both get past StopTool() while it still saw _cts == null. The second would then overwrite
        // _cts and _toolTask and orphan the first loop, which kept writing to the shared serial port
        // with no way to stop it while the UI showed the other tool. A click during a start is a
        // no-op, and reports no error.
        if (_startInProgress) return true;
        _startInProgress = true;
        _startCancelled = false;
        try
        {
            return await StartToolCoreAsync(id);
        }
        finally
        {
            _startInProgress = false;
        }
    }

    private async Task<bool> StartToolCoreAsync(string id)
    {
        var stopped = StopTool(fromStart: true);
        if (stopped != null)
        {
            // Wait for the previous tool loop to leave the shared serial port before this one
            // starts writing to it — otherwise their byte streams can interleave. Bounded, so a
            // wedged tool can't hang the UI's Start click.
            await Task.WhenAny(stopped, Task.Delay(3000));
        }

        var ser = await ArduinoPortAsync();
        // A Stop clicked while the port was opening has nothing to cancel (no _cts yet), so it set
        // _startCancelled instead. Honour it rather than starting the tool anyway.
        if (ser == null || _startCancelled)
        {
            return false;
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
                // The tool card is the only place the user can see this — the published WinExe has no
                // console — so the reason goes on the state, not just the log. Without it a crashed
                // tool rendered as "paused" with no explanation.
                state.Message = $"{id} stopped: {ex.Message}";
                state.Running = false;
                Console.WriteLine($"[!] {id} crashed: {ex}");
                try
                {
                    var logDir = Path.Combine(_rootDir, "logs");
                    // Create it here: nothing else makes this directory that early, so a crash before
                    // the first capture or calibration used to fail the append and record nothing.
                    Directory.CreateDirectory(logDir);
                    File.AppendAllText(Path.Combine(logDir, "error.log"),
                        $"{DateTime.Now:yyyy-MM-dd HH:mm:ss}: {ex}\n\n");
                }
                catch { /* the card message above is the part that matters */ }
            }
        });
        return true;
    }

    /// <summary>Stops the current tool and releases the Arduino COM port. Returns a task that
    /// completes once the tool loop has exited and its CTS is disposed — await it before starting
    /// another tool, since all tools share one serial port. Null when nothing was running.</summary>
    public Task? StopTool(bool fromStart = false)
    {
        var cts = _cts;
        var task = _toolTask;
        if (cts == null)
        {
            // Nothing running, but a start may be sitting on the port wait with no tool installed
            // yet. Flag it so the start bails on resume, instead of launching a tool the user has
            // already asked to stop. StartToolAsync's own StopTool call passes fromStart: true, or
            // it would cancel itself here.
            if (_startInProgress && !fromStart) _startCancelled = true;
            return null;
        }

        // Null the fields first so a re-entrant call (e.g. StartTool -> StopTool) sees no
        // current tool and doesn't double-cancel.
        _cts = null;
        _toolTask = null;
        _state = null;
        _currentId = null;

        cts.Cancel();

        if (task == null)
        {
            cts.Dispose();
            return null;
        }

        // Dispose the CTS only after the tool thread has fully exited — the tool loop reads
        // ct.IsCancellationRequested, which throws ObjectDisposedException on a disposed CTS.
        return task.ContinueWith(
            _ => cts.Dispose(),
            CancellationToken.None,
            TaskContinuationOptions.ExecuteSynchronously,
            TaskScheduler.Default);
    }

    /// <summary>Persists the portable config (defaults.yaml). Machine coords are written separately by the calibrator.</summary>
    public void SaveConfig() => _loader.SaveDefaults(Config);

    /// <summary>Why the calibrated bag grid and its single-slot box disagree, or null when they
    /// agree. The calibrator shows this while dragging, and refuses to save on it — the whole grid
    /// derivation assumes the slots are uniform, and the single slot is the only evidence that they
    /// are. Null also means "both are set and consistent".</summary>
    public string? GridCheck()
    {
        var bs = Config.BuySell;
        if (!Core.BagGrid.IsValidRect(bs.BagGrid)) return "the grid area isn't set";
        if (!Core.BagGrid.IsValidRect(bs.BagSlot)) return "the slot box isn't set";
        return Core.BagGrid.Disagreement(bs.BagGrid!, bs.BagSlot!);
    }

    /// <summary>Releases the spacebar directly, independent of the Hold Space tool's loop — so a stop
    /// click can never leave the key stuck down.</summary>
    public void ReleaseSpace() { try { _arduino?.Write("U\n"); } catch { } }

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
        "holdspace" => new HoldSpace(Config).Run(ser, state, ct),
        "buy" => new ShopTool(Config, ShopMode.Buy, PendingBuyPreset, PendingBuyCount).Run(ser, state, ct),
        "sell" => new ShopTool(Config, ShopMode.Sell, null).Run(ser, state, ct),
        "tuner" => new SealTuner(Config, Attributes, _rootDir).Run(ser, state, ct),
        "gem" => new GemComposerTool(Config, _rootDir).Run(ser, state, ct),
        "spammer" => new SkillSpammer(Config).Run(ser, state, ct),
        "pet" => new PetTool(Config).Run(ser, state, ct),
        _ => 1,
    };

    public void Dispose()
    {
        try
        {
            _diagnosticOcr?.Dispose();
            var stopped = StopTool();
            // Let the tool loop leave the port before we close it, or its last write throws on a
            // disposed SerialPort and gets logged as a spurious crash. Bounded — shutdown wins.
            try { stopped?.Wait(TimeSpan.FromSeconds(3)); } catch { /* the tool logs its own failure */ }
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

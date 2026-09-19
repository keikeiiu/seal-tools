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
/// Owns the config, the attribute dictionary, and the running tools (one Arduino COM port).
/// Control is in-memory (CancellationToken), state is a shared <see cref="ToolState"/>.
/// </summary>
public sealed class LauncherService : IDisposable
{
    /// <summary>One tool's live run. These were four parallel fields (`_cts` / `_toolTask` / `_state` /
    /// `_currentId`), which quietly encoded "there is only ever one tool" in the shape of the class
    /// rather than in a rule anyone could read. The pet feeder becoming resident breaks that, so the
    /// four travel together now — a stop is then "cancel this one" instead of "null all of them", and
    /// the question "is THAT tool still running?" is answerable.
    ///
    /// <see cref="Task"/> is assigned immediately after the record is registered, in the same
    /// synchronous block, so a stop can never observe the record without it — see the note where the
    /// start builds one.</summary>
    private sealed class RunningTool
    {
        public required string Id { get; init; }
        public required CancellationTokenSource Cts { get; init; }
        public required ToolState State { get; init; }
        public Task Task { get; set; } = Task.CompletedTask;
    }

    private readonly string _rootDir;
    private readonly ConfigLoader _loader;
    private readonly Dictionary<string, RunningTool> _running = new();
    /// <summary>Which running tool the UI calls "current". The others are running but not it — the
    /// resident pet feeder, once it stops being killed by every other Start.</summary>
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

    /// <summary>The id of the tool the UI calls current, or null when there is none.</summary>
    public string? CurrentId => _currentId;

    /// <summary>The live state of the current tool, or null when there is none.</summary>
    public ToolState? CurrentState => _currentId is { } id ? StateFor(id) : null;

    /// <summary>The live state of one tool, or null when that tool is not running. The card loop asks
    /// this per id rather than reading one "current" tool, because the pet feeder can be running while
    /// another tool is — and a card that read "stopped" while its tool was feeding pets is exactly the
    /// kind of lie this replaces.</summary>
    public ToolState? StateFor(string id) => _running.TryGetValue(id, out var r) ? r.State : null;

    /// <summary>Whether that tool is running right now.</summary>
    public bool IsRunning(string id) => _running.ContainsKey(id);

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
            // A failed open is not a board that has told us anything; a report left over from an
            // earlier port must not be shown as though it were about this one.
            FirmwareLevel = null;
            FirmwareReport = null;
            LastArduinoError = $"Could not open {port}: {ex.Message}";
            return null;
        }

        await Task.Delay(2000); // one-time boot delay after the serial open (non-blocking)

        // Ask the board what it is running, once per open — the one moment nothing else can be
        // writing, because StartToolCoreAsync has already waited for the previous tool to leave.
        // Off the dispatcher: ReadLine blocks its thread until the timeout, and a frozen window on
        // every cold start is not a price worth paying for a diagnostic.
        FirmwareLevel = await Task.Run(() => QueryFirmwareVersion(_arduino));
        FirmwareReport = FirmwareVersion.Describe(FirmwareLevel);
        Console.WriteLine($"[arduino] firmware {FirmwareReport}");
        return _arduino;
    }

    /// <summary>The firmware protocol level the board reported when the port was opened, or null when
    /// it reported nothing — see <see cref="FirmwareVersion"/> for why the silence is an answer and
    /// not a failed read. Also null before the port has been opened at all.</summary>
    public int? FirmwareLevel { get; private set; }

    /// <summary>The sentence to show for <see cref="FirmwareLevel"/>, or null when the board has not
    /// been asked yet.</summary>
    public string? FirmwareReport { get; private set; }

    /// <summary>How long to wait for the board's reply before asking again, and then before giving up.
    /// The sketch's `setup()` sits in a 3 s delay while the launcher's boot wait is only 2 s, so on a
    /// board that has just been flashed or plugged in the first `V` can land before `loop()` is
    /// running. The bytes are not lost — they wait in the USB CDC buffer — but the reply is late, so
    /// the window has to reach past that 3 s mark or a board that is merely slow to start reads as
    /// an old one, which is the exactly wrong conclusion from the right evidence.</summary>
    private const int FirmwareReadMs = 1500;

    /// <summary>Sends `V` and reads one line back, twice. Null means the board did not answer, which
    /// for this command is the answer: the sketch that predates it writes nothing at all.</summary>
    private static int? QueryFirmwareVersion(SerialPort? port)
    {
        if (port is not { IsOpen: true }) return null;

        for (var attempt = 0; attempt < 2; attempt++)
        {
            try
            {
                port.Write(FirmwareVersion.Query + "\n");
            }
            catch
            {
                // The port went away mid-query. Nothing to report as a version, and the tool start
                // that follows will fail on its own write with a real message.
                return null;
            }

            if (ReadLineQuietly(port, FirmwareReadMs) is { } line && FirmwareVersion.Parse(line) is { } level)
            {
                return level;
            }
        }

        return null;
    }

    /// <summary>ReadLine with the exception turned into a null, on a timeout that does not outlive the
    /// call — the port's own ReadTimeout is 1 s and is left as it was found.</summary>
    private static string? ReadLineQuietly(SerialPort port, int timeoutMs)
    {
        var was = port.ReadTimeout;
        try
        {
            port.ReadTimeout = timeoutMs;
            return port.ReadLine();
        }
        catch
        {
            // TimeoutException for the silence this exists to detect; anything else is a port that
            // has gone away, which reads the same way — no version.
            return null;
        }
        finally
        {
            try { port.ReadTimeout = was; } catch { /* port closed meanwhile */ }
        }
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
        // Everything that has to give way to this start, stopped before anything else is touched.
        // Today that is everything; see StopAll.
        foreach (var stopping in StopAll())
        {
            // Wait for each loop to leave the shared serial port before this one starts writing to
            // it — otherwise their byte streams can interleave. Bounded, so a wedged tool can't hang
            // the UI's Start click.
            await Task.WhenAny(stopping, Task.Delay(3000));
        }

        var ser = await ArduinoPortAsync();
        // A Stop clicked while the port was opening has nothing to cancel (no _cts yet), so it set
        // _startCancelled instead. Honour it rather than starting the tool anyway.
        if (ser == null || _startCancelled)
        {
            return false;
        }
        _currentId = id;
        // Starting a hold takes the spacebar over deliberately, so any standing "the release failed"
        // warning describes a state that no longer applies — it would otherwise sit red on the status
        // line for the rest of the session, after the player had already dealt with it.
        if (id == "holdspace") LastSpaceReleaseError = null;

        var cts = new CancellationTokenSource();
        var state = new ToolState { Running = true };
        var ct = cts.Token;
        var entry = new RunningTool { Id = id, Cts = cts, State = state };

        // Registered BEFORE the loop is started, and with no await between the two, so a Stop from
        // the UI cannot fall between them and miss a tool that is already running. (The whole block
        // is synchronous on the dispatcher thread, which is what makes that safe.)
        _running[id] = entry;
        entry.Task = Task.Run(() =>
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

    /// <summary>Stops every running tool but <paramref name="keeper"/>. Returns one task per stopped
    /// tool, each completed once that loop has left the port.
    ///
    /// The start path wants everything gone (today <paramref name="keeper"/> is always null, which is
    /// what keeps the current behaviour: pressing Start on any tool stops whatever was running). A
    /// previous instance of the SAME tool counts — pressing Start on a tool that is already running
    /// restarts it, and leaving the old loop alive would orphan it writing to the shared port with no
    /// way to stop it. Shutdown wants everything gone without exception. The pet feeder has to be a
    /// keeper on the start path and not at shutdown, and this is the single expression of that rule.
    ///
    /// Materialised, and every stop issued before any of them is awaited: a lazily-evaluated version
    /// would only cancel the second tool after the first had finished leaving, which is needless
    /// waiting rather than a safety property.</summary>
    private List<Task> StopAll(string? keeper = null)
    {
        var stopping = new List<Task>();
        foreach (var id in _running.Keys.ToList())
        {
            if (id == keeper) continue;
            if (StopTool(id, fromStart: true) is { } t) stopping.Add(t);
        }
        return stopping;
    }

    /// <summary>Stops a tool and releases the Arduino COM port. Returns a task that completes once the
    /// tool loop has exited and its CTS is disposed — await it before starting another tool, since all
    /// tools share one serial port. Null when that tool was not running.
    ///
    /// <paramref name="id"/> names which tool; null means the one the UI calls current, which is what
    /// the Stop buttons mean.</summary>
    public Task? StopTool(string? id = null, bool fromStart = false)
    {
        var target = id ?? _currentId;
        if (target == null || !_running.TryGetValue(target, out var running))
        {
            // Nothing running in that slot, but a start may be sitting on the port wait with no tool
            // installed yet. Flag it so the start bails on resume, instead of launching a tool the
            // user has already asked to stop. StartToolAsync's own stop passes fromStart: true, or
            // it would cancel itself here.
            if (_startInProgress && !fromStart) _startCancelled = true;
            return null;
        }

        // Deregister first so a re-entrant call (e.g. StartTool -> StopTool) sees no such tool and
        // doesn't double-cancel.
        _running.Remove(target);
        if (_currentId == target) _currentId = null;

        // A tool that holds a key down must not depend on its own finally to let go of it. The loop
        // that would send the release is what a stop is interrupting, and the case that matters is
        // exactly the one where its own write threw — so every stop path (the card's Stop, the
        // toggle, Quit, Dispose) releases from here instead. "U" is idempotent in the firmware, so
        // the tool's own release arriving too is harmless.
        if (HeldKeys.NeedsSpaceRelease(target)) ReleaseSpace();

        running.Cts.Cancel();

        // Dispose the CTS only after the tool thread has fully exited — the tool loop reads
        // ct.IsCancellationRequested, which throws ObjectDisposedException on a disposed CTS.
        return running.Task.ContinueWith(
            _ => running.Cts.Dispose(),
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

    /// <summary>Why the last <see cref="ReleaseSpace"/> failed, or null when the last one succeeded.
    /// Hold Space is the one tool with no card of its own — its state.Message has nowhere to be drawn
    /// — so this property is its only report surface, and the status line beside the toggle renders
    /// it. A release that fails leaves a real key down on the player's keyboard: not something to log
    /// and move past.</summary>
    public string? LastSpaceReleaseError { get; private set; }

    /// <summary>Releases the spacebar directly, independent of the Hold Space tool's loop, so every
    /// stop path can let go of it rather than only the toggle button. Returns null on success or the
    /// failure message, and records the same on <see cref="LastSpaceReleaseError"/>.
    ///
    /// This used to swallow its exception, which made it the one path that failed silently — while
    /// the tool's own copy of the same write, the one that is only reachable when the loop is not
    /// being interrupted, was the one that reported. That is backwards.</summary>
    public string? ReleaseSpace()
    {
        try
        {
            _arduino?.Write("U\n");
            LastSpaceReleaseError = null;
            return null;
        }
        catch (Exception ex)
        {
            LastSpaceReleaseError = ex.Message;
            return ex.Message;
        }
    }

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

    /// <summary>Reads a region as plain text — for a calibrator that wants to show what the OCR sees
    /// before anything is built on the reading. Distinct from <see cref="CheckOcr"/> because that one
    /// interprets the region as the tuning window.</summary>
    public IReadOnlyList<string> ReadText(RegionConfig region, int upscale = 3, string? saveDebug = null)
    {
        _diagnosticOcr ??= new OcrEngine(Config, Attributes, _rootDir);
        return _diagnosticOcr.ReadLines(region, upscale, saveDebug);
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
        // The firmware report goes in because a pet run lasts days and its log is the only record
        // left afterwards — and it is the one run where a board that ignored a command looks exactly
        // like a board that acted on it.
        "pet" => new PetTool(Config, Attributes, _rootDir, PersistPetState, FirmwareReport).Run(ser, state, ct),
        _ => 1,
    };

    /// <summary>Records the pet run's own state: how many food cells are used up, and which rows are
    /// boarding. Both are written as they change rather than at the end of a run, because the run is
    /// meant to last days and a machine that loses power mid-run would otherwise come back aiming at
    /// cells it had already emptied — or pressing a toggle whose current meaning it has forgotten,
    /// which would end the boarding it meant to start.
    ///
    /// The SESSION half only, like the Pet tab's Save. The tool changes run state; it has no business
    /// rewriting a bag grid or a row's geometry, and writing the whole block would make every
    /// persisted count a chance to lose a calibration.</summary>
    private void PersistPetState()
    {
        try
        {
            var local = _loader.LoadLocal() ?? new ConfigLoader.LocalOverrides();
            local.Pet ??= new ConfigLoader.LocalPet();
            ConfigLoader.LocalPet.ApplySession(local.Pet, Config.Pet);
            _loader.SaveLocal(local);
        }
        catch (Exception ex)
        {
            // Losing the count is bad but not fatal — the next reload clicks a used cell and the one
            // after that recovers. Killing the tool over it would be worse.
            Console.WriteLine("[pet] couldn't persist the food-cell count: " + ex.Message);
        }
    }

    public void Dispose()
    {
        try
        {
            _diagnosticOcr?.Dispose();
            // Every tool, not just the current one. Slots make "everything" expressible, and a
            // shutdown that left a loop writing to a port this method is about to close would be the
            // sort of thing that gets logged as a spurious crash on the way out.
            foreach (var stopped in StopAll())
            {
                // Let each loop leave the port before we close it, or its last write throws on a
                // disposed SerialPort and gets logged as a spurious crash. Bounded — shutdown wins.
                try { stopped.Wait(TimeSpan.FromSeconds(3)); } catch { /* the tool logs its own failure */ }
            }
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

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

    /// <summary>The one tool that stays resident while other tools come and go. It holds nothing
    /// between reloads — it needs the game for ~30 s, five times a day — so it is the tool that WAITS,
    /// and every other Start leaves it alone. See <see cref="StopAll"/> for the rule and
    /// docs/PLAN-RESIDENT-PET.md for why the tuner is deliberately not in this class.</summary>
    public const string ResidentId = "pet";

    private readonly string _rootDir;
    private readonly ConfigLoader _loader;
    private readonly Dictionary<string, RunningTool> _running = new();

    /// <summary>Guards <see cref="_running"/> and <see cref="_currentId"/> — the tool LIFECYCLE, which
    /// is read and written by two threads:
    ///
    ///  - the UI thread reads both every 750 ms, per card, and writes them on Start and Stop;
    ///  - a **worker** thread writes both when a tool finishes on its own (the continuation below).
    ///
    /// A `Dictionary` read while another thread writes it is undefined behaviour — a corrupted lookup or
    /// a spin, not a clean exception. This class already said so:
    /// <see cref="Gate"/>'s own comment describes itself as *"thread-safe, unlike `_running`"*. That was
    /// true and deliberate when only one tool could exist; the resident pet feeder ended that.
    ///
    /// A LOCK rather than <see cref="System.Collections.Concurrent.ConcurrentDictionary{TKey,TValue}"/>,
    /// deliberately: two of the sites are check-then-act — the continuation compares the CTS before
    /// removing, so that a tool which finished is not confused with one the player has already replaced.
    /// A concurrent dictionary makes each CALL atomic and leaves the PAIR racing, which would look like a
    /// fix while leaving the same bug.
    ///
    /// Never held across a serial write, a task continuation or anything else that can block: every
    /// scope below is dictionary work and nothing more.</summary>
    private readonly object _lifecycleLock = new();

    /// <summary>Ids whose stop has been REQUESTED and whose loop has not yet left. Read by the UI to
    /// answer "is this tool still winding down", which `_running` cannot answer — see IsStopping.</summary>
    private readonly HashSet<string> _stopping = new();
    /// <summary>Guards <see cref="_stopping"/>. It is set on the UI thread when a stop is requested and
    /// cleared on a worker thread when the loop finishes, which is exactly the read/write pair that
    /// needs one. Its own lock rather than the registry's, so this change stands alone.</summary>
    private readonly object _stoppingLock = new();
    // EVERY tool id, in one place: this map is both what StartToolAsync accepts and what each id
    // actually runs. It replaced a whitelist that was a second hand-kept copy of the launcher's card
    // list — see the note at the guard in StartToolAsync.
    private readonly Dictionary<string, Func<SerialPort, ToolState, CancellationToken, int>> _toolRunners;
    /// <summary>Which running tool the UI calls "current". Deliberately never the resident tool: it is
    /// background furniture rather than a run the player is watching, and the UI keys mini mode and the
    /// Hold Space card off this. The pet's own card still shows live, via <see cref="StateFor"/>.</summary>
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

    /// <summary>The quest target the next run uses when Start is pressed on the CARD — a sequence name,
    /// or a flow name. The card carries the choice so a run can be aimed without opening Configuration,
    /// and only one of the two is ever set: they are the card's "what am I running" picker, split into
    /// its two kinds rather than packed into one string that somebody would later have to parse.
    ///
    /// Null for both means "use the tab's choice", which is what the tab's own Test buttons want.</summary>
    public string? PendingQuestSequence { get; set; }

    public string? PendingQuestFlow { get; set; }

    /// <summary>How many loops this run does, when the card set one. Null means the flow's or
    /// sequence's own `loops`.</summary>
    public int? PendingQuestLoops { get; set; }
    private OcrEngine? _diagnosticOcr;
    private SerialPort? _arduino;

    public LauncherService(string rootDir)
    {
        _rootDir = rootDir;
        // Built on the one gate rather than owning a rule of its own — see the property above.
        Arbiter = new InputArbiter(Gate);
        _loader = new ConfigLoader(Path.Combine(rootDir, "config"));
        Config = _loader.Load();

        // The active game variant's dictionary — the TW one when no variant is configured. A missing
        // file throws with its path rather than matching nothing.
        Attributes = _loader.LoadAttributes(Config.Game.Active?.Attributes);

        // The one list of tools. Adding a tool means one entry here and nothing else: the same map
        // answers "may this start?" and "what runs for it?".
        _toolRunners = new Dictionary<string, Func<SerialPort, ToolState, CancellationToken, int>>(StringComparer.Ordinal)
        {
            ["holdspace"] = (ser, state, ct) => new HoldSpace(Config).Run(ser, state, ct),
            ["buy"] = (ser, state, ct) => new ShopTool(Config, ShopMode.Buy, PendingBuyPreset, PendingBuyCount).Run(ser, state, ct),
            ["sell"] = (ser, state, ct) => new ShopTool(Config, ShopMode.Sell, null).Run(ser, state, ct),
            ["tuner"] = (ser, state, ct) => new SealTuner(Config, Attributes, _rootDir).Run(ser, state, ct),
            ["gem"] = (ser, state, ct) => new GemComposerTool(Config, _rootDir).Run(ser, state, ct),
            // The gate, so the spammer can stand down when the pet feeder needs the game. It is the one
            // foreground tool that never ends on its own, which makes it the one that would otherwise
            // hold the game for as long as the player leaves it running.
            ["spammer"] = (ser, state, ct) => new SkillSpammer(Config, Gate).Run(ser, state, ct),
            // The firmware report goes in because a pet run lasts days and its log is the only record
            // left afterwards — and it is the one run where a board that ignored a command looks exactly
            // like a board that acted on it.
            ["pet"] = (ser, state, ct) => new PetTool(Config, Attributes, _rootDir, PersistPetState, FirmwareReport, Gate).Run(ser, state, ct),
            ["quest"] = (ser, state, ct) => new SealTools.Quest.QuestTool(Config, PendingQuestLoops, PendingQuestSequence,
                PendingQuestFlow).Run(ser, state, ct),
        };
    }

    /// <summary>The merged config (portable + local). Mutable so edits are reflected live by the tools.</summary>
    public AppConfig Config { get; }

    /// <summary>The OCR attribute dictionary from attributes.yaml.</summary>
    public AttributesConfig Attributes { get; }

    /// <summary>The id of the tool the UI calls current, or null when there is none. Through the lock
    /// because a worker thread clears this when a tool finishes on its own — see
    /// <see cref="_lifecycleLock"/>.</summary>
    public string? CurrentId { get { lock (_lifecycleLock) return _currentId; } }

    /// <summary>The live state of the current tool, or null when there is none.</summary>
    public ToolState? CurrentState => CurrentId is { } id ? StateFor(id) : null;

    /// <summary>The live state of one tool, or null when that tool is not running. The card loop asks
    /// this per id rather than reading one "current" tool, because the pet feeder can be running while
    /// another tool is — and a card that read "stopped" while its tool was feeding pets is exactly the
    /// kind of lie this replaces.</summary>
    public ToolState? StateFor(string id)
    {
        lock (_lifecycleLock) return _running.TryGetValue(id, out var r) ? r.State : null;
    }

    /// <summary>Who is driving the game — the launcher for a foreground tool, or the pet feeder while
    /// it reloads. Its own thread-safe claim rather than a read of the registry: the pet tool asks this
    /// from its own thread, so the "is it free?" check and the claim have to be one operation.</summary>
    public PortGate Gate { get; } = new();

    /// <summary>The UI's way to ask for the game. Wraps <see cref="Gate"/> rather than replacing it —
    /// the gate stays the single answer to "who is driving?", and this only gives the test and
    /// calibration buttons a way to ask the question and to hold what they are given. See
    /// <see cref="InputArbiter"/> for why it is not a second rule beside the gate.</summary>
    public InputArbiter Arbiter { get; }

    /// <summary>Enumerates the serial ports the OS sees, flagging any matching the configured
    /// Arduino VID/PID. Used by the "Arduino" status tab for connection diagnostics.</summary>
    public List<ArduinoDevice> ArduinoDevices() => Arduino.Diagnose(Config.Arduino.Vid, Config.Arduino.Pid);

    /// <summary>Which port Start would open right now, or why it would refuse — the same resolution
    /// <see cref="ArduinoPortAsync"/> uses, exposed so the Arduino tab can SHOW it rather than work it
    /// out again.
    ///
    /// The tab used to answer this question itself, from <see cref="ArduinoDevices"/>, and got two
    /// different answers: it lights on a VID/PID match, while the port that is actually opened also
    /// honours a port NAMED in `arduino.port` (present → that one, whatever its IDs; absent → refuse)
    /// and `Arduino.Find`'s name-based fallback, which `Diagnose` deliberately does not have. So the
    /// tab read green "Connected" for a port Start would refuse, and red "Not found" for one it would
    /// happily open — on the very tab that exists to diagnose "Arduino not found". One resolution,
    /// two readers, so they cannot drift apart again.
    ///
    /// Read-only: it resolves and reports, and opens nothing.</summary>
    public (string? Port, string? Refusal) ResolveArduinoPort() =>
        PortChoice.Choose(
            Config.Arduino.Port,
            System.IO.Ports.SerialPort.GetPortNames(),
            () => Arduino.Find(Config.Arduino.Vid, Config.Arduino.Pid));

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
        // Arduino.Find runs a WMI query SYNCHRONOUSLY on the caller's thread — the dispatcher, when
        // this is reached from a button click — so its duration is logged rather than assumed. A
        // slow one is a frozen window, which looks exactly like a dead button from the outside.
        // A port NAMED in the config wins, and a named port that is not present is an error rather than
        // a fall-through to discovery — see PortChoice. Until this, `arduino.port` was documented as an
        // override and read by nothing, so naming a port did silently nothing at all.
        var findSw = System.Diagnostics.Stopwatch.StartNew();
        var (port, portError) = ResolveArduinoPort();
        HoldDiag($"arduino port {port ?? "(none)"} took {findSw.ElapsedMilliseconds}ms");
        if (port == null)
        {
            LastArduinoError = portError ?? $"Arduino not found (VID 0x{Config.Arduino.Vid:X4}, " +
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

    /// <summary>Obtains the game for a test or calibration button, or refuses and says who has it.
    /// The UI's one way in: every test button goes through here instead of taking the port itself, so
    /// "no button writes to the board outside the arbiter" is one rule in one place rather than 19
    /// places that each have to remember it.
    ///
    /// The claim is taken BEFORE the port is opened, deliberately. Opening is a 2 s boot wait on a
    /// cold start, and spending it only to refuse would leave the button looking dead for two seconds
    /// before it said why — while the refusal is knowable immediately.
    ///
    /// Returns null with <see cref="LastArduinoError"/> set, so a caller's existing message path
    /// reports the real reason rather than repeating a guess about the cable.</summary>
    public async Task<InputLease?> AcquireInputAsync()
    {
        LastArduinoError = null;

        if (!Arbiter.TryAcquireUiLease(out var refusal))
        {
            LastArduinoError = refusal;
            return null;
        }

        var ser = await ArduinoPortAsync();
        if (ser == null)
        {
            // The reason is ArduinoPortAsync's and is already on LastArduinoError. Hand the claim
            // back, or the claim outlives the attempt and every later test is refused with "another
            // test is using the game" while no test is.
            Arbiter.ReleaseUiLease();
            return null;
        }

        return new InputLease(Arbiter, ser);
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
        // The acceptance test and the dispatch are the SAME map (_toolRunners) — that is the whole
        // point of it. This used to be a whitelist that was a second hand-kept copy of the launcher's
        // card list, and it failed LOUDLY rather than safely: a tool registered for its card and its
        // tab but missed here looked wired up and did nothing when Start was pressed, because the
        // exception came out of an async void handler — no message box, no card line, only an entry in
        // logs\error.log. That is exactly what the quest tool did on its first launch.
        if (!_toolRunners.ContainsKey(id))
        {
            throw new ArgumentException($"Unknown tool id: {id}", nameof(id));
        }

        // One start at a time. ArduinoPortAsync waits out a 2 s boot delay on a cold start, and its
        // fast path returns an already-open port with no delay at all — so two Start clicks could
        // both get past StopTool() while it still saw _cts == null. The second would then overwrite
        // _cts and _toolTask and orphan the first loop, which kept writing to the shared serial port
        // with no way to stop it while the UI showed the other tool. A click during a start is a
        // no-op, and reports no error.
        if (_startInProgress)
        {
            // The silent no-op, and the single reason this report could not be diagnosed from the
            // outside: it returns SUCCESS and starts nothing, so the caller shows no message and the
            // click is indistinguishable from a dead button. Logged with the id it swallowed.
            HoldDiag($"start {id} SWALLOWED - a start is already in flight");
            return true;
        }
        _startInProgress = true;
        _startCancelled = false;
        StartingId = id;
        var sw = System.Diagnostics.Stopwatch.StartNew();
        HoldDiag($"start {id} begin");
        try
        {
            var ok = await StartToolCoreAsync(id);
            HoldDiag($"start {id} {(ok ? "ok" : "REFUSED")} after {sw.ElapsedMilliseconds}ms");
            return ok;
        }
        finally
        {
            StartingId = null;
            _startInProgress = false;
        }
    }

    private async Task<bool> StartToolCoreAsync(string id)
    {
        // What gives way, which is the whole of residency in two lines:
        //
        //  - starting anything else displaces every other tool EXCEPT the pet, so a run can be driven
        //    while the pets keep their schedule;
        //  - starting the pet displaces only a PREVIOUS PET, so it neither kills a run in progress nor
        //    leaves its own earlier loop orphaned — writing to the shared port with no way to stop it.
        var stopping = id == ResidentId
            ? StopAll(keep: running => running != ResidentId)
            : StopAll(keep: running => running == ResidentId);
        foreach (var t in stopping)
        {
            // Wait for each loop to leave the shared serial port before this one starts writing to
            // it — otherwise their byte streams can interleave. Bounded, so a wedged tool can't hang
            // the UI's Start click.
            await Task.WhenAny(t, Task.Delay(3000));
        }

        // A foreground tool owns the game for its whole run, so it has to wait for a reload in
        // progress rather than interrupt one — and a reload is the one place a half-finished sequence
        // leaves a pet unfed. Bounded and reported: the reload takes ~30 s, and a Start that waited
        // silently and then did nothing would be indistinguishable from a broken button.
        if (id != ResidentId && !await WaitForGameAsync(id))
        {
            // Said, not left null: the Start handler shows this in a dialog, and a silent null there
            // reads as "Arduino not found", which would be a wrong explanation for a cancelled start.
            LastArduinoError = _startCancelled
                ? "Start cancelled."
                : $"The pet feeder is still using the game ({Gate.Owner}) — {id} was not started.";
            return false;
        }

        var ser = await ArduinoPortAsync();
        // A Stop clicked while the port was opening has nothing to cancel (no _cts yet), so it set
        // _startCancelled instead. Honour it rather than starting the tool anyway.
        if (ser == null || _startCancelled)
        {
            // The claim is the launcher's, not the tool's, so a start that gives up has to hand it
            // back — the pet feeder waits on this and would otherwise never reload again.
            if (id != ResidentId) Gate.Release(id);
            return false;
        }
        // A drag needs firmware 2. On an older board 'L' and 'l' are ignored — the button is never
        // pressed, the stack is never picked up, and it looks exactly like a mis-aimed drag, with food
        // silently not loaded. Refuse the start instead: this is the one place the board's version
        // changes behaviour rather than being printed, which is why the level says what it means.
        if (id == ResidentId && FoodLoadMode.Complaint(Config.Pet.FoodLoadMode, FirmwareLevel) is { } complaint)
        {
            LastArduinoError = complaint;
            return false;
        }

        // The resident tool never becomes "current": it is background furniture, and the UI keys mini
        // mode and the Hold Space card off this. Its card still shows live through StateFor.
        if (id != ResidentId) { lock (_lifecycleLock) _currentId = id; }
        // Starting a hold takes the spacebar over deliberately, so any standing "the release failed"
        // warning describes a state that no longer applies — it would otherwise sit red on the status
        // line for the rest of the session, after the player had already dealt with it.
        if (id == "holdspace") LastReleaseError = null;

        var cts = new CancellationTokenSource();
        var state = new ToolState { Running = true };
        var ct = cts.Token;
        var entry = new RunningTool { Id = id, Cts = cts, State = state };

        // Registered BEFORE the loop is started, and with no await between the two, so a Stop from
        // the UI cannot fall between them and miss a tool that is already running. (The whole block
        // is synchronous on the dispatcher thread, which is what makes that safe.)
        // A crash from a previous run of THIS tool stops being news the moment it is running again.
        // Cleared by id rather than wholesale, so another tool's failure stays on its own card.
        if (LastCrash?.Id == id) LastCrash = null;
        lock (_lifecycleLock) _running[id] = entry;
        entry.Task = Task.Run(() =>
        {
            try
            {
                RunTool(id, ser, state, ct);
            }
            catch (OperationCanceledException) when (ct.IsCancellationRequested)
            {
                // A STOP, NOT A CRASH. SleepCheck throws so a step in progress — a pet drag, a reload
                // wait — is abandoned instead of finishing first, which is what made Stop on the pet
                // feeder look like it did nothing. Without this clause that ordinary stop lands in the
                // handler below: "pet stopped: The operation was cancelled." on the card, and a line
                // in logs/error.log, every single time the player pressed Stop.
                //
                // Nothing reported and nothing logged, deliberately: silence is what a normal stop
                // does today, and a stop is not news. The finally below tears down either way.
            }
            catch (Exception ex)
            {
                // The tool card is the only place the user can see this — the published WinExe has no
                // console — so the reason goes on the state, not just the log. Without it a crashed
                // tool rendered as "paused" with no explanation.
                state.Message = $"{id} stopped: {ex.Message}";
                state.Running = false;
                // And on the service, because the state does not outlive the teardown below — see
                // LastCrash. Without this the card renders a crash as a plain "stopped".
                LastCrash = (id, ex.Message);
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
            finally
            {
                // A TOOL THAT RETURNS ON ITS OWN still has to be torn down, and until the quest tool
                // existed none did: every other tool loops until it is stopped, so the stop path was the
                // only teardown there was and this block did not need one. A tool that finished left
                // Running = true for good — the card read "● RUNNING" after the work was done, the entry
                // stayed in _running, and the next Start would have deferred to a tool that had already
                // finished.
                //
                // ONLY WHEN THIS ENTRY IS STILL REGISTERED, which is what tells a self-end from a stop.
                // StopTool removes the entry first and its own continuation disposes the CTS and releases
                // the gate; doing either again here would release a claim this tool no longer holds.
                // Compared by CTS rather than by the entry object, because the lambda can run before
                // `entry.Task` has even been assigned.
                // The CHECK AND THE REMOVE are one critical section, and that is the whole reason this
                // uses a lock rather than a concurrent dictionary: between the two, a Stop could clear
                // the entry and a fresh Start could install a NEW one — and this would then remove a
                // tool it does not own.
                bool ours;
                lock (_lifecycleLock)
                {
                    ours = _running.TryGetValue(id, out var stillRunning)
                           && ReferenceEquals(stillRunning.Cts, cts);
                    if (ours)
                    {
                        _running.Remove(id);
                        if (_currentId == id) _currentId = null;
                    }
                }

                if (ours)
                {
                    state.Running = false;

                    // The same release the stop path does, and for the same reason: the tool's own
                    // finally is not what should be holding a key down.
                    if (HeldKeys.NeedsReleaseOnStop(id)) ReleaseHeld();

                    cts.Dispose();
                    if (id != ResidentId) Gate.Release(id);
                }
            }
        });
        return true;
    }

    /// <summary>How long a foreground Start waits for the pet feeder to finish a reload before giving
    /// up and saying so. A reload takes ~30 s, so the rest is slack rather than a measured worst case —
    /// long enough not to cut off a reload that is merely slow, short enough that a wedged one is not
    /// an unkillable Start click.</summary>
    private const int GameWaitMs = 45_000;

    /// <summary>Waits for the game to be free and CLAIMS it for <paramref name="id"/>, or false when
    /// the wait ran out or a Stop arrived. The claim happens inside the loop's own check, so there is
    /// no window between "it is free" and "take it".</summary>
    private async Task<bool> WaitForGameAsync(string id)
    {
        for (var waited = 0; waited < GameWaitMs; waited += 250)
        {
            if (_startCancelled) return false;
            if (Gate.TryAcquire(id)) return true;

            // The only channel to the Start handler, which shows it if this ends in failure. Cleared
            // by ArduinoPortAsync on the way out, so a wait that succeeds leaves nothing behind.
            LastArduinoError = $"Waiting for the pet feeder to finish its reload ({Gate.Owner})…";
            await Task.Delay(250);
        }
        return false;
    }

    /// <summary>Stops every running tool that <paramref name="keep"/> does not name, and returns one
    /// task per stopped tool, each completed once that loop has left the port. A null predicate keeps
    /// nothing, which is what shutdown wants.
    ///
    /// Materialised, and every stop issued before any of them is awaited: a lazily-evaluated version
    /// would only cancel the second tool after the first had finished leaving, which is needless
    /// waiting rather than a safety property.</summary>
    private List<Task> StopAll(Func<string, bool>? keep = null)
    {
        var stopping = new List<Task>();
        // The ToList is taken UNDER the lock: enumerating the live dictionary while a worker finishes a
        // tool on its own is the same undefined behaviour the rest of this block exists to remove, and
        // ToList does not copy safely on its own.
        List<string> ids;
        lock (_lifecycleLock) ids = _running.Keys.ToList();
        foreach (var id in ids)
        {
            if (keep?.Invoke(id) == true) continue;
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
        RunningTool? running;
        string? target;
        lock (_lifecycleLock)
        {
            target = id ?? _currentId;
            _running.TryGetValue(target ?? "", out running);
        }
        HoldDiag($"stop enter id={id ?? "(current)"} target={target ?? "(null)"} fromStart={fromStart} " +
                 $"registered={(running != null ? "yes" : "no")}");
        if (target == null || running == null)
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
        lock (_lifecycleLock)
        {
            _running.Remove(target);
            if (_currentId == target) _currentId = null;
        }
        HoldDiag($"stop deregistered {target}; releasing={HeldKeys.NeedsReleaseOnStop(target)}");

        // A tool that holds a key down must not depend on its own finally to let go of it. The loop
        // that would send the release is what a stop is interrupting, and the case that matters is
        // exactly the one where its own write threw — so every stop path (the card's Stop, the
        // toggle, Quit, Dispose) releases from here instead. "U" is idempotent in the firmware, so
        // the tool's own release arriving too is harmless.
        if (HeldKeys.NeedsReleaseOnStop(target)) ReleaseHeld();

        running.Cts.Cancel();

        // Dispose the CTS only after the tool thread has fully exited — the tool loop reads
        // ct.IsCancellationRequested, which throws ObjectDisposedException on a disposed CTS.
        //
        // The gate is handed back here too, and NOT at the Cancel above: releasing the moment a stop
        // is requested would let the pet feeder start clicking while this tool is still sending its
        // last command. The cost is that a tool loop which never exits keeps the game — and the pet
        // feeder says "waiting for <tool>" on its card the whole time, which is the honest reading
        // rather than a silent overlap. The pet's own claims are not the launcher's to release.
        // Set BEFORE the continuation is attached, so the flag cannot be cleared by a continuation that
        // runs the instant it is attached — the tool's task is often already complete by here, and
        // ExecuteSynchronously means it would otherwise clear a flag that had not been set yet.
        lock (_stoppingLock) _stopping.Add(target);

        return running.Task.ContinueWith(
            _ =>
            {
                running.Cts.Dispose();
                if (target != ResidentId) Gate.Release(target);
                lock (_stoppingLock) _stopping.Remove(target);
            },
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

    /// <summary>Why the last <see cref="ReleaseHeld"/> failed, or null when the last one succeeded.
    /// Hold Space is the one tool with no card of its own — its state.Message has nowhere to be drawn
    /// — so this property is its only report surface, and the status line beside the toggle renders
    /// it. A release that fails leaves a real key or mouse button down on the player's machine: not
    /// something to log and move past.</summary>
    public string? LastReleaseError { get; private set; }

    /// <summary>Which tool crashed last, and why — or null when none has this session.
    ///
    /// The crash handler already writes the reason to the tool's own <see cref="ToolState"/>, and its
    /// comment says why that matters: *"the tool card is the only place the user can see this — the
    /// published WinExe has no console"*. But the teardown that follows removes the tool from the
    /// registry microseconds later, the card reads the registry, and so the card fell through to a
    /// bare "stopped" — a crash rendered exactly like a deliberate Stop, with the reason surviving
    /// only in logs/error.log, which is the one place that comment says is not enough.
    ///
    /// Held HERE rather than on the state because it has to OUTLIVE the run: the whole difficulty is
    /// that the failing tool is gone by the time anyone can read it. Cleared when that same tool
    /// starts again, so it can never describe a run the player has already replaced.</summary>
    public (string Id, string Message)? LastCrash { get; private set; }

    /// <summary>True while a Start is between its first line and its tool actually running. A click
    /// during that window is deliberately a silent no-op — it returns true and reports nothing — and
    /// the Hold Space report of 2026-10-01 is exactly a click that appeared to do nothing. Diagnostic
    /// only; the UI needs to be able to say which of the two it was.</summary>
    public bool StartInProgress => _startInProgress;

    /// <summary>The tool id a Start is currently in flight for, or null when none is. The Hold Space
    /// toggle needs THIS and not just <see cref="StartInProgress"/>: <see cref="CurrentId"/> is only
    /// set once a start COMPLETES, so during the ~2 s cold start the toggle had no way to tell "a
    /// hold is coming" from "nothing is happening" — it answered "not running", re-entered Start, and
    /// that click was swallowed while the spacebar stayed held.</summary>
    public string? StartingId { get; private set; }

    /// <summary>True while a stop for this id is in flight — requested, and not yet finished.
    ///
    /// THIS EXISTS BECAUSE <see cref="StopTool"/> CLEARS THE CURRENT ID FIRST, before the worker has left
    /// its loop. A UI that asked only "is it loaded?" therefore read *idle* while the tool was still
    /// winding down, and the next press started it again — which is how a toggle that had just been
    /// switched off came back on. `CurrentId` cannot answer this; this can.
    ///
    /// Cleared in the same continuation that disposes the CTS, so it is true for exactly as long as the
    /// tool is genuinely still on its way out.</summary>
    public bool IsStopping(string id)
    {
        lock (_stoppingLock) return _stopping.Contains(id);
    }

    // Hold Space diagnostics (2026-10-01) — the service half of the same trail MainWindow writes.
    // Same file, same reason: a click that never reached StopTool and a stop whose release did
    // nothing are different faults and look the same from the outside. Delete with the UI half.
    private void HoldDiag(string line)
    {
        try { File.AppendAllText(Path.Combine(_rootDir, "logs", "holdspace.log"),
            $"{DateTime.Now:HH:mm:ss.fff} [svc] {line}\n"); }
        catch { /* diagnostics must never break a stop */ }
    }

    /// <summary>Lets go of everything a tool can hold down — the spacebar and the left mouse button —
    /// directly and independent of any tool's loop, so every stop path can release rather than only
    /// the toggle button. Returns null on success or the failure message, and records the same on
    /// <see cref="LastReleaseError"/>.
    ///
    /// BOTH commands, always, rather than whichever the stopping tool happens to use. They are
    /// idempotent in the firmware — releasing a spacebar that is not held is a no-op, and so is
    /// releasing a button that is not pressed — and a board too old to know 'l' ignores it. Picking
    /// between them would mean the release path had to be right about which tool was stopping, and
    /// being wrong about that is exactly how a release path comes to fail.
    ///
    /// This used to swallow its exception, which made it the one path that failed silently — while
    /// the tool's own copy of the same write, the one that is only reachable when the loop is not
    /// being interrupted, was the one that reported. That is backwards.
    ///
    /// A null or closed port is now a REPORTED failure rather than a silent success. It was the last
    /// way this method could lie, and the worst one to lie about: what it claims to have released is
    /// a key that is still down on the player's machine, so "released" and "never sent" cannot be the
    /// same answer. Nothing is written on that path, so nothing is reported as written.</summary>
    public string? ReleaseHeld()
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        if (_arduino is not { IsOpen: true } port)
        {
            // Said, not merely logged. The log line was the old mitigation and it is not a report —
            // the player never sees logs/holdspace.log, and the status line beside the toggle would
            // have gone on reading as though the key had been let go.
            // Short, because the status line already says "input may be stuck" and its tooltip already
            // gives the remedy — this only has to supply the reason, not repeat the advice.
            LastReleaseError = "the Arduino port is not open, so nothing was sent";
            HoldDiag($"release NOT SENT — port is {(_arduino == null ? "null" : "closed")}");
            return LastReleaseError;
        }

        // The write's duration is on the record rather than assumed to be instant, which is the other
        // thing the log is for.
        HoldDiag($"release enter port={port.PortName}");
        try
        {
            port.Write("U\n");   // spacebar
            port.Write("l\n");   // left mouse button (firmware 2)
            LastReleaseError = null;
            HoldDiag($"release ok {sw.ElapsedMilliseconds}ms");
            return null;
        }
        catch (Exception ex)
        {
            LastReleaseError = ex.Message;
            HoldDiag($"release FAILED after {sw.ElapsedMilliseconds}ms: {ex.Message}");
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

    /// <summary>Like <see cref="ReadText"/>, but keeps each line's confidence — for a caller that has
    /// to choose between lines rather than merely drop weak ones.</summary>
    public IReadOnlyList<(string Text, double Score)> ReadTextScored(RegionConfig region, int upscale = 3,
        string? saveDebug = null, double minScore = 0)
    {
        _diagnosticOcr ??= new OcrEngine(Config, Attributes, _rootDir);
        return _diagnosticOcr.ReadLinesScored(region, upscale, saveDebug, minScore);
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

    /// <summary>Runs the tool an accepted id names. The id was accepted by the same map that answers
    /// here, so an unknown one is a programming error rather than a user error — but a tool thread must
    /// not throw over it, so it reports the old "one attempt" and says so on the console.</summary>
    private int RunTool(string id, SerialPort ser, ToolState state, CancellationToken ct)
    {
        if (_toolRunners.TryGetValue(id, out var run)) return run(ser, state, ct);
        Console.WriteLine($"[launcher] no runner for tool id '{id}'");
        return 1;
    }

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

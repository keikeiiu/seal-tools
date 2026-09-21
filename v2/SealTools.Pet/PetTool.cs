using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO.Ports;
using System.Threading;
using OpenCvSharp;
using SealTools.Core;
using SealTools.Core.Config;

namespace SealTools.Pet;

// Keeps a boarded pet fed, on a schedule.
//
// The game feeds the pet; this only RELOADS THE FEEDER. That reload is not a top-up: ending boarding
// returns the pet to the bag as well as the food, so every reload is
//
//     end  →  find the pet  →  re-place it  →  load the stacks  →  start
//
// and it is why the pet has to be FOUND rather than pointed at — it lands in the first free bag slot,
// which depends on whatever else the bag held at that instant.
//
// The schedule is the whole trigger. Polling for a signal was designed and dropped: the hunger readout
// at the bottom right belongs to the CARRIED pet rather than the boarded one, so nothing on the main
// screen says anything about the pet this tool is feeding. What is left is arithmetic — rate and load
// are both known — and it is enough, because reloading early is nearly free (the leftover food comes
// back) while leaving the pet unfed is the one thing here that cannot be undone.
//
// See docs/PLAN-PET-AUTOFEED.md.
public sealed class PetTool : ToolBase
{
    // Waits. Generous for the same reason the shop tool's are: a click that outruns the game lands on
    // whatever is behind it, and that reads as a calibration failure rather than a timing one.
    private const double WindowWait = 0.9;
    private const double ClickWait = 0.35;
    private const double DialogWait = 0.5;

    /// <summary>Minutes to wait PAST the feeder emptying, from config — positive by default, so the
    /// reload lands on a guaranteed-empty feeder rather than a partial one.</summary>
    private double WaitAfterEmptyMinutes => _cfg.Pet.WaitAfterEmptyMinutes;

    private const double RetryMinutes = 5;

    /// <summary>How long a row waits when the BAG holds no pet that can be fed — see
    /// <see cref="RowOutcome.NothingToBoard"/>. Longer than <see cref="RetryMinutes"/> because it is not
    /// a fault to be recovered from: nothing changes until the player puts a feedable pet in the bag,
    /// or an existing one is taken out of a loader and becomes boardable again. Half an hour is a
    /// compromise between noticing soon and opening the window on a bag that has not changed.</summary>
    private const double NoPetRetryMinutes = 30;

    /// <summary>How many times to re-click the pet before giving up. More than one because the failure
    /// is intermittent rather than a wrong calibration — the cursor is verified on target — so a retry
    /// is the fix and the count only bounds it.</summary>
    private const int PlaceAttempts = 3;

    /// <summary>After a click, before looking at the slot. The game needs a moment to move the pet.</summary>
    private const double PlaceSettle = 0.9;

    /// <summary>How long the left button is HELD before the cursor starts moving, on a food drag.
    ///
    /// The drag used to press and move in the same frame. The player watched the first drag of a
    /// reload miss while the second, a second later, worked — and the log agrees they differ: the
    /// first presses and drops inside the same second. A human drag holds still for a moment before
    /// pulling; this is that moment. It is a GUESS at the size, and the log now records enough
    /// (landed positions per move, plus whether the bag cell actually emptied) for it to be corrected
    /// from evidence rather than argued about.</summary>
    private const double DragGrabWait = 0.35;

    /// <summary>How many times a food drag is re-attempted when the bag cell still holds its stack.
    /// The same reasoning as <see cref="PlaceAttempts"/>: the failure is intermittent, the cursor is
    /// verified on target, so a retry is the fix and the count only bounds it.</summary>
    private const int FoodDragAttempts = 3;

    /// <summary>How much the dragged bag cell must change for the stack to count as moved.
    ///
    /// MEASURED on the bag itself: a cell holding something differs from an empty one by 0.47 and up,
    /// while two empty cells differ by 0.000. 0.10 sits well inside that gap, and the direction matters
    /// — reading a successful drag as FAILED would re-drag a cell that is already empty, which picks up
    /// nothing and then fires a MAX click and an Enter at a dialog that is not there. So the bar for
    /// "it moved" is deliberately low, and the number is logged either way so it can be corrected from
    /// real drags rather than from this note.</summary>
    private const double BagCellChangedAbove = 0.10;

    /// <summary>The buffer after each action on the pet, from config. Every step in a reload changes the
    /// breeder's state, and the next click is aimed at a window still absorbing the last one.</summary>
    private double ActionWait => Math.Max(0.2, _cfg.Pet.ActionWaitMs / 1000.0);

    /// <summary>After a bag page tab is clicked, before anything is clicked inside the grid.
    ///
    /// Switching pages re-renders the grid, and a right-click delivered during that is dropped — with
    /// the cursor sitting on the right cell the whole time, so it reads as a miss rather than as
    /// timing. This is the same class of bug as the click delays that bit the shop on a second PC: a
    /// click that outruns an animation is indistinguishable from a click that was never sent.
    /// </summary>
    private const double PageWait = 0.9;

    /// <summary>After pressing start, before the window is closed. The player's call, and it closes a
    /// real hole: the close is the cleanup and runs immediately after the start click, so the window
    /// was being shut while the game was still acting on the press. Watching it work is what makes
    /// this the right order — nothing here is slow enough to matter.</summary>
    private const double StartWait = 2.0;

    /// <summary>How many reloads may fail in a row before the tool stops. Unattended retrying is how
    /// a misread becomes a loop of clicks, and this one runs while nobody is watching.</summary>
    private const int MaxFailures = 3;

    /// <summary>How unlike the pet a cell may look and still be clicked, as a fraction of pixels that
    /// differ.
    ///
    /// MEASURED, finally, after being a guess for a long time. It was 0.12 — the composer's empty-box
    /// number carried over, on the reasoning that strict is safe because a near-miss right-clicks some
    /// other item and there is no undo. Against the player's real bag (2026-09-19, ten pets across
    /// three pages) that was simply too tight, and it was costing them four pets on every scan:
    ///
    ///     the pets         0.000 – 0.150     (one at 0.320)
    ///     everything else  0.831 and up
    ///
    /// Half a bag of pets read as "not the pet" while the nearest non-pet was five times further away.
    /// The offset search covers a shift; this covers the rest — the same portrait rendered a few
    /// percent differently, which no amount of searching can align.
    ///
    /// 0.5 sits in the middle of a gap from 0.32 to 0.83, so it is not a threshold tuned until the
    /// answer came out right — it is one placed where the measurements left room.
    ///
    /// PUBLIC so the Pet tab's "scan the bag" reports against the number the tool actually uses. A
    /// diagnostic with its own copy of a threshold is a diagnostic that can agree with a run and be
    /// wrong about it.</summary>
    public const double MatchLimit = 0.5;

    /// <summary>The reload's own log. Written because the card's message is the only other record of a
    /// reload, and it is overwritten by the next line and gone once the tool stops — so a failure that
    /// happened while nobody was watching left nothing at all to read. Lands beside the launcher's
    /// crash log, under the bin directory.</summary>
    private static void Log(string line)
    {
        try
        {
            var dir = System.IO.Path.Combine(AppContext.BaseDirectory, "logs");
            System.IO.Directory.CreateDirectory(dir);
            System.IO.File.AppendAllText(System.IO.Path.Combine(dir, "pet.log"),
                $"{DateTime.Now:yyyy-MM-dd HH:mm:ss} {line}\n");
        }
        catch
        {
            // Logging must never be the reason a tool fails.
        }
    }

    private readonly AppConfig _cfg;
    /// <summary>Persists how many food cells are used up. Passed in rather than done here because a
    /// tool has no config loader — and the count has to outlive the process: a restart that reset it
    /// would aim the next reload at cells this run had already emptied.</summary>
    private readonly Action? _persistState;

    /// <summary>The OCR dictionary and the root the models live under — what an
    /// <see cref="OcrEngine"/> needs, and what this tool needed the moment it wanted to read a count
    /// rather than only a crop. Passed in the same shape <see cref="Tuner.SealTuner"/> takes them,
    /// which is the precedent for a tool owning an engine rather than borrowing the launcher's.</summary>
    private readonly AttributesConfig _attrs;

    /// <summary>Which bag page the tool last put the bag on, or -1 for "don't know".
    ///
    /// Measured from the player's config: **all 22 marked food cells are on ONE page**, and
    /// `SelectPage` was called before every stack — so a 5-stack row clicked that one tab five times,
    /// four of them provably redundant. Each cost a cursor placement and about 1.2 s of waiting, and
    /// every one was another chance for the cursor to miss.
    ///
    /// The risk this carries, stated: the page is never READ back, so if the bag ever moved without the
    /// tool moving it, this would skip a tab click that was needed and the next action would aim at the
    /// wrong page. Two things bound that. Every page change in the tool goes through
    /// <see cref="SelectPage"/>, and the value is dropped whenever the boarding window opens or closes,
    /// because reopening brings the bag up on whatever page it likes — so the assumption only ever
    /// covers the seconds between one stack and the next.</summary>
    private int _bagPage = -1;

    /// <summary>What the guard read about the pet it just boarded, and which line that pet is.
    ///
    /// Set by <see cref="RememberBoarded"/> when the pet actually goes in, and read immediately after by
    /// the reload to work out when this row next needs looking at. Fields rather than out-parameters
    /// only because the pair would otherwise be threaded through three signatures for a value whose
    /// life is three lines; both are CLEARED at the start of every placement, so a stale pair can never
    /// be read as this pet's.</summary>
    private PetPanel? _boardedPanel;
    private string? _boardedSpecies;

    /// <summary>The feeding table — scraped, measured, and the player's chosen source for `wyz`:
    /// *"the base feeding value is predetermined, nowhere can you find it in the game."* Loaded once
    /// per run rather than kept in memory for the life of the process.</summary>
    private IReadOnlyList<PetFeeding.Line> _feeding = Array.Empty<PetFeeding.Line>();

    /// <summary>Records what was read about the pet that just went into the loader. See
    /// <see cref="_boardedPanel"/>.</summary>
    private void RememberBoarded(int entry, PetPanel? panel)
    {
        _boardedPanel = panel;
        _boardedSpecies = entry >= 0 && entry < _cfg.Pet.Queue.Count
            ? _cfg.Pet.Queue[entry].Species
            : null;
    }

    /// <summary>How long the pet that was just boarded still needs, or null when that cannot be
    /// answered — no species named for its queue entry, no panel read for it, or no row for that line
    /// at its stage.
    ///
    /// NULL IS THE NORMAL CASE AND IS NOT AN ERROR. A queue entry with no line named is allowed, and a
    /// pet the tool could not read a panel for boards anyway. Both fall back to the configured cycle,
    /// which is what the tool did before any of this existed.</summary>
    private double? MinutesForBoardedPet()
    {
        if (_boardedPanel is not { } panel || _feeding.Count == 0) return null;

        var line = PetFeeding.Find(_feeding, _boardedSpecies, panel.Stage ?? -1);
        var minutes = PetFeeding.MinutesToFinish(line, panel.Growth, panel.Exp);

        if (minutes is { } m)
            Log($"  {line!.Name} ({line.Species} +{line.Stage}) at +{panel.Growth} " +
                $"{panel.Exp:0.##}% — {m:0} min of feeding left" +
                (m < 1 ? "  ← done, or within a minute of it" : ""));
        else
            Log($"  no feeding estimate — " + (_boardedSpecies == null
                ? "no pet line is named for this queue entry"
                : $"the table has no {_boardedSpecies} at stage " +
                  $"{(panel.Stage is { } s ? s.ToString(CultureInfo.InvariantCulture) : "?")}"));

        return minutes;
    }
    private readonly string _rootDir;

    /// <summary>What the board said it was running when the port was opened, or null when it said
    /// nothing. Recorded on the run's first log line because a run lasts days and the log is what is
    /// left afterwards: a stuck spacebar or a command the board ignored is unanswerable without it,
    /// and until the board could be asked there was no way to tell an old sketch from a failure.</summary>
    private readonly string? _firmware;

    /// <summary>Who else is driving the game, or null when this tool was handed no gate. Claimed
    /// around each look and each reload, and never held in between — the whole reason this tool can be
    /// resident is that it holds nothing while it waits.</summary>
    private readonly PortGate? _gate;

    public PetTool(AppConfig cfg, AttributesConfig attrs, string rootDir, Action? persistState = null,
        string? firmware = null, PortGate? gate = null)
        // The only tool that ignores the quit hotkey: it is resident, so one Quit press meant for a
        // foreground run must not end a schedule that is feeding four pets. Its own Stop still works.
        : base(cfg.Hotkeys, ignoresQuitHotkey: true)
    {
        _cfg = cfg;
        _attrs = attrs;
        _rootDir = rootDir;
        _persistState = persistState;
        _firmware = firmware;
        _gate = gate;
    }

    public int Run(SerialPort ser, ToolState state, CancellationToken ct)
    {
        if (Ready(state) is { } problem)
        {
            Console.WriteLine("[!] " + problem);
            state.Message = problem;
            state.Running = false;
            return 0;
        }

        // THE FEEDING TABLE, once per run. A missing or unreadable table costs the COMPUTED schedule
        // and nothing else: every pet still boards and still gets fed, on the configured cycle.
        _feeding = PetFeeding.Load(System.IO.Path.Combine(_rootDir, "docs", "pet-data.csv"));

        Log($"run started — board firmware {_firmware ?? "not reported"}, " +
            $"{_cfg.Pet.ActiveRows.Count()} row(s) at {_cfg.Pet.ItemsPerMinute}/min, plus " +
            $"{WaitAfterEmptyMinutes:0} min after empty: " +
            string.Join(", ", _cfg.Pet.ActiveRows.Select(r =>
                $"{NameOf(r)} every {CycleMinutesFor(r):0} min " +
                $"({PetConfig.LoadItemsFor(r)} items, {r.Stacks} stacks)")));

        // One schedule PER ROW. Not one clock for all of them, and the reason is arithmetic rather
        // than taste: the free row holds two food stacks and a paid row five, so a full load lasts
        // 200 minutes on one and 500 on the other. A single timer would reload the paid rows while
        // they were still half full, or leave the free row dry for five hours.
        //
        // LOOK FIRST, DON'T RELOAD FIRST (player, 2026-09-19). Every row used to be due immediately,
        // on the reasoning that the tool cannot read how much food a row holds so the only way to know
        // the state is to establish it. That reasoning has expired twice over: the boarding slot says
        // whether a pet is in there, and the feeder counts say how much food is left — so a look
        // answers what a reload used to be performed to find out.
        //
        // The look returns the SCHEDULE, not just the flags: each row's next reload is worked out from
        // what was read about it, and a row it could not read falls back to the configured cycle.
        //
        // It also opens the breeder and clicks, so it needs the game — and it is the first thing that
        // can be deferred. A run started while another tool is busy therefore starts and WAITS to look,
        // rather than reporting rows it never read.
        var looked = ClaimGame(state,
            () => $"Starting up — waiting for {GameOwner} before reading the rows", ct);
        Dictionary<PetSlotConfig, DateTime> next;
        try
        {
            next = looked ? InspectRows(ser, state) : new Dictionary<PetSlotConfig, DateTime>();
        }
        finally
        {
            // In a finally like the reload's, and for the same reason: a throw out of the look would
            // otherwise leak the claim and nothing could ever take the game again.
            ReleaseGame();
        }

        // Stopped while waiting to look. Nothing is scheduled and nothing was fed, so the run ends —
        // and Running has to be cleared here because this return is above the try/finally that does it
        // for every other exit, which would otherwise leave the card claiming "● RUNNING" for a tool
        // that is already gone.
        if (!looked)
        {
            state.Message = "Stopped before it could read the rows.";
            state.Running = false;
            return 0;
        }

        // THE CHECKING LINE COMES OFF NOW. InspectRows sets "Checking what is already running…" so the
        // card says something while it clicks, and nothing ever cleared it — so a run that had finished
        // looking sat there claiming to be looking, for as long as its first reload was away. A
        // progress note is not a state, and on this tool the gap between the two is hours.
        state.Message = null;
        var failures = _cfg.Pet.ActiveRows.ToDictionary(r => r, _ => 0);

        // THE CARD'S STANDING LINE. A reload is hours apart, so a card that only reported the last
        // thing that happened said nothing at all for most of a run — the player asked for "which row
        // is running and how long until the rerun", and both are known here. The MOMENT goes on the
        // state rather than a countdown, so the card can recompute the wait on every UI tick instead
        // of showing a number that was true when it was written.
        void PublishSchedule()
        {
            var boarding = next.Keys.Where(r => r.BoardingRunning).Select(NameOf).ToList();
            if (next.Count == 0)
            {
                state.Schedule = "no rows scheduled";
                state.NextActionAt = null;
                return;
            }

            var soonest = next.OrderBy(kv => kv.Value).First();
            var queued = next.Count - 1;
            state.NextActionAt = soonest.Value;
            state.Schedule = (boarding.Count > 0
                    ? $"boarding {string.Join(", ", boarding)}"
                    : "nothing boarding") +
                $"  ·  next {NameOf(soonest.Key)} at {soonest.Value:HH:mm}" +
                (queued > 0 ? $"  ·  {queued} more queued" : "");
        }

        PublishSchedule();

        try
        {
            // QuitPressed stays false for this tool by construction — it is the one tool that ignores
            // the quit hotkey, so that a press meant for a foreground run cannot end a schedule that
            // is feeding four pets. The check is kept because the loop should stop on a stop, whatever
            // sets the flag; the cancellation token is the one that actually arrives.
            while (!QuitPressed && !ct.IsCancellationRequested)
            {
                var live = next.Keys.ToList();
                if (live.Count == 0)
                {
                    state.Message = "Every row has failed repeatedly — stopped.";
                    break;
                }

                var soonest = live.OrderBy(r => next[r]).First();
                if (!SleepUntil(next[soonest], ct)) break;

                // EVERY ROW DUE NOW goes into ONE visit, in ROW ORDER — the player's "row 1, row 2,
                // row 3". Normally that is one row and the visit is exactly what it always was; a cold
                // start, or RELOAD EVERY ROW ON START, makes it four — which used to be four opens.
                var ordered = _cfg.Pet.ActiveRows.ToList();
                var now = DateTime.Now;
                var batch = live.Where(r => next[r] <= now)
                                .OrderBy(r => ordered.IndexOf(r))
                                .ToList();
                if (batch.Count == 0) batch.Add(soonest);

                // The soonest row is due, but the game may belong to another tool. Deferring here needs
                // no bookkeeping at all: next[row] is left in the past, so the next pass picks this same
                // row again — which is why the wait belongs here and not in the scheduler. It is never
                // a silent wait; see WaitingForRow.
                if (!ClaimGame(state, () => WaitingForRow(soonest, next[soonest], GameOwner), ct)) break;

                VisitResult visited;
                try
                {
                    visited = Visit(ser, state, batch);
                }
                finally
                {
                    ReleaseGame();
                }

                // THE RE-MEASURE, for every row the visit did NOT act on. A row it did reload gets its
                // next from the reload below, because the reading was taken BEFORE the reload and
                // describes the state that reload just replaced.
                foreach (var (readRow, readNext) in visited.Schedule)
                    if (next.ContainsKey(readRow) && !visited.Rows.ContainsKey(readRow))
                        next[readRow] = readNext;

                foreach (var (row, result) in visited.Rows)
                {
                    if (!next.ContainsKey(row)) continue;

                    if (result.Ok)
                    {
                        failures[row] = 0;

                        // WHICHEVER RUNS OUT FIRST: what the pet still needs, or what was just loaded.
                        // The pet's figure came from the panel the guard read and the line its queue
                        // entry names; the food's is the full load that was just put in. With no pet
                        // figure this is CycleMinutesFor exactly as before — no line named, no panel
                        // read, or no row for that line at that stage all land here.
                        var loaded = _cfg.Pet.LoadMinutesFor(row);
                        var wait = result.PetMinutes is { } needs
                            ? Math.Max(0, Math.Min(needs, loaded))
                            : loaded;
                        next[row] = DateTime.Now.AddMinutes(wait + WaitAfterEmptyMinutes);

                        state.Message = $"{NameOf(row)} reloaded. Next {next[row]:HH:mm}." +
                                        (result.PetMinutes is { } m && m < loaded
                                            ? $" (the pet finishes first, in {m:0} min)"
                                            : "");
                        Console.WriteLine(state.Message);
                    }
                    else if (result.NothingToBoard)
                    {
                        // NOT A FAILURE. The bag holds no pet that can be fed — every one has finished,
                        // or none is in there. The player may simply not have another yet, and a row
                        // must not be counted against, or dropped, for the state of the bag.
                        failures[row] = 0;
                        next[row] = DateTime.Now.AddMinutes(NoPetRetryMinutes);
                        state.Message = $"{NameOf(row)} is waiting for a pet that can be fed.";
                        Log($"  {NameOf(row)}: nothing in the bag can be boarded — the row keeps its " +
                            $"place and is looked at again in {NoPetRetryMinutes} min. NOT a failure.");
                        Console.WriteLine(state.Message);
                    }
                    else
                    {
                        failures[row]++;
                        state.Message = $"{NameOf(row)} failed {failures[row]}/{MaxFailures}: {result.Error}";
                        Console.WriteLine(state.Message);

                        // A row that keeps failing is DROPPED rather than taking the run with it. With
                        // four rows that matters: one bad calibration should not stop the other three
                        // being fed, and previously any row reaching MaxFailures stopped everything.
                        if (failures[row] >= MaxFailures)
                        {
                            Log($"  {NameOf(row)} given up on after {MaxFailures} failures — the other " +
                                "rows carry on");
                            next.Remove(row);
                            continue;
                        }
                        next[row] = DateTime.Now.AddMinutes(RetryMinutes);
                    }
                }

                PublishSchedule();
                if (visited.Rows.Values.All(r => r.Ok)) Beep(523, 100);
                else Beep(200, 400);
            }
        }
        finally
        {
            state.Running = false;
        }

        Console.WriteLine("Pet tool done.");
        return 0;
    }

    /// <summary>The owner name this tool claims the game under — the same as its tool id, so a card
    /// that says it is waiting for "gem" names the tool the player just started.</summary>
    private const string GateOwner = "pet";

    /// <summary>Who holds the game while this tool is waiting, for the line on the card. "?" only when
    /// there is no gate at all, in which case nothing is ever waiting.</summary>
    private string GameOwner => _gate?.Owner ?? "?";

    /// <summary>Waits for the game to be free and takes it, or false when the run was cancelled while
    /// waiting. Returns true immediately when no gate was handed in.
    ///
    /// This is what makes the feeder resident: it needs the game for about thirty seconds at a time,
    /// five times a day, so rather than the launcher stopping it to run something else, it steps aside
    /// and comes back. It is never a silent wait — the card says which tool it is waiting for, because
    /// a stalled pet feeder and a broken one look identical otherwise.</summary>
    private bool ClaimGame(ToolState state, Func<string> waiting, CancellationToken ct)
    {
        var gate = _gate;
        if (gate == null || gate.TryAcquire(GateOwner)) return true;

        var said = "";
        while (!gate.TryAcquire(GateOwner))
        {
            if (ct.IsCancellationRequested) return false;

            var message = waiting();
            state.Message = message;
            // Only on a change: the wait can last for hours, and a line every two seconds would bury
            // the reload history that this log exists to keep.
            if (message != said)
            {
                said = message;
                Console.WriteLine(message);
                Log("  " + message);
            }

            if (!SleepUntil(DateTime.Now.AddSeconds(2), ct)) return false;
        }
        return true;
    }

    /// <summary>What to say while a row waits for the game, including how long its food lasts.
    ///
    /// When the food actually runs out is exactly computable with no new state: a row is scheduled at
    /// (fill + LoadMinutes + WaitAfterEmpty), so the last WaitAfterEmpty of that window is the time it
    /// spends empty. Until then, waiting costs the pet nothing — and after it, the card has to say so,
    /// because "starving quietly" and "feeding fine" look the same from outside.</summary>
    private string WaitingForRow(PetSlotConfig row, DateTime dueAt, string owner)
    {
        var slack = dueAt.AddMinutes(-WaitAfterEmptyMinutes) - DateTime.Now;
        return slack > TimeSpan.Zero
            ? $"{NameOf(row)} is due — waiting for {owner} ({slack.TotalMinutes:0} min of food left)"
            : $"{NameOf(row)} is OUT of food — still waiting for {owner}";
    }

    /// <summary>Gives the game back. Always called — a tool that dies holding the gate would leave the
    /// launcher waiting on it for the rest of the session.</summary>
    private void ReleaseGame() => _gate?.Release(GateOwner);

    /// <summary>Why a run can't start, or null when it can. Checked before anything moves, so a
    /// half-calibrated setup says what is missing instead of clicking into empty screen.</summary>
    private string? Ready(ToolState state)
    {
        _ = state;
        var pet = _cfg.Pet;

        if (!IsPoint(pet.MenuButton)) return "目錄 isn't calibrated — Calibrate Pet.";
        if (!IsPoint(pet.FeedIcon)) return "The pet feed icon isn't calibrated — Calibrate Pet.";
        if (!IsPoint(pet.CloseButton)) return "The boarding window's X isn't calibrated — Calibrate Pet.";
        if (!BagGrid.IsValidRect(pet.BagGrid) || !BagGrid.IsValidRect(pet.BagSlot))
            return "The boarding bag's grid isn't calibrated — Calibrate Pet. (Its own, not the shop's.)";
        if (pet.PageTabs.Count == 0) return "The bag page tabs aren't calibrated — Calibrate Pet.";
        if (_cfg.Pet.Slots.Count == 0)
            return "No breeding row is set up — Calibrate Pet. Each row needs its start/end button " +
                   "and its pet slot marked.";
        if (!_cfg.Pet.ActiveRows.Any())
            return "Every row is unticked — nothing to feed. Tick a row on the Pet tab.";

        // EVERY row the tool DRIVES, not just the first. A row with no button marked would otherwise
        // be discovered mid-run — as a click at (0,0) or at whatever the empty box's centre works out
        // to, during an unattended run.
        //
        // Only the ticked ones: a row that is switched off is not driven, so demanding it be fully
        // calibrated is what used to make a half-set-up row block a run that never touches it.
        for (int i = 0; i < _cfg.Pet.Slots.Count; i++)
        {
            if (!_cfg.Pet.Slots[i].Enabled) continue;
            if (!BagGrid.IsValidRect(_cfg.Pet.Slots[i].ToggleLabel))
                return $"Row {i + 1}'s start/end button isn't marked — Calibrate Pet. The tool has " +
                       "nothing to click for that row. Untick it on the Pet tab to leave it alone.";
        }

        // A DRAG has to name the box it drops into, and the food strip is what says where the boxes
        // are. Right-click does not need it — it lets the game choose, which is the entire difference
        // between the two modes — so this is required only when the drag is the mode.
        if (FoodLoadMode.IsDrag(pet.FoodLoadMode))
        {
            for (int i = 0; i < pet.Slots.Count; i++)
            {
                if (!pet.Slots[i].Enabled) continue;
                if (FeederLayout.SlotBoxes(pet.Slots[i].FeederStrip, pet.Slots[i].Stacks) is null)
                    return $"Row {i + 1}'s food strip isn't drawn — Calibrate Pet. The food load is " +
                           "set to drag, which has to name the box it drops into, and the strip is " +
                           "what says where the boxes are.";
            }
        }

        // Every marked cell carries a PAGE, so a tab that was never calibrated is a mark pointing at a
        // page the tool cannot reach. Checked here rather than discovered mid-flow: the reload closes
        // the window on any failure, so a bad page surfaced as "it opened and then shut again" with
        // nothing saying why.
        var pages = new List<(string What, int Page)>();
        if (pet.ReturnSlot is { Count: 2 } rs) pages.Add(("The return slot", rs[0]));
        foreach (var cell in pet.FoodSlots)
            if (cell is { Count: 2 }) pages.Add(("A food cell", cell[0]));

        foreach (var (what, page) in pages)
        {
            if (page < 0 || page >= pet.PageTabs.Count)
                return $"{what} is marked on bag page {page + 1}, but only {pet.PageTabs.Count} page " +
                       "tab(s) are calibrated — mark the tab on Calibrate Pet, or move the cell.";
            if (!IsPoint(pet.PageTabs[page]))
                return $"{what} is marked on bag page {page + 1}, and that page's tab isn't " +
                       "calibrated — mark ITEM" + (page + 1) + " on Calibrate Pet.";
        }
        // The fixed cell is only required while there is no queue to find the pet by — see
        // PetConfig.Queue. With icons captured the tool looks for the pet rather than for a position.
        if (pet.ReturnSlot is not { Count: 2 } && pet.Queue.Count == 0)
            return "Neither a return slot nor any queued pet icon is set — the tool has nothing to " +
                   "find the pet by. Mark the return slot on the Pet tab, or capture a pet icon.";
        if (pet.FoodSlots.Count == 0)
            return "No food cells are marked — Calibrate Pet. Nothing says where the pet food lives.";
        if (EffectiveMax() == null)
            return "No MAX is calibrated — Calibrate Buy/Sell, or mark one on Calibrate Pet.";
        return null;
    }

    /// <summary>The count dialog's MAX: the pet override when set, otherwise the shared buy/sell one.
    /// They are the same dialog, so normally one mark serves both.</summary>
    private List<int>? EffectiveMax() => _cfg.Pet.MaxButton ?? _cfg.BuySell.MaxButton;

    /// <summary>How a row is named in a log line or on the card. 1-based, matching the window.</summary>
    private string NameOf(PetSlotConfig row) =>
        $"Row {_cfg.Pet.Slots.IndexOf(row) + 1}";

    /// <summary>Opens the breeder once at the start of a run and LOOKS — no reload, nothing clicked
    /// but the two icons to open the window and the X to close it.
    ///
    /// This exists because the player has a breeder running when they press Start, and reloading
    /// everything on start means ending four feeds that were already going and redoing them — a
    /// minute of clicking and a round of food cells to learn what one look tells us.
    ///
    /// What it reads is the boarding slot, which already had a reference crop for the "did the pet
    /// actually go in?" check. That same comparison answers "is a pet in there right now", and an
    /// empty slot cannot be boarding — so the answer is one-way and safe even though the tool never
    /// learned to read the toggle label:
    ///
    ///     slot EMPTY     → boarding is stopped, certainly
    ///     slot occupied  → boarding is running... which is also the state a pet sits in before Start
    ///                      is pressed, so it does NOT prove it
    ///     unreadable     → keep whatever the player ticked on the Pet tab
    ///
    /// Only the first line is a proof, and that is enough: the rows it settles are exactly the ones a
    /// blind reload would have disturbed for no reason.</summary>
    private Dictionary<PetSlotConfig, DateTime> InspectRows(SerialPort ser, ToolState state)
    {
        state.Message = "Checking what is already running…";
        Log("start: opening the breeder to check each row (no reload)");

        if (!OpenBoarding(ser, out var error))
        {
            Log("  couldn't open the breeder to check: " + error + " — the ticked state is used");
            return UnreadSchedule();
        }

        // Every exit past this point closes the window — the look is a visit like any other.
        try
        {
            return ReadRows(ser);
        }
        finally
        {
            CloseBoarding(ser);
        }
    }

    /// <summary>The schedule for a look that could not happen: every row falls back to its configured
    /// cycle. A null count already means "could not read", which <see cref="ScheduleFor"/> treats as a
    /// full load.</summary>
    private Dictionary<PetSlotConfig, DateTime> UnreadSchedule()
    {
        var schedule = new Dictionary<PetSlotConfig, DateTime>();
        foreach (var row in _cfg.Pet.ActiveRows) schedule[row] = ScheduleFor(row, null, null);
        return schedule;
    }

    /// <summary>Reads every active row's slot and feeder — the LOOK, with the boarding window already
    /// OPEN. Split out of <see cref="InspectRows"/> for one reason: a visit has to read the rows it is
    /// about to act on, and it must not open the window a second time to do it. There is deliberately
    /// ONE reader — two would eventually disagree about what a row's state is.
    ///
    /// It WRITES the rows: BoardingRunning is set from what the slot says. The schedule comes back so
    /// the caller can re-derive every row's next from THIS reading rather than from an old one.</summary>
    private Dictionary<PetSlotConfig, DateTime> ReadRows(SerialPort ser)
    {
        var schedule = new Dictionary<PetSlotConfig, DateTime>();

        // The engine is built here and disposed with the look, rather than held for the life of the
        // run: the counts are wanted once per visit, and an ONNX session parked in memory for days to
        // be used once is a cost with nothing buying it.
        OcrEngine? ocr = null;
        try
        {
            // ActiveRows, not Slots: a row with its tick off is meant to be invisible, and reading
            // its slot is the first thing that would act on it.
            foreach (var row in _cfg.Pet.ActiveRows)
            {
                var difference = PetSlotDifference(row);
                var empty = difference is { } d ? d <= _cfg.Pet.PetSlotOccupiedAbove : (bool?)null;
                switch (empty)
                {
                    case true:
                        row.BoardingRunning = false;
                        Log($"  {NameOf(row)}: slot reads EMPTY, so this row is not boarding — " +
                            "it will be reloaded now");
                        break;
                    case false:
                        row.BoardingRunning = true;
                        Log($"  {NameOf(row)}: a pet is in the loader");
                        break;
                    default:
                        Log($"  {NameOf(row)}: slot couldn't be read — keeping the ticked state " +
                            $"({(row.BoardingRunning ? "boarding" : "not boarding")})");
                        break;
                }

                if (difference is { } diff)
                    Log($"  {NameOf(row)}: slot differs from its empty reference by {diff:0.###} " +
                        $"(occupied above {_cfg.Pet.PetSlotOccupiedAbove:0.###})" +
                        (diff > _cfg.Pet.PetSlotOccupiedAbove && diff < 0.25
                            ? "  ← marginal: if this row is EMPTY, re-capture its reference"
                            : ""));

                // Only worth reading a feeder on a row that is actually boarding — an empty one has
                // nothing loaded and will be filled in a moment anyway.
                if (!row.BoardingRunning) { schedule[row] = ScheduleFor(row, null, null); continue; }

                ocr ??= new OcrEngine(_cfg, _attrs, _rootDir);
                var left = ReadFeederCounts(ocr, row);
                var eta = ReadEta(ocr, row);
                schedule[row] = ScheduleFor(row, left, eta);

                // The time line is quoted RAW, because it is a sentence rather than a number and the
                // only way to know the region found it is to see what it read. A row whose line is
                // missing here reads as the food figure, which is what happened before it existed.
                var quoted = eta is null ? "  [no time line read]" : $"  [{eta.Line}]";

                if (left is { } items)
                    Log($"  {NameOf(row)}: {items} item(s) left in the feeder — reloading in " +
                        $"{Math.Max(0, (schedule[row] - DateTime.Now).TotalMinutes):0} min{quoted}");
                else
                    Log($"  {NameOf(row)}: the feeder counts couldn't be read — reloading in " +
                        $"{Math.Max(0, (schedule[row] - DateTime.Now).TotalMinutes):0} min{quoted}");
            }
            _persistState?.Invoke();
        }
        finally
        {
            ocr?.Dispose();
        }

        return schedule;
    }

    /// <summary>When a row next needs a reload, from what was read about it.
    ///
    /// <paramref name="itemsLeft"/> is the feeder count when it could be read. From that the reload
    /// lands just past the moment the row runs dry — the same "past empty, not early" rule the
    /// configured cycle follows, and for the same reason: reloading onto a part-used slot is the case
    /// whose behaviour is unknown.
    ///
    /// **ZERO IS RELOAD NOW, with no margin.** The wait exists to arrive *after* a tray that still has
    /// food in it has drained; a tray already reading zero IS past empty, so there is nothing to wait
    /// for. The player asked the obvious question — *"if we know it is empty, why wait another 5
    /// minutes?"* — and the answer was that it was sleep for its own sake. The five minutes cost the pet
    /// food, which is the opposite of what a margin is for.
    ///
    /// Null means either "not boarding" (reload now — it needs a pet and a fill) or "couldn't read"
    /// (fall back to the configured cycle, which assumes a full load). The two are different and the
    /// caller logs which.</summary>
    private DateTime ScheduleFor(PetSlotConfig row, int? itemsLeft, FeederEta.Reading? eta)
    {
        if (_cfg.Pet.ReloadOnStart) return DateTime.Now;
        if (!row.BoardingRunning) return DateTime.Now;

        // THE GAME'S OWN COMPLETION TIME, when it gives one. A pet that finishes is MAILED, so the row
        // will be empty then and wants a new one — the case that otherwise waits out a whole cycle with
        // nothing boarding, which is exactly what row 3 was doing at 17:47: six hours scheduled, a pet
        // fourteen minutes from done. Read live; see FeederEta.
        var finish = FeederEta.CompletionMinutes(eta);

        if (itemsLeft is not { } items)
        {
            // No counts. A finish time is still a real answer, and a far better one than the cycle.
            return finish is { } f
                ? DateTime.Now.AddMinutes(f + WaitAfterEmptyMinutes)
                : DateTime.Now.AddMinutes(CycleMinutesFor(row));
        }

        if (itemsLeft <= 0) return DateTime.Now;

        var rate = Math.Max(1, _cfg.Pet.ItemsPerMinute);
        var food = Math.Max(0, itemsLeft.Value / (double)rate);

        // WHICHEVER RUNS OUT FIRST — the food in the tray, or the pet itself.
        return DateTime.Now.AddMinutes(
            (finish is { } fin ? Math.Min(food, fin) : food) + WaitAfterEmptyMinutes);
    }

    /// <summary>The row's time line — the game's own statement of when this boarding completes. Null
    /// when there is nothing readable there, which the schedule answers with the food figure.</summary>
    private static FeederEta.Reading? ReadEta(OcrEngine ocr, PetSlotConfig row)
    {
        if (FeederLayout.EtaRegion(row.FeederStrip) is not { } region) return null;

        return FeederEta.Parse(ocr.ReadLines(
            new RegionConfig { Left = region[0], Top = region[1], Width = region[2], Height = region[3] },
            3, null));
    }

    /// <summary>How many food items are left in a row, read off the counts the game draws on each of
    /// its slots. Null when it cannot be read.
    ///
    /// This is the read the tool has most wanted: the boarding slot answers "is a pet in there", and
    /// nothing answered "how much food is left" — so a row whose feeder had run dry overnight looked
    /// exactly like one filled a minute ago, and the schedule had to assume a full load. The player
    /// has had to tick a box to say otherwise.
    ///
    /// **A partial read understates the food, and that is the safe direction.** A slot with nothing in
    /// it reads nothing and is simply not counted, so a row where three of five counts were legible
    /// looks emptier than it is — and reloading early is nearly free, because ending boarding returns
    /// the leftover food with the pet. Overstating would leave a pet unfed, which is the one thing
    /// here that cannot be undone.</summary>
    private int? ReadFeederCounts(OcrEngine ocr, PetSlotConfig row)
    {
        // The row's slots, derived from the strip the player dragged rather than from one box per
        // slot. No strip means no reading, which the caller treats as "assume a full load".
        if (FeederLayout.SlotBoxes(row.FeederStrip, row.Stacks) is not { } slots) return null;

        var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
        if (hwnd == IntPtr.Zero || WindowFinder.IsMinimized(hwnd)) return null;

        // The read is a SCREEN GRAB, so anything in front of the game is what gets measured. The
        // composer refuses to judge its empty box for the same reason — measuring the launcher's own
        // UI once produced a confident answer about a window that had nothing to do with the game.
        if (WindowFinder.ForegroundWindow() != hwnd) return null;

        var total = 0;
        var counted = 0;

        // EVERY SLOT LOGGED, not just the total. The total cannot say whether a slot was missed, read
        // as zero, or read wrong — and that is the difference between "the feeder holds 300" and "one
        // of its two slots did not read, so the tool thinks it holds 300". It cost a live round trip:
        // row 1's slots showed 105 and 300, the tool scheduled from 300, and the log had nothing to
        // say about which slot had gone missing.
        var seen = new List<string>();
        for (int i = 0; i < slots.Count; i++)
        {
            var n = FeederCountOf(ocr, slots[i]);
            seen.Add(n is { } v ? v.ToString(CultureInfo.InvariantCulture) : "—");
            if (n is { } value) { total += value; counted++; }
        }

        Log($"  feeder counts: [{string.Join(" / ", seen)}] -> {total} " +
            $"({(counted == 0 ? "nothing read" : $"{counted} of {slots.Count} slots read")})");

        // NOTHING read is null; ZERO is a real reading meaning an empty feeder. Summing into a total
        // that starts at zero conflates them, and the two mean opposite things downstream: null says
        // "assume a full load and keep to the cycle", zero says "reload NOW". A partial read understates
        // deliberately — a row that reads half its slots looks emptier than it is, and reloading early
        // is nearly free because ending boarding returns the leftover food with the pet.
        return counted == 0 ? null : total;
    }

    /// <summary>One food slot's count, or null when it cannot be read with confidence.
    ///
    /// The region comes from the REFERENCE the player dragged — one slot and the count box on it —
    /// applied to this slot. Two crops a few pixels apart are then read, and the number only counts
    /// when BOTH return the same thing. That is not belt-and-braces: the measured failure of a
    /// slightly-wrong crop is a confident WRONG number — "300" came back as "0" at 0.56 and "84" as
    /// "4" at 0.89 — and clipping changes the answer between two crops while a genuine read does not.
    /// See <see cref="FeederCount"/> and <see cref="FeederLayout"/>.</summary>
    /// <summary>One food slot's count, or null when it cannot be read.
    ///
    /// ONE read of ONE crop, because that is what the measurements showed is needed: given the right
    /// crop the count reads at 0.99-1.00 at every upscale. Two earlier revisions did more — four crops
    /// with a majority vote, and before that a proportional fraction of the slot — and both were
    /// machinery around a crop nobody had found yet. There is nothing to search for now: the crop is
    /// four tenths of the way across each slot, computed from the slot itself.</summary>
    private int? FeederCountOf(OcrEngine ocr, List<int>? slotBox)
    {
        if (RegionFor(slotBox) is not { } region) return null;

        if (ReadStack(ocr, region) is { } count) return count;

        // AN EMPTY SLOT IS NOT A FAILURE. It holds no food, so it has no digits, and zero is the right
        // answer — taken FIRST, so a healthy run writes no files and the save below fires only on the
        // case that is genuinely odd. This order cost a live run two confusing saves before it was
        // right: the log said "a slot read NOTHING" about a slot that was simply empty.
        //
        // NO NUMBER is either a crop the reader failed on or a slot with NOTHING IN IT, and those mean
        // opposite things: one says "assume a full load", the other says "reload now". The pixels settle
        // it. See FeederCount.WarmFraction.
        if (SlotHasNoFood(region) is true) return 0;

        // FOOD IS THERE AND NO NUMBER WAS READ. That one is worth its pixels: a crop in the wrong place
        // and a crop the reader cannot see text in look identical in the log and need opposite fixes —
        // and today a slot holding a legible 300 read as nothing and left three theories and no evidence.
        SaveFailedSlotCrop(ocr, region);
        return null;
    }

    /// <summary>Saves the image a failed slot read was given, so a blank reading arrives with its
    /// pixels attached. The same image the reader saw — the engine writes it upscaled, which is what
    /// recognition actually ran on — plus the lines it made of it.
    ///
    /// Best-effort: evidence is a convenience and must never fail a reload.</summary>
    private void SaveFailedSlotCrop(OcrEngine ocr, List<int> region)
    {
        try
        {
            var dir = System.IO.Path.Combine(AppContext.BaseDirectory, "logs", "reads");
            System.IO.Directory.CreateDirectory(dir);

            var path = System.IO.Path.Combine(dir,
                $"feeder_blank_{DateTime.Now:yyyyMMdd_HHmmss_fff}.png");

            ocr.ReadLinesScored(
                new RegionConfig { Left = region[0], Top = region[1], Width = region[2], Height = region[3] },
                3, path, _cfg.Pet.FeederCountMinScore);

            Log($"  a slot read NOTHING — saved what the reader was given: " +
                System.IO.Path.GetFileName(path));
        }
        catch
        {
            // ignore — see the summary
        }
    }

    /// <summary>Whether the slot's crop shows a cell with no food in it — or null when that cannot be
    /// told.
    ///
    /// Only consulted when the OCR found NO number, so it costs a screen grab on the slots that failed
    /// to read rather than on every slot of every row.
    ///
    /// The reason it exists, live on 2026-09-21: a row whose feeder had run dry read as "the counts
    /// couldn't be read", the tool assumed a full load, and the pet sat unfed while the row waited 205
    /// minutes. Both its slots showed a blank cell — no digits, because there was nothing in them.</summary>
    private bool? SlotHasNoFood(List<int> region)
    {
        var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
        if (hwnd == IntPtr.Zero || WindowFinder.IsMinimized(hwnd)) return null;

        var cap = ScreenCapture.CaptureClientRegion(hwnd, new RegionConfig
        {
            Left = region[0], Top = region[1], Width = region[2], Height = region[3],
        });
        if (cap == null) return null;

        using var img = cap.Image;
        var warm = FeederCount.WarmFraction(img);

        // Logged either way: the number is what says whether the threshold is sitting in a gap or on a
        // cliff, and that is the difference between a measurement and a guess.
        Log($"  a slot with no number is {warm * 100:0.0}% food-coloured " +
            $"(empty below {FeederCount.EmptySlotBelow * 100:0.0}%)");

        return warm < FeederCount.EmptySlotBelow;
    }

    /// <summary>Where to read a given slot: this far across it, to its own right edge, at its full
    /// height.
    ///
    /// The position is COMPUTED from the slot rather than taken from a box the player drew, and that
    /// is the outcome of four attempts. Three of them put the read position under a hand-drawn box —
    /// a count box, a count slot read as drawn, and a count slot applied by offset — and all three
    /// failed, because the band of positions that reads is about three pixels wide and a hand cannot
    /// put a box into three pixels. Measured four times, missed four times.
    ///
    /// Null when the slot or the setting is unusable, which the caller treats as no reading.</summary>
    private List<int>? RegionFor(List<int>? slotBox) =>
        FeederLayout.ReadRegion(slotBox, _cfg.Pet.FeederCountLeftFraction);

    /// <summary>One crop, read to a number.</summary>
    private int? ReadStack(OcrEngine ocr, List<int> box)
    {
        var region = new RegionConfig { Left = box[0], Top = box[1], Width = box[2], Height = box[3] };
        // Scored, so the caller can pick the BEST line rather than the first survivor: this crop comes
        // back with the real count beside junk from the food icon, and which is which is the score.
        var min = _cfg.Pet.FeederCountMinScore;
        return FeederCount.Parse(ocr.ReadLinesScored(region, 3, null, min), min);
    }

    /// <summary>How long one load lasts, minus the safety margin — i.e. when to reload next.
    /// Derived from the config rather than stored, so the rate and the load can never disagree with
    /// the interval they produce.
    ///
    /// Per ROW, because the free row holds two stacks and a paid row five — so the two empty at
    /// different times and one timer cannot serve both.</summary>
    private double CycleMinutesFor(PetSlotConfig row)
    {
        var full = _cfg.Pet.LoadMinutesFor(row);
        if (full <= 0) return 60;   // a broken rate must not spin the loop
        return Math.Max(5, full + WaitAfterEmptyMinutes);
    }

    // ── One visit ───────────────────────────────────────────────────────────

    /// <summary>What one visit to the boarding window did, per row, and the schedule it read.
    ///
    /// The schedule covers EVERY active row, not just the rows the visit touched, because a visit
    /// re-derives each row's next from what it just read. A row whose pet has finished, or whose feeder
    /// ran dry early, is then noticed on the visit that happens to be nearby instead of whenever its
    /// own hours-old timer comes round.</summary>
    private sealed record VisitResult(
        Dictionary<PetSlotConfig, RowOutcome> Rows,
        Dictionary<PetSlotConfig, DateTime> Schedule);

    /// <summary>One row's result from a visit. <paramref name="PetMinutes"/> is how long the pet that
    /// was boarded still needs, or null when that could not be answered — the caller falls back to the
    /// configured cycle either way.
    ///
    /// <paramref name="NothingToBoard"/> says the row could not be filled because the BAG holds no
    /// feedable pet — every queued one has finished, or none is in there at all. That is a WAIT, not a
    /// fault: the player may simply not have another pet yet, and a row must not be counted against, or
    /// dropped, for the state of the bag.</summary>
    private sealed record RowOutcome(bool Ok, string Error, double? PetMinutes, bool NothingToBoard);

    /// <summary>Everything one boarding-window open should do: read every row, then do the rows that
    /// are due — one at a time, in row order — and close once.
    ///
    /// WHY A VISIT EXISTS. The reading was already batched — one open reads every row — and the ACTING
    /// was not: the reload opened and closed around a single row. Four rows due at once was four
    /// opens and three closes, nine window operations where one would do, and two of a live run's four
    /// failed on exactly that reopen.
    ///
    /// THE SINGLE-ROW CASE IS THE POINT. Most of the day one row comes due on its own, and this then
    /// does what it always did: open, that row, close. The batching changes nothing about it, which is
    /// the constraint the whole restructure is judged against.
    ///
    /// A ROW THAT FAILS DOES NOT TAKE THE VISIT WITH IT. Each row is reported separately, the rest are
    /// still attempted, and the window closes once at the end either way.</summary>
    private VisitResult Visit(SerialPort ser, ToolState state, List<PetSlotConfig> batch)
    {
        var results = new Dictionary<PetSlotConfig, RowOutcome>();
        var schedule = new Dictionary<PetSlotConfig, DateTime>();

        state.Message = batch.Count > 1
            ? $"Opening the breeder for {batch.Count} rows…"
            : $"{NameOf(batch[0])}: opening the boarding window…";
        Log($"visit: opening the breeder once for {batch.Count} row(s) — " +
            string.Join(", ", batch.Select(NameOf)));

        if (!OpenBoarding(ser, out var openError))
        {
            Log("  FAILED opening: " + openError);
            foreach (var row in batch) results[row] = new RowOutcome(false, openError, null, false);

            // NO SCHEDULE, deliberately. The reader never ran, so there is nothing to re-derive — and
            // returning the fallback schedule here would OVERWRITE every other row's measured next with
            // "assume a full load", turning one failed open into a run-wide loss of what was known.
            return new VisitResult(results, new Dictionary<PetSlotConfig, DateTime>());
        }

        // Every exit past this point closes the window: leaving it open would sit on top of the game
        // while the tool waits out its next cycle, and the next cycle would click 目錄 behind it.
        try
        {
            // READ FIRST, and read EVERYTHING — the due rows to act on, and the rest because this
            // reading is also what re-derives their next.
            schedule = ReadRows(ser);

            foreach (var row in batch)
            {
                var ok = ReloadRowInPlace(ser, state, row, out var rowError, out var petMinutes,
                    out var nothingToBoard);
                results[row] = new RowOutcome(ok, rowError, petMinutes, nothingToBoard);

                if (!ok)
                    Log($"  {NameOf(row)} failed inside the visit — the other rows carry on");
            }
        }
        finally
        {
            CloseBoarding(ser);
        }

        return new VisitResult(results, schedule);
    }

    // ── One reload ──────────────────────────────────────────────────────────

    /// <summary>Reloads ONE ROW, with the boarding window ALREADY OPEN.
    ///
    /// The open and the close used to live here, and that is the whole reason a row was a window visit:
    /// the loop called this once per row and every call opened and closed. They belong to
    /// <see cref="Visit"/> now, so rows that come due together are done in ONE open. Everything between
    /// them — ending a boarding, placing the pet, loading the food, starting — is unchanged.
    ///
    /// The rows are still serviced one at a time, which is the player's own ordering constraint
    /// (2026-09-19): each row is offloaded and re-boarded before the next is touched.</summary>
    private bool ReloadRowInPlace(SerialPort ser, ToolState state, PetSlotConfig row,
        out string error, out double? petMinutes, out bool nothingToBoard)
    {
        error = "";
        petMinutes = null;
        nothingToBoard = false;

        // A plain block rather than a try/finally: there is nothing to clean up per row any more, and
        // the braces are kept so this stays a reviewable diff against the method it was — this is the
        // most heavily live-tested code in the tool and a reindent would bury the real change.
        {
            // Each step names itself on the card as it runs. Without this a failure reads as "it opened
            // the window and then closed it again", because the close is the cleanup and it is the only
            // thing the eye catches.
            state.Message = $"{NameOf(row)}: reloading…";
            // The pet has to be IN THE BAG before it can be put back, and while boarding runs it is
            // in the loader instead. So a reload off a running boarding starts by ending it — which is
            // also what returns the leftover food. Skipped when boarding is already stopped, because
            // this button TOGGLES: pressing it then would start the very thing we are about to end.
            if (row.BoardingRunning)
            {
                state.Message = $"{NameOf(row)}: ending boarding…";
                Log("  ending boarding (the pet is in the loader, so it has to come back first)");
                if (!PressToggle(ser, row, out error)) { Log("  FAILED ending: " + error); return false; }

                // The flag follows the PRESS, not the end of the reload. It used to be set once, at the
                // bottom, only on success — so a reload that failed between here and the start left it
                // saying "boarding is running" while the pet was already back in the bag. The retry
                // then trusted it, pressed the toggle to END a boarding that was already stopped, and
                // STARTED one instead: the exact opposite of the step, on the path that runs when
                // something has already gone wrong.
                //
                // Pressing it is what changes the state, so recording it here is recording what
                // happened rather than what was hoped for. It assumes the press landed, which every
                // click in this tool assumes; the pet-slot check after placement is what catches it
                // when that is wrong.
                row.BoardingRunning = false;
                _persistState?.Invoke();
                SleepCheck(Math.Max(EndWait, ActionWait));

                // CHECK THE EFFECT, do not assume it. That press is a TOGGLE, so if the row's state
                // was misread it does the opposite — and it has already done exactly that on a live
                // run: a row that read "occupied" but was stopped got a boarding STARTED on it, with
                // no pet, which the game answered with an error box sitting over the window and
                // blocking every click after it.
                //
                // The pet has to have come back to the bag for the press to have been the right one.
                // An Enter first, because a dialog raised by a bad press would otherwise swallow the
                // read — and Enter on no dialog is a no-op, the same reasoning the open uses.
                Enter(ser, out _);
                SleepCheck(ActionWait);

                if (PetSlotIsEmpty(row) == false)
                {
                    error = $"{NameOf(row)}: the pet did NOT come back out of the loader after " +
                            "pressing the end button — so that press did the opposite of what was " +
                            "asked and a boarding may now be running with nothing in it. Stopping " +
                            "rather than clicking on: check the row in the game, and re-capture its " +
                            "empty-slot reference on Calibrate Pet if the slot is empty.";
                    return false;
                }
            }

            state.Message = $"{NameOf(row)}: placing the pet…";
            Log("  placing the pet");
            if (!PlacePet(ser, row, out error, out nothingToBoard))
            {
                Log("  FAILED placing the pet: " + error);
                return false;
            }

            // HOW LONG THIS PET NEEDS, from the panel the guard just read and the line its queue entry
            // names. Asked HERE, while the pet that was boarded is still the one the fields describe —
            // everything downstream of the food load has moved on. See PLAN-RELOAD-VISIT §7 and §8.
            petMinutes = MinutesForBoardedPet();

            state.Message = $"{NameOf(row)}: loading the food…";
            Log("  loading the food");
            if (!LoadFood(ser, row, out error)) { Log("  FAILED loading the food: " + error); return false; }

            state.Message = $"{NameOf(row)}: starting boarding…";
            Log("  starting boarding");
            if (!StartBoarding(ser, row, out error)) { Log("  FAILED starting: " + error); return false; }

            // Same reasoning as the end above: the press is what starts the feed, so the flag is set
            // here rather than at the end of a reload that can still fail after it (the sleep below
            // cannot fail, but the intent is what matters — the state changed at the press).
            row.BoardingRunning = true;
            _persistState?.Invoke();

            // Let the start take before the cleanup closes the window it was pressed in.
            SleepCheck(Math.Max(StartWait, ActionWait));

            Log("  reload complete");
            return true;
        }
    }

    /// <summary>目錄 → the pet feed icon → the boarding window (which brings the bag up with it),
    /// then an Enter to clear anything the window opened with.
    ///
    /// That Enter is for one case: when the pet has used up ALL its food, opening the breeder raises a
    /// message saying so, and the 結束代養 button cannot be used until it is dismissed. Before that
    /// point there is no message and the Enter does nothing — confirmed by the player, who put it as
    /// "an extra enter doesn't affect anything".
    ///
    /// Sent unconditionally rather than only when the food is known to be out, because the tool cannot
    /// know that: it is the same choice the sell flow makes with its second Enter, and the same
    /// reason — a keystroke with no dialog in front of it is a no-op, where a missing keystroke in
    /// front of one is a reload that stops dead.
    /// </summary>
    private bool OpenBoarding(SerialPort ser, out string error)
    {
        if (!Click(ser, _cfg.Pet.MenuButton!, right: false, "the 目錄 button", out error)) return false;
        SleepCheck(WindowWait);
        if (!Click(ser, _cfg.Pet.FeedIcon!, right: false, "the pet feed icon", out error)) return false;

        SleepCheck(ActionWait);
        if (!Enter(ser, out error)) return false;
        Log("  enter (clears the out-of-food message if the breeder opened with one)");
        SleepCheck(ActionWait);

        // The bag comes up on whatever page it likes, so anything known about the page is now wrong.
        _bagPage = -1;
        return true;
    }

    private void CloseBoarding(SerialPort ser)
    {
        _bagPage = -1;
        if (!IsPoint(_cfg.Pet.CloseButton)) return;
        // Logged even though it cannot fail the reload: it is the cleanup, and on a failed reload it
        // is the LAST thing that moves the cursor. Without a line here the log ends at the failure and
        // the closing reads as a step that ran and did something unexplained.
        if (!Click(ser, _cfg.Pet.CloseButton!, right: false, "the boarding X", out var err))
            Log("  close: couldn't reach the X — " + err);
        else
            Log("  close: clicked the boarding window's X");
        SleepCheck(ClickWait);
    }

    /// <summary>Right-click the marked cell to put the pet back into the boarding slot.
    ///
    /// ONE right-click, with no dialog behind it — confirmed by the player (2026-09-17), not assumed.
    /// A pet is a single item rather than a stack, so nothing asks how many the way the food does.
    ///
    /// This is the SIMPLE path and it is what runs today: the cell is marked, so nothing has to be
    /// found. It holds while the bag is stable, which is the testing case.
    ///
    /// It is NOT correct while farming, and that is a known and accepted gap rather than an oversight:
    /// the pet drops into the first free slot, and loot takes the first free slot too, so the pet does
    /// not necessarily land here. IconMatch exists for exactly that case and is deliberately not wired
    /// in yet — proving the rest of the flow end to end is worth more than solving the hard case
    /// first.</summary>
    private bool PlacePet(SerialPort ser, PetSlotConfig row, out string error, out bool nothingToBoard)
    {
        error = "";
        nothingToBoard = false;
        var pet = _cfg.Pet;

        // Cleared FIRST, so whatever the reload reads after this is about THIS pet or is nothing. A
        // pair left over from the previous row would otherwise be read as this one's.
        _boardedPanel = null;
        _boardedSpecies = null;

        var centres = BagGrid.Centres(pet.BagGrid!);
        if (centres.Count == 0)
        {
            error = "The bag grid isn't calibrated — there is nothing to right-click.";
            return false;
        }

        // THE RETURN SLOT IS TRIED FIRST (player, 2026-09-22). It is ONE cell and one hover against a
        // three-page scan, and it is where the pet this row was just feeding comes back to — so it is
        // both the cheapest thing to check and the likeliest thing to board.
        //
        // The guard below decides whether it is usable: a cell holding a pet that can still be fed is
        // boarded, and anything else — no pet there at all, or one already finished — falls through to
        // the scan. This is the reverse of what the code used to do, which was the queue first with
        // deliberately NO fallback to the return slot; that reasoning held while the slot was assumed
        // empty, and it is not.
        var candidates = new List<PetCandidate>();
        if (pet.ReturnSlot is { Count: 2 } back && back[1] >= 0 && back[1] < centres.Count)
        {
            Log($"  trying the return slot first: page {back[0] + 1}, cell {back[1]}");
            candidates.Add(new PetCandidate(-1, back[0], back[1], 0, "the return slot"));
        }

        // …THEN THE SCAN, for the case the slot could not answer. A pet that is not in the bag either
        // finished and was mailed or was never there, which is the honest end state and is reported as
        // a WAIT rather than as a failure — see the caller.
        var why = "the queue has no icons in it";
        if (pet.Queue.Count > 0) candidates.AddRange(FindQueuedPetCandidates(ser, out why));

        if (candidates.Count == 0)
        {
            error = pet.ReturnSlot is not { Count: 2 }
                ? "Neither a queued pet icon nor a return slot is set — there is nothing to " +
                  "right-click. Mark the return slot or capture a pet icon, on the Pet tab."
                : $"No queued pet is in the bag ({why}). Right-clicked nothing. If the pet finished " +
                  "it was mailed, so this is the queue being empty of live pets — capture the next " +
                  "one's icon on the Pet tab.";
            nothingToBoard = true;   // a WAIT, not a fault — see the caller
            return false;
        }

        // Built once for the placement and disposed with it — the same lifetime rule InspectRows
        // follows. LAZY, because a placement with nothing to check should not pay for an ONNX session.
        OcrEngine? ocr = null;
        try
        {
            var refused = new List<string>();

            foreach (var cand in candidates)
            {
                if (cand.Cell < 0 || cand.Cell >= centres.Count)
                {
                    error = $"Cell {cand.Cell} is outside the bag grid.";
                    return false;
                }

                // After a scan the bag is left on whichever page was swept last, so the page the cell
                // lives on has to be brought back up before anything is clicked on it.
                if (!SelectPage(ser, cand.Page, out error)) return false;

                var (cx, cy) = centres[cand.Cell];

                if (cand.Entry >= 0)
                    Log($"  found a queued pet: page {cand.Page + 1}, cell {cand.Cell}, match {cand.Score:0.###}");

                // THE +9/100% GUARD. A finished pet cannot be boarded: the right-click simply does not
                // place it, which in a live run looked like "the pet did not go in after 3 right-clicks"
                // — three attempts spent on a pet that could never go in — and boarding one raises the
                // error dialog that wedges everything after it.
                //
                // UNKNOWN BOARDS AS BEFORE. A null panel (no tooltip calibrated, the cursor won't move,
                // the read throws, the panel doesn't parse) falls through to the click. The guard may
                // only ever REMOVE a boarding; inventing one would leave a pet unfed, which is the
                // direction this whole tool treats as the unrecoverable one.
                PetPanel? boarded = null;
                if (_cfg.Tooltip.IsSet)
                {
                    // BOTH KINDS ARE HOVERED NOW — the return slot as well as a matched cell — because
                    // for the slot the hover IS the decision: nothing there means nothing to
                    // right-click, and a pet there that has finished means trying the scan instead.
                    ocr ??= new OcrEngine(_cfg, _attrs, _rootDir);
                    boarded = ReadHoverPanel(ocr, ser, cx, cy);

                    if (boarded is { IsFinished: true })
                    {
                        refused.Add($"{cand.Label} at page {cand.Page + 1} cell {cand.Cell}");
                        Log($"  {cand.Label} at page {cand.Page + 1} cell {cand.Cell} reads " +
                            $"{boarded.Describe()} — NOT boarding it, trying the next one");
                        continue;
                    }

                    // THE RETURN SLOT MUST ACTUALLY HOLD A PET. A matched candidate boards even when its
                    // panel cannot be read — unknown must never remove a boarding — but the slot is a
                    // fixed cell that is normally empty, so NO PANEL there means there is nothing to
                    // click rather than that we do not know. Right-clicking an empty cell is the guess
                    // the icon matching exists to avoid.
                    if (boarded == null && cand.Entry < 0)
                    {
                        Log($"  nothing in the return slot (page {cand.Page + 1}, cell {cand.Cell}) — " +
                            "scanning for a queued pet instead");
                        continue;
                    }

                    if (boarded == null)
                        Log("  the hover panel couldn't be read — boarding this one as before");
                }

                // Click, then LOOK. A right-click can fail to register — a live 12-hour run lost
                // roughly half its boarded time to reloads that loaded food into an empty slot and
                // reported success — and the placement is verified to within 2px, so the cursor was on
                // target when the click went out. An intermittent action cannot be made reliable by
                // aiming better; it can only be checked and repeated.
                for (var attempt = 1; attempt <= PlaceAttempts; attempt++)
                {
                    if (!Click(ser, new List<int> { cx, cy }, right: true,
                            $"the PET at cell {cand.Cell} (page {cand.Page + 1})", out error))
                        return false;

                    SleepCheck(Math.Max(PlaceSettle, ActionWait));

                    switch (PetSlotIsEmpty(row))
                    {
                        case false:
                            if (attempt > 1) Log($"  the pet went in on attempt {attempt}");
                            RememberBoarded(cand.Entry, boarded);
                            return true;

                        case null:
                            // No reference, or the region couldn't be read. Unknown is NOT failure —
                            // refusing to run because a safety net is absent would be worse than the
                            // thing it guards.
                            Log("  pet slot not checked (no empty-slot reference, or it couldn't be read)");
                            RememberBoarded(cand.Entry, boarded);
                            return true;

                        default:
                            Log($"  pet slot is still EMPTY after attempt {attempt}");
                            break;
                    }
                }

                error = $"The pet did not go in after {PlaceAttempts} right-clicks — the slot still looks " +
                        "empty. Nothing was loaded. Check the return slot on the Pet tab.";
                return false;
            }

            // Every candidate was refused. Distinct from "no queued pet is in the bag": the pets ARE
            // there, they are simply all finished, and the fix is a different one — capture the icon of
            // a pet that still needs feeding.
            // Nothing usable, and the two shapes of that need different sentences: pets WERE found and
            // every one has finished, or nothing was found at all — the return slot empty and no queued
            // icon matching anything in the bag.
            error = refused.Count > 0
                ? "Every pet that could be boarded reads +9/100%. Refused " +
                  string.Join("; ", refused) + ". Capture the icon of a pet that can still be fed, " +
                  "on the Pet tab."
                : "The return slot holds no pet, and no queued icon matched one in the bag. Capture " +
                  "the icon of a pet that can still be fed, on the Pet tab.";
            nothingToBoard = true;   // a WAIT, not a fault — see the caller
            return false;
        }
        finally
        {
            ocr?.Dispose();
        }
    }

    /// <summary>Reads the hover panel of the bag cell at (cx, cy), or null when it cannot be read.
    ///
    /// A MOVE onto the cell and never a click, for the reason FocusThenHover gives on the launcher
    /// side: a click on a pet in the bag SWITCHES THE EQUIPPED PET, so clicking to measure would
    /// change the very thing being measured.
    ///
    /// Null covers every way this can fail to produce an answer — no tooltip calibrated, the cursor
    /// won't move, the read throws, the panel doesn't parse. The caller boards as before on null.</summary>
    private PetPanel? ReadHoverPanel(OcrEngine ocr, SerialPort ser, int cx, int cy)
    {
        var tip = _cfg.Tooltip;
        if (!tip.IsSet) return null;

        if (!PlaceOn(ser, cx, cy, out _)) return null;
        Log($"  hovering ({cx},{cy}) for the panel — landed {CursorWhere()}");

        // The panel only exists while the cursor RESTS, so the calibrated delay is the read's
        // precondition rather than a nicety.
        //
        // MILLISECONDS to SECONDS, and it is not cosmetic: SleepCheck takes seconds, and passing 700
        // to it does not sleep a little long — it computes 14000 steps of 50 ms and sleeps for ELEVEN
        // AND A HALF MINUTES. A live run sat wedged exactly here, idle at 0% CPU, on the first hover
        // of the guard. The launcher's own hover uses Task.Delay, which does take milliseconds, so the
        // same number is right there and wrong here.
        SleepCheck(tip.HoverDelayMs / 1000.0);

        var region = new RegionConfig
        {
            Left = cx + tip.OffsetX,
            Top = cy + tip.OffsetY,
            Width = tip.Width,
            Height = tip.Height,
        };

        try
        {
            return PetPanel.Parse(ocr.ReadLines(region, 3, null));
        }
        catch
        {
            return null;
        }
    }

    /// <summary>Moves the cursor out of the bag before a capture.
    ///
    /// The scan reads the SCREEN, so whatever is under the pointer lands in the image — and a pet the
    /// cursor covers is a pet the matcher cannot see. The player's bag had exactly this and it read as
    /// a matcher failure for a while: their seventh pet "differed" at every offset, because part of its
    /// portrait was the mouse arrow. It was not a different pet at all.
    ///
    /// The point is the buy/sell one, reused rather than re-marked: it is already calibrated, already
    /// documented as somewhere nothing responds to a click, and it sits outside the bag grid.
    ///
    /// Best-effort — a cursor that will not move is not a reason to skip a scan, and the failure it
    /// would cause (one more pet missed) is the one this is trying to avoid anyway.</summary>
    private void ParkCursor(SerialPort ser)
    {
        if (_cfg.BuySell.ScrollPoint is { Count: 2 } p)
            PlaceOn(ser, p[0], p[1], out _);
    }

    /// <summary>Looks for a queued pet across every calibrated bag page, by icon.
    ///
    /// This is what §13 calls the icon scan, and it exists because POSITION cannot be trusted: a pet
    /// returned by the boarding window lands in the first free bag slot, and the character farms
    /// throughout, so by the time a reload needs to find it the cell it came from holds something
    /// else. The icon is the pet's own portrait and does not move with the bag.
    ///
    /// The winner and the RUNNER-UP are both logged, which is the point of using ScoreAll rather than
    /// FindBestCell: a runner-up close to the winner is a tool that will eventually right-click the
    /// wrong item, and the two scores are the only way to see that coming before it happens.
    ///
    /// No result means every queued pet is absent from the bag — not "the search failed". A pet that
    /// finished is mailed and gone, so there is nothing in the bag to find.</summary>
    /// <summary>One place a queued pet was found: which queue entry, where it is, and how well it
    /// matched. The ENTRY matters because the guard below has to name the pet it refused to board.</summary>
    private readonly record struct PetCandidate(int Entry, int Page, int Cell, double Score, string Label);

    private List<PetCandidate> FindQueuedPetCandidates(SerialPort ser, out string why)
    {
        why = "";
        var found = new List<PetCandidate>();

        var pet = _cfg.Pet;
        if (!BagGrid.IsValidRect(pet.BagGrid)) { why = "the bag grid isn't calibrated"; return found; }

        var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
        if (hwnd == IntPtr.Zero || WindowFinder.IsMinimized(hwnd))
        { why = "the game window isn't open"; return found; }

        // A capture reads the SCREEN, so anything in front of the game is what gets matched. The
        // player found this the hard way: their game window was not focused, so every page was
        // captured as whatever was over it, both crops scored 0.915 against it, and the tool reported
        // "no queued pet is in the bag" — the RIGHT verdict from the wrong image, which is
        // indistinguishable from a genuinely missing pet and was chased as one. The composer refuses
        // to judge its empty box for exactly this reason, and the feeder counts already refuse here.
        if (WindowFinder.ForegroundWindow() != hwnd)
        {
            why = "the game isn't the front window, so the capture would be of whatever is";
            return found;
        }

        // The best and the next best, kept together so the log can show how close the call was.
        var best = double.MaxValue;
        var runnerUp = double.MaxValue;

        for (int p = 0; p < pet.PageTabs.Count; p++)
        {
            if (!IsPoint(pet.PageTabs[p])) continue;
            if (!SelectPage(ser, p, out why)) return found;
            SleepCheck(PageWait);

            // Off the bag, so the pointer is not sitting on a pet's portrait when the page is read.
            ParkCursor(ser);
            SleepCheck(ClickWait);

            var cap = ScreenCapture.CaptureClient(hwnd);
            if (cap == null) { why = "the bag couldn't be captured"; return found; }

            // Saved, for the same reason the Pet tab's scan saves its own: a scan that finds nothing
            // is indistinguishable from a scan that looked at the wrong thing, and the image is the
            // only evidence that tells them apart.
            try
            {
                var dir = System.IO.Path.Combine(AppContext.BaseDirectory, "logs", "reads");
                System.IO.Directory.CreateDirectory(dir);
                cap.Image.ImWrite(System.IO.Path.Combine(dir,
                    $"run_page{p + 1}_{DateTime.Now:yyyyMMdd_HHmmss}.png"));
            }
            catch
            {
                // Evidence is a convenience; never fail a scan over it.
            }

            using var bag = cap.Image;
            for (int i = 0; i < pet.Queue.Count; i++)
            {
                var entry = pet.Queue[i];
                using var icon = IconMatch.FromBase64(entry.Png);
                if (icon == null) continue;

                // ScoreAll is sorted best-first, so the first is this icon's best cell — and the second
                // is the cell it nearly tied with, which is the number worth knowing.
                var scores = IconMatch.ScoreAll(bag, pet.BagGrid!, icon);
                if (scores.Count == 0) continue;

                // EVERY under-limit cell becomes a candidate, not just the best one for this icon. One
                // icon stands for a KIND of pet and a bag holds several of each — the player's holds
                // eight across two icons — so the best-matching instance is quite often one that has
                // FINISHED while its siblings still want feeding. Keeping only scores[0] left the guard
                // nothing to fall through to: both winners read +9/100%, both were skipped, and the row
                // went unboarded with four feedable pets one cell away.
                foreach (var (cell, score) in scores)
                {
                    if (score > MatchLimit) break;   // sorted best first, so nothing later can qualify
                    if (found.Any(c => c.Page == p && c.Cell == cell)) continue;
                    found.Add(new PetCandidate(i, p, cell, score, entry.Label ?? "(unlabelled)"));
                }

                if (scores[0].Score < best)
                {
                    runnerUp = Math.Min(best, scores.Count > 1 ? scores[1].Score : double.MaxValue);
                    best = scores[0].Score;
                }
            }
        }

        // BEST FIRST. The tool boards the best match it has and only falls through to a worse one when
        // the better one turns out to be unboardable, so the order is the whole point of the list.
        found.Sort((a, b) => a.Score.CompareTo(b.Score));

        if (found.Count > 0)
        {
            var top = found[0];
            Log($"  icon scan: best {top.Score:0.###} at page {top.Page + 1} cell {top.Cell} " +
                $"({top.Label}), runner-up " +
                $"{(runnerUp == double.MaxValue ? "n/a" : runnerUp.ToString("0.###", CultureInfo.InvariantCulture))}" +
                (found.Count > 1 ? $", {found.Count} candidate(s)" : ""));
            return found;
        }

        Log($"  icon scan found nothing within {MatchLimit:0.###}: best was " +
            $"{(best == double.MaxValue ? "no scores" : best.ToString("0.###", CultureInfo.InvariantCulture))} across " +
            $"{pet.Queue.Count} icon(s) and {pet.PageTabs.Count} page(s)");
        why = $"the closest match was {(best == double.MaxValue ? "nothing" : best.ToString("0.###", CultureInfo.InvariantCulture))}, " +
              $"and anything above {MatchLimit:0.###} is too unlike the pet to click";
        return found;
    }

    /// <summary>Whether this row's pet slot in the boarding window still looks empty.
    ///
    /// Null when it cannot be told — no reference captured, or the region unreadable — which callers
    /// treat as "carry on" rather than "failed": a missing check must not stop a run that would
    /// otherwise work.</summary>
    private bool? PetSlotIsEmpty(PetSlotConfig row) => PetSlotDifference(row) is { } d
        ? d <= _cfg.Pet.PetSlotOccupiedAbove
        : null;

    /// <summary>How unlike the row's empty reference its slot looks right now, or null when it cannot
    /// be told — no reference captured, no window, an unreadable grab.
    ///
    /// Split out from the yes/no so the NUMBER can be logged. A slot that reads "occupied" by a hair
    /// is a reference that no longer matches its box — a re-drawn box leaves the crop stale, and the
    /// comparison can only resize, not re-aim — and that failure is invisible in a boolean: it looks
    /// exactly like a pet being there. It cost a live run an empty boarding started on a row with no
    /// pet, which the game answered with an error box over everything.
    ///
    /// The threshold it is compared against is deliberately LOW, because the cost of the strict
    /// direction is a retry and the cost of the loose direction is not noticing a pet that never went
    /// in. PetSlotOccupiedAbove carries that reasoning.</summary>
    private double? PetSlotDifference(PetSlotConfig row)
    {
        if (!BagGrid.IsValidRect(row.BoardingPetSlot)) return null;

        using var reference = IconMatch.FromBase64(row.PetSlotEmptyPng);
        if (reference == null) return null;

        var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
        if (hwnd == IntPtr.Zero || WindowFinder.IsMinimized(hwnd)) return null;

        var box = row.BoardingPetSlot!;
        var cap = ScreenCapture.CaptureClientRegion(hwnd,
            new RegionConfig { Left = box[0], Top = box[1], Width = box[2], Height = box[3] });
        if (cap == null) return null;

        using var now = cap.Image;
        return IconMatch.DifferingFraction(reference, now);
    }

    /// <summary>One transaction per stack, filling as many of the row's slots as it holds — see
    /// <see cref="PetSlotConfig.Stacks"/> for why that is two on the free row and five on a paid one.
    ///
    /// The cell to use rotates through the marked set rather than always taking cell 0: the first
    /// stack empties a cell, so a fixed index would right-click an empty slot on every pass after the
    /// first. A reload consumes one cell per stack, so the marked set wants to be at least that large
    /// per row — and the "no food cells left" failure comes correspondingly sooner if it is not.</summary>
    private bool LoadFood(SerialPort ser, PetSlotConfig row, out string error)
    {
        error = "";
        var pet = _cfg.Pet;

        var stacks = Math.Max(1, row.Stacks);
        var drag = FoodLoadMode.IsDrag(pet.FoodLoadMode);
        for (int stack = 0; stack < stacks; stack++)
        {
            var cell = NextFoodCell(pet);
            if (cell == null)
            {
                error = $"No food cells left — {pet.FoodSlotsUsed} of {pet.FoodSlots.Count} used. " +
                        "Mark the cells holding food again on the Pet tab.";
                return false;
            }

            var (page, index) = cell.Value;
            if (!SelectPage(ser, page, out error)) return false;

            var centres = BagGrid.Centres(pet.BagGrid!);
            if (index >= centres.Count)
            {
                error = $"A marked food cell ({index}) is outside the bag grid.";
                return false;
            }
            var (cx, cy) = centres[index];
            var bagCell = new List<int> { cx, cy };

            List<int>? slot = null;
            if (drag)
            {
                // The row's OWN box for this stack, named. See FoodLoadMode for why a right-click
                // cannot be used to do this.
                if (FeederSlotCentre(row, stack) is not { } box)
                {
                    error = $"{NameOf(row)}: food load is set to drag but this row's food strip does " +
                            $"not divide into {row.Stacks} slot(s) — draw the strip on Calibrate Pet, " +
                            "or set the Pet tab's food load back to right-click.";
                    return false;
                }
                slot = box;
            }

            // A DRAG IS CHECKED AND RE-DRAGGED; a right-click is not. The game chooses the box for a
            // right-click, so there is no cell it can be said to have left, and no way to look. The drag
            // names a cell, so the cell can be looked at.
            var attempts = drag ? FoodDragAttempts : 1;
            var moved = false;

            for (var attempt = 1; attempt <= attempts && !moved; attempt++)
            {
                using var before = drag ? CaptureBagCell(bagCell) : null;

                if (drag)
                {
                    if (!DragFood(ser, bagCell, slot!, index, page, stack, attempt, out error)) return false;
                }
                else if (!Click(ser, bagCell, right: true, $"FOOD cell {index} (page {page + 1})", out error))
                {
                    return false;
                }

                // MEASUREMENT, no decision taken on it. The decision below is made after the count is
                // confirmed, because that is certainly when the stack is gone — but if the cell already
                // reads empty at the drop, then a missed drag could be caught BEFORE the MAX click and
                // the Enter, which are input this tool currently fires at a dialog a missed drag never
                // raised. One live run's numbers answer which of the two it is; nothing needs guessing.
                if (drag && before != null)
                {
                    using var mid = CaptureBagCell(bagCell);
                    if (mid != null)
                        Log($"  the bag cell is {IconMatch.DifferingFraction(before, mid):0.###} different " +
                            "at the drop, before the count dialog");
                }

                SleepCheck(ActionWait);
                if (!Click(ser, EffectiveMax()!, right: false, "MAX", out error)) return false;
                SleepCheck(ActionWait);

                // ONE Enter, not two. The sell flow's second Enter dismisses a confirmation this dialog
                // does not have, so sending it would press an Enter into whatever follows.
                if (!Enter(ser, out error)) return false;
                // Logged because it is the one action with nothing to see: a keypress has no cursor
                // movement and no visible effect of its own, so a reload that skipped it would read
                // exactly like one that sent it.
                Log("  enter (confirms the count dialog — no second one: this dialog has no confirmation)");
                SleepCheck(ActionWait);

                if (!drag) { moved = true; break; }

                var emptied = BagCellEmptied(bagCell, before);
                if (emptied == null)
                {
                    // UNKNOWN IS NOT FAILURE. Re-dragging a cell that is already empty picks up nothing
                    // and then fires a MAX click and an Enter at a dialog that is not there — worse than
                    // the missed drag it would be trying to fix.
                    Log("  the bag cell couldn't be compared — carrying on without a re-drag");
                    moved = true;
                    break;
                }

                moved = emptied.Value;
                if (!moved && attempt < attempts)
                    Log($"  the stack is STILL in the bag — re-dragging ({attempt + 1} of {attempts})");
            }

            if (!moved)
            {
                error = $"{NameOf(row)}: stack {stack + 1} would not leave the bag after {attempts} " +
                        "drag(s) — the cell still holds it, so nothing was loaded for that stack.";
                return false;
            }
        }

        return true;
    }

    /// <summary>The centre of THIS ROW's food box for <paramref name="stack"/>, or null when the row
    /// has no strip to divide.
    ///
    /// The box comes from the row's strip divided by its stack count — one drag instead of one box per
    /// slot. Its centre is what a drop needs. Null rather than a guess: a drag that releases over
    /// whatever happens to be there is the one failure here that can lose an item.</summary>
    private static List<int>? FeederSlotCentre(PetSlotConfig row, int stack)
    {
        if (FeederLayout.SlotBoxes(row.FeederStrip, row.Stacks) is not { } slots) return null;
        if (stack >= slots.Count) return null;

        var box = slots[stack];
        return new List<int> { box[0] + box[2] / 2, box[1] + box[3] / 2 };
    }

    /// <summary>Drags one bag stack onto one named food box: place on the bag cell, press, move to the
    /// box, release. The caller then MAXes and Enters as it always did — the drag replaces only the
    /// gesture that chose the destination, not the count dialog behind it.
    ///
    /// The release is in a `finally` and that is not tidiness. A left button left down follows the
    /// player's REAL cursor and drops the stack on whatever it is next over — and a failure part way
    /// through the move is exactly when that would happen, so the failure path is the one that must
    /// release, not the one that must not.</summary>
    private bool DragFood(SerialPort ser, List<int> bagCell, List<int> slot, int index, int page,
        int stack, int attempt, out string error)
    {
        if (!PlaceOn(ser, bagCell[0], bagCell[1], out error))
        {
            Log($"  FAILED moving to FOOD cell {index} (page {page + 1}) at " +
                $"({bagCell[0]},{bagCell[1]}): {error}");
            return false;
        }
        Log($"  drag[{attempt}] onto FOOD cell {index} (page {page + 1}) at ({bagCell[0]},{bagCell[1]})" +
            $" — landed {CursorWhere()}");
        SleepCheck(ClickWait);

        // Re-aim before pressing, for the same reason Click does: the wait exists so the game is ready,
        // and something else moves the cursor during it. A press meant for the bag cell but delivered
        // beside it picks up nothing and drags nothing.
        if (!PlaceOn(ser, bagCell[0], bagCell[1], out error))
        {
            Log($"  FAILED re-aiming at FOOD cell {index} before dragging: {error}");
            return false;
        }

        HidPointer.LeftDown(ser);
        Log($"  drag[{attempt}] leftdown at {CursorWhere()}");

        // HOLD before pulling. Not a nicety: this is what the player's eye caught — the first drag
        // pressed and moved in the same frame and picked nothing up. See DragGrabWait.
        SleepCheck(DragGrabWait);
        try
        {
            if (!PlaceOn(ser, slot[0], slot[1], out error))
            {
                Log($"  FAILED dragging stack {stack + 1} to its box at ({slot[0]},{slot[1]}): {error}");
                return false;
            }
            SleepCheck(ClickWait);

            // "leftup", not "dropped" and not "stack N onto": those were claims about the game made at
            // the moment the frames were sent, and a drag that picked up nothing logged exactly the same
            // lines as one that worked. What is logged here is what this code DID; whether the stack
            // moved is answered by the caller, which looks at the bag cell.
            Log($"  drag[{attempt}] leftup at {CursorWhere()} — sent stack {stack + 1} toward the row's " +
                $"food box {stack + 1} at ({slot[0]},{slot[1]})");
            return true;
        }
        finally
        {
            HidPointer.LeftUp(ser);
        }
    }

    /// <summary>Where the cursor actually is, for the log — the position the placement aims in, so the
    /// goal and the landing can be compared.
    ///
    /// The step logs used to record only where the cursor was SENT. A move that landed somewhere else
    /// read identically to one that hit, which is the whole reason a failing run is hard to diagnose:
    /// the numbers on the line were the intent, not the outcome.</summary>
    private static string CursorWhere()
    {
        var p = WindowFinder.LogicalCursorPosition();
        return p is { } at ? $"({at.X},{at.Y})" : "(cursor position unreadable)";
    }

    /// <summary>One bag cell's pixels, centred on <paramref name="centre"/> at the grid's pitch — the
    /// same crop the icon capture uses. Null when the window or the grab is unavailable, which the
    /// caller reads as "don't know" rather than as "it moved" or "it didn't".</summary>
    private Mat? CaptureBagCell(List<int> centre)
    {
        var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
        if (hwnd == IntPtr.Zero || WindowFinder.IsMinimized(hwnd)) return null;
        if (!BagGrid.IsValidRect(_cfg.Pet.BagGrid)) return null;

        var w = (int)Math.Round(BagGrid.PitchX(_cfg.Pet.BagGrid!));
        var h = (int)Math.Round(BagGrid.PitchY(_cfg.Pet.BagGrid!));

        var cap = ScreenCapture.CaptureClientRegion(hwnd, new RegionConfig
        {
            Left = centre[0] - w / 2,
            Top = centre[1] - h / 2,
            Width = w,
            Height = h,
        });
        return cap?.Image;
    }

    /// <summary>Whether the stack left the bag cell, or null when it cannot be told.
    ///
    /// THE LOOK the food load never had. Every other risky action in this tool checks its own effect —
    /// the pet placement looks at the slot, the toggle checks it did not do the opposite, the feeder
    /// counts refuse to judge a capture that is not the game. The drag sent its frames and asserted
    /// success, so a missed drag was invisible: the run went on to click MAX and press Enter at a count
    /// dialog that was never raised, and the food simply was not there.</summary>
    private bool? BagCellEmptied(List<int> centre, Mat? before)
    {
        if (before == null) return null;
        using var after = CaptureBagCell(centre);
        if (after == null) return null;

        var changed = IconMatch.DifferingFraction(before, after);
        Log($"  the bag cell is {changed:0.###} different from before the drag " +
            $"(moved above {BagCellChangedAbove:0.###})");
        return changed > BagCellChangedAbove;
    }

    /// <summary>The next marked food cell, and the first one NOT yet used.
    ///
    /// Starts from the persisted count rather than from zero, so a restart continues where the last
    /// run stopped instead of re-clicking cells it already emptied. Returns null once the marked cells
    /// are exhausted, which stops the reload rather than guessing — the honest failure, since the
    /// alternative is clicking an empty slot and reporting success.</summary>
    private (int Page, int Cell)? NextFoodCell(PetConfig cfg)
    {
        var cells = cfg.FoodSlots;
        if (cells.Count == 0) return null;
        if (cfg.FoodSlotsUsed < 0 || cfg.FoodSlotsUsed >= cells.Count) return null;

        var cell = cells[cfg.FoodSlotsUsed];
        if (cell is not { Count: 2 }) return null;

        cfg.FoodSlotsUsed++;
        _persistState?.Invoke();
        Log($"  food cell {cfg.FoodSlotsUsed}/{cells.Count} used (page {cell[0] + 1}, cell {cell[1]})");
        return (cell[0], cell[1]);
    }

    /// <summary>The count dialog's Enter and the toggling of boarding both want a beat after the
    /// window has changed state, and a boarding window that has just opened or closed is animating.
    /// </summary>
    /// <summary>Raised from 1.2s after a live true-reload where every cursor placement was visibly
    /// correct and the pet still did not board: the likely cause is that the pet has not reappeared
    /// in the bag yet when its cell is right-clicked, so the click lands on an empty slot and looks
    /// like a miss. The window is a guess until the real figure is known — see the log line, which
    /// timestamps the end press against the placement so the gap can be read off a run.</summary>
    private const double EndWait = 2.5;

    /// <summary>Presses the 開始代養 / 結束代養 button — the same press serves both, which is exactly
    /// why the caller has to know which one it wants. The label box's centre is the click: the label
    /// sits on the button.</summary>
    private bool PressToggle(SerialPort ser, PetSlotConfig row, out string error)
    {
        var label = row.ToggleLabel!;
        var centre = new List<int> { label[0] + label[2] / 2, label[1] + label[3] / 2 };
        return Click(ser, centre, right: false, "the start/end boarding button", out error);
    }

    private bool StartBoarding(SerialPort ser, PetSlotConfig row, out string error)
    {
        // The toggle's LABEL box is what is calibrated, so its centre is the click. The label sits on
        // the button, which is why one box serves as both the read and the press.
        var label = row.ToggleLabel!;
        var centre = new List<int> { label[0] + label[2] / 2, label[1] + label[3] / 2 };
        return Click(ser, centre, right: false, "the start button", out error);
    }

    private bool SelectPage(SerialPort ser, int page, out string error)
    {
        error = "";
        var tabs = _cfg.Pet.PageTabs;
        if (page < 0 || page >= tabs.Count || !IsPoint(tabs[page]))
        {
            error = $"Bag page {page + 1} isn't calibrated.";
            return false;
        }

        // ALREADY THERE — the click is skipped, not repeated. The food loop asks for the same page once
        // per stack and all 22 marked cells are on one page, so this is the common case rather than an
        // optimisation for an edge. See _bagPage for what bounds the assumption.
        if (_bagPage == page)
        {
            Log($"  bag page {page + 1} is already showing — no tab click");
            return true;
        }

        // Absolute tabs, not next/previous: clicking ITEM2 lands on page 2 whatever page we were on,
        // so there is no relative position to lose track of and nothing to read back.
        if (!Click(ser, tabs[page], right: false, $"the ITEM{page + 1} tab", out error)) return false;
        SleepCheck(Math.Max(PageWait, ActionWait));
        _bagPage = page;
        return true;
    }

    // ── Plumbing ────────────────────────────────────────────────────────────

    /// <summary>Place, then click. Both halves are logged against <paramref name="what"/>, because
    /// "the cursor went to the pet and nothing happened" is ambiguous otherwise: the move can have
    /// succeeded with the click never sent, or been sent at the wrong moment. The log now says which.
    /// </summary>
    private bool Click(SerialPort ser, List<int> point, bool right, string what, out string error)
    {
        if (!PlaceOn(ser, point[0], point[1], out error))
        {
            Log($"  FAILED moving to {what} at ({point[0]},{point[1]}): {error}");
            return false;
        }

        SleepCheck(ClickWait);

        // Re-aim before pressing. The wait exists so the game is ready for the click, but anything
        // that moves the cursor during it moves what the click lands on — and something does: the
        // live trace shows the cursor shifting hundreds of pixels on its own, enough to turn a click
        // meant for the pet into a click on empty bag. Placing again is usually one extra move and it
        // is the difference between aiming and having aimed.
        if (!PlaceOn(ser, point[0], point[1], out error))
        {
            Log($"  FAILED re-aiming at {what} before clicking: {error}");
            return false;
        }

        if (right) HidPointer.RightClick(ser);
        else HidPointer.Click(ser);

        Log($"  {(right ? "right-click" : "click")} {what} at ({point[0]},{point[1]})");
        return true;
    }

    private static bool Enter(SerialPort ser, out string error)
    {
        error = "";
        try
        {
            ser.Write("E\n");
            return true;
        }
        catch (Exception ex)
        {
            error = "Lost the Arduino: " + ex.Message;
            return false;
        }
    }

    private bool PlaceOn(SerialPort ser, int x, int y, out string error)
    {
        error = "";
        var display = HidPointer.Display(_cfg.Window.Title);
        if (display == null)
        {
            error = "Game window not found (or minimized) — open and restore the game first.";
            return false;
        }
        var placed = HidPointer.To(ser, WindowFinder.ComputeCursorTarget(display, x, y));
        if (!placed.Ok)
        {
            error = placed.Error ?? $"Couldn't place the cursor at ({x},{y}).";
            return false;
        }
        return true;
    }

    private static bool IsPoint(List<int>? p) => p is { Count: 2 };

    /// <summary>Sleep in one-second slices so the quit hotkey is honoured within a second, even
    /// across a wait measured in hours.</summary>
    private bool SleepMinutes(double minutes, CancellationToken ct)
        => SleepUntil(DateTime.Now.AddMinutes(minutes), ct);

    private bool SleepUntil(DateTime when, CancellationToken ct)
    {
        while (DateTime.Now < when)
        {
            if (QuitPressed || ct.IsCancellationRequested) return false;
            SleepCheck(1.0);
        }
        return true;
    }
}

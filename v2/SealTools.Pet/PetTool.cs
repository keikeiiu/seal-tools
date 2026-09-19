using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO.Ports;
using System.Threading;
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

    /// <summary>How many times to re-click the pet before giving up. More than one because the failure
    /// is intermittent rather than a wrong calibration — the cursor is verified on target — so a retry
    /// is the fix and the count only bounds it.</summary>
    private const int PlaceAttempts = 3;

    /// <summary>After a click, before looking at the slot. The game needs a moment to move the pet.</summary>
    private const double PlaceSettle = 0.9;

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
    private readonly string _rootDir;

    public PetTool(AppConfig cfg, AttributesConfig attrs, string rootDir, Action? persistState = null)
        : base(cfg.Hotkeys)
    {
        _cfg = cfg;
        _attrs = attrs;
        _rootDir = rootDir;
        _persistState = persistState;
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

        Log($"run started — {_cfg.Pet.Slots.Count} row(s) at {_cfg.Pet.ItemsPerMinute}/min, plus " +
            $"{WaitAfterEmptyMinutes:0} min after empty: " +
            string.Join(", ", _cfg.Pet.Slots.Select(r =>
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
        var next = InspectRows(ser, state);
        var failures = _cfg.Pet.Slots.ToDictionary(r => r, _ => 0);

        try
        {
            while (!QuitPressed && !ct.IsCancellationRequested)
            {
                var live = next.Keys.ToList();
                if (live.Count == 0)
                {
                    state.Message = "Every row has failed repeatedly — stopped.";
                    break;
                }

                var row = live.OrderBy(r => next[r]).First();
                if (!SleepUntil(next[row], ct)) break;

                if (ReloadRow(ser, state, row, out var error))
                {
                    failures[row] = 0;
                    next[row] = DateTime.Now.AddMinutes(CycleMinutesFor(row));
                    state.Message = $"{NameOf(row)} reloaded. Next {next[row]:HH:mm}.";
                    Console.WriteLine(state.Message);
                    Beep(523, 100);
                }
                else
                {
                    failures[row]++;
                    state.Message = $"{NameOf(row)} failed {failures[row]}/{MaxFailures}: {error}";
                    Console.WriteLine(state.Message);
                    Beep(200, 400);

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
        }
        finally
        {
            state.Running = false;
        }

        Console.WriteLine("Pet tool done.");
        return 0;
    }

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

        // EVERY row, not the first. The tool drives them all, so a row with no button marked would
        // otherwise be discovered mid-run — as a click at (0,0) or at whatever the empty box's centre
        // works out to, during an unattended run.
        for (int i = 0; i < _cfg.Pet.Slots.Count; i++)
        {
            if (!BagGrid.IsValidRect(_cfg.Pet.Slots[i].ToggleLabel))
                return $"Row {i + 1}'s start/end button isn't marked — Calibrate Pet. The tool has " +
                       "nothing to click for that row, and it drives every configured row.";
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
        var schedule = new Dictionary<PetSlotConfig, DateTime>();
        state.Message = "Checking what is already running…";
        Log("start: opening the breeder to check each row (no reload)");

        if (!OpenBoarding(ser, out var error))
        {
            Log("  couldn't open the breeder to check: " + error + " — the ticked state is used");
            foreach (var row in _cfg.Pet.Slots)
                schedule[row] = ScheduleFor(row, null);
            return schedule;
        }

        // The engine is built here and disposed with the look, rather than held for the life of the
        // run: the counts are wanted at start, and an ONNX session parked in memory for days to be
        // used once is a cost with nothing buying it.
        OcrEngine? ocr = null;
        try
        {
            foreach (var row in _cfg.Pet.Slots)
            {
                var empty = PetSlotIsEmpty(row);
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

                // Only worth reading a feeder on a row that is actually boarding — an empty one has
                // nothing loaded and will be filled in a moment anyway.
                if (!row.BoardingRunning) { schedule[row] = ScheduleFor(row, null); continue; }

                ocr ??= new OcrEngine(_cfg, _attrs, _rootDir);
                var left = ReadFeederCounts(ocr, row);
                schedule[row] = ScheduleFor(row, left);

                if (left is { } items)
                    Log($"  {NameOf(row)}: {items} item(s) left in the feeder — reloading in " +
                        $"{Math.Max(0, (schedule[row] - DateTime.Now).TotalMinutes):0} min");
                else
                    Log($"  {NameOf(row)}: the feeder counts couldn't be read — assuming a full " +
                        $"load, so reloading in {CycleMinutesFor(row):0} min");
            }
            _persistState?.Invoke();
        }
        finally
        {
            ocr?.Dispose();
            CloseBoarding(ser);
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
    /// Null means either "not boarding" (reload now — it needs a pet and a fill) or "couldn't read"
    /// (fall back to the configured cycle, which assumes a full load). The two are different and the
    /// caller logs which.</summary>
    private DateTime ScheduleFor(PetSlotConfig row, int? itemsLeft)
    {
        if (_cfg.Pet.ReloadOnStart) return DateTime.Now;
        if (!row.BoardingRunning) return DateTime.Now;
        if (itemsLeft is not { } items) return DateTime.Now.AddMinutes(CycleMinutesFor(row));

        var rate = Math.Max(1, _cfg.Pet.ItemsPerMinute);
        return DateTime.Now.AddMinutes(Math.Max(0, items / (double)rate) + WaitAfterEmptyMinutes);
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
        if (row.FeederSlots.Count == 0) return null;

        var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
        if (hwnd == IntPtr.Zero || WindowFinder.IsMinimized(hwnd)) return null;

        // The read is a SCREEN GRAB, so anything in front of the game is what gets measured. The
        // composer refuses to judge its empty box for the same reason — measuring the launcher's own
        // UI once produced a confident answer about a window that had nothing to do with the game.
        if (WindowFinder.ForegroundWindow() != hwnd) return null;

        var total = 0;
        var counted = 0;

        foreach (var box in row.FeederSlots)
        {
            if (!BagGrid.IsValidRect(box)) continue;

            var region = new RegionConfig { Left = box[0], Top = box[1], Width = box[2], Height = box[3] };
            var digits = new string(string.Join("", ocr.ReadLines(region, 3)).Where(char.IsDigit).ToArray());
            if (digits.Length == 0) continue;

            if (int.TryParse(digits, CultureInfo.InvariantCulture, out var n))
            {
                total += n;
                counted++;
            }
        }

        return counted == 0 ? null : total;
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

    // ── One reload ──────────────────────────────────────────────────────────

    /// <summary>One reload of ONE ROW — the sequence is unchanged from the single-row tool; what is
    /// new is that the row is a parameter rather than whatever the config happened to name. The rows
    /// are serviced one at a time, which is the player's own ordering constraint (2026-09-19): each
    /// row is offloaded and re-boarded before the next is touched.</summary>
    private bool ReloadRow(SerialPort ser, ToolState state, PetSlotConfig row, out string error)
    {
        error = "";

        // Each step names itself on the card as it runs. Without this a failure reads as "it opened
        // the window and then closed it again", because the close below is the cleanup and it is the
        // only thing the eye catches.
        state.Message = $"{NameOf(row)}: opening the boarding window…";
        Log($"reload {NameOf(row)}: opening the boarding window");
        if (!OpenBoarding(ser, out error)) { Log("  FAILED opening: " + error); return false; }

        // Every exit past this point closes the window: leaving it open would sit on top of the game
        // while the tool waits out its next cycle, and the next cycle would click 目錄 behind it.
        try
        {
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
            }

            state.Message = $"{NameOf(row)}: placing the pet…";
            Log("  placing the pet");
            if (!PlacePet(ser, row, out error)) { Log("  FAILED placing the pet: " + error); return false; }

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
        finally
        {
            CloseBoarding(ser);
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
        return true;
    }

    private void CloseBoarding(SerialPort ser)
    {
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
    private bool PlacePet(SerialPort ser, PetSlotConfig row, out string error)
    {
        error = "";
        var pet = _cfg.Pet;

        int page, cell;

        if (pet.Queue.Count > 0)
        {
            // THE QUEUE IS THE ANSWER when it exists, and there is deliberately NO fallback to the
            // return slot if it finds nothing. A pet that is not in the bag either finished and was
            // mailed or was never there — and right-clicking a marked cell on the assumption that it
            // holds the pet is exactly the guess the icon matching exists to replace. Failing here
            // says so on the card and makes the reload stop, which is §13's honest end state.
            if (!FindQueuedPet(ser, out page, out cell, out var score, out var why))
            {
                error = $"No queued pet is in the bag ({why}). Right-clicked nothing. If the pet " +
                        "finished it was mailed, so this is the queue being empty of live pets — " +
                        "capture the next one's icon on the Pet tab.";
                return false;
            }
            Log($"  found a queued pet: page {page + 1}, cell {cell}, match {score:0.###}");
        }
        else if (pet.ReturnSlot is { Count: 2 } back)
        {
            page = back[0];
            cell = back[1];
            Log($"  no queue captured — placing from the return slot, page {page + 1}, cell {cell}");
        }
        else
        {
            error = "Neither a queued pet icon nor a return slot is set — there is nothing to " +
                    "right-click. Mark the return slot or capture a pet icon, on the Pet tab.";
            return false;
        }

        // After a scan the bag is left on whichever page was swept last, so the page the cell lives on
        // has to be brought back up before anything is clicked on it.
        if (!SelectPage(ser, page, out error)) return false;

        var centres = BagGrid.Centres(pet.BagGrid!);
        if (cell < 0 || cell >= centres.Count)
        {
            error = $"The return slot ({cell}) is outside the bag grid.";
            return false;
        }

        var (cx, cy) = centres[cell];

        // Click, then LOOK. A right-click can fail to register — a live 12-hour run lost roughly half
        // its boarded time to reloads that loaded food into an empty slot and reported success — and
        // the placement is verified to within 2px, so the cursor was on target when the click went out.
        // An intermittent action cannot be made reliable by aiming better; it can only be checked and
        // repeated.
        for (var attempt = 1; attempt <= PlaceAttempts; attempt++)
        {
            if (!Click(ser, new List<int> { cx, cy }, right: true,
                    $"the PET at cell {cell} (page {page + 1})", out error))
                return false;

            SleepCheck(Math.Max(PlaceSettle, ActionWait));

            switch (PetSlotIsEmpty(row))
            {
                case false:
                    if (attempt > 1) Log($"  the pet went in on attempt {attempt}");
                    return true;

                case null:
                    // No reference, or the region couldn't be read. Unknown is NOT failure — refusing
                    // to run because a safety net is absent would be worse than the thing it guards.
                    Log("  pet slot not checked (no empty-slot reference, or it couldn't be read)");
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
    private bool FindQueuedPet(SerialPort ser, out int page, out int cell, out double score, out string why)
    {
        page = 0;
        cell = -1;
        score = 1;
        why = "";

        var pet = _cfg.Pet;
        if (!BagGrid.IsValidRect(pet.BagGrid)) { why = "the bag grid isn't calibrated"; return false; }

        var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
        if (hwnd == IntPtr.Zero || WindowFinder.IsMinimized(hwnd))
        { why = "the game window isn't open"; return false; }

        // The best and the next best, kept together so the log can show how close the call was.
        var best = double.MaxValue;
        var runnerUp = double.MaxValue;

        for (int p = 0; p < pet.PageTabs.Count; p++)
        {
            if (!IsPoint(pet.PageTabs[p])) continue;
            if (!SelectPage(ser, p, out why)) return false;
            SleepCheck(PageWait);

            // Off the bag, so the pointer is not sitting on a pet's portrait when the page is read.
            ParkCursor(ser);
            SleepCheck(ClickWait);

            var cap = ScreenCapture.CaptureClient(hwnd);
            if (cap == null) { why = "the bag couldn't be captured"; return false; }

            using var bag = cap.Image;
            foreach (var entry in pet.Queue)
            {
                using var icon = IconMatch.FromBase64(entry.Png);
                if (icon == null) continue;

                // ScoreAll is sorted best-first, so the first is this icon's best cell — and the second
                // is the cell it nearly tied with, which is the number worth knowing.
                var scores = IconMatch.ScoreAll(bag, pet.BagGrid!, icon);
                if (scores.Count == 0) continue;

                if (scores[0].Score < best)
                {
                    runnerUp = Math.Min(best, scores.Count > 1 ? scores[1].Score : double.MaxValue);
                    best = scores[0].Score;
                    page = p;
                    cell = scores[0].Cell;
                    why = entry.Label ?? "(unlabelled)";
                }
            }
        }

        score = best;
        if (best <= MatchLimit)
        {
            Log($"  icon scan: best {best:0.###} at page {page + 1} cell {cell} ({why}), runner-up " +
                $"{(runnerUp == double.MaxValue ? "n/a" : runnerUp.ToString("0.###", CultureInfo.InvariantCulture))}");
            return true;
        }

        Log($"  icon scan found nothing within {MatchLimit:0.###}: best was " +
            $"{(best == double.MaxValue ? "no scores" : best.ToString("0.###", CultureInfo.InvariantCulture))} across " +
            $"{pet.Queue.Count} icon(s) and {pet.PageTabs.Count} page(s)");
        why = $"the closest match was {(best == double.MaxValue ? "nothing" : best.ToString("0.###", CultureInfo.InvariantCulture))}, " +
              $"and anything above {MatchLimit:0.###} is too unlike the pet to click";
        return false;
    }

    /// <summary>Whether this row's pet slot in the boarding window still looks empty.
    ///
    /// Null when it cannot be told — no reference captured, or the region unreadable — which callers
    /// treat as "carry on" rather than "failed": a missing check must not stop a run that would
    /// otherwise work.</summary>
    private bool? PetSlotIsEmpty(PetSlotConfig row)
    {
        var pet = _cfg.Pet;
        if (!BagGrid.IsValidRect(row.BoardingPetSlot)) return null;

        using var reference = IconMatch.FromBase64(pet.PetSlotEmptyPng);
        if (reference == null) return null;

        var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
        if (hwnd == IntPtr.Zero || WindowFinder.IsMinimized(hwnd)) return null;

        var box = row.BoardingPetSlot!;
        var cap = ScreenCapture.CaptureClientRegion(hwnd,
            new RegionConfig { Left = box[0], Top = box[1], Width = box[2], Height = box[3] });
        if (cap == null) return null;

        using var now = cap.Image;
        var difference = IconMatch.DifferingFraction(reference, now);
        return difference <= pet.PetSlotOccupiedAbove;
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
            if (!Click(ser, new List<int> { cx, cy }, right: true, $"FOOD cell {index} (page {page + 1})", out error)) return false;

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
        }

        return true;
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

        // Absolute tabs, not next/previous: clicking ITEM2 lands on page 2 whatever page we were on,
        // so there is no relative position to lose track of and nothing to read back.
        if (!Click(ser, tabs[page], right: false, $"the ITEM{page + 1} tab", out error)) return false;
        SleepCheck(Math.Max(PageWait, ActionWait));
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

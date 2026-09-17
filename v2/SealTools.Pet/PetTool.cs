using System;
using System.Collections.Generic;
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
//     end  →  find the pet  →  re-place it  →  load two stacks  →  start
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

    /// <summary>Reload this many minutes BEFORE the feeder is due to empty. Early is safe — the
    /// leftover returns to the bag — and late is the path to a pet going hungry, so the margin wants
    /// to be generous rather than tight.</summary>
    private const double SafetyMarginMinutes = 15;

    private const double RetryMinutes = 5;

    /// <summary>After a bag page tab is clicked, before anything is clicked inside the grid.
    ///
    /// Switching pages re-renders the grid, and a right-click delivered during that is dropped — with
    /// the cursor sitting on the right cell the whole time, so it reads as a miss rather than as
    /// timing. This is the same class of bug as the click delays that bit the shop on a second PC: a
    /// click that outruns an animation is indistinguishable from a click that was never sent.
    /// </summary>
    private const double PageWait = 0.9;

    /// <summary>How many reloads may fail in a row before the tool stops. Unattended retrying is how
    /// a misread becomes a loop of clicks, and this one runs while nobody is watching.</summary>
    private const int MaxFailures = 3;

    /// <summary>How unlike the pet a cell may look and still be clicked, as a fraction of pixels that
    /// differ. Deliberately strict: a near-miss right-clicks some other item, which has no undo. This
    /// wants measuring against a real farming bag rather than trusting — see the plan's open question.</summary>
    private const double MatchLimit = 0.12;

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

    public PetTool(AppConfig cfg, Action? persistState = null) : base(cfg.Hotkeys)
    {
        _cfg = cfg;
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

        Log($"run started — cycling every {CycleMinutes():0} min " +
            $"({_cfg.Pet.LoadItems} items at {_cfg.Pet.ItemsPerMinute}/min, minus " +
            $"{SafetyMarginMinutes:0} min margin)");

        var failures = 0;
        try
        {
            while (!QuitPressed && !ct.IsCancellationRequested)
            {
                if (ReloadOnce(ser, state, ct, out var error))
                {
                    failures = 0;
                    var next = DateTime.Now.AddMinutes(CycleMinutes());
                    state.Message = $"Reloaded. Next reload {next:HH:mm}.";
                    Console.WriteLine(state.Message);
                    Beep(523, 100);
                    if (!SleepUntil(next, ct)) break;
                }
                else
                {
                    failures++;
                    state.Message = $"Reload failed {failures}/{MaxFailures}: {error}";
                    Console.WriteLine(state.Message);
                    Beep(200, 400);
                    if (failures >= MaxFailures)
                    {
                        state.Message = "Stopped after repeated failures: " + error;
                        break;
                    }
                    if (!SleepMinutes(RetryMinutes, ct)) break;
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
        if (!BagGrid.IsValidRect(pet.ToggleLabel)) return "The boarding toggle label isn't calibrated — Calibrate Pet.";
        if (!IsPoint(pet.CloseButton)) return "The boarding window's X isn't calibrated — Calibrate Pet.";
        if (!BagGrid.IsValidRect(pet.BagGrid) || !BagGrid.IsValidRect(pet.BagSlot))
            return "The boarding bag's grid isn't calibrated — Calibrate Pet. (Its own, not the shop's.)";
        if (pet.PageTabs.Count == 0) return "The bag page tabs aren't calibrated — Calibrate Pet.";

        // Every marked cell carries a PAGE, so a tab that was never calibrated is a mark pointing at a
        // page the tool cannot reach. Checked here rather than discovered mid-flow: the reload closes
        // the window on any failure, so a bad page surfaced as "it opened and then shut again" with
        // nothing saying why.
        var pages = new List<(string What, int Page)>();
        if (pet.PetCell is { Count: 2 } pc) pages.Add(("The pet's bag cell", pc[0]));
        foreach (var cell in pet.FoodCells)
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
        if (pet.PetCell is not { Count: 2 })
            return "The pet's bag cell isn't marked — Calibrate Pet. Nothing says where to put the " +
                   "pet back.";
        if (pet.FoodCells.Count == 0)
            return "No food cells are marked — Calibrate Pet. Nothing says where the pet food lives.";
        if (EffectiveMax() == null)
            return "No MAX is calibrated — Calibrate Buy/Sell, or mark one on Calibrate Pet.";
        return null;
    }

    /// <summary>The count dialog's MAX: the pet override when set, otherwise the shared buy/sell one.
    /// They are the same dialog, so normally one mark serves both.</summary>
    private List<int>? EffectiveMax() => _cfg.Pet.MaxButton ?? _cfg.BuySell.MaxButton;

    /// <summary>How long one load lasts, minus the safety margin — i.e. when to reload next.
    /// Derived from the config rather than stored, so the rate and the load can never disagree with
    /// the interval they produce.</summary>
    private double CycleMinutes()
    {
        var full = _cfg.Pet.LoadMinutes;
        if (full <= 0) return 60;   // a broken rate must not spin the loop
        return Math.Max(5, full - SafetyMarginMinutes);
    }

    // ── One reload ──────────────────────────────────────────────────────────

    private bool ReloadOnce(SerialPort ser, ToolState state, CancellationToken ct, out string error)
    {
        error = "";
        _ = ct;

        // Each step names itself on the card as it runs. Without this a failure reads as "it opened
        // the window and then closed it again", because the close below is the cleanup and it is the
        // only thing the eye catches.
        state.Message = "Opening the boarding window…";
        Log("reload: opening the boarding window");
        if (!OpenBoarding(ser, out error)) { Log("  FAILED opening: " + error); return false; }

        // Every exit past this point closes the window: leaving it open would sit on top of the game
        // while the tool waits out its next cycle, and the next cycle would click 目錄 behind it.
        try
        {
            // The pet has to be IN THE BAG before it can be put back, and while boarding runs it is
            // in the loader instead. So a reload off a running boarding starts by ending it — which is
            // also what returns the leftover food. Skipped when boarding is already stopped, because
            // this button TOGGLES: pressing it then would start the very thing we are about to end.
            if (_cfg.Pet.BoardingRunning)
            {
                state.Message = "Ending boarding…";
                Log("  ending boarding (the pet is in the loader, so it has to come back first)");
                if (!PressToggle(ser, out error)) { Log("  FAILED ending: " + error); return false; }
                SleepCheck(EndWait);
            }

            state.Message = "Placing the pet…";
            Log("  placing the pet");
            if (!PlacePet(ser, out error)) { Log("  FAILED placing the pet: " + error); return false; }

            state.Message = "Loading the food…";
            Log("  loading the food");
            if (!LoadFood(ser, out error)) { Log("  FAILED loading the food: " + error); return false; }

            state.Message = "Starting boarding…";
            Log("  starting boarding");
            if (!StartBoarding(ser, out error)) { Log("  FAILED starting: " + error); return false; }

            // A finished reload always leaves boarding running, so the next one knows to end first —
            // recorded rather than assumed, because the tool has no way to read it back yet.
            _cfg.Pet.BoardingRunning = true;
            _persistState?.Invoke();

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

        SleepCheck(ClickWait);
        if (!Enter(ser, out error)) return false;
        Log("  enter (clears the out-of-food message if the breeder opened with one)");
        SleepCheck(DialogWait);
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
    private bool PlacePet(SerialPort ser, out string error)
    {
        error = "";
        var pet = _cfg.Pet;

        if (pet.PetCell is not { Count: 2 })
        {
            error = "The pet's bag cell isn't marked — Calibrate Pet.";
            return false;
        }

        var page = pet.PetCell[0];
        var cell = pet.PetCell[1];
        if (!SelectPage(ser, page, out error)) return false;

        var centres = BagGrid.Centres(pet.BagGrid!);
        if (cell < 0 || cell >= centres.Count)
        {
            error = $"The marked pet cell ({cell}) is outside the bag grid.";
            return false;
        }

        var (cx, cy) = centres[cell];
        Console.WriteLine($"[pet] placing from page {page + 1}, cell {cell}");
        return Click(ser, new List<int> { cx, cy }, right: true, $"the PET at cell {cell} (page {page + 1})", out error);
    }

    /// <summary>Two stacks, one transaction each. The cell to use rotates through the marked set
    /// rather than always taking cell 0 — the first stack empties a cell, so a fixed index would
    /// right-click an empty slot on the second pass.</summary>
    private bool LoadFood(SerialPort ser, out string error)
    {
        error = "";
        var pet = _cfg.Pet;

        for (int stack = 0; stack < 2; stack++)
        {
            var cell = NextFoodCell(pet);
            if (cell == null)
            {
                error = $"No food cells left — {pet.FoodCellsUsed} of {pet.FoodCells.Count} used. " +
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

            SleepCheck(DialogWait);
            if (!Click(ser, EffectiveMax()!, right: false, "MAX", out error)) return false;
            SleepCheck(DialogWait);

            // ONE Enter, not two. The sell flow's second Enter dismisses a confirmation this dialog
            // does not have, so sending it would press an Enter into whatever follows.
            if (!Enter(ser, out error)) return false;
            // Logged because it is the one action with nothing to see: a keypress has no cursor
            // movement and no visible effect of its own, so a reload that skipped it would read
            // exactly like one that sent it.
            Log("  enter (confirms the count dialog — no second one: this dialog has no confirmation)");
            SleepCheck(DialogWait);
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
        var cells = cfg.FoodCells;
        if (cells.Count == 0) return null;
        if (cfg.FoodCellsUsed < 0 || cfg.FoodCellsUsed >= cells.Count) return null;

        var cell = cells[cfg.FoodCellsUsed];
        if (cell is not { Count: 2 }) return null;

        cfg.FoodCellsUsed++;
        _persistState?.Invoke();
        Log($"  food cell {cfg.FoodCellsUsed}/{cells.Count} used (page {cell[0] + 1}, cell {cell[1]})");
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
    private bool PressToggle(SerialPort ser, out string error)
    {
        var label = _cfg.Pet.ToggleLabel!;
        var centre = new List<int> { label[0] + label[2] / 2, label[1] + label[3] / 2 };
        return Click(ser, centre, right: false, "the start/end boarding button", out error);
    }

    private bool StartBoarding(SerialPort ser, out string error)
    {
        // The toggle's LABEL box is what is calibrated, so its centre is the click. The label sits on
        // the button, which is why one box serves as both the read and the press.
        var label = _cfg.Pet.ToggleLabel!;
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
        SleepCheck(PageWait);
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

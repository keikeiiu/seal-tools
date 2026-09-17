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
    private int _nextFoodCell;

    public PetTool(AppConfig cfg) : base(cfg.Hotkeys) => _cfg = cfg;

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
            state.Message = "Placing the pet…";
            Log("  placing the pet");
            if (!PlacePet(ser, out error)) { Log("  FAILED placing the pet: " + error); return false; }

            state.Message = "Loading the food…";
            Log("  loading the food");
            if (!LoadFood(ser, out error)) { Log("  FAILED loading the food: " + error); return false; }

            state.Message = "Starting boarding…";
            Log("  starting boarding");
            if (!StartBoarding(ser, out error)) { Log("  FAILED starting: " + error); return false; }

            Log("  reload complete");
            return true;
        }
        finally
        {
            CloseBoarding(ser);
        }
    }

    /// <summary>目錄 → the pet feed icon → the boarding window (which brings the bag up with it).</summary>
    private bool OpenBoarding(SerialPort ser, out string error)
    {
        if (!Click(ser, _cfg.Pet.MenuButton!, right: false, out error)) return false;
        SleepCheck(WindowWait);
        return Click(ser, _cfg.Pet.FeedIcon!, right: false, out error);
    }

    private void CloseBoarding(SerialPort ser)
    {
        if (!IsPoint(_cfg.Pet.CloseButton)) return;
        if (!Click(ser, _cfg.Pet.CloseButton!, right: false, out _)) return;
        SleepCheck(ClickWait);
    }

    /// <summary>Right-click the marked cell to put the pet back into the boarding slot.
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
        return Click(ser, new List<int> { cx, cy }, right: true, out error);
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
                error = "Ran out of marked food cells.";
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
            if (!Click(ser, new List<int> { cx, cy }, right: true, out error)) return false;

            SleepCheck(DialogWait);
            if (!Click(ser, EffectiveMax()!, right: false, out error)) return false;
            SleepCheck(DialogWait);

            // ONE Enter, not two. The sell flow's second Enter dismisses a confirmation this dialog
            // does not have, so sending it would press an Enter into whatever follows.
            if (!Enter(ser, out error)) return false;
            SleepCheck(DialogWait);
        }

        return true;
    }

    private (int Page, int Cell)? NextFoodCell(PetConfig cfg)
    {
        var cells = cfg.FoodCells;
        if (cells.Count == 0) return null;

        var cell = cells[_nextFoodCell % cells.Count];
        _nextFoodCell++;
        return cell is { Count: 2 } ? (cell[0], cell[1]) : null;
    }

    private bool StartBoarding(SerialPort ser, out string error)
    {
        // The toggle's LABEL box is what is calibrated, so its centre is the click. The label sits on
        // the button, which is why one box serves as both the read and the press.
        var label = _cfg.Pet.ToggleLabel!;
        var centre = new List<int> { label[0] + label[2] / 2, label[1] + label[3] / 2 };
        return Click(ser, centre, right: false, out error);
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
        if (!Click(ser, tabs[page], right: false, out error)) return false;
        SleepCheck(ClickWait);
        return true;
    }

    // ── Plumbing ────────────────────────────────────────────────────────────

    private bool Click(SerialPort ser, List<int> point, bool right, out string error)
    {
        if (!PlaceOn(ser, point[0], point[1], out error)) return false;
        SleepCheck(ClickWait);
        if (right) HidPointer.RightClick(ser);
        else HidPointer.Click(ser);
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

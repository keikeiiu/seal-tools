using System;
using System.Collections.Generic;
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

        var failures = 0;
        try
        {
            while (!QuitPressed && !ct.IsCancellationRequested)
            {
                if (ReloadOnce(ser, ct, out var error))
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
        if (IconMatch.FromBase64(pet.PetIconPng) == null)
            return "The pet icon isn't captured — Calibrate Pet. Without it the pet cannot be found " +
                   "after boarding drops it.";
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

    private bool ReloadOnce(SerialPort ser, CancellationToken ct, out string error)
    {
        error = "";

        if (!OpenBoarding(ser, out error)) return false;

        // Every exit past this point closes the window: leaving it open would sit on top of the game
        // while the tool waits out its next cycle, and the next cycle would click 目錄 behind it.
        try
        {
            if (!PlacePet(ser, out error)) return false;
            if (!LoadFood(ser, out error)) return false;
            if (!StartBoarding(ser, out error)) return false;
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

    /// <summary>Find the pet in the bag and right-click it into the boarding slot.
    ///
    /// Searched across ALL pages, because the pet lands wherever the bag is free and that is not
    /// necessarily the page the food is on.</summary>
    private bool PlacePet(SerialPort ser, out string error)
    {
        error = "";
        var pet = _cfg.Pet;

        using var icon = IconMatch.FromBase64(pet.PetIconPng);
        if (icon == null)
        {
            error = "The pet icon is missing or no longer decodes — re-capture it on Calibrate Pet.";
            return false;
        }

        var pages = Math.Max(1, pet.PageTabs.Count);
        var bestCell = -1;
        var bestScore = double.MaxValue;

        for (int page = 0; page < pages; page++)
        {
            if (!SelectPage(ser, page, out error)) return false;

            var shot = CaptureClient();
            if (shot == null)
            {
                error = "Couldn't read the game window while looking for the pet.";
                return false;
            }

            using var bag = shot.Image;
            var found = IconMatch.FindBestCell(bag, pet.BagGrid!, icon);
            if (found != null && found.Value.Score < bestScore)
            {
                // Cell index is the same on every page, so the page it was found on has to be kept —
                // the same index on another page is a different item.
                bestScore = found.Value.Score;
                bestCell = found.Value.Cell;
                _petPage = page;
            }
        }

        if (bestCell < 0)
        {
            error = "Couldn't look for the pet — check the bag grid calibration.";
            return false;
        }

        if (bestScore > MatchLimit)
        {
            // The refusal IS the safety feature. Every other failure here is recoverable; feeding the
            // wrong item is not, so a weak match stops the run rather than guessing.
            error = $"No bag cell looks like the pet — the closest is {bestScore:P0} different and the " +
                    $"limit is {MatchLimit:P0}. Nothing was clicked.";
            return false;
        }

        if (!SelectPage(ser, _petPage, out error)) return false;

        var centres = BagGrid.Centres(pet.BagGrid!);
        var (cx, cy) = centres[bestCell];
        Console.WriteLine($"[pet] found at page {_petPage + 1}, cell {bestCell} ({bestScore:P1} different)");
        return Click(ser, new List<int> { cx, cy }, right: true, out error);
    }

    private int _petPage;

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

    /// <summary>A fresh capture of the game client. Null when the window can't be read — which the
    /// callers treat as "stop", never as "carry on with an empty image".</summary>
    private GameCapture? CaptureClient()
    {
        var hwnd = WindowFinder.FindByTitle(_cfg.Window.Title);
        if (hwnd == IntPtr.Zero || WindowFinder.IsMinimized(hwnd)) return null;
        return ScreenCapture.CaptureClient(hwnd);
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

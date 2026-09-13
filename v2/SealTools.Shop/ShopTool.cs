using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO.Ports;
using System.Linq;
using System.Threading;
using SealTools.Core;
using SealTools.Core.Config;

namespace SealTools.Shop;

public enum ShopMode { Buy, Sell }

// Bulk buying and selling through the shop and bag windows. Both are the same transaction: open the
// count dialog, press MAX, Enter, Enter. Buying opens it by left-clicking a shop row, selling by
// right-clicking a bag slot — everything after that is identical, which is why one class does both.
//
// Selling is the only irreversible thing in this suite: a mis-aimed click sells the wrong stack and
// it is gone. Two consequences, both enforced here rather than left to the caller:
//   * slots are sold HIGHEST INDEX FIRST, which is correct whether or not the bag compacts after a
//     sale. (Ascending would silently skip items if it does: removing slot 0 shifts slot 1 into it.)
//   * SellCap is a hard ceiling on the loop, not advice.
public sealed class ShopTool : ToolBase
{
    // Waits between steps. Generous on purpose — a transaction that outruns the game's animation
    // sells the wrong thing, and the cost of waiting is a slower correct run.
    private const double DialogWait = 0.45;
    private const double ClickWait = 0.35;

    private readonly AppConfig _cfg;
    private readonly ShopMode _mode;
    private readonly string? _presetName;

    public ShopTool(AppConfig cfg, ShopMode mode, string? presetName)
        : base(cfg.Hotkeys)
    {
        _cfg = cfg;
        _mode = mode;
        _presetName = presetName;
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

        bool running = false;
        bool f12Was = Hotkeys.IsDown(_cfg.Hotkeys.Start);

        try
        {
            while (true)
            {
                SleepCheck(0.05);
                if (QuitPressed || ct.IsCancellationRequested) break;

                if (state.Running && !running)
                {
                    running = true;
                    QuitPressed = false;
                    Console.WriteLine(_mode == ShopMode.Buy ? "[Panel] BUY" : "[Panel] SELL");
                    Beep(523, 100);
                }
                else if (!state.Running && running)
                {
                    running = false;
                    Console.WriteLine("[Panel] STOP");
                    Beep(1000, 150);
                }

                bool f12Now = Hotkeys.IsDown(_cfg.Hotkeys.Start);
                if (f12Now && !f12Was)
                {
                    running = !running;
                    state.Running = running;
                    Console.WriteLine(running ? "[GO]" : "[STOP]");
                    Beep(running ? 523 : 1000, running ? 100 : 150);
                }
                f12Was = f12Now;

                if (!running) continue;

                // One pass, then stop: these are counted jobs, not loops. Running them forever would
                // be a way to spend a fortune or empty a bag by leaving the room.
                if (_mode == ShopMode.Buy) BuyPass(ser, state, ct);
                else SellPass(ser, state, ct);

                BeepMany();
                running = false;
                state.Running = false;
                break;
            }
        }
        finally
        {
            state.Running = false;
        }

        Console.WriteLine("Done.");
        return 0;
    }

    /// <summary>Why a run can't start, or null when it can. Checked before anything moves so a
    /// half-calibrated setup says what is missing instead of clicking into empty screen.</summary>
    private string? Ready(ToolState state)
    {
        var bs = _cfg.BuySell;

        if (_mode == ShopMode.Sell)
        {
            if (!BagGrid.IsValidRect(bs.BagGrid)) return "The bag grid isn't calibrated — Calibrate Buy/Sell.";
            if (!IsPoint(bs.MaxButton)) return "The MAX button isn't calibrated — Calibrate Buy/Sell.";
            if (bs.SellSlots.Count == 0) return "No slots are selected to sell.";
            if (bs.SellSlots.Count > bs.SellCap)
                return $"{bs.SellSlots.Count} slots selected but the per-run cap is {bs.SellCap}. " +
                       "Raise the cap on purpose, or narrow the selection.";
            return null;
        }

        if (_presetName == null) return "No buy preset selected.";
        if (!bs.Presets.TryGetValue(_presetName, out var preset))
            return $"Buy preset '{_presetName}' no longer exists.";

        var rowProblem = ShopGeometry.Problem(bs.ShopFirstRow, bs.ShopSecondRow);
        if (rowProblem != null) return "Shop rows aren't calibrated — " + rowProblem + ".";
        if (!IsPoint(bs.ScrollPoint)) return "The scroll point isn't calibrated — Calibrate Buy/Sell.";
        if (!IsPoint(bs.MaxButton)) return "The MAX button isn't calibrated — Calibrate Buy/Sell.";
        if (preset.Count < 1) return $"Preset '{_presetName}' is set to buy 0.";
        return null;
    }

    private static bool IsPoint(List<int>? p) => p is { Count: 2 };

    // ── Buy ──────────────────────────────────────────────────────────────────

    private void BuyPass(SerialPort ser, ToolState state, CancellationToken ct)
    {
        var bs = _cfg.BuySell;
        var preset = bs.Presets[_presetName!];

        // Scroll to the top first, always. The stored amount is "notches down from the top", so
        // without a known origin it would mean something different every run.
        if (!ScrollToTop(ser, state)) return;
        if (preset.Scroll > 0)
        {
            Scroll(ser, preset.Scroll, down: true);
            SleepCheck(DialogWait);
        }

        var row = ShopGeometry.RowCentre(bs.ShopFirstRow, bs.ShopSecondRow, preset.Row);
        if (row is not { } target)
        {
            state.Message = "Couldn't work out the shop row — recalibrate the two row marks.";
            return;
        }

        for (int i = 0; i < preset.Count; i++)
        {
            if (QuitPressed || ct.IsCancellationRequested) return;

            state.Cycle = i + 1;
            state.Current = $"{_presetName} {i + 1}/{preset.Count}";

            if (!ClickAt(ser, target.X, target.Y, right: false, out var error))
            {
                Stop(state, error);
                return;
            }
            if (!MaxEnterEnter(ser, out error))
            {
                Stop(state, error);
                return;
            }
        }

        state.Message = $"Bought {preset.Count}x '{_presetName}'.";
    }

    // ── Sell ─────────────────────────────────────────────────────────────────

    private void SellPass(SerialPort ser, ToolState state, CancellationToken ct)
    {
        var bs = _cfg.BuySell;
        var centres = BagGrid.Centres(bs.BagGrid!);

        // Highest index first — see the class comment. The cap is re-applied here even though Ready()
        // checked it, because this is the loop that destroys things.
        var slots = bs.SellSlots.Where(i => i >= 0 && i < centres.Count)
                              .Distinct()
                              .OrderByDescending(i => i)
                              .Take(bs.SellCap)
                              .ToList();

        for (int n = 0; n < slots.Count; n++)
        {
            if (QuitPressed || ct.IsCancellationRequested) return;

            var centre = centres[slots[n]];
            state.Cycle = n + 1;
            state.Current = $"slot {slots[n]} ({n + 1}/{slots.Count})";

            if (!ClickAt(ser, centre.X, centre.Y, right: true, out var error))
            {
                Stop(state, error);
                return;
            }
            if (!MaxEnterEnter(ser, out error))
            {
                Stop(state, error);
                return;
            }
        }

        state.Message = $"Sold {slots.Count} slot(s).";
    }

    // ── Shared ───────────────────────────────────────────────────────────────

    /// <summary>The half of a transaction that is identical for buying and selling: MAX, then Enter
    /// for the count dialog, then Enter again for the confirmation.</summary>
    private bool MaxEnterEnter(SerialPort ser, out string error)
    {
        SleepCheck(DialogWait);

        var max = _cfg.BuySell.MaxButton!;
        if (!ClickAt(ser, max[0], max[1], right: false, out error)) return false;
        SleepCheck(ClickWait);

        Enter(ser);
        SleepCheck(DialogWait);

        Enter(ser);
        SleepCheck(DialogWait);
        return true;
    }

    private static void Enter(SerialPort ser) => ser.Write("E\n");

    private static void Scroll(SerialPort ser, int notches, bool down)
        => ser.Write((down ? "Z " : "Q ") + notches.ToString(CultureInfo.InvariantCulture) + "\n");

    /// <summary>Wheel-up at the firmware's maximum and let it clamp, giving a known list origin.</summary>
    private bool ScrollToTop(SerialPort ser, ToolState state)
    {
        var point = _cfg.BuySell.ScrollPoint!;
        if (!PlaceOn(ser, point[0], point[1], out var error))
        {
            Stop(state, error);
            return false;
        }
        Scroll(ser, WheelMax, down: false);
        SleepCheck(DialogWait);
        return true;
    }

    private bool ClickAt(SerialPort ser, int x, int y, bool right, out string error)
    {
        if (!PlaceOn(ser, x, y, out error)) return false;
        SleepCheck(ClickWait);
        if (right) HidPointer.RightClick(ser);
        else HidPointer.Click(ser);
        return true;
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
            // Never click after a failed placement: the cursor is somewhere unknown, and for selling
            // "somewhere unknown" means an arbitrary stack.
            error = placed.Error ?? $"couldn't place the cursor at ({x},{y})";
            return false;
        }
        return true;
    }

    /// <summary>The firmware clamps a scroll to 30; sending that is "go to the top".</summary>
    private const int WheelMax = 30;

    private static void Stop(ToolState state, string reason)
    {
        Console.WriteLine("[!] " + reason);
        state.Message = reason;
        state.Running = false;
    }
}

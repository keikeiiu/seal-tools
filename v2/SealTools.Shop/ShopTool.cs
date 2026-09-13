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

// Bulk buying and selling through the shop and bag windows. Both are the same transaction, and now
// the same gesture too: RIGHT-click the item — a shop row to buy, a bag slot to sell — then MAX,
// Enter, Enter. Everything after the first click is identical, which is why one class does both.
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
            if (!IsPoint(bs.ScrollPoint)) return "The focus point isn't calibrated — Calibrate Buy/Sell. " +
                "The game ignores a click while it is unfocused, and it is unfocused because you just " +
                "clicked this window to start the run.";
            if (bs.SellSlots.Count == 0) return "No slots are selected to sell.";
            if (bs.SellSlots.Count > bs.SellCap)
                return $"{bs.SellSlots.Count} slots selected but the per-run cap is {bs.SellCap}. " +
                       "Raise the cap on purpose, or narrow the selection.";
            return null;
        }

        if (_presetName == null) return "No buy preset selected.";
        if (!bs.Presets.TryGetValue(_presetName, out var preset))
            return $"Buy preset '{_presetName}' no longer exists.";

        var rowProblem = ShopGeometry.Problem(bs.ShopRegion, bs.ShopRows);
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

        // From wherever the list is — which must be the top, because the preset's amount is measured
        // from there. Left at the top by the user; see ScrollFromTop.
        if (!ScrollFromTop(ser, state, preset.Scroll)) return;

        var row = ShopGeometry.RowCentre(bs.ShopRegion, bs.ShopRows, preset.Row);
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

            // RIGHT click, like selling. The shop row takes a right-click to open the count dialog —
            // a left click does nothing, which is what made the first live buy silently no-op.
            if (!ClickAt(ser, target.X, target.Y, right: true, out var error))
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

        // Focus before the first right-click. Without it the first sale does nothing, and because
        // nothing reports back that looks exactly like a wrong coordinate.
        if (!FocusGame(ser, state)) return;

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

    /// <summary>How long the firmware needs to walk a scroll out: one gap per notch plus slack. A
    /// short wait is worse than a long one here — the HID moves that follow queue behind the scroll
    /// on the board, so placing the cursor too early reads a cursor that has not moved yet and keeps
    /// correcting against a stale position.</summary>
    private static double ScrollSettle(int notches) => notches * WheelGapSeconds + 0.4;

    /// <summary>Must match WHEEL_NOTCH_GAP_MS in seal_mouse.ino.</summary>
    private const double WheelGapSeconds = 0.025;

    private static void Scroll(SerialPort ser, int notches, bool down)
        => ser.Write((down ? "Z " : "Q ") + notches.ToString(CultureInfo.InvariantCulture) + "\n");

    /// <summary>Focuses the game, then applies the preset's scroll. Called ONCE at the start of a buy
    /// run — the purchases that follow just return to the same row and click, so nothing scrolls again.
    ///
    /// The click is required. The wheel needs the game focused, and starting the tool means clicking
    /// the launcher, which takes focus away — so without this the first scroll arrives at an unfocused
    /// game and is silently ignored. It also means THE SCROLL POINT SHOULD BE SOMEWHERE INERT: this
    /// left-clicks it, so a point over a list row would select or buy that row.
    ///
    /// There is deliberately no scroll-to-top: the list is expected to already BE at the top, and
    /// putting it there would be the length of the whole list on every run.</summary>
    /// <summary>Left-clicks the calibrated point to give the game focus. BOTH flows need this, for
    /// the same reason: starting a tool means clicking the launcher, and that takes focus away — after
    /// which the game ignores a wheel notch, and ignores a right-click. So it must be a real click,
    /// which is why the point has to be somewhere inert.
    ///
    /// It is the mark the Buy tab calls the "scroll point": buying also scrolls from here, since the
    /// wheel acts anywhere in the focused window and so the two jobs can share one point.</summary>
    private bool FocusGame(SerialPort ser, ToolState state)
    {
        if (_cfg.BuySell.ScrollPoint is not { Count: 2 } point)
        {
            Stop(state, "The focus point isn't calibrated — the game has to be focused before it will " +
                        "take a click or a wheel notch. Mark it in Calibrate Buy / Sell.");
            return false;
        }
        if (!PlaceOn(ser, point[0], point[1], out var error))
        {
            Stop(state, error);
            return false;
        }
        SleepCheck(ClickWait);
        HidPointer.Click(ser);
        SleepCheck(DialogWait);
        return true;
    }

    private bool ScrollFromTop(SerialPort ser, ToolState state, int notches)
    {
        if (!FocusGame(ser, state)) return false;

        if (notches > 0)
        {
            Scroll(ser, notches, down: true);
            SleepCheck(ScrollSettle(notches));
        }
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

    private static void Stop(ToolState state, string reason)
    {
        Console.WriteLine("[!] " + reason);
        state.Message = reason;
        state.Running = false;
    }
}

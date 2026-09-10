using System;
using System.IO.Ports;

namespace SealTools.Core;

/// <summary>Where an Arduino cursor placement ended up. <see cref="Steps"/> is how many HID moves
/// were sent; <see cref="X"/>/<see cref="Y"/> are the final cursor position (null when it couldn't
/// be read); <see cref="Error"/> is a user-facing reason when <see cref="Ok"/> is false.</summary>
public sealed record CursorPlacement(bool Ok, int Steps, int? X, int? Y, string? Error);

// Shared low-level "tool calls" for the Gem Composer (runtime loop) and the calibrate tab's test
// buttons (GemTestClick / GemTestRelativeMove). Both drive the same Arduino HID mouse and both need
// the game window's measured geometry. Centralising them means the test path and the runtime path
// send the EXACT same bytes and move the cursor the EXACT same way — so a click that lands in a test
// lands in the composer too. Callers keep their own sleeps in between so the composer's quit-aware
// SleepCheck (and its tuned delays) is preserved unchanged.
//
// Coordinates are CLIENT-RELATIVE PHYSICAL pixels (docs/COORDINATES.md); To() converts the target
// with the measured DisplayInfo into the process's cursor space and drives the cursor there.
//
// The cursor is positioned with the ARDUINO, not SetCursorPos (docs/CURSOR-INVESTIGATION.md): the
// launcher's process is intermittently refused by SetCursorPos/SetPhysicalCursorPos while the
// identical call succeeds from another process, and a refused move is silently ignored — so the old
// code clicked wherever the mouse happened to be. The HID path cannot be refused, and To() now
// verifies the landing instead of assuming it.
//
// Focus model (do not "fix" by adding a Win32 focus API, a centre-click, or a dedicated
// focus-click): when a tool is started from the launcher the game is not the foreground window,
// but a single click on the target button BOTH activates the game window and presses the button —
// the same thing v1's SetCursorPos + C does. Click() is the only focus mechanism needed.
public static class GemPointer
{
    // A click target is a button tens of pixels wide, so landing within this of the calibrated
    // point is a hit. Two px also absorbs the rounding in the physical -> cursor-space conversion.
    private const int TolerancePx = 2;

    // Bounded so a wedged pointer can't hang the composer. One iteration is the normal case: the
    // measured gain is 1:1 (D 100 0 -> +100 px in GetCursorPos space), so the first move lands.
    private const int MaxSteps = 6;

    // The firmware walks a "D dx dy" move in 10-px chunks with a 1 ms gap (arduino/seal_mouse.ino),
    // so a long move keeps arriving for tens of ms after Write returns. Sampling too early makes the
    // loop chase a stale position and overshoot, so each move is followed by a poll until the
    // position stops changing.
    private const int PollMs = 15;
    private const int MaxSettlePolls = 24;

    /// <summary>Cap on a single HID move — a wrong target must not fling the pointer across the
    /// desktop (and into whatever is there).</summary>
    private const int MaxStepPx = 600;

    /// <summary>Measures the game window (scale + physical/logical rects). Null when it isn't open
    /// OR is minimized — a minimized window's rect is off-screen, which would fling the cursor to
    /// the screen edge instead of the button.</summary>
    public static DisplayInfo? Display(string titleSubstring)
    {
        var hwnd = WindowFinder.FindByTitle(titleSubstring);
        if (hwnd == IntPtr.Zero || WindowFinder.IsMinimized(hwnd)) return null;
        return Dpi.Measure(hwnd);
    }

    /// <summary>Move the cursor to a precomputed target by driving the Arduino HID mouse in a closed
    /// loop against <see cref="WindowFinder.LogicalCursorPosition"/> (which reads correctly in this
    /// process even when the move API is refused). Never throws for a missed target — the caller
    /// must check <see cref="CursorPlacement.Ok"/> and refuse to click rather than click blind.
    ///
    /// Compute the target with WindowFinder.ComputeCursorTarget(display, x, y).</summary>
    public static CursorPlacement To(SerialPort ser, WindowFinder.CursorTarget target)
    {
        int goalX = target.LogicalX, goalY = target.LogicalY;
        int divisor = 1;
        int lastError = int.MaxValue;
        int moves = 0;

        for (int step = 1; step <= MaxSteps; step++)
        {
            if (WindowFinder.LogicalCursorPosition() is not { } cursor)
                return new CursorPlacement(false, moves, null, null, "the cursor position can't be read (GetCursorPos failed)");
            var (x, y) = cursor;

            int dx = goalX - x, dy = goalY - y;
            int error = Math.Max(Math.Abs(dx), Math.Abs(dy));
            if (error <= TolerancePx)
                return new CursorPlacement(true, moves, x, y, null);

            // A step that made the error worse means the machine scales the HID counts: damp
            // instead of oscillating. On the reference machine this never triggers.
            if (error >= lastError) divisor *= 2;
            lastError = error;

            Move(ser, StepCount(dx, divisor), StepCount(dy, divisor));
            moves++;
            WaitForCursorToSettle();
        }

        var final = WindowFinder.LogicalCursorPosition();
        return new CursorPlacement(false, moves, final?.X, final?.Y,
            $"the cursor wouldn't move to ({goalX},{goalY}) — it stopped at ({final?.X},{final?.Y})");
    }

    /// <summary>Blocks until two consecutive polls report the same position, i.e. the HID move the
    /// firmware is walking out in chunks has finished. Bounded — a cursor that never stops moving
    /// must not hang the composer.</summary>
    private static void WaitForCursorToSettle()
    {
        var previous = WindowFinder.LogicalCursorPosition();
        if (previous is null) return;
        for (int i = 0; i < MaxSettlePolls; i++)
        {
            Thread.Sleep(PollMs);
            var now = WindowFinder.LogicalCursorPosition();
            if (now == previous) return;
            previous = now;
        }
    }

    /// <summary>One axis of a closed-loop correction: the remaining error, damped and clamped, and
    /// never rounded away to 0 (that would stall the loop short of the tolerance).</summary>
    private static int StepCount(int delta, int divisor)
    {
        if (delta == 0) return 0;
        int v = delta / divisor;
        return v == 0 ? Math.Sign(delta) : Math.Clamp(v, -MaxStepPx, MaxStepPx);
    }

    /// <summary>HID left-click at the current cursor position. This is what focuses the game.</summary>
    public static void Click(SerialPort ser) => ser.Write("C\n");

    /// <summary>HID relative "D dx dy" move. The composer's route moves pass raw hand-tuned counts
    /// (never scaled by DPI); <see cref="To"/> computes its own correction counts.</summary>
    public static void Move(SerialPort ser, int dx, int dy) => ser.Write($"D {dx} {dy}\n");

    /// <summary>HID right-click at the current cursor position (clears a stuck resource gem).</summary>
    public static void RightClick(SerialPort ser) => ser.Write("R\n");
}

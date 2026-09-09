using System;
using System.IO.Ports;

namespace SealTools.Core;

// Shared low-level "tool calls" for the Gem Composer (runtime loop) and the calibrate tab's test
// buttons (GemTestClick / GemTestRelativeMove). Both drive the same Arduino HID mouse and both need
// the game window's measured geometry. Centralising them means the test path and the runtime path
// send the EXACT same bytes and move the cursor the EXACT same way — so a click that lands in a test
// lands in the composer too. Callers keep their own sleeps in between so the composer's quit-aware
// SleepCheck (and its tuned delays) is preserved unchanged.
//
// Coordinates are CLIENT-RELATIVE PHYSICAL pixels (docs/COORDINATES.md). To() uses the measured
// DisplayInfo: SetPhysicalCursorPos when it works, else SetCursorPos with the physical offset
// divided by the measured scale.
//
// Focus model (do not "fix" by adding a Win32 focus API, a centre-click, or a dedicated
// focus-click): when a tool is started from the launcher the game is not the foreground window,
// but a single click on the target button BOTH activates the game window and presses the button —
// the same thing v1's SetCursorPos + C does. Click() is the only focus mechanism needed.
public static class GemPointer
{
    /// <summary>Measures the game window (scale + physical/logical rects). Null when it isn't open
    /// OR is minimized — a minimized window's rect is off-screen, which would fling the cursor to
    /// the screen edge instead of the button.</summary>
    public static DisplayInfo? Display(string titleSubstring)
    {
        var hwnd = WindowFinder.FindByTitle(titleSubstring);
        if (hwnd == IntPtr.Zero || WindowFinder.IsMinimized(hwnd)) return null;
        return Dpi.Measure(hwnd);
    }

    /// <summary>Move the OS cursor to a precomputed target (no click), via SetCursorPos on the
    /// virtualised coordinates — v1's call. Compute the target with
    /// WindowFinder.ComputeCursorTarget(display, x, y); SetPhysicalCursorPos is NOT used (measured
    /// on the reference machine, it does not land where it claims).</summary>
    public static void To(WindowFinder.CursorTarget target)
        => WindowFinder.SetLogicalCursorPosition(target);

    /// <summary>HID left-click at the current cursor position. This is what focuses the game.</summary>
    public static void Click(SerialPort ser) => ser.Write("C\n");

    /// <summary>HID relative "D dx dy" move — raw hand-tuned counts, never scaled by DPI.</summary>
    public static void Move(SerialPort ser, int dx, int dy) => ser.Write($"D {dx} {dy}\n");

    /// <summary>HID right-click at the current cursor position (clears a stuck resource gem).</summary>
    public static void RightClick(SerialPort ser) => ser.Write("R\n");
}

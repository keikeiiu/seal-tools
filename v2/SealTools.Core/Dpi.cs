using System;
using System.Runtime.InteropServices;

namespace SealTools.Core;

// The same window as it appears in the two coordinate spaces that matter here.
public sealed record DisplayInfo(
    double Scale,             // physical px per logical px, measured (e.g. 1.5)
    uint MonitorDpi,          // GetDpiForWindow on the aware thread (e.g. 144)
    WindowRect PhysicalFrame,
    WindowRect PhysicalClient,
    WindowRect LogicalClient);

// Measures the display scale of the monitor a window is on.
//
// The process runs DPI-UNAWARE (see docs/COORDINATES.md), so every Win32 call is virtualised and
// GetDpiForWindow reports 96 — the real scale is invisible. The only way to learn it is to read the
// window twice: once normally, once with THIS THREAD briefly switched to per-monitor-aware. The
// ratio of the two client widths is the scale (2865/1910 = 1.5 on the reference machine).
//
// HARD RULE: the aware context is thread-scoped and restored in a finally. No SetCursorPos /
// SetPhysicalCursorPos call may run while a thread is aware — that is what broke in 7f9e1c7
// (ok=False on a mixed-DPI dual-monitor setup). This class only measures.
public static class Dpi
{
    // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
    private static readonly IntPtr PerMonitorAwareV2 = new(-4);

    [DllImport("user32.dll")]
    private static extern IntPtr SetThreadDpiAwarenessContext(IntPtr dpiContext);

    /// <summary>Runs <paramref name="action"/> with THIS THREAD switched to per-monitor-aware, so
    /// Win32 calls return physical pixels, then restores the previous context.
    ///
    /// HARD RULE: the action must never move the cursor (SetCursorPos / SetPhysicalCursorPos) —
    /// they fail with ok=False on an aware thread on a mixed-DPI dual-monitor setup (7f9e1c7).
    /// Measurement and screen capture only.</summary>
    public static T WithAwareContext<T>(Func<T> action)
    {
        var previous = SetThreadDpiAwarenessContext(PerMonitorAwareV2);
        bool switched = previous != IntPtr.Zero; // null return means the call failed; nothing to restore
        try
        {
            return action();
        }
        finally
        {
            if (switched) SetThreadDpiAwarenessContext(previous);
        }
    }

    /// <summary>Reads the window in both spaces and returns the measured scale, or null when the
    /// window handle is invalid / the rects can't be read.</summary>
    public static DisplayInfo? Measure(IntPtr hwnd)
    {
        if (hwnd == IntPtr.Zero) return null;

        // Unaware view first (the process default) — must be read before switching.
        var logicalClient = WindowFinder.GetClientRectInScreen(hwnd);
        if (logicalClient == null) return null;

        return WithAwareContext(() =>
        {
            var physicalClient = WindowFinder.GetClientRectInScreen(hwnd);
            var physicalFrame = WindowFinder.GetFrameRect(hwnd);
            if (physicalClient == null || physicalFrame == null) return (DisplayInfo?)null;

            // Widths, not heights: the height ratio truncates (1193 * 1.5 = 1789.5 -> 1789).
            double scale = logicalClient.Width > 0
                ? physicalClient.Width / (double)logicalClient.Width
                : 1.0;

            // GetDpiForWindow reports the WINDOW's awareness (96 for a DPI-unaware game), not the
            // monitor's scale, so derive the effective DPI from the measured ratio instead.
            uint dpi = (uint)Math.Round(96 * scale);

            return new DisplayInfo(scale, dpi, physicalFrame, physicalClient, logicalClient);
        });
    }
}

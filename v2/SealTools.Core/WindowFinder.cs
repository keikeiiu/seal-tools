using System;
using System.IO;
using System.Runtime.InteropServices;

namespace SealTools.Core;

// Client-area coordinates in screen space, DPI-aware.
// Canonical origin = game window client-area top-left (GetClientRect + ClientToScreen),
// NOT GetWindowRect (which includes the title bar/borders that vary across themes).

public sealed record WindowRect(int Left, int Top, int Width, int Height)
{
    public int Right => Left + Width;
    public int Bottom => Top + Height;
}

public static class WindowFinder
{
    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowTextLengthW(IntPtr hWnd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowTextW(IntPtr hWnd, [Out] char[] lpString, int nMaxCount);

    [DllImport("user32.dll")]
    private static extern bool GetClientRect(IntPtr hWnd, out RECT lpRect);

    [DllImport("user32.dll")]
    private static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);

    [DllImport("user32.dll")]
    private static extern bool ClientToScreen(IntPtr hWnd, ref POINT lpPoint);

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr hWnd);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetCursorPos(int x, int y);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetPhysicalCursorPos(int x, int y);

    [DllImport("user32.dll")]
    private static extern bool GetCursorPos(out POINT lpPoint);

    [DllImport("user32.dll")]
    private static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    private static extern int GetSystemMetrics(int nIndex);

    [DllImport("user32.dll")]
    private static extern bool IsIconic(IntPtr hWnd);

    /// <summary>True when the window is minimized. A minimized window reports its rect off-screen
    /// (e.g. -48000,-48000 with size 0x0), so any coordinate derived from it is garbage.</summary>
    public static bool IsMinimized(IntPtr hwnd) => hwnd != IntPtr.Zero && IsIconic(hwnd);

    /// <summary>Primary screen size as this thread sees it — PHYSICAL on a DPI-aware thread,
    /// virtualised (logical) on the unaware default. Only meaningful inside Dpi.WithAwareContext.</summary>
    public static (int Width, int Height) PrimaryScreenSize() => (GetSystemMetrics(0), GetSystemMetrics(1));

    // Title of the current foreground window ("" if none). Diagnostic aid for focus issues.
    public static string ForegroundTitle()
    {
        var hwnd = GetForegroundWindow();
        if (hwnd == IntPtr.Zero) return "";
        int len = GetWindowTextLengthW(hwnd);
        if (len == 0) return "";
        var buf = new char[len + 1];
        int written = GetWindowTextW(hwnd, buf, len + 1);
        return written <= 0 ? "" : new string(buf, 0, written);
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT { public int X, Y; }

    public static IntPtr FindByTitle(string titleSubstring)
    {
        IntPtr found = IntPtr.Zero;
        EnumWindows((hWnd, _) =>
        {
            if (!IsWindowVisible(hWnd)) return true;
            int len = GetWindowTextLengthW(hWnd);
            if (len == 0) return true;
            var buffer = new char[len + 1];
            int copied = GetWindowTextW(hWnd, buffer, buffer.Length);
            var title = new string(buffer, 0, copied);
            if (title.Contains(titleSubstring, StringComparison.Ordinal))
            {
                found = hWnd;
                return false;
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    // Client-area rect expressed in screen coordinates (the canonical origin). The origin is the
    // CLIENT-area top-left (GetClientRect + ClientToScreen), NOT the window frame — the frame
    // includes the title bar/border, which would offset every region/click by ~(8,31) px.
    public static WindowRect? GetClientRectInScreen(IntPtr hWnd)
    {
        if (hWnd == IntPtr.Zero) return null;
        if (!GetClientRect(hWnd, out RECT cr)) return null;
        var origin = new POINT { X = 0, Y = 0 };
        if (!ClientToScreen(hWnd, ref origin)) return null;
        return new WindowRect(origin.X, origin.Y, cr.Right - cr.Left, cr.Bottom - cr.Top);
    }

    // Window FRAME rect in screen coords — includes the title bar and borders. Diagnostic only:
    // the canonical origin everywhere else is the CLIENT area (see GetClientRectInScreen). The
    // difference between the two is the non-client offset, reported by the calibrator's
    // "Diagnose capture" button.
    public static WindowRect? GetFrameRect(IntPtr hWnd)
    {
        if (hWnd == IntPtr.Zero) return null;
        if (!GetWindowRect(hWnd, out RECT r)) return null;
        return new WindowRect(r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top);
    }

    // Re-query per call — never cache across a mixed-DPI multi-monitor move.
    public static uint GetDpi(IntPtr hWnd) => GetDpiForWindow(hWnd);

    /// <summary>Where a client-relative physical offset lands on screen, in both coordinate spaces.</summary>
    public sealed record CursorTarget(int LogicalX, int LogicalY, int PhysicalX, int PhysicalY);

    /// <summary>Pure calculation, no Win32 call: converts a client-relative PHYSICAL offset into the
    /// absolute screen coordinates each cursor API expects.
    ///   logical  = logicalClientOrigin  + round(offset / scale)   (what SetCursorPos wants)
    ///   physical = physicalClientOrigin + offset                  (what SetPhysicalCursorPos wants)</summary>
    public static CursorTarget ComputeCursorTarget(DisplayInfo display, int physicalX, int physicalY) => new(
        display.LogicalClient.Left + (int)Math.Round(physicalX / display.Scale),
        display.LogicalClient.Top + (int)Math.Round(physicalY / display.Scale),
        display.PhysicalClient.Left + physicalX,
        display.PhysicalClient.Top + physicalY);

    /// <summary>Position the cursor with the PHYSICAL API (ignores DPI virtualisation) at a
    /// precomputed target. Returns whether the call was accepted. Debug/diagnostic use only — the
    /// tools position the cursor with the Arduino (GemPointer.To, docs/CURSOR-INVESTIGATION.md).</summary>
    public static bool SetPhysicalCursorPosition(CursorTarget target)
        => SetPhysicalCursorPos(target.PhysicalX, target.PhysicalY);

    /// <summary>Position the cursor with the virtualised (logical) API — v1's call — at a
    /// precomputed target. Returns whether the call was accepted.</summary>
    public static bool SetLogicalCursorPosition(CursorTarget target)
        => SetCursorPos(target.LogicalX, target.LogicalY);

    /// <summary>Where the cursor is now, in the process's (logical) space, or null when the position
    /// can't be read. Nullable on purpose: a NEGATIVE coordinate is legitimate on a monitor left of
    /// the primary one, so it must never double as the failure signal.</summary>
    public static (int X, int Y)? LogicalCursorPosition() => GetCursorPos(out var p) ? (p.X, p.Y) : null;

    // Note: the Set*CursorPosition helpers exist for the calibrator's debug buttons only. The tools
    // position the cursor with the Arduino — see GemPointer.To and docs/CURSOR-INVESTIGATION.md.

    /// <summary>Diagnostic: move the OS cursor to an absolute screen point and log target vs actual.</summary>
    public static void DebugCursor(int x, int y)
    {
        bool ok = SetCursorPos(x, y);
        LogCursor("DEBUG", x, y, ok, Marshal.GetLastWin32Error());
    }

    public static void DebugPhysicalCursor(int x, int y)
    {
        bool ok = SetPhysicalCursorPos(x, y);
        LogCursor("PHYS", x, y, ok, Marshal.GetLastWin32Error());
    }

    private static void LogCursor(string tag, int x, int y, bool ok, int err)
    {
        var after = GetCursorPos(out var p) ? (p.X, p.Y) : ((int)-1, (int)-1);
        try
        {
            var dir = Path.Combine(AppContext.BaseDirectory, "logs");
            Directory.CreateDirectory(dir);
            File.AppendAllText(Path.Combine(dir, "cursor_debug.txt"),
                $"{DateTime.Now:HH:mm:ss} [{tag}] target=({x},{y}) ok={ok} err={err} after=({after.Item1},{after.Item2})\n");
        }
        catch { /* ignore logging failures */ }
    }
}

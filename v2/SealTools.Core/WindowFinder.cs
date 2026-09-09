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

    // Re-query per call — never cache across a mixed-DPI multi-monitor move.
    public static uint GetDpi(IntPtr hWnd) => GetDpiForWindow(hWnd);

    // No logging here: this is the composer's per-move hot path. The Debug* methods below log
    // explicitly when you actually want a cursor trace.
    public static void MoveCursor(WindowRect client, int clientX, int clientY)
        => SetCursorPos(client.Left + clientX, client.Top + clientY);

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

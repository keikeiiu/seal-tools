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
    private const int DpiAwarenessContextPerMonitorAwareV2 = -4;

    [DllImport("user32.dll")]
    private static extern bool SetProcessDpiAwarenessContext(IntPtr value);

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
    private static extern IntPtr GetThreadDpiAwarenessContext();

    [DllImport("user32.dll")]
    private static extern int GetAwarenessFromDpiAwarenessContext(IntPtr value);

    [DllImport("user32.dll")]
    private static extern bool ClipCursor(IntPtr lpRect);

    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    private static extern bool IsIconic(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern IntPtr GetForegroundWindow();

    private const int SwRestore = 9;

    // Bring the window to the foreground via a CLICK at the game window centre, not the
    // SetForegroundWindow API (Windows blocks programmatic focus-stealing, so that request can be
    // refused). This mirrors v1's reliable focus method: a real cursor move + click is allowed to
    // take focus. Restores the window first if it's minimised, moves the OS cursor to the client
    // centre, and returns the client rect so the caller issues the Arduino click on it.
    public static WindowRect? BringToForeground(IntPtr hWnd)
    {
        if (hWnd == IntPtr.Zero) return null;
        if (IsIconic(hWnd)) ShowWindow(hWnd, SwRestore);
        var client = GetClientRectInScreen(hWnd);
        if (client == null) return null;
        MoveCursor(client, client.Width / 2, client.Height / 2);
        return client;
    }

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

    // True when the window whose title contains titleSubstring is currently the foreground
    // window (v1 is_game_focused). Settles whether a focus click is needed first.
    public static bool IsForeground(string titleSubstring)
    {
        var hwnd = GetForegroundWindow();
        if (hwnd == IntPtr.Zero) return false;
        int len = GetWindowTextLengthW(hwnd);
        if (len == 0) return false;
        var buf = new char[len + 1];
        int written = GetWindowTextW(hwnd, buf, len + 1);
        if (written <= 0) return false;
        var title = new string(buf, 0, written);
        return title.Contains(titleSubstring, StringComparison.Ordinal);
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT { public int X, Y; }

    // Call once at startup so all Win32 APIs share one physical-pixel space.
    public static void EnablePerMonitorDpiAwareness()
    {
        try { SetProcessDpiAwarenessContext(new IntPtr(DpiAwarenessContextPerMonitorAwareV2)); }
        catch { /* not available on very old OS — ignore */ }
    }

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

    public static void MoveCursor(WindowRect client, int clientX, int clientY)
    {
        int sx = client.Left + clientX, sy = client.Top + clientY;
        bool ok = SetCursorPos(sx, sy);
        LogCursor("MOVE", sx, sy, ok, Marshal.GetLastWin32Error());
    }

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

    /// <summary>Diagnostic: replicate the full client-relative -> screen calculation and log every step.</summary>
    public static void DebugCursorCalc(int clientLeft, int clientTop, int relX, int relY)
    {
        int sx = clientLeft + relX, sy = clientTop + relY;
        bool ok = SetCursorPos(sx, sy);
        int err = Marshal.GetLastWin32Error();
        var after = GetCursorPos(out var p) ? (p.X, p.Y) : ((int)-1, (int)-1);
        try
        {
            var dir = Path.Combine(AppContext.BaseDirectory, "logs");
            Directory.CreateDirectory(dir);
            File.AppendAllText(Path.Combine(dir, "cursor_debug.txt"),
                $"{DateTime.Now:HH:mm:ss} [CALC] clientOrigin=({clientLeft},{clientTop}) rel=({relX},{relY}) screen=({sx},{sy}) ok={ok} err={err} after=({after.Item1},{after.Item2})\n");
        }
        catch { /* ignore logging failures */ }
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

    // Diagnostic: log the process DPI-awareness context so we can confirm the app is running
    // DPI-unaware (the mixed-DPI SetCursorPos fix) vs PerMonitorV2. 0=unaware, 1=system-aware,
    // 2=per-monitor-aware (v1/v2).
    public static void LogDpiAwareness()
    {
        try
        {
            var ctx = GetThreadDpiAwarenessContext();
            int awareness = GetAwarenessFromDpiAwarenessContext(ctx);
            var dir = Path.Combine(AppContext.BaseDirectory, "logs");
            Directory.CreateDirectory(dir);
            File.AppendAllText(Path.Combine(dir, "cursor_debug.txt"),
                $"{DateTime.Now:HH:mm:ss} [DPI] awareness={awareness} ctx=0x{ctx.ToInt64():X}\n");
        }
        catch { /* diagnostics must never break startup */ }
    }
}

using System;
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
    private static extern bool ClientToScreen(IntPtr hWnd, ref POINT lpPoint);

    [DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern bool SetCursorPos(int x, int y);

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

    // Client-area rect expressed in screen coordinates (the canonical origin).
    public static WindowRect? GetClientRectInScreen(IntPtr hWnd)
    {
        if (hWnd == IntPtr.Zero) return null;
        if (!GetClientRect(hWnd, out RECT cr)) return null;
        var p = new POINT { X = 0, Y = 0 };
        if (!ClientToScreen(hWnd, ref p)) return null;
        return new WindowRect(p.X, p.Y, cr.Right - cr.Left, cr.Bottom - cr.Top);
    }

    // Re-query per call — never cache across a mixed-DPI multi-monitor move.
    public static uint GetDpi(IntPtr hWnd) => GetDpiForWindow(hWnd);

    public static void MoveCursor(WindowRect client, int clientX, int clientY)
        => SetCursorPos(client.Left + clientX, client.Top + clientY);
}

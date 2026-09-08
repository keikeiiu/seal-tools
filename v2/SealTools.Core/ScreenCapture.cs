using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using OpenCvSharp;
using OpenCvSharp.Extensions;
using SealTools.Core.Config;

namespace SealTools.Core;

// Captures the game window via PrintWindow, which works for DirectX/DirectDraw titles that
// GDI's CopyFromScreen cannot read. Returns an OpenCvSharp Mat (BGR).
public static class ScreenCapture
{
    private const int PwRenderFullContent = 0x00000002;

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, int nFlags);

    public static Mat Capture(IntPtr hwnd, WindowRect rect)
    {
        using var bmp = new Bitmap(rect.Width, rect.Height, PixelFormat.Format24bppRgb);
        using (var g = Graphics.FromImage(bmp))
        {
            IntPtr hdc = g.GetHdc();
            try { PrintWindow(hwnd, hdc, PwRenderFullContent); }
            finally { g.ReleaseHdc(hdc); }
        }
        return BitmapConverter.ToMat(bmp);
    }

    // Capture a window-relative region: capture the whole window, then crop.
    public static Mat CaptureRegion(IntPtr hwnd, WindowRect window, RegionConfig region)
    {
        using var whole = Capture(hwnd, window);
        var left = Math.Clamp(region.Left, 0, window.Width - 1);
        var top = Math.Clamp(region.Top, 0, window.Height - 1);
        var w = Math.Min(region.Width, window.Width - left);
        var h = Math.Min(region.Height, window.Height - top);
        using var roi = new Mat(whole, new OpenCvSharp.Rect(left, top, w, h));
        return roi.Clone();
    }
}

using System;
using System.Drawing;
using System.Drawing.Imaging;
using OpenCvSharp;
using OpenCvSharp.Extensions;
using SealTools.Core.Config;

namespace SealTools.Core;

// A captured image plus the display measurement taken in the same peek.
public sealed record GameCapture(Mat Image, DisplayInfo Display);

// GDI CopyFromScreen capture — the only capture method. PrintWindow was measured to return a solid
// black frame for this game; do not reintroduce it.
//
// Everything is captured in PHYSICAL pixels (docs/COORDINATES.md): the process is DPI-unaware, so
// Win32 reports divided-down (logical) rects, but the screen blit is 1:1 physical. Requesting a
// logical rect therefore grabbed only the top-left 2/3 of the game. Every capture here runs on a
// thread switched to per-monitor-aware (Dpi.WithAwareContext) so the rects and the blit agree.
//
// CopyFromScreen reads what is actually on screen: the game must be visible (the launcher hides
// itself for the grab — see MainWindow.WithLauncherHiddenAsync).
public static class ScreenCapture
{
    /// <summary>GDI screen capture of an absolute PHYSICAL screen region (BGR).</summary>
    public static Mat CaptureScreen(WindowRect rect)
    {
        using var bmp = new Bitmap(rect.Width, rect.Height, PixelFormat.Format24bppRgb);
        using (var g = Graphics.FromImage(bmp))
        {
            g.CopyFromScreen(rect.Left, rect.Top, 0, 0,
                new System.Drawing.Size(rect.Width, rect.Height), CopyPixelOperation.SourceCopy);
        }
        return BitmapConverter.ToMat(bmp);
    }

    /// <summary>Capture the whole client area of a window in physical pixels, together with the
    /// display measurement (scale, physical/logical rects). Null when the window can't be read.</summary>
    public static GameCapture? CaptureClient(IntPtr hwnd)
    {
        if (hwnd == IntPtr.Zero) return null;

        // Logical view first (the process default) — must be read before switching context.
        var logicalClient = WindowFinder.GetClientRectInScreen(hwnd);
        if (logicalClient == null) return null;

        return Dpi.WithAwareContext(() =>
        {
            var client = WindowFinder.GetClientRectInScreen(hwnd);
            if (client == null) return (GameCapture?)null;
            var frame = WindowFinder.GetFrameRect(hwnd) ?? client;

            double scale = logicalClient.Width > 0 ? client.Width / (double)logicalClient.Width : 1.0;
            var info = new DisplayInfo(scale, (uint)Math.Round(96 * scale), frame, client, logicalClient);
            return new GameCapture(CaptureScreen(client), info);
        });
    }

    /// <summary>Capture a CLIENT-RELATIVE region in physical pixels (client origin + offset), on the
    /// aware thread. Used by OCR and the composer's empty-box check.</summary>
    public static GameCapture? CaptureClientRegion(IntPtr hwnd, RegionConfig region)
    {
        if (hwnd == IntPtr.Zero) return null;

        var logicalClient = WindowFinder.GetClientRectInScreen(hwnd);
        if (logicalClient == null) return null;

        return Dpi.WithAwareContext(() =>
        {
            var client = WindowFinder.GetClientRectInScreen(hwnd);
            if (client == null) return (GameCapture?)null;
            var frame = WindowFinder.GetFrameRect(hwnd) ?? client;

            double scale = logicalClient.Width > 0 ? client.Width / (double)logicalClient.Width : 1.0;
            var info = new DisplayInfo(scale, (uint)Math.Round(96 * scale), frame, client, logicalClient);

            var rect = new WindowRect(
                client.Left + region.Left,
                client.Top + region.Top,
                region.Width,
                region.Height);
            return new GameCapture(CaptureScreen(rect), info);
        });
    }
}

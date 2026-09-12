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
    /// <summary>GDI screen capture of an absolute PHYSICAL screen region (BGR). Null when the blit
    /// can't be done: a region with no area, or a locked/disconnected session, where CopyFromScreen
    /// throws because there is no input desktop. Callers rely on "null when the window can't be
    /// read" — returning null keeps a failed capture from taking down the tool that asked for it.</summary>
    public static Mat? CaptureScreen(WindowRect rect)
    {
        if (rect.Width <= 0 || rect.Height <= 0) return null;

        try
        {
            using var bmp = new Bitmap(rect.Width, rect.Height, PixelFormat.Format24bppRgb);
            using (var g = Graphics.FromImage(bmp))
            {
                g.CopyFromScreen(rect.Left, rect.Top, 0, 0,
                    new System.Drawing.Size(rect.Width, rect.Height), CopyPixelOperation.SourceCopy);
            }
            return BitmapConverter.ToMat(bmp);
        }
        catch (Exception)
        {
            return null;
        }
    }

    /// <summary>Capture the whole client area of a window in physical pixels, together with the
    /// display measurement (scale, physical/logical rects). Null when the window can't be read.</summary>
    public static GameCapture? CaptureClient(IntPtr hwnd) => Capture(hwnd, client => client);

    /// <summary>Capture a CLIENT-RELATIVE region in physical pixels (client origin + offset), on the
    /// aware thread. Used by OCR and the composer's empty-box check. Null when it can't be read.</summary>
    public static GameCapture? CaptureClientRegion(IntPtr hwnd, RegionConfig region)
        => Capture(hwnd, client => new WindowRect(
            client.Left + region.Left,
            client.Top + region.Top,
            region.Width,
            region.Height));

    // The two entry points above were the same body end to end — logical rect read, aware rect +
    // frame read, scale maths, DisplayInfo — differing only in the rect finally blitted. One copy,
    // so a fix to the measurement logic can't land in one path and be forgotten in the other.
    private static GameCapture? Capture(IntPtr hwnd, Func<WindowRect, WindowRect> region)
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
            var screen = WindowFinder.PrimaryScreenSize();
            var info = new DisplayInfo(scale, (uint)Math.Round(96 * scale), screen.Width, screen.Height,
                frame, client, logicalClient);

            var image = CaptureScreen(region(client));
            return image == null ? null : new GameCapture(image, info);
        });
    }
}

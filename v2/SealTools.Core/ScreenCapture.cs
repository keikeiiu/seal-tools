using System.Drawing;
using System.Drawing.Imaging;
using OpenCvSharp;
using OpenCvSharp.Extensions;
using SealTools.Core.Config;

namespace SealTools.Core;

// GDI CopyFromScreen capture, used for everything: calibration, OCR and the composer loop. The game
// is a windowed title, so GDI can read it, and using one method everywhere means the capture origin
// is always the CLIENT area — the space all stored coordinates live in.
//
// Do NOT reintroduce PrintWindow. It was used for the calibration screenshot and returned a solid
// black frame for this game (measured 2026-09-09 via the "Diagnose capture" button:
// diag_printwindow.png was a black client-sized image, diag_copyfromscreen.png the real screen).
// It also rendered from the window FRAME origin while everything else uses the CLIENT origin.
//
// Caveat: CopyFromScreen reads what is actually on screen, so the game must be visible — not
// covered by the launcher or another window — when capturing.
public static class ScreenCapture
{
    /// <summary>GDI screen capture of an absolute screen region (BGR).</summary>
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

    /// <summary>CopyFromScreen of a client-relative region (absolute coords = client origin + region).</summary>
    public static Mat CaptureScreenRegion(WindowRect client, RegionConfig region)
        => CaptureScreen(new WindowRect(
            client.Left + region.Left,
            client.Top + region.Top,
            region.Width,
            region.Height));
}

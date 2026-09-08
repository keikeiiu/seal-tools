using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using OpenCvSharp;
using OpenCvSharp.Extensions;
using SealTools.Core.Config;

namespace SealTools.Core;

// Two capture methods:
// - PrintWindow (Capture): grabs the WHOLE game window (incl. title bar). Used by the calibrator,
//   which needs the full window image to click points on.
// - CopyFromScreen (CaptureScreen): grabs a SCREEN region at absolute coords. Used by OCR and the
//   composer loop. The game is windowed (not exclusive-fullscreen), so GDI can read it.
// Both return an OpenCvSharp Mat (BGR).
public static class ScreenCapture
{
    private const int PwRenderFullContent = 0x00000002;

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, int nFlags);

    /// <summary>PrintWindow capture of the whole game window (frame-relative). Calibration only.</summary>
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

    /// <summary>GDI screen capture of an absolute screen region. OCR + composer loop.</summary>
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

    /// <summary>CopyFromScreen of a window-relative region (absolute coords = window origin + region).</summary>
    public static Mat CaptureScreenRegion(WindowRect client, RegionConfig region)
        => CaptureScreen(new WindowRect(
            client.Left + region.Left,
            client.Top + region.Top,
            region.Width,
            region.Height));
}

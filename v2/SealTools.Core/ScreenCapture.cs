using System.Drawing;
using System.Drawing.Imaging;
using OpenCvSharp;
using OpenCvSharp.Extensions;
using SealTools.Core.Config;

namespace SealTools.Core;

// GDI screen capture → OpenCvSharp Mat (BGR). Runs DPI-aware (physical pixels)
// once WindowFinder.EnablePerMonitorDpiAwareness() has been called.
public static class ScreenCapture
{
    public static Mat Capture(WindowRect rect)
    {
        using var bmp = new Bitmap(rect.Width, rect.Height, PixelFormat.Format24bppRgb);
        using (var g = Graphics.FromImage(bmp))
        {
            g.CopyFromScreen(rect.Left, rect.Top, 0, 0, new System.Drawing.Size(rect.Width, rect.Height), CopyPixelOperation.SourceCopy);
        }
        return BitmapConverter.ToMat(bmp);
    }

    // Capture a client-area-relative region (canonical coordinate model, plan §3).
    public static Mat CaptureRegion(WindowRect client, RegionConfig region)
    {
        var left = client.Left + region.Left;
        var top = client.Top + region.Top;
        return Capture(new WindowRect(left, top, region.Width, region.Height));
    }
}

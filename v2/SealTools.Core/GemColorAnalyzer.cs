using System;
using OpenCvSharp;
using SealTools.Core.Config;

namespace SealTools.Core;

// Colour-composition fingerprint of a region, used to tell whether the gem-composer
// RESULT box is EMPTY (no gem) or holds a gem. A result gem is bright and saturated; an
// empty slot is a dim/neutral surface. Instead of hardcoding colour constants, the EMPTY
// reference is sampled live during gem calibration (the user ensures the box is empty at
// that moment), and runtime frames are compared to that reference. When the distance is
// small the box is judged empty.

public sealed class ColorComposition
{
    public double MeanV { get; set; }             // mean value (brightness) 0..1
    public double MeanSat { get; set; }           // mean saturation 0..1
    public double ColoredFraction { get; set; }   // fraction of distinctly-coloured pixels 0..1
    public double MeanR { get; set; }             // 0..1
    public double MeanG { get; set; }             // 0..1
    public double MeanB { get; set; }             // 0..1
}

public static class GemColorAnalyzer
{
    // Capture a client-relative box (physical pixels) and analyse it. Null when the game window
    // can't be read.
    public static ColorComposition? Analyze(IntPtr hwnd, int x, int y, int w, int h, int coloredGapMin)
    {
        var cap = ScreenCapture.CaptureClientRegion(hwnd, new RegionConfig { Left = x, Top = y, Width = w, Height = h });
        if (cap == null) return null;
        using var mat = cap.Image;
        return Analyze(mat, coloredGapMin);
    }

    // Analyse a BGR image (24bpp, 3-channel). coloredGapMin is the channel max-min gap (0-255)
    // above which a pixel counts as "coloured" (config: gem.colored_gap_min).
    public static ColorComposition Analyze(Mat img, int coloredGapMin)
    {
        using var typed = new Mat<Vec3b>(img);
        var idx = typed.GetIndexer();
        double sumV = 0, sumSat = 0, sumR = 0, sumG = 0, sumB = 0, colored = 0;

        for (int y = 0; y < img.Height; y++)
            for (int x = 0; x < img.Width; x++)
            {
                var px = idx[y, x];
                int B = px.Item0, G = px.Item1, R = px.Item2;
                int v = Math.Max(R, Math.Max(G, B));
                int mn = Math.Min(R, Math.Min(G, B));
                int sat = v == 0 ? 0 : (v - mn) * 255 / v; // 0..255

                sumV += v / 255.0;
                sumSat += sat / 255.0;
                sumR += R / 255.0;
                sumG += G / 255.0;
                sumB += B / 255.0;
                if (v - mn >= coloredGapMin) colored++;
            }

        double n = (double)img.Height * img.Width;
        if (n <= 0) return new ColorComposition();
        return new ColorComposition
        {
            MeanV = sumV / n,
            MeanSat = sumSat / n,
            ColoredFraction = colored / n,
            MeanR = sumR / n,
            MeanG = sumG / n,
            MeanB = sumB / n,
        };
    }

    // Distance between two compositions: the EUCLIDEAN norm over the six features, not their mean.
    //
    // The mean was measured to be useless here (2026-09-10): a genuinely empty box and the same box
    // holding a gem differ by only 0.10 as a mean of six absolute differences, because the gem
    // covers part of the box and no single feature moves much on its own — so a box full of gem read
    // as "empty" and the composer advanced the grade. The same pair is 0.29 as a norm, which the
    // calibrated threshold (0.18) separates cleanly. The features are all 0..1, so the norm is
    // 0..sqrt(6); `gem.empty_distance` is on that scale.
    public static double Distance(ColorComposition a, ColorComposition b)
    {
        double dv = a.MeanV - b.MeanV;
        double ds = a.MeanSat - b.MeanSat;
        double df = a.ColoredFraction - b.ColoredFraction;
        double dr = a.MeanR - b.MeanR;
        double dg = a.MeanG - b.MeanG;
        double db = a.MeanB - b.MeanB;
        return Math.Sqrt(dv * dv + ds * ds + df * df + dr * dr + dg * dg + db * db);
    }

    // True when the frame is close enough to the sampled EMPTY reference. A null reference
    // means empty-detection hasn't been calibrated yet, so we conservatively say NOT empty.
    public static bool IsEmpty(ColorComposition frame, ColorComposition? emptyRef, double emptyDistance) =>
        emptyRef != null && Distance(frame, emptyRef) <= emptyDistance;

    // Fraction of pixels (0..1) that differ from the reference crop by more than channelTolerance
    // on ANY channel. 0 = pixel-identical; null when the images aren't the same size.
    //
    // This is the empty test's primary signal because it is colour- AND shape-blind: it asks "is
    // this still the same picture?" instead of "what colour is it", so a red, green or blue gem —
    // or a differently-shaped higher-grade gem — all read the same. Measured on the reference
    // machine (2026-09-10, 62x59 crop): an EMPTY box scores 0.000 (pixel-identical), a box holding
    // a gem scores 0.357. Anything from 0.05 to 0.30 separates them, so the calibrated threshold
    // (gem.empty_distance) has a wide margin on both sides.
    public static double? DiffFraction(Mat live, Mat reference, int channelTolerance)
    {
        if (live.Width != reference.Width || live.Height != reference.Height) return null;

        using var livePx = new Mat<Vec3b>(live);
        using var refPx = new Mat<Vec3b>(reference);
        var a = livePx.GetIndexer();
        var b = refPx.GetIndexer();
        int differing = 0;
        for (int y = 0; y < live.Height; y++)
            for (int x = 0; x < live.Width; x++)
            {
                var pa = a[y, x];
                var pb = b[y, x];
                int d = Math.Max(Math.Abs(pa.Item0 - pb.Item0),
                        Math.Max(Math.Abs(pa.Item1 - pb.Item1), Math.Abs(pa.Item2 - pb.Item2)));
                if (d > channelTolerance) differing++;
            }

        return differing / (double)(live.Width * live.Height);
    }
}

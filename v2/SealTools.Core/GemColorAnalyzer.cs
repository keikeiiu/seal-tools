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
    private const int ColoredGapMin = 40; // channel max-min gap above which a pixel is "coloured"

    // Capture a client-relative box inside the given client rect and analyse it.
    public static ColorComposition Analyze(IntPtr hwnd, WindowRect client, int x, int y, int w, int h)
    {
        using var mat = ScreenCapture.CaptureScreenRegion(client, new RegionConfig { Left = x, Top = y, Width = w, Height = h });
        return Analyze(mat);
    }

    // Analyse a BGR image (24bpp, 3-channel).
    public static ColorComposition Analyze(Mat img)
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
                if (v - mn >= ColoredGapMin) colored++;
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

    // Normalised distance between two compositions (0..1). Brightness, saturation and the
    // coloured fraction are the distinguishing signals for empty-vs-gem; the RGB means add
    // a coarse hue cue. Equal weight, mean of the per-metric absolute differences.
    public static double Distance(ColorComposition a, ColorComposition b) =>
        (Math.Abs(a.MeanV - b.MeanV) + Math.Abs(a.MeanSat - b.MeanSat) +
         Math.Abs(a.ColoredFraction - b.ColoredFraction) +
         Math.Abs(a.MeanR - b.MeanR) + Math.Abs(a.MeanG - b.MeanG) + Math.Abs(a.MeanB - b.MeanB)) / 6.0;

    // True when the frame is close enough to the sampled EMPTY reference. A null reference
    // means empty-detection hasn't been calibrated yet, so we conservatively say NOT empty.
    public static bool IsEmpty(ColorComposition frame, ColorComposition? emptyRef, double emptyDistance) =>
        emptyRef != null && Distance(frame, emptyRef) <= emptyDistance;
}

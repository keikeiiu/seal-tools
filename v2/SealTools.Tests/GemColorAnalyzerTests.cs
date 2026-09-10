using OpenCvSharp;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// Pins the empty-result-box discriminator. The numbers are the real ones measured on the reference
// machine (2026-09-10, 62x59 result-box crop): an EMPTY box is pixel-identical to the saved empty
// crop, a box holding a gem differs on ~36% of its pixels. Anything that makes the metric call a
// gem "empty" again is the bug this file exists to catch.
public class GemColorAnalyzerTests
{
    private static Mat Solid(int w, int h, byte b, byte g, byte r)
    {
        var mat = new Mat(h, w, MatType.CV_8UC3, new Scalar(b, g, r)); // Scalar is B,G,R
        return mat;
    }

    [Fact]
    public void IdenticalImagesGiveZeroDifference()
    {
        using var a = Solid(10, 8, 161, 219, 214);
        using var b = Solid(10, 8, 161, 219, 214);
        Assert.Equal(0.0, GemColorAnalyzer.DiffFraction(a, b, 30));
    }

    [Fact]
    public void WholeImageChangedIsOneHundredPercent()
    {
        using var a = Solid(10, 8, 161, 219, 214);   // pale empty box
        using var b = Solid(10, 8, 220, 150, 100);   // saturated gem colour
        Assert.Equal(1.0, GemColorAnalyzer.DiffFraction(a, b, 30));
    }

    [Fact]
    public void CountsOnlyPixelsPastTheTolerance()
    {
        // 4x4 = 16 px: 4 differ by 10 (below tolerance), 4 by 200 (above), 8 identical.
        using var a = Solid(4, 4, 100, 100, 100);
        using var b = Solid(4, 4, 100, 100, 100);
        b.Row(0).SetTo(new Scalar(110, 100, 100));   // +10 on B -> below tolerance
        b.Row(1).SetTo(new Scalar(100, 100, 255));   // +155 on R -> above tolerance

        Assert.Equal(4 / 16.0, GemColorAnalyzer.DiffFraction(a, b, 30));
    }

    [Fact]
    public void TheInsetExcludesTheBorderRowsFromTheComparison()
    {
        // 20x20: the reference's outer 2 rows differ everywhere a gem-like way, the interior is
        // identical. With inset 0 every pixel of those rows counts; with inset 2 none of them do.
        using var reference = Solid(20, 20, 100, 100, 100);
        using var live = Solid(20, 20, 100, 100, 100);
        live.Row(0).SetTo(new Scalar(0, 0, 255));
        live.Row(19).SetTo(new Scalar(0, 0, 255));

        Assert.Equal(40 / 400.0, GemColorAnalyzer.DiffFraction(live, reference, 30));
        Assert.Equal(0.0, GemColorAnalyzer.DiffFraction(live, reference, 30, inset: 2));
        // An inset larger than the image falls back to comparing everything rather than dividing by 0.
        Assert.Equal(40 / 400.0, GemColorAnalyzer.DiffFraction(live, reference, 30, inset: 10));
    }

    [Fact]
    public void DifferentSizesReturnNull()
    {
        using var a = Solid(10, 8, 0, 0, 0);
        using var b = Solid(9, 8, 0, 0, 0);
        Assert.Null(GemColorAnalyzer.DiffFraction(a, b, 30));
    }

    [Fact]
    public void DistanceIsEuclideanNotTheMeanThatHidAGem()
    {
        // The exact pair measured on the reference machine: the old mean-of-six gave 0.100, below
        // the threshold, so a box full of gem read as empty. The norm is 0.294.
        var empty = new ColorComposition
        {
            MeanV = 0.859564, MeanSat = 0.274034, ColoredFraction = 1.0,
            MeanR = 0.839423, MeanG = 0.859273, MeanB = 0.631923,
        };
        var gem = new ColorComposition
        {
            MeanV = 0.811451, MeanSat = 0.414936, ColoredFraction = 0.934937,
            MeanR = 0.640172, MeanG = 0.716273, MeanB = 0.638671,
        };

        var d = GemColorAnalyzer.Distance(empty, gem);
        Assert.InRange(d, 0.29, 0.30);
        Assert.False(GemColorAnalyzer.IsEmpty(gem, empty, 0.01));
        Assert.True(GemColorAnalyzer.IsEmpty(empty, empty, 0.01));
    }
}

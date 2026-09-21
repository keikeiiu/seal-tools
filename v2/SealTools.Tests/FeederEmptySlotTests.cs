using System;
using OpenCvSharp;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// Pins the test that tells an EMPTY feeder slot from a crop the reader failed on.
//
// Both arrive as "no digits", and they mean opposite things: one is a real reading of zero — the row is
// out of food and wants reloading now — and the other is a failure, which is answered by assuming a
// full load. Live on 2026-09-21 a dry row was scheduled 205 minutes into the future on the second
// reading, and its pet went unfed.
//
// The measured crops that produced these numbers, from the failed reads themselves:
//
//     empty       0.0 %            (both slots of the dry row)
//     has food   30.4 – 37.0 %     (every slot of a full one)
public class FeederEmptySlotTests
{
    // BGR. The bare cell is the game's beige; the food icon is ochre — high red, low blue.
    private static readonly Scalar Beige = new(210, 225, 235);
    private static readonly Scalar Ochre = new(80, 140, 190);

    private static Mat Cell() => new(40, 40, MatType.CV_8UC3, Beige);

    [Fact]
    public void ABareCellReadsAsEmpty()
    {
        using var cell = Cell();

        Assert.Equal(0, FeederCount.WarmFraction(cell), 6);
        Assert.True(FeederCount.WarmFraction(cell) < FeederCount.EmptySlotBelow);
    }

    /// <summary>A third of the cell covered by the icon is far above the threshold — the measured
    /// crops had 30 % and up, so this is the shape of the real thing rather than a marginal case.</summary>
    [Fact]
    public void ACellWithFoodInItDoesNot()
    {
        using var cell = Cell();
        Cv2.Rectangle(cell, new Rect(4, 4, 32, 12), Ochre, -1);   // inside the inset, so it survives it

        var warm = FeederCount.WarmFraction(cell);

        Assert.True(warm > FeederCount.EmptySlotBelow,
            $"expected food above the empty threshold, got {warm:0.###}");
    }

    /// <summary>The gap is an order of magnitude wide in both directions, which is the whole reason a
    /// single number can be trusted here: nothing measured sits near the line.</summary>
    [Fact]
    public void TheThresholdIsNotCloseToEitherMeasuredValue()
    {
        Assert.True(FeederCount.EmptySlotBelow > 0.0 * 3, "must sit above the measured empty value");
        Assert.True(FeederCount.EmptySlotBelow < 0.304 / 3, "must sit well below the measured food value");
    }

    /// <summary>An unusable image answers 0 — empty — which is the reading that gets the row FED. The
    /// other direction would leave it waiting, so the failure has to fall towards the safe side.</summary>
    [Fact]
    public void AnUnusableImageIsNotFood()
    {
        using var tiny = new Mat(4, 4, MatType.CV_8UC3, Beige);

        Assert.Equal(0, FeederCount.WarmFraction(tiny), 6);
        Assert.Equal(0, FeederCount.WarmFraction(new Mat()), 6);
    }
}

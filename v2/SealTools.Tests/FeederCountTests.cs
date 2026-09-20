using System.Collections.Generic;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// Reading a stack count off the boarding window. Everything here guards a failure that returns a
// PLAUSIBLE WRONG NUMBER rather than nothing — measured: a slightly-too-tight crop read "300" as "0"
// and "84" as "4", both at confidence a naive caller would accept. A wrong count schedules a pet to
// starve or overfeeds it, so the parsing and the agreement are worth pinning.
public class FeederCountTests
{
    // A slot 66 wide at (100,200) — the free row's measured size.
    private static List<int> Slot() => new() { 100, 200, 66, 56 };

    [Fact]
    public void TheCropTakesTheRightHandSideOfTheSlotAtFullHeight()
    {
        var crop = FeederCount.Crop(Slot(), FeederCount.CropLeft)!;

        Assert.Equal(4, crop.Count);
        Assert.Equal(200, crop[1]);          // top unchanged
        Assert.Equal(56, crop[3]);           // FULL height — half height finds no text at all, measured
        Assert.Equal(166, crop[0] + crop[2]); // right edge is the slot's
        Assert.True(crop[0] > 100, "the crop must start inside the slot, not at its left edge");
    }

    [Fact]
    public void TheSecondCropIsToTheRightOfTheFirst()
    {
        // The pair only detects clipping if they actually differ.
        var a = FeederCount.Crop(Slot(), FeederCount.CropLeft)!;
        var b = FeederCount.Crop(Slot(), FeederCount.CropLeftShifted)!;

        Assert.True(b[0] > a[0]);
        Assert.True(b[2] < a[2]);
    }

    [Fact]
    public void ABadSlotBoxIsNotACrop()
    {
        // A half-marked or zero-area row must produce no reading rather than a crop of nothing, which
        // would read as "0 items" and schedule a reload that is not needed.
        Assert.Null(FeederCount.Crop(null!, FeederCount.CropLeft));
        Assert.Null(FeederCount.Crop(new List<int>(), FeederCount.CropLeft));
        Assert.Null(FeederCount.Crop(new List<int> { 1, 2, 3 }, FeederCount.CropLeft));
        Assert.Null(FeederCount.Crop(new List<int> { 0, 0, 0, 0 }, FeederCount.CropLeft));
    }

    [Fact]
    public void TheBestScoringLineWinsNotTheFirst()
    {
        // Measured shape: the real count beside junk from the food icon, and the junk can come first.
        var lines = new List<(string, double)> { ("中", 0.13), ("300", 1.00), ("0", 0.56) };

        Assert.Equal(300, FeederCount.Parse(lines, FeederCount.MinScore));
    }

    [Fact]
    public void JunkBelowTheGateIsNotACount()
    {
        var lines = new List<(string, double)> { ("0", 0.56), ("中", 0.51) };

        Assert.Null(FeederCount.Parse(lines, FeederCount.MinScore));
    }

    [Theory]
    [InlineData("300", 1.00, 300)]
    [InlineData("0", 0.90, 0)]
    [InlineData("84", 0.99, 84)]
    [InlineData("7", 0.95, 7)]
    public void AReadIsTheDigitsOfTheLine(string text, double score, int expected)
    {
        Assert.Equal(expected, FeederCount.Parse(new List<(string, double)> { (text, score) },
            FeederCount.MinScore));
    }

    [Theory]
    [InlineData("1200")]    // four digits: a stack holds at most 300, so this is a misread
    [InlineData("999")]
    [InlineData("abc")]
    [InlineData("")]
    public void AnythingImplausibleIsRejectedRatherThanScheduled(string text)
    {
        Assert.Null(FeederCount.Parse(new List<(string, double)> { (text, 1.00) },
            FeederCount.MinScore));
    }

    [Fact]
    public void TwoReadsMustAgreeOrThereIsNoNumber()
    {
        Assert.Equal(300, FeederCount.Agreed(300, 300));
        Assert.Equal(0, FeederCount.Agreed(0, 0));
    }

    [Fact]
    public void AClipIsCaughtBecauseTheTwoCropsDisagree()
    {
        // This is the whole point of reading twice. "300" clipped on the left reads "00" or "0";
        // "84" clipped reads "4". Both are plausible integers, and only the disagreement gives
        // them away.
        Assert.Null(FeederCount.Agreed(0, 300));
        Assert.Null(FeederCount.Agreed(4, 84));
    }

    [Fact]
    public void OneFailedReadIsNoRead()
    {
        // Either crop failing means the other cannot be trusted either — a blank beside a number is
        // as likely to be a clipped crop as a clean one.
        Assert.Null(FeederCount.Agreed(null, 300));
        Assert.Null(FeederCount.Agreed(300, null));
        Assert.Null(FeederCount.Agreed(null, null));
    }

    [Fact]
    public void TheGateSitsInsideTheMeasuredGap()
    {
        // Real counts measured 0.90-1.00; everything the icon produced scored at most 0.63. The gate
        // has to be between them, or the junk that outscored one genuine edge-of-window read would be
        // accepted as a count.
        Assert.True(FeederCount.MinScore > 0.63);
        Assert.True(FeederCount.MinScore < 0.90);
    }
}

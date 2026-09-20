using System;
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
    public void TheGateSitsInsideTheMeasuredGap()
    {
        // Real counts measured 0.90-1.00; everything the icon produced scored at most 0.63. The gate
        // has to be between them, or the junk that outscored one genuine edge-of-window read would be
        // accepted as a count.
        Assert.True(FeederCount.MinScore > 0.63);
        Assert.True(FeederCount.MinScore < 0.90);
    }
}

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
    public void TheValueTwoReadingsAgreeOnWins()
    {
        Assert.Equal(300, FeederCount.Vote(new int?[] { 300, 300, null }));
    }

    [Fact]
    public void ASingleReadingIsNotEnoughToCarryIt()
    {
        // One confident wrong answer must not win alone. A CLIPPED crop scores as highly as a correct
        // one — a clipped "138" came back as "3" at 0.99 — so confidence cannot break the tie and the
        // count has to.
        Assert.Null(FeederCount.Vote(new int?[] { 300, null, null, null }));
        Assert.Null(FeederCount.Vote(new int?[] { 3 }));
    }

    [Fact]
    public void TheMajorityWinsOverAMinorityOfWrongReads()
    {
        // The measured shape: a couple of offsets clip or swallow the icon and return junk, while the
        // ones that land in the band agree. The gate has already dropped the junk; this is the vote.
        Assert.Equal(138, FeederCount.Vote(new int?[] { 138, 138, null, 851 }));
    }

    [Fact]
    public void ATieIsNoReadingRatherThanAGuess()
    {
        // Two against two resolves to nothing on purpose: "no reading" makes the caller fall back to
        // the fixed cycle it used before any of this existed, while a wrong number feeds the pet
        // wrongly.
        Assert.Null(FeederCount.Vote(new int?[] { 138, 138, 300, 300 }));
    }

    [Fact]
    public void NothingReadIsNothing()
    {
        Assert.Null(FeederCount.Vote(new int?[] { null, null, null, null }));
        Assert.Null(FeederCount.Vote(Array.Empty<int?>()));
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

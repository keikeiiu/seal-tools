using System.Collections.Generic;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

/// <summary>The gem composer's per-grade cycle tally. Pure, so it is tested here rather than through the
/// tool — running the composer needs a serial port, a game window and a calibration.</summary>
public class GradeCyclesTests
{
    private static readonly List<string> Grades = new() { "N", "G", "DG" };

    [Fact]
    public void EveryCycleIsCountedAgainstTheGradeItRanOn()
    {
        var cycles = new GradeCycles();

        cycles.Add("N");
        cycles.Add("N");
        cycles.Add("G");
        cycles.Add("N");

        Assert.Equal(3, cycles.CountFor("N"));
        Assert.Equal(1, cycles.CountFor("G"));
        Assert.Equal(0, cycles.CountFor("DG"));
    }

    [Fact]
    public void ANewRunStartsFromZero()
    {
        var cycles = new GradeCycles();
        cycles.Add("N");
        cycles.Add("G");

        cycles.Reset();

        Assert.Equal(0, cycles.CountFor("N"));
        Assert.Equal(0, cycles.CountFor("G"));
    }

    [Fact]
    public void TheCardShowsARowPerGradeInRunOrder()
    {
        // Every grade, including one not reached: a list that grows as the run proceeds moves under the
        // reader's eye, and "DG: 0 cycles" is itself information.
        var cycles = new GradeCycles();
        cycles.Add("N");
        cycles.Add("N");
        cycles.Add("G");

        var rows = cycles.Rows(Grades);

        Assert.Equal(3, rows.Count);
        Assert.Equal("N", rows[0].Name);
        Assert.Equal("2 cycles", rows[0].Status);
        Assert.Equal("1 cycle", rows[1].Status);      // singular, because "1 cycles" reads as a bug
        Assert.Equal("0 cycles", rows[2].Status);
    }

    [Fact]
    public void TheSummaryLineCarriesEveryGradeAndTheTotal()
    {
        var cycles = new GradeCycles();
        for (var i = 0; i < 12; i++) cycles.Add("N");
        for (var i = 0; i < 8; i++) cycles.Add("G");
        for (var i = 0; i < 3; i++) cycles.Add("DG");

        Assert.Equal("N 12 · G 8 · DG 3 · 23 total", cycles.Summary(Grades));
    }

    [Fact]
    public void AGradeTheRunNeverReachedStillAppearsAtZero()
    {
        var cycles = new GradeCycles();
        cycles.Add("N");

        Assert.Equal("N 1 · G 0 · DG 0 · 1 total", cycles.Summary(Grades));
    }

    [Fact]
    public void TheTotalComesFromTheGradesSoTheTwoCannotDisagree()
    {
        var cycles = new GradeCycles();
        cycles.Add("N");
        cycles.Add("DG");

        Assert.Equal(2, cycles.Total(Grades));
    }
}

using System;
using System.Collections.Generic;
using System.Linq;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// Deriving a row's food slots and the count region inside them, from six drags instead of twenty.
// The arithmetic is the whole feature: it replaces fourteen hand-drawn boxes AND the two fractions that
// were measured on one machine and assumed to be resolution-independent everywhere else.
public class FeederLayoutTests
{
    // Row 2's real strip: its five slots measured 349, 411, 475, 541 and 603 across, ~64px wide at
    // y=456, height ~57. Dragged as one strip, that is x 349 to 666.
    private static List<int> Strip() => new() { 349, 454, 317, 58 };

    [Fact]
    public void AStripDividesIntoThisRowsStackCount()
    {
        Assert.Equal(2, FeederLayout.SlotBoxes(Strip(), 2).Count);
        Assert.Equal(5, FeederLayout.SlotBoxes(Strip(), 5).Count);
    }

    [Fact]
    public void TheSlotsCoverTheStripExactly()
    {
        // No gap and no overlap: a sliver left unclaimed at the right end is precisely where the count
        // is read from, so the segments must add up to the strip.
        var slots = FeederLayout.SlotBoxes(Strip(), 5);

        Assert.Equal(349, slots[0][0]);
        Assert.Equal(666, slots[^1][0] + slots[^1][2]);
        foreach (var s in slots) Assert.Equal(454, s[1]);          // same top
        foreach (var s in slots) Assert.Equal(58, s[3]);           // same height
    }

    [Fact]
    public void TheDivisionsAreEvenAndInOrder()
    {
        var slots = FeederLayout.SlotBoxes(Strip(), 5);

        for (int i = 1; i < slots.Count; i++)
        {
            // Contiguous, and never inverted.
            Assert.Equal(slots[i - 1][0] + slots[i - 1][2], slots[i][0]);
            Assert.True(slots[i][2] > 0);
        }

        // Every slot the same width, to within the one pixel integer division can cost.
        var widths = slots.Select(s => s[2]).Distinct().ToList();
        Assert.True(widths.Max() - widths.Min() <= 1, $"uneven slots: {string.Join(",", widths)}");
    }

    [Fact]
    public void AUsableStripIsRequired()
    {
        // A half-calibrated row must yield NO slots, not a row of boxes at (0,0) that the tool would
        // then click and read.
        Assert.Null(FeederLayout.SlotBoxes(null, 5));
        Assert.Null(FeederLayout.SlotBoxes(new List<int>(), 5));
        Assert.Null(FeederLayout.SlotBoxes(new List<int> { 1, 2, 3 }, 5));
        Assert.Null(FeederLayout.SlotBoxes(new List<int> { 0, 0, 0, 0 }, 5));
    }

    [Theory]
    [InlineData(0)]
    [InlineData(-1)]
    public void AUsableStackCountIsRequired(int stacks)
    {
        Assert.Null(FeederLayout.SlotBoxes(Strip(), stacks));
    }

    [Fact]
    public void TheCountRegionIsTheDrawnBoxMovedOntoTheSlot()
    {
        // The count box is read AS DRAWN on the slot it was drawn on, and moved for every other. The
        // offset is a measured position — where you put the box relative to a slot you drew — so it
        // survives a machine where the digits sit differently inside the slot, which no fraction can.
        var drawnOn = new List<int> { 344, 260, 66, 55 };
        var countBox = new List<int> { 368, 293, 43, 22 };
        var other = new List<int> { 410, 260, 66, 55 };

        var region = FeederLayout.CountRegion(other, drawnOn, countBox)!;

        Assert.Equal(410 + (368 - 344), region[0]);
        Assert.Equal(260 + (293 - 260), region[1]);
        Assert.Equal(43, region[2]);
        Assert.Equal(22, region[3]);
    }

    [Fact]
    public void TheDrawnSlotReadsBackExactlyWhatWasDrawn()
    {
        // The identity case, and the one that says the offset is an offset: deriving the region for the
        // slot the box was drawn on must give back the box itself.
        var drawnOn = new List<int> { 344, 260, 66, 55 };
        var countBox = new List<int> { 368, 293, 43, 22 };

        Assert.Equal(countBox, FeederLayout.CountRegion(drawnOn, drawnOn, countBox));
    }

    [Fact]
    public void AMissingBoxIsNoRegion()
    {
        var slot = new List<int> { 344, 260, 66, 55 };

        Assert.Null(FeederLayout.CountRegion(slot, null, new List<int> { 1, 2, 3, 4 }));
        Assert.Null(FeederLayout.CountRegion(slot, new List<int> { 1, 2, 3, 4 }, null));
        Assert.Null(FeederLayout.CountRegion(null, new List<int> { 1, 2, 3, 4 },
            new List<int> { 1, 2, 3, 4 }));
    }

    [Fact]
    public void TheNearestSlotFindsWhereTheBoxWasDrawn()
    {
        // The calibration stores boxes, not identities: nothing says which slot the count box belongs
        // to, and the offset only means anything against the right one.
        var slots = new List<List<int>>
        {
            new() { 344, 260, 66, 55 },
            new() { 410, 260, 66, 55 },
        };

        Assert.Equal(slots[0], FeederLayout.NearestSlot(slots, new List<int> { 368, 293, 43, 22 }));
        Assert.Equal(slots[1], FeederLayout.NearestSlot(slots, new List<int> { 430, 295, 43, 22 }));
        Assert.Null(FeederLayout.NearestSlot(slots, null));
        Assert.Null(FeederLayout.NearestSlot(new List<List<int>>(), new List<int> { 1, 2, 3, 4 }));
    }
}

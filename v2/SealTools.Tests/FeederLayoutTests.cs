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
    public void TheReadRegionRunsFromTheFractionToTheSlotsRightEdge()
    {
        var slot = new List<int> { 344, 260, 66, 55 };

        var region = FeederLayout.ReadRegion(slot, FeederLayout.ReadLeftFraction)!;

        Assert.Equal(344 + (int)Math.Round(66 * 0.40), region[0]);   // 40% across
        Assert.Equal(410, region[0] + region[2]);                    // the slot's own right edge
        Assert.Equal(260, region[1]);                                // the slot's top
        Assert.Equal(55, region[3]);                                 // and FULL HEIGHT
    }

    [Fact]
    public void TheReadRegionIsFullHeightBecauseAShortOneReadsNothing()
    {
        // The most expensive measurement in this feature, and unexplained: a crop holding a perfectly
        // legible number found ZERO detected boxes at digit height inside a slot — "174", "36" and "18",
        // three separate crops, all plainly readable to the eye — and read at 1.00 at the slot's full
        // height. Whatever the mechanism, the height comes from the slot and never from a drawn box.
        var slot = new List<int> { 344, 260, 66, 55 };

        Assert.Equal(slot[3], FeederLayout.ReadRegion(slot, 0.40)![3]);
    }

    [Fact]
    public void TheFractionIsASettingSoItHasToMoveTheRegion()
    {
        var slot = new List<int> { 344, 260, 66, 55 };

        var narrow = FeederLayout.ReadRegion(slot, 0.55)!;
        var wide = FeederLayout.ReadRegion(slot, 0.25)!;

        Assert.True(narrow[0] > wide[0], "a larger fraction must start further right");
        Assert.Equal(slot[0] + slot[2], narrow[0] + narrow[2]);
        Assert.Equal(slot[0] + slot[2], wide[0] + wide[2]);
    }

    [Fact]
    public void ABadSlotIsNotAReadRegion()
    {
        Assert.Null(FeederLayout.ReadRegion(null, 0.40));
        Assert.Null(FeederLayout.ReadRegion(new List<int> { 1, 2, 3 }, 0.40));
        Assert.Null(FeederLayout.ReadRegion(new List<int> { 0, 0, 0, 0 }, 0.40));
    }
}

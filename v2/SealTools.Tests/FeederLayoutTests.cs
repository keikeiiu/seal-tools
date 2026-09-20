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
    public void TheCountRegionIsTheReferenceOffsetAppliedToTheSlot()
    {
        // The reference slot at 349 and its count text at 371 — 22px in, 20px down from the top.
        var refSlot = new List<int> { 349, 454, 64, 57 };
        var refText = new List<int> { 371, 474, 40, 30 };
        var other = new List<int> { 541, 454, 64, 57 };

        var region = FeederLayout.CountRegion(other, refSlot, refText)!;

        Assert.Equal(541 + 22, region[0]);
        Assert.Equal(474, region[1]);
        Assert.Equal(40, region[2]);
        Assert.Equal(30, region[3]);
    }

    [Fact]
    public void TheReferenceSlotReadsTheSameAsItself()
    {
        // The identity case, and it is the one that proves the offset is an offset: deriving the
        // reference slot's own region must give back the box the player drew.
        var refSlot = new List<int> { 349, 454, 64, 57 };
        var refText = new List<int> { 371, 474, 40, 30 };

        Assert.Equal(refText, FeederLayout.CountRegion(refSlot, refSlot, refText));
    }

    [Fact]
    public void AMissingReferenceYieldsNoRegion()
    {
        // No reference drawn yet is the state every existing config is in, and it must read as
        // "nothing to read" rather than as a region at (0,0).
        var slot = new List<int> { 349, 454, 64, 57 };

        Assert.Null(FeederLayout.CountRegion(slot, null, new List<int> { 1, 2, 3, 4 }));
        Assert.Null(FeederLayout.CountRegion(slot, new List<int> { 1, 2, 3, 4 }, null));
        Assert.Null(FeederLayout.CountRegion(null, new List<int> { 1, 2, 3, 4 }, new List<int> { 1, 2, 3, 4 }));
    }

    [Fact]
    public void TheSecondReadIsTheFirstShiftedRight()
    {
        var region = new List<int> { 371, 474, 40, 30 };

        var shifted = FeederLayout.Shift(region, 3);

        Assert.Equal(374, shifted[0]);
        Assert.Equal(region[1], shifted[1]);
        Assert.Equal(region[2], shifted[2]);
        Assert.Equal(region[3], shifted[3]);
    }
}

using System;
using System.Collections.Generic;
using System.Linq;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// The bag grid is derived from two dragged rectangles rather than 64 pinned points, so the arithmetic
// is the part that has to be right — a pitch that drifts by a pixel misses the last column entirely.
// These pin the derivation and the cross-check between the two rectangles.
public class BagGridTests
{
    // A grid whose pitch comes out at a clean 51 px, matching what the live capture measured.
    private static List<int> Grid() => new() { 100, 200, 408, 408 };

    [Fact]
    public void CentresAreRowMajorWithTheFirstAtTheTopLeft()
    {
        var centres = BagGrid.Centres(Grid());

        Assert.Equal(64, BagGrid.SlotCount);
        Assert.Equal(64, centres.Count);

        // Slot 0 is the top-left cell's centre: half a pitch in from the grid's origin.
        Assert.InRange(centres[0].X, 125, 127);
        Assert.InRange(centres[0].Y, 225, 227);

        // Slot 63 is the bottom-right one, half a pitch in from the far corner.
        Assert.InRange(centres[63].X, 481, 483);
        Assert.InRange(centres[63].Y, 481 + 100, 483 + 100);

        // Reading order, not column order: slot 1 is to the right of slot 0, slot 8 is below it.
        Assert.True(centres[1].X > centres[0].X);
        Assert.Equal(centres[0].Y, centres[1].Y);
        Assert.Equal(centres[0].X, centres[8].X);
        Assert.True(centres[8].Y > centres[0].Y);
    }

    [Fact]
    public void AdjacentCentresAreOnePitchApartEverywhere()
    {
        var centres = BagGrid.Centres(Grid());
        double px = BagGrid.PitchX(Grid()), py = BagGrid.PitchY(Grid());

        // The whole design assumes uniformity, so the derived points must stay on the pitch across
        // all eight columns and rows — not just near the origin.
        for (int row = 0; row < BagGrid.Rows; row++)
        {
            for (int col = 1; col < BagGrid.Cols; col++)
            {
                var a = centres[row * BagGrid.Cols + col - 1];
                var b = centres[row * BagGrid.Cols + col];
                Assert.InRange(Math.Abs((b.X - a.X) - px), 0, 1);
            }
        }
        for (int col = 0; col < BagGrid.Cols; col++)
        {
            for (int row = 1; row < BagGrid.Rows; row++)
            {
                var a = centres[(row - 1) * BagGrid.Cols + col];
                var b = centres[row * BagGrid.Cols + col];
                Assert.InRange(Math.Abs((b.Y - a.Y) - py), 0, 1);
            }
        }
    }

    [Fact]
    public void PitchIsTheGridDividedByEight()
    {
        Assert.Equal(51.0, BagGrid.PitchX(Grid()));
        Assert.Equal(51.0, BagGrid.PitchY(Grid()));
    }

    // The single slot box exists only to answer "is the grid actually uniform?". It is not a second
    // source of truth, so the two agreeing must be silent and disagreeing must be loud.
    [Fact]
    public void ASlotThatMatchesThePitchIsAccepted()
    {
        Assert.Null(BagGrid.Disagreement(Grid(), new List<int> { 110, 210, 51, 51 }));
    }

    // The case that shipped broken. A slot is naturally SMALLER than the pitch, because the pitch is
    // centre-to-centre and counts the gap between slots while the slot box measures the slot itself.
    // An earlier exact-match tolerance read a perfectly good 47px slot as 4px wrong and refused to
    // save, which is worse than no check at all.
    [Fact]
    public void ASlotSmallerThanThePitchByAGapIsAccepted()
    {
        Assert.Null(BagGrid.Disagreement(Grid(), new List<int> { 110, 210, 47, 47 }));
        Assert.Equal(4.0, BagGrid.ImpliedGap(Grid(), new List<int> { 110, 210, 47, 47 }));
    }

    [Fact]
    public void AGrosslyWrongSlotIsReported()
    {
        // Two slots dragged as one, or the icon only — the kind of error the check is actually for.
        var problem = BagGrid.Disagreement(Grid(), new List<int> { 110, 210, 95, 20 });

        Assert.NotNull(problem);
        Assert.Contains("95x20", problem);
        Assert.Contains("per slot", problem);
    }

    // The calibrator holds a half-dragged or empty rectangle at any moment, so anything that is not
    // four positive numbers has to be refused rather than quietly producing 64 nonsense centres.
    [Fact]
    public void InvalidRectanglesAreRejected()
    {
        var invalid = new List<int>?[]
        {
            null,
            new List<int>(),
            new List<int> { 1, 2, 3 },
            new List<int> { 1, 2, 0, 50 },    // zero width
            new List<int> { 1, 2, 50, -5 },   // negative height
        };

        foreach (var rect in invalid)
        {
            Assert.False(BagGrid.IsValidRect(rect), $"should be invalid: {rect?.Count} values");
            Assert.Empty(BagGrid.Centres(rect ?? new List<int>()));
        }
    }

    // A half-set calibration is the normal state while dragging, so it must produce a message rather
    // than an exception or an empty overlay.
    [Fact]
    public void DisagreementCopesWithAnIncompleteCalibration()
    {
        Assert.NotNull(BagGrid.Disagreement(new List<int>(), new List<int> { 1, 2, 30, 30 }));
        Assert.NotNull(BagGrid.Disagreement(Grid(), new List<int>()));
    }
}

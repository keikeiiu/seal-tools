using System.Collections.Generic;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// A buy preset says "row N", and the row's position is the first marked row plus N pitches. The
// pitch comes from two adjacent marks rather than an assumed row height, because nine rows of drift
// would put the click on a different item — and buying the wrong thing costs gold.
public class ShopGeometryTests
{
    private static List<int> First() => new() { 400, 300 };
    private static List<int> Second() => new() { 400, 347 };

    [Fact]
    public void PitchComesFromTheTwoMarks()
    {
        Assert.Equal(47, ShopGeometry.RowPitch(First(), Second()));
    }

    [Fact]
    public void RowZeroIsTheFirstMark()
    {
        var c = ShopGeometry.RowCentre(First(), Second(), 0);

        Assert.NotNull(c);
        Assert.Equal(400, c!.Value.X);
        Assert.Equal(300, c.Value.Y);
    }

    [Fact]
    public void EachRowIsOnePitchFurtherDown()
    {
        // Row 9 is the last of the ten visible rows in the captured shop list — it must stay on the
        // pitch, not merely near it, or the click lands between two items.
        var c = ShopGeometry.RowCentre(First(), Second(), 9);

        Assert.NotNull(c);
        Assert.Equal(300 + 9 * 47, c!.Value.Y);
    }

    // A second mark above the first is a mis-click, not a pitch of zero. Returning 0 keeps it out of
    // the arithmetic; Problem() is what tells the user.
    [Fact]
    public void ASecondMarkAboveTheFirstIsNotUsable()
    {
        var above = new List<int> { 400, 250 };

        Assert.Equal(0, ShopGeometry.RowPitch(First(), above));
        Assert.Null(ShopGeometry.RowCentre(First(), above, 3));
        Assert.NotNull(ShopGeometry.Problem(First(), above));
    }

    [Fact]
    public void HalfMarkedRowsAreRefusedNotGuessed()
    {
        Assert.NotNull(ShopGeometry.Problem(First(), null));
        Assert.NotNull(ShopGeometry.Problem(null, Second()));
    }

    [Fact]
    public void FullyMarkedRowsReportNoProblem()
    {
        Assert.Null(ShopGeometry.Problem(First(), Second()));
        Assert.Null(ShopGeometry.Problem(new List<int> { 1, 2 }, new List<int> { 1, 30 }));
    }

    [Fact]
    public void ANegativeRowIsRefused()
    {
        Assert.Null(ShopGeometry.RowCentre(First(), Second(), -1));
    }
}

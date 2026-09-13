using System.Collections.Generic;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// A buy preset says "row N". Row positions are derived from one dragged region rather than marked by
// hand, because two hand-clicked rows can disagree with each other and a single region cannot. The
// arithmetic is therefore the part that has to be right — it is what the whole buy path clicks
// against, and a pitch that drifts puts the click on the wrong item.
public class ShopGeometryTests
{
    // 10 rows in a 470px tall region gives a clean 47px pitch.
    private static List<int> Region() => new() { 100, 300, 400, 470 };

    [Fact]
    public void PitchIsTheRegionDividedByTheRowCount()
    {
        Assert.Equal(47.0, ShopGeometry.RowPitch(Region(), 10));
        Assert.Equal(10, ShopGeometry.DefaultRows);
    }

    [Fact]
    public void RowZeroIsHalfAPitchDownFromTheTop()
    {
        var c = ShopGeometry.RowCentre(Region(), 10, 0);

        Assert.NotNull(c);
        Assert.Equal(100 + 400 / 2, c!.Value.X);   // horizontally centred in the region
        Assert.InRange(c.Value.Y, 323, 324);        // half of 47 is 23.5; either rounding is fine
    }

    [Fact]
    public void TheLastRowStaysOnThePitch()
    {
        // The row that matters most: nine pitches down, still on the region rather than past it.
        var c = ShopGeometry.RowCentre(Region(), 10, 9);

        Assert.NotNull(c);
        Assert.Equal(300 + (int)System.Math.Round(9.5 * 47), c!.Value.Y);
        Assert.True(c.Value.Y < 300 + 470);
    }

    [Fact]
    public void EveryRowIsOnePitchApart()
    {
        var centres = ShopGeometry.RowCentres(Region(), 10);

        Assert.Equal(10, centres.Count);
        for (int i = 1; i < centres.Count; i++)
            Assert.InRange(centres[i].Y - centres[i - 1].Y, 46, 48);
    }

    [Fact]
    public void ADifferentRowCountChangesThePitch()
    {
        // The row count is a property of the game, not of the setup — if a game update changes it,
        // every row moves, which is exactly why it is config rather than a constant.
        Assert.Equal(470 / 10.0, ShopGeometry.RowPitch(Region(), 10));
        Assert.Equal(470 / 5.0, ShopGeometry.RowPitch(Region(), 5));
    }

    [Fact]
    public void AnUndrawnRegionIsRefusedNotGuessed()
    {
        Assert.NotNull(ShopGeometry.Problem(null, 10));
        Assert.NotNull(ShopGeometry.Problem(new List<int>(), 10));
        Assert.NotNull(ShopGeometry.Problem(Region(), 0));
        Assert.Null(ShopGeometry.Problem(Region(), 10));

        Assert.Null(ShopGeometry.RowCentre(null, 10, 0));
        Assert.Empty(ShopGeometry.RowCentres(null, 10));
    }

    [Fact]
    public void ANegativeRowIsRefused()
    {
        Assert.Null(ShopGeometry.RowCentre(Region(), 10, -1));
    }
}

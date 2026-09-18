using System.Collections.Generic;
using OpenCvSharp;
using SealTools.Core;
using Xunit;

namespace SealTools.Tests;

// Pins the offset search in the icon matcher — the thing that turned "3 of my 7 pets" into 6.
//
// MEASURED, on the player's own bag (2026-09-19). Seven pets, two crops, and the crops found three:
// the pets of the same kind scored 0.43 to 0.78 against each other while every other item in the bag
// sat at 0.83 or more. Nothing was wrong with the crops. **The pets' sprites are drawn at different
// SUB-CELL OFFSETS**, so a pixel-exact comparison only matched the cells where the sprite happened to
// land in the same place — and the seven pets were spread across several positions.
//
// Shrinking the compared region to the middle of the cell was tried first, because it is the obvious
// answer to "the neighbours leak in". It made things strictly WORSE: every pet scored 0.84 or more.
// The offset is a shift of the whole image, and cropping harder does not chase a shift.
//
// These tests are synthetic — a marker drawn at a known offset — because the property being pinned is
// about offsets, not about that particular bag.
public class IconMatchTests
{
    private static readonly List<int> Grid = new() { 0, 0, 200, 200 };   // 8x8 of 25px cells

    private const int Cell = 25;
    private const int MaxShift = 8;

    /// <summary>A patch larger than a cell, with structure plus a marker block.
    ///
    /// The structure is the point, and it took a wrong test to notice: a marker on a FLAT background
    /// matches at any offset, because moving it leaves almost every pixel the same colour. The blocks
    /// are five pixels and odd, so no shift within the search window can realign the pattern and
    /// disguise itself as a match.</summary>
    private static Mat Pattern()
    {
        var size = Cell + 2 * MaxShift;
        var m = new Mat(size, size, MatType.CV_8UC3, new Scalar(40, 40, 40));
        for (int y = 0; y < size; y += 5)
            for (int x = 0; x < size; x += 5)
                if (((x / 5) + (y / 5)) % 2 == 0)
                    Cv2.Rectangle(m, new Rect(x, y, 5, 5), new Scalar(95, 95, 95), -1);

        Cv2.Rectangle(m, new Rect(MaxShift + 5, MaxShift + 5, Cell / 4, Cell / 4),
            new Scalar(230, 120, 60), -1);
        return m;
    }

    /// <summary>A cell-sized window into the patch, taken (dx, dy) off centre.
    ///
    /// This is how a translation is done here, and the two attempts before it are worth recording.
    /// Shifting only the MARKER tests the wrong thing — the marker and the pattern then move
    /// independently, no window offset can align both, and a shift the matcher should absorb scored
    /// 0.09. Warping the whole patch with an affine transform scored 0.57, which is worse than not
    /// trying; sampling a window out of a larger image is the same translation with no interpolation
    /// to argue about.
    ///
    /// The whole cell's contents moving together is the real case: that is what the game does to a
    /// pet's sprite, and it is exactly what a window offset can undo.</summary>
    private static Mat Window(Mat pattern, int dx, int dy)
    {
        using var w = new Mat(pattern, new Rect(MaxShift - dx, MaxShift - dy, Cell, Cell));
        return w.Clone();
    }

    /// <summary>A 200x200 bag with <paramref name="contents"/> sitting in one of its 25px cells,
    /// aligned so that offset (0,0) means the contents are where the matcher expects them.</summary>
    private static Mat BagWithCell(int cell, Mat contents)
    {
        var bag = new Mat(200, 200, MatType.CV_8UC3, new Scalar(10, 10, 10));
        var (cx, cy) = BagGrid.Centres(Grid)[cell];
        using var cellImg = new Mat(bag, new Rect(cx - Cell / 2, cy - Cell / 2, Cell, Cell));
        contents.CopyTo(cellImg);
        return bag;
    }

    /// <summary>The property the whole change exists for: the same icon in a cell, shifted by a few
    /// pixels, must still be found. This is the case that lost four pets.</summary>
    [Fact]
    public void AnIconShiftedWithinItsCellIsStillFound()
    {
        using var pattern = Pattern();
        using var icon = Window(pattern, 0, 0);

        foreach (var (dx, dy) in new[] { (0, 0), (2, 0), (0, 2), (3, 3), (-3, 0), (0, -3) })
        {
            using var shifted = Window(pattern, dx, dy);

            using var bag = BagWithCell(17, shifted);
            var all = IconMatch.ScoreAll(bag, Grid, icon);

            Assert.NotEmpty(all);
            Assert.Equal(17, all[0].Cell);
            Assert.True(all[0].Score < 0.05,
                $"a shift of ({dx},{dy}) should still match; it scored {all[0].Score:0.###}");
        }
    }

    /// <summary>And a shift it should NOT tolerate. If the search window were unbounded every cell
    /// would eventually match something, which is a tool that right-clicks anything.</summary>
    [Fact]
    public void AnIconShiftedBeyondTheWindowDoesNotMatch()
    {
        using var pattern = Pattern();
        using var icon = Window(pattern, 0, 0);
        using var shifted = Window(pattern, IconMatch.SearchRadius + 3, 0);

        using var bag = BagWithCell(17, shifted);
        var all = IconMatch.ScoreAll(bag, Grid, icon);

        // The marker has left the frame entirely, so the cell cannot read as the same image.
        Assert.True(all.Count == 0 || all[0].Score > 0.05,
            $"a shift past the window must not match; it scored {(all.Count == 0 ? 1 : all[0].Score):0.###}");
    }

    /// <summary>A wrong item must not be dragged in by the wider search. The offset window widens what
    /// counts as a match, and this is the line it must not cross.</summary>
    [Fact]
    public void AWrongItemIsStillRejected()
    {
        using var pattern = Pattern();
        using var icon = Window(pattern, 0, 0);

        using var other = Window(pattern, 0, 0);
        Cv2.Circle(other, new Point(12, 12), 9, new Scalar(60, 220, 90), -1);

        using var bag = BagWithCell(17, other);
        var all = IconMatch.ScoreAll(bag, Grid, icon);

        Assert.NotEmpty(all);
        Assert.True(all[0].Score > 0.5,
            $"an unrelated icon should stay far away; it scored {all[0].Score:0.###}");
    }
}

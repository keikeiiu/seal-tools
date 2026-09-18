using System;
using System.Collections.Generic;
using OpenCvSharp;

namespace SealTools.Core;

// Finding an item's icon inside a calibrated grid — how the pet flow locates the pet it has to
// re-board.
//
// This is deliberately NOT OCR. The project has no bag OCR, and the Sell flow's answer to "which slot
// holds the thing I want" is to make the player point at it. The pet is the one case where that
// cannot work: ending boarding drops it into the FIRST FREE SLOT, which is a function of whatever
// else the bag holds at that instant — and the character is farming throughout, so the bag is never
// still. The pet has to be FOUND, not pointed at.
//
// The mechanism is the composer's empty-box check: compare a crop against a region by the FRACTION of
// pixels that differ, with an inset. Reusing it is deliberate — it is already trusted and already
// tuned, and one comparison routine means one place to fix.
public static class IconMatch
{
    /// <summary>The default inset. Not optional and not decoration: a one-pixel window shift once
    /// made an EMPTY box score 7.7 % different from itself, because the border pixels moved against
    /// the frame behind them. Six pixels of edge is what that measurement bought.</summary>
    public const int DefaultInset = 6;

    /// <summary>Per-channel difference (0-255) below which two pixels count as the same. Small but
    /// not zero: the game re-renders its icons, so an identical item is not bit-identical.</summary>
    public const int DefaultTolerance = 24;

    /// <summary>Fraction of pixels that differ by more than <paramref name="tolerance"/>, after
    /// insetting both. 0 = alike, 1 = nothing in common. Lower is better, so this reads as a
    /// DISTANCE despite the name.</summary>
    public static double DifferingFraction(Mat a, Mat b, int inset = DefaultInset,
        int tolerance = DefaultTolerance)
    {
        if (a.Empty() || b.Empty()) return 1.0;

        using var ca = Inset(a, inset);
        using var cb = Inset(b, inset);
        if (ca.Empty() || cb.Empty()) return 1.0;

        // Sizes differ because the saved crop came from wherever it was taken and the cell is a
        // derived size. Resizing the CROP down to the cell is the right direction: the cell is the
        // thing that is always the same.
        Mat other = cb;
        Mat? resized = null;
        if (ca.Size() != cb.Size())
        {
            resized = new Mat();
            Cv2.Resize(cb, resized, ca.Size(), 0, 0, InterpolationFlags.Area);
            other = resized;
        }

        try
        {
            using var diff = new Mat();
            Cv2.Absdiff(ca, other, diff);

            // Collapse to one channel by MAX, not by averaging to grey: a hue change that leaves
            // brightness alone is exactly the case that separates two similar items, and a grey
            // conversion would average it away.
            using var worst = new Mat();
            var split = diff.Split();
            try
            {
                split[0].CopyTo(worst);
                for (int i = 1; i < split.Length; i++)
                    Cv2.Max(worst, split[i], worst);
            }
            finally
            {
                foreach (var channel in split) channel.Dispose();
            }

            using var mask = new Mat();
            Cv2.Threshold(worst, mask, tolerance, 255, ThresholdTypes.Binary);
            int differing = Cv2.CountNonZero(mask);
            return differing / (double)(ca.Rows * ca.Cols);
        }
        finally
        {
            resized?.Dispose();
        }
    }

    /// <summary>The cell whose contents look most like <paramref name="icon"/>, and how different it
    /// is. Coordinates are client-relative, the same space <see cref="BagGrid.Centres"/> works in, so
    /// <paramref name="bag"/> must be a client capture rather than a crop of just the grid.</summary>
    public static (int Cell, double Score)? FindBestCell(Mat bag, IReadOnlyList<int> grid, Mat icon,
        int inset = DefaultInset, int tolerance = DefaultTolerance)
    {
        var all = ScoreAll(bag, grid, icon, inset, tolerance);
        return all.Count == 0 ? null : all[0];
    }

    /// <summary>How far a cell's contents may be shifted and still count as the same image.
    ///
    /// MEASURED, and it is the whole reason this parameter exists. The player's bag held seven pets,
    /// and the crops found three — with the pets of the same kind scoring 0.43 to 0.78 against each
    /// other while everything else in the bag sat at 0.83 and up. The cause was not the pets being
    /// different: it is that their sprites are drawn at different SUB-CELL OFFSETS, so a pixel-exact
    /// comparison only matched the cells where the sprite happened to land in the same place.
    ///
    /// Shrinking the compared box to the middle of the cell — the first thing tried, and the obvious
    /// one — made it strictly worse: every pet scored 0.84 or more, because the offset is a shift of
    /// the whole image and cropping harder does not chase it.
    ///
    /// So each cell is scored at a small spread of offsets and the BEST is kept. That is ordinary
    /// template matching, and it is what the measurements asked for.</summary>
    public const int SearchRadius = 3;

    /// <summary>The lowest differing fraction over every offset within <see cref="SearchRadius"/> of
    /// the cell's centre. Lower is better, as everywhere else here.</summary>
    private static double BestOffsetScore(Mat bag, (int X, int Y) centre, int cw, int ch, Mat icon,
        int inset, int tolerance, int radius)
    {
        var best = double.MaxValue;
        for (int dy = -radius; dy <= radius; dy++)
        {
            for (int dx = -radius; dx <= radius; dx++)
            {
                var rect = new Rect(centre.X + dx - cw / 2, centre.Y + dy - ch / 2, cw, ch);
                if (rect.X < 0 || rect.Y < 0 || rect.Right > bag.Width || rect.Bottom > bag.Height)
                    continue;

                using var cell = new Mat(bag, rect);
                var score = DifferingFraction(icon, cell, inset, tolerance);
                if (score < best) best = score;
                if (best == 0) return 0;   // cannot do better; stop looking
            }
        }
        return best;
    }

    /// <summary>Every cell's score, best first — the shape the calibration test wants. Reading only
    /// the winner hides how close the runner-up was, and a runner-up at 0.02 is a tool that will
    /// eventually click the wrong item.</summary>
    public static IReadOnlyList<(int Cell, double Score)> ScoreAll(Mat bag, IReadOnlyList<int> grid,
        Mat icon, int inset = DefaultInset, int tolerance = DefaultTolerance,
        int radius = SearchRadius)
    {
        var results = new List<(int Cell, double Score)>();
        if (bag.Empty() || icon.Empty() || !BagGrid.IsValidRect(grid)) return results;

        var centres = BagGrid.Centres(grid);
        int cw = (int)Math.Round(BagGrid.PitchX(grid));
        int ch = (int)Math.Round(BagGrid.PitchY(grid));
        if (cw <= 0 || ch <= 0) return results;

        for (int i = 0; i < centres.Count; i++)
        {
            var score = BestOffsetScore(bag, centres[i], cw, ch, icon, inset, tolerance, radius);
            if (score != double.MaxValue) results.Add((i, score));
        }

        results.Sort((a, b) => a.Score.CompareTo(b.Score));
        return results;
    }

    /// <summary>Encode a crop for storage in local.yaml. Base64 in the YAML rather than a side-car
    /// image file, because local.yaml is already the machine-specific store and a second file is a
    /// second thing to lose — a machine that copies its config but not the image gets a tool that
    /// matches the pet against nothing and cannot say why.</summary>
    public static string ToBase64(Mat image)
    {
        Cv2.ImEncode(".png", image, out byte[] bytes);
        return Convert.ToBase64String(bytes);
    }

    /// <summary>The stored crop, or null when there isn't one or it no longer decodes. Null is the
    /// answer the caller acts on — the tool refuses to match rather than matching against garbage.</summary>
    public static Mat? FromBase64(string? encoded)
    {
        if (string.IsNullOrWhiteSpace(encoded)) return null;
        try
        {
            var decoded = Cv2.ImDecode(Convert.FromBase64String(encoded), ImreadModes.Color);
            return decoded.Empty() ? null : decoded;
        }
        catch
        {
            return null;
        }
    }

    private static Mat Inset(Mat m, int inset)
    {
        int w = m.Width - inset * 2;
        int h = m.Height - inset * 2;
        if (w <= 0 || h <= 0) return new Mat();
        return new Mat(m, new Rect(inset, inset, w, h));
    }
}

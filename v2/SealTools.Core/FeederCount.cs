using System;
using System.Collections.Generic;
using System.Linq;
using OpenCvSharp;

namespace SealTools.Core;

// Reading a food stack's count off the boarding window, for the row scheduling that wants it.
//
// READ FROM A CROP, NOT FROM THE SLOT. The detector finds no text in a whole-slot image — the food
// icon fills the frame and it returns nothing at all — but it reads the same digits perfectly from a
// crop of the slot's right side at FULL HEIGHT. Both halves of that are measured:
//
//   whole slot (198x168)      -> 0 boxes
//   right portion, full height -> "84" at 0.99, and "300" at 1.00 on a second sample
//   right portion, HALF height -> 0 boxes
//
// so it is not "crop tighter is better": cutting the height is as fatal as including the icon.
//
// WHY THE WINDOW IS NARROW, AND WHAT THAT COSTS. The digits sit at a static place and grow LEFTWARD,
// so a three-digit count starts further left than a two-digit one:
//
//   "84"  correct for a left edge of 0.404 - 0.505 of the slot's width
//   "300" correct for                      0.29  - 0.45
//
// The overlap is roughly 0.40-0.45, about THREE native pixels. Outside it the failure is not blank —
// it is confidently WRONG:
//
//   "84"  at 0.55 -> "4"  0.89     (clipped the 8)
//   "300" at 0.50 -> "0"  0.56     (a full stack read as empty)
//
// A row scheduled from that starves or over-feeds. So a read is only accepted when TWO crops, a few
// pixels apart, return the same number: clipping changes the answer, a genuine read does not.
//
// WHERE that crop is is not this class's business any more: it is measured, not proportioned. The
// player drags one reference slot and the count region on it, and FeederLayout applies that offset to
// every slot in every row — see FeederLayout, and the six-drag calibration it describes. This class
// only owns what is done with the text once a crop has been taken.
public static class FeederCount
{
    /// <summary>The most items a stack can hold — a MAX load. A number above this is a misread, not a
    /// count, and is rejected rather than scheduled from.</summary>
    public const int MaxStack = 300;

    /// <summary>How sure the reader must be for a line to count. MEASURED, and the gap is wide: a
    /// genuine count scores 0.90-1.00, while everything the food icon produces scores at most 0.63 in
    /// the same crop. 0.75 sits inside that gap. It also rejects a genuine read taken at the very edge
    /// of the crop's window (0.72 was seen there), which is the right call — a marginal crop is the
    /// one that clips a digit.</summary>
    public const double MinScore = 0.75;

    /// <summary>The fraction of a crop that looks like FOOD — strongly red over blue.
    ///
    /// IT ANSWERS THE ONE QUESTION THE OCR CANNOT. An empty slot holds no digits, so it reads as
    /// "nothing" — exactly like a crop the reader failed on — and the tool resolved that ambiguity the
    /// wrong way round, as "assume a full load". Live, 2026-09-21: a row whose feeder had run dry was
    /// scheduled 205 minutes into the future and left its pet unfed, while both its slots showed a
    /// perfectly blank cell.
    ///
    /// MEASURED, on the crops the failed reads saved:
    ///
    ///     empty       0.0 %            (both slots of a dry row)
    ///     has food   30.4 – 37.0 %     (every slot of a full one)
    ///
    /// The icon is ochre — high red, low blue — and an empty cell is bare. Three statistics separate
    /// them; this is the widest margin and the easiest to explain, which is why it is the one used.
    /// OpenCV is BGR, so channel 2 is red and channel 0 is blue.</summary>
    public static double WarmFraction(Mat image, int redOverBlue = 40, int inset = 4)
    {
        if (image.Empty() || image.Width <= inset * 2 || image.Height <= inset * 2) return 0;

        // Inset for the same reason IconMatch insets: the cell's border sits in the same place in every
        // slot, and its pixels say nothing about what is inside.
        using var inner = new Mat(image, new Rect(inset, inset, image.Width - inset * 2, image.Height - inset * 2));
        var split = inner.Split();
        try
        {
            using var diff = new Mat();
            Cv2.Subtract(split[2], split[0], diff);   // R − B, saturating at 0 rather than wrapping
            using var mask = new Mat();
            Cv2.Threshold(diff, mask, redOverBlue, 255, ThresholdTypes.Binary);
            return Cv2.CountNonZero(mask) / (double)(inner.Rows * inner.Cols);
        }
        finally
        {
            foreach (var channel in split) channel.Dispose();
        }
    }

    /// <summary>Below this, a slot holds no food. 2 % against a measured 0.0 % for empty and 30.4 % for
    /// a full one — an order of magnitude inside the gap in both directions, so it is not a line
    /// anything sits near.</summary>
    public const double EmptySlotBelow = 0.02;

    /// <summary>The number out of one read: the digits of the BEST-scoring line above
    /// <paramref name="minScore"/>, or null.
    ///
    /// Best-scoring rather than concatenating everything, because a real read comes back with junk
    /// beside it — the same crop returned "300" at 1.00 next to a junk "0" at 0.56 — and the junk can
    /// outscore a genuine reading at a bad crop. Taking the single best line is what makes the score
    /// gate mean anything.</summary>
    public static int? Parse(IEnumerable<(string Text, double Score)> lines, double minScore)
    {
        var best = lines
            .Where(l => l.Score >= minScore)
            .OrderByDescending(l => l.Score)
            .FirstOrDefault();

        if (best.Text is null) return null;

        var digits = new string(best.Text.Where(char.IsDigit).ToArray());
        if (digits.Length == 0 || digits.Length > 3) return null;
        if (!int.TryParse(digits, out var n)) return null;

        return n is >= 0 and <= MaxStack ? n : null;
    }
}

using System;
using System.Collections.Generic;

namespace SealTools.Core;

// The bag grid, derived rather than pinned. The bag is a uniform 8x8, so 64 calibrated points would
// be 64 chances to drag one wrong — instead the calibrator takes two rectangles and everything else
// falls out of them:
//
//   BagGrid — the whole 8x8 region. This is what actually matters, because the pitch comes from
//             dividing it by 8, and a pitch error of one pixel is a whole slot by column eight.
//   BagSlot — a single slot. Not redundant: it is the independent check that the grid really is
//             uniform, which is the assumption the whole derivation rests on. A drag that disagrees
//             with the pitch is reported, not averaged away.
//
// Verified against a live capture before this existed: slot borders fitted to a pitch of 51.0 x 50.9
// with no drift across eight columns or rows, which is what makes the two-rectangle approach sound.
public static class BagGrid
{
    public const int Cols = 8;
    public const int Rows = 8;
    public const int SlotCount = Cols * Rows;

    /// <summary>A calibrated rectangle is [x, y, w, h] in client-relative physical pixels.</summary>
    public static bool IsValidRect(IReadOnlyList<int>? rect)
        => rect is { Count: 4 } && rect[2] > 0 && rect[3] > 0;

    /// <summary>Horizontal pitch the whole-grid rectangle implies — a slot plus its gap.</summary>
    public static double PitchX(IReadOnlyList<int> grid) => grid[2] / (double)Cols;

    /// <summary>Vertical pitch the whole-grid rectangle implies.</summary>
    public static double PitchY(IReadOnlyList<int> grid) => grid[3] / (double)Rows;

    /// <summary>The 64 slot CENTRES as client-relative physical offsets, row-major — index 0 is the
    /// top-left slot, index 63 the bottom-right. Centres, not corners, because that is where a click
    /// has to land.</summary>
    public static IReadOnlyList<(int X, int Y)> Centres(IReadOnlyList<int> grid)
    {
        if (!IsValidRect(grid)) return Array.Empty<(int, int)>();

        double px = PitchX(grid), py = PitchY(grid);
        var centres = new List<(int X, int Y)>(SlotCount);
        for (int row = 0; row < Rows; row++)
            for (int col = 0; col < Cols; col++)
                centres.Add((
                    grid[0] + (int)Math.Round((col + 0.5) * px),
                    grid[1] + (int)Math.Round((row + 0.5) * py)));
        return centres;
    }

    /// <summary>Where the single calibrated slot disagrees with the pitch the whole-grid rectangle
    /// implies, as a human-readable reason — or null when they agree. Returned rather than thrown so
    /// the calibrator can show it while the user is still dragging.</summary>
    public static string? Disagreement(IReadOnlyList<int> grid, IReadOnlyList<int> slot, int tolerancePx = 3)
    {
        if (!IsValidRect(grid)) return "the grid area isn't set";
        if (!IsValidRect(slot)) return "the slot box isn't set";

        double px = PitchX(grid), py = PitchY(grid);
        double dx = Math.Abs(slot[2] - px), dy = Math.Abs(slot[3] - py);
        if (dx <= tolerancePx && dy <= tolerancePx) return null;

        return $"the slot box is {slot[2]}x{slot[3]} but the grid implies {Math.Round(px)}x{Math.Round(py)} " +
               $"per slot (off by {Math.Round(dx)}x{Math.Round(dy)} px). One of the two drags is wrong — " +
               "re-drag the one that looks off.";
    }
}

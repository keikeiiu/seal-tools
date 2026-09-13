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

    // A slot is naturally SMALLER than the pitch — the pitch is centre-to-centre and includes the gap
    // between slots, while the slot box measures the slot itself. So an exact match is the wrong
    // expectation, and a tight tolerance rejects good calibrations. This allows a gap of up to a
    // quarter of a cell and still catches the thing the check exists for: a gross mis-drag, like a
    // slot box drawn around two slots or the grid box around half the bag.
    private const double SlotToleranceFraction = 0.25;
    private const double MinTolerancePx = 5;

    /// <summary>Where the single calibrated slot is too far from the pitch the whole-grid rectangle
    /// implies, as a human-readable reason — or null when it is plausible. Returned rather than thrown
    /// so the calibrator can show it while the user is still dragging.</summary>
    public static string? Disagreement(IReadOnlyList<int> grid, IReadOnlyList<int> slot)
    {
        if (!IsValidRect(grid)) return "the grid area isn't set";
        if (!IsValidRect(slot)) return "the slot box isn't set";

        double px = PitchX(grid), py = PitchY(grid);
        double dx = Math.Abs(slot[2] - px), dy = Math.Abs(slot[3] - py);
        if (dx <= Math.Max(MinTolerancePx, px * SlotToleranceFraction) &&
            dy <= Math.Max(MinTolerancePx, py * SlotToleranceFraction))
            return null;

        return $"the slot box is {slot[2]}x{slot[3]} but the grid works out to {Math.Round(px)}x" +
               $"{Math.Round(py)} per slot. A slot is normally a little smaller than that — the " +
               "difference is the gap between slots — so this is too far apart to be a gap. One of " +
               "the two drags is wrong; re-drag the one that looks off.";
    }

    /// <summary>The gap between slots implied by the two rectangles, for display. Not used in the
    /// derivation — the slot box is evidence, not a second source of truth.</summary>
    public static double ImpliedGap(IReadOnlyList<int> grid, IReadOnlyList<int> slot)
        => IsValidRect(grid) && IsValidRect(slot) ? PitchX(grid) - slot[2] : 0;
}

using System;
using System.Collections.Generic;

namespace SealTools.Core;

// Where a boarding row's food slots are, and where the count sits inside them — FOUR drags, and nothing
// else.
//
// The calibration it replaces was one hand-drawn box per food slot: 2 on the free row and 5 on each
// paid one, fourteen boxes, all of them the same shape repeated. A middle revision asked for six drags
// — the four strips plus a reference slot and a count box drawn on it — on the reasoning that a
// MEASURED offset beats a proportional one. That reasoning was sound and the drag was not: the band of
// left edges that reads correctly is about THREE PIXELS wide, and the player, asked twice to draw a box
// into it, missed it twice. An earlier revision read four offsets and voted, which was papering over
// a crop nobody had found yet: given the RIGHT crop the count reads at 0.99-1.00 at every upscale, so
// the run needs one read and the searching belongs in finding the fraction.
//
//   4  a strip across each row's food slots  ->  every slot box, divided by the row's stack count,
//                                                and every count region derived inside them
public static class FeederLayout
{
    /// <summary>The slot boxes of one row, from a strip dragged across its food slots.
    ///
    /// Divided evenly, which is an approximation: the player's own calibration had slots 62, 64, 66 and
    /// 62 apart in one row, so an even division is out by a pixel or two. That is harmless for the
    /// FOOD DRAG — a 64px slot does not care about 2px — and it is the thinnest part of the count read,
    /// where each crop's own tolerance measured 7-10px but the crops' common window was only 3. Worth
    /// knowing rather than discovering.
    ///
    /// Null when the strip or the stack count is unusable, so a half-calibrated row yields no slots
    /// rather than a row of boxes at (0,0).</summary>
    public static List<List<int>> SlotBoxes(List<int>? strip, int stacks)
    {
        if (strip is not { Count: 4 } || !BagGrid.IsValidRect(strip)) return null!;
        if (stacks < 1) return null!;

        var result = new List<List<int>>(stacks);

        // Integer arithmetic throughout, on the SLOT EDGES rather than on the width divided and then
        // re-multiplied: rounding the width first leaves a sliver of unclaimed space at the right end,
        // which is exactly where the count is read from.
        for (int i = 0; i < stacks; i++)
        {
            var left = strip[0] + (int)((long)strip[2] * i / stacks);
            var right = strip[0] + (int)((long)strip[2] * (i + 1) / stacks);
            result.Add(new List<int> { left, strip[1], Math.Max(1, right - left), strip[3] });
        }

        return result;
    }

    /// <summary>The count region for one slot, given the reference slot you DREW and the count box on
    /// it: the same box, moved to this slot.
    ///
    /// The offset is a measured position — where you put the box relative to a slot you drew — rather
    /// than a proportion of the slot. That is the point of drawing it: a drawn box says exactly where
    /// the number is, including on a machine where the digits sit differently inside the slot, which no
    /// fraction can express.
    ///
    /// Null when any of the three is unusable, so a half-drawn calibration reads as nothing rather than
    /// as a region at (0,0).</summary>
    public static List<int>? CountRegion(List<int>? slot, List<int>? drawnSlot, List<int>? countBox)
    {
        if (slot is not { Count: 4 } || !BagGrid.IsValidRect(slot)) return null;
        if (drawnSlot is not { Count: 4 } || !BagGrid.IsValidRect(drawnSlot)) return null;
        if (countBox is not { Count: 4 } || !BagGrid.IsValidRect(countBox)) return null;

        return new List<int>
        {
            slot[0] + (countBox[0] - drawnSlot[0]),
            slot[1] + (countBox[1] - drawnSlot[1]),
            countBox[2],
            countBox[3],
        };
    }

    /// <summary>The slot a drawn box belongs to: the nearest of the given slots by centre distance.
    ///
    /// Needed because the calibration stores boxes, not identities: nothing says which row or slot the
    /// count box was drawn on, and the offset only means anything relative to the right one.</summary>
    public static List<int>? NearestSlot(List<List<int>> slots, List<int>? box)
    {
        if (box is not { Count: 4 } || !BagGrid.IsValidRect(box) || slots.Count == 0) return null;

        var cx = box[0] + box[2] / 2.0;
        var cy = box[1] + box[3] / 2.0;
        List<int>? best = null;
        var bestD = double.MaxValue;
        foreach (var s in slots)
        {
            if (!BagGrid.IsValidRect(s)) continue;
            var dx = s[0] + s[2] / 2.0 - cx;
            var dy = s[1] + s[3] / 2.0 - cy;
            var d = dx * dx + dy * dy;
            if (d < bestD) { bestD = d; best = s; }
        }
        return best;
    }
}

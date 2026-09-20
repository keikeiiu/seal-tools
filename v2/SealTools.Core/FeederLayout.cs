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

    /// <summary>Where the count is read from inside a slot, as a fraction of the slot's width. The
    /// region runs from here to the slot's own right edge, at the slot's full height.
    ///
    /// MEASURED on the player's own slots (2026-09-21), by sweeping a real crop's left edge and
    /// reading the result at upscales 2 through 6:
    ///
    ///     trim of the crop       "138"                  "300"
    ///     0 - 12 px (native)     wrong or nothing       reads
    ///     13 - 22 px             reads at every scale   reads at every scale
    ///     23 px and beyond       junk                   reads, then junk
    ///
    /// so the working left edge is slot_left + 25.3 .. 28.3 px of a 65 px slot, i.e. 38.9 - 43.5%.
    /// 0.40 sits inside that with room either side. The right edge is the slot's own, because the
    /// digits were measured ENDING 5px inside it — an earlier revision took the region 3px past the
    /// frame on the theory that the number spills, which it does not here, and an absolute overhang
    /// would not have survived another machine anyway.
    ///
    /// MEASURED, and then measured again: a scan of every left edge a pixel apart on two real crops
    /// read correctly at 0.369-0.431 ("138") and 0.323-0.477 ("300"), so 0.40 sits inside the overlap
    /// with room either side. It is a SETTING rather than a constant so another machine can move it
    /// without a rebuild — this is the one number the whole count read comes down to.</summary>
    public const double ReadLeftFraction = 0.40;

    /// <summary>The region a count is read from: from that fraction of the slot's width to the slot's
    /// RIGHT EDGE, at the slot's FULL HEIGHT.
    ///
    /// Full height is not a preference. Measured: a crop cut down to the digits alone finds NOTHING —
    /// a box holding a perfectly legible "174" returned zero detected boxes at 21px tall inside a
    /// 58px slot — and the same crop at full height reads it.</summary>
    public static List<int>? ReadRegion(List<int>? slot, double leftFraction)
    {
        if (slot is not { Count: 4 } || !BagGrid.IsValidRect(slot)) return null;

        var left = slot[0] + (int)Math.Round(slot[2] * leftFraction);
        var right = slot[0] + slot[2];
        if (right - left < 4) return null;

        return new List<int> { left, slot[1], right - left, slot[3] };
    }
}

using System;
using System.Collections.Generic;

namespace SealTools.Core;

// Where a boarding row's food slots are, and where the count is inside them — derived from SIX drags
// instead of twenty.
//
// The calibration it replaces was one hand-drawn box per food slot: 2 on the free row and 5 on each
// paid one, fourteen boxes, all of them the same shape repeated. And it carried two fractions
// (FeederCount.CropLeft) for where the count sits inside a slot, which were measured on ONE machine
// and assumed to hold on every other — an assumption nobody had tested, which is the kind this project
// has paid for repeatedly.
//
// The six drags:
//
//   4  a strip across each row's food slots      -> every slot box, divided by the row's stack count
//   1  ONE reference slot                        -> the anchor
//   1  the count region on that reference slot    -> its offset and size
//
// The last two are what replace the fractions. The offset from the reference slot to its count box is
// a MEASURED position rather than a guessed proportion, so it survives a resolution where the digits
// are drawn at a different size relative to the slot — which is exactly the case a fraction cannot
// express and the reason this exists.
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

    /// <summary>The count region for one slot: the reference offset applied to that slot.
    ///
    /// The offset is <paramref name="refText"/> relative to <paramref name="refSlot"/> — the two boxes
    /// the player drew — and it is added to the slot being read. A duplicate of the reference slot's
    /// own box, so the reference slot reads identically whether it is reached by derivation or drawn.
    ///
    /// Null when any of the three is missing: a slot with no count region reads as nothing, which the
    /// caller already treats as "assume a full load".</summary>
    public static List<int>? CountRegion(List<int>? slot, List<int>? refSlot, List<int>? refText)
    {
        if (slot is not { Count: 4 } || !BagGrid.IsValidRect(slot)) return null;
        if (refSlot is not { Count: 4 } || !BagGrid.IsValidRect(refSlot)) return null;
        if (refText is not { Count: 4 } || !BagGrid.IsValidRect(refText)) return null;

        return new List<int>
        {
            slot[0] + (refText[0] - refSlot[0]),
            slot[1] + (refText[1] - refSlot[1]),
            refText[2],
            refText[3],
        };
    }

    /// <summary>The same region shifted right — the second read, which must agree with the first.
    /// Clipping a digit changes the answer between the two; a genuine read does not.</summary>
    public static List<int> Shift(List<int> region, int pixels) =>
        new() { region[0] + pixels, region[1], region[2], region[3] };
}

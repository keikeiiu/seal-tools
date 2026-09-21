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

    /// <summary>Where the count is read from inside a slot: this far across it, running to its own
    /// right edge at its FULL HEIGHT.
    ///
    /// MEASURED, and this is the only approach of four that has held. Two crops read at 0.369-0.431 of
    /// the slot ("138" and "300"); a scan of every left edge a pixel apart put the good band there and
    /// nowhere else. Three attempts to let the position be DRAWN instead all failed — a count box, a
    /// count slot read as drawn, and a count slot with its offset applied — because the band is about
    /// three pixels wide and a hand cannot put a box into three pixels. Measured four times, missed
    /// four times.
    ///
    /// FULL HEIGHT is not a preference either. A crop cut to the digits' height finds NOTHING while
    /// holding a perfectly legible number — measured on three separate crops ("174", "36", "18"), and
    /// the reason is not known; the engine's own "text below ~20px" note does not explain it, since the
    /// digits are ~66px tall upscaled.
    ///
    /// A SETTING rather than a constant, because the band is narrow: a machine that reads nothing, or
    /// reads too short a number, has to be able to move it without a rebuild.</summary>
    public const double ReadLeftFraction = 0.40;

    /// <summary>Where a row's TIME line is read from — the text under its food slots, derived from the
    /// strip the same way the count crop is.
    ///
    /// MEASURED on the live window, in the config's own coordinates, on two rows that agree:
    ///
    ///     row 2  slots end 516   text at 527 / 550      (+11 / +34)
    ///     row 3  slots end 705   text at 715 / 737      (+10 / +32)
    ///
    /// **DERIVED AS RATIOS OF THE SLOT HEIGHT, not as fixed pixels**, and that is the point: the strip
    /// is calibrated per machine, so a PC whose slots are taller gets a proportionally lower and taller
    /// region without anything to calibrate or drag. A fixed `+32` would be right here and wrong on any
    /// screen whose UI scales.
    ///
    /// The width is deliberately generous — the OCR ignores background — because the text is longer on
    /// some rows than others and a tight box would clip the number off the end.
    ///
    /// Null when the strip is unusable, so a half-calibrated row yields no region rather than one at
    /// (0,0).</summary>
    public static List<int>? EtaRegion(List<int>? strip)
    {
        if (strip is not { Count: 4 } || !BagGrid.IsValidRect(strip)) return null;

        var h = strip[3];
        return new List<int>
        {
            strip[0] - (int)Math.Round(h * 0.10),
            strip[1] + strip[3] + (int)Math.Round(h * 0.53),
            (int)Math.Round(h * 4.7),
            (int)Math.Round(h * 0.40),
        };
    }

    /// <summary>The region a count is read from for one slot.</summary>
    public static List<int>? ReadRegion(List<int>? slot, double leftFraction)
    {
        if (slot is not { Count: 4 } || !BagGrid.IsValidRect(slot)) return null;

        var left = slot[0] + (int)Math.Round(slot[2] * leftFraction);
        var right = slot[0] + slot[2];
        if (right - left < 4) return null;

        return new List<int> { left, slot[1], right - left, slot[3] };
    }
}

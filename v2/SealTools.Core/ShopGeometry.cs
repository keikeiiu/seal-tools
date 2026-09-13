using System;
using System.Collections.Generic;

namespace SealTools.Core;

// Where to click for a shop list row. The list scrolls, so a preset stores (scroll, row): scroll to
// the top, apply N notches, then click row R.
//
// Row positions are DERIVED from the list region rather than marked by hand. The calibrator drags one
// box around the visible part of the list, the game shows a fixed number of rows inside it, and
// everything else follows:
//
//     pitch      = regionHeight / rows
//     row k Y    = regionTop + (k + 0.5) * pitch
//     click X    = the region's horizontal centre
//
// Two hand-clicked rows would give the same information and can disagree with each other; a single
// region cannot. The verification is the same as the bag grid's — draw the derived centres over the
// capture and look at whether they sit on the rows.
public static class ShopGeometry
{
    /// <summary>The row count for this game's shop list. A config value rather than a constant so a
    /// game update that changes it does not need a code change — but it is deliberately NOT exposed
    /// in the UI: it is a property of the game, not of anyone's setup, and a wrong value here shifts
    /// every derived row.</summary>
    public const int DefaultRows = 10;

    /// <summary>The most notches one scroll command may carry — the firmware's WHEEL_MAX_NOTCHES,
    /// mirrored here so the tool and the calibrator share ONE number rather than keeping a copy each.
    /// (Three copies previously drifted apart, and the stale one silently rejected valid setups.)
    ///
    /// It is a guard and nothing more: it stops a malformed value wedging the board, which cannot
    /// read serial while it loops. It is NOT the reach — a preset's scroll is applied from wherever
    /// the list already is, and putting the list at the top is the user's setup rather than a scroll
    /// the tool sends. An earlier design did scroll to the top, which made this ceiling
    /// load-bearing; that is gone.
    ///
    /// Cannot be shared with the firmware itself — different language — so seal_mouse.ino's define is
    /// the one genuine duplicate.</summary>
    public const int MaxScrollNotches = 400;

    public static bool IsValidRegion(IReadOnlyList<int>? region) => BagGrid.IsValidRect(region);

    /// <summary>Centre-to-centre row spacing implied by the region and the row count.</summary>
    public static double RowPitch(IReadOnlyList<int>? region, int rows)
        => IsValidRegion(region) && rows > 0 ? region![3] / (double)rows : 0;

    /// <summary>Centre of the row at <paramref name="index"/> in the visible list, or null when the
    /// region isn't drawn. Row 0 is the top one. Deliberately NOT clamped: asking for a row past the
    /// bottom means the preset is wrong, and that is the caller's to report, not to round off.</summary>
    public static (int X, int Y)? RowCentre(IReadOnlyList<int>? region, int rows, int index)
    {
        if (!IsValidRegion(region) || rows < 1 || index < 0) return null;
        var pitch = RowPitch(region, rows);
        if (pitch <= 0) return null;
        return (region![0] + region[2] / 2,
                region[1] + (int)Math.Round((index + 0.5) * pitch));
    }

    /// <summary>Every row centre, for the calibrator's overlay.</summary>
    public static IReadOnlyList<(int X, int Y)> RowCentres(IReadOnlyList<int>? region, int rows)
    {
        var centres = new List<(int X, int Y)>(Math.Max(0, rows));
        for (int i = 0; i < rows; i++)
            if (RowCentre(region, rows, i) is { } c) centres.Add(c);
        return centres;
    }

    /// <summary>Why the row geometry can't be used, or null when it can.</summary>
    public static string? Problem(IReadOnlyList<int>? region, int rows)
    {
        if (!IsValidRegion(region)) return "the shop list region isn't drawn";
        if (rows < 1) return "the configured row count is 0";
        return null;
    }
}

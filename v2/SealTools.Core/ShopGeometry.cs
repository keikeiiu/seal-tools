using System;
using System.Collections.Generic;

namespace SealTools.Core;

// Where to click for a shop list row. The list scrolls, so a preset stores (scroll, row): scroll to
// the top, apply N notches, then click row R. Row R's position is the first row plus R pitches.
//
// The pitch comes from two adjacent clicks rather than one click and an assumed row height — nine
// rows of drift would put the last one on the wrong item.
public static class ShopGeometry
{
    /// <summary>Vertical pitch between list rows, from the two calibrated rows. Zero when either is
    /// unset, or when the second is not below the first — which is a calibration mistake, not a
    /// pitch of zero, so callers should treat 0 as "not usable" rather than "no gap".</summary>
    public static int RowPitch(IReadOnlyList<int>? first, IReadOnlyList<int>? second)
    {
        if (first is not { Count: 2 } || second is not { Count: 2 }) return 0;
        var pitch = second[1] - first[1];
        return pitch > 0 ? pitch : 0;
    }

    /// <summary>Centre of the row at <paramref name="index"/> in the currently visible list, or null
    /// when the row geometry isn't calibrated. Index 0 is the row the first-row click marked; it is
    /// not clamped, because asking for a row past the bottom means the preset is wrong and a click
    /// into the void is better reported than silently moved.</summary>
    public static (int X, int Y)? RowCentre(IReadOnlyList<int>? first, IReadOnlyList<int>? second, int index)
    {
        if (first is not { Count: 2 } || index < 0) return null;
        var pitch = RowPitch(first, second);
        if (pitch == 0) return null;
        return (first[0], first[1] + index * pitch);
    }

    /// <summary>Why the row geometry can't be used, or null when it can. The calibrator shows this
    /// rather than letting a half-set pair produce a click at the list's top-left corner.</summary>
    public static string? Problem(IReadOnlyList<int>? first, IReadOnlyList<int>? second)
    {
        if (first is not { Count: 2 }) return "the first row isn't marked";
        if (second is not { Count: 2 }) return "the second row isn't marked";
        if (second[1] <= first[1]) return "the second row must be BELOW the first one";
        return null;
    }
}

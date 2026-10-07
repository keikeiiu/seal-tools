using System;
using System.Globalization;

namespace SealTools.Core;

// The board has never been able to answer a question. Every command goes one way, so a host cannot
// tell a board that lacks a feature from a board whose feature did not fire — the two look identical
// from here, and both look like nothing happening.
//
// "V" is the first thing the sketch writes BACK. The valuable half of the answer is the silence: a
// board that does not reply is a definite no, because the older sketch never wrote anything at all.
//
// The number is a PROTOCOL LEVEL, incremented when the board's behaviour changes — not a build
// counter, which would tell a player nothing about whether their board has the feature they need.
// It starts at 1 rather than the plan's 3: no board can report anything today, so a scheme with two
// predecessor levels would imply sketches that were never flashed and that nobody can tell apart.
public static class FirmwareVersion
{
    /// <summary>The command that asks the board what it is running.</summary>
    public const string Query = "V";

    /// <summary>The protocol level this host knows about. A board reporting less is missing something;
    /// one reporting more is newer than this launcher.</summary>
    public const int Current = 2;

    /// <summary>The level that added L/l — hold and release the left mouse button, which is what a
    /// drag is made of. Below it the letters are ignored and the item is never picked up, which looks
    /// exactly like a mis-aimed drag: see <see cref="FoodLoadMode"/>.</summary>
    public const int DragLevel = 2;

    /// <summary>Reads the level out of a reply line — "V 1" gives 1. Null when the line is not a
    /// version reply at all, which is the case that must not be mistaken for a level: a board that
    /// answers with anything else has not answered *this*.</summary>
    public static int? Parse(string? line)
    {
        if (string.IsNullOrWhiteSpace(line)) return null;

        var parts = line.Trim().Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
        if (parts.Length != 2) return null;
        if (!string.Equals(parts[0], Query, StringComparison.Ordinal)) return null;

        return int.TryParse(parts[1], NumberStyles.None, CultureInfo.InvariantCulture, out var level)
            && level > 0
            ? level
            : null;
    }

    /// <summary>The sentence to show for a reply — or for the silence, which is an answer too.
    ///
    /// Written for the LOG and for a bug report, where the number is the useful part. For the player
    /// see <see cref="Summary"/>: "protocol level 2 (current)" is exact and tells someone who only
    /// wants to know whether their board is up to date precisely nothing.</summary>
    public static string Describe(int? level) => level switch
    {
        null => "did not answer — its firmware predates version reporting",
        var l when l == Current => $"protocol level {l} (current)",
        var l when l < Current => $"protocol level {l} — older than this launcher's {Current}",
        var l => $"protocol level {l} — newer than this launcher's {Current}",
    };

    /// <summary>The same fact said the other way round: what the level MEANS, and what to do about it.
    ///
    /// The Arduino card shows this. The level is a developer's unit — a player cannot tell whether
    /// "level 2" is good — and the only thing it decides today is whether the board can DRAG food
    /// (<see cref="DragLevel"/>), so that is what the sentence is about, in the same shape
    /// <see cref="FoodLoadMode.Complaint"/> already uses: what is wrong, and the two ways out.
    ///
    /// The number is not lost — the card keeps <see cref="Describe"/> in a tooltip, because a report
    /// is exactly where it belongs.</summary>
    public static string Summary(int? level) => level switch
    {
        // Silence is a definite no rather than an unknown: the sketch that predates version reporting
        // also predates L/l. Saying so is the point — the drag it cannot do fails by doing NOTHING,
        // which from outside is identical to a mis-aimed drag.
        null => "Too old to report a version, so it also predates the drag commands — a food drag " +
                "would silently do nothing. Reflash the board, or set the Pet tab's food load back " +
                "to right-click.",
        var l when l < DragLevel => $"Older than this launcher's — it cannot drag food, and a drag " +
                "would silently do nothing. Reflash the board, or set the Pet tab's food load back " +
                "to right-click.",
        var l when l > Current => "Newer than this launcher knows — it should work, but anything " +
                "added since is not something this launcher can tell you about.",
        _ => "Up to date — it supports everything this launcher sends, dragging food included.",
    };
}

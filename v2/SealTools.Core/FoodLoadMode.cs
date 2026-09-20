using System;

namespace SealTools.Core;

// How the pet feeder puts a food stack into the boarding window.
//
// This exists because of a game rule that was costing real feedings. A RIGHT-CLICK drops the stack
// into the EARLIEST empty food box in the boarder, and every row's boxes form one queue ordered
// top-down — so a stack meant for row 3 lands in row 1's box whenever row 1 has run dry, and the
// tool never noticed because it never looked at where the food went. A DRAG names the box.
//
// Both are kept because they need different boards: a drag needs firmware level 2 (the L/l commands),
// a right-click works on every board ever flashed. The setting is per machine, on the Pet tab, so a
// player can switch to drag the day they reflash — and switch back if it misbehaves.
public static class FoodLoadMode
{
    /// <summary>Right-click the bag stack; the game chooses the box. Works on any firmware.</summary>
    public const string RightClick = "right_click";

    /// <summary>Press on the bag stack, drag to the row's own food box, release. Needs firmware 2.</summary>
    public const string Drag = "drag";

    /// <summary>Every value the setting accepts, in the order the picker shows them.</summary>
    public static readonly string[] All = { RightClick, Drag };

    /// <summary>Whether the configured value asks for a drag.</summary>
    public static bool IsDrag(string? mode) =>
        string.Equals(mode?.Trim(), Drag, StringComparison.OrdinalIgnoreCase);

    /// <summary>Whether a board at <paramref name="firmwareLevel"/> can do a drag.
    ///
    /// Null means the board never answered — which is a definite no, not an unknown: the sketch that
    /// predates version reporting also predates L/l. This is the one place the firmware version
    /// decides behaviour rather than being printed, which is why the level says what it means.
    /// </summary>
    public static bool BoardCanDrag(int? firmwareLevel) => firmwareLevel >= FirmwareVersion.DragLevel;

    /// <summary>What to do about the setting and the board disagreeing, or null when they agree.
    /// Returns a sentence for the card, because "the drag did nothing" and "the board is too old to
    /// drag" look identical from outside.</summary>
    public static string? Complaint(string? mode, int? firmwareLevel)
    {
        if (!IsDrag(mode)) return null;
        if (BoardCanDrag(firmwareLevel)) return null;

        return firmwareLevel is { } level
            ? $"Food load is set to drag but the board reports firmware {level}, which has no drag " +
              "commands — reflash the board, or set the Pet tab's food load back to right-click."
            : "Food load is set to drag but the board did not report a firmware version, so it " +
              "predates the drag commands — reflash the board, or set the Pet tab's food load back " +
              "to right-click.";
    }
}

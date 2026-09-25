using System;
using System.Globalization;
using SealTools.Core.Config;

namespace SealTools.Core;

// What one step of a quest flow sends. Five values, and no more, because the board's protocol has no
// more: C/R click, E Enter, and K for a single printable character.
//
// This exists as its own type, rather than as bare strings in the config, for one reason: a step that
// CANNOT BE SENT must say so. A `key` step with no character, or with two, is a step that does nothing —
// and in a flow replayed 500 times, a step that does nothing is indistinguishable from a quest that
// behaves differently today. FoodLoadMode.Complaint is the same shape for the same reason: "the drag did
// nothing" and "the board is too old to drag" look identical from outside.
public static class QuestAction
{
    /// <summary>Left click, at wherever the cursor is. The tool never moves it.</summary>
    public const string Click = "click";

    /// <summary>Right click, at wherever the cursor is.</summary>
    public const string RightClick = "right_click";

    /// <summary>Press one printable character — a digit, a letter. The firmware's K handler.</summary>
    public const string Key = "key";

    /// <summary>Enter. The board's own command, not K with a newline.</summary>
    public const string Enter = "enter";

    /// <summary>Send nothing and wait. For a dialogue beat that needs longer than the others.</summary>
    public const string Wait = "wait";

    /// <summary>Every value the setting accepts, in the order the picker shows them.</summary>
    public static readonly string[] All = { Click, RightClick, Key, Enter, Wait };

    /// <summary>Whether a step sends input at all. A wait step exists to pass time.</summary>
    public static bool SendsInput(string? action) =>
        !string.Equals(action?.Trim(), Wait, StringComparison.OrdinalIgnoreCase);

    /// <summary>Why this step cannot be sent, or null when it can. The caller names the step's number,
    /// because "step 7" is what the player has to go and fix.
    ///
    /// A character outside printable ASCII is refused rather than sent: the firmware's K handler presses
    /// one character, so anything else is a command the board would ignore silently — and this tool has
    /// nothing else watching to notice.</summary>
    public static string? Complaint(QuestStep step)
    {
        var action = step.Action?.Trim() ?? "";

        if (Array.IndexOf(All, action) < 0)
            return $"\"{step.Action}\" is not a step this tool can send — pick one of " +
                   string.Join(", ", All) + ".";

        if (!string.Equals(action, Key, StringComparison.OrdinalIgnoreCase)) return null;

        var value = step.Value ?? "";
        if (value.Length == 0) return "a key step with no character would send nothing at all.";
        if (value.Length > 1)
            return $"a key step sends ONE character, and \"{value}\" is {value.Length} — only the " +
                   "board's own F1-F12 command carries a number, and that is not this.";
        if (value[0] < 0x20 || value[0] > 0x7E)
            return $"\"{value}\" is not a printable character the board can press.";

        return null;
    }

    /// <summary>How a step reads in a log line or on the card.</summary>
    public static string Describe(QuestStep step)
    {
        var action = step.Action?.Trim() ?? "";
        var delay = step.DelaySeconds.ToString("0.##", CultureInfo.InvariantCulture);

        return string.Equals(action, Key, StringComparison.OrdinalIgnoreCase)
            ? $"key {step.Value} ({delay}s)"
            : string.Equals(action, Wait, StringComparison.OrdinalIgnoreCase)
                ? $"wait ({delay}s)"
                : $"{action.Replace('_', ' ')} ({delay}s)";
    }
}

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text.RegularExpressions;

namespace SealTools.Core;

/// <summary>What the boarding window says about TIME — the game's own answer to "when should this row be
/// looked at again".
///
/// Each row carries two lines of text under its food slots, and the second changes form depending on
/// where the pet is in its run. Read off a live window (2026-09-21):
///
///     代養完成預計所需時間：約14分        boarding COMPLETES in about 14 min   ← a pet about to finish
///     到2為止預計所需時間：約 20分        reaches level 2 in about 20 min      ← a pet mid-run
///     該欄位約27日9時51分16 秒後到期。     the food in this slot expires in 27d   ← shelf life, not a time-to
///     每1分 攝取3個。                     consumes 3 per minute
///
/// **The completion form is the one that matters**, and it is why this class exists: a pet that finishes
/// is mailed, the row goes empty, and without this line the row waits out its whole cycle — six hours in
/// the case that produced it — with nothing in the tray and no pet boarding.
///
/// **Only the completion form drives a schedule.** The level form resets every level and reads 20-39
/// minutes on a healthy pet; treating it as a revisit trigger would visit constantly. It is parsed and
/// reported, and the food figure keeps the row fed.
///
/// The Chinese is matched loosely and in Traditional, which is what the GAME writes — not the Simplified
/// of the scraped data, which is a different source and must never be matched against this one.</summary>
public static class FeederEta
{
    /// <summary>A row's time line: how many minutes, and whether it is the COMPLETION form.</summary>
    public sealed record Reading(int Minutes, bool IsCompletion, string Line);

    /// <summary>"about N min", and nothing else may be a time-to. The anchor is 約 — the expiry line
    /// writes `約27日9時51分16秒後到期`, and its number is followed by 日 rather than 分, so the anchor
    /// plus the unit is what keeps a shelf life from being read as a countdown.</summary>
    private static readonly Regex AboutMinutes =
        new(@"約\s*(\d{1,4})\s*分", RegexOptions.Compiled);

    /// <summary>Reads the time line out of a row's text, or null when there is nothing to read.
    ///
    /// Null is an ordinary answer — an older window, a mangled read, a pet whose line says something
    /// else — and the caller falls back to the food figure, which is what it did before this existed.</summary>
    public static Reading? Parse(IEnumerable<string>? lines)
    {
        if (lines == null) return null;

        foreach (var line in lines)
        {
            if (string.IsNullOrWhiteSpace(line)) continue;

            // The food's shelf life carries a 約 and a 分-shaped number too; 到期 is what tells them
            // apart, and the regex alone would not.
            if (line.Contains('到') && line.Contains('期')) continue;

            var match = AboutMinutes.Match(line);
            if (!match.Success) continue;

            if (!int.TryParse(match.Groups[1].Value, NumberStyles.Integer,
                    CultureInfo.InvariantCulture, out var minutes)) continue;

            // COMPLETION FIRST: that line also carries 所需時間, so testing for the level form first
            // would classify a finished pet as one that is merely levelling.
            if (line.Contains('完') || line.Contains("完成"))
                return new Reading(minutes, true, line.Trim());

            if (line.Contains("所需時間"))
                return new Reading(minutes, false, line.Trim());
        }

        return null;
    }

    /// <summary>How long until the row next needs looking at BECAUSE OF THE PET — null unless the game
    /// said the boarding completes, in which case the pet is about to be mailed and the row emptied.</summary>
    public static double? CompletionMinutes(Reading? reading) =>
        reading is { IsCompletion: true } ? reading.Minutes : null;
}

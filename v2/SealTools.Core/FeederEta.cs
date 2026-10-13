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

    /// <summary>`到9為止` — the 餵養值 / growth the line counts down TO. 「到9為止预計所需時間：約94分」
    /// says "estimated time to reach +9", and +9 with a full bar is a finished pet (see
    /// <c>PetPanel.MaxGrowth</c>). 止 is matched in both scripts because the OCR mixes them line by
    /// line — the measured logs show Traditional 為止 beside Simplified 预 in the same sentence.</summary>
    private static readonly Regex GrowthTo = new(@"到\s*(\d{1,2})\s*[為为]止", RegexOptions.Compiled);

    /// <summary>The GROWTH (餵養值, +0 … +9) a line says the pet is heading for, or null when it names
    /// none.
    ///
    /// THREE NUMBERS LIVE NEAR EACH OTHER, and the first version of this method confused two of them
    /// — the player caught it: *"there is no stage 8 stage 9."* The hover panel's `(6階)` is the
    /// **stage** (it has no 8/9); the panel's `+7` and this line's `到9為止` are the **growth**, which
    /// runs +0 … +9 and ends at +9/100 % — the finished pet. The card shows `→ +9` for that reason,
    /// in the panel's own vocabulary.
    ///
    /// DISPLAY ONLY: the level form drives nothing (see <see cref="CompletionMinutes"/>), and this
    /// exists so a card can say what the line is counting to instead of quoting a sentence nobody
    /// reads.</summary>
    public static int? GrowthTarget(string? line)
    {
        if (line is null) return null;
        var match = GrowthTo.Match(line);
        return match.Success && int.TryParse(match.Groups[1].Value, NumberStyles.Integer,
            CultureInfo.InvariantCulture, out var growth) && growth >= 0 && growth <= PetPanel.MaxGrowth
            ? growth
            : null;
    }

    /// <summary>`每1分 攝取3個` — the game's own statement of the FEEDING RATE, and the line the
    /// player pointed at when they explained that the rate follows the pets' feeding stage
    /// (2026-10-11: *"the reason why it is 3 food per pet is those pets are stage 6 pets"*).
    ///
    /// MATCHED LOOSELY, because the live OCR mangles this line far worse than a dictionary word: the
    /// forms actually in pet.log are `每1分最取适` (37 times — the ITEM COUNT IS GONE), `每分最取尚`,
    /// `每1分瑜取3固` and `每1分输取3固`. So the ending is NOT matched at all — `個` arrives as `固`,
    /// `适`, `尚` — and only the two NUMBERS are required: the minute base after 每, and the nearest
    /// number after 分. When the item count did not survive there is nothing to read and this returns
    /// null, which is the honest answer (the configured rate stands; it is the fallback).
    ///
    /// The gap is capped and excludes the time line's own characters, so a line where the OCR joined
    /// the rate sentence to 「到7為止…約25分」 cannot read the 7 as an item count.</summary>
    private static readonly Regex RateLine = new(
        @"每\s*(\d{1,3})\s*分[^\d到約為止]{0,6}(\d{1,4})", RegexOptions.Compiled);

    /// <summary>Items per minute as the game states it, or null when the line is not there or does
    /// not parse.
    ///
    /// WHY IT EXISTS, and why reading beats deducing: the player asked whether the tool could work
    /// the rate out from the pets' stage ("possible we can deduce that?"). It does not have to — the
    /// window prints the rate beside every row ("每1分 攝取3個"), so the row can carry its OWN rate
    /// instead of one configured number for all four. That matters the moment two rows hold pets at
    /// different feeding stages, which is exactly when a single constant starts mis-timing one of
    /// them. A stage→rate table would have been a guess; this is a reading.
    ///
    /// Implausible values are rejected rather than clamped: a misread rate would silently rescale
    /// every time this row computes, and the caller falls back to the configured number instead.</summary>
    public static int? ParseRate(IEnumerable<string>? lines)
    {
        if (lines == null) return null;

        foreach (var line in lines)
        {
            if (string.IsNullOrWhiteSpace(line)) continue;

            var match = RateLine.Match(line);
            if (!match.Success) continue;
            if (!int.TryParse(match.Groups[1].Value, NumberStyles.Integer,
                    CultureInfo.InvariantCulture, out var minutes) || minutes <= 0) continue;
            if (!int.TryParse(match.Groups[2].Value, NumberStyles.Integer,
                    CultureInfo.InvariantCulture, out var items)) continue;

            // Whole items per minute, 1..30: the config's own scale, and wide enough that no honest
            // line is refused while a two-digit misread still is.
            if (items <= 0 || items > 30 * minutes) continue;
            var rate = (int)Math.Round(items / (double)minutes);
            if (rate is >= 1 and <= 30) return rate;
        }

        return null;
    }

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

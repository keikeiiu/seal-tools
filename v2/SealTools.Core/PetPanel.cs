using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;

namespace SealTools.Core;

/// <summary>What a pet's hover panel says — the read behind every decision the feeder makes about
/// WHICH pet to board. See docs/PLAN-HOVER-INFO.md and docs/PLAN-PET-AUTOFEED.md §13.
///
/// Only the three NUMBERS are parsed, and that is the design rather than a shortcut. The panel reads
/// as <c>（6）真蔚蓝凤凰+7[52.18%]</c> against a screen showing <c>(6階) 真蔚藍鳳凰 +7 [52.18%]</c>:
/// every digit comes back exact while the Chinese does not, and the recogniser's errors are not
/// uniform — some characters arrive Simplified and are converted by the text_fixes table, others
/// arrive Traditional already, others are simply misread. A parser keyed on any character inherits
/// all of it. The digits and the punctuation around them have shown no such variance.
///
/// It also sidesteps the script mismatch with PET-DATA.md: those names are Simplified (they come
/// from the Simplified data site) and the game is not, so even a perfect read would not match, and
/// a name lookup could not have worked. It is not needed — the panel states the stage outright.</summary>
public sealed record PetPanel(int? Stage, int Growth, double Exp)
{
    /// <summary>The growth a pet stops at. The game shows +0 … +9, and +9 is the last of them.</summary>
    public const int MaxGrowth = 9;

    public const double FinishedExp = 100;

    /// <summary>Whether this pet is done — what the docs call <b>+10</b>, which is OUR name for it.
    /// The string appears nowhere in the game, which shows +9 with a percentage instead, and that
    /// gap is why this test needs BOTH numbers: a +9 whose bar is still filling is not done, and
    /// neither is a full bar on a +7. Either one alone is wrong in one direction.
    ///
    /// EQUALS, not at-least, and that is the fail-safe choice: 9 is the game's maximum, so a growth
    /// above it can only be a misread — and "at least 9" would answer "finished" to a garbage panel,
    /// which is the one direction that makes the tool stop feeding a pet that still needs it.</summary>
    public bool IsFinished => Growth == MaxGrowth && Exp >= FinishedExp;

    /// <summary>For a log line or the Test read. Deliberately not the record's default ToString —
    /// the numbers are the whole content, and "未读" for a stage the OCR missed says so rather than
    /// printing a zero that looks measured.</summary>
    public string Describe() =>
        $"stage {(Stage is { } s ? s.ToString(CultureInfo.InvariantCulture) : "?")}, " +
        $"+{Growth.ToString(CultureInfo.InvariantCulture)}, " +
        $"{Exp.ToString("0.##", CultureInfo.InvariantCulture)}%" +
        (IsFinished ? "  ← finished (+10)" : "");

    /// <summary>Reads the panel out of the OCR's lines. Null when the panel is not understood.
    ///
    /// Takes LINES rather than one string because the panel arrives as several — the name row, the
    /// usable-by row, the sell-price row — and which ones carry the numbers is the recogniser's
    /// business, not ours. Everything is joined and searched as one text.
    ///
    /// Null rather than a defaulted record when growth or EXP is missing: the caller's finish test
    /// needs both, so a panel that yielded only one is a panel we did not understand, and answering
    /// "not finished" from a half-read would be inventing the very reading the guard exists to make.
    /// The stage is optional — it is the one value nothing here acts on yet.
    ///
    /// Nothing is anchored to the surrounding characters. The EXP is found by its <c>%</c> (the only
    /// percentage a pet panel shows) and the growth by its <c>+</c>; neither needs the brackets or the
    /// parentheses to have survived, which is deliberate, because those are exactly the glyphs a
    /// stylised game font can lose.</summary>
    public static PetPanel? Parse(IEnumerable<string>? lines)
    {
        if (lines == null) return null;

        var text = Normalise(string.Join(" ", lines));

        // The EXP bar is the anchor, and it is the bracketed percentage — nothing else in a game
        // tooltip carries one. It has to be the anchor, because a +0 pet shows NO growth at all (see
        // below), so "has a +N" cannot be the test for "this is a pet".
        var exp = MatchDouble(ExpPattern, text);
        if (exp == null) return null;

        // The stage, e.g. （6階）. The unit is optional AND may be misread — the measured panel reads
        // "（6踏）" — so a couple of characters are tolerated between the digits and the bracket.
        var stage = MatchInt(StagePattern, text);

        // A MISSING growth is ZERO, and that is measured rather than assumed: a +0 pet renders no
        // "+N" at all. The panel for 真蔚藍米魯 at 0.06 % (live, 2026-09-19) reads
        // "（6踏）真蔚蓝米鲁[0.06%]" — a space where a +7 sits on a levelled pet, with the image
        // crisp enough that nothing was lost. So requiring the +N rejected every pet at the very
        // start of its run, which is the pet this tool is most often feeding.
        //
        // The cost, stated: a +N the OCR *dropped* is also read as +0. For the finished test that is
        // harmless — 0 is never 9 — but it would overstate a remaining-time estimate. Nothing
        // distinguishes the two from one read, and reading zero as a failure would be wrong far more
        // often than reading a dropped +N as zero.
        var growth = MatchInt(GrowthPattern, text) ?? 0;

        return new PetPanel(stage, growth, exp.Value);
    }

    // A full-width digit is a different character to the recogniser and the same number to a reader,
    // and the + is a plausible full-width return too. Folded before matching so a read that is
    // correct but wide does not look like a panel we failed to understand.
    private static string Normalise(string text)
    {
        var sb = new StringBuilder(text.Length);
        foreach (var c in text)
        {
            sb.Append(c switch
            {
                >= '０' and <= '９' => (char)(c - '０' + '0'),
                '＋' => '+',
                '％' => '%',
                _ => c,
            });
        }
        return sb.ToString();
    }

    // Bounded to a plausible stage so a stray long number in a misread row cannot be read as one, and
    // tolerant of up to two characters where the 階 should be — the live panel read "（6踏）".
    private static readonly Regex StagePattern =
        new(@"[（(]\s*(\d{1,2})\s*[^)）]{0,2}[)）]", RegexOptions.Compiled);

    // Not followed by a % — that would be an EXP-shaped number, and the growth is the one with the
    // plus in front of it and nothing behind it.
    private static readonly Regex GrowthPattern =
        new(@"\+\s*(\d{1,2})(?!\s*%)", RegexOptions.Compiled);

    // The opening bracket is required and the closing one is not: it is the opening plus the % that
    // say "EXP bar", and a lost trailing glyph should not cost the whole reading.
    private static readonly Regex ExpPattern =
        new(@"[\[［【]\s*(\d+(?:\.\d+)?)\s*%", RegexOptions.Compiled);

    private static int? MatchInt(Regex re, string text)
    {
        var m = re.Match(text);
        return m.Success && int.TryParse(m.Groups[1].Value, out var v) ? v : null;
    }

    private static double? MatchDouble(Regex re, string text)
    {
        var m = re.Match(text);
        return m.Success &&
               double.TryParse(m.Groups[1].Value, System.Globalization.NumberStyles.Float,
                   System.Globalization.CultureInfo.InvariantCulture, out var v)
            ? v : null;
    }
}

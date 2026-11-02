//! `PetPanel` — what a pet's hover panel says, ported from `v2/SealTools.Core/PetPanel.cs`.
//!
//! Only the three NUMBERS are parsed, and that is the design rather than a shortcut. The measured
//! panel reads `（6）真蔚蓝凤凰+7[52.18%]` against a screen showing `(6階) 真蔚藍鳳凰 +7 [52.18%]`:
//! every digit comes back exact while the Chinese does not, and the recogniser's errors are not
//! uniform — some characters arrive Simplified and are converted by `text_fixes`, others arrive
//! Traditional already, others are simply misread. A parser keyed on any character inherits all of
//! it. The digits and the punctuation around them have shown no such variance.
//!
//! The caller FAILS OPEN (an unparsed panel reloads as normal), so the dangerous bug is a panel that
//! parses WRONG, not one that parses not at all. That is why every pattern here is anchored to a
//! punctuation mark and every bound is one the live game showed.

use std::fmt::Write as _;

/// What a pet's hover panel says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetPanel {
    /// The stage, e.g. `（6階）`. Optional: it is the one value nothing acts on yet, and losing only
    /// it still leaves a usable panel.
    pub stage: Option<i32>,
    /// The growth the pet has reached, `+0` … `+9`.
    pub growth: i32,
    /// The EXP bar's percentage, found by its `%`.
    pub exp: f64,
}

impl PetPanel {
    /// The growth a pet stops at. The game shows `+0` … `+9`, and `+9` is the last of them.
    pub const MAX_GROWTH: i32 = 9;

    /// A full EXP bar.
    pub const FINISHED_EXP: f64 = 100.0;

    /// Whether this pet is done — what the docs call **+10**, which is OUR name for it. The string
    /// appears nowhere in the game, which shows `+9` with a percentage instead, and that gap is why
    /// this test needs BOTH numbers: a `+9` whose bar is still filling is not done, and neither is a
    /// full bar on a `+7`. Either one alone is wrong in one direction.
    ///
    /// EQUALS, not at-least, and that is the fail-safe choice: 9 is the game's maximum, so a growth
    /// above it can only be a misread — and "at least 9" would answer "finished" to a garbage panel,
    /// which is the one direction that makes the tool stop feeding a pet that still needs it. It stays
    /// safe because the growth pattern reads ONE digit, so a panel that reads a digit too many yields
    /// the leading digit rather than a number above the maximum.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.growth == Self::MAX_GROWTH && self.exp >= Self::FINISHED_EXP
    }

    /// For a log line or the Test read. Deliberately not a derived `Debug`: the numbers are the whole
    /// content, and "stage ?" for a stage the OCR missed says so rather than printing a zero that
    /// looks measured.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut line = format!(
            "stage {}, +{}, {}%",
            self.stage
                .map_or_else(|| "?".to_owned(), |stage| stage.to_string()),
            self.growth,
            format_percentage(self.exp),
        );
        if self.is_finished() {
            line.push_str("  ← finished (+10)");
        }
        line
    }

    /// Reads the panel out of the OCR's lines. `None` when the panel is not understood.
    ///
    /// Takes LINES rather than one string because the panel arrives as several — the name row, the
    /// usable-by row, the sell-price row — and which ones carry the numbers is the recogniser's
    /// business, not ours. Everything is joined and searched as one text.
    ///
    /// `None` rather than a defaulted panel when EXP is missing: the caller's finish test needs both
    /// numbers, so a panel that yielded only one is a panel we did not understand, and answering "not
    /// finished" from a half-read would be inventing the very reading the guard exists to make.
    ///
    /// (v2's `Parse` also accepts a null line list and answers null; a `&[String]` cannot be null,
    /// and the empty slice — its Rust spelling — takes the same path.)
    #[must_use]
    pub fn parse(lines: &[String]) -> Option<Self> {
        let text = normalise(&lines.join(" "));
        let characters: Vec<char> = text.chars().collect();

        // The EXP bar is the anchor, and it is the bracketed percentage — nothing else in a game
        // tooltip carries one. It has to be the anchor, because a +0 pet shows NO growth at all, so
        // "has a +N" cannot be the test for "this is a pet".
        let exp = match_percentage(&characters)?;

        // The stage, e.g. （6階）. The unit is optional AND may be misread — the measured panel reads
        // "（6踏）" — so a couple of characters are tolerated between the digits and the bracket.
        let stage = match_stage(&characters);

        // A MISSING growth is ZERO, and that is measured rather than assumed: a +0 pet renders no
        // "+N" at all (live, 2026-09-19 — a space where a +7 sits on a levelled pet, with the image
        // crisp enough that nothing was lost). The cost, stated: a +N the OCR DROPPED is also read as
        // +0. For the finish test that is harmless (0 is never 9); reading zero as a failure would be
        // wrong far more often.
        let growth = match_growth(&characters).unwrap_or(0);

        Some(Self { stage, growth, exp })
    }
}

/// `0.##`: trailing zeros trimmed, no locale. v2's `ToString("0.##", InvariantCulture)`.
fn format_percentage(value: f64) -> String {
    let mut formatted = String::new();
    let _ = write!(formatted, "{value:.2}");
    if formatted.contains('.') {
        while formatted.ends_with('0') {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            formatted.pop();
        }
    }
    formatted
}

/// A full-width digit is a different character to the recogniser and the same number to a reader,
/// and the `+`/`%` are plausible full-width returns too. Folded before matching so a read that is
/// correct but wide does not look like a panel we failed to understand.
fn normalise(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '０'..='９' => {
                char::from_u32(u32::from(character) - u32::from('０') + u32::from('0'))
                    .unwrap_or(character)
            }
            '＋' => '+',
            '％' => '%',
            _ => character,
        })
        .collect()
}

fn skip_whitespace(characters: &[char], from: usize) -> usize {
    let mut at = from;
    while at < characters.len() && characters[at].is_whitespace() {
        at += 1;
    }
    at
}

/// `[（(]\s*(\d{1,2})\s*[^)）]{0,2}[)）]` — the stage row.
fn match_stage(characters: &[char]) -> Option<i32> {
    for start in 0..characters.len() {
        if characters[start] != '（' && characters[start] != '(' {
            continue;
        }

        let digits_start = skip_whitespace(characters, start + 1);
        let mut digits_end = digits_start;
        while digits_end < characters.len()
            && digits_end - digits_start < 2
            && characters[digits_end].is_ascii_digit()
        {
            digits_end += 1;
        }

        // Greedy with backtracking, as the regex is: two digits then one; two tail characters then
        // one then none.
        for taken in (1..=digits_end - digits_start).rev() {
            let after_digits = skip_whitespace(characters, digits_start + taken);
            for tail in (0..=2).rev() {
                let close = after_digits + tail;
                if close >= characters.len() {
                    continue;
                }
                if characters[after_digits..close]
                    .iter()
                    .any(|character| *character == ')' || *character == '）')
                {
                    continue;
                }
                if characters[close] != ')' && characters[close] != '）' {
                    continue;
                }
                let digits: String = characters[digits_start..digits_start + taken]
                    .iter()
                    .collect();
                if let Ok(stage) = digits.parse::<i32>() {
                    return Some(stage);
                }
            }
        }
    }
    None
}

/// `\+\s*(\d)(?!\s*%)` — the growth, anchored on its `+`.
///
/// ONE digit, not two, because the game's growth IS one digit. Allowing two cost the finished test
/// both ways round, and both were measured: a "+9" read as "+99" came back 99 and failed the
/// exact-9 test, and "+15 Days" — the paid extension's duration label — came back 15. With one digit
/// the first reads 9 and the second reads 1; 1 is never 9, so neither can pass the finish test.
fn match_growth(characters: &[char]) -> Option<i32> {
    for start in 0..characters.len() {
        if characters[start] != '+' {
            continue;
        }

        let digit_at = skip_whitespace(characters, start + 1);
        if digit_at >= characters.len() || !characters[digit_at].is_ascii_digit() {
            continue;
        }

        // The negative lookahead: a number followed by a % is an EXP-shaped number, not a growth.
        let after_whitespace = skip_whitespace(characters, digit_at + 1);
        if after_whitespace < characters.len() && characters[after_whitespace] == '%' {
            continue;
        }

        return characters[digit_at]
            .to_digit(10)
            .and_then(|digit| i32::try_from(digit).ok());
    }
    None
}

/// `[\[［【]\s*(\d+(?:\.\d+)?)\s*%` — the EXP bar, anchored on its opening bracket and `%`.
///
/// The opening bracket is required and the closing one is not: it is the opening plus the `%` that
/// say "EXP bar", and a lost trailing glyph should not cost the whole reading.
fn match_percentage(characters: &[char]) -> Option<f64> {
    for start in 0..characters.len() {
        if !matches!(characters[start], '[' | '［' | '【') {
            continue;
        }

        let mut at = skip_whitespace(characters, start + 1);
        let digits_start = at;
        while at < characters.len() && characters[at].is_ascii_digit() {
            at += 1;
        }
        if at == digits_start {
            continue;
        }

        let mut number_end = at;
        if at < characters.len() && characters[at] == '.' {
            let mut fraction_end = at + 1;
            while fraction_end < characters.len() && characters[fraction_end].is_ascii_digit() {
                fraction_end += 1;
            }
            if fraction_end > at + 1 {
                number_end = fraction_end;
            }
        }

        let percent_at = skip_whitespace(characters, number_end);
        if percent_at >= characters.len() || characters[percent_at] != '%' {
            continue;
        }

        let number: String = characters[digits_start..number_end].iter().collect();
        if let Ok(value) = number.parse::<f64>() {
            return Some(value);
        }
    }
    None
}

//! `TextCleaner` — the OCR text-cleanup table from `attributes.yaml`, applied the way v2's
//! `SealTools.Core/TextCleaner.cs` applies it.
//!
//! ORDER IS THE CONTRACT, and it comes from the code rather than from taste:
//!
//! 1. **`whole`** — if the WHOLE trimmed string equals a `from`, return its `to` immediately. Nothing
//!    later runs on the replacement.
//! 2. **`substring`** — in file order. A pure-ASCII `from` is replaced on WORD BOUNDARIES, because a
//!    plain replace turned a correct "GRADE" into "GGRADE" (v1 matched with a `\b` regex); every
//!    other `from` is a plain replace.
//! 3. **`simplified_traditional`** — plain replaces; RapidOCR defaults to simplified Chinese.
//! 4. **`final`** — plain replaces, after the conversion.
//! 5. **`regex`** — the dictionary's own `.NET` regular expressions.
//!
//! Two port decisions worth stating:
//!
//! - **The regular-expression table has no engine behind it here.** The workspace builds offline and
//!   a new dependency was not allowed, so this module honours the shape the committed dictionary
//!   uses — literal text followed by zero or more `(?!…)` negative lookaheads — and refuses any
//!   other pattern AT CONSTRUCTION, naming it. An approximated regex would corrupt an OCR reading
//!   with no trace; a refusal is a loud config error.
//! - **`\b` and `\s` are Unicode, as they are in .NET.** `\w` is approximated as
//!   "alphanumeric or `_`" (letters and digits of any script, which includes CJK), which is what the
//!   dictionary's ASCII fixes actually meet.

use crate::config::{TextFix, TextFixesConfig};
use std::fmt;

/// A fix table entry that the cleaner cannot honour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextFixError {
    /// A replace table has an entry with no `from`. v2 handed that to `string.Replace`, which refuses
    /// a zero-length old value — so it was a crash, not a no-op.
    EmptyFrom { table: &'static str },
    /// A `regex` entry uses a regular-expression feature this port does not implement.
    UnsupportedRegex { fix: TextFix },
}

impl TextFixError {
    pub fn message(&self) -> String {
        match self {
            Self::EmptyFrom { table } => format!(
                "attributes.yaml text_fixes.{table} has an entry with an empty `from`; a replacement \
                 needs something to replace."
            ),
            Self::UnsupportedRegex { fix } => format!(
                "\"{}\" in attributes.yaml text_fixes.regex uses a regular-expression feature this \
                 core does not support. Only literal text with `(?!…)` negative lookaheads is \
                 supported.",
                fix.from
            ),
        }
    }
}

impl fmt::Display for TextFixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for TextFixError {}

/// Applies one fix per scan, as a plain replacement or on word boundaries.
#[derive(Debug, Clone)]
enum Replacement {
    /// A pure-ASCII `from` in the `substring` table: matched only where a word boundary sits on both
    /// sides (`\bfrom\b`).
    WordBounded { from: String, to: String },
    /// Everything else: `string.Replace`, straight through.
    Plain { from: String, to: String },
}

impl Replacement {
    /// `word_bounded` is the SUBSTRING table's rule only: v2 calls `Regex.Replace` for that table and
    /// `string.Replace` for the other three, and the difference is observable for an ASCII entry.
    fn compile(
        table: &'static str,
        fix: &TextFix,
        word_bounded: bool,
    ) -> Result<Self, TextFixError> {
        if fix.from.is_empty() {
            return Err(TextFixError::EmptyFrom { table });
        }
        Ok(if word_bounded && fix.from.is_ascii() {
            Self::WordBounded {
                from: fix.from.clone(),
                to: fix.to.clone(),
            }
        } else {
            Self::Plain {
                from: fix.from.clone(),
                to: fix.to.clone(),
            }
        })
    }

    fn apply(&self, text: &str) -> String {
        match self {
            Self::Plain { from, to } => text.replace(from.as_str(), to),
            Self::WordBounded { from, to } => replace_on_word_boundaries(text, from, to),
        }
    }
}

/// One configured `regex` fix, compiled to what this port can actually honour.
///
/// The pattern grammar is `literal* ( "(" "?!" literal ")" )*`: literal text with zero or more
/// negative lookaheads of literal text. That is exactly the shape the committed dictionary uses, and
/// exactly what the dictionary's comment says the table is for: restoring a dropped trailing stat
/// character where a plain substring replace would corrupt the correct form.
#[derive(Debug, Clone)]
struct RegexFix {
    prefix: String,
    lookaheads: Vec<String>,
    to: String,
}

impl RegexFix {
    fn compile(fix: &TextFix) -> Result<Self, TextFixError> {
        let unsupported = || TextFixError::UnsupportedRegex { fix: fix.clone() };

        let mut prefix = String::new();
        let mut lookaheads = Vec::new();
        let mut rest = fix.from.as_str();
        while !rest.is_empty() {
            if let Some(after_open) = rest.strip_prefix("(?!") {
                let Some(close) = after_open.find(')') else {
                    return Err(unsupported());
                };
                let body = &after_open[..close];
                if body.is_empty() || body.chars().any(is_regex_meta) {
                    return Err(unsupported());
                }
                lookaheads.push(body.to_owned());
                rest = &after_open[close + 1..];
            } else {
                let character = rest.chars().next().expect("rest is non-empty");
                if is_regex_meta(character) {
                    return Err(unsupported());
                }
                prefix.push(character);
                rest = &rest[character.len_utf8()..];
            }
        }

        Ok(Self {
            prefix,
            lookaheads,
            to: fix.to.clone(),
        })
    }

    /// `Regex.Replace(text, pattern, to)`: every non-overlapping match, left to right.
    fn apply(&self, text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        let prefix: Vec<char> = self.prefix.chars().collect();
        let lookaheads: Vec<Vec<char>> = self
            .lookaheads
            .iter()
            .map(|lookahead| lookahead.chars().collect())
            .collect();

        let mut candidates = Vec::new();
        if prefix.is_empty() {
            // A zero-width pattern matches at every boundary; .NET inserts `to` at each of them.
            candidates.extend(0..=chars.len());
        } else if prefix.len() <= chars.len() {
            for start in 0..=chars.len() - prefix.len() {
                if chars[start..start + prefix.len()] == prefix[..] {
                    candidates.push(start);
                }
            }
        }

        let mut cleaned = String::with_capacity(text.len());
        let mut copied = 0;
        for start in candidates {
            if start < copied {
                // Overlaps a match already emitted; .NET resumes after that match.
                continue;
            }
            let end = start + prefix.len();
            let lookaheads_hold = lookaheads
                .iter()
                .all(|lookahead| !starts_at(&chars, end, lookahead));
            if lookaheads_hold {
                cleaned.extend(&chars[copied..start]);
                cleaned.push_str(&self.to);
                copied = end;
            }
        }
        cleaned.extend(&chars[copied..]);
        cleaned
    }
}

fn is_regex_meta(character: char) -> bool {
    matches!(
        character,
        '\\' | '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
    )
}

fn starts_at(chars: &[char], at: usize, needle: &[char]) -> bool {
    at + needle.len() <= chars.len() && chars[at..at + needle.len()] == needle[..]
}

/// "A word character" for `\b`: .NET's `\w` is the Unicode letters, decimal digits, connector
/// punctuation and combining marks. This approximates it with `is_alphanumeric() || '_'`, which
/// covers every character the dictionary's fixes meet — including CJK, so an ASCII fix inside Chinese
/// text does NOT look like it has a boundary.
fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// `Regex.Replace(text, \bfrom\b, to)` for a literal, ASCII `from`, without an engine.
fn replace_on_word_boundaries(text: &str, from: &str, to: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let needle: Vec<char> = from.chars().collect();
    if needle.is_empty() || needle.len() > chars.len() {
        return text.to_owned();
    }

    let mut replaced = String::with_capacity(text.len());
    let mut copied = 0;
    let mut start = 0;
    while start + needle.len() <= chars.len() {
        if chars[start..start + needle.len()] == needle[..] {
            let before_is_word = start > 0 && is_word(chars[start - 1]);
            let after_is_word =
                start + needle.len() < chars.len() && is_word(chars[start + needle.len()]);
            // A word boundary needs a word character on exactly one side of the position.
            let boundary_before = before_is_word != is_word(chars[start]);
            let boundary_after = is_word(needle[needle.len() - 1]) != after_is_word;
            if boundary_before && boundary_after {
                replaced.extend(&chars[copied..start]);
                replaced.push_str(to);
                copied = start + needle.len();
                start = copied;
                continue;
            }
        }
        start += 1;
    }
    replaced.extend(&chars[copied..]);
    replaced
}

/// The compiled form of one dictionary's [`TextFixesConfig`].
#[derive(Debug, Clone)]
pub struct TextCleaner {
    whole: Vec<(String, String)>,
    substring: Vec<Replacement>,
    simplified_traditional: Vec<Replacement>,
    final_rules: Vec<Replacement>,
    regex: Vec<RegexFix>,
}

impl TextCleaner {
    /// Compiles the tables. Fails when an entry cannot be honoured, which is a dictionary problem to
    /// report at load rather than a wrong reading to discover later.
    pub fn new(fixes: &TextFixesConfig) -> Result<Self, TextFixError> {
        let mut substring = Vec::with_capacity(fixes.substring.len());
        for fix in &fixes.substring {
            substring.push(Replacement::compile("substring", fix, true)?);
        }

        let mut simplified_traditional = Vec::with_capacity(fixes.simplified_traditional.len());
        for fix in &fixes.simplified_traditional {
            simplified_traditional.push(Replacement::compile(
                "simplified_traditional",
                fix,
                false,
            )?);
        }

        let mut final_rules = Vec::with_capacity(fixes.final_rules.len());
        for fix in &fixes.final_rules {
            final_rules.push(Replacement::compile("final", fix, false)?);
        }

        let mut regex = Vec::with_capacity(fixes.regex.len());
        for fix in &fixes.regex {
            regex.push(RegexFix::compile(fix)?);
        }

        Ok(Self {
            whole: fixes
                .whole
                .iter()
                .map(|fix| (fix.from.clone(), fix.to.clone()))
                .collect(),
            substring,
            simplified_traditional,
            final_rules,
            regex,
        })
    }

    /// One OCR line in, the text the matcher would see out.
    pub fn clean(&self, text: &str) -> String {
        let cleaned = text.trim().to_owned();

        for (from, to) in &self.whole {
            if cleaned == *from {
                return to.clone();
            }
        }

        let cleaned = self
            .substring
            .iter()
            .fold(cleaned, |text, fix| fix.apply(&text));
        let cleaned = self
            .simplified_traditional
            .iter()
            .fold(cleaned, |text, fix| fix.apply(&text));
        let cleaned = self
            .final_rules
            .iter()
            .fold(cleaned, |text, fix| fix.apply(&text));
        self.regex
            .iter()
            .fold(cleaned, |text, fix| fix.apply(&text))
    }
}

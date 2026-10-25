//! `AttrMatcher` — matching cleaned OCR lines to the attribute dictionary, and deciding whether a
//! roll passes the tuner's filter. Ported from `v2/SealTools.Tuner/AttrMatcher.cs`.
//!
//! The dictionary comes from `attributes.yaml`; nothing here is hardcoded. Two behaviours look like
//! bugs to a reader who does not know why they are there, and both are deliberate:
//!
//! - **An unreadable value still satisfies a bounded rule** ([`value_ok`]). For a rare attribute the
//!   NAME match is the signal, and disqualifying a roll over a number the OCR failed to read would
//!   mean missing one that should have stopped the run. The bounds apply whenever a value WAS read.
//! - **An overflowing digit run is "no value", not an exception.** The captures are unbounded digit
//!   runs, so a long enough one overflows; v2 used `int.Parse` and threw `OverflowException` out of
//!   the matcher where nothing on the path caught it. "No number" is the honest answer, and the first
//!   rule already covers that state.
//!
//! The numeric scans are hand-written rather than regex-driven because this crate has no regex
//! dependency (it builds offline). They follow .NET's definitions of `\d`, `\s` and "the first match"
//! closely enough for this dictionary, and the v2 tests pin the outcomes.

use crate::config::{AttributeDef, AttributesConfig, FilterConfig, FilterRule};
use std::collections::BTreeMap;

/// One OCR line matched to a dictionary entry.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchedAttr {
    pub name: String,
    pub category: String,
    /// The attribute's number, when one was read. `None` is a real state, not a failure (see the
    /// module docs).
    pub value: Option<i32>,
    /// The per-level breakdown, when the attribute is a per-level one and the line said enough.
    pub extra: Option<PerLevelInfo>,
    /// The line's cells, kept for logs and the Test read.
    pub raw: Vec<String>,
}

/// A per-level attribute's interval and stat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerLevelInfo {
    /// The level count from `每 N 等級`.
    pub levels: Option<i32>,
    /// The stat name from [`AttributesConfig::per_level_stats`].
    pub stat: Option<String>,
    /// 1, or 2 when the line marks the doubled variant (`+2`).
    pub bonus: i32,
}

/// One rule the filter consulted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterHit {
    pub rule: String,
    pub found: Option<String>,
    pub value: Option<i32>,
    /// v2's `Override` property; `override` is a Rust keyword.
    pub is_override: bool,
}

/// The filter's verdict, with the reason line the tuner logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterResult {
    pub passed: bool,
    pub reason: String,
    pub hits: Vec<FilterHit>,
}

/// The matcher over one game client's dictionary.
#[derive(Debug, Clone)]
pub struct AttrMatcher {
    attributes: Vec<AttributeDef>,
    per_level_stats: Vec<String>,
}

impl AttrMatcher {
    pub fn new(config: &AttributesConfig) -> Self {
        Self {
            attributes: config.attributes.clone(),
            per_level_stats: config.per_level_stats.clone(),
        }
    }

    /// Matches every OCR line to at most one attribute.
    ///
    /// The line with the highest score wins; a per-level (`plv*`) definition short-circuits the set,
    /// because its variants are single words ("每", "等級", "力量") and its own rule is stricter. A
    /// score of 1 or less is not a match: one character in common is not evidence the OCR read the
    /// word.
    pub fn match_attributes(&self, ocr_lines: &[Vec<String>]) -> Vec<MatchedAttr> {
        let mut results = Vec::new();

        for line in ocr_lines {
            if line.is_empty() {
                continue;
            }
            let combined = line.join(" ");
            let mut best: Option<(&AttributeDef, &String)> = None;
            let mut best_score = 0.0_f64;

            for definition in &self.attributes {
                if definition.category.starts_with("plv") {
                    let Some(stat_variant) = definition.variants.last() else {
                        // A definition with no variants cannot be matched; v2 threw on the index.
                        continue;
                    };
                    if !combined.contains(stat_variant.as_str()) {
                        continue;
                    }
                    if !combined.contains('每') && !combined.contains("等級") {
                        continue;
                    }
                    best = Some((definition, stat_variant));
                    best_score = utf16_len(stat_variant) as f64 + 2.0;
                    break;
                }

                for variant in &definition.variants {
                    let score = if combined.contains(variant.as_str()) {
                        utf16_len(variant) as f64
                    } else {
                        // The fuzzy fallback: the characters are all present but not contiguously,
                        // which is what a misread order looks like. Two characters minimum, because
                        // one character in common means nothing.
                        let needle = normalize(variant);
                        let haystack = normalize(&combined);
                        if utf16_len(&needle) >= 2 && all_chars_in(&needle, &haystack) {
                            utf16_len(&needle) as f64 * 0.8
                        } else {
                            0.0
                        }
                    };
                    if score > best_score {
                        best_score = score;
                        best = Some((definition, variant));
                    }
                }
            }

            let Some((definition, variant)) = best else {
                continue;
            };
            if best_score <= 1.0 {
                continue;
            }

            let mut name = definition.name.clone();
            let mut value = extract_value(line, Some(variant));
            let mut extra = None;
            if definition.category.starts_with("plv") {
                extra = self.extract_per_level(line);
                if let Some(per_level) = &extra {
                    value = per_level.levels;
                    if per_level.bonus == 2 {
                        name = name.replace("+1", "+2");
                    }
                }
            }

            results.push(MatchedAttr {
                name,
                category: definition.category.clone(),
                value,
                extra,
                raw: line.clone(),
            });
        }

        results
    }

    /// `ExtractPerLevel`: the level count, the stat, and the `+2` marker.
    fn extract_per_level(&self, texts: &[String]) -> Option<PerLevelInfo> {
        let combined = texts.join(" ");

        let level_digits = interval_with_unit(&combined).or_else(|| interval_any(&combined));
        let levels = level_digits.and_then(|digits| try_int(&digits));
        let bonus = if has_plus_two(&combined) { 2 } else { 1 };
        let stat = self
            .per_level_stats
            .iter()
            .find(|stat| combined.contains(stat.as_str()))
            .cloned();

        match (levels, stat) {
            (Some(levels), Some(stat)) => Some(PerLevelInfo {
                levels: Some(levels),
                stat: Some(stat),
                bonus,
            }),
            _ => None,
        }
    }

    /// `CheckFilter`: whether a roll passes the tuning filter.
    ///
    /// In `per_attr` mode each rule needs its own COUNT of matching attributes; in `any`/`all` each
    /// rule needs one passing attribute. An override rule short-circuits everything.
    pub fn check_filter(matched: &[MatchedAttr], filter: &FilterConfig) -> FilterResult {
        if !filter.enabled {
            return FilterResult {
                passed: true,
                reason: "filter disabled".to_owned(),
                hits: Vec::new(),
            };
        }

        // Override rules: if any matches the required count, stop immediately.
        for rule in &filter.override_rules {
            let count = count_rule(matched, rule);
            if count >= needed(rule) {
                return FilterResult {
                    passed: true,
                    reason: format!("override: {} x{count}", rule.name),
                    hits: vec![FilterHit {
                        rule: rule.name.clone(),
                        found: None,
                        value: None,
                        is_override: true,
                    }],
                };
            }
        }

        let mode = if filter.match_mode.is_empty() {
            "any"
        } else {
            filter.match_mode.as_str()
        };

        if mode == "per_attr" {
            let mut rule_counts: BTreeMap<&str, i32> = BTreeMap::new();
            let mut hits = Vec::new();

            for attribute in matched {
                for rule in &filter.rules {
                    if attribute.name != rule.name {
                        continue;
                    }
                    if value_ok(attribute.value, rule) {
                        *rule_counts.entry(rule.name.as_str()).or_default() += 1;
                        hits.push(FilterHit {
                            rule: rule.name.clone(),
                            found: Some(attribute.name.clone()),
                            value: attribute.value,
                            is_override: false,
                        });
                        break;
                    }
                }
            }

            for rule in &filter.rules {
                let need = needed(rule);
                let got = rule_counts.get(rule.name.as_str()).copied().unwrap_or(0);
                if got < need {
                    return FilterResult {
                        passed: false,
                        reason: format!("'{}' matched {got} attrs, need {need}", rule.name),
                        hits,
                    };
                }
            }

            return FilterResult {
                passed: true,
                reason: format!("{} attributes matched", hits.len()),
                hits,
            };
        }

        let mut matched_rules = Vec::new();
        for rule in &filter.rules {
            for attribute in matched {
                if attribute.name != rule.name {
                    continue;
                }
                if value_ok(attribute.value, rule) {
                    matched_rules.push(FilterHit {
                        rule: rule.name.clone(),
                        found: Some(attribute.name.clone()),
                        value: attribute.value,
                        is_override: false,
                    });
                    break; // one hit per rule; a later attribute with the name may still pass
                }
            }
        }

        if mode == "any" {
            let passed = !matched_rules.is_empty();
            return FilterResult {
                passed,
                reason: if passed {
                    format!(
                        "matched {}/{} rules",
                        matched_rules.len(),
                        filter.rules.len()
                    )
                } else {
                    "no rules matched".to_owned()
                },
                hits: matched_rules,
            };
        }

        // "all"
        let all_passed = matched_rules.len() >= filter.rules.len();
        FilterResult {
            passed: all_passed,
            reason: if all_passed {
                format!(
                    "matched {}/{} rules",
                    matched_rules.len(),
                    filter.rules.len()
                )
            } else {
                format!(
                    "only matched {}/{}",
                    matched_rules.len(),
                    filter.rules.len()
                )
            },
            hits: matched_rules,
        }
    }
}

/// A rule's count requirement: a non-positive configured count means one.
fn needed(rule: &FilterRule) -> i32 {
    if rule.count > 0 { rule.count } else { 1 }
}

/// DELIBERATE: a null value — the OCR read the attribute NAME but not its number — satisfies the
/// bounds. The name match is the signal for a rare attribute, and a number that failed to read must
/// not disqualify a roll. The bounds apply whenever a value was actually read.
fn value_ok(value: Option<i32>, rule: &FilterRule) -> bool {
    if let (Some(min), Some(value)) = (rule.min, value)
        && value < min
    {
        return false;
    }
    if let (Some(max), Some(value)) = (rule.max, value)
        && value > max
    {
        return false;
    }
    true
}

fn count_rule(matched: &[MatchedAttr], rule: &FilterRule) -> i32 {
    matched
        .iter()
        .filter(|attribute| attribute.name == rule.name && value_ok(attribute.value, rule))
        .count() as i32
}

/// `ExtractValue`: the number on the row, preferring the cell that carries the attribute's variant.
fn extract_value(texts: &[String], preferred: Option<&str>) -> Option<i32> {
    if let Some(matcher) = preferred {
        for raw in texts {
            let text = raw.trim();
            if !text.contains(matcher) {
                continue;
            }
            if let Some(signed) = find_signed_number(text) {
                return Some(signed);
            }
            if let Some(number) = find_any_number(text) {
                return Some(number);
            }
        }
    }

    for raw in texts {
        let text = raw.trim();
        if let Some(signed) = find_signed_number(text) {
            return Some(signed);
        }
        if let Some(number) = find_whole_number(text) {
            return Some(number);
        }
    }
    None
}

/// v2's `Normalize`: strip the noise OCR inserts inside a word, then compare characters.
fn normalize(text: &str) -> String {
    text.trim()
        .chars()
        .filter(|character| *character != '|' && *character != '`' && !character.is_whitespace())
        .collect()
}

fn all_chars_in(needles: &str, haystack: &str) -> bool {
    needles
        .chars()
        .all(|character| haystack.contains(character))
}

/// C# `string.Length`, which is UTF-16 code units — the unit the dictionary's scores were tuned in.
fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// .NET's `\d` is the Unicode decimal-digit category; approximated with `char::is_numeric`, and the
/// conversion to an `int` afterwards only accepts ASCII, exactly like `int.TryParse`.
fn is_decimal_digit(character: char) -> bool {
    character.is_numeric()
}

fn try_int(digits: &str) -> Option<i32> {
    if digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn skip_whitespace(characters: &[char], from: usize) -> usize {
    let mut at = from;
    while at < characters.len() && characters[at].is_whitespace() {
        at += 1;
    }
    at
}

/// `([+-])\s*(\d+)` — the FIRST match, as `Regex.Match` takes it. A value that does not fit an
/// `int` is "no match" for the caller's purposes, which then tries the plain-number scan.
fn find_signed_number(text: &str) -> Option<i32> {
    let characters: Vec<char> = text.chars().collect();
    for start in 0..characters.len() {
        let sign = characters[start];
        if sign != '+' && sign != '-' {
            continue;
        }
        let digits_start = skip_whitespace(&characters, start + 1);
        let mut digits_end = digits_start;
        while digits_end < characters.len() && is_decimal_digit(characters[digits_end]) {
            digits_end += 1;
        }
        if digits_end == digits_start {
            continue;
        }
        let digits: String = characters[digits_start..digits_end].iter().collect();
        let value = try_int(&digits)?;
        return Some(if sign == '+' { value } else { -value });
    }
    None
}

/// `(\d+)` — the first digit run anywhere in the text.
fn find_any_number(text: &str) -> Option<i32> {
    let characters: Vec<char> = text.chars().collect();
    let mut at = 0;
    while at < characters.len() {
        if !is_decimal_digit(characters[at]) {
            at += 1;
            continue;
        }
        let start = at;
        while at < characters.len() && is_decimal_digit(characters[at]) {
            at += 1;
        }
        let digits: String = characters[start..at].iter().collect();
        return try_int(&digits);
    }
    None
}

/// `^\s*(\d+)\s*$` — the whole (already trimmed) text is one number.
fn find_whole_number(text: &str) -> Option<i32> {
    if text.is_empty() || !text.chars().all(is_decimal_digit) {
        return None;
    }
    try_int(text)
}

/// `每\s*(\d+)\s*[等級级]` — the first match.
fn interval_with_unit(text: &str) -> Option<String> {
    let characters: Vec<char> = text.chars().collect();
    for start in 0..characters.len() {
        if characters[start] != '每' {
            continue;
        }
        let digits_start = skip_whitespace(&characters, start + 1);
        let mut digits_end = digits_start;
        while digits_end < characters.len() && is_decimal_digit(characters[digits_end]) {
            digits_end += 1;
        }
        if digits_end == digits_start {
            continue;
        }
        let unit = skip_whitespace(&characters, digits_end);
        if unit < characters.len() && matches!(characters[unit], '等' | '級' | '级') {
            return Some(characters[digits_start..digits_end].iter().collect());
        }
    }
    None
}

/// `每\s*(\d+)` — the second interval pattern, tried only when the first found nothing.
fn interval_any(text: &str) -> Option<String> {
    let characters: Vec<char> = text.chars().collect();
    for start in 0..characters.len() {
        if characters[start] != '每' {
            continue;
        }
        let digits_start = skip_whitespace(&characters, start + 1);
        let mut digits_end = digits_start;
        while digits_end < characters.len() && is_decimal_digit(characters[digits_end]) {
            digits_end += 1;
        }
        if digits_end > digits_start {
            return Some(characters[digits_start..digits_end].iter().collect());
        }
    }
    None
}

/// `[+]\s*2` — the doubled-per-level marker.
fn has_plus_two(text: &str) -> bool {
    let characters: Vec<char> = text.chars().collect();
    for start in 0..characters.len() {
        if characters[start] != '+' {
            continue;
        }
        let digit = skip_whitespace(&characters, start + 1);
        if digit < characters.len() && characters[digit] == '2' {
            return true;
        }
    }
    false
}

/// `OcrEngine.FixSign`: restore the sign the OCR dropped, before matching.
///
/// A negative attribute's number arrives unsigned — the recogniser loses the minus — so the sign is
/// inferred from the LABEL: any label containing one of
/// [`AttributesConfig::negative_attr_markers`] makes the whole line negative. In the other direction
/// an unsigned all-digit value on a positive attribute gains a `+`, so a bonus reads as one.
///
/// The result is the line the matcher sees: the labels first, then the values, in v2's
/// `labels.Concat(result)` order.
pub fn fix_sign(labels: &[String], values: &[String], negative_markers: &[String]) -> Vec<String> {
    let negative = labels
        .iter()
        .any(|label| negative_markers.iter().any(|marker| label.contains(marker)));

    let fixed: Vec<String> = values
        .iter()
        .map(|value| {
            let trimmed = value.trim().trim_start_matches('+').to_owned();
            if negative && !trimmed.starts_with('-') {
                format!("-{trimmed}")
            } else if !negative && trimmed.chars().all(|character| character.is_ascii_digit()) {
                format!("+{trimmed}")
            } else {
                trimmed
            }
        })
        .collect();

    labels.iter().cloned().chain(fixed).collect()
}

//! `AttrMatcher` — translated from `v2/SealTools.Tests/AttrMatcherTests.cs` (the decision-level
//! assertions, verbatim) plus the matcher behaviour that file does not reach.
//!
//! Two behaviours look like bugs to a reader who does not know why they are there, and both are
//! pinned FIRST, so a future "cleanup" has to argue with a failing test rather than quietly change
//! when the tuner stops:
//!
//! - an attribute the OCR read the NAME of but not its VALUE still satisfies a bounded rule;
//! - an overflowing digit run is "no value", not an exception out of the matcher.

use seal_domain::config::{AttributeDef, AttributesConfig, FilterConfig, FilterRule};
use seal_domain::ocr::{AttrMatcher, MatchedAttr};

fn attr(name: &str, value: Option<i32>) -> MatchedAttr {
    MatchedAttr {
        name: name.to_owned(),
        category: String::new(),
        value,
        extra: None,
        raw: Vec::new(),
    }
}

fn rule(name: &str, count: i32, min: Option<i32>, max: Option<i32>) -> FilterRule {
    FilterRule {
        name: name.to_owned(),
        count,
        min,
        max,
    }
}

fn make_filter(enabled: bool, mode: &str, rules: Vec<FilterRule>) -> FilterConfig {
    FilterConfig {
        enabled,
        match_mode: mode.to_owned(),
        require_grade: None,
        rules,
        override_rules: Vec::new(),
    }
}

fn attribute(name: &str, category: &str, variants: &[&str]) -> AttributeDef {
    AttributeDef {
        name: name.to_owned(),
        category: category.to_owned(),
        variants: variants
            .iter()
            .map(|variant| (*variant).to_owned())
            .collect(),
    }
}

fn matcher(attributes: Vec<AttributeDef>) -> AttrMatcher {
    AttrMatcher::new(&AttributesConfig {
        attributes,
        ..AttributesConfig::default()
    })
}

fn lines(lines: &[&[&str]]) -> Vec<Vec<String>> {
    lines
        .iter()
        .map(|line| line.iter().map(|cell| (*cell).to_owned()).collect())
        .collect()
}

// ── the filter (the v2 tests, translated) ──────────────────────────────────────────────────────

/// DELIBERATE, do not "fix" this into a rejection. 減少傷害 is rare enough that the NAME matching is
/// the signal; a value the OCR failed to read must not disqualify a roll that would otherwise stop
/// the run. Bounds still apply whenever a value was actually read — see the next test.
#[test]
fn an_unreadable_value_still_satisfies_a_bounded_rule() {
    let filter = make_filter(
        true,
        "per_attr",
        vec![rule("減少傷害", 2, Some(1), Some(1))],
    );
    let matched = vec![attr("減少傷害", None), attr("減少傷害", None)];

    assert!(AttrMatcher::check_filter(&matched, &filter).passed);
}

/// The other half of that decision: when the value IS read, the bounds are enforced.
#[test]
fn a_read_value_outside_the_bounds_does_not_satisfy_the_rule() {
    let filter = make_filter(
        true,
        "per_attr",
        vec![rule("減少傷害", 2, Some(1), Some(1))],
    );
    let matched = vec![attr("減少傷害", Some(3)), attr("減少傷害", Some(3))];

    assert!(!AttrMatcher::check_filter(&matched, &filter).passed);
}

/// With the filter off, `check_filter` reports passed — so the tuner has to consult `enabled` itself
/// rather than trusting this result. Pinned from this side so the coupling is visible from either
/// file.
#[test]
fn a_disabled_filter_reports_passed() {
    let result = AttrMatcher::check_filter(&[], &make_filter(false, "per_attr", Vec::new()));

    assert!(result.passed);
    assert_eq!(result.reason, "filter disabled");
}

/// The count has to be met by distinct attribute lines, not by one line counted repeatedly.
#[test]
fn too_few_matching_lines_fails_the_rule() {
    let filter = make_filter(
        true,
        "per_attr",
        vec![rule("減少傷害", 2, Some(1), Some(1))],
    );

    let result = AttrMatcher::check_filter(&[attr("減少傷害", Some(1))], &filter);

    assert!(!result.passed);
    assert_eq!(result.reason, "'減少傷害' matched 1 attrs, need 2");
}

/// An override rule short-circuits the whole filter, which is why the null-value behaviour above
/// reaches the stop condition so directly.
#[test]
fn an_override_rule_passes_without_consulting_the_normal_rules() {
    let mut filter = make_filter(true, "all", vec![rule("增加傷害", 3, Some(1), Some(1))]);
    filter.override_rules = vec![rule("減少傷害", 2, Some(1), Some(1))];
    let matched = vec![attr("減少傷害", None), attr("減少傷害", None)];

    let result = AttrMatcher::check_filter(&matched, &filter);

    assert!(result.passed);
    assert!(result.reason.starts_with("override"), "{}", result.reason);
    assert_eq!(result.hits.len(), 1);
    assert!(result.hits[0].is_override);
}

/// The audit's finding, pinned: every other parser uses `TryParse` and this one used `int.Parse` on
/// an unbounded `(\d+)` capture, so ONE OCR line with an 11+ digit run threw `OverflowException` out
/// of the matcher where nothing on the path caught it. An overflowing run is "no value" now.
#[test]
fn an_overflowing_number_is_no_value_rather_than_an_exception() {
    let matcher = matcher(vec![attribute("增加傷害", "attack", &["增加傷害"])]);

    let matched = matcher.match_attributes(&lines(&[&["增加傷害", "99999999999999999999"]]));

    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].name, "增加傷害");
    assert_eq!(matched[0].value, None);
}

#[test]
fn the_any_mode_passes_when_any_rule_matches() {
    let filter = make_filter(
        true,
        "any",
        vec![
            rule("增加傷害", 1, None, None),
            rule("減少傷害", 1, None, None),
        ],
    );

    let result = AttrMatcher::check_filter(&[attr("減少傷害", None)], &filter);

    assert!(result.passed);
    assert_eq!(result.reason, "matched 1/2 rules");

    let none = make_filter(true, "any", vec![rule("增加傷害", 1, None, None)]);
    assert_eq!(
        AttrMatcher::check_filter(&[], &none).reason,
        "no rules matched"
    );
}

#[test]
fn the_all_mode_needs_every_rule_and_says_how_many_it_got() {
    let filter = make_filter(
        true,
        "all",
        vec![
            rule("增加傷害", 1, None, None),
            rule("減少傷害", 1, None, None),
        ],
    );

    let both =
        AttrMatcher::check_filter(&[attr("增加傷害", None), attr("減少傷害", None)], &filter);
    assert!(both.passed);
    assert_eq!(both.reason, "matched 2/2 rules");

    let one = AttrMatcher::check_filter(&[attr("增加傷害", None)], &filter);
    assert!(!one.passed);
    // v2's wording has no trailing "rules" on this branch — the failing line reads "only matched 1/2".
    assert_eq!(one.reason, "only matched 1/2");
}

/// One hit per rule in `any`/`all` mode: the first passing attribute with that name is enough, so a
/// later attribute with the same name that happens to pass does not count twice.
#[test]
fn a_rule_counts_once_in_the_any_and_all_modes() {
    let filter = make_filter(true, "any", vec![rule("減少傷害", 1, Some(1), Some(1))]);
    let matched = vec![attr("減少傷害", Some(1)), attr("減少傷害", Some(1))];

    let result = AttrMatcher::check_filter(&matched, &filter);

    assert!(result.passed);
    assert_eq!(result.hits.len(), 1);
}

// ── matching lines to attribute names ─────────────────────────────────────────────────────────

#[test]
fn an_exact_variant_matches_and_its_value_is_read() {
    let matcher = matcher(vec![attribute("攻擊力", "atk", &["攻擊力", "攻擎力"])]);

    let matched = matcher.match_attributes(&lines(&[&["攻擊力", "+3"]]));

    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].name, "攻擊力");
    assert_eq!(matched[0].category, "atk");
    assert_eq!(matched[0].value, Some(3));
    assert_eq!(matched[0].raw, vec!["攻擊力".to_owned(), "+3".to_owned()]);
}

/// The score gate (`bestScore > 1`): a one-character variant matches by containment with score 1,
/// which is not enough to report an attribute. This is the difference between "the dictionary knows a
/// word" and "the OCR probably read this word".
#[test]
fn a_one_character_containment_is_not_enough_to_match() {
    let matcher = matcher_of(vec![attribute("力", "str", &["力"])]);

    let matched = matcher.match_attributes(&lines(&[&["力量"]]));

    assert!(matched.is_empty());
}

fn matcher_of(attributes: Vec<AttributeDef>) -> AttrMatcher {
    AttrMatcher::new(&AttributesConfig {
        attributes,
        ..AttributesConfig::default()
    })
}

/// The fuzzy fallback: when the contiguous variant is absent, a variant of two or more characters
/// whose characters ALL appear (in any order) scores 80% of its length. That is what keeps a line
/// whose characters were read in the wrong order from being dropped.
#[test]
fn a_garbled_order_matches_through_the_character_containment_fallback() {
    let matcher = matcher_of(vec![attribute("攻擊力", "atk", &["攻擊力"])]);

    let matched = matcher.match_attributes(&lines(&[&["擊攻力"]]));

    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].name, "攻擊力");
    // No number was read, so the value is unknown — not a default.
    assert_eq!(matched[0].value, None);
}

/// `normalize` drops `|`, backticks and all whitespace before the fuzzy comparison, because OCR
/// inserts them inside a word.
#[test]
fn the_fuzzy_fallback_ignores_noise_characters_and_spacing() {
    let matcher = matcher_of(vec![attribute("攻擊力", "atk", &["攻擊力"])]);

    let matched = matcher.match_attributes(&lines(&[&["攻|擊 力"]]));

    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].name, "攻擊力");
}

/// The longest (highest-scoring) variant wins, so a generic short word does not shadow a specific
/// attribute that shares its prefix.
#[test]
fn the_best_scoring_attribute_wins() {
    let matcher = matcher_of(vec![
        attribute("攻擊", "short", &["攻擊"]),
        attribute("攻擊速度", "aspd", &["攻擊速度"]),
    ]);

    let matched = matcher.match_attributes(&lines(&[&["攻擊速度"]]));

    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].name, "攻擊速度");
}

/// The value is read PREFERENTIALLY from the cell that carries the attribute name: a number on that
/// cell wins over a signed number on a neighbour. With no number there, the whole-row pass still
/// finds one — v2 falls back rather than answering "no value".
#[test]
fn the_value_is_read_preferentially_from_the_cell_that_names_the_attribute() {
    let matcher = matcher_of(vec![attribute("減少傷害", "dmg-", &["減少傷害"])]);

    // The naming cell carries 2; the neighbour carries -3. The naming cell wins.
    let preferred = matcher.match_attributes(&lines(&[&["減少傷害 2", "-3"]]));
    assert_eq!(preferred[0].value, Some(2));

    // The naming cell has no number, so the whole-row pass reads the neighbour's signed number.
    let fallback = matcher.match_attributes(&lines(&[&["foo -4", "減少傷害"]]));
    assert_eq!(fallback[0].value, Some(-4));
}

/// A whole-row number counts only when the row cell IS the number; a cell like "bar 9" is not a
/// value (the whole-number pattern is anchored).
#[test]
fn an_unsigned_number_must_be_the_whole_cell() {
    let matcher = matcher_of(vec![attribute("攻擊力", "atk", &["攻擊力"])]);

    let whole_cell = matcher.match_attributes(&lines(&[&["攻擊力", "7"]]));
    assert_eq!(whole_cell[0].value, Some(7));

    let embedded = matcher.match_attributes(&lines(&[&["攻擊力", "bar 9"]]));
    assert_eq!(embedded[0].value, None);
}

/// An ambiguous number is the first signed one, as v2's `Regex.Match` takes the first match.
#[test]
fn the_first_signed_number_wins() {
    let matcher = matcher_of(vec![attribute("攻擊力", "atk", &["攻擊力"])]);

    let matched = matcher.match_attributes(&lines(&[&["攻擊力", "+2", "+5"]]));

    assert_eq!(matched[0].value, Some(2));
}

#[test]
fn empty_lines_are_skipped() {
    let matcher = matcher_of(vec![attribute("攻擊力", "atk", &["攻擊力"])]);

    let matched = matcher.match_attributes(&lines(&[&[], &["攻擊力"]]));

    assert_eq!(matched.len(), 1);
}

// ── per-level attributes ──────────────────────────────────────────────────────────────────────

fn per_level_matcher() -> AttrMatcher {
    AttrMatcher::new(&AttributesConfig {
        attributes: vec![attribute("每級+1力量", "plv_str", &["每", "等級", "力量"])],
        per_level_stats: vec!["力量".to_owned(), "敏捷".to_owned()],
        ..AttributesConfig::default()
    })
}

/// A per-level attribute is recognised where the line says 每…等級 (or 每…) AND names the stat; the
/// LEVEL COUNT is the value, not the number after the attribute.
#[test]
fn a_per_level_attribute_reads_its_level_count() {
    let matched = per_level_matcher().match_attributes(&lines(&[&["每", "5", "等級", "力量"]]));

    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].name, "每級+1力量");
    assert_eq!(matched[0].category, "plv_str");
    assert_eq!(matched[0].value, Some(5));
    let extra = matched[0].extra.as_ref().unwrap();
    assert_eq!(extra.levels, Some(5));
    assert_eq!(extra.stat.as_deref(), Some("力量"));
    assert_eq!(extra.bonus, 1);
}

/// `每 5` without the 等級 unit still reads (v2's second interval pattern).
#[test]
fn the_per_level_interval_unit_is_optional() {
    let matched = per_level_matcher().match_attributes(&lines(&[&["每", "7", "力量"]]));

    assert_eq!(matched[0].value, Some(7));
}

/// The `+2` marking renames the attribute from its `+1` form — the dictionary lists one definition
/// per stat, and the game marks the doubled variant in the text.
#[test]
fn a_plus_two_per_level_attribute_is_renamed() {
    let matched =
        per_level_matcher().match_attributes(&lines(&[&["每", "5", "等級", "+2", "力量"]]));

    assert_eq!(matched[0].name, "每級+2力量");
    assert_eq!(matched[0].extra.as_ref().unwrap().bonus, 2);
    assert_eq!(matched[0].value, Some(5));
}

/// The stat name alone is not a per-level attribute: the line must also say 每 or 等級. This is what
/// keeps a plain 力量 line from being read as the per-level one. v2's branch `continue`s on the whole
/// definition, so the normal variants do not rescue it either.
#[test]
fn a_stat_name_without_the_interval_is_not_per_level() {
    let matched = per_level_matcher().match_attributes(&lines(&[&["力量"]]));

    assert!(matched.is_empty());
}

/// A per-level line with the stat but no parsable level keeps the plain extracted value rather than
/// inventing one; `extra` is then absent.
#[test]
fn a_per_level_line_without_a_level_falls_back_to_the_extracted_value() {
    let matched = per_level_matcher().match_attributes(&lines(&[&["等級", "力量", "+3"]]));

    assert_eq!(matched.len(), 1);
    assert!(matched[0].extra.is_none());
    assert_eq!(matched[0].value, Some(3));
}

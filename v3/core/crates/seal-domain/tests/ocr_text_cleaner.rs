//! `TextCleaner` — v2's `Clean()`, ported with its ORDER and its two recorded quirks.
//!
//! There are no v2 tests for the cleaner itself; the oracle is the code plus the comments that say
//! why each table exists (`TextCleaner.cs`, and PROGRESS.md entry "two live-testing bugs fixed").
//! Every test here pins a decision from that source, not a new expectation:
//!
//! - **Whole first, and it STOPS.** A whole match returns its replacement without letting the later
//!   tables touch it.
//! - **A pure-ASCII substring fix runs on WORD BOUNDARIES.** `\bRADE\b` must not turn a correct
//!   "GRADE" into "GGRADE" — v1 matched with a `\b` regex for exactly this.
//! - **Order matters**: substring → simplified→traditional → final → regex, each table in file order.
//! - **The `regex` table exists for a prefix problem**: a dropped trailing stat character gives a
//!   string that is a PREFIX of the correct one, so a plain substring replace would corrupt the
//!   correct form; the negative lookahead is what makes it safe.

use seal_domain::config::{TextFix, TextFixesConfig};
use seal_domain::ocr::{TextCleaner, TextFixError};

fn fix(from: &str, to: &str) -> TextFix {
    TextFix {
        from: from.to_owned(),
        to: to.to_owned(),
    }
}

/// The committed TW dictionary's `substring` table order is load-bearing: the specific entries come
/// before the generic ones.
fn tw_substring_table() -> TextFixesConfig {
    TextFixesConfig {
        substring: vec![
            fix("攻孽力", "攻擊力"),
            fix("攻孽遠度", "攻擊速度"),
            fix("攻孽", "攻擊"),
            fix("RADE", "GRADE"),
            fix("4&", "46"),
        ],
        ..TextFixesConfig::default()
    }
}

#[test]
fn empty_tables_are_the_identity_on_trimmed_text() {
    let cleaner = TextCleaner::new(&TextFixesConfig::default()).unwrap();

    assert_eq!(cleaner.clean("  abc  "), "abc");
    assert_eq!(cleaner.clean(""), "");
}

/// A whole-string fix is tried on the TRIMMED text and returns immediately: nothing later may touch
/// its replacement. This is what keeps "GRADB 8" from becoming "GRADE 8" via the substring table.
#[test]
fn a_whole_fix_replaces_the_whole_string_and_stops() {
    let config = TextFixesConfig {
        whole: vec![fix("GRADB 8", "GRADE :"), fix("GRADB", "GRADE :")],
        substring: vec![fix("GRADB", "GRADE")],
        ..TextFixesConfig::default()
    };
    let cleaner = TextCleaner::new(&config).unwrap();

    assert_eq!(cleaner.clean("GRADB 8"), "GRADE :");
    assert_eq!(cleaner.clean("  GRADB  "), "GRADE :");
    // Not a whole match, so the substring table runs instead.
    assert_eq!(cleaner.clean("GRADB 9"), "GRADE 9");
}

/// And the replacement itself is not fed back through the later tables — the whole rule is
/// "replace and STOP".
#[test]
fn a_whole_replacement_is_not_processed_again() {
    let config = TextFixesConfig {
        whole: vec![fix("ABC", "GRADB")],
        substring: vec![fix("GRADB", "GRADE")],
        ..TextFixesConfig::default()
    };
    let cleaner = TextCleaner::new(&config).unwrap();

    assert_eq!(cleaner.clean("ABC"), "GRADB");
}

/// The recorded reason for `\b`: "GRADE" contains "RADE", and a plain replace would corrupt the
/// correct read into "GGRADE".
#[test]
fn an_ascii_substring_fix_is_word_bounded() {
    let cleaner = TextCleaner::new(&tw_substring_table()).unwrap();

    assert_eq!(cleaner.clean("GRADE"), "GRADE");
    assert_eq!(cleaner.clean("RADE"), "GRADE");
    assert_eq!(cleaner.clean(" RADE "), "GRADE");
    assert_eq!(cleaner.clean("(RADE)"), "(GRADE)");
    // A word character on either side is not a boundary: the fix does not fire.
    assert_eq!(cleaner.clean("RADER"), "RADER");
    // .NET's \w includes Unicode letters, so a CJK neighbour is a word character too and there is no
    // boundary before/after the ASCII run. Ported as-is: this table's fixes rely on the same rule.
    assert_eq!(cleaner.clean("攻擊RADE"), "攻擊RADE");
    assert_eq!(cleaner.clean("RADE攻擊"), "RADE攻擊");
}

/// The boundary rule applies to the trailing side as well, and a non-word character is not a word:
/// "4&" only fires when a word character follows it. That follows from `\b` on `4&\b`, and it is a
/// quirk of the committed table rather than a choice made here.
#[test]
fn a_trailing_non_word_character_needs_a_word_character_after_it() {
    let cleaner = TextCleaner::new(&tw_substring_table()).unwrap();

    assert_eq!(cleaner.clean("4&7"), "467");
    assert_eq!(cleaner.clean("x 4& y"), "x 4& y");
}

/// A non-ASCII fix is a plain replace — no boundaries — and the table order decides which one wins.
#[test]
fn a_non_ascii_substring_fix_replaces_plainly_in_table_order() {
    let cleaner = TextCleaner::new(&tw_substring_table()).unwrap();

    assert_eq!(cleaner.clean("攻孽力"), "攻擊力");
    // The specific entry runs before the generic "攻孽"; reversed, this would read "攻擊遠度".
    assert_eq!(cleaner.clean("攻孽遠度"), "攻擊速度");
    // The generic entry still catches anything else.
    assert_eq!(cleaner.clean("攻孽命中"), "攻擊命中");
}

/// The table order is substring, then simplified→traditional, then final, then regex.
#[test]
fn the_tables_apply_in_the_order_the_code_applies_them() {
    let config = TextFixesConfig {
        substring: vec![fix("A", "B")],
        simplified_traditional: vec![fix("B", "C")],
        final_rules: vec![fix("C", "D")],
        ..TextFixesConfig::default()
    };
    let cleaner = TextCleaner::new(&config).unwrap();

    assert_eq!(cleaner.clean("A"), "D");
}

/// The simplified→traditional pass is a plain replace and runs before `final`, which is why a `final`
/// entry can correct what the conversion produced.
#[test]
fn the_conversion_runs_before_the_final_corrections() {
    let config = TextFixesConfig {
        simplified_traditional: vec![fix("减少", "減少"), fix("等级", "等級")],
        final_rules: vec![fix("減少害", "減少傷害")],
        ..TextFixesConfig::default()
    };
    let cleaner = TextCleaner::new(&config).unwrap();

    assert_eq!(cleaner.clean("减少害"), "減少傷害");
    assert_eq!(cleaner.clean("等级"), "等級");
}

/// The regex table's reason for existing: a dropped TRAILING stat character leaves a prefix of the
/// correct string, so a plain substring replace would corrupt the correct form. The negative
/// lookahead is the whole safety.
#[test]
fn the_regex_fixes_restore_a_dropped_trailing_character_only_when_it_is_missing() {
    let config = TextFixesConfig {
        regex: vec![
            fix("增加幸(?!運)", "增加幸運"),
            fix("增加敏(?!捷)", "增加敏捷"),
            fix("增加智(?!力)", "增加智力"),
            fix("增加體(?!力)", "增加體力"),
            fix("增加精(?!神)", "增加精神"),
            fix("增加力(?!量)", "增加力量"),
        ],
        ..TextFixesConfig::default()
    };
    let cleaner = TextCleaner::new(&config).unwrap();

    // Truncated: restored.
    assert_eq!(cleaner.clean("增加幸"), "增加幸運");
    assert_eq!(cleaner.clean("裝備增加敏"), "裝備增加敏捷");
    assert_eq!(cleaner.clean("增加體3"), "增加體力3");
    // Correct already: untouched, and NOT doubled into "增加幸運運".
    assert_eq!(cleaner.clean("增加幸運"), "增加幸運");
    assert_eq!(cleaner.clean("增加敏捷"), "增加敏捷");
    assert_eq!(cleaner.clean("增加力量"), "增加力量");
    // Nothing is anchored to the ends: the fix works mid-string.
    assert_eq!(cleaner.clean("+增加力34"), "+增加力量34");
}

/// v2 applies a whole-string fix with `string.Replace` and a regex fix with `Regex.Replace`; both
/// replace EVERY occurrence, not just the first.
#[test]
fn every_occurrence_is_replaced() {
    let config = TextFixesConfig {
        simplified_traditional: vec![fix("级", "級")],
        regex: vec![fix("增加幸(?!運)", "增加幸運")],
        ..TextFixesConfig::default()
    };
    let cleaner = TextCleaner::new(&config).unwrap();

    assert_eq!(cleaner.clean("1级2级"), "1級2級");
    assert_eq!(cleaner.clean("增加幸。增加幸"), "增加幸運。增加幸運");
}

/// An entry with an empty replacement DELETES its `from`. `to` is a replacement, not a requirement;
/// v2 would have thrown on a null here, and the decode repairs that to "".
#[test]
fn a_fix_with_no_replacement_deletes_its_from() {
    let config = TextFixesConfig {
        substring: vec![fix("攻", "")],
        ..TextFixesConfig::default()
    };
    let cleaner = TextCleaner::new(&config).unwrap();

    assert_eq!(cleaner.clean("攻擊"), "擊");
}

// ── what cannot be honoured is REFUSED, at construction, rather than mis-applied ────────────────

/// The port has no regular-expression engine (no new dependency was allowed), so it honours the
/// shape the committed dictionary uses — literal text with `(?!…)` negative lookaheads — and refuses
/// anything else BY NAME. A silently approximated fix would be a wrong OCR reading with no trace.
#[test]
fn a_regex_fix_outside_the_supported_shape_is_refused_with_its_pattern() {
    for unsupported in [
        "增加.*",
        "增加\\s+力",
        "(?<=增加)力量",
        "[力量]+",
        "增加力(?!.*量)",
    ] {
        let config = TextFixesConfig {
            regex: vec![fix(unsupported, "增加力量")],
            ..TextFixesConfig::default()
        };
        let err = TextCleaner::new(&config).unwrap_err();
        assert!(
            matches!(err, TextFixError::UnsupportedRegex { .. }),
            "{unsupported:?} must be refused, got {err:?}"
        );
        assert!(
            err.message().contains(unsupported),
            "the refusal must name the pattern, got {:?}",
            err.message()
        );
    }
}

/// An empty `from` is a crash in v2 (`string.Replace` refuses a zero-length old value), so it is
/// refused at construction rather than silently doing nothing on every scan.
#[test]
fn an_empty_from_in_a_replace_table_is_refused_with_the_table_name() {
    let config = TextFixesConfig {
        substring: vec![fix("", "X")],
        ..TextFixesConfig::default()
    };
    let err = TextCleaner::new(&config).unwrap_err();
    assert!(matches!(
        err,
        TextFixError::EmptyFrom { table: "substring" }
    ));

    let config = TextFixesConfig {
        simplified_traditional: vec![fix("", "X")],
        ..TextFixesConfig::default()
    };
    assert!(matches!(
        TextCleaner::new(&config).unwrap_err(),
        TextFixError::EmptyFrom {
            table: "simplified_traditional"
        }
    ));

    let config = TextFixesConfig {
        final_rules: vec![fix("", "X")],
        ..TextFixesConfig::default()
    };
    assert!(matches!(
        TextCleaner::new(&config).unwrap_err(),
        TextFixError::EmptyFrom { table: "final" }
    ));
}

#[test]
fn an_empty_whole_fix_is_not_an_error_because_v2_compares_instead_of_replacing() {
    let config = TextFixesConfig {
        whole: vec![fix("", "EMPTY")],
        ..TextFixesConfig::default()
    };
    let cleaner = TextCleaner::new(&config).unwrap();

    assert_eq!(cleaner.clean("abc"), "abc");
    assert_eq!(cleaner.clean("   "), "EMPTY");
}

/// The shipped US dictionary has no fix tables at all; the cleaner must construct and be the
/// identity, because every other tool runs on the US client with that dictionary.
#[test]
fn the_empty_dictionary_cleans_nothing() {
    let cleaner = TextCleaner::new(&TextFixesConfig::default()).unwrap();
    assert_eq!(cleaner.clean("GRADB  8"), "GRADB  8");
}

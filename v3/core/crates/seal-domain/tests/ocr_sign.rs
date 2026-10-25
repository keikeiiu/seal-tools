//! Sign fixing, translated from `OcrEngine.FixSign` and its `negative_attr_markers` table.
//!
//! The OCR drops the minus sign on a negative attribute — `減少傷害 3` comes back with the number
//! unsigned — so the sign is inferred from the attribute's LABEL: a label in
//! `negative_attr_markers` means the value is negative. The other direction matters too: an unsigned
//! plain number on a positive attribute gains a `+` so the value reads as an explicit bonus.
//!
//! This is the pure half of the pipeline that feeds the matcher: cleaner → sign fixing → matcher.

use seal_domain::config::AttributesConfig;
use seal_domain::ocr::fix_sign;

fn markers() -> Vec<String> {
    vec![
        "減少".to_owned(),
        "限制等級".to_owned(),
        "配戴限制".to_owned(),
    ]
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// The negative case: the minus the OCR lost is put back, and the existing one is not doubled.
#[test]
fn a_negative_label_makes_an_unsigned_value_negative() {
    let fixed = fix_sign(&owned(&["減少傷害"]), &owned(&["3"]), &markers());
    assert_eq!(fixed, owned(&["減少傷害", "-3"]));

    let already_signed = fix_sign(&owned(&["減少傷害"]), &owned(&["-3"]), &markers());
    assert_eq!(already_signed, owned(&["減少傷害", "-3"]));
}

/// The `+` an OCR read on a negative attribute is wrong and is replaced by the minus.
#[test]
fn a_negative_label_replaces_an_ocr_plus_with_a_minus() {
    let fixed = fix_sign(&owned(&["減少傷害"]), &owned(&["+3"]), &markers());
    assert_eq!(fixed, owned(&["減少傷害", "-3"]));
}

/// The positive case: a bare number gains its `+`; a signed value is left as it is.
#[test]
fn a_positive_label_marks_an_unsigned_number_with_a_plus() {
    let fixed = fix_sign(&owned(&["攻擊力"]), &owned(&["3"]), &markers());
    assert_eq!(fixed, owned(&["攻擊力", "+3"]));

    let signed = fix_sign(&owned(&["攻擊力"]), &owned(&["-3"]), &markers());
    assert_eq!(signed, owned(&["攻擊力", "-3"]));
}

/// Every whitespace-carrying and plus-prefixed spelling is trimmed before the sign is decided.
#[test]
fn the_value_is_trimmed_and_a_leading_plus_is_consumed() {
    let fixed = fix_sign(&owned(&["攻擊力"]), &owned(&["  3 ", " +4"]), &markers());
    assert_eq!(fixed, owned(&["攻擊力", "+3", "+4"]));
}

/// The label row is carried through in front of the values: the result is the line the matcher sees.
#[test]
fn the_labels_are_kept_and_come_first() {
    let fixed = fix_sign(&owned(&["攻擊力", "減少"]), &owned(&["3"]), &markers());
    // ANY label containing a marker makes the whole line negative, exactly as v2's `.Any(...)` does.
    assert_eq!(fixed, owned(&["攻擊力", "減少", "-3"]));
}

/// The markers are a config value, not a constant: an empty table leaves bare numbers bare.
#[test]
fn an_empty_marker_table_means_nothing_is_negative() {
    let config = AttributesConfig::default();
    assert!(config.negative_attr_markers.is_empty());

    let fixed = fix_sign(
        &owned(&["減少傷害"]),
        &owned(&["3"]),
        &config.negative_attr_markers,
    );
    // No marker matches, so the positive rule applies and a bare number gains its plus.
    assert_eq!(fixed, owned(&["減少傷害", "+3"]));
}

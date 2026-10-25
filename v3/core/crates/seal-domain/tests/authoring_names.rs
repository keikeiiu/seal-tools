//! Name rules from the launcher's `PresetPicker` (v2.16) and `PresetCopy.cs`.
//!
//! The picker serves the Quest flow, Quest sequence and Spammer preset lists. It owns the rules that
//! are the same everywhere: a refused empty or duplicate name, and a generated copy name checked
//! against every name in play. v2 wrote it once because the three hand-rolled copies each had their
//! own bugs — the rules are pure, so they belong in the domain rather than in a widget.

use seal_domain::authoring::{PresetNoun, copy_name, validate_name};

/// "No name is taken yet", as a named constant so the generic parameter can infer its item type.
const NO_NAMES: [&str; 0] = [];

#[test]
fn a_name_is_trimmed_and_returned_as_it_will_be_stored() {
    assert_eq!(
        validate_name(PresetNoun::Flow, "  nightly  ", None, NO_NAMES).unwrap(),
        "nightly"
    );
}

#[test]
fn an_empty_or_blank_name_is_refused_with_the_nouns_word() {
    for blank in ["", "   ", "\t"] {
        let err = validate_name(PresetNoun::Flow, blank, None, NO_NAMES).unwrap_err();
        assert_eq!(err.message(), "A flow needs a name.");
    }

    // The message names the thing the player is looking at, because the same rule serves three lists.
    let err = validate_name(PresetNoun::Sequence, "", None, NO_NAMES).unwrap_err();
    assert_eq!(err.message(), "A sequence needs a name.");
    let err = validate_name(PresetNoun::Preset, "", None, NO_NAMES).unwrap_err();
    assert_eq!(err.message(), "A preset needs a name.");
}

#[test]
fn a_name_already_in_play_is_refused_and_named() {
    let err = validate_name(PresetNoun::Flow, "red", None, ["red", "blue"]).unwrap_err();
    assert_eq!(
        err.message(),
        "There is already a flow called \"red\" — pick another name."
    );
}

/// v2's `Dictionary.Contains(name)` is case-sensitive, unlike the copy-name generator below: a name
/// the player TYPES is compared exactly, and "Mine" beside "mine" is allowed. That asymmetry is
/// deliberate in the port — the picker sorts ignore-case, so the generated-name rule has to refuse a
/// case-only difference, while a typed name is an explicit choice.
#[test]
fn a_typed_name_is_compared_exactly() {
    assert_eq!(
        validate_name(PresetNoun::Flow, "Mine", None, ["mine"]).unwrap(),
        "Mine"
    );
}

/// Renaming a flow to its own name is a no-op that succeeds — the picker closes the prompt rather
/// than reporting a duplicate of the name being renamed.
#[test]
fn renaming_a_name_to_itself_is_a_no_op_that_succeeds() {
    assert_eq!(
        validate_name(
            PresetNoun::Flow,
            "nightly",
            Some("nightly"),
            ["nightly", "other"]
        )
        .unwrap(),
        "nightly"
    );
}

#[test]
fn renaming_onto_another_name_is_refused() {
    let err = validate_name(
        PresetNoun::Flow,
        "other",
        Some("nightly"),
        ["nightly", "other"],
    )
    .unwrap_err();
    assert!(err.message().contains("\"other\""));
    assert!(err.message().contains("already"));
}

// ── generated copy names (`PresetCopy.CopyName`, pinned by v2's PresetCopyTests) ─────────────────

fn copied_from(source: &str, taken: &[&str]) -> String {
    copy_name(source, taken.iter().copied())
}

#[test]
fn the_first_copy_is_named_copy() {
    assert_eq!(copied_from("mine", &["mine"]), "mine (copy)");
}

#[test]
fn the_count_rises_until_the_name_is_free() {
    assert_eq!(
        copied_from("mine", &["mine", "mine (copy)"]),
        "mine (copy 2)"
    );
    assert_eq!(
        copied_from("mine", &["mine", "mine (copy)", "mine (copy 2)"]),
        "mine (copy 3)"
    );
}

/// The suffix is stripped before counting: copying the copy must not produce
/// "mine (copy) (copy)" — a name that grows every time and says nothing.
#[test]
fn copying_a_copy_continues_the_count() {
    assert_eq!(
        copied_from("mine (copy)", &["mine (copy)"]),
        "mine (copy 2)"
    );
}

/// And it must not walk BACKWARDS: with "mine (copy)" free and the source at "mine (copy 2)",
/// restarting the count would hand back a name that sorts above the preset it was copied from.
#[test]
fn copying_a_numbered_copy_does_not_walk_backwards() {
    assert_eq!(
        copied_from("mine (copy 2)", &["mine", "mine (copy)", "mine (copy 2)"]),
        "mine (copy 3)"
    );
}

/// The pickers sort ignore-case, so "mine (copy)" beside "Mine (copy)" is two rows the player cannot
/// tell apart. The dictionaries are keyed case-sensitively and would accept both, so the GENERATED
/// name is the one place that has to refuse.
#[test]
fn a_name_that_only_differs_by_case_is_treated_as_taken() {
    assert_eq!(
        copied_from("mine", &["mine", "Mine (Copy)"]),
        "mine (copy 2)"
    );
}

#[test]
fn a_look_alike_suffix_is_left_alone_because_the_digits_are_what_make_it_ours() {
    assert_eq!(
        copied_from("chapter (copy x)", &["chapter (copy x)"]),
        "chapter (copy x) (copy)"
    );
    // "(copy 0)" is not a suffix we wrote — the count starts at 1 — so it is a literal name.
    assert_eq!(
        copied_from("mine (copy 0)", &["mine (copy 0)"]),
        "mine (copy 0) (copy)"
    );
}

/// The parsed index is an int32 in v2 and its counter used to be one too, so `int.MaxValue` wrapped
/// to `int.MinValue` and named the copy "(copy -2147483648)". The port counts in i64, and the pin
/// stays so a later "simplification" back to i32 fails here.
#[test]
fn the_copy_count_does_not_overflow_at_the_integer_boundary() {
    let name = copied_from("mine (copy 2147483647)", &["mine (copy 2147483647)"]);

    assert_eq!(name, "mine (copy 2147483648)");
    assert!(!name.contains('-'));
}

//! The launcher's other inline rules, as pure functions: hotkey names (`ParseVk`/`VkName`), the
//! game-variant switch (`AppConfig.ApplyVariant` + `LauncherService.SwitchVariant`), and Setup's
//! blank-client-size refusal.
//!
//! The client-size rule does not exist in v2 — PROGRESS.md entry 63 records the defect it fixes
//! ("Save Setup accepts a blank client size, writing `{0,0}` … a false warning the UI itself
//! created"), and `PLAN-V3-CORE.md` §4.1 names the refusal as a rule that moves into the domain.

use seal_domain::authoring::{
    ClientSize, ClientSizeError, ClientSizeField, HotkeyError, HotkeyNames, VariantError,
    parse_hotkeys, parse_virtual_key, plan_variant_switch, resolve_window_title,
    validate_client_size, virtual_key_name,
};
use seal_domain::config::GameConfig;

// ── hotkeys (`MainWindow.ParseVk` / `VkName`) ────────────────────────────────────────────────────

#[test]
fn function_keys_are_accepted_up_to_the_vk_tables_f24() {
    assert_eq!(parse_virtual_key("F1").unwrap(), 0x70);
    assert_eq!(parse_virtual_key("F12").unwrap(), 0x7B);
    assert_eq!(parse_virtual_key("F24").unwrap(), 0x87);

    // The F prefix is case-insensitive HERE — unlike the board's key spelling, where `SendKey` is
    // case-sensitive. Hotkey names and board keys are two questions with different answers.
    assert_eq!(parse_virtual_key("f5").unwrap(), 0x74);
    assert_eq!(parse_virtual_key("  F2  ").unwrap(), 0x71);
}

#[test]
fn the_named_keys_are_accepted_with_both_spellings() {
    let pairs = [
        ("Esc", 0x1B),
        ("Escape", 0x1B),
        ("CapsLock", 0x14),
        ("Caps", 0x14),
        ("Space", 0x20),
        ("Tab", 0x09),
        ("Enter", 0x0D),
        ("Return", 0x0D),
        ("Backspace", 0x08),
    ];

    for (name, vk) in pairs {
        assert_eq!(parse_virtual_key(name).unwrap(), vk, "name {name}");
    }
}

#[test]
fn a_single_letter_or_digit_is_its_uppercase_virtual_key() {
    assert_eq!(parse_virtual_key("a").unwrap(), 0x41);
    assert_eq!(parse_virtual_key("Q").unwrap(), 0x51);
    assert_eq!(parse_virtual_key("5").unwrap(), 0x35);
}

#[test]
fn a_name_that_is_not_a_key_is_refused_with_what_the_names_can_be() {
    for bad in ["", "   ", "F25", "F0", "hello", "Shift", "ab", "0x41"] {
        let err = parse_virtual_key(bad).unwrap_err();
        let message = err.message();
        assert!(
            message.contains("F1") && message.contains("Esc"),
            "message for {bad:?} should list the accepted names, got {message:?}"
        );
    }

    // A bare "F" is the LETTER F, not a nameless function key: v2's fallback for any single
    // character runs after the F1–F24 branch fails, so this quirk is part of the ported rule.
    assert_eq!(parse_virtual_key("F").unwrap(), 0x46);
    assert_eq!(parse_virtual_key("f").unwrap(), 0x46);
}

/// The two halves are inverses for every value the config can hold: the Hotkeys tab shows
/// `VkName`, the player edits it, and Save parses it back. A name that does not round-trip is a
/// setting that changes itself the first time the tab is saved.
#[test]
fn a_name_parses_back_to_the_same_virtual_key() {
    let representable = [
        0x70u32, 0x7B, 0x87, // F1, F12, F24
        0x1B, 0x14, 0x20, 0x09, 0x0D, 0x08, // the named keys
        0x41, 0x35, 0x21, 0x7E, // letters, digits, punctuation
    ];

    for vk in representable {
        let name = virtual_key_name(vk);
        assert_eq!(parse_virtual_key(&name).unwrap(), vk, "vk {vk:#X} → {name}");
    }
}

#[test]
fn a_virtual_key_with_no_name_reads_as_its_hex_code() {
    assert_eq!(virtual_key_name(0xF1), "0xF1");
    assert_eq!(virtual_key_name(0x01), "0x1");
}

#[test]
fn all_four_hotkeys_are_parsed_together_or_none_are() {
    let hotkeys = parse_hotkeys(HotkeyNames {
        start: "F5",
        quit: "Esc",
        advance_grade: "F11",
        pause: "Space",
    })
    .unwrap();

    assert_eq!(hotkeys.start, 0x74);
    assert_eq!(hotkeys.quit, 0x1B);
    assert_eq!(hotkeys.advance_grade, 0x7A);
    assert_eq!(hotkeys.pause, 0x20);
}

/// v2's Save refuses the whole set with one line and writes nothing. The port keeps the refusal but
/// names EACH bad field, because "one of the names isn't a key" over four boxes is a hunt.
#[test]
fn an_invalid_hotkey_names_the_field_and_nothing_is_written() {
    let err = parse_hotkeys(HotkeyNames {
        start: "F5",
        quit: "hello",
        advance_grade: "F11",
        pause: "Space",
    })
    .unwrap_err();

    assert_eq!(err.len(), 1);
    assert_eq!(err[0].field(), "quit");
    assert!(matches!(err[0], HotkeyError::Invalid { .. }));
    assert!(err[0].message().contains("hello"), "{}", err[0].message());
}

#[test]
fn every_bad_hotkey_is_reported_not_just_the_first() {
    let err = parse_hotkeys(HotkeyNames {
        start: "nope",
        quit: "Esc",
        advance_grade: "F11",
        pause: "nope too",
    })
    .unwrap_err();

    let fields: Vec<&str> = err.iter().map(HotkeyError::field).collect();
    assert_eq!(fields, vec!["start", "pause"]);
}

// ── game variants (`AppConfig.ApplyVariant` + `LauncherService.SwitchVariant`) ─────────────────

fn shipped_game() -> GameConfig {
    GameConfig {
        variant: "tw".to_owned(),
        ..GameConfig::default()
    }
}

/// The Setup tab's switch, resolved BEFORE anything is committed: a known name yields the variant,
/// the title and the dictionary to load, and it changes nothing itself. The caller loads the
/// dictionary between the two — a config pointing at a variant whose dictionary is missing is not a
/// degraded launcher, it is one that throws in its constructor.
#[test]
fn selecting_a_variant_resolves_it_and_hands_back_its_dictionary() {
    let game = shipped_game();

    let switch = plan_variant_switch(&game, "us").unwrap();

    assert_eq!(switch.variant, "us");
    assert_eq!(switch.window_title, "US_LIVE");
    assert_eq!(switch.dictionary, "attributes.us.yaml");
    // The plan changes nothing: the caller commits only after the dictionary has loaded.
    assert_eq!(game.variant, "tw");
}

/// Case is the variants map's business, not the player's: a hand-typed "TW" must work.
#[test]
fn a_hand_typed_name_works_regardless_of_case_or_padding() {
    let game = shipped_game();

    let switch = plan_variant_switch(&game, "  TW  ").unwrap();
    assert_eq!(switch.variant, "TW");
    assert_eq!(switch.window_title, "TW_LIVE");
    assert_eq!(switch.dictionary, "attributes.yaml");
}

/// An unknown name changes NOTHING and lists the known ones — the message v2 puts on the Setup tab.
#[test]
fn an_unknown_variant_is_refused_and_names_the_known_clients() {
    let game = shipped_game();

    let err = plan_variant_switch(&game, "nope").unwrap_err();

    assert!(matches!(err, VariantError::Unknown { .. }));
    assert_eq!(
        err.message(),
        "\"nope\" is not a known game variant. Known: tw, us."
    );
}

#[test]
fn a_blank_variant_name_is_unknown_rather_than_silently_ignored() {
    let game = shipped_game();

    let err = plan_variant_switch(&game, "   ").unwrap_err();
    assert_eq!(
        err.message(),
        "\"\" is not a known game variant. Known: tw, us."
    );
}

/// A variant whose dictionary entry is blank falls back to the TW dictionary, exactly as
/// `ConfigLoader.LoadAttributes(null)` does — a config saved before a client grew its own dictionary
/// must still start.
#[test]
fn a_variant_with_no_dictionary_falls_back_to_the_default_one() {
    let mut game = shipped_game();
    game.variants.get_mut("us").unwrap().attributes = String::new();

    assert_eq!(
        plan_variant_switch(&game, "us").unwrap().dictionary,
        "attributes.yaml"
    );
}

/// The load-time rule: an unset or UNKNOWN variant changes nothing. Unset is the shape of every
/// config written before variants existed; unknown is a typo. Blanking the title would send every
/// tool hunting a window called "".
#[test]
fn the_active_variant_resolves_the_window_title_but_only_when_it_is_known() {
    let mut game = shipped_game();
    assert_eq!(resolve_window_title(&game, "configured"), "TW_LIVE");

    game.variant = String::new();
    assert_eq!(resolve_window_title(&game, "configured"), "configured");

    game.variant = "nope".to_owned();
    assert_eq!(resolve_window_title(&game, "configured"), "configured");

    // v2's guard is `{ Title.Length: > 0 }`: a variant with an empty title does not blank the target.
    game.variant = "us".to_owned();
    game.variants.get_mut("us").unwrap().title = String::new();
    assert_eq!(resolve_window_title(&game, "configured"), "configured");
}

// ── Setup's client size (the v2.16 defect PLAN-V3-CORE §4.1 closes) ────────────────────────────

#[test]
fn a_real_client_size_is_accepted_and_trimmed() {
    assert_eq!(
        validate_client_size("2865", "1789").unwrap(),
        ClientSize {
            width: 2865,
            height: 1789
        }
    );
    assert_eq!(
        validate_client_size("  2865 ", " +1789 ").unwrap(),
        ClientSize {
            width: 2865,
            height: 1789
        }
    );
}

/// The defect: Save Setup used to write `{0,0}`, which later made the tool warn "measured on a 0×0
/// client, recalibrate" — a false warning the UI itself created. A blank box is refused now, and the
/// refusal says what to do instead.
#[test]
fn a_blank_client_size_is_refused_instead_of_writing_zeros() {
    let err = validate_client_size("", "1789").unwrap_err();
    assert!(matches!(
        err,
        ClientSizeError::Blank {
            field: ClientSizeField::Width
        }
    ));
    assert!(err.message().contains("Detect"), "{}", err.message());

    let err = validate_client_size("2865", "   ").unwrap_err();
    assert!(matches!(
        err,
        ClientSizeError::Blank {
            field: ClientSizeField::Height
        }
    ));
}

#[test]
fn a_client_size_that_is_not_a_number_names_the_box_and_the_text() {
    let err = validate_client_size("wide", "1789").unwrap_err();

    assert!(matches!(err, ClientSizeError::NotANumber { .. }));
    assert_eq!(err.field(), "width");
    assert!(err.message().contains("wide"), "{}", err.message());
}

#[test]
fn a_zero_or_negative_client_size_is_refused_with_the_reason() {
    for bad in ["0", "-1"] {
        let err = validate_client_size("2865", bad).unwrap_err();
        assert!(matches!(err, ClientSizeError::NotPositive { .. }), "{bad}");
        assert_eq!(err.field(), "height");
        assert!(
            err.message().contains("greater than 0"),
            "{}",
            err.message()
        );
    }
}

//! The wire grammar: one command per line, '\n'-terminated, exactly as
//! `arduino/seal_mouse/seal_mouse.ino` reads it.
//!
//! This is pinned BYTE FOR BYTE because the firmware's parser is `buf[0]` plus `sscanf`: an extra
//! space, a `\r` (which a text-mode serial layer could add), or a rounded count is a different
//! command. The `D` counts in particular are RAW HID counts (`DESIGN.md` §2.1, `MOVE-SETS.md`): the
//! grammar does not scale, round, or clamp them — the caller measured them per machine, and the
//! closed-loop placement computes its own corrections.

use seal_board::command::{BoardCommand, CommandError};

#[test]
fn the_simple_commands_are_one_letter_and_a_newline() {
    let cases = [
        (BoardCommand::LeftClick, "C\n"),
        (BoardCommand::RightClick, "R\n"),
        (BoardCommand::Enter, "E\n"),
        (BoardCommand::HoldLeft, "L\n"),
        (BoardCommand::ReleaseLeft, "l\n"),
        (BoardCommand::Version, "V\n"),
    ];

    for (command, expected) in cases {
        assert_eq!(command.encode(), expected, "wire text for {command:?}");
    }
}

#[test]
fn a_move_keeps_its_raw_counts_exactly() {
    // The counts are measured per machine and are never DPI-scaled, so every one of these must come
    // out unchanged — including the ones a scaling/rounding layer would mangle.
    let cases = [
        ((0, 0), "D 0 0\n"),
        ((1, -1), "D 1 -1\n"),
        ((100, 0), "D 100 0\n"),
        ((599, -601), "D 599 -601\n"),
        ((1200, -2400), "D 1200 -2400\n"),
        ((-9600, 42), "D -9600 42\n"),
    ];

    for ((dx, dy), expected) in cases {
        assert_eq!(BoardCommand::move_by(dx, dy).encode(), expected);
    }
}

#[test]
fn keys_are_the_letter_with_the_character() {
    assert_eq!(
        BoardCommand::key('a', false).expect("sendable").encode(),
        "K a\n"
    );
    assert_eq!(
        BoardCommand::key('7', true).expect("sendable").encode(),
        "k 7\n"
    );
    // Printable ASCII, including space and the punctuation the game's shortcuts use.
    for character in [' ', '$', 'Z', '~'] {
        let encoded = BoardCommand::key(character, false)
            .expect("printable ASCII")
            .encode();
        assert_eq!(encoded, format!("K {character}\n"));
    }
}

#[test]
fn function_keys_are_one_through_twelve() {
    assert_eq!(
        BoardCommand::function_key(1, false).expect("F1").encode(),
        "F 1\n"
    );
    assert_eq!(
        BoardCommand::function_key(12, true).expect("F12").encode(),
        "f 12\n"
    );

    for number in [0, 13, 99] {
        let error = BoardCommand::function_key(number, false).expect_err("not a function key");
        assert!(
            error.to_string().contains("F1–F12"),
            "the refusal names the range: {error}"
        );
    }
}

#[test]
fn wheel_notches_are_one_through_the_firmware_ceiling() {
    assert_eq!(
        BoardCommand::wheel(true, 1).expect("one notch").encode(),
        "Q 1\n"
    );
    assert_eq!(
        BoardCommand::wheel(false, 400)
            .expect("the ceiling")
            .encode(),
        "Z 400\n"
    );

    for notches in [0, 401] {
        let error = BoardCommand::wheel(true, notches).expect_err("out of range");
        assert!(
            error.to_string().contains("1–400"),
            "the refusal names the range: {error}"
        );
    }
}

#[test]
fn a_key_that_cannot_be_sent_is_refused_rather_than_sent() {
    for character in ['\n', '\r', '\t', '\0', 'é', '中'] {
        let error = BoardCommand::key(character, false).expect_err("not one printable ASCII byte");
        assert!(
            error.to_string().contains("printable ASCII"),
            "the refusal says what a key may be: {error}"
        );
    }
}

#[test]
fn every_command_is_exactly_one_newline_terminated_line() {
    let commands = [
        BoardCommand::LeftClick,
        BoardCommand::RightClick,
        BoardCommand::move_by(-1234, 5678),
        BoardCommand::Enter,
        BoardCommand::key('q', false).expect("sendable"),
        BoardCommand::function_key(12, true).expect("F12"),
        BoardCommand::wheel(false, 60).expect("in range"),
        BoardCommand::HoldLeft,
        BoardCommand::ReleaseLeft,
        BoardCommand::Version,
    ];

    for command in commands {
        let encoded = command.encode();
        assert!(
            encoded.ends_with('\n'),
            "{encoded:?} must end in a newline: the sketch reads until '\\n'"
        );
        assert_eq!(
            encoded.matches('\n').count(),
            1,
            "{encoded:?} must be exactly one line"
        );
        assert!(
            !encoded.contains('\r'),
            "{encoded:?} must not carry a carriage return — the sketch would see it as the argument"
        );
    }
}

#[test]
fn the_errors_are_sentences_for_the_card() {
    let error = BoardCommand::key('\n', false).expect_err("refused");
    // A `Display` that reads as a sentence, so a caller can put it on the card unchanged.
    assert!(matches!(&error, CommandError { .. }));
    assert!(error.to_string().ends_with('.'), "{error}");
}

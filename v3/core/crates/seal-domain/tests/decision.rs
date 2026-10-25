//! `Decision` — the vocabulary a tool's pure decision half returns and its loop executes
//! (`PLAN-V3-CORE.md` §6.4).
//!
//! The point of the type is testability: `decide(read, cfg) -> Decision` needs no board, no screen
//! and no game, so the interesting half of each tool can be pinned by tests instead of by a manual
//! run. The loop is the only thing that performs effects.

use seal_domain::decision::{Decision, Key, KeyError, StopKind, StopReason};
use seal_domain::geometry::Point;

#[test]
fn the_four_decisions_carry_their_payload() {
    assert_eq!(
        Decision::Click(Point::new(30, 45)),
        Decision::click(Point::new(30, 45))
    );
    assert_eq!(
        Decision::Key(Key::character('5').unwrap()),
        Decision::key('5').unwrap()
    );
    assert_eq!(Decision::Wait(0.25), Decision::wait(0.25));

    let stop = Decision::stop(StopReason::done("DONE — 4 row(s)"));
    assert_eq!(stop.stop_reason().unwrap().kind, StopKind::Done);
    assert_eq!(stop.stop_reason().unwrap().message, "DONE — 4 row(s)");
}

#[test]
fn only_stop_ends_the_run() {
    assert!(!Decision::Wait(1.0).is_terminal());
    assert!(!Decision::Click(Point::new(0, 0)).is_terminal());
    assert!(!Decision::key('q').unwrap().is_terminal());
    assert!(Decision::stop(StopReason::cancelled("Stopped.")).is_terminal());
}

/// The board's `K` handler presses ONE printable ASCII character (`SkillSpammer.SendKey`, the
/// firmware's `C`/`R`/`E`/`K` vocabulary). A key that cannot be sent must be refused where it is
/// built rather than sent and dropped on the board, where nothing watches for the silence.
#[test]
fn a_single_printable_ascii_character_is_a_key() {
    assert_eq!(Key::parse("5").unwrap(), Key::character('5').unwrap());
    assert_eq!(Key::parse("q").unwrap(), Key::character('q').unwrap());
    assert_eq!(Key::parse("Q").unwrap(), Key::character('Q').unwrap());
    // Space is printable and the firmware can press it; holdspace uses it for real.
    assert_eq!(Key::parse(" ").unwrap(), Key::character(' ').unwrap());
    assert_eq!(Key::parse("~").unwrap(), Key::character('~').unwrap());
}

/// F1–F12 are the firmware's table. `ParseVk` accepts up to F24 for HOTKEYS — a different question,
/// with a different answer — and the spammer refuses F13 at the board instead of sending it into a
/// silent no-op (`SkillSpammer.SendKey` says exactly this).
#[test]
fn function_keys_stop_at_f12_like_the_firmware_table() {
    assert_eq!(Key::parse("F1").unwrap(), Key::function(1).unwrap());
    assert_eq!(Key::parse("F12").unwrap(), Key::function(12).unwrap());

    for too_high in ["F13", "F24", "F0", "F99"] {
        assert!(
            matches!(Key::parse(too_high), Err(KeyError::Unsendable { .. })),
            "{too_high} must be refused, got {:?}",
            Key::parse(too_high)
        );
    }
}

/// `int.TryParse` accepts a padded number, so v2 sends "F 1" for a config that says F01. Refusing it
/// here would refuse a key the board would have accepted.
#[test]
fn a_padded_function_number_parses_like_v2() {
    assert_eq!(Key::parse("F01").unwrap(), Key::function(1).unwrap());
}

#[test]
fn keys_that_cannot_be_sent_are_refused_with_a_player_facing_message() {
    for unsendable in ["", "ab", "f1", "é", "\n", "*1", "K 5"] {
        let err = match Key::parse(unsendable) {
            Err(err) => err,
            Ok(key) => panic!("{unsendable:?} must be refused, parsed as {key:?}"),
        };
        let message = err.message();
        assert!(
            message.contains("printable") || message.contains("F1"),
            "message for {unsendable:?} should say what a key can be, got {message:?}"
        );
    }
}

/// The stop vocabulary is the protocol's `tool.finished.reason` (`PROTOCOL.md` §5), so the core can
/// put these straight into that event instead of translating a second enum.
#[test]
fn the_stop_kinds_are_the_protocols_finished_reasons() {
    assert_eq!(serde_json::to_value(StopKind::Done).unwrap(), "done");
    assert_eq!(
        serde_json::to_value(StopKind::Cancelled).unwrap(),
        "cancelled"
    );
    assert_eq!(serde_json::to_value(StopKind::Error).unwrap(), "error");
    assert_eq!(serde_json::to_value(StopKind::Guard).unwrap(), "guard");

    let decoded: StopKind = serde_json::from_value(serde_json::json!("cancelled")).unwrap();
    assert_eq!(decoded, StopKind::Cancelled);
}

#[test]
fn every_stop_kind_keeps_the_message_the_player_reads() {
    let reasons = [
        StopReason::done("DONE — 4 row(s)"),
        StopReason::cancelled("Stopped."),
        StopReason::error("OCR failed: no model file"),
        StopReason::guard("The mouse moved — the run was stopped."),
    ];

    for reason in reasons {
        assert!(
            !reason.message.is_empty(),
            "{reason:?} needs a player-facing message"
        );
    }

    assert_eq!(StopReason::error("boom").kind, StopKind::Error);
}

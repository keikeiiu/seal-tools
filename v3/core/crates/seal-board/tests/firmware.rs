//! Firmware level detection: `V` is the only thing the sketch writes back, so silence is an answer.
//!
//! Ported from `FirmwareVersionTests.cs` (parse table, both message families verbatim) plus the
//! `LauncherService.QueryFirmwareVersion` behaviour: two attempts, each `V` followed by a 1500 ms
//! read window, and a board that never parses as a version is `None` — the DEFINITE NO that
//! `FoodLoadMode.board_can_drag` rests on.

use seal_board::firmware::{
    self, CURRENT_LEVEL, DRAG_LEVEL, FIRMWARE_READ_SECONDS, Firmware, parse,
};
use seal_board::transport::{FakeTransport, LineRead};

#[test]
fn the_level_is_read_from_the_reply() {
    assert_eq!(parse("V 1"), Some(1));
    assert_eq!(parse("V 2"), Some(2));
    assert_eq!(parse("  V   3  "), Some(3));
    assert_eq!(parse("V\t3"), Some(3));
    assert_eq!(parse("V 007"), Some(7));
}

#[test]
fn anything_that_is_not_an_unambiguous_level_is_none() {
    // The v2 table, verbatim: the cases that must not be mistaken for a level.
    for line in [
        "",
        "   ",
        "V",
        "V 1 extra",
        "v 1",
        "W 1",
        "V x",
        "V -1",
        "V 0",
        "1",
        "V +1",
        "V 1.5",
    ] {
        assert_eq!(parse(line), None, "{line:?} is not a version reply");
    }
}

#[test]
fn the_silence_says_what_it_means() {
    let firmware = Firmware::silent();

    assert!(firmware.describe().contains("predates"));
    assert!(!firmware.can_drag(), "silence is a definite no");
    assert!(
        firmware.summary().contains("drag"),
        "the player-facing line says what it costs: {}",
        firmware.summary()
    );
}

#[test]
fn a_mismatch_says_which_way_it_goes() {
    let older = firmware::describe(Some(1));
    let newer = firmware::describe(Some(3));

    assert!(older.contains("older"), "{older}");
    assert!(newer.contains("newer"), "{newer}");
    assert!(firmware::describe(Some(2)).contains("current"));
    assert!(!firmware::describe(Some(2)).contains("older"));
    assert!(!firmware::describe(Some(2)).contains("newer"));
}

#[test]
fn the_player_facing_summary_never_falls_back_to_the_number() {
    let cannot_drag = firmware::summary(Some(1)).to_lowercase();
    assert!(cannot_drag.contains("reflash"), "{cannot_drag}");
    assert!(!cannot_drag.contains("protocol level"), "{cannot_drag}");
    assert!(
        firmware::summary(None).contains("silently do nothing"),
        "the failure is silent, so the sentence must say so"
    );
    assert!(firmware::summary(Some(2)).contains("Up to date"));
    assert!(firmware::summary(Some(3)).contains("Newer"));
    assert!(!firmware::summary(Some(3)).contains("Up to date"));
}

#[test]
fn the_levels_are_the_ones_the_sketch_and_domain_agree_on() {
    assert_eq!(CURRENT_LEVEL, 2, "the sketch's FW_VERSION");
    assert_eq!(DRAG_LEVEL, 2, "the level that added L/l");
    // The domain half-owns the drag rule; the board half must agree with it rather than restate it.
    assert!(seal_domain::tools::pet::food_load::board_can_drag(Some(
        DRAG_LEVEL as i32
    )));
    assert!(!seal_domain::tools::pet::food_load::board_can_drag(Some(
        DRAG_LEVEL as i32 - 1
    )));
}

#[test]
fn a_reply_answers_and_reports_the_window_it_waited() {
    let mut transport = FakeTransport::with_reads(vec![LineRead::Line("V 2".to_owned())]);

    let firmware = firmware::detect(&mut transport);

    assert_eq!(firmware.reported(), Some(2));
    assert_eq!(firmware.attempts(), 1);
    assert_eq!(transport.writes(), vec!["V\n"]);
    assert_eq!(transport.deadlines(), vec![FIRMWARE_READ_SECONDS]);
    assert!(firmware.can_drag());
}

#[test]
fn silence_is_retried_once_then_read_as_no_version_at_all() {
    let mut transport = FakeTransport::with_reads(vec![LineRead::Silent, LineRead::Silent]);

    let firmware = firmware::detect(&mut transport);

    assert_eq!(firmware.reported(), None);
    assert_eq!(firmware.attempts(), 2);
    assert_eq!(transport.writes(), vec!["V\n", "V\n"]);
    assert_eq!(
        transport.deadlines(),
        vec![FIRMWARE_READ_SECONDS, FIRMWARE_READ_SECONDS],
        "the window has to reach past the sketch's 3 s setup() on a just-flashed board"
    );
    assert!(!firmware.can_drag());
}

#[test]
fn a_slow_board_is_answered_on_the_second_attempt() {
    // A just-flashed board can miss the first V while setup()'s 3 s delay runs; the reply is late,
    // not lost — so the second attempt is the one that matters.
    let mut transport =
        FakeTransport::with_reads(vec![LineRead::Silent, LineRead::Line("V 2".to_owned())]);

    let firmware = firmware::detect(&mut transport);

    assert_eq!(firmware.reported(), Some(2));
    assert_eq!(firmware.attempts(), 2);
}

#[test]
fn garbage_is_not_a_version_even_on_the_second_attempt() {
    for garbage in ["hello", "V", "V 0", "W 3"] {
        let mut transport = FakeTransport::with_reads(vec![
            LineRead::Line(garbage.to_owned()),
            LineRead::Line(garbage.to_owned()),
        ]);

        let firmware = firmware::detect(&mut transport);

        assert_eq!(
            firmware.reported(),
            None,
            "{garbage:?} must not be read as a level"
        );
        assert_eq!(firmware.attempts(), 2);
    }
}

#[test]
fn garbage_then_a_real_reply_is_retried_and_answered() {
    let mut transport = FakeTransport::with_reads(vec![
        LineRead::Line("V 1 extra".to_owned()),
        LineRead::Line("V 2".to_owned()),
    ]);

    let firmware = firmware::detect(&mut transport);

    assert_eq!(firmware.reported(), Some(2));
    assert_eq!(firmware.attempts(), 2);
}

#[test]
fn a_read_failure_is_retried_like_silence_because_v2_could_not_tell_them_apart() {
    // `LauncherService.ReadLineQuietly` catches everything and returns null: a timeout and a port
    // error both become "no version". The next attempt's write is where a dead port reports itself.
    let mut transport = FakeTransport::with_reads(vec![
        LineRead::Failed {
            message: "the port went away".to_owned(),
        },
        LineRead::Line("V 2".to_owned()),
    ]);

    let firmware = firmware::detect(&mut transport);

    assert_eq!(firmware.reported(), Some(2));
    assert_eq!(firmware.attempts(), 2);
}

#[test]
fn a_write_failure_stops_at_once_because_there_is_no_board_to_ask() {
    let mut transport = FakeTransport::new();
    transport.fail_writes("the port is not open");

    let firmware = firmware::detect(&mut transport);

    assert_eq!(firmware.reported(), None);
    assert_eq!(
        firmware.attempts(),
        1,
        "v2 returns null on the first write failure"
    );
    assert!(!firmware.can_drag());
    assert!(firmware.describe().contains("predates"));
}

#[test]
fn the_drag_rule_is_the_domains_not_a_second_copy() {
    // The board crate reports the level; whether a level can drag is `FoodLoadMode`'s rule. This
    // pins the seam: a board at DragLevel can drag, one below cannot, silence cannot.
    assert!(Firmware::reporting(DRAG_LEVEL).can_drag());
    assert!(!Firmware::reporting(DRAG_LEVEL - 1).can_drag());
    assert!(!Firmware::silent().can_drag());
}

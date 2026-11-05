//! The firmware sketch guard, ported from `FirmwareSketchTests.cs`: the sketch's protocol level and
//! its command set are pinned TOGETHER, against the real file in the repo.
//!
//! The file is read from the checkout (`arduino/seal_mouse/seal_mouse.ino`), found by walking up from
//! the test binary rather than by a brittle relative path — a test that silently passed because it
//! could not find the sketch would be worse than no test. Read-only; the test writes nothing.
//!
//! What this cannot see, said plainly (v2 said it too): it pins the COMMANDS, not the behaviour. A
//! timing constant or a changed failsafe alters the board without touching the command set.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use seal_board::firmware::{CURRENT_LEVEL, DRAG_LEVEL};

/// The sketch's protocol level. Bump the sketch, extend its level table, and update this pin and
/// `CURRENT_LEVEL` in the same commit — then tag it `fw<n>`.
const PINNED_LEVEL: u32 = 3;

/// Every command the sketch dispatches, sorted — 20 at level 2.
const PINNED_COMMANDS: &str = "CDEFHKLPQRSTUVWXZfkl";

/// Walks up from the test binary to the repository root that holds the sketch. Panics with the path
/// it started from when nothing is found, because a silently-skipped guard is worse than a failure.
fn sketch_source() -> String {
    let start = Path::new(env!("CARGO_MANIFEST_DIR"));
    for directory in start.ancestors() {
        let candidate = directory
            .join("arduino")
            .join("seal_mouse")
            .join("seal_mouse.ino");
        if candidate.is_file() {
            return std::fs::read_to_string(&candidate)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", candidate.display()));
        }
    }
    panic!(
        "arduino/seal_mouse/seal_mouse.ino was not found above {}",
        start.display()
    );
}

fn sketch_level(source: &str) -> u32 {
    let marker = "#define FW_VERSION";
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("the sketch has no `{marker}`"));
    source[start + marker.len()..]
        .split_whitespace()
        .next()
        .and_then(|digits| digits.parse().ok())
        .unwrap_or_else(|| panic!("FW_VERSION is not a number in {source:.0}"))
}

/// Every `type == '<letter>'` the sketch dispatches, sorted and deduplicated.
fn sketch_commands(source: &str) -> String {
    let mut commands = BTreeSet::new();
    let marker = "type == '";
    let mut rest = source;
    while let Some(start) = rest.find(marker) {
        rest = &rest[start + marker.len()..];
        if let Some(letter) = rest.chars().next()
            && rest[..=letter.len_utf8()].ends_with('\'')
        {
            commands.insert(letter);
        }
    }
    commands.into_iter().collect()
}

#[test]
fn the_level_and_the_command_set_are_pinned_together() {
    // Both halves in ONE test on purpose: the failure worth catching is one changing WITHOUT the
    // other, and two separate tests would each stay green in exactly that case.
    let source = sketch_source();

    assert_eq!(sketch_level(&source), PINNED_LEVEL);
    assert_eq!(
        sketch_commands(&source),
        PINNED_COMMANDS,
        "the sketch's command set changed — bump FW_VERSION in the sketch and this pin together"
    );
}

#[test]
fn every_command_is_still_dispatched_in_the_shape_this_pin_can_see() {
    let source = sketch_source();

    assert!(
        !source.contains("switch (type") && !source.contains("switch(type"),
        "commands moved into a switch — the pinning above can no longer see them"
    );
    assert!(source.contains("type == 'V'"));
}

#[test]
fn the_hosts_level_constants_match_the_sketch() {
    let source = sketch_source();

    assert_eq!(CURRENT_LEVEL, sketch_level(&source));
    assert!(
        DRAG_LEVEL <= sketch_level(&source),
        "the host claims dragging arrives at a level the sketch has never reported"
    );
    assert!(source.contains("type == 'L'") && source.contains("type == 'l'"));
}

#[test]
fn the_hosting_test_knows_where_the_sketch_is() {
    // The lookup is the part that can rot silently: a moved sketch would fail every assertion above
    // for the wrong reason, and the message would point at the firmware.
    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = start
        .ancestors()
        .map(|directory| directory.join("arduino/seal_mouse/seal_mouse.ino"))
        .find(|candidate| candidate.is_file());
    assert!(
        path.is_some(),
        "the sketch lookup found nothing above {}",
        start.display()
    );
}

#[test]
fn the_sketch_still_sends_the_version_reply_this_crate_parses() {
    let source = sketch_source();

    // The reply shape the host parses is "V <n>": keep it that way, and keep the failsafe and the
    // host-gone release the level-1 promise rests on.
    assert!(
        source.contains("Serial.print(\"V \")"),
        "the V reply's shape is what the host parses"
    );
    // The flags are tested before `!Serial`: ArduinoCore-avr's `operator bool()` sleeps 10 ms on
    // every loop pass otherwise, which delays every command even when nothing is held (HW-F6).
    assert!(
        source.contains("if ((spaceHeld || leftHeld)")
            && source.contains("(uint32_t)(millis() - lastRx) > HOST_SILENCE_MS"),
        "the held-key failsafe must cover a HUNG host, not only a closed port (HW-F7)"
    );
}

#[test]
fn a_long_frame_can_be_aborted_and_the_watchdog_keeps_its_budget() {
    // The board must not need the host to wait out a ten-second wheel frame (HW-F7). This is a
    // source pin because the behaviour lives on the board.
    let source = sketch_source();

    assert!(
        source.contains("#define HOST_SILENCE_MS 3000"),
        "the watchdog threshold is part of the published behaviour"
    );
    assert_eq!(
        source.matches("Serial.peek() == '!'").count(),
        2,
        "both the move walk and the wheel loop must check for the abort byte"
    );
    assert!(
        source.contains("void releaseAll()"),
        "both failsafe paths release through one function"
    );
}

#[test]
fn the_move_frame_is_parsed_wide_bounded_and_never_split_into_two_commands() {
    // An AVR `int` is 16 bits: the old `%d` parse wrapped a large frame, and the old fixed buffer
    // turned an overlong line's tail into a fresh command whose first byte picked the action
    // (HW-F11). These are source pins because the behaviour lives on the board; the host's own
    // range refusal is pinned by `tests/command.rs`.
    let source = sketch_source();

    assert!(
        source.contains("#define MOVE_MAX 9999"),
        "the sketch must bound one move frame"
    );
    assert!(
        source.contains("sscanf(buf, \"D %ld %ld\""),
        "the move counts must be parsed as 32-bit longs, not 16-bit ints"
    );
    assert!(
        source.contains("dx >= -MOVE_MAX && dx <= MOVE_MAX")
            && source.contains("dy >= -MOVE_MAX && dy <= MOVE_MAX"),
        "an out-of-range move is refused, never wrapped"
    );
    assert!(
        source.contains("if (n == sizeof(buf) - 1 && buf[n - 1] != '\\n')"),
        "an overlong line must be discarded to its newline instead of parsed as two commands"
    );
}

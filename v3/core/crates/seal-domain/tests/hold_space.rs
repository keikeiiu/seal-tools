//! `HoldSpace`'s decision half — the AUTHORED oracle (v2 has no test for `HoldSpace.cs`).
//!
//! Each test cites the v2 line it encodes: `v2/SealTools.Spammer/HoldSpace.cs`. If a test and that
//! file disagree, the file is right.

use seal_domain::tools::hold_space::{
    Beep, HOLD_BEEP, HoldSpaceAction, HoldSpaceTick, HoldSpaceWhy, RELEASE_BEEP, cleanup,
    hold_failed, release_failed, tick,
};

fn idle_tick() -> HoldSpaceTick {
    tick(false, false, false, false)
}

/// Nothing moving: the loop keeps its memory and writes nothing (`HoldSpace.cs:37-68`, the no-branch
/// tick).
#[test]
fn an_idle_tick_does_nothing() {
    let decided = idle_tick();

    assert_eq!(
        decided.actions,
        [] as [seal_domain::tools::hold_space::HoldSpaceStep; 0]
    );
    assert!(!decided.mirror_running);
    assert_eq!(decided.panel_running, None);
    assert!(!decided.clear_quit);
}

/// `HoldSpace.cs:43-50` — the launcher says the tool started: hold, log, beep, and clear the pending
/// quit.
#[test]
fn the_launcher_starting_holds_the_spacebar() {
    let decided = tick(false, true, false, false);

    assert_eq!(
        decided.actions,
        [seal_domain::tools::hold_space::HoldSpaceStep {
            action: HoldSpaceAction::Hold,
            why: HoldSpaceWhy::Panel,
            log: "[Panel] HOLDING space",
            beep: HOLD_BEEP,
        }]
    );
    assert!(decided.mirror_running);
    assert!(decided.clear_quit);
    // The card is already correct; the launcher's mirror is what triggered this, so the tick does not
    // ask the loop to write it back.
    assert_eq!(decided.panel_running, None);
}

/// `HoldSpace.cs:51-57` — the launcher says the tool stopped: release with the falling beep.
#[test]
fn the_launcher_stopping_releases_the_spacebar() {
    let decided = tick(true, false, false, false);

    assert_eq!(decided.actions.len(), 1);
    let step = decided.actions[0];
    assert_eq!(step.action, HoldSpaceAction::Release);
    assert_eq!(step.why, HoldSpaceWhy::Panel);
    assert_eq!(step.log, "[Panel] RELEASED space");
    assert_eq!(step.beep, RELEASE_BEEP);
    assert!(!decided.mirror_running);
    assert!(!decided.clear_quit);
}

/// `HoldSpace.cs:59-66` — the hotkey's RISING edge while idle starts a run AND makes the card follow.
#[test]
fn the_hotkey_rising_while_idle_starts_and_tells_the_launcher() {
    let decided = tick(false, false, true, false);

    assert_eq!(decided.actions.len(), 1);
    let step = decided.actions[0];
    assert_eq!(step.action, HoldSpaceAction::Hold);
    assert_eq!(step.why, HoldSpaceWhy::Hotkey);
    assert_eq!(step.log, "[GO] holding space");
    assert_eq!(step.beep, HOLD_BEEP);
    assert!(decided.mirror_running);
    assert_eq!(decided.panel_running, Some(true));
    assert!(decided.clear_quit);
}

/// The same edge while running stops it, and the card follows.
#[test]
fn the_hotkey_rising_while_running_stops_and_tells_the_launcher() {
    let decided = tick(true, true, true, false);

    assert_eq!(decided.actions.len(), 1);
    let step = decided.actions[0];
    assert_eq!(step.action, HoldSpaceAction::Release);
    assert_eq!(step.why, HoldSpaceWhy::Hotkey);
    assert_eq!(step.log, "[STOP] released");
    assert!(!decided.mirror_running);
    assert_eq!(decided.panel_running, Some(false));
    assert!(!decided.clear_quit);
}

/// Only the EDGE acts: a held-down hotkey across ticks, or its falling edge, changes nothing
/// (`HoldSpace.cs:60,67` compare `f12Now` with `f12Was`).
#[test]
fn only_the_rising_edge_of_the_hotkey_acts() {
    let held = tick(false, false, true, true);
    assert_eq!(
        held.actions,
        [] as [seal_domain::tools::hold_space::HoldSpaceStep; 0]
    );
    assert_eq!(held.panel_running, None);

    let falling = tick(false, false, false, true);
    assert_eq!(
        falling.actions,
        [] as [seal_domain::tools::hold_space::HoldSpaceStep; 0]
    );
    assert_eq!(falling.panel_running, None);
}

/// The quirk the module docs name: v2 checks the mirror first and the hotkey second in the SAME pass,
/// so a launcher start on the tick the player presses F12 sends `P` and then `U`. Pinned, not fixed.
#[test]
fn a_launcher_start_and_a_hotkey_stop_on_the_same_tick_do_both_in_v2s_order() {
    let decided = tick(false, true, true, false);

    assert_eq!(decided.actions.len(), 2);
    assert_eq!(decided.actions[0].why, HoldSpaceWhy::Panel);
    assert_eq!(decided.actions[0].action, HoldSpaceAction::Hold);
    assert_eq!(decided.actions[1].why, HoldSpaceWhy::Hotkey);
    assert_eq!(decided.actions[1].action, HoldSpaceAction::Release);
    // The last action wins the memory: the hotkey stopped it.
    assert!(!decided.mirror_running);
    assert_eq!(decided.panel_running, Some(false));
    // A start DID happen on this tick (the panel's), so a pending quit is cleared.
    assert!(decided.clear_quit);
}

/// The mirror image: a launcher stop and a hotkey start in one tick release then hold.
#[test]
fn a_launcher_stop_and_a_hotkey_start_on_the_same_tick_do_both() {
    let decided = tick(true, false, true, false);

    assert_eq!(decided.actions.len(), 2);
    assert_eq!(decided.actions[0].why, HoldSpaceWhy::Panel);
    assert_eq!(decided.actions[0].action, HoldSpaceAction::Release);
    assert_eq!(decided.actions[1].why, HoldSpaceWhy::Hotkey);
    assert_eq!(decided.actions[1].action, HoldSpaceAction::Hold);
    assert!(decided.mirror_running);
    assert_eq!(decided.panel_running, Some(true));
}

/// Steady state: once the mirror has been followed, later ticks with the same inputs write nothing.
#[test]
fn a_settled_hold_is_not_re_asserted_every_tick() {
    let first = tick(false, true, false, false);
    assert_eq!(first.actions.len(), 1);

    let second = tick(first.mirror_running, true, false, false);
    assert_eq!(
        second.actions,
        [] as [seal_domain::tools::hold_space::HoldSpaceStep; 0]
    );
    assert!(second.mirror_running);
}

/// `HoldSpace.cs:70-74` — the `finally` release, as a value the loop's teardown owes on every exit.
#[test]
fn teardown_always_owes_a_release() {
    assert_eq!(cleanup(), HoldSpaceAction::Release);
}

/// `HoldSpace.cs:32-33` — the two failure lines verbatim, and the fact that the dangerous one says what
/// to do.
#[test]
fn the_two_failure_lines_are_v2s() {
    assert_eq!(
        hold_failed("the port is gone"),
        "Could not hold the spacebar: the port is gone"
    );
    assert_eq!(
        release_failed("the port is gone"),
        "Could not release the spacebar — tap it once in game (the port is gone)"
    );
    assert!(release_failed("x").contains("tap it once in game"));
}

/// The wire encoding belongs to the decision, so the effect side and the tests cannot disagree about
/// what `Hold` means; the transport is `seal-board`'s.
#[test]
fn the_actions_carry_the_write_they_mean() {
    assert_eq!(HoldSpaceAction::Hold.wire(), "P\n");
    assert_eq!(HoldSpaceAction::Release.wire(), "U\n");
}

/// The beeps are effect data, pinned as values because they are part of what the player experiences.
#[test]
fn the_beeps_are_v2s_notes() {
    assert_eq!(
        HOLD_BEEP,
        Beep {
            frequency: 523,
            duration_ms: 100
        }
    );
    assert_eq!(
        RELEASE_BEEP,
        Beep {
            frequency: 1000,
            duration_ms: 150
        }
    );
}

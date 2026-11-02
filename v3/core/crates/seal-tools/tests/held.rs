//! The held-input guard: a key or mouse button that is down must not survive the tool.
//!
//! v2 kept this as two rules that had to agree: `HeldKeys.NeedsReleaseOnStop` said WHICH tools could
//! leave something down, and every stop path (`StopTool`, the self-end continuation, `Dispose`) plus
//! two tools' own `finally` blocks called `ReleaseHeld()`. `PROGRESS.md` entry 65 is the list of the
//! paths that did not: a drag's left button pressed ABOVE its `try` so the `finally` never ran, and a
//! boarding window left open because the close's own `SleepCheck` threw.
//!
//! Here the release is not a rule to remember: [`HeldInput`] releases everything it pressed when it
//! is dropped, so normal exit, early return, a panic and a cancellation all leave the board the same
//! way. The firmware releasing held inputs when the serial port closes (`DESIGN.md` §2.1) stays the
//! last resort for a process that dies, not the design.
//!
//! Release order and messages are v2's: the stop path wrote `U` then `l` always, and the Hold Space
//! tool's own failure texts are quoted below.

use std::panic::{AssertUnwindSafe, catch_unwind};

use seal_tools::cancel::{CancelToken, Cancelled, RunControl};
use seal_tools::clock::Clock;
use seal_tools::held::{BoardError, BoardInput, Held, HeldInput};

/// A board that records what it was told, and can be made to fail.
#[derive(Default)]
struct FakeBoard {
    presses: Vec<Held>,
    /// Every release CALL, successful or not — order included.
    releases: Vec<Held>,
    fail_press: Option<Held>,
    /// Fail this many release calls (the call is still recorded).
    fail_releases: u32,
}

impl BoardInput for FakeBoard {
    fn press(&mut self, input: Held) -> Result<(), BoardError> {
        if self.fail_press == Some(input) {
            return Err(BoardError::new("protocol level 1 board"));
        }
        self.presses.push(input);
        Ok(())
    }

    fn release(&mut self, input: Held) -> Result<(), BoardError> {
        self.releases.push(input);
        if self.fail_releases > 0 {
            self.fail_releases -= 1;
            return Err(BoardError::new("the port is not open"));
        }
        Ok(())
    }
}

/// A clock that never waits, for the cancellation path.
struct NoopClock;

impl Clock for NoopClock {
    fn sleep_seconds(&self, _seconds: f64) {}
}

#[test]
fn dropping_the_guard_releases_in_lifo_order() {
    let mut board = FakeBoard::default();
    {
        let mut held = HeldInput::new(&mut board);
        held.press(Held::LeftButton).unwrap();
        held.press(Held::Space).unwrap();
    }

    assert_eq!(board.presses, vec![Held::LeftButton, Held::Space]);
    assert_eq!(
        board.releases,
        vec![Held::Space, Held::LeftButton],
        "the most recent hold is let go first"
    );
}

#[test]
fn lifo_order_is_not_v2s_fixed_sequence() {
    // v2 wrote U then l always, because the stop path did not know what the tool had pressed. The
    // guard does know, and the Lead's decision is LIFO: let go of the last gesture first. Pressing
    // in the order v2 wrote makes the difference visible.
    let mut board = FakeBoard::default();
    {
        let mut held = HeldInput::new(&mut board);
        held.press(Held::Space).unwrap();
        held.press(Held::LeftButton).unwrap();
    }

    assert_eq!(board.releases, vec![Held::LeftButton, Held::Space]);
}

#[test]
fn a_guard_with_nothing_held_releases_nothing() {
    let mut board = FakeBoard::default();
    {
        let _held = HeldInput::new(&mut board);
    }

    assert!(
        board.releases.is_empty(),
        "a guard with nothing held releases nothing"
    );
}

#[test]
fn an_early_return_releases_what_was_held() {
    // The v2 pet drag's failure mode: the press sat above the try, so an early exit left the button
    // down. Here the guard is the try.
    fn arm_then_give_up(board: &mut FakeBoard) -> Result<(), &'static str> {
        let mut held = HeldInput::new(board);
        held.press(Held::LeftButton).unwrap();
        Err("stop")
    }

    let mut board = FakeBoard::default();
    assert_eq!(arm_then_give_up(&mut board), Err("stop"));
    assert_eq!(board.releases, vec![Held::LeftButton]);
}

#[test]
fn a_panic_releases_what_was_held() {
    let mut board = FakeBoard::default();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut held = HeldInput::new(&mut board);
        held.press(Held::Space).unwrap();
        panic!("the tool crashed mid-hold");
    }));

    assert!(result.is_err(), "the panic propagates");
    assert_eq!(
        board.releases,
        vec![Held::Space],
        "a crashed tool must not leave the player's spacebar down"
    );
}

#[test]
fn a_cancelled_run_releases_what_was_held() {
    // The stop path: the loop unwinds with `Cancelled`, the guard drops, the board is let go.
    fn run(board: &mut FakeBoard, control: &RunControl) -> Result<(), Cancelled> {
        let mut held = HeldInput::new(board);
        held.press(Held::Space)
            .expect("the fake board accepts the press");
        control.sleep(&NoopClock, 30.0)?;
        held.press(Held::LeftButton)
            .expect("the fake board accepts the press");
        Ok(())
    }

    let token = CancelToken::new();
    token.cancel();
    let control = RunControl::new(token);
    let mut board = FakeBoard::default();

    assert_eq!(run(&mut board, &control), Err(Cancelled));
    assert_eq!(board.releases, vec![Held::Space]);
}

#[test]
fn an_explicit_release_reports_and_the_drop_does_not_resend() {
    let mut board = FakeBoard::default();
    {
        let mut held = HeldInput::new(&mut board);
        held.press(Held::Space).unwrap();
        held.press(Held::LeftButton).unwrap();
        held.release_all().unwrap();
        assert!(!held.is_held(Held::Space));
        assert!(!held.is_held(Held::LeftButton));
    }

    assert_eq!(
        board.releases,
        vec![Held::LeftButton, Held::Space],
        "LIFO, and the drop has nothing left to send"
    );
}

#[test]
fn a_failed_release_is_reported_and_not_retried() {
    // v2's `ReleaseHeld` reported the failure and did NOT retry — and the Lead's rule is the same:
    // the second attempt is a no-op rather than a second board write. The firmware's release-on-close
    // is the last resort for an input that a failed write left down, not a retry loop in the guard.
    let mut board = FakeBoard {
        fail_releases: 1,
        ..FakeBoard::default()
    };
    {
        let mut held = HeldInput::new(&mut board);
        held.press(Held::Space).unwrap();
        let error = held.release_all().expect_err("the first release fails");
        assert_eq!(
            Held::Space.release_failure_message(&error),
            "Could not release the spacebar — tap it once in game (the port is not open)"
        );
        assert!(
            !held.is_held(Held::Space),
            "the attempt is spent: a second one would be a second board write"
        );
    }

    assert_eq!(
        board.releases,
        vec![Held::Space],
        "attempted once, never re-sent by the drop"
    );
}

#[test]
fn a_failed_press_is_still_released_on_drop() {
    // The Hold Space tool's own `finally` always sent the release, even when the press had failed:
    // "never leave the spacebar stuck down". The guard records the intent before pressing, so a
    // failed press is released (harmlessly — the firmware's release is idempotent).
    let mut board = FakeBoard {
        fail_press: Some(Held::Space),
        ..FakeBoard::default()
    };
    {
        let mut held = HeldInput::new(&mut board);
        let error = held.press(Held::Space).expect_err("the press fails");
        assert_eq!(
            Held::Space.press_failure_message(&error),
            "Could not hold the spacebar: protocol level 1 board"
        );
    }

    assert_eq!(board.releases, vec![Held::Space]);
}

#[test]
fn releasing_something_that_was_never_held_is_a_harmless_no_op() {
    let mut board = FakeBoard::default();
    {
        let mut held = HeldInput::new(&mut board);
        held.release(Held::LeftButton).unwrap();
        assert!(!held.is_held(Held::LeftButton));
    }

    assert!(
        board.releases.is_empty(),
        "releasing an input that was never held sends nothing"
    );
}

#[test]
fn a_released_input_is_not_released_again() {
    let mut board = FakeBoard::default();
    {
        let mut held = HeldInput::new(&mut board);
        held.press(Held::Space).unwrap();
        held.release(Held::Space).unwrap();
        assert!(!held.is_held(Held::Space));
    }

    assert_eq!(board.releases, vec![Held::Space]);
}

#[test]
fn the_left_button_messages_are_defined_where_v2_had_none() {
    // v2 has player-facing text for the spacebar (Hold Space's own report) but the pet's drag only
    // logged. The same two sentences are defined for the button by analogy — the release one carries
    // the same "get it back yourself" instruction as the spacebar's.
    let error = BoardError::new("the port is not open");
    assert_eq!(
        Held::LeftButton.press_failure_message(&error),
        "Could not hold the left mouse button: the port is not open"
    );
    assert_eq!(
        Held::LeftButton.release_failure_message(&error),
        "Could not release the left mouse button — click once in game (the port is not open)"
    );
}

#[test]
fn the_spacebar_messages_are_v2_text() {
    let error = BoardError::new("Access is denied");
    assert_eq!(
        Held::Space.press_failure_message(&error),
        "Could not hold the spacebar: Access is denied"
    );
    assert_eq!(
        Held::Space.release_failure_message(&error),
        "Could not release the spacebar — tap it once in game (Access is denied)"
    );
}

#[test]
fn held_inputs_are_named_for_logs() {
    assert_eq!(Held::ALL, [Held::Space, Held::LeftButton]);
    assert_eq!(Held::Space.as_str(), "space");
    assert_eq!(Held::LeftButton.as_str(), "left mouse button");
}

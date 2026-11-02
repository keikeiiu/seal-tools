//! The closed-loop cursor placement, ported from `v2/SealTools.Core/HidPointer.cs`.
//!
//! The cursor is placed with the ARDUINO, not `SetCursorPos`: the launcher's process is
//! intermittently refused its own cursor placement while the identical call succeeds from another
//! process (`docs/investigations/CURSOR-INVESTIGATION.md`). The HID path cannot be refused, so the
//! loop verifies the landing instead of assuming it.
//!
//! The loop (`HidPointer.To`):
//!
//! 1. read the cursor; if the error is inside [`TOLERANCE_PX`], done;
//! 2. if the last step made the error WORSE, double the divisor (damp rather than oscillate);
//! 3. ask for the remaining error over the divisor, clamped to [`MAX_STEP_PX`] and never rounded to 0;
//! 4. wait for the cursor to SETTLE — a move is not finished until it has been seen to move
//!    (`HidPointer.WaitForCursorToSettle`: the firmware walks `D` out in 10 px chunks, so the first
//!    polls after the write legitimately read the old position);
//! 5. repeat, bounded by [`MAX_STEPS`].
//!
//! On failure the answer carries BOTH the goal and where the cursor actually stopped, because "it
//! did not work" is useless and the numbers are gone by the time anyone looks
//! (`HidPointer.cs:110-115`). The gain is measured 1:1 on the reference machine; on a machine where
//! it is not, the damping is what saves the loop.
//!
//! The cursor itself is a seam ([`Cursor`]) and time is a [`Clock`]: the real ones read
//! `GetCursorPos` and sleep, and measuring either against the OS is the adapter's job (see
//! `crate::transport`). A failed placement never clicks blind — the caller must check
//! [`Placement::ok`] and refuse to click.

use crate::clock::Clock;
use crate::command::BoardCommand;
use crate::transport::Transport;

/// A button is tens of pixels wide, and two px also absorbs the physical → cursor-space rounding
/// (`HidPointer.TolerancePx = 2`).
pub const TOLERANCE_PX: i32 = 2;

/// The step budget. Raised from 6 after a live failure (2026-09-17): with a damped gain a correction
/// needs roughly ten steps, and the loop gave up ~19 px short. The budget only gets spent when
/// something has already gone wrong (`HidPointer.MaxSteps = 16`).
pub const MAX_STEPS: u32 = 16;

/// Cap on a single HID move: a wrong target must not fling the pointer across the desktop.
pub const MAX_STEP_PX: i32 = 600;

/// The settle poll (`HidPointer.PollMs = 15`) in SECONDS.
pub const POLL_SECONDS: f64 = 0.015;

/// Polls allowed for one move to settle (`HidPointer.MaxSettlePolls = 24`).
pub const MAX_SETTLE_POLLS: u32 = 24;

/// Where the cursor can be read. The real implementation is `GetCursorPos`; tests script it.
pub trait Cursor {
    /// The cursor position, or `None` when it cannot be read. `None` is never "the cursor is at 0,0":
    /// v2 refuses to make that assumption, and so does this loop.
    fn position(&mut self) -> Option<(i32, i32)>;
}

/// Where a placement ended up (`CursorPlacement`): ok, how many HID moves were sent, the final
/// position when it could be read, and the player-facing reason when it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub ok: bool,
    pub steps: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    /// The sentence the card shows; `None` on success.
    pub error: Option<String>,
}

impl Placement {
    fn landed(steps: u32, x: i32, y: i32) -> Self {
        Self {
            ok: true,
            steps,
            x: Some(x),
            y: Some(y),
            error: None,
        }
    }

    fn failed(steps: u32, position: Option<(i32, i32)>, error: String) -> Self {
        Self {
            ok: false,
            steps,
            x: position.map(|(x, _)| x),
            y: position.map(|(_, y)| y),
            error: Some(error),
        }
    }
}

/// Moves the cursor to `goal` (in the cursor's own space) with the Arduino, closed-loop.
///
/// A miss is an ordinary result, never a panic: the caller refuses to click when
/// [`Placement::ok`] is false.
pub fn place(
    transport: &mut dyn Transport,
    cursor: &mut dyn Cursor,
    clock: &dyn Clock,
    goal: (i32, i32),
) -> Placement {
    let (goal_x, goal_y) = goal;
    let mut divisor: i32 = 1;
    let mut last_error = i32::MAX;
    let mut moves: u32 = 0;

    for _ in 0..MAX_STEPS {
        let Some((x, y)) = cursor.position() else {
            return Placement::failed(
                moves,
                None,
                "the cursor position can't be read (GetCursorPos failed)".to_owned(),
            );
        };

        let (dx, dy) = (goal_x - x, goal_y - y);
        let error = dx.abs().max(dy.abs());
        if error <= TOLERANCE_PX {
            return Placement::landed(moves, x, y);
        }

        // A step that made the error worse means the machine scales the HID counts: damp instead of
        // oscillating. On the reference machine this never triggers.
        if error >= last_error {
            divisor *= 2;
        }
        last_error = error;

        let ask_x = step_count(dx, divisor);
        let ask_y = step_count(dy, divisor);
        if let Err(error) = transport.write_line(&BoardCommand::move_by(ask_x, ask_y).encode()) {
            return Placement::failed(
                moves,
                Some((x, y)),
                format!("the board would not take the move — {error}"),
            );
        }
        moves += 1;

        wait_for_cursor_to_settle(cursor, clock, (x, y));
    }

    let final_position = cursor.position();
    let error = match final_position {
        Some((x, y)) => {
            format!("the cursor wouldn't move to ({goal_x},{goal_y}) — it stopped at ({x},{y})")
        }
        // v2's interpolation would print "(,)" here; a defined sentence is better than porting that.
        None => format!(
            "the cursor wouldn't move to ({goal_x},{goal_y}) — the cursor position can't be read \
             (GetCursorPos failed)"
        ),
    };
    Placement::failed(moves, final_position, error)
}

/// Blocks until the move has arrived and stopped: a move is not finished until it has been SEEN to
/// move (`HidPointer.WaitForCursorToSettle`).
///
/// The "seen to move" half is load-bearing: the firmware walks a `D` out in 10 px chunks with a 1 ms
/// gap, so between the write returning and the first chunk landing the cursor legitimately still
/// reads its old position — and two such reads in a row are indistinguishable from a finished move.
/// Treating them as finished is what made the live loop (2026-09-17) feed corrections forward against
/// a stale number: two asks of -600 and -481 were issued while the cursor had not moved, and all
/// three landed together and clamped at the top of the screen.
fn wait_for_cursor_to_settle(cursor: &mut dyn Cursor, clock: &dyn Clock, before: (i32, i32)) {
    let Some(mut previous) = cursor.position() else {
        return;
    };
    let mut seen_move = previous != before;

    for _ in 0..MAX_SETTLE_POLLS {
        clock.sleep_seconds(POLL_SECONDS);
        let Some(now) = cursor.position() else {
            return;
        };
        if seen_move && now == previous {
            return;
        }
        if now != before {
            seen_move = true;
        }
        previous = now;
    }
}

/// One axis of a closed-loop correction: the remaining error, damped and clamped, and never rounded
/// away to 0 — that would stall the loop short of the tolerance (`HidPointer.StepCount`).
fn step_count(delta: i32, divisor: i32) -> i32 {
    if delta == 0 {
        return 0;
    }
    let value = delta / divisor;
    if value == 0 {
        delta.signum()
    } else {
        value.clamp(-MAX_STEP_PX, MAX_STEP_PX)
    }
}

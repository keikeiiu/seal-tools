//! The closed-loop cursor placement, ported from `v2/SealTools.Core/HidPointer.cs`.
//!
//! The cursor is placed with the ARDUINO, not `SetCursorPos`: the launcher's process is
//! intermittently refused its own cursor placement while the identical call succeeds from another
//! process (`docs/investigations/CURSOR-INVESTIGATION.md`). The HID path cannot be refused, so the
//! loop verifies the landing instead of assuming it.
//!
//! The loop (`HidPointer.To`, with the gain estimate that replaces the old open-ended damping):
//!
//! 1. read the cursor; if the error is inside [`TOLERANCE_PX`], done;
//! 2. ask for the remaining error over the CURRENT gain estimate, clamped to [`MAX_STEP_PX`] and
//!    never rounded to 0;
//! 3. wait for the cursor to SETTLE — a move is not finished until it has been seen to move
//!    (`HidPointer.WaitForCursorToSettle`: the firmware walks `D` out in 10 px chunks, so the first
//!    polls after the write legitimately read the old position);
//! 4. measure the gain from what actually moved (`moved / asked`, smoothed) so the next ask is
//!    scaled to THIS machine instead of the reference machine (HW-F9);
//! 5. repeat, bounded by [`MAX_STEPS`].
//!
//! v2 (and the first v3 port) only ever GREW a damping divisor when a step failed to help. One bad
//! sample — a missed settle, the game warping the cursor, or "Enhance pointer precision" swallowing
//! a small move — halved the gain for the rest of the placement, and from 600 px the remaining
//! convergence needed more steps than [`MAX_STEPS`]. The damp now falls only on a step whose motion
//! is unusable (nothing moved, or the pointer moved AGAINST the ask), and a clean improving step
//! releases it again.
//!
//! On failure the answer carries BOTH the goal and where the cursor actually stopped, because "it
//! did not work" is useless and the numbers are gone by the time anyone looks
//! (`HidPointer.cs:110-115`).
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

/// Polls after which a move of at most [`SMALL_ASK_PX`] that has not moved at all is treated as
/// swallowed (HW-F9). A 1–3 count ask lands within a millisecond of the write, so three polls seeing
/// the old position is not "still arriving".
pub const SMALL_ASK_SETTLE_POLLS: u32 = 3;

/// The ask size at which the short settle applies: "Enhance pointer precision" rounds a move this
/// small away entirely, and waiting the full 24 polls for it costs 360 ms per correction (HW-F9).
pub const SMALL_ASK_PX: i32 = 3;

/// The measured gain (`moved / asked`) is clamped to this band before it is smoothed, so one wild
/// reading cannot send the next ask across the desktop (HW-F9).
const MIN_GAIN: f64 = 0.25;
const MAX_GAIN: f64 = 4.0;

/// How much of a new gain measurement is believed: the estimate follows a real machine's scale in a
/// few steps without letting one spurious sample own it (HW-F9).
const GAIN_SMOOTHING: f64 = 0.5;

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
    // Per-axis gain: how many cursor pixels one asked HID count produces on THIS machine. 1.0 is the
    // reference machine; the estimate is what makes a 0.5 or 1.5 machine converge in a few steps
    // instead of relying on a divisor that only ever grows (HW-F9).
    let mut gain_x: f64 = 1.0;
    let mut gain_y: f64 = 1.0;
    // The damp, released by a clean improving step: it only absorbs a sample whose motion says
    // nothing about the gain (nothing moved, or the pointer moved against the ask).
    let mut divisor: i32 = 1;
    let mut last_error = i32::MAX;
    let mut last_delta: Option<(i32, i32)> = None;
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

        // An OVERSHOOT: the remaining error points the other way than it did before the last
        // correction. Damp for one step rather than chasing the pointer back and forth.
        if let Some((last_x, last_y)) = last_delta
            && ((dx != 0 && dx.signum() != last_x.signum() && last_x != 0)
                || (dy != 0 && dy.signum() != last_y.signum() && last_y != 0))
        {
            divisor = (divisor * 2).min(8);
        }
        last_delta = Some((dx, dy));

        let ask_x = step_count(dx, gain_x, divisor);
        let ask_y = step_count(dy, gain_y, divisor);
        // The correction is clamped to ±600, so this cannot be out of range; the fallible grammar is
        // what keeps a future caller from sending a frame the sketch would reject (HW-F11).
        let Ok(command) = BoardCommand::move_by(ask_x, ask_y) else {
            return Placement::failed(
                moves,
                Some((x, y)),
                format!("the board cannot carry the correction ({ask_x},{ask_y})"),
            );
        };
        if let Err(error) = transport.write_line(&command.encode()) {
            return Placement::failed(
                moves,
                Some((x, y)),
                format!("the board would not take the move — {error}"),
            );
        }
        moves += 1;

        if let Some((settled_x, settled_y)) =
            wait_for_cursor_to_settle(cursor, clock, (x, y), (ask_x, ask_y))
        {
            observe(&mut gain_x, ask_x, settled_x - x);
            observe(&mut gain_y, ask_y, settled_y - y);
        }
        // A step that reduced the error earned the full gain back; the damp is for a bad sample, not
        // a permanent penalty.
        if error < last_error {
            divisor = 1;
        }
        last_error = error;
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

/// Fold one move's measured motion into the per-axis gain estimate (HW-F9).
///
/// A move of the right sign is the measurement: `moved / asked`, clamped to [`MIN_GAIN`]..[`MAX_GAIN`]
/// so one wild sample cannot fling the next ask across the desktop, then smoothed. A move that did
/// not happen, or went the WRONG way, is not a gain measurement at all (a swallowed 1-count ask, a
/// missed settle, the game warping the pointer) — the estimate is halved instead of inverted, which
/// makes the next ask bigger and recovers the stall.
fn observe(gain: &mut f64, asked: i32, moved: i32) {
    if asked == 0 {
        return;
    }
    if moved == 0 || moved.signum() != asked.signum() {
        *gain = (*gain / 2.0).max(MIN_GAIN);
        return;
    }
    let observed = (f64::from(moved) / f64::from(asked)).clamp(MIN_GAIN, MAX_GAIN);
    *gain = (*gain + (observed - *gain) * GAIN_SMOOTHING).clamp(MIN_GAIN, MAX_GAIN);
}

/// Blocks until the move has arrived and stopped, and returns the position it settled at (so the
/// caller can measure the gain from it).
///
/// The "seen to move" half is load-bearing: the firmware walks a `D` out in 10 px chunks with a 1 ms
/// gap, so between the write returning and the first chunk landing the cursor legitimately still
/// reads its old position — and two such reads in a row are indistinguishable from a finished move.
/// Treating them as finished is what made the live loop (2026-09-17) feed corrections forward against
/// a stale number: two asks of -600 and -481 were issued while the cursor had not moved, and all
/// three landed together and clamped at the top of the screen.
///
/// A move whose largest ask is at most [`SMALL_ASK_PX`] and that has not moved at all after
/// [`SMALL_ASK_SETTLE_POLLS`] is treated as swallowed: waiting the full budget for a 1–3 count move
/// costs 360 ms and tells the loop nothing (HW-F9).
fn wait_for_cursor_to_settle(
    cursor: &mut dyn Cursor,
    clock: &dyn Clock,
    before: (i32, i32),
    ask: (i32, i32),
) -> Option<(i32, i32)> {
    let mut previous = cursor.position()?;
    let mut seen_move = previous != before;
    let polls = if ask.0.abs().max(ask.1.abs()) <= SMALL_ASK_PX {
        SMALL_ASK_SETTLE_POLLS
    } else {
        MAX_SETTLE_POLLS
    };

    for _ in 0..polls {
        clock.sleep_seconds(POLL_SECONDS);
        let now = cursor.position()?;
        if seen_move && now == previous {
            return Some(now);
        }
        if now != before {
            seen_move = true;
        }
        previous = now;
    }
    Some(previous)
}

/// One axis of a closed-loop correction: the remaining error over the measured gain and the damp,
/// clamped and never rounded away to 0 — that would stall the loop short of the tolerance
/// (`HidPointer.StepCount`).
fn step_count(delta: i32, gain: f64, divisor: i32) -> i32 {
    if delta == 0 {
        return 0;
    }
    let scale = gain * f64::from(divisor);
    let value = (f64::from(delta) / scale).round();
    // `as` truncates, and a NaN cannot occur (delta != 0, scale >= 0.25); the clamp below is the
    // guard that matters.
    #[allow(clippy::cast_possible_truncation)]
    let value = value as i32;
    if value == 0 {
        delta.signum()
    } else {
        value.clamp(-MAX_STEP_PX, MAX_STEP_PX)
    }
}

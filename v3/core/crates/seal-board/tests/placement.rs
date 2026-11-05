//! The closed-loop cursor placement, ported from `HidPointer.To`/`WaitForCursorToSettle`.
//!
//! `HidPointer.cs` is the oracle. What matters and is pinned here:
//!
//! - the loop reads the cursor, corrects by the remaining error, and stops inside a 2 px tolerance;
//! - it DAMPS (doubles the divisor) when a correction made the error worse, instead of oscillating;
//! - a step is clamped to 600 px so a wrong target cannot fling the pointer across the desktop;
//! - after a move it waits for the cursor to SETTLE, and a move is not finished until it has been
//!   SEEN to move — two stale reads in a row are not "settled";
//! - the step budget is 16, and on failure the answer carries BOTH the goal and where the cursor
//!   actually stopped;
//! - every wait is `clock.sleep_seconds(0.015)` — SECONDS (the eleven-minute-sleep incident).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use seal_board::clock::Clock;
use seal_board::placement::{Cursor, MAX_STEP_PX, MAX_STEPS, POLL_SECONDS, TOLERANCE_PX, place};
use seal_board::transport::{Deadline, LineRead, Transport, TransportError};

#[derive(Default)]
struct Board {
    x: i32,
    y: i32,
    /// How many cursor pixels one asked HID count produces, in percent (`100` is the reference
    /// machine, `50` a machine that under-moves, `150` one that over-moves).
    gain_percent: i32,
    /// A board whose first move after the open is swallowed entirely: a missed settle or a stalled
    /// frame.
    swallow_first: bool,
    swallowed: bool,
    /// Moves whose largest count is below this land at HALF scale: the "Enhance pointer precision"
    /// curve, where small HID counts are accelerated away.
    epp_below: i32,
    /// A wedged board: it accepts moves and moves nothing.
    stuck: bool,
    /// One absolute X per move, used to script an overshoot/worsening correction.
    responses: VecDeque<i32>,
    /// Absolute reads for `position()`, used to script stale/settling/unreadable reads.
    reads: VecDeque<Option<(i32, i32)>>,
    writes: Vec<String>,
    events: Vec<&'static str>,
    sleeps: Vec<f64>,
    fail_writes: Option<String>,
}

impl Board {
    fn scaled(value: i32, percent: i32) -> i32 {
        #[allow(clippy::cast_possible_truncation)]
        let scaled = (f64::from(value) * f64::from(percent) / 100.0).round() as i32;
        scaled
    }

    fn apply_move(&mut self, dx: i32, dy: i32) {
        if self.swallow_first && !self.swallowed {
            self.swallowed = true;
            return;
        }
        let percent = if self.epp_below > 0 && dx.abs().max(dy.abs()) < self.epp_below {
            self.gain_percent / 2
        } else {
            self.gain_percent
        };
        self.x += Self::scaled(dx, percent);
        self.y += Self::scaled(dy, percent);
    }
}

struct Rig(Rc<RefCell<Board>>);

impl Rig {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(Board {
            gain_percent: 100,
            ..Board::default()
        })))
    }

    fn transport(&self) -> RigTransport {
        RigTransport(Rc::clone(&self.0))
    }

    fn cursor(&self) -> RigCursor {
        RigCursor(Rc::clone(&self.0))
    }

    fn clock(&self) -> RigClock {
        RigClock(Rc::clone(&self.0))
    }

    fn writes(&self) -> Vec<String> {
        self.0.borrow().writes.clone()
    }

    fn events(&self) -> Vec<&'static str> {
        self.0.borrow().events.clone()
    }

    fn sleeps(&self) -> Vec<f64> {
        self.0.borrow().sleeps.clone()
    }

    fn script_reads(&self, reads: Vec<Option<(i32, i32)>>) {
        self.0.borrow_mut().reads = reads.into();
    }

    fn script_moves(&self, positions: Vec<i32>) {
        self.0.borrow_mut().responses = positions.into();
    }

    fn set_position(&self, x: i32, y: i32) {
        let mut board = self.0.borrow_mut();
        board.x = x;
        board.y = y;
    }

    fn set_gain_percent(&self, percent: i32) {
        self.0.borrow_mut().gain_percent = percent;
    }

    fn swallow_first_move(&self) {
        self.0.borrow_mut().swallow_first = true;
    }

    fn epp_below(&self, counts: i32) {
        self.0.borrow_mut().epp_below = counts;
    }

    fn set_stuck(&self) {
        self.0.borrow_mut().stuck = true;
    }

    fn fail_writes(&self, message: &str) {
        self.0.borrow_mut().fail_writes = Some(message.to_owned());
    }
}

struct RigTransport(Rc<RefCell<Board>>);

impl Transport for RigTransport {
    fn write_line(&mut self, wire: &str) -> Result<(), TransportError> {
        let mut board = self.0.borrow_mut();
        board.events.push("write");
        if let Some(message) = &board.fail_writes {
            return Err(TransportError::new(message.clone()));
        }
        board.writes.push(wire.to_owned());

        let text = wire.trim_end();
        if let Some(rest) = text.strip_prefix("D ") {
            let mut parts = rest.split_whitespace();
            let dx: i32 = parts.next().expect("D dx").parse().expect("dx");
            let dy: i32 = parts.next().expect("D dy").parse().expect("dy");
            if let Some(absolute_x) = board.responses.pop_front() {
                board.x = absolute_x;
            } else if !board.stuck {
                board.apply_move(dx, dy);
            }
        }
        Ok(())
    }

    fn read_line(&mut self, _deadline: Deadline) -> LineRead {
        LineRead::Silent
    }
}

struct RigCursor(Rc<RefCell<Board>>);

impl Cursor for RigCursor {
    fn position(&mut self) -> Option<(i32, i32)> {
        let mut board = self.0.borrow_mut();
        board.events.push("read");
        match board.reads.pop_front() {
            Some(read) => read,
            None => Some((board.x, board.y)),
        }
    }
}

struct RigClock(Rc<RefCell<Board>>);

impl Clock for RigClock {
    fn sleep_seconds(&self, seconds: f64) {
        let mut board = self.0.borrow_mut();
        board.events.push("sleep");
        board.sleeps.push(seconds);
    }
}

fn goal(x: i32, y: i32) -> (i32, i32) {
    (x, y)
}

#[test]
fn a_cursor_already_inside_the_tolerance_is_not_moved() {
    let rig = Rig::new();
    rig.set_position(100, 100);
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(
        &mut transport,
        &mut cursor,
        &rig.clock(),
        goal(102, 101), // within the 2 px tolerance on both axes
    );

    assert!(placement.ok);
    assert_eq!(placement.steps, 0);
    assert_eq!((placement.x, placement.y), (Some(100), Some(100)));
    assert!(rig.writes().is_empty(), "a hit is not a move");
    assert!(rig.sleeps().is_empty(), "a hit pays no settle time");
}

#[test]
fn one_correction_lands_exactly() {
    let rig = Rig::new();
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(10, 5));

    assert!(placement.ok);
    assert_eq!(placement.steps, 1);
    assert_eq!((placement.x, placement.y), (Some(10), Some(5)));
    assert_eq!(rig.writes(), vec!["D 10 5\n"]);
    assert_eq!(placement.error, None);
}

#[test]
fn a_move_is_waited_out_with_seconds_not_milliseconds() {
    // `SleepCheck` takes SECONDS; passing milliseconds sleeps for eleven minutes and looks like a
    // hang (DESIGN.md §2.1). The settle poll is 15 ms = 0.015 s.
    let rig = Rig::new();
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(10, 0));

    assert!(placement.ok);
    assert_eq!(POLL_SECONDS, 0.015);
    for slept in rig.sleeps() {
        assert!(
            (slept - POLL_SECONDS).abs() < f64::EPSILON,
            "the settle poll slept {slept} — that is not {POLL_SECONDS} s"
        );
    }
}

#[test]
fn the_settle_wait_comes_between_the_move_and_the_next_correction() {
    // The order the loop needs: read, move, wait for settle, read again. A loop that corrected
    // before waiting would chase a stale position and overshoot.
    let rig = Rig::new();
    // loop read (0,0); settle previous read (0,0); two stale polls still at 0; then the move lands;
    // a further poll proves it settled; the loop reads the new position and is inside tolerance.
    rig.script_reads(vec![
        Some((0, 0)),
        Some((0, 0)),
        Some((0, 0)),
        Some((0, 0)),
        Some((10, 0)),
        Some((10, 0)),
        Some((10, 0)),
    ]);
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(10, 0));

    assert!(placement.ok);
    assert_eq!(
        rig.writes(),
        vec!["D 10 0\n"],
        "no correction may be sent while the cursor still reads its old position"
    );
    let events = rig.events();
    assert_eq!(
        &events[..4],
        ["read", "write", "read", "sleep"],
        "the loop reads, moves, then waits before the next read: {events:?}"
    );
    assert!(
        !rig.sleeps().is_empty(),
        "the stale polls are what tell the loop the move is still arriving"
    );
}

#[test]
fn an_overshoot_is_corrected_with_the_measured_gain_not_chased() {
    // The first move overshoots to 140 because the board scales the counts by 1.4. The loop measures
    // that gain, damps once for the sign flip, and asks for a small correction instead of chasing
    // the pointer back and forth (HW-F9).
    let rig = Rig::new();
    rig.script_moves(vec![140]);
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(100, 0));

    assert!(placement.ok);
    assert!(
        (placement.x.expect("read") - 100).abs() <= TOLERANCE_PX,
        "landed within tolerance: {:?}",
        placement.x
    );
    assert!(
        placement.steps <= 4,
        "the gain estimate converges in a few steps: {}",
        placement.steps
    );
    let first_correction = &rig.writes()[1];
    assert!(
        first_correction != "D -40 0\n" && first_correction.starts_with("D -"),
        "the correction asks BACK, scaled by the measured gain: {first_correction}"
    );
}

#[test]
fn a_move_in_the_wrong_direction_damps_instead_of_inverting_the_gain() {
    // A sample that says nothing about the gain — the pointer moved AGAINST the ask (a missed
    // settle, a game warp, a mis-signed calibration) — must halve the estimate, never invert it:
    // inverting would ask for 400 here (HW-F9).
    let rig = Rig::new();
    rig.script_moves(vec![-20, 40, 70, 85, 92, 96, 98]);
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(100, 0));

    assert!(placement.ok, "{placement:?}");
    let writes = rig.writes();
    assert_eq!(
        writes[1], "D 240 0\n",
        "halving the gain asks twice the remaining error, not four times it: {writes:?}"
    );
    assert!(
        placement.steps <= 8,
        "the loop converges after the bad sample: {} steps, {writes:?}",
        placement.steps
    );
}

#[test]
fn a_half_gain_machine_converges_within_eight_steps() {
    // The machine moves half of what was asked (a HID-to-cursor scale below 1:1). The old divisor
    // could only grow, so it needed ~40 steps from 600 px — more than MAX_STEPS (HW-F9).
    let rig = Rig::new();
    rig.set_gain_percent(50);
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(600, 0));

    assert!(placement.ok, "{placement:?}");
    assert!(
        placement.steps <= 8,
        "converged in {} steps: {:?}",
        placement.steps,
        rig.writes()
    );
    assert!(
        placement.x.is_some_and(|x| (x - 600).abs() <= TOLERANCE_PX),
        "{:?}",
        placement.x
    );
}

#[test]
fn a_one_and_a_half_gain_machine_does_not_oscillate() {
    let rig = Rig::new();
    rig.set_gain_percent(150);
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(600, 0));

    assert!(placement.ok, "{placement:?}");
    assert!(
        placement.steps <= 8,
        "converged in {} steps: {:?}",
        placement.steps,
        rig.writes()
    );
}

#[test]
fn a_swallowed_move_is_measured_as_no_motion_and_the_loop_recovers() {
    // "Enhance pointer precision" can round a whole correction away. The old loop doubled its
    // divisor permanently on that sample; the gain estimate halves once, asks bigger, and the next
    // clean move brings it back (HW-F9).
    let rig = Rig::new();
    rig.swallow_first_move();
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(100, 0));

    assert!(placement.ok, "{placement:?}");
    assert!(
        placement.steps <= 8,
        "converged in {} steps: {:?}",
        placement.steps,
        rig.writes()
    );
}

#[test]
fn an_epp_curve_that_halves_small_moves_still_converges() {
    // "Enhance pointer precision" accelerates small counts away: a 5-count ask lands as 2. The gain
    // estimate must still converge, not stall (HW-F9).
    let rig = Rig::new();
    rig.epp_below(10);
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(605, 0));

    assert!(placement.ok, "{placement:?}");
    assert!(
        placement.steps <= 8,
        "converged in {} steps: {:?}",
        placement.steps,
        rig.writes()
    );
}

#[test]
fn a_small_ask_that_is_swallowed_does_not_burn_the_full_settle_budget() {
    // A ≤3 count ask that produces no motion is swallowed, not "still arriving": waiting the full
    // 24 × 15 ms for it costs 360 ms per correction and tells the loop nothing (HW-F9).
    let rig = Rig::new();
    rig.swallow_first_move();
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    // Goal 3: the first ask is 3 (inside the small-ask band) and this board swallows it.
    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(3, 0));

    assert!(placement.ok, "{placement:?}");
    assert_eq!(placement.steps, 3, "{:?}", rig.writes());
    assert!(
        rig.sleeps().len() <= 6,
        "the swallowed small ask gave up after {} polls instead of the full budget",
        rig.sleeps().len()
    );
}

#[test]
fn a_single_step_is_clamped_so_a_wrong_target_cannot_fling_the_pointer() {
    let rig = Rig::new();
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    // A 1200 px move is two clamped 600 px steps on a healthy board.
    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(1200, 0));

    assert!(placement.ok);
    assert_eq!(rig.writes(), vec!["D 600 0\n", "D 600 0\n"]);
    assert_eq!(MAX_STEP_PX, 600);
}

#[test]
fn the_step_budget_is_bounded_and_reports_goal_and_actual() {
    // A wedged pointer: every move is accepted and nothing moves. The loop must stop at the budget
    // rather than hang the tool, and the failure must name BOTH the target and where it ended.
    let rig = Rig::new();
    rig.set_position(0, 0);
    rig.set_stuck();
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(100, 0));

    assert!(!placement.ok);
    assert_eq!(placement.steps, MAX_STEPS);
    assert_eq!(rig.writes().len(), MAX_STEPS as usize);
    assert_eq!(placement.x, Some(0));
    assert_eq!(placement.y, Some(0));
    assert_eq!(
        placement.error.as_deref(),
        Some("the cursor wouldn't move to (100,0) — it stopped at (0,0)")
    );
    assert!(
        rig.sleeps().len() > MAX_SETTLE_POLLS_EXPECTED,
        "each stationary correction waits out the settle budget"
    );
}

const MAX_SETTLE_POLLS_EXPECTED: usize = 24;

#[test]
fn an_unreadable_cursor_is_refused_before_anything_is_sent() {
    let rig = Rig::new();
    rig.script_reads(vec![None]);
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(10, 10));

    assert!(!placement.ok);
    assert_eq!(placement.steps, 0);
    assert_eq!((placement.x, placement.y), (None, None));
    assert_eq!(
        placement.error.as_deref(),
        Some("the cursor position can't be read (GetCursorPos failed)")
    );
    assert!(
        rig.writes().is_empty(),
        "nothing is sent when the cursor can't be read"
    );
}

#[test]
fn a_cursor_that_becomes_unreadable_mid_run_stops_the_loop() {
    let rig = Rig::new();
    // First loop read works, then the settle poll cannot read, then the next loop read fails.
    rig.script_reads(vec![Some((0, 0)), Some((0, 0)), None, None]);
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(10, 0));

    assert!(!placement.ok);
    assert_eq!(placement.steps, 1);
    assert_eq!(
        placement.error.as_deref(),
        Some("the cursor position can't be read (GetCursorPos failed)")
    );
}

#[test]
fn a_board_that_will_not_take_the_move_reports_why() {
    let rig = Rig::new();
    rig.fail_writes("the port is not open");
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    let placement = place(&mut transport, &mut cursor, &rig.clock(), goal(10, 0));

    assert!(!placement.ok);
    assert!(
        placement
            .error
            .as_deref()
            .is_some_and(|error| error.contains("the port is not open")),
        "the refusal carries the transport's line: {:?}",
        placement.error
    );
    assert_eq!(placement.x, Some(0));
}

#[test]
fn the_tolerance_is_v2s_two_pixels() {
    let rig = Rig::new();
    rig.set_position(100, 100);
    let mut transport = rig.transport();
    let mut cursor = rig.cursor();

    // 2 px off is a hit; 3 px off is a correction.
    let hit = place(&mut transport, &mut cursor, &rig.clock(), goal(102, 102));
    assert!(hit.ok);
    assert_eq!(hit.steps, 0);

    let miss = place(&mut transport, &mut cursor, &rig.clock(), goal(103, 100));
    assert!(miss.ok);
    assert_eq!(miss.steps, 1);
    assert_eq!(TOLERANCE_PX, 2);
}

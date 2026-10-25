//! The cancellation types: v2's `ToolBase.SleepCheck` + `WithoutCancellation`, as a signature.
//!
//! The v2 rule is "a stop abandons the WORK, not the CLEANUP": `SleepCheck` throws so a step in
//! progress is abandoned, but it throws from ANY point — including inside teardown, where it aborted
//! the close of a window and replaced the exception it was unwinding. The fix there was a mute flag
//! and the discipline to use it (`ToolBase.WithoutCancellation`). Here the mute is a TYPE: cleanup
//! code receives [`Cleanup`], which has no cancellation check at all, so "this is cleanup" is
//! visible at the call site and cannot be forgotten by the code it protects.
//!
//! These tests port `ToolBaseCancellationTests.cs` case for case, plus the token sharing a clone
//! relies on.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use seal_tools::cancel::{CancelToken, Cancelled, RunControl};
use seal_tools::clock::Clock;

/// A clock that records what it was asked to sleep and never really waits.
#[derive(Default)]
struct RecordingClock {
    slept: Mutex<Vec<f64>>,
}

impl RecordingClock {
    fn slept(&self) -> Vec<f64> {
        self.slept
            .lock()
            .expect("the clock is not poisoned")
            .clone()
    }

    fn total(&self) -> f64 {
        self.slept().iter().sum()
    }
}

impl Clock for RecordingClock {
    fn sleep_seconds(&self, seconds: f64) {
        self.slept
            .lock()
            .expect("the clock is not poisoned")
            .push(seconds);
    }
}

/// A clock that stops the run during its first tick — the "cancelling mid-sleep" case, with no real
/// time and no second thread involved.
struct CancellingClock {
    token: CancelToken,
    slept: Mutex<Vec<f64>>,
}

impl CancellingClock {
    fn new(token: CancelToken) -> Self {
        Self {
            token,
            slept: Mutex::new(Vec::new()),
        }
    }

    fn ticks(&self) -> usize {
        self.slept.lock().expect("the clock is not poisoned").len()
    }

    fn total(&self) -> f64 {
        self.slept
            .lock()
            .expect("the clock is not poisoned")
            .iter()
            .sum()
    }
}

impl Clock for CancellingClock {
    fn sleep_seconds(&self, seconds: f64) {
        self.slept
            .lock()
            .expect("the clock is not poisoned")
            .push(seconds);
        self.token.cancel();
    }
}

// ── the token itself ────────────────────────────────────────────────────────────────────────────

#[test]
fn a_fresh_token_is_not_cancelled_and_checks_clean() {
    let control = RunControl::new(CancelToken::new());

    assert!(!control.is_cancelled());
    assert_eq!(control.check(), Ok(()));
}

#[test]
fn a_cancelled_run_checks_as_stopped() {
    let token = CancelToken::new();
    let control = RunControl::new(token.clone());
    token.cancel();

    assert!(control.is_cancelled());
    assert_eq!(control.check(), Err(Cancelled));
}

#[test]
fn cancellation_is_shared_between_clones_and_is_one_way() {
    // The arbiter holds the token it cancels; the runner holds the one it polls. That is one signal,
    // not a copy that could disagree — and nothing can un-cancel it.
    let token = CancelToken::new();
    let control = RunControl::new(token.clone());

    token.cancel();
    token.cancel();
    assert!(control.is_cancelled());
    assert_eq!(control.check(), Err(Cancelled));
}

// ── sleeping, and being abandoned DURING it (the pet feeder's ~30 s reload) ─────────────────────

#[test]
fn an_uncancelled_sleep_returns_after_the_whole_wait() {
    let control = RunControl::new(CancelToken::new());
    let clock = RecordingClock::default();

    assert_eq!(control.sleep(&clock, 0.2), Ok(()));

    assert_eq!(clock.slept().len(), 4, "0.2 s is four 50 ms ticks");
    assert!((clock.total() - 0.2).abs() < 1e-9, "{:?}", clock.slept());
}

#[test]
fn a_cancelled_sleep_stops_it_early_rather_than_at_the_end() {
    // The whole point: a long step is abandoned DURING it. Without this, a pet visit's ~30 s reload
    // would run to completion before anything noticed the stop, which reads as a broken Stop button.
    let token = CancelToken::new();
    let clock = CancellingClock::new(token.clone());
    let control = RunControl::new(token);

    assert_eq!(control.sleep(&clock, 5.0), Err(Cancelled));
    assert_eq!(
        clock.ticks(),
        1,
        "the first 50 ms tick must observe the stop, not the hundredth"
    );
    assert!(clock.total() < 5.0);
}

#[test]
fn a_sleep_that_is_already_cancelled_still_observes_the_stop() {
    let token = CancelToken::new();
    let control_token = token.clone();
    token.cancel();
    let control = RunControl::new(control_token);
    let clock = RecordingClock::default();

    assert_eq!(control.sleep(&clock, 1.0), Err(Cancelled));
}

#[test]
fn a_zero_wait_still_observes_the_stop() {
    // v2 clamps to at least one step, so even a 0 s wait checks the token. A step boundary that
    // quietly skipped the check would be a stop that works everywhere except there.
    let token = CancelToken::new();
    token.cancel();
    let control = RunControl::new(token);
    let clock = RecordingClock::default();

    assert_eq!(control.sleep(&clock, 0.0), Err(Cancelled));
    assert_eq!(clock.slept().len(), 1);
}

#[test]
fn a_negative_or_nonsense_wait_is_clamped_not_rejected() {
    // v2's `SleepCheck` clamps with `Math.Max(1, …)` rather than failing; a nonsense number from a
    // config is a tick, not a crash.
    let control = RunControl::new(CancelToken::new());
    let clock = RecordingClock::default();

    assert_eq!(control.sleep(&clock, -3.0), Ok(()));
    assert_eq!(clock.slept().len(), 1);
    assert!(clock.total() > 0.0);
}

// ── "a stop abandons the WORK, not the CLEANUP" ────────────────────────────────────────────────

#[test]
fn a_sleep_inside_cleanup_does_not_throw_even_when_the_run_is_cancelled() {
    let token = CancelToken::new();
    token.cancel();
    let mut control = RunControl::new(token);
    let clock = RecordingClock::default();

    // The cleanup type has no cancellation check to throw with — that is the signature doing the
    // work v2 needed a mute flag for.
    control.without_cancellation(|cleanup| cleanup.sleep(&clock, 0.1));

    assert!(clock.total() > 0.0, "the cleanup really did its wait");
}

#[test]
fn the_stop_is_observed_again_after_the_cleanup_ends() {
    // If it were not, the FIRST stop would leave the tool permanently unable to be stopped — a far
    // worse bug than the one the mute fixes. With a borrowed cleanup section this is enforced by the
    // borrow ending, and asserted here so a future redesign cannot lose it.
    let token = CancelToken::new();
    token.cancel();
    let mut control = RunControl::new(token);
    let clock = RecordingClock::default();

    control.without_cancellation(|cleanup| cleanup.sleep(&clock, 0.05));

    assert_eq!(control.check(), Err(Cancelled));
    assert_eq!(control.sleep(&clock, 0.05), Err(Cancelled));
}

#[test]
fn a_finished_inner_cleanup_does_not_end_an_outer_one() {
    // Nesting is real: a cleanup helper may itself have cleanup. The inner section ending must not
    // hand cancellation back while the outer one is still running.
    let token = CancelToken::new();
    token.cancel();
    let mut control = RunControl::new(token);
    let clock = RecordingClock::default();

    control.without_cancellation(|outer| {
        outer.without_cancellation(|inner| inner.sleep(&clock, 0.05));
        // Still inside the outer cleanup: this sleep must also be quiet.
        outer.sleep(&clock, 0.05);
    });

    assert_eq!(control.check(), Err(Cancelled));
}

#[test]
fn a_cleanup_that_panics_still_leaves_the_run_stoppable() {
    // v2 restored the mute in a `finally` for exactly this: a teardown that throws is the case a
    // teardown is most likely to be running. The borrowed cleanup section unwinds with the panic and
    // the control is usable again.
    let token = CancelToken::new();
    token.cancel();
    let mut control = RunControl::new(token);

    let panicked = catch_unwind(AssertUnwindSafe(|| {
        control.without_cancellation(|_cleanup| -> () { panic!("teardown failed") });
    }));

    assert!(panicked.is_err(), "the cleanup's panic propagates");
    assert_eq!(control.check(), Err(Cancelled));
}

#[test]
fn a_cleanup_section_returns_the_closure_value() {
    let mut control = RunControl::new(CancelToken::new());

    let value = control.without_cancellation(|_cleanup| 7);

    assert_eq!(value, 7);
}

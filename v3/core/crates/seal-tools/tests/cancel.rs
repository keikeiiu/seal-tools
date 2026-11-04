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

use seal_tools::cancel::{CancelToken, Cancelled, MAX_WAIT_SECONDS, RunControl, TICK_SECONDS};
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

/// TOOL-F16: a huge or infinite wait used to saturate the tick count (`usize::MAX` steps) and then
/// busy-spin at 100 % CPU, because the real clock refused the overflowing `Duration`. The wait is
/// clamped to one day before any tick arithmetic happens.
#[test]
fn an_absurd_wait_is_clamped_to_a_day() {
    /// Counts ticks without storing them: the clamped wait is 24 h of 50 ms ticks.
    struct CountingClock {
        ticks: std::cell::Cell<usize>,
        total: std::cell::Cell<f64>,
    }

    impl Clock for CountingClock {
        fn sleep_seconds(&self, seconds: f64) {
            self.ticks.set(self.ticks.get() + 1);
            self.total.set(self.total.get() + seconds);
        }
    }

    let control = RunControl::new(CancelToken::new());
    for absurd in [f64::INFINITY, f64::MAX, 1.0e12] {
        let clock = CountingClock {
            ticks: std::cell::Cell::new(0),
            total: std::cell::Cell::new(0.0),
        };

        assert_eq!(control.sleep(&clock, absurd), Ok(()));

        let expected = (MAX_WAIT_SECONDS / TICK_SECONDS) as usize;
        assert_eq!(
            clock.ticks.get(),
            expected,
            "{absurd} was clamped to {MAX_WAIT_SECONDS} s"
        );
        assert!(
            (clock.total.get() - MAX_WAIT_SECONDS).abs() < 1e-3,
            "{absurd}: the clamped wait is the day, not the input: {}",
            clock.total.get()
        );
    }
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

// ── the deadline wait and the suspend it can see (PET-F12) ─────────────────────────────────────

/// A clock with both readings. Each sleep advances the clocks by what was asked plus `overshoot`
/// (a slow timer, the thing a sum of relative sleeps accumulates), and a one-shot `jump` is applied
/// to the WALL clock only — which is what a suspend looks like from inside a wait.
struct TimeKeepingClock {
    state: Mutex<TimeState>,
}

struct TimeState {
    monotonic: f64,
    wall: f64,
    overshoot: f64,
    jump: Option<f64>,
    slept: Vec<f64>,
}

impl TimeKeepingClock {
    fn new(overshoot: f64) -> Self {
        Self {
            state: Mutex::new(TimeState {
                monotonic: 0.0,
                wall: 0.0,
                overshoot,
                jump: None,
                slept: Vec::new(),
            }),
        }
    }

    fn with_jump(overshoot: f64, jump: f64) -> Self {
        let clock = Self::new(overshoot);
        clock.state.lock().expect("not poisoned").jump = Some(jump);
        clock
    }

    fn slept(&self) -> Vec<f64> {
        self.state.lock().expect("not poisoned").slept.clone()
    }

    fn monotonic(&self) -> f64 {
        self.state.lock().expect("not poisoned").monotonic
    }
}

impl Clock for TimeKeepingClock {
    fn sleep_seconds(&self, seconds: f64) {
        let mut state = self.state.lock().expect("not poisoned");
        state.slept.push(seconds);
        state.monotonic += seconds + state.overshoot;
        state.wall += seconds + state.overshoot;
        if let Some(jump) = state.jump.take() {
            state.wall += jump;
        }
    }

    fn monotonic_seconds(&self) -> Option<f64> {
        Some(self.monotonic())
    }

    fn wall_seconds(&self) -> Option<f64> {
        Some(1_000.0 + self.state.lock().expect("not poisoned").wall)
    }
}

#[test]
fn a_timed_wait_sleeps_to_a_deadline_instead_of_adding_ticks() {
    // Each tick overruns by 10 ms (a real timer's resolution). The old sum of 50 ms sleeps asked for
    // 0.2 s and let the error accumulate; the deadline asks for what is LEFT, so the last tick is the
    // remainder rather than another 50 ms.
    let control = RunControl::new(CancelToken::new());
    let clock = TimeKeepingClock::new(0.01);

    assert_eq!(control.sleep(&clock, 0.2), Ok(()));

    let last = clock.slept().last().copied().expect("at least one tick");
    assert!(
        (last - 0.02).abs() < 1e-9,
        "the last tick is the remainder the deadline left: {:?}",
        clock.slept()
    );
    assert!(
        clock.monotonic() <= 0.2 + TICK_SECONDS,
        "the wait ends at the deadline, not a tick past it: {}",
        clock.monotonic()
    );
}

#[test]
fn a_wall_clock_jump_during_a_wait_is_reported_as_a_resume() {
    // The machine slept for 90 s in the middle of a five-minute wait. The wall clock saw it, the
    // monotonic clock did not: the wait ends there and the run is told how much real time passed.
    let control = RunControl::new(CancelToken::new());
    let clock = TimeKeepingClock::with_jump(0.0, 90.0);

    assert_eq!(control.sleep(&clock, 300.0), Ok(()));

    assert_eq!(
        clock.slept().len(),
        1,
        "the wait does not keep sleeping through a suspend: {:?}",
        clock.slept()
    );
    let slept = control.take_resume().expect("the resume is reported");
    assert!(
        (slept - 90.05).abs() < 1e-6,
        "real time that passed: {slept}"
    );
    assert_eq!(
        control.take_resume(),
        None,
        "a resume is reported exactly once"
    );
}

#[test]
fn ordinary_timer_drift_is_not_a_resume() {
    // A tick arriving a little late is not a suspend: the threshold is well above any scheduling
    // latency, so the wait finishes normally and the run is not told to re-read.
    let control = RunControl::new(CancelToken::new());
    let clock = TimeKeepingClock::with_jump(0.0, 1.0);

    assert_eq!(control.sleep(&clock, 0.2), Ok(()));

    assert!(clock.slept().len() >= 4, "{:?}", clock.slept());
    assert_eq!(control.take_resume(), None);
}

#[test]
fn a_timed_wait_still_observes_a_stop() {
    let token = CancelToken::new();
    let control = RunControl::new(token.clone());
    let clock = TimeKeepingClock::new(0.0);
    // Cancels on the first tick, which must be seen during the wait rather than at the deadline.
    struct Cancelling(TimeKeepingClock, CancelToken);
    impl Clock for Cancelling {
        fn sleep_seconds(&self, seconds: f64) {
            self.0.sleep_seconds(seconds);
            self.1.cancel();
        }
        fn monotonic_seconds(&self) -> Option<f64> {
            self.0.monotonic_seconds()
        }
        fn wall_seconds(&self) -> Option<f64> {
            self.0.wall_seconds()
        }
    }
    let cancelling = Cancelling(clock, token);

    assert_eq!(control.sleep(&cancelling, 5.0), Err(Cancelled));
    assert_eq!(
        cancelling.0.slept().len(),
        1,
        "the first tick observed the stop"
    );
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

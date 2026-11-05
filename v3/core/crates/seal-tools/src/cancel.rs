//! The cancellation types: v2's `ToolBase.SleepCheck` and `WithoutCancellation`, as a signature.
//!
//! The v2 rule is "a stop abandons the WORK, not the CLEANUP" (`PROGRESS.md` entries 62 and 65).
//! `SleepCheck` throws so a step in progress is abandoned instead of finishing first ??without that,
//! Stop on the pet feeder was honoured only at a visit boundary, which reads exactly like a broken
//! button. But it throws from ANY point, including inside teardown, so the code that closes a window
//! or releases a button was itself being cancelled, and a `finally` that throws REPLACES the
//! exception it was unwinding.
//!
//! v2's fix was a mute flag plus the discipline to wrap every teardown in `WithoutCancellation`.
//! Here the mute is a TYPE: cleanup code receives [`Cleanup`], which has no cancellation check at
//! all. "This is cleanup" is then visible at the call site, a cleanup section cannot accidentally
//! observe the stop, and the stop is observed again the moment the section's borrow ends. Nothing
//! can un-cancel a [`CancelToken`] ??cancellation is one way.
//!
//! The token is shared between the arbiter (which cancels) and the runner (which polls), so those
//! two never hold copies that can disagree.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::clock::Clock;

/// One tick of a wait. v2's `SleepCheck` sleeps in 50 ms chunks so a stop is noticed during a long
/// wait rather than after it.
pub const TICK_SECONDS: f64 = 0.05;

/// The floor for a single tick, in seconds. v2's `Math.Max(1, ??` of a millisecond ??it exists so a
/// zero or negative wait still passes through at least one tick and therefore observes the token.
pub const MIN_TICK_SECONDS: f64 = 0.001;

/// How far the wall clock may run ahead of the monotonic clock before a wait is treated as having
/// been INTERRUPTED BY A SUSPEND rather than merely having overshot: a tick is 50 ms, so five seconds
/// is two orders of magnitude past any scheduling latency and still far below a real sleep.
pub const RESUME_JUMP_SECONDS: f64 = 5.0;

/// The longest a single wait may be asked for (TOOL-F16). A huge or infinite `seconds` used to
/// saturate the tick count ??`usize::MAX` steps ??and the real clock then refused the overflowing
/// `Duration`, so the loop busy-spun at 100 % CPU until Stop. A day is longer than any honest wait in
/// this suite (the pet's cycle tops out around four hours).
pub const MAX_WAIT_SECONDS: f64 = 24.0 * 60.0 * 60.0;

/// The stop signal one run polls. Cloning is cheap and yields the SAME signal, not a copy that could
/// drift: the arbiter holds a clone to cancel, the loop holds one to poll.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops the run. One way by construction: nothing sets this back, so a section of code cannot
    /// "restore" a run somebody else ended.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// [`Cancelled`] when the run has been stopped, `Ok(())` while it may continue.
    ///
    /// # Errors
    ///
    /// Returns [`Cancelled`] when the run has been stopped; `Ok(())` carries no error.
    pub fn check(&self) -> Result<(), Cancelled> {
        if self.is_cancelled() {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}

/// The stop arrived. The loop unwinds with this; every `Drop` and cleanup section still runs, which
/// is the whole point of it being an unwind rather than a silent `break`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the run was stopped")
    }
}

impl std::error::Error for Cancelled {}

/// One run's view of its stop signal: the loop's `check`/`sleep`, and the cleanup section that is
/// deliberately deaf to both.
pub struct RunControl {
    token: CancelToken,
    /// Set when a wait was interrupted by a suspend, carrying how much real time passed; read by the
    /// runner through [`Self::take_resume`] and given to the tool (PET-F12).
    resume: Mutex<Option<f64>>,
}

impl RunControl {
    #[must_use]
    pub fn new(token: CancelToken) -> Self {
        Self {
            token,
            resume: Mutex::new(None),
        }
    }

    /// The raw signal, for a loop's own exit test (`v2`'s `ct.IsCancellationRequested` in the loop
    /// head). Cleanup code cannot reach it: a [`Cleanup`] section only has its own quiet operations.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// # Errors
    ///
    /// Returns [`Cancelled`] when the run has been stopped; `Ok(())` while it may continue.
    pub fn check(&self) -> Result<(), Cancelled> {
        self.token.check()
    }

    /// Whether the last wait was interrupted by a suspend, and how much REAL time passed across it.
    /// Takes the flag: one resume is reported once (PET-F12).
    #[must_use]
    pub fn take_resume(&self) -> Option<f64> {
        self.resume
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    /// Waits, watching for a stop between ticks, and returns [`Cancelled`] the moment one arrives ??    /// mid-wait, not at the end. A non-positive or nonsense wait is clamped to one tick (v2's
    /// `Math.Max(1, ??`) rather than rejected: a nonsense number from a config is a tick, not a
    /// crash.
    ///
    /// Two implementations, by what the clock can report. A clock with a MONOTONIC reading sleeps to
    /// a DEADLINE ??each tick asks for the time actually left, so a 205-minute wait no longer grows
    /// by the timer's own overshoot 246,000 times over ??and compares the wall clock with it so a
    /// machine that slept through the wait says so ([`Self::take_resume`]). A clock with no reading
    /// (the tests') keeps the count-the-ticks behaviour.
    ///
    /// # Errors
    ///
    /// Returns [`Cancelled`] as soon as the token is set, which can be during the every-tick check
    /// rather than at the end of the wait.
    pub fn sleep(&self, clock: &dyn Clock, seconds: f64) -> Result<(), Cancelled> {
        let seconds = clamp_wait(seconds);
        match clock.monotonic_seconds() {
            Some(now) => self.sleep_to_deadline(clock, seconds, now),
            None => self.sleep_in_ticks(clock, seconds),
        }
    }

    /// The deadline wait: each tick sleeps whatever is actually left, and the wall clock is checked
    /// for a jump the monotonic clock did not see (PET-F12).
    fn sleep_to_deadline(
        &self,
        clock: &dyn Clock,
        seconds: f64,
        start: f64,
    ) -> Result<(), Cancelled> {
        let total = seconds.max(0.0);
        let start_wall = clock.wall_seconds();
        let deadline = start + total;
        // A zero-length wait still observes the stop (v2's `Math.Max(1, ??`); the loop below would
        // otherwise return without checking.
        self.check()?;
        loop {
            let Some(now) = clock.monotonic_seconds() else {
                return Ok(());
            };
            let remaining = deadline - now;
            if remaining <= 0.0 {
                return Ok(());
            }
            clock.sleep_seconds(remaining.clamp(MIN_TICK_SECONDS, TICK_SECONDS));
            self.check()?;
            let elapsed_monotonic = clock
                .monotonic_seconds()
                .map_or(0.0, |after| (after - start).max(0.0));
            let Some(elapsed_wall) = start_wall.and_then(|wall| {
                clock
                    .wall_seconds()
                    .map(|after| (after - wall).max(elapsed_monotonic))
            }) else {
                continue;
            };
            if elapsed_wall - elapsed_monotonic >= RESUME_JUMP_SECONDS {
                // The wall clock ran while the monotonic one did not: the machine was SUSPENDED.
                // Stop waiting (the deadline means nothing across a suspend) and report how much real
                // time passed, so the tool re-reads instead of acting on a stale screen.
                *self
                    .resume
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(elapsed_wall);
                return Ok(());
            }
        }
    }

    /// The tick wait for a clock that cannot tell time: the count is what the caller asked for, and
    /// every tick carries an equal share so the total is still the requested wait.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a wait length converted to 50 ms ticks: the count is clamped to >= 1 first, a \
                  tick count is exact in f64, and only the sub-tick fraction is truncated"
    )]
    fn sleep_in_ticks(&self, clock: &dyn Clock, seconds: f64) -> Result<(), Cancelled> {
        let steps = ((seconds / TICK_SECONDS) as usize).max(1);
        let tick = (seconds / steps as f64).max(MIN_TICK_SECONDS);
        for _ in 0..steps {
            clock.sleep_seconds(tick);
            self.check()?;
        }
        Ok(())
    }

    /// Runs teardown that must finish even though the run is being stopped.
    ///
    /// The section receives a [`Cleanup`] ??a type with no cancellation check ??and its borrow of
    /// this control ends with the call, so the next `check`/`sleep` outside it observes the stop
    /// again. Nestable by reborrowing, and a panic inside unwinds without leaving any state behind to
    /// restore: there is no mute flag to get stuck.
    pub fn without_cancellation<R>(&mut self, teardown: impl FnOnce(&mut Cleanup<'_>) -> R) -> R {
        let mut cleanup = Cleanup { control: self };
        teardown(&mut cleanup)
    }
}

/// How often a cleanup section currently uses [`RunControl`]'s operations. It has none.
///
/// This is the compile-time half of v2's `WithoutCancellation`: within a cleanup section the quiet
/// `sleep` below is the only wait in reach, so "a stop abandons the work, not the cleanup" cannot be
/// lost by forgetting to set a flag.
pub struct Cleanup<'a> {
    control: &'a mut RunControl,
}

impl Cleanup<'_> {
    /// Waits without watching for the stop ??the operation the whole type exists to provide.
    ///
    /// One call rather than v2's 50 ms ticks: there is nothing to observe between ticks here, so
    /// chunking would only add places for a future reader to think the tolerance is conditional.
    pub fn sleep(&mut self, clock: &dyn Clock, seconds: f64) {
        if seconds > 0.0 {
            clock.sleep_seconds(seconds);
        }
    }

    /// Nested cleanup. A cleanup helper may itself have cleanup, and the inner section ending must
    /// not hand cancellation back while the outer one is still running ??exactly v2's nesting rule,
    /// enforced here by the reborrow rather than by restoring a saved flag value.
    pub fn without_cancellation<R>(&mut self, teardown: impl FnOnce(&mut Cleanup<'_>) -> R) -> R {
        let mut inner = Cleanup {
            control: &mut *self.control,
        };
        teardown(&mut inner)
    }
}

/// The wait a [`RunControl::sleep`] really pays: negatives and nonsense become a tick, and anything
/// past [`MAX_WAIT_SECONDS`] becomes that ceiling instead of a number of ticks the process cannot
/// represent (TOOL-F16).
fn clamp_wait(seconds: f64) -> f64 {
    if seconds.is_finite() {
        seconds.clamp(0.0, MAX_WAIT_SECONDS)
    } else {
        MAX_WAIT_SECONDS
    }
}

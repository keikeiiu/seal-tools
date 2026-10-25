//! `Clock`: the one impure thing the loop needs, passed in so the tests never wait.
//!
//! `PLAN-V3-CORE.md` §6.4 splits each tool into a pure decision half and a thin effect loop; the
//! loop still has to pass time between decisions (`Decision::Wait`, ported from `ToolBase.SleepCheck`
//! — which takes SECONDS, not milliseconds: passing one to the other slept for eleven minutes once,
//! live). Putting the sleep behind this trait is what lets the tests drive a whole run without
//! waiting and what lets the real loop be given a clock that sleeps in small enough chunks for a stop
//! to be noticed during one.

use std::thread;
use std::time::Duration;

/// Something the loop can pass time with.
pub trait Clock {
    /// Passes `seconds` of time. A negative or nonsense value is the caller's problem (the
    /// cancellation sleep clamps it); this waits what it is given.
    fn sleep_seconds(&self, seconds: f64);
}

/// The real clock, for the binary: the only implementation that actually waits.
///
/// It is here so the crate has one and `seal-core` does not need a second copy; it performs no I/O,
/// and `Duration::try_from_secs_f64` refuses a nonsense value rather than panicking the tool thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealClock;

impl Clock for RealClock {
    fn sleep_seconds(&self, seconds: f64) {
        if let Ok(duration) = Duration::try_from_secs_f64(seconds.max(0.0)) {
            thread::sleep(duration);
        }
    }
}

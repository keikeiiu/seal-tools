//! `Clock`: the one impure thing the loop needs, passed in so the tests never wait.
//!
//! `PLAN-V3-CORE.md` §6.4 splits each tool into a pure decision half and a thin effect loop; the
//! loop still has to pass time between decisions (`Decision::Wait`, ported from `ToolBase.SleepCheck`
//! — which takes SECONDS, not milliseconds: passing one to the other slept for eleven minutes once,
//! live). Putting the sleep behind this trait is what lets the tests drive a whole run without
//! waiting and what lets the real loop be given a clock that sleeps in small enough chunks for a stop
//! to be noticed during one.
//!
//! # Why it also reads the time (PET-F12)
//!
//! A wait used to be a SUM OF 50 ms RELATIVE SLEEPS: each tick overshot by the timer's own latency and
//! the errors accumulated over a 205-minute wait, and nothing could tell that the machine had slept
//! through the wait — after a suspend the loop just carried on clicking into a client that was
//! probably disconnected. Two readings fix both: a MONOTONIC one to sleep to a deadline, and a WALL
//! one to notice that more time passed than the monotonic clock saw.

use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Something the loop can pass time with.
pub trait Clock {
    /// Passes `seconds` of time. A negative or nonsense value is the caller's problem (the
    /// cancellation sleep clamps it); this waits what it is given.
    fn sleep_seconds(&self, seconds: f64);

    /// Seconds since an arbitrary fixed point, immune to the user changing the clock — `None` when
    /// this clock cannot tell time, in which case waits fall back to counting ticks.
    fn monotonic_seconds(&self) -> Option<f64> {
        None
    }

    /// Seconds since the Unix epoch — the reading that KEEPS RUNNING while the machine is asleep,
    /// which is what makes a resume detectable. `None` when this clock cannot tell time.
    fn wall_seconds(&self) -> Option<f64> {
        None
    }
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

    fn monotonic_seconds(&self) -> Option<f64> {
        // One process-wide epoch, so the value is comparable across the calls of a single wait (and
        // harmless if a second clock object is made): only differences within one wait are used.
        static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        Some(EPOCH.get_or_init(Instant::now).elapsed().as_secs_f64())
    }

    fn wall_seconds(&self) -> Option<f64> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|since| since.as_secs_f64())
    }
}

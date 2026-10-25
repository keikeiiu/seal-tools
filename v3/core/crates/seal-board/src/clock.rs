//! A `Clock` the waits take, so every test pays nothing and the unit is pinned.
//!
//! `DESIGN.md` §2.1: "**`SleepCheck` takes SECONDS.** The launcher's `Task.Delay` takes
//! milliseconds. Passing one to the other sleeps for eleven minutes and looks exactly like a hang —
//! it has happened, once, live." Every method here is seconds, named seconds, and the settle-wait
//! test pins that a 15 ms poll is handed over as `0.015`.
//!
//! This trait duplicates `seal-tools::clock::Clock` (same one method, same unit) because `seal-board`
//! does not depend on `seal-tools` and Cargo edits are the Lead's. If the two should meet, the home
//! is `seal-domain`, not a third copy — reported with the crate.

/// Sleeping, in SECONDS.
pub trait Clock {
    /// Sleeps for `seconds`. Negative or nonsense values are the caller's bug; the real clock clamps
    /// to zero rather than panicking (a panic inside a worker would take the whole core down).
    fn sleep_seconds(&self, seconds: f64);
}

/// The real clock — the only thing in this crate that touches the OS's time.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealClock;

impl Clock for RealClock {
    fn sleep_seconds(&self, seconds: f64) {
        if let Ok(duration) = std::time::Duration::try_from_secs_f64(seconds.max(0.0)) {
            std::thread::sleep(duration);
        }
    }
}

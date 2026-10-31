//! The game port: the capability a RUNNING tool uses to ask for, give back, and be told about the
//! one game.
//!
//! The arbiter (`seal-tools::arbiter`) already owns every rule — who holds the game, who is waiting,
//! who may stand down, and the bounded deferral of a foreground start behind the resident feeder.
//! What was missing after the pet feeder work was memory-free plumbing: a running worker could not
//! reach the arbiter, so the resident could not yield between visits and two runs could both be
//! started. This trait is that capability, and deliberately nothing more:
//!
//! - **It is not a decision.** It carries no rules: `request` asks, `release` gives back,
//!   `should_stand_down` answers the one question the spammer's loop will ask (increment 2). The
//!   arbiter decides; this only lets the worker talk to it.
//! - **`request` BLOCKS its caller until the game is granted.** For a resident that means its worker
//!   thread waits, producing no decisions, while a foreground tool finishes — exactly the yield the
//!   pet feeder needs between visits. It must never block the supervisor thread: the production port
//!   is a channel whose answers are stashed on the supervisor side.
//! - **`AllowAll`** is the fake/unit-test implementation: grants immediately, releases as a no-op,
//!   never stands down. The tools that take the game through `start` use it unchanged.

use crate::tool::ToolId;

/// A running tool's handle on the game hand-over. `Send + Sync` because one worker owns it while the
/// supervisor may clone the arc into another worker.
pub trait GamePort: Send + Sync {
    /// Ask for the game and BLOCK until the arbiter grants it, or return the line explaining why it
    /// cannot be granted (the core is shutting down, or the port is gone).
    ///
    /// # Errors
    ///
    /// Returns the line to show when the game cannot be granted: the core is shutting down, or the
    /// supervisor port is gone.
    fn request(&self, tool: ToolId) -> Result<(), String>;
    /// Give the game back. Idempotent and fire-and-forget: a release for a game the caller no longer
    /// holds is harmless (the arbiter answers `Freed`).
    fn release(&self, tool: ToolId);
    /// Whether this tool should step aside for an announced waiter. Only the spammer acts on it; the
    /// runner's yield hook is a recorded follow-up, so today this is the capability only.
    fn should_stand_down(&self) -> bool;
}

/// The implementation every fake and every tool that takes the game by `start` uses: grants,
/// releases, never stands down. Keeping it here means a test never needs a supervisor to exercise a
/// tool's decision half.
#[derive(Debug, Default, Clone, Copy)]
pub struct AllowAll;

impl GamePort for AllowAll {
    fn request(&self, _tool: ToolId) -> Result<(), String> {
        Ok(())
    }

    fn release(&self, _tool: ToolId) {}

    fn should_stand_down(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_all_grants_releases_and_never_stands_down() {
        let port = AllowAll;

        assert_eq!(port.request(ToolId::Pet), Ok(()));
        port.release(ToolId::Pet);
        port.release(ToolId::Tuner);
        assert!(!port.should_stand_down());
    }

    #[test]
    fn allow_all_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AllowAll>();
    }
}

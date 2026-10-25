//! The production registry: where a tool's decision half and its effects come from.
//!
//! The port has no real tools yet — the decision halves are in `seal-domain`, and the effects
//! (board, capture, OCR) are the Win32/serial/OCR crates that cannot build under the current
//! toolchain pin. The registry exists NOW so the supervisor's shape is the production shape, and the
//! placeholder REFUSES every start with a named line instead of accepting one and doing nothing —
//! "wired up and does nothing when Start is pressed" is the v2 failure class (`PROGRESS.md` entry
//! 59: the quest tool looked wired and did nothing, with the exception swallowed out of an async
//! handler).
//!
//! Tests inject a fake registry with fake tools/effects; the binary has no test-only switch.

use seal_tools::runner::{Effects, Tool};
use seal_tools::tool::ToolId;

/// One run's two halves: the decision half the runner calls, and the effects it performs decisions
/// with. They arrive together because a run cannot exist with only one of them.
pub struct Session {
    pub tool: Box<dyn Tool + Send>,
    pub effects: Box<dyn Effects + Send>,
}

/// A start refused before it becomes a run, with the line the player sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable {
    pub line: String,
}

/// Where tools come from.
///
/// [`Self::available`] is the cheap gate the supervisor checks BEFORE the arbiter accepts a start, so
/// an unavailable tool gets a clean refusal instead of an opening phase that immediately fails.
/// [`Self::create`] runs in the worker: for a real registry it is the OPENING phase (open the board,
/// load the OCR models), which is why it is not on the supervisor's thread.
pub trait ToolRegistry: Send + Sync {
    fn available(&self, tool: ToolId) -> Result<(), Unavailable>;
    fn create(&self, tool: ToolId) -> Result<Session, String>;
}

/// The line every unavailable tool reports. Defined where v3 has no player-facing text yet: the tool
/// is known to the protocol, but its loop is not ported in this build.
pub fn not_available_line(tool: ToolId) -> String {
    format!("\"{tool}\" is not available in this build yet — its tool loop is not ported.")
}

/// The binary's registry until the tools and their effects exist. Every start is refused by name.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProductionRegistry;

impl ToolRegistry for ProductionRegistry {
    fn available(&self, tool: ToolId) -> Result<(), Unavailable> {
        Err(Unavailable {
            line: not_available_line(tool),
        })
    }

    fn create(&self, tool: ToolId) -> Result<Session, String> {
        Err(not_available_line(tool))
    }
}

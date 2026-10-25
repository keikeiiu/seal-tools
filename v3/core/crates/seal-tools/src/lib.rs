//! The tool lifecycle and arbitration: one game, one tool at a time, and a stop that abandons the
//! WORK but never the CLEANUP (`PLAN-V3-CORE.md` §1, §6.4; v2's `LauncherService`, `ToolBase`).
//!
//! This crate is pure: no Win32, no serial port, no OCR, no threads bought from an operating system
//! — the loop that performs effects lives in `seal-core` and is implemented against the interfaces
//! here. What is here is the part v2 could not test: which tool may run, what a stop does to the
//! others, what a resident feeder does while another tool holds the game, and how held inputs are
//! guaranteed to be released on every exit path.
//!
//! The two shapes worth stating before anything is written: a [`seal_domain::decision::Decision`]
//! carrying its own message is what a tool's decision half returns, and every piece of published
//! state is an immutable snapshot, so a reader can never see a half-updated tool.

pub mod cancel;
pub mod clock;
pub mod held;
pub mod tool;

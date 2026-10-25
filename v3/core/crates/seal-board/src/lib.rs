//! The Arduino board side: the level-2 command grammar, the write/read transport seam, firmware-level
//! detection, the closed-loop cursor placement and the pure port choice.
//!
//! The grammar, the firmware level and the closed-loop placement are all over the
//! [`transport::Transport`] seam. The Windows OS adapter now EXISTS: [`serial`] opens the port with
//! v2's DTR/RTS sequence and boot wait, and [`enumerate`] reduces the OS port list to the decision
//! [`port_choice`] makes. Everything testable without a board is tested (fake device, fake cursor,
//! fake clock); the live-only facts are named in [`serial`] and exercised by `tests/live_only.rs`.
//!
//! The measured rules that must survive, each pinned by a test:
//!
//! - `D dx dy` counts are **RAW HID counts, never scaled** (`DESIGN.md` §2.1, `MOVE-SETS.md`):
//!   [`command::BoardCommand::move_by`] passes them through untouched.
//! - Placement is a **closed loop**: move, wait for the move to settle, read the cursor, correct,
//!   until within tolerance or the step budget runs out. A failure reports BOTH the goal and where
//!   the cursor stopped (`DESIGN.md` §2.1, `HidPointer.cs`).
//! - `V` is the **only reply the sketch ever sends**, so silence is an ANSWER: a board that does not
//!   answer predates version reporting (`firmware`, `FirmwareVersion.cs:10-16`).
//! - **SECONDS, not milliseconds.** `SleepCheck` takes seconds and the launcher's `Task.Delay` takes
//!   milliseconds; passing one to the other slept for eleven minutes and looked exactly like a hang —
//!   it happened, once, live (`DESIGN.md` §2.1). Every wait here goes through [`clock::Clock`], whose
//!   one method is named `sleep_seconds`.
//!
//! What is deliberately NOT here (the adapter's, with what would prove it — see [`transport`]):
//! port enumeration over WMI, opening the port, driver timeout behaviour, the 2 s boot wait, and
//! surviving the game's anti-cheat sharing the port.

pub mod clock;
pub mod command;
pub mod enumerate;
pub mod firmware;
pub mod placement;
pub mod port_choice;
pub mod serial;
pub mod transport;

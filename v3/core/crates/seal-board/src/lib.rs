//! The Arduino board side: the level-2 command grammar, the write/read transport seam, firmware-level
//! detection, the closed-loop cursor placement and the pure port choice.
//!
//! Pure over a [`transport::Transport`]: the OS serial port is NOT opened here — that adapter
//! (enumeration, `DtrEnable`/`RtsEnable`, the boot wait, driver timeouts, unplug handling) is named
//! in [`transport`] and stays the Windows adapter's job. Every test in this crate is single-threaded
//! over [`transport::FakeTransport`], a fake cursor and a fake clock.
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
pub mod firmware;
pub mod placement;
pub mod port_choice;
pub mod transport;

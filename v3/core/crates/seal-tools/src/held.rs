//! Held inputs: a key or mouse button that is down must not survive the tool.
//!
//! v2 kept this as rules that had to agree: `HeldKeys.NeedsReleaseOnStop` said WHICH tools could
//! leave something down, and every stop path plus two tools' own `finally` blocks called
//! `ReleaseHeld()`. `PROGRESS.md` entry 65 is the list of paths that did not — a drag's left button
//! pressed ABOVE its `try` so the `finally` never ran, and a boarding window left open because the
//! close's own `SleepCheck` threw.
//!
//! Here the release is not a rule to remember: [`HeldInput`] releases everything it pressed when it
//! is dropped, so a normal exit, an early return, a panic and a cancellation all leave the board the
//! same way. The firmware releasing held inputs when the serial port closes (`DESIGN.md` §2.1) is the
//! last resort for a process that dies — it is not the mechanism.
//!
//! Three details are deliberate:
//!
//! - **Releases happen LIFO.** v2's stop path always wrote `U` then `l` because it did not know what
//!   the tool had pressed; the guard does know, and the last gesture let go first is the honest
//!   order (Lead's decision, 2026-10-25).
//! - **One attempt per held input.** `release`/`release_all` drop the record on the FIRST attempt,
//!   successful or not, so the drop can never send a second release for the same input. A failed
//!   write is reported by the explicit call; the firmware is the fallback for the wasted key, not a
//!   retry loop.
//! - **A failed PRESS is still released.** v2's Hold Space `finally` sent the release even when the
//!   press had failed — "never leave the spacebar stuck down" — so the guard records its intent
//!   before writing and releases it on the way out.

use std::fmt;

/// Something the board can hold down. The two v2 has, and the reason `HeldKeys` exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Held {
    /// The spacebar: `P` holds, `U` releases — Hold Space.
    Space,
    /// The left mouse button: `L` holds, `l` releases — the pet feeder's food drag.
    LeftButton,
}

impl Held {
    /// Both inputs, in v2's release order for the service-level `ReleaseHeld` (space, then button).
    /// The guard releases in LIFO order; this list is what a caller iterates when it needs "every
    /// input the suite can hold" without pressing anything first.
    pub const ALL: [Held; 2] = [Self::Space, Self::LeftButton];

    /// The name a log or a report uses. The messages below spell it properly.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Space => "space",
            Self::LeftButton => "left mouse button",
        }
    }

    /// The line the player sees when a press or a hold fails. v2's Hold Space texts, verbatim, for
    /// the spacebar; the same two sentences defined for the button, where v2 only logged (see the
    /// module's report — there was no player-facing line for a failed drag button).
    pub fn press_failure_message(self, error: &BoardError) -> String {
        match self {
            Self::Space => format!("Could not hold the spacebar: {error}"),
            Self::LeftButton => format!("Could not hold the left mouse button: {error}"),
        }
    }

    pub fn release_failure_message(self, error: &BoardError) -> String {
        match self {
            Self::Space => {
                format!("Could not release the spacebar — tap it once in game ({error})")
            }
            Self::LeftButton => {
                format!("Could not release the left mouse button — click once in game ({error})")
            }
        }
    }
}

/// A write to the board failed. The detail is the transport's own message; the player-facing line is
/// [`Held::press_failure_message`] / [`Held::release_failure_message`], which add what to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardError {
    detail: String,
}

impl BoardError {
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for BoardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for BoardError {}

/// The board's press/release surface. `seal-core` implements it over the serial link; the tests
/// implement it over a recording fake.
pub trait BoardInput {
    fn press(&mut self, input: Held) -> Result<(), BoardError>;
    fn release(&mut self, input: Held) -> Result<(), BoardError>;
}

/// A tool's hold on the board: everything pressed through it is released on the way out, on every
/// path, including a panic.
pub struct HeldInput<'board> {
    board: &'board mut dyn BoardInput,
    /// Press order. Releases walk it backwards (LIFO).
    held: Vec<Held>,
}

impl<'board> HeldInput<'board> {
    pub fn new(board: &'board mut dyn BoardInput) -> Self {
        Self {
            board,
            held: Vec::new(),
        }
    }

    pub fn is_held(&self, input: Held) -> bool {
        self.held.contains(&input)
    }

    /// The inputs currently recorded as held, in press order.
    pub fn held(&self) -> &[Held] {
        &self.held
    }

    /// Presses `input` and records it. The record is made BEFORE the write, so a failed press is
    /// still released on the way out (v2's `finally` behaviour) — the release is idempotent in the
    /// firmware, so the worst case is a harmless no-op.
    pub fn press(&mut self, input: Held) -> Result<(), BoardError> {
        if !self.is_held(input) {
            self.held.push(input);
        }
        self.board.press(input)
    }

    /// Releases one input. The record is dropped on the FIRST attempt whether the write succeeded or
    /// not: a second call — including the drop's — is a no-op rather than a second board write.
    pub fn release(&mut self, input: Held) -> Result<(), BoardError> {
        if !self.is_held(input) {
            return Ok(());
        }
        self.held.retain(|held| *held != input);
        self.board.release(input)
    }

    /// Releases everything still held, LIFO. Every input is attempted even if one fails (each write
    /// is independent); the first failure is returned for the caller to report.
    pub fn release_all(&mut self) -> Result<(), BoardError> {
        let mut first_error = None;
        while let Some(input) = self.held.pop() {
            if let Err(error) = self.board.release(input)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for HeldInput<'_> {
    fn drop(&mut self) {
        // The backstop for every exit a caller did not plan. A `Drop` cannot report, so this is
        // best-effort: the explicit `release`/`release_all` are where a failure becomes a message.
        while let Some(input) = self.held.pop() {
            let _ = self.board.release(input);
        }
    }
}

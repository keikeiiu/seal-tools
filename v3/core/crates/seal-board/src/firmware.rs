//! Firmware-level detection: `V` is the only command the sketch writes back, so its silence is an
//! ANSWER, not a failed read.
//!
//! Ported from `v2/SealTools.Core/FirmwareVersion.cs` and `LauncherService.QueryFirmwareVersion`
//! (`LauncherService.cs:323-357`):
//!
//! - The number is a PROTOCOL LEVEL, bumped when the board's BEHAVIOUR changes — not a build counter.
//!   Level 2 is the first load-bearing one: it added `L`/`l` (the two halves of a drag), so a host
//!   that drags on a level-1 board gets silence and the item is never picked up, which looks exactly
//!   like a mis-aimed drag.
//! - **Silence is a definite no.** A board that never answers predates version reporting, so it also
//!   predates the drag commands. It is not "unknown", and it must not be retried into a maybe.
//! - Two attempts, each `V` followed by a 1500 ms read window (`FirmwareReadMs`): a just-flashed
//!   board sits in the sketch's 3 s `setup()` and can miss the first query; the reply waits in the
//!   USB CDC buffer, so the window has to reach past that mark.
//! - A reply that does not parse is not a version (`parse`), and a read failure is treated exactly
//!   like silence because v2's `ReadLineQuietly` catches everything and returns null; the next
//!   write is where a dead port reports itself.
//!
//! The drag DECISION is not restated here: [`Firmware::can_drag`] calls the domain's
//! `food_load::board_can_drag`, which already owns the level rule.

use seal_domain::tools::pet::food_load;

use crate::command::BoardCommand;
use crate::transport::{Deadline, LineRead, Transport};

/// The command that asks the board what it is running (`FirmwareVersion.Query`).
pub const QUERY: &str = "V";

/// The protocol level this host knows (`FirmwareVersion.Current`). The sketch's `FW_VERSION` is
/// pinned equal to it by `tests/sketch.rs`.
pub const CURRENT_LEVEL: u32 = 2;

/// The level that added `L`/`l`, i.e. dragging (`FirmwareVersion.DragLevel`).
pub const DRAG_LEVEL: u32 = 2;

/// How long a single reply window waits (`FirmwareReadMs = 1500`), in SECONDS.
pub const FIRMWARE_READ_SECONDS: f64 = 1.5;

/// How many times `V` is asked (`QueryFirmwareVersion`'s loop).
pub const ATTEMPTS: usize = 2;

/// Reads the level out of a reply line — `"V 1"` gives `1`.
///
/// `None` when the line is not a version reply at all, which is the case that must not be mistaken
/// for a level: exact `V` token, exactly two whitespace-separated tokens, digits only (no sign, no
/// decimal point), and a level above 0 (`FirmwareVersion.Parse`, `FirmwareVersion.cs:34-46`).
#[must_use]
pub fn parse(line: &str) -> Option<u32> {
    if line.trim().is_empty() {
        return None;
    }
    let mut parts = line.split_whitespace();
    let command = parts.next()?;
    let number = parts.next()?;
    if parts.next().is_some() || command != QUERY {
        return None;
    }
    if !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    number.parse::<u32>().ok().filter(|level| *level > 0)
}

/// One detection's outcome: the reported level (or `None` for silence) and how many attempts it took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Firmware {
    reported: Option<u32>,
    attempts: usize,
}

impl Firmware {
    /// A board that never answered: predates version reporting, so it cannot drag.
    #[must_use]
    pub fn silent() -> Self {
        Self {
            reported: None,
            attempts: ATTEMPTS,
        }
    }

    /// A board that reported `level`.
    #[must_use]
    pub fn reporting(level: u32) -> Self {
        Self {
            reported: Some(level),
            attempts: 1,
        }
    }

    #[must_use]
    pub fn reported(self) -> Option<u32> {
        self.reported
    }

    #[must_use]
    pub fn attempts(self) -> usize {
        self.attempts
    }

    /// Whether this board can drag food — the DOMAIN's rule over the reported level
    /// (`food_load::board_can_drag`), so the level-to-feature table has one home.
    ///
    /// A level too large for the domain's `i32` is mapped to "no level": it is a newer board this
    /// build does not understand, and the domain rule treats an unknown level as "cannot drag".
    #[must_use]
    pub fn can_drag(self) -> bool {
        let level = self.reported.and_then(|level| i32::try_from(level).ok());
        food_load::board_can_drag(level)
    }

    /// The log/bug-report sentence (`FirmwareVersion.Describe`), verbatim.
    #[must_use]
    pub fn describe(self) -> String {
        describe(self.reported)
    }

    /// The player-facing sentence (`FirmwareVersion.Summary`), verbatim: what the level MEANS and the
    /// two ways out. The number is not lost — [`Self::describe`] keeps it for a report.
    #[must_use]
    pub fn summary(self) -> String {
        summary(self.reported)
    }
}

/// Asks the board for its level. Two attempts; any unambiguous version reply wins; otherwise silence.
pub fn detect(transport: &mut dyn Transport) -> Firmware {
    let wire = BoardCommand::Version.encode();
    for attempt in 1..=ATTEMPTS {
        if transport.write_line(&wire).is_err() {
            // v2: a write failure returns null at once — there is no board left to ask.
            return Firmware {
                reported: None,
                attempts: attempt,
            };
        }
        if let LineRead::Line(line) =
            transport.read_line(Deadline::after_seconds(FIRMWARE_READ_SECONDS))
            && let Some(level) = parse(&line)
        {
            return Firmware {
                reported: Some(level),
                attempts: attempt,
            };
        }
    }
    Firmware::silent()
}

/// `FirmwareVersion.Describe`: the developer/log sentence, naming the direction of a mismatch.
#[must_use]
pub fn describe(level: Option<u32>) -> String {
    match level {
        None => "did not answer — its firmware predates version reporting".to_owned(),
        Some(level) if level == CURRENT_LEVEL => {
            format!("protocol level {level} (current)")
        }
        Some(level) if level < CURRENT_LEVEL => {
            format!("protocol level {level} — older than this launcher's {CURRENT_LEVEL}")
        }
        Some(level) => {
            format!("protocol level {level} — newer than this launcher's {CURRENT_LEVEL}")
        }
    }
}

/// `FirmwareVersion.Summary`: the card's sentence — what it means and what to do about it.
///
/// A player cannot judge "protocol level 2", and the only thing the level decides today is whether
/// the board can drag food, so every case says that and gives a way out.
#[must_use]
pub fn summary(level: Option<u32>) -> String {
    match level {
        None => "Too old to report a version, so it also predates the drag commands — a food drag would silently do nothing. Reflash the board, or set the load mode back to \"right_click\" on the Pet tab."
            .to_owned(),
        Some(level) if level < DRAG_LEVEL => "Older than this launcher's — it cannot drag food, and a drag would silently do nothing. Reflash the board, or set the load mode back to \"right_click\" on the Pet tab."
            .to_owned(),
        Some(level) if level > CURRENT_LEVEL => "Newer than this launcher knows — it should work, but \
                                                 anything added since is not something this launcher \
                                                 can tell you about."
            .to_owned(),
        Some(_) => "Up to date — it supports everything this launcher sends, dragging food \
                    included."
            .to_owned(),
    }
}

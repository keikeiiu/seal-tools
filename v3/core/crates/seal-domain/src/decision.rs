//! `Decision` — the vocabulary a tool's pure decision half returns and its loop executes.
//!
//! `PLAN-V3-CORE.md` §6.4: "each tool's decision half becomes a pure function returning a `Decision`
//! (`Click`, `Key`, `Wait`, `Stop(Reason)`) that carries the message to show; the loop only performs
//! effects." That split is the reason the tools can be tested at all: v2's nine-defects entry in
//! `PROGRESS.md` (63) is a list of decisions made somewhere no test could reach.
//!
//! Nothing here performs I/O, and a `Decision` is data — the tool loop is what clicks, presses and
//! sleeps. `Wait` is ported from `ToolBase.SleepCheck`, which sleeps in chunks so a stop abandons the
//! wait DURING it rather than after it.

use crate::geometry::Point;
use serde::{Deserialize, Serialize};

/// What a tool's decision half wants done next.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Place the cursor at this client-relative PHYSICAL offset and left-click there. The offset is
    /// in the same space as every calibrated coordinate, so it is directly comparable with the
    /// config's points and needs no rescaling (the core is DPI-unaware on purpose).
    Click(Point),
    /// Press one key. See [`Key`] for what "one key" means here.
    Key(Key),

    /// Press one key as the FAST tap — the firmware's lowercase `k`/`f`, which the spammer's `*key`
    /// spelling asks for. A separate decision rather than a flag on [`Self::Key`] because the two
    /// are different wires and a tool that does not model fast taps cannot accidentally ask for one.
    KeyFast(Key),

    /// Press and HOLD the spacebar (the board's `P`). Only the Hold Space tool asks for this, and it
    /// must be paired with [`Self::ReleaseSpace`] on every exit path — the runner's effects object
    /// owns that guard, not this value.
    HoldSpace,

    /// Release the spacebar (the board's `U`). Harmless when nothing is held: the release path never
    /// has to ask whether it needs to.
    ReleaseSpace,

    /// Press Enter (the board's `E`). A separate decision because `Key` deliberately means one
    /// printable character or F1-F12, and the firmware's Enter is its own command: the tuner's spring
    /// sequence is exactly "click, then Enter", and the quest executor's `enter` action is the same
    /// pair. Sent as `E`, never as `K E`.
    Enter,

    /// Move the cursor to this client-relative PHYSICAL offset and DO NOT click. The tuner's HID
    /// spring placement is the reason it exists: v2's `PlaceOnSpring` puts the cursor on the spring
    /// button and the loop clicks separately (an extra click here would open the wrong window), and
    /// its recenter path is the same move with no click.
    Place(Point),

    /// Click wherever the cursor already is (the board's bare `C`). v2's tuner in `manual` spring
    /// mode clicks the button the PLAYER positioned the mouse on, so there is no point to place at —
    /// and `Click(Point)` would move the cursor first, which manual mode exists to avoid.
    ClickHere,

    /// Right-click wherever the cursor already is (the board's bare `R`). The quest executor's
    /// `right_click` action is the only caller today; like [`Self::ClickHere`] it never moves the
    /// cursor, because the player positioned the mouse during the run's countdown.
    RightClickHere,
    /// Sleep this many seconds — or, without a game to drive, pass that much time before the next
    /// decision. The loop implements it as `ToolBase.SleepCheck`: in chunks, watching the stop signal.
    ///
    /// Not validated: v2's `SleepCheck` clamps a non-positive or nonsense value to one tick
    /// (`Math.Max(1, …)`) rather than failing, and a wait of 0 is a legitimate "no pause".
    Wait(f64),
    /// End the run, carrying the message the player sees (`PROTOCOL.md` §5's `tool.finished.message`
    /// is this exact string).
    Stop(StopReason),
}

impl Decision {
    pub const fn click(point: Point) -> Self {
        Self::Click(point)
    }

    /// A one-character key press, or [`KeyError`] when the character cannot be sent.
    pub fn key(character: char) -> Result<Self, KeyError> {
        Key::character(character).map(Self::Key)
    }

    pub const fn wait(seconds: f64) -> Self {
        Self::Wait(seconds)
    }

    pub const fn stop(reason: StopReason) -> Self {
        Self::Stop(reason)
    }

    /// True when this decision ends the run; the loop may then skip its next read entirely.
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Stop(_))
    }

    /// The stop reason, when this decision is a [`Decision::Stop`].
    pub const fn stop_reason(&self) -> Option<&StopReason> {
        match self {
            Self::Stop(reason) => Some(reason),
            _ => None,
        }
    }
}

/// One key press the board can send.
///
/// The firmware's `K` handler presses ONE printable ASCII character and its `F` handler one function
/// key from a table that ends at F12 (`SkillSpammer.SendKey`: "the firmware's table goes to twelve.
/// ParseVk accepts F1–F24 elsewhere, so anything above twelve is refused here rather than sent and
/// silently ignored on board"). Hotkey names and board keys are therefore two questions with
/// different answers, and this type answers the board's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// One printable ASCII character, `0x20..=0x7E`.
    Char(char),
    /// A function key, 1 through 12.
    Function(u8),
}

impl Key {
    /// A printable ASCII character, or [`KeyError`] with the message a player needs.
    pub fn character(character: char) -> Result<Self, KeyError> {
        if (' '..='~').contains(&character) {
            Ok(Self::Char(character))
        } else {
            Err(KeyError::Unsendable {
                input: character.to_string(),
            })
        }
    }

    /// A function key, 1 through 12 (the firmware's table), or [`KeyError`].
    pub fn function(number: u8) -> Result<Self, KeyError> {
        if (1..=12).contains(&number) {
            Ok(Self::Function(number))
        } else {
            Err(KeyError::Unsendable {
                input: format!("F{number}"),
            })
        }
    }

    /// A key as a config file or a preset spells it: `"5"`, `"q"`, `"F1"` … `"F12"`.
    ///
    /// Case-sensitive on the `F` prefix, exactly like `SkillSpammer.SendKey` — a lowercase `f1` is
    /// not read as a function key there, and inventing that here would let a run send a key the v2
    /// build refused. The number is trimmed and parsed the way C# `int.TryParse` parses it, so a
    /// padded `F01` still means F1 rather than being refused for a spelling the board accepted.
    pub fn parse(text: &str) -> Result<Self, KeyError> {
        if let Some(number) = text.strip_prefix('F') {
            if let Ok(number) = number.trim().parse::<u8>() {
                return Self::function(number);
            }
            return Err(KeyError::Unsendable {
                input: text.to_owned(),
            });
        }

        let mut characters = text.chars();
        match (characters.next(), characters.next()) {
            (Some(character), None) => Self::character(character),
            _ => Err(KeyError::Unsendable {
                input: text.to_owned(),
            }),
        }
    }
}

/// A key that cannot be sent as one board command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    /// Neither one printable ASCII character nor F1–F12. Refused HERE rather than sent and dropped
    /// on the board, where a silent no-op in a loop that presses 500 times is invisible.
    Unsendable { input: String },
}

impl KeyError {
    /// The line to show the player. v2 refuses these too (`SkillSpammer.WarnUnsupported`,
    /// `QuestAction.Complaint`), so this is a port rather than a new rule.
    pub fn message(&self) -> String {
        match self {
            Self::Unsendable { input } if input.is_empty() => {
                "A key step with no character would send nothing at all — use one printable ASCII \
                 character, or F1–12."
                    .to_owned()
            }
            Self::Unsendable { input } => format!(
                "\"{input}\" cannot be sent as one key — use one printable ASCII character, or F1–12."
            ),
        }
    }
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for KeyError {}

/// Why a run ended, and the line the player reads about it.
///
/// [`StopKind`] is the protocol's `tool.finished.reason` (`PROTOCOL.md` §5), so the core can put
/// these values into that event without a second translation table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopReason {
    pub kind: StopKind,
    /// The message v2 writes to its log ("DONE — 4 row(s)", "OCR failed: …"). Shown to the player,
    /// so it must be a sentence rather than a debug dump.
    pub message: String,
}

impl StopReason {
    pub fn new(kind: StopKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// The work reached its end normally.
    pub fn done(message: impl Into<String>) -> Self {
        Self::new(StopKind::Done, message)
    }

    /// The player (or the shell) stopped the run. The cleanup still happens — "a stop abandons the
    /// WORK, not the CLEANUP" (`ToolBase.WithoutCancellation`).
    pub fn cancelled(message: impl Into<String>) -> Self {
        Self::new(StopKind::Cancelled, message)
    }

    /// The run failed. The message is the failure, not a panic dump.
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(StopKind::Error, message)
    }

    /// A guard tripped — the cursor moved, the game lost the foreground, the board went away.
    pub fn guard(message: impl Into<String>) -> Self {
        Self::new(StopKind::Guard, message)
    }
}

/// The protocol's four reasons a run can end (`PROTOCOL.md` §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StopKind {
    Done,
    Cancelled,
    Error,
    Guard,
}

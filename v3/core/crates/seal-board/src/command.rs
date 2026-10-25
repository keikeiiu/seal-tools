//! The board command grammar: the v2 host's half of `arduino/seal_mouse/seal_mouse.ino`'s serial
//! protocol, encoded to the exact wire text the sketch reads.
//!
//! One command per line, '\n'-terminated, first character selects the command
//! (`v2/docs/spec/DESIGN.md` §2.1). The encoding is deliberately not "approximately" the wire: the
//! sketch parses with `buf[0]` and `sscanf`, so the byte-for-byte tests in `tests/command.rs` are the
//! contract.
//!
//! `D dx dy` counts are **RAW HID counts**, measured per machine and never scaled
//! (`DESIGN.md` §2.1, `MOVE-SETS.md`). [`BoardCommand::move_by`] takes the counts as they are: no
//! DPI scaling, no rounding, no clamping to a "safe" range — the caller measured them, and the
//! closed-loop placement (`placement`) computes its own corrections.
//!
//! Validation lives here for the arguments the firmware would silently ignore: a bad `K`/`F`/`Q`/`Z`
//! frame is dropped by the sketch ("nothing is guessed at"), so the host refuses it with a sentence
//! instead of sending a command that does nothing.

/// A host-to-board command. The variants are exactly what v2 sends: `C`/`R` clicks, `D` move, `E`
/// Enter, `K`/`F` keys, `Q`/`Z` wheel, `L`/`l` hold/release the left button, and `V` the version
/// query. The sketch also understands `H`/`T`/`S`/`X`/`W`/`P`/`U`; v2's host never sends them and the
/// grammar does not grow a variant it cannot use (the sketch guard test pins the sketch's whole set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardCommand {
    /// A left click: press, hold 50–150 ms, release — all inside the firmware.
    LeftClick,
    /// A right click, same shape.
    RightClick,
    /// A relative move by RAW HID counts.
    Move { dx: i32, dy: i32 },
    /// Enter.
    Enter,
    /// One printable ASCII character, normal (`K`) or fast (`k`) hold.
    Key { character: char, fast: bool },
    /// F1–F12, normal (`F`) or fast (`f`) hold.
    FunctionKey { number: u8, fast: bool },
    /// One wheel notch is `Q 1` (up) / `Z 1` (down); n notches is n separate notches in the firmware.
    Wheel { up: bool, notches: u16 },
    /// Press and hold the left button — the first half of a drag. Needs firmware level 2.
    HoldLeft,
    /// Release the left button — the second half of a drag. Needs firmware level 2.
    ReleaseLeft,
    /// Ask the board for its protocol level (`V`). The only command that gets a reply.
    Version,
}

/// Refused before sending, because the firmware would silently ignore the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    message: String,
}

impl CommandError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

/// The firmware's wheel ceiling (`WHEEL_MAX_NOTCHES`, `seal_mouse.ino`) — not how far a tool may
/// scroll, just the largest single frame the sketch accepts. The sketch clamps above it; the host
/// refuses rather than sending a frame the board would quietly truncate.
pub const WHEEL_MAX_NOTCHES: u16 = 400;

/// The firmware's F-key table ends at 12 (`seal_mouse.ino` `fkeys[]`: index 1–12).
pub const FUNCTION_KEY_MAX: u8 = 12;

impl BoardCommand {
    /// A relative move by raw HID counts. The counts are passed through untouched.
    pub const fn move_by(dx: i32, dy: i32) -> Self {
        Self::Move { dx, dy }
    }

    /// One printable ASCII character press. The sketch accepts `0x20..=0x7E`; anything else is a
    /// frame it silently ignores, so the host refuses it here.
    pub fn key(character: char, fast: bool) -> Result<Self, CommandError> {
        if !matches!(character, ' '..='~') {
            return Err(CommandError::new(format!(
                "{character:?} cannot be sent as one key — the board sends one printable ASCII \
                 character, and anything else would be silently dropped."
            )));
        }
        Ok(Self::Key { character, fast })
    }

    /// F1–F12, normal or fast hold.
    pub fn function_key(number: u8, fast: bool) -> Result<Self, CommandError> {
        if number == 0 || number > FUNCTION_KEY_MAX {
            return Err(CommandError::new(format!(
                "F{number} is not a function key — the board sends F1–F12."
            )));
        }
        Ok(Self::FunctionKey { number, fast })
    }

    /// Wheel notches, up or down. 1–400 per frame (`WHEEL_MAX_NOTCHES`).
    pub fn wheel(up: bool, notches: u16) -> Result<Self, CommandError> {
        if notches == 0 || notches > WHEEL_MAX_NOTCHES {
            return Err(CommandError::new(format!(
                "{notches} wheel notch(es) is out of range — the board sends 1–{WHEEL_MAX_NOTCHES}."
            )));
        }
        Ok(Self::Wheel { up, notches })
    }

    /// The exact wire text, including the terminating `\n` the sketch reads until.
    ///
    /// The transport writes these bytes verbatim (`transport`): the grammar owns the line, so a
    /// `\r` cannot sneak in from a text-mode serial layer.
    pub fn encode(&self) -> String {
        match *self {
            Self::LeftClick => "C\n".to_owned(),
            Self::RightClick => "R\n".to_owned(),
            Self::Move { dx, dy } => format!("D {dx} {dy}\n"),
            Self::Enter => "E\n".to_owned(),
            Self::Key { character, fast } => {
                format!("{} {character}\n", if fast { 'k' } else { 'K' })
            }
            Self::FunctionKey { number, fast } => {
                format!("{} {number}\n", if fast { 'f' } else { 'F' })
            }
            Self::Wheel { up, notches } => {
                format!("{} {notches}\n", if up { 'Q' } else { 'Z' })
            }
            Self::HoldLeft => "L\n".to_owned(),
            Self::ReleaseLeft => "l\n".to_owned(),
            Self::Version => "V\n".to_owned(),
        }
    }
}

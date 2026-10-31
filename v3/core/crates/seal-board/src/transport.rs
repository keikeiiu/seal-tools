//! The transport seam: write one wire line, read one reply line, with a deadline in SECONDS.
//!
//! v2 passed `I/O` IN rather than doing it in the decision code (`PortChoice.cs:15-17` gives the
//! reason for the port half: "the decision is the part that has to be right and the part that can be
//! checked without a board"). The same split applies here: this module is the seam, and the Windows
//! ADAPTER is [`crate::serial`] (and [`crate::enumerate`] for the port list).
//!
//! What stays the adapter's live-only list (named, not faked — `tests` prove the fake only):
//!
//! - **Port enumeration**: WMI `Win32_SerialPort` VID/PID match plus the "name mentions Arduino"
//!   fallback (`Arduino.cs:25-76`). Proving it needs the real driver stack of a machine with and
//!   without the board.
//! - **Opening**: `DtrEnable`/`RtsEnable` on the `ATmega32U4` and the **2 s boot wait**
//!   (`Arduino.cs:131-145`, `LauncherService.cs:255`), plus the sketch's own 3 s `setup()` delay.
//!   Proving it needs a cold-plugged, just-flashed board — the reason the firmware query retries past
//!   that window (`firmware`).
//! - **Driver timeouts**: `ReadTimeout`/`WriteTimeout` behaviour and what a timeout exception looks
//!   like — the adapter maps them to [`LineRead::Silent`]/[`LineRead::Failed`]; measuring them needs
//!   the driver, and the 1500 ms firmware window is passed IN as a [`Deadline`] so the number is the
//!   caller's, not the driver's.
//! - **Port sharing with the game's anti-cheat** and surviving a device unplugged mid-run.
//!
//! The fake lives in `fake`: it records writes verbatim and scripts reads, so every test in this
//! crate (and in consumers) is single-threaded and pays no serial time.

use std::collections::VecDeque;

/// How long a read may wait, in SECONDS.
///
/// Named and constructed in seconds on purpose: `DESIGN.md` §2.1 records the eleven-minute sleep that
/// came from handing a millisecond value to a seconds API. There is no millisecond constructor to
/// confuse this with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Deadline {
    seconds: f64,
}

impl Deadline {
    /// A deadline `seconds` from now. Seconds — see the type's doc and `DESIGN.md` §2.1.
    #[must_use]
    pub fn after_seconds(seconds: f64) -> Self {
        Self { seconds }
    }

    #[must_use]
    pub fn seconds(self) -> f64 {
        self.seconds
    }
}

/// What a read found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineRead {
    /// A whole line, terminator stripped.
    Line(String),
    /// Nothing arrived before the deadline. For `V` this is an ANSWER (`firmware`).
    Silent,
    /// The adapter failed (the port went away, a read error). v2 treats this the same as silence —
    /// `ReadLineQuietly` catches everything and returns null (`LauncherService.cs:359-374`) — and so
    /// does the firmware query, because the next write reports the real failure.
    Failed { message: String },
}

/// The serial port, as this crate needs it: one line out, one line in.
pub trait Transport {
    /// Writes `wire` verbatim — the exact text [`crate::command::BoardCommand::encode`] produced,
    /// including its terminating `\n`. The transport must not add, strip or translate terminators:
    /// the grammar owns the wire text, so a `\r` cannot be introduced by a text-mode layer.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the write or its flush fails; the message is the line the
    /// caller shows instead of pretending the command reached the board.
    fn write_line(&mut self, wire: &str) -> Result<(), TransportError>;

    /// Reads one line, waiting at most `deadline`.
    fn read_line(&mut self, deadline: Deadline) -> LineRead;
}

/// A write was refused. The message is the line the caller shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportError {
    message: String,
}

impl TransportError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TransportError {}

/// A recording transport for tests (and for testing a consumer without a board).
///
/// Not wired into the binary: the production adapter is the Windows serial port, which is not in this
/// crate (see the module docs). Reads come from a script; when it is exhausted the board is SILENT,
/// which is exactly what an old board looks like.
#[derive(Debug, Default)]
pub struct FakeTransport {
    writes: Vec<String>,
    reads: VecDeque<LineRead>,
    deadlines: Vec<f64>,
    fail_writes: Option<String>,
}

impl FakeTransport {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A fake whose reads are the given script.
    #[must_use]
    pub fn with_reads(reads: Vec<LineRead>) -> Self {
        Self {
            reads: reads.into(),
            ..Self::default()
        }
    }

    /// Every wire line written, in order and verbatim.
    #[must_use]
    pub fn writes(&self) -> Vec<String> {
        self.writes.clone()
    }

    /// The deadlines each read was asked to wait, in seconds, in order — so a test can pin the
    /// firmware window without a board.
    #[must_use]
    pub fn deadlines(&self) -> Vec<f64> {
        self.deadlines.clone()
    }

    /// Makes every write fail with `message`.
    pub fn fail_writes(&mut self, message: impl Into<String>) {
        self.fail_writes = Some(message.into());
    }

    /// Appends a scripted read (for a test that arranges reads after construction).
    pub fn push_read(&mut self, read: LineRead) {
        self.reads.push_back(read);
    }
}

impl Transport for FakeTransport {
    fn write_line(&mut self, wire: &str) -> Result<(), TransportError> {
        if let Some(message) = &self.fail_writes {
            return Err(TransportError::new(message.clone()));
        }
        self.writes.push(wire.to_owned());
        Ok(())
    }

    fn read_line(&mut self, deadline: Deadline) -> LineRead {
        self.deadlines.push(deadline.seconds());
        self.reads.pop_front().unwrap_or(LineRead::Silent)
    }
}

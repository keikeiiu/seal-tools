//! The Windows serial adapter: `serialport` behind the crate's [`Transport`] seam.
//!
//! The seam (`crate::transport`) is the contract; this is the one implementation that touches a real
//! device. v2's open sequence is kept exactly (`LauncherService.ArduinoPortAsync` /
//! `Arduino.Open`):
//!
//! 1. open at the configured baud with the `ATmega32U4`'s required `DTR` asserted
//!    (`Arduino.cs:131-145`: the 32U4 handles USB CDC on the main chip and needs DTR/RTS to keep the
//!    connection alive — unlike the Uno, it does not reset on DTR);
//! 2. assert `RTS` as v2 did (`RtsEnable = true`) and purge both buffers;
//! 3. **ask `V` until the board answers**, through the [`Clock`] seam, instead of v2's flat 2 s boot
//!    wait (`LauncherService.cs:255`): a warm board opens in milliseconds, and a cold one is still
//!    covered because the sketch's own `setup()` costs ~3 s and the firmware probe retries
//!    (`crate::firmware`, HW-F8).
//!
//! The write path writes the EXACT wire line ([`crate::command::BoardCommand::encode`] already ends
//! it with `\n`) and flushes; a reader gets one line at a time, assembled across driver reads, with a
//! [`Deadline`] in SECONDS bounding the WHOLE read (the remaining window is set per driver call). A
//! driver timeout is [`LineRead::Silent`] (for `V` that is the ANSWER: an old board stays silent) and
//! an I/O error / closed port is [`LineRead::Failed`] — the adapter never panics.
//!
//! # What needs the real machine (live-only, never faked here)
//!
//! - **Cold-plug / just-flashed boot window.** The sketch's `setup()` takes ~3 s; readiness polling
//!   and the firmware probe's two 1.5 s `V` attempts are sized for it. Proven by
//!   `tests/live_only.rs::a_just_flashed_board_answers_the_version_probe` — a plugged-in board (or a
//!   re-plugged one) answering with a level.
//! - **Unplug mid-run.** Our mapping of the driver error to `LineRead::Failed` is fake-testable; the
//!   firmware's host-gone failsafe (releasing a held spacebar/left button when `Serial` goes false)
//!   is the board's half. Proven by `an_unplug_mid_run_is_a_typed_failure` plus watching the held
//!   input release.
//! - **Anti-cheat port sharing.** Whether the game's anti-cheat lets the port open while the game
//!   runs is a live-session fact. Proven by `a_click_reaches_the_board_while_the_game_is_running`.
//! - **Driver timeout values.** The `Deadline`→`set_timeout` mapping is fake-testable, but the real
//!   driver decides how closely a timeout tracks the request. Proven by
//!   `a_silent_read_returns_after_roughly_the_deadline`.

use std::io;
use std::time::{Duration, Instant};

use crate::clock::Clock;
use crate::transport::{Deadline, LineRead, Transport, TransportError};

/// How long the board is given to answer its first `V`, in SECONDS. v2 waited a flat 2 s for the
/// boot window (`LauncherService.cs:255`), and the 32U4 does not reset on DTR, so every open paid it
/// even though it only matters on a cold plug. Readiness polling replaces it (HW-F8): a warm board
/// answers at once, and a cold one is covered by the sketch's ~3 s `setup()` plus the firmware
/// probe's own two read windows.
pub const READINESS_TIMEOUT_SECONDS: f64 = 3.0;

/// One readiness window: `V` is written and a reply waited for this long (HW-F8).
pub const READINESS_POLL_SECONDS: f64 = 0.15;

/// The pause between two readiness polls, so a silent board is asked about four times a second
/// rather than hammered back to back (HW-F8).
pub const READINESS_GAP_SECONDS: f64 = 0.1;

/// Polls before the board is called silent. 12 × (150 ms window + 100 ms gap) stays inside
/// [`READINESS_TIMEOUT_SECONDS`].
pub const READINESS_ATTEMPTS: u32 = 12;

/// v2's `SerialPort.ReadTimeout = 1000` (`Arduino.cs:135`): the initial window before a read is told
/// to set its own per the [`Deadline`].
const INITIAL_READ_SECONDS: f64 = 1.0;

/// How long one wire line may take to reach the driver, in SECONDS. `serialport`'s
/// `set_timeout` writes BOTH `ReadTotalTimeoutConstant` and `WriteTotalTimeoutConstant`
/// (windows/com.rs `set_timeouts`), so without this the write deadline would be whatever the last
/// read happened to ask for. A board that is busy walking a long frame can make `WriteFile` block;
/// this makes that a reported failure instead of a wedged worker (HW-F10).
pub const WRITE_TIMEOUT_SECONDS: f64 = 1.0;

/// The device the transport talks to. `serialport` implements it for real; tests implement it with a
/// script. Keeping this a trait is the failure seam: a failing writer is injected here, so the
/// adapter's error path is tested without a board (and without a driver that can be unplugged).
pub trait SerialDevice: Send {
    /// Writes every byte of `bytes`.
    ///
    /// # Errors
    ///
    /// Returns the driver's I/O error when the port is gone or the device refuses the write.
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()>;

    /// Flushes the driver's write buffer.
    ///
    /// # Errors
    ///
    /// Returns the driver's I/O error when the port is gone or the flush fails.
    fn flush(&mut self) -> io::Result<()>;

    /// Sets the timeout for subsequent reads.
    ///
    /// # Errors
    ///
    /// Returns the driver's I/O error when the port is gone or the driver refuses the timeout.
    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()>;

    /// Reads up to `buffer.len()` bytes. A driver timeout is an `io::Error` of kind
    /// `TimedOut`/`WouldBlock`; `Ok(0)` means the port closed.
    ///
    /// # Errors
    ///
    /// Returns the driver's I/O error for a real failure (a closed port, a device error); a read
    /// timeout is reported as one of the silence kinds above rather than as a failure.
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize>;
}

/// The production transport: one open serial port and the bytes not yet a full line.
pub struct SerialTransport {
    device: Box<dyn SerialDevice>,
    buffer: Vec<u8>,
}

impl SerialTransport {
    /// Builds a transport over an injected device (the test seam; production uses [`Self::open`]).
    #[must_use]
    pub fn with_device(device: Box<dyn SerialDevice>) -> Self {
        Self {
            device,
            buffer: Vec::new(),
        }
    }

    /// Opens the port and waits for it to answer, through `clock`.
    ///
    /// A failure is a [`TransportError`] naming the port; the caller shows that line rather than
    /// pretending a board is there. A board that opens but never answers is NOT a failure: it is an
    /// older firmware, which the version probe reports as silence.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] naming the port when the driver refuses to open it, to assert RTS,
    /// or to clear its buffers.
    #[cfg(windows)]
    pub fn open(port_name: &str, baud: u32, clock: &dyn Clock) -> Result<Self, TransportError> {
        let opened = serialport::new(port_name, baud)
            .timeout(Duration::from_secs_f64(INITIAL_READ_SECONDS))
            .dtr_on_open(true)
            .open()
            .map_err(|error| {
                TransportError::new(format!(
                    "cannot open the serial port {port_name} at {baud} baud: {error}"
                ))
            })?;
        let mut device = OsDevice { port: opened };
        device.port.write_request_to_send(true).map_err(|error| {
            TransportError::new(format!(
                "cannot assert RTS on the serial port {port_name}: {error}"
            ))
        })?;
        // Purge both directions before the board is asked anything: a previous session's reply can
        // still sit in the input buffer, and a half-sent line in the output buffer would be flushed
        // into the new conversation (HW-F10).
        device
            .port
            .clear(serialport::ClearBuffer::All)
            .map_err(|error| {
                TransportError::new(format!(
                    "cannot clear the serial buffers on {port_name}: {error}"
                ))
            })?;
        let mut transport = Self::with_device(Box::new(device));
        // Readiness polling replaces v2's flat 2 s wait: the board is asked `V` until it answers, so
        // a warm board opens in milliseconds instead of paying the cold-plug budget every time
        // (HW-F8). A silent answer is not an error — an old board stays silent by design.
        transport.wait_until_ready(clock);
        Ok(transport)
    }

    /// The named not-built error for a non-Windows build (the dependency is Windows-only).
    #[cfg(not(windows))]
    pub fn open(_port_name: &str, _baud: u32, _clock: &dyn Clock) -> Result<Self, TransportError> {
        Err(TransportError::new(
            "the serial adapter is only built on Windows — `serialport` is a Windows-only \
             dependency of seal-board",
        ))
    }

    /// Ask `V` until the board answers, up to [`READINESS_ATTEMPTS`] polls through `clock`.
    ///
    /// `true` means a version reply arrived; `false` means the board stayed silent (an old firmware,
    /// which is a valid board and not an error) or the port failed. This is v2's boot wait made
    /// adaptive: the first command no longer pays two seconds on a board that is already up (HW-F8).
    pub fn wait_until_ready(&mut self, clock: &dyn Clock) -> bool {
        for attempt in 0..READINESS_ATTEMPTS {
            if self
                .write_line(&format!("{}\n", crate::firmware::QUERY))
                .is_err()
            {
                return false;
            }
            match self.read_line(Deadline::after_seconds(READINESS_POLL_SECONDS)) {
                LineRead::Line(line) if crate::firmware::parse(&line).is_some() => return true,
                // A line that is not a version reply is not readiness; keep asking.
                LineRead::Line(_) | LineRead::Silent => {}
                LineRead::Failed { .. } => return false,
            }
            if attempt + 1 < READINESS_ATTEMPTS {
                clock.sleep_seconds(READINESS_GAP_SECONDS);
            }
        }
        false
    }
}

impl Transport for SerialTransport {
    fn write_line(&mut self, wire: &str) -> Result<(), TransportError> {
        // The grammar's wire text already ends in '\n'; a bare line gets the one newline the sketch
        // needs. Exactly one, either way.
        let mut bytes = Vec::with_capacity(wire.len() + 1);
        bytes.extend_from_slice(wire.as_bytes());
        if !wire.ends_with('\n') {
            bytes.push(b'\n');
        }
        // `serialport`'s timeout drives the WRITE deadline too, so it is set explicitly rather than
        // inherited from the last read. A blocked `WriteFile` is what wedged the worker for good.
        self.device
            .set_timeout(Duration::from_secs_f64(WRITE_TIMEOUT_SECONDS))
            .map_err(|error| {
                TransportError::new(format!("cannot set the serial write timeout: {error}"))
            })?;
        self.device.write_all(&bytes).map_err(|error| {
            if is_silence(&error) {
                TransportError::new(format!(
                    "the board did not accept the line within {WRITE_TIMEOUT_SECONDS} s — it is \
                     busy or not responding: {error}"
                ))
            } else {
                TransportError::new(format!("the serial write failed: {error}"))
            }
        })?;
        // A line is not sent until it is flushed. After a failed write there is deliberately NO
        // flush: nothing was written, so nothing is confirmed.
        self.device
            .flush()
            .map_err(|error| TransportError::new(format!("the serial flush failed: {error}")))
    }

    fn read_line(&mut self, deadline: Deadline) -> LineRead {
        let total = Duration::try_from_secs_f64(deadline.seconds().max(0.0))
            .unwrap_or(Duration::from_secs_f64(INITIAL_READ_SECONDS));
        let started = Instant::now();

        loop {
            if let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') {
                let mut line: Vec<u8> = self.buffer.drain(..=index).collect();
                line.pop(); // the '\n'
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return LineRead::Line(String::from_utf8_lossy(&line).into_owned());
            }

            // The deadline bounds the WHOLE read, not each chunk: a reply that trickles in must not
            // stretch the wait past the window the caller asked for (HW-F10). A driver timeout below
            // means the remaining window was spent, so it is silence for this call.
            let remaining = total.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return LineRead::Silent;
            }
            if let Err(error) = self.device.set_timeout(remaining) {
                return LineRead::Failed {
                    message: format!("cannot set the serial read timeout: {error}"),
                };
            }

            let mut chunk = [0u8; 256];
            match self.device.read(&mut chunk) {
                // A serial port does not return 0 bytes for a timeout (that is an error kind); 0 is
                // the port closing, which must not look like a silent board.
                Ok(0) => {
                    self.buffer.clear();
                    return LineRead::Failed {
                        message: "the serial port closed while reading".to_owned(),
                    };
                }
                Ok(count) => self.buffer.extend_from_slice(&chunk[..count]),
                Err(error) if is_silence(&error) => return LineRead::Silent,
                Err(error) => {
                    self.buffer.clear();
                    return LineRead::Failed {
                        message: format!("the serial read failed: {error}"),
                    };
                }
            }
        }
    }
}

/// A driver timeout is silence; anything else is a failure. `Interrupted` is a retry, which for one
/// blocking read is silence for this call (the caller's deadline is the driver's business).
fn is_silence(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}

/// The real device: a `serialport` handle adapted to [`SerialDevice`].
#[cfg(windows)]
struct OsDevice {
    port: Box<dyn serialport::SerialPort>,
}

#[cfg(windows)]
impl SerialDevice for OsDevice {
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        io::Write::write_all(&mut self.port, bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut self.port)
    }

    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        serialport::SerialPort::set_timeout(&mut *self.port, timeout).map_err(io::Error::other)
    }

    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        io::Read::read(&mut self.port, buffer)
    }
}

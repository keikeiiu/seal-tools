//! The Windows serial adapter: `serialport` behind the crate's [`Transport`] seam.
//!
//! The seam (`crate::transport`) is the contract; this is the one implementation that touches a real
//! device. v2's open sequence is kept exactly (`LauncherService.ArduinoPortAsync` /
//! `Arduino.Open`):
//!
//! 1. open at the configured baud with the `ATmega32U4`'s required `DTR` asserted
//!    (`Arduino.cs:131-145`: the 32U4 handles USB CDC on the main chip and needs DTR/RTS to keep the
//!    connection alive — unlike the Uno, it does not reset on DTR);
//! 2. assert `RTS` as v2 did (`RtsEnable = true`);
//! 3. **wait ~2 s for the boot window** (`LauncherService.cs:255`), through the [`Clock`] seam so the
//!    wait is testable without sleeping 2 s in the suite. The sketch's own `setup()` costs another
//!    3 s, which is why the firmware probe's two 1.5 s attempts exist (`crate::firmware`).
//!
//! The write path writes the EXACT wire line ([`crate::command::BoardCommand::encode`] already ends
//! it with `\n`) and flushes; a reader gets one line at a time, assembled across driver reads, with a
//! [`Deadline`] in SECONDS set on the driver per call. A driver timeout is [`LineRead::Silent`] (for
//! `V` that is the ANSWER: an old board stays silent) and an I/O error / closed port is
//! [`LineRead::Failed`] — the adapter never panics.
//!
//! # What needs the real machine (live-only, never faked here)
//!
//! - **Cold-plug / just-flashed boot window.** DTR resets the 32U4 and the sketch's `setup()` takes
//!   3 s; our 2 s wait + two 1.5 s `V` attempts are sized for that. Proven by
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
use std::time::Duration;

use crate::clock::Clock;
use crate::transport::{Deadline, LineRead, Transport, TransportError};

/// v2's post-open boot wait (`LauncherService.cs:255`, `await Task.Delay(2000)`), in SECONDS.
pub const BOOT_WAIT_SECONDS: f64 = 2.0;

/// v2's `SerialPort.ReadTimeout = 1000` (`Arduino.cs:135`): the initial window before a read is told
/// to set its own per the [`Deadline`].
const INITIAL_READ_SECONDS: f64 = 1.0;

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

    /// Opens the port exactly as v2 did and pays the boot wait through `clock`.
    ///
    /// A failure is a [`TransportError`] naming the port; the caller shows that line rather than
    /// pretending a board is there.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] naming the port when the driver refuses to open it or to assert
    /// RTS. The boot wait itself cannot fail; it only sleeps through `clock`.
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
        boot_wait(clock);
        Ok(Self::with_device(Box::new(device)))
    }

    /// The named not-built error for a non-Windows build (the dependency is Windows-only).
    #[cfg(not(windows))]
    pub fn open(_port_name: &str, _baud: u32, _clock: &dyn Clock) -> Result<Self, TransportError> {
        Err(TransportError::new(
            "the serial adapter is only built on Windows — `serialport` is a Windows-only \
             dependency of seal-board",
        ))
    }
}

/// v2's boot wait, on its own so the suite can prove its unit without sleeping.
pub fn boot_wait(clock: &dyn Clock) {
    clock.sleep_seconds(BOOT_WAIT_SECONDS);
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
        self.device
            .write_all(&bytes)
            .map_err(|error| TransportError::new(format!("the serial write failed: {error}")))?;
        // A line is not sent until it is flushed. After a failed write there is deliberately NO
        // flush: nothing was written, so nothing is confirmed.
        self.device
            .flush()
            .map_err(|error| TransportError::new(format!("the serial flush failed: {error}")))
    }

    fn read_line(&mut self, deadline: Deadline) -> LineRead {
        let timeout = Duration::try_from_secs_f64(deadline.seconds().max(0.0))
            .unwrap_or(Duration::from_secs_f64(INITIAL_READ_SECONDS));
        if let Err(error) = self.device.set_timeout(timeout) {
            return LineRead::Failed {
                message: format!("cannot set the serial read timeout: {error}"),
            };
        }

        loop {
            if let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') {
                let mut line: Vec<u8> = self.buffer.drain(..=index).collect();
                line.pop(); // the '\n'
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                return LineRead::Line(String::from_utf8_lossy(&line).into_owned());
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

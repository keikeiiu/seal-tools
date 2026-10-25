//! The serial adapter, tested against an injected device — no board, no driver, no port.
//!
//! What CAN be tested without the OS: the wire bytes and flush, the chunked-line assembly, the
//! `Deadline`→`Silent`/`Failed` mapping, the boot wait through the `Clock` seam, and the injected
//! writer failure. What needs the real machine is named in `crate::serial`'s docs and in
//! `tests/live_only.rs` (ignored by default).

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use seal_board::clock::Clock;
use seal_board::serial::{BOOT_WAIT_SECONDS, SerialDevice, SerialTransport, boot_wait};
use seal_board::transport::{Deadline, LineRead, Transport};

/// One scripted read result.
type ReadResult = io::Result<Vec<u8>>;

/// What the fake device was asked to do, in order — so a test can prove the wire bytes and that a
/// failed write was NOT followed by a flush.
#[derive(Debug, Clone, PartialEq)]
enum Call {
    Write(Vec<u8>),
    Flush,
    SetTimeout(Duration),
    Read,
}

type Calls = Arc<Mutex<Vec<Call>>>;

struct FakeDevice {
    reads: VecDeque<ReadResult>,
    calls: Calls,
    fail_write: Option<&'static str>,
}

impl FakeDevice {
    fn new(reads: Vec<ReadResult>) -> Self {
        Self {
            reads: reads.into(),
            calls: Calls::default(),
            fail_write: None,
        }
    }

    fn failing_write(message: &'static str) -> Self {
        Self {
            fail_write: Some(message),
            ..Self::new(Vec::new())
        }
    }
}

impl SerialDevice for FakeDevice {
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.calls
            .lock()
            .expect("not poisoned")
            .push(Call::Write(bytes.to_vec()));
        match self.fail_write {
            Some(message) => Err(io::Error::new(io::ErrorKind::BrokenPipe, message)),
            None => Ok(()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.calls.lock().expect("not poisoned").push(Call::Flush);
        Ok(())
    }

    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.calls
            .lock()
            .expect("not poisoned")
            .push(Call::SetTimeout(timeout));
        Ok(())
    }

    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.calls.lock().expect("not poisoned").push(Call::Read);
        let next = self.reads.pop_front().unwrap_or(Ok(Vec::new()));
        let bytes = next?;
        let count = bytes.len().min(buffer.len());
        buffer[..count].copy_from_slice(&bytes[..count]);
        Ok(count)
    }
}

#[derive(Default)]
struct RecordingClock {
    slept: Arc<Mutex<Vec<f64>>>,
}

impl RecordingClock {
    fn total(&self) -> Vec<f64> {
        self.slept.lock().expect("not poisoned").clone()
    }
}

impl Clock for RecordingClock {
    fn sleep_seconds(&self, seconds: f64) {
        self.slept.lock().expect("not poisoned").push(seconds);
    }
}

/// A transport plus the shared record of what the fake device was asked to do.
fn transport(reads: Vec<ReadResult>) -> (SerialTransport, Calls) {
    let device = FakeDevice::new(reads);
    let calls = Arc::clone(&device.calls);
    (SerialTransport::with_device(Box::new(device)), calls)
}

fn recorded_calls(calls: &Calls) -> Vec<Call> {
    calls.lock().expect("not poisoned").clone()
}

fn timeout() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "the driver timed out")
}

#[test]
fn a_command_goes_out_as_the_exact_wire_line_and_one_flush() {
    let (mut transport, calls) = transport(Vec::new());

    transport
        .write_line("D 100 -20\n")
        .expect("the write succeeds");

    assert_eq!(
        recorded_calls(&calls),
        vec![Call::Write(b"D 100 -20\n".to_vec()), Call::Flush],
        "the wire text is written verbatim (its one newline included), then flushed"
    );
}

#[test]
fn a_line_without_its_newline_still_gets_exactly_one_newline() {
    // The grammar owns the wire text and always ends it with '\n'; this is the defensive promise the
    // adapter makes if a caller ever hands it a bare line.
    let (mut transport, calls) = transport(Vec::new());

    transport.write_line("V").expect("the write succeeds");

    assert_eq!(
        recorded_calls(&calls),
        vec![Call::Write(b"V\n".to_vec()), Call::Flush]
    );
}

#[test]
fn an_injected_write_failure_is_a_typed_error_and_not_a_flush() {
    // The seam the task asks for: a failing writer must come back as `TransportError`, never a
    // panic, and it must not go on to flush (a half-write reported as success is the bug class).
    let device = FakeDevice::failing_write("the device was unplugged mid-write");
    let calls = Arc::clone(&device.calls);
    let mut transport = SerialTransport::with_device(Box::new(device));

    let error = transport
        .write_line("C\n")
        .expect_err("the failure must be reported");

    assert!(
        error.to_string().contains("unplugged mid-write"),
        "the transport error carries the device's line: {error}"
    );
    assert_eq!(
        recorded_calls(&calls),
        vec![Call::Write(b"C\n".to_vec())],
        "no flush after a failed write: nothing succeeded, so nothing is confirmed"
    );
}

#[test]
fn a_reply_is_assembled_across_read_chunks() {
    let (mut transport, calls) = transport(vec![Ok(b"V ".to_vec()), Ok(b"2\n".to_vec())]);

    let read = transport.read_line(Deadline::after_seconds(1.5));

    assert_eq!(read, LineRead::Line("V 2".to_owned()));
    assert_eq!(
        recorded_calls(&calls).first(),
        Some(&Call::SetTimeout(Duration::from_secs_f64(1.5))),
        "the Deadline is set on the driver in SECONDS"
    );
}

#[test]
fn a_driver_timeout_is_silence() {
    let (mut transport, _) = transport(vec![Err(timeout())]);

    assert_eq!(
        transport.read_line(Deadline::after_seconds(1.5)),
        LineRead::Silent
    );
}

#[test]
fn an_io_error_is_failed_with_its_line() {
    let (mut transport, _) = transport(vec![Err(io::Error::new(
        io::ErrorKind::BrokenPipe,
        "the device disappeared",
    ))]);

    match transport.read_line(Deadline::after_seconds(1.5)) {
        LineRead::Failed { message } => assert!(
            message.contains("device disappeared"),
            "the failure carries the driver's line: {message}"
        ),
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn a_closed_port_is_failed_not_an_endless_silence() {
    let (mut transport, _) = transport(vec![Ok(Vec::new())]);

    match transport.read_line(Deadline::after_seconds(1.5)) {
        LineRead::Failed { message } => assert!(
            message.contains("closed"),
            "a closed port reads differently from a silent board: {message}"
        ),
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn a_partial_line_survives_a_timeout_until_its_newline_arrives() {
    let (mut transport, _) = transport(vec![
        Ok(b"V".to_vec()),
        Err(timeout()),
        Ok(b" 2\n".to_vec()),
    ]);

    assert_eq!(
        transport.read_line(Deadline::after_seconds(1.5)),
        LineRead::Silent,
        "a half line plus a timeout is still silence for this read"
    );
    assert_eq!(
        transport.read_line(Deadline::after_seconds(1.5)),
        LineRead::Line("V 2".to_owned()),
        "the bytes are not lost; the next read completes the line"
    );
}

#[test]
fn a_carriage_return_is_not_part_of_the_line() {
    let (mut transport, _) = transport(vec![Ok(b"V 2\r\n".to_vec())]);

    assert_eq!(
        transport.read_line(Deadline::after_seconds(1.5)),
        LineRead::Line("V 2".to_owned())
    );
}

#[test]
fn the_boot_wait_is_two_seconds_through_the_clock_seam() {
    // v2 opened the port, asserted DTR/RTS and then `Task.Delay(2000)`: the ATmega32U4 resets on
    // DTR and the sketch's `setup()` costs another 3 s, so the first command must wait.
    let clock = RecordingClock::default();

    boot_wait(&clock);

    assert_eq!(BOOT_WAIT_SECONDS, 2.0);
    assert_eq!(clock.total(), vec![2.0], "SECONDS, not milliseconds");
}

#[test]
fn the_adapter_fits_the_transport_trait_object_the_core_holds() {
    fn speak(transport: &mut dyn Transport) -> LineRead {
        transport.write_line("V\n").expect("the write succeeds");
        transport.read_line(Deadline::after_seconds(0.5))
    }

    let (mut transport, _) = transport(vec![Ok(b"V 2\n".to_vec())]);

    assert_eq!(speak(&mut transport), LineRead::Line("V 2".to_owned()));
}

#[cfg(windows)]
#[test]
fn opening_a_port_that_does_not_exist_is_a_typed_error() {
    // No board on this machine: the OS refuses the name and the adapter reports it, rather than
    // panicking. (The successful-open path is live-only; see tests/live_only.rs.)
    let clock = RecordingClock::default();

    let error = match SerialTransport::open("COM_DOES_NOT_EXIST_999", 115200, &clock) {
        Ok(_) => panic!("there is no such port on this machine"),
        Err(error) => error,
    };

    assert!(
        error.to_string().contains("COM_DOES_NOT_EXIST_999"),
        "the line names the port: {error}"
    );
    assert!(
        clock.total().is_empty(),
        "the boot wait is not paid when the port never opened"
    );
}

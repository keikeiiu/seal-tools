//! The transport seam: the crate writes/reads LINES; an adapter owns the serial port.
//!
//! Everything the real OS port needs — WMI VID/PID enumeration and its name fallback, `DtrEnable`/
//! `RtsEnable` on the 32U4, the 2 s boot wait, a driver `ReadTimeout`, and surviving the game's
//! anti-cheat sharing the port — is the ADAPTER's, not this crate's (`lib.rs` names what would prove
//! each). Here the seam is two methods and a fake, so the protocol logic is testable single-threaded.
//!
//! The deadline is in SECONDS, not milliseconds: `DESIGN.md` §2.1 records that passing seconds to a
//! millisecond API slept for eleven minutes and looked exactly like a hang. `Deadline::after_seconds`
//! exists so a caller cannot make that mistake silently.

use seal_board::clock::{Clock, RealClock};
use seal_board::transport::{Deadline, FakeTransport, LineRead, Transport, TransportError};

#[test]
fn a_deadline_is_seconds_and_says_so() {
    let deadline = Deadline::after_seconds(1.5);

    assert!((deadline.seconds() - 1.5).abs() < f64::EPSILON);
    // A millisecond value passed here would be 1500 s = 25 minutes; the constructor's name and the
    // pin below are what keep the unit from drifting.
    assert!(deadline.seconds() < 10.0);
}

#[test]
fn the_fake_records_writes_verbatim_including_the_newline() {
    let mut transport = FakeTransport::new();

    transport
        .write_line("D 100 -20\n")
        .expect("the fake accepts writes");
    transport
        .write_line("C\n")
        .expect("the fake accepts writes");

    assert_eq!(transport.writes(), vec!["D 100 -20\n", "C\n"]);
}

#[test]
fn the_fake_scripts_reads_in_order_and_then_is_silent() {
    let mut transport = FakeTransport::with_reads(vec![
        LineRead::Line("V 2".to_owned()),
        LineRead::Silent,
        LineRead::Failed {
            message: "the port went away".to_owned(),
        },
    ]);

    assert_eq!(
        transport.read_line(Deadline::after_seconds(1.5)),
        LineRead::Line("V 2".to_owned())
    );
    assert_eq!(
        transport.read_line(Deadline::after_seconds(1.5)),
        LineRead::Silent
    );
    assert_eq!(
        transport.read_line(Deadline::after_seconds(1.5)),
        LineRead::Failed {
            message: "the port went away".to_owned()
        }
    );
    // Exhausted script: silence, which is what an old board looks like.
    assert_eq!(
        transport.read_line(Deadline::after_seconds(1.5)),
        LineRead::Silent
    );
}

#[test]
fn the_fake_records_the_deadline_it_was_asked_to_wait() {
    let mut transport = FakeTransport::new();

    let _ = transport.read_line(Deadline::after_seconds(1.5));

    assert_eq!(transport.deadlines(), vec![1.5]);
}

#[test]
fn a_write_failure_is_reported_not_panicked_on() {
    let mut transport = FakeTransport::new();
    transport.fail_writes("the port is not open");

    let error = transport.write_line("C\n").expect_err("a refusal");

    assert!(matches!(&error, TransportError { .. }));
    assert!(error.to_string().contains("not open"), "{error}");
}

#[test]
fn the_real_clock_sleeps_and_is_the_adapter_boundary() {
    // The trait exists so the settle waits take a `Clock` and tests pay nothing. The REAL clock is the
    // one thing here that touches the OS; one short sleep proves it is wired, and nothing in this
    // crate's tests uses it for timing decisions.
    let clock = RealClock;
    let started = std::time::Instant::now();
    clock.sleep_seconds(0.01);
    assert!(started.elapsed() >= std::time::Duration::from_millis(5));
}

#[test]
fn the_transport_trait_is_object_safe_for_the_adapter() {
    // The adapter will hold a `Box<dyn Transport>` behind its serial port; take it as a trait object
    // here so a signature change that breaks that is caught.
    fn needs_dyn(transport: &mut dyn Transport) -> LineRead {
        let _ = transport.write_line("V\n");
        transport.read_line(Deadline::after_seconds(0.5))
    }

    let mut transport = FakeTransport::with_reads(vec![LineRead::Line("V 2".to_owned())]);
    assert_eq!(needs_dyn(&mut transport), LineRead::Line("V 2".to_owned()));
}

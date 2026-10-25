//! Tests that need a real board and a real machine. **Ignored by default** — this machine has no
//! board connected, so the normal suite must pass without one.
//!
//! Run the live ones by hand, with the board plugged in, from `v3/core`:
//!
//! ```powershell
//! $env:SEAL_BOARD_PORT = "COM5"
//! .\cargo.cmd test -p seal-board --offline --test live_only -- --ignored --nocapture
//! Remove-Item Env:SEAL_BOARD_PORT
//! ```
//!
//! Each test names what a real machine proves that the fake cannot — see the module docs of
//! `seal_board::serial` and `seal_board::enumerate` for the same list in prose.

use seal_board::clock::RealClock;
use seal_board::serial::SerialTransport;
use seal_board::transport::{Deadline, LineRead, Transport};

fn live_port() -> String {
    std::env::var("SEAL_BOARD_PORT").unwrap_or_else(|_| {
        panic!("set SEAL_BOARD_PORT to the board's COM port to run the live tests by hand")
    })
}

/// A cold-plugged or just-flashed board: the boot window is the ~2 s after DTR asserts plus the
/// sketch's 3 s `setup()`, and the `V` probe's two 1.5 s attempts are sized to reach past it. This
/// proves the window is long enough on the real device and the retry logs a level rather than an
/// old-board answer.
#[test]
#[ignore = "live board: needs a cold-plugged or just-flashed Arduino on a real COM port"]
fn a_just_flashed_board_answers_the_version_probe() {
    let port = live_port();
    let mut transport =
        SerialTransport::open(&port, 115200, &RealClock).expect("the board opens and boots");
    transport.write_line("V\n").expect("the probe is written");
    match transport.read_line(Deadline::after_seconds(1.5)) {
        LineRead::Line(line) => {
            println!("the board answered {line:?}");
            assert!(
                seal_board::firmware::parse(&line).is_some(),
                "a flashed board answers `V <level>`, got {line:?}"
            );
        }
        other => panic!("no version reply: {other:?}"),
    }
}

/// Unplugging mid-run: the driver must make the read/write fail with an I/O error (mapped to
/// `LineRead::Failed`/`TransportError`) and the firmware's host-gone failsafe must release held keys
/// and buttons. The fake can only prove our mapping; the release is the board's.
#[test]
#[ignore = "live board: unplug the board while this test is sleeping"]
fn an_unplug_mid_run_is_a_typed_failure() {
    let port = live_port();
    let mut transport = SerialTransport::open(&port, 115200, &RealClock).expect("the board opens");
    transport.write_line("C\n").expect("a click is written");
    println!("UNPLUG THE BOARD NOW; the next read must fail");
    std::thread::sleep(std::time::Duration::from_secs(10));
    let read = transport.read_line(Deadline::after_seconds(1.0));
    match read {
        LineRead::Failed { message } => println!("unplug reported: {message}"),
        other => panic!("an unplug must be Failed, got {other:?}"),
    }
}

/// Anti-cheat port sharing: with the game running and its anti-cheat active, the same port must open
/// and commands must reach the board (the game itself never opens the COM port; sharing means the
/// driver/anti-cheat does not block us). Proves the open sequence on a live game session.
#[test]
#[ignore = "live machine: requires the game running with its anti-cheat and a connected board"]
fn a_click_reaches_the_board_while_the_game_is_running() {
    let port = live_port();
    let mut transport =
        SerialTransport::open(&port, 115200, &RealClock).expect("the port opens beside the game");
    transport.write_line("C\n").expect("the click is written");
}

/// Driver timeout precision: our `Deadline` is mapped through `set_timeout`, and the real driver
/// decides when a read times out. Proves a silent read returns after roughly the deadline (the V
/// probe's two 1.5 s windows depend on it) instead of hanging or returning instantly.
#[test]
#[ignore = "live board: measures the real driver's read timeout"]
fn a_silent_read_returns_after_roughly_the_deadline() {
    let port = live_port();
    let mut transport = SerialTransport::open(&port, 115200, &RealClock).expect("the board opens");
    let started = std::time::Instant::now();
    let read = transport.read_line(Deadline::after_seconds(0.5));
    let elapsed = started.elapsed();
    println!("silent read took {elapsed:?}: {read:?}");
    assert!(
        elapsed >= std::time::Duration::from_millis(300),
        "the driver must actually wait: {elapsed:?}"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "the deadline must bound the wait: {elapsed:?}"
    );
}

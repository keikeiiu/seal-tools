//! Port enumeration decisions, tested against hand-built lists — the OS call is the only untested
//! line (`crate::enumerate::available_ports`, named as such).
//!
//! v2's `Arduino.Find`: match the configured VID/PID; when nothing matches, fall back to a port whose
//! device name mentions Arduino. Both halves are decisions over data, so both are pinned here.

use seal_board::enumerate::{PortInfo, choose_port, discover, names};

fn usb(name: &str, vid: u16, pid: u16, product: &str) -> PortInfo {
    PortInfo::usb(name, vid, pid, Some(product.to_owned()))
}

fn plain(name: &str) -> PortInfo {
    PortInfo::plain(name)
}

const ARDUINO_VID: u16 = 0x2341;
const LEONARDO_PID: u16 = 0x8036;

#[test]
fn the_configured_vid_and_pid_win_over_list_order() {
    let ports = vec![
        usb("COM3", 0x0403, 0x6001, "FTDI serial adapter"),
        usb("COM5", ARDUINO_VID, LEONARDO_PID, "Arduino Leonardo"),
    ];

    assert_eq!(
        discover(&ports, ARDUINO_VID, &[LEONARDO_PID]),
        Some("COM5".to_owned())
    );
}

#[test]
fn any_of_the_configured_pids_matches() {
    let ports = vec![
        usb("COM3", 0x0403, 0x6001, "other"),
        usb("COM7", ARDUINO_VID, 0x8037, "Arduino Micro"),
    ];

    assert_eq!(
        discover(&ports, ARDUINO_VID, &[LEONARDO_PID, 0x8037]),
        Some("COM7".to_owned())
    );
}

#[test]
fn the_name_fallback_fires_when_no_vid_pid_matches() {
    // The case the fallback exists for: a machine whose USB IDs are not reported correctly, so the
    // board is found by its product name instead.
    let ports = vec![
        usb("COM3", 0x0403, 0x6001, "FTDI serial adapter"),
        usb("COM9", 0x0000, 0x0000, "Arduino Leonardo"),
    ];

    assert_eq!(
        discover(&ports, ARDUINO_VID, &[LEONARDO_PID]),
        Some("COM9".to_owned())
    );
}

#[test]
fn the_name_fallback_is_case_insensitive_and_first_match_wins() {
    let ports = vec![
        usb("COM4", 0, 0, "arduino pro micro"),
        usb("COM6", 0, 0, "ARDUINO LEONARDO"),
    ];

    assert_eq!(
        discover(&ports, ARDUINO_VID, &[LEONARDO_PID]),
        Some("COM4".to_owned())
    );
}

#[test]
fn the_name_fallback_can_use_the_manufacturer() {
    let mut info = usb("COM8", 0, 0, "usb serial");
    info = info.with_manufacturer(Some("Arduino LLC".to_owned()));

    assert_eq!(
        discover(&[info], ARDUINO_VID, &[LEONARDO_PID]),
        Some("COM8".to_owned())
    );
}

#[test]
fn nothing_matching_is_none_and_the_caller_writes_the_message() {
    let ports = vec![plain("COM1"), usb("COM3", 0x0403, 0x6001, "FTDI")];

    assert_eq!(discover(&ports, ARDUINO_VID, &[LEONARDO_PID]), None);
}

#[test]
fn a_named_port_that_is_present_is_used_and_a_missing_one_refuses() {
    let ports = vec![usb("COM5", ARDUINO_VID, LEONARDO_PID, "Arduino Leonardo")];

    let (port, error) = choose_port(Some("com5"), &ports, ARDUINO_VID, &[LEONARDO_PID]);
    assert_eq!(
        port.as_deref(),
        Some("com5"),
        "the configured spelling is kept"
    );
    assert_eq!(error, None);

    let (port, error) = choose_port(Some("COM7"), &ports, ARDUINO_VID, &[LEONARDO_PID]);
    assert_eq!(port, None);
    assert!(
        error.expect("the refusal").contains("COM7"),
        "a named but absent port is refused, never silently replaced by the discovered one"
    );
}

#[test]
fn no_configured_port_uses_the_enumeration() {
    let ports = vec![usb("COM5", ARDUINO_VID, LEONARDO_PID, "Arduino Leonardo")];

    let (port, error) = choose_port(None, &ports, ARDUINO_VID, &[LEONARDO_PID]);

    assert_eq!(port.as_deref(), Some("COM5"));
    assert_eq!(error, None);
}

#[test]
fn the_names_helper_is_the_list_port_choice_decides_over() {
    let ports = vec![
        plain("COM1"),
        usb("COM5", ARDUINO_VID, LEONARDO_PID, "Arduino"),
    ];

    assert_eq!(names(&ports), vec!["COM1".to_owned(), "COM5".to_owned()]);
}

//! `arduino.port` is an override, not a hint — ported from `PortChoiceTests.cs`.
//!
//! The whole point is the DIRECTION that matters: a player who names a port and does not get it must
//! get a refusal, never a quiet fall-through to whatever discovery found. The OS enumeration is the
//! adapter's; this crate only decides over a list it is handed.

use seal_board::port_choice::choose;

const MACHINE: [&str; 3] = ["COM3", "COM5", "COM9"];

fn discovered() -> String {
    "COM9".to_owned()
}

#[test]
fn a_named_port_that_exists_is_used() {
    let (port, error) = choose(Some("COM5"), &MACHINE, || Some(discovered()));

    assert_eq!(port.as_deref(), Some("COM5"));
    assert_eq!(error, None);
}

#[test]
fn a_named_port_that_does_not_exist_is_an_error_rather_than_a_fall_through() {
    let (port, error) = choose(Some("COM7"), &MACHINE, || Some(discovered()));

    assert_eq!(port, None);
    let error = error.expect("the refusal");
    assert!(
        error.contains("COM7"),
        "the line names which port is wrong: {error}"
    );
    assert!(
        error.contains("opening a different board"),
        "the line says why it refused instead of falling through: {error}"
    );
}

#[test]
fn the_port_name_is_matched_without_case() {
    let (port, error) = choose(Some("com5"), &MACHINE, || Some(discovered()));

    assert_eq!(
        port.as_deref(),
        Some("com5"),
        "the configured spelling is kept"
    );
    assert_eq!(error, None);
}

#[test]
fn surrounding_space_is_not_part_of_the_name() {
    let (port, _) = choose(Some("  COM5  "), &MACHINE, || Some(discovered()));

    assert_eq!(port.as_deref(), Some("COM5"));
}

#[test]
fn no_configured_port_falls_back_to_discovery() {
    for configured in [None, Some(""), Some("   ")] {
        let (port, error) = choose(configured, &MACHINE, || Some(discovered()));

        assert_eq!(port.as_deref(), Some("COM9"));
        assert_eq!(error, None);
    }
}

#[test]
fn nothing_named_and_nothing_found_is_the_callers_to_explain() {
    // No port and NO error: only the caller knows the configured VID/PIDs to name in the message.
    let (port, error) = choose(Some(""), &MACHINE, || None);

    assert_eq!(port, None);
    assert_eq!(error, None);
}

#[test]
fn a_named_port_is_used_even_when_discovery_would_fail() {
    // The case the override exists for: a board whose USB IDs are not reported properly, so discovery
    // finds nothing but the player knows the port.
    let (port, error) = choose(Some("COM3"), &MACHINE, || None);

    assert_eq!(port.as_deref(), Some("COM3"));
    assert_eq!(error, None);
}

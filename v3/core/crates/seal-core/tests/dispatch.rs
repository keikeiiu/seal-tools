//! The request dispatcher behind the stdio loop: `PROTOCOL.md` §1 (handshake) and §4 (commands).

use seal_core::dispatch::{CoreState, handle};
use seal_proto::Request;
use serde_json::json;
use std::path::PathBuf;

fn state() -> CoreState {
    CoreState::new(PathBuf::from(r"C:\repo"))
}

fn hello() -> Request {
    Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"}))
}

#[test]
fn hello_answers_with_the_handshake_data() {
    let mut state = state();

    let response = handle(&mut state, hello());

    assert!(response.ok, "hello must succeed: {:?}", response.error);
    let result = response.result.expect("hello carries a result");
    assert_eq!(result["protocol"], 1);
    assert_eq!(result["core_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["root"], r"C:\repo");
    assert_eq!(result["rev"], 0);
    assert!(result["config"].is_object(), "config starts as an object");
}

#[test]
fn a_request_before_hello_is_refused() {
    let mut state = state();

    let response = handle(&mut state, Request::new("2", "window.find", json!({})));

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "not_ready");
}

#[test]
fn a_protocol_mismatch_is_refused() {
    let mut state = state();

    let response = handle(
        &mut state,
        Request::new("3", "hello", json!({"protocol": 99, "client": "0.1.0"})),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_version");
}

#[test]
fn an_unknown_command_is_a_bad_request() {
    let mut state = state();
    handle(&mut state, hello());

    let response = handle(&mut state, Request::new("4", "no.such.command", json!({})));

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_request");
}

#[test]
fn shutdown_is_acknowledged_and_stops_the_loop() {
    let mut state = state();
    handle(&mut state, hello());

    let response = handle(&mut state, Request::new("5", "shutdown", json!({})));

    assert!(response.ok, "shutdown is answered before the process exits");
    assert!(
        state.stopping,
        "the loop must stop after the acknowledgement"
    );
}

fn handshaken_state() -> CoreState {
    let mut state = state();
    handle(&mut state, hello());
    state
}

/// The dispatcher-level checks: argument shape and the revision contract, without a config folder
/// (the successful write paths are exercised end-to-end in `tests/config.rs`).
#[test]
fn config_validate_answers_with_no_errors_while_domain_rules_are_pending() {
    let mut state = handshaken_state();

    let response = handle(
        &mut state,
        Request::new(
            "6",
            "config.validate",
            json!({"sections": {"quest": {"active": "run1"}}}),
        ),
    );

    assert!(response.ok, "shape validation passes: {:?}", response.error);
    assert_eq!(
        response.result.expect("a result"),
        json!({"errors": [], "warnings": []})
    );
}

#[test]
fn config_validate_refuses_a_block_the_app_does_not_own() {
    let mut state = handshaken_state();

    let response = handle(
        &mut state,
        Request::new(
            "7",
            "config.validate",
            json!({"sections": {"my_notes": {}}}),
        ),
    );

    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert_eq!(error.detail["section"], "my_notes");
}

#[test]
fn config_apply_refuses_a_missing_or_non_numeric_rev() {
    let mut state = handshaken_state();

    for args in [
        json!({"sections": {"quest": {}}}),
        json!({"rev": "1", "sections": {"quest": {}}}),
        json!({"rev": -1, "sections": {"quest": {}}}),
    ] {
        let response = handle(&mut state, Request::new("8", "config.apply", args.clone()));
        assert!(!response.ok, "args {args} must be refused");
        assert_eq!(response.error.expect("a refusal").code, "bad_request");
    }
}

#[test]
fn config_apply_refuses_a_stale_rev_and_carries_the_current_one() {
    let mut state = handshaken_state();
    assert_eq!(state.rev, 0);

    let response = handle(
        &mut state,
        Request::new(
            "9",
            "config.apply",
            json!({"rev": 7, "sections": {"quest": {"active": "x"}}}),
        ),
    );

    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "conflict");
    assert_eq!(error.detail["rev"], 0);
    assert_eq!(state.rev, 0, "a refused call must not consume a revision");
}

#[test]
fn config_apply_refuses_a_section_that_is_not_a_mapping() {
    let mut state = handshaken_state();

    let response = handle(
        &mut state,
        Request::new(
            "10",
            "config.apply",
            json!({"rev": 0, "sections": {"quest": "not a block"}}),
        ),
    );

    assert!(!response.ok);
    assert_eq!(response.error.expect("a refusal").code, "bad_request");
}

#[test]
fn ui_window_refuses_malformed_arguments() {
    let mut state = handshaken_state();

    for args in [
        json!([]),
        json!({"left": 100, "top": 50}),
        json!({"left": "100", "top": 50, "width": 900, "height": 700, "pinned": true}),
    ] {
        let response = handle(&mut state, Request::new("11", "ui.window", args.clone()));
        assert!(!response.ok, "args {args} must be refused");
        assert_eq!(response.error.expect("a refusal").code, "bad_request");
    }
}

#[test]
fn the_config_commands_before_hello_are_not_ready() {
    for cmd in [
        "config.apply",
        "config.validate",
        "config.reload",
        "ui.window",
    ] {
        let mut state = state();
        let response = handle(&mut state, Request::new("12", cmd, json!({})));
        assert!(!response.ok, "{cmd} before hello");
        assert_eq!(response.error.expect("a refusal").code, "not_ready");
    }
}

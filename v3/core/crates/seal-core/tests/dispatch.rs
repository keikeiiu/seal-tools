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

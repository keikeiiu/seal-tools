//! The wire envelope, as `v3/docs/spec/PROTOCOL.md` §2 defines it.
//!
//! These tests are the contract: the exact JSON shapes the Flutter shell will also decode.

use seal_proto::{Error, Event, Request, Response};
use serde_json::json;

#[test]
fn a_request_serialises_to_the_documented_envelope() {
    let request = Request::new("7", "window.measure", json!({}));

    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({"v": 1, "id": "7", "cmd": "window.measure", "args": {}})
    );
}

#[test]
fn a_request_round_trips_through_json() {
    let raw = json!({"v": 1, "id": "7", "cmd": "window.measure", "args": {}});

    let request: Request = serde_json::from_value(raw.clone()).unwrap();

    assert_eq!(serde_json::to_value(&request).unwrap(), raw);
}

#[test]
fn an_ok_response_echoes_the_id_and_carries_the_result() {
    let response = Response::ok("7", json!({"display": {"scale": 1.5}}));

    assert_eq!(
        serde_json::to_value(&response).unwrap(),
        json!({"v": 1, "id": "7", "ok": true, "result": {"display": {"scale": 1.5}}})
    );
}

#[test]
fn an_error_response_carries_the_code_message_and_detail() {
    let response = Response::err(
        "9",
        Error::new(
            "conflict",
            "config changed while you were editing",
            json!({"rev": 12}),
        ),
    );

    assert_eq!(
        serde_json::to_value(&response).unwrap(),
        json!({
            "v": 1,
            "id": "9",
            "ok": false,
            "error": {
                "code": "conflict",
                "message": "config changed while you were editing",
                "detail": {"rev": 12}
            }
        })
    );
}

#[test]
fn an_event_has_no_id() {
    let event = Event::new("state", json!({"tool": "pet", "running": true}));

    assert_eq!(
        serde_json::to_value(&event).unwrap(),
        json!({"v": 1, "event": "state", "data": {"tool": "pet", "running": true}})
    );
}

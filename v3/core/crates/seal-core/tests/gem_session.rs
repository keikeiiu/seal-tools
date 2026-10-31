//! G16 `calibration.gem.moves.save` through the dispatcher.

mod common;

use common::TestDir;
use seal_core::dispatch::{CoreState, handle};
use seal_proto::{Request, Response};
use serde_json::{Value, json};

fn root() -> TestDir {
    let dir = TestDir::new();
    std::fs::create_dir_all(dir.path().join("config")).unwrap();
    dir
}

fn state() -> (TestDir, CoreState) {
    let dir = root();
    let mut state = CoreState::new(dir.path().to_path_buf());
    state.config = json!({
        "gem": {
            "empty_mode": "stop",
            "movements": {"grade_next": [1, 2], "grade_prev": [-1, -2]}
        }
    });
    let hello = handle(
        &mut state,
        Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"})),
    );
    assert!(hello.ok, "hello: {:?}", hello.error);
    (dir, state)
}

fn call(state: &mut CoreState, id: &str, args: Value) -> Response {
    handle(state, Request::new(id, "calibration.gem.moves.save", args))
}

#[test]
fn a_partial_save_merges_into_gem_movements_and_keeps_the_other_routes() {
    let (dir, mut state) = state();
    let response = call(
        &mut state,
        "2",
        json!({"movements": {
            "grade_next": [3, 4],
            "radio_to_register": {"N": [10, 20], "G": [-5, 6]}
        }}),
    );
    assert!(response.ok, "save: {:?}", response.error);
    let result = response.result.expect("a result");
    assert_eq!(result["rev"], 1);
    assert_eq!(result["block"]["movements"]["grade_next"], json!([3, 4]));
    assert_eq!(
        result["block"]["movements"]["grade_prev"],
        json!([-1, -2]),
        "a route the save does not mention survives"
    );
    assert_eq!(
        result["block"]["movements"]["radio_to_register"]["N"],
        json!([10, 20])
    );
    let local = dir.read("config/local.yaml");
    assert!(local.contains("movements"), "{local}");
    assert!(local.contains("radio_to_register"), "{local}");
    let events = state.take_queued_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data["block"], "gem");
}

#[test]
fn a_malformed_leg_is_refused_naming_the_route() {
    let (_dir, mut state) = state();
    let response = call(
        &mut state,
        "2",
        json!({"movements": {"register_combine": [1]}}),
    );
    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert_eq!(error.detail["field"], "register_combine");
    assert!(error.message.contains("exactly two"), "{}", error.message);
}

#[test]
fn an_empty_payload_or_unknown_route_is_refused() {
    let (_dir, mut state) = state();
    let empty = call(&mut state, "2", json!({"movements": {}}));
    assert!(!empty.ok);
    assert_eq!(empty.error.unwrap().code, "bad_request");

    let unknown = call(
        &mut state,
        "3",
        json!({"movements": {"grade_sidways": [1, 2]}}),
    );
    assert!(!unknown.ok);
    let error = unknown.error.expect("a refusal");
    assert_eq!(error.detail["field"], "grade_sidways");
    assert!(error.message.contains("unknown"), "{}", error.message);
}

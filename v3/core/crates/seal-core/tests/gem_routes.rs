//! G17 `calibration.gem.test_route` / `calibration.gem.cycle` through the dispatcher.

mod common;

use common::TestDir;
use seal_core::dispatch::{CoreState, handle};
use seal_proto::{Request, Response};
use serde_json::{Value, json};

fn state() -> (TestDir, CoreState) {
    let dir = TestDir::new();
    std::fs::create_dir_all(dir.path().join("config")).unwrap();
    let mut state = CoreState::new(dir.path().to_path_buf());
    state.config = json!({
        "gem": {
            "grade_positions": {"N": [10, 20], "G": [30, 40], "DG": [50, 60], "Register": [70, 80], "Combine": [90, 100]},
            "resource_gems": [[110, 120], [130, 140], [150, 160]],
            "movements": {
                "radio_to_register": {"N": [1, 2]},
                "register_combine": [3, 4],
                "combine_register": [5, 6]
            }
        }
    });
    let hello = handle(
        &mut state,
        Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"})),
    );
    assert!(hello.ok, "hello: {:?}", hello.error);
    (dir, state)
}

fn call(state: &mut CoreState, id: &str, cmd: &str, args: Value) -> Response {
    handle(state, Request::new(id, cmd, args))
}

#[test]
fn a_route_with_an_uncalibrated_target_is_refused_before_any_click() {
    let (_dir, mut state) = state();
    let response = call(
        &mut state,
        "2",
        "calibration.gem.test_route",
        json!({"move": "arduino", "from": "N", "to": "Missing"}),
    );
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().detail["field"], "to");
}

#[test]
fn a_tuned_route_without_counts_is_refused_naming_the_pair() {
    let (_dir, mut state) = state();
    let response = call(
        &mut state,
        "2",
        "calibration.gem.test_route",
        json!({"move": "tuned", "from": "Combine", "to": "Resource1"}),
    );
    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.detail["field"], "tuned");
    assert!(error.message.contains("Combine"), "{}", error.message);
}

#[test]
fn a_cycle_without_confirm_or_with_an_uncalibrated_point_is_refused() {
    let (_dir, mut state) = state();
    let unconfirmed = call(&mut state, "2", "calibration.gem.cycle", json!({}));
    assert!(!unconfirmed.ok);
    assert_eq!(unconfirmed.error.unwrap().detail["field"], "confirm");

    state.config["gem"]["grade_positions"] = json!({"N": [10, 20]});
    let incomplete = call(
        &mut state,
        "3",
        "calibration.gem.cycle",
        json!({"confirm": true}),
    );
    assert!(!incomplete.ok);
    let error = incomplete.error.expect("a refusal");
    assert_eq!(error.detail["field"], "Register");
    assert!(
        error.message.contains("nothing was clicked"),
        "{}",
        error.message
    );
}

#[test]
fn a_bad_move_set_is_refused() {
    let (_dir, mut state) = state();
    let response = call(
        &mut state,
        "2",
        "calibration.gem.test_route",
        json!({"move": "teleport", "from": "N", "to": "Register"}),
    );
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().detail["field"], "move");
}

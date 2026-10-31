//! G19/G05 `calibration.buysell.preview`, `buy_sell.preflight` and the Test-scroll refusals.

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
        "buy_sell": {
            "bag_grid": [0, 0, 800, 400],
            "bag_slot": [0, 0, 100, 50],
            "shop_region": [100, 300, 400, 400],
            "shop_rows": 10,
            "scroll_point": [1000, 420],
            "max_button": [900, 500],
            "sell_cap": 8,
            "presets": {"Potion": {"scroll": 0, "row": 2, "count": 2}}
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
fn the_preview_returns_the_64_centres_and_row_overlay() {
    let (_dir, mut state) = state();
    let response = call(
        &mut state,
        "2",
        "calibration.buysell.preview",
        json!({"draft": {
            "bag_grid": [0, 0, 800, 400],
            "bag_slot": [0, 0, 100, 50],
            "shop_region": [100, 300, 400, 400],
            "shop_rows": 10
        }}),
    );
    assert!(response.ok, "{:?}", response.error);
    let result = response.result.unwrap();
    assert_eq!(result["bag_centres"].as_array().unwrap().len(), 64);
    assert_eq!(result["row_centres"].as_array().unwrap().len(), 10);
    assert_eq!(result["pitch"], json!({"w": 100, "h": 50}));
    assert_eq!(result["row_pitch"], 40.0);
    assert!(result["problem"].is_null());
}

#[test]
fn a_disagreeing_slot_returns_a_problem_instead_of_an_error() {
    let (_dir, mut state) = state();
    let response = call(
        &mut state,
        "2",
        "calibration.buysell.preview",
        json!({"draft": {
            "bag_grid": [0, 0, 800, 400],
            "bag_slot": [0, 0, 250, 200],
            "shop_region": [100, 300, 400, 400],
            "shop_rows": 10
        }}),
    );
    assert!(response.ok, "{:?}", response.error);
    let problem = response.result.unwrap()["problem"].clone();
    assert!(problem.as_str().unwrap().contains("slot box"), "{problem}");
}

#[test]
fn the_preflight_reports_ready_and_each_mark() {
    let (_dir, mut state) = state();
    let response = call(&mut state, "2", "buy_sell.preflight", json!({}));
    assert!(response.ok, "{:?}", response.error);
    let result = response.result.unwrap();
    assert_eq!(result["ready"], true);
    assert!(result["lines"].as_array().unwrap().is_empty());
    assert!(
        result["calibration"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["label"] == "Bag grid" && item["ok"] == true)
    );
}

#[test]
fn test_scroll_refuses_too_many_notches_and_a_missing_focus_point() {
    let (_dir, mut state) = state();
    let too_many = call(
        &mut state,
        "2",
        "calibration.shop.test_scroll",
        json!({"notches": 401}),
    );
    assert!(!too_many.ok);
    assert_eq!(too_many.error.unwrap().detail["field"], "notches");

    state.config["buy_sell"]["scroll_point"] = Value::Null;
    let missing = call(
        &mut state,
        "3",
        "calibration.shop.test_scroll",
        json!({"notches": 3}),
    );
    assert!(!missing.ok);
    assert_eq!(missing.error.unwrap().detail["field"], "scroll_point");
}

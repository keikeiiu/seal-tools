//! G14 `pet.scan_feeding_page` / `pet.scan_apply_queue` through the dispatcher.

mod common;

use base64::Engine as _;
use common::TestDir;
use seal_core::dispatch::{CoreState, handle};
use seal_proto::{Request, Response};
use serde_json::{Value, json};

fn state() -> (TestDir, CoreState) {
    let dir = TestDir::new();
    std::fs::create_dir_all(dir.path().join("config")).unwrap();
    let mut state = CoreState::new(dir.path().to_path_buf());
    state.config = json!({"pet": {}});
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

fn icon_png() -> String {
    let pixels = [120u8, 30, 200, 255].repeat(16 * 16);
    let png = seal_win::pure::encode_png(16, 16, &pixels).expect("a PNG");
    base64::engine::general_purpose::STANDARD.encode(png)
}

#[test]
fn a_queue_replacement_writes_only_the_queue_and_bumps_the_revision() {
    let (dir, mut state) = state();
    let before = state.rev;
    let response = call(
        &mut state,
        "2",
        "pet.scan_apply_queue",
        json!({"queue": [{"label": "page 1, cell 7", "png": icon_png(), "rect": [10, 20, 30, 40]}]}),
    );
    assert!(response.ok, "apply: {:?}", response.error);
    let result = response.result.expect("a result");
    assert_eq!(result["rev"], before + 1);
    assert_eq!(result["block"]["queue"][0]["label"], "page 1, cell 7");
    let local = dir.read("config/local.yaml");
    assert!(local.contains("page 1, cell 7"), "{local}");
}

#[test]
fn an_empty_or_corrupt_queue_replacement_is_refused() {
    let (_dir, mut state) = state();
    let empty = call(
        &mut state,
        "2",
        "pet.scan_apply_queue",
        json!({"queue": []}),
    );
    assert!(!empty.ok);
    assert_eq!(empty.error.unwrap().detail["field"], "queue");

    let corrupt = call(
        &mut state,
        "3",
        "pet.scan_apply_queue",
        json!({"queue": [{"label": "x", "png": "not-base64"}]}),
    );
    assert!(!corrupt.ok);
    assert_eq!(corrupt.error.unwrap().detail["field"], "queue");
}

#[test]
fn the_sweep_refuses_missing_prerequisites_before_touching_the_game() {
    let (_dir, mut state) = state();
    let missing_tooltip = call(
        &mut state,
        "2",
        "pet.scan_feeding_page",
        json!({"page": 1, "harvest": true}),
    );
    assert!(!missing_tooltip.ok);
    assert_eq!(missing_tooltip.error.unwrap().detail["field"], "bag_grid");

    let bad_page = call(&mut state, "3", "pet.scan_feeding_page", json!({"page": 0}));
    assert!(!bad_page.ok);
    assert_eq!(bad_page.error.unwrap().detail["field"], "page");
}

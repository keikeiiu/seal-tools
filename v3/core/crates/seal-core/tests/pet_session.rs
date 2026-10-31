//! G03 `calibration.pet.preview` / `.save` through the dispatcher, on a temp root.
//!
//! `test_read` needs a live game and OCR models, so its ladder's pure pixel helper is unit-tested in
//! the module and the command's refusal paths are covered here. `save` crops/annotates a synthetic
//! frame and is fully testable without the models.

mod common;

use common::TestDir;
use seal_core::dispatch::{CoreState, handle};
use seal_proto::{Request, Response};
use serde_json::{Value, json};

fn root() -> TestDir {
    let dir = TestDir::new();
    std::fs::create_dir_all(dir.path().join("config")).expect("makes config dir");
    dir
}

fn harness(width: u32, height: u32) -> (TestDir, CoreState, String) {
    let dir = root();
    let mut state = CoreState::new(dir.path().to_path_buf());
    let hello = handle(
        &mut state,
        Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"})),
    );
    assert!(hello.ok, "hello: {:?}", hello.error);
    let pixels = vec![255u8; (width * height * 4) as usize];
    let png = seal_win::pure::encode_png(width, height, &pixels).expect("encodes a frame");
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png);
    let frame_id = state.frames.insert(json!({
        "format": "png",
        "width": width,
        "height": height,
        "origin": {"x": 0, "y": 0},
        "data": data,
    }));
    (dir, state, frame_id)
}

fn call(state: &mut CoreState, id: &str, cmd: &str, args: Value) -> Response {
    handle(state, Request::new(id, cmd, args))
}

fn points() -> Value {
    json!({
        "menu": {"x": 10, "y": 10, "w": 20, "h": 20},
        "feed": {"x": 40, "y": 10, "w": 20, "h": 20},
        "close": {"x": 70, "y": 10, "w": 20, "h": 20},
        "page_tabs": [{"x": 300, "y": 20}, {"x": 340, "y": 20}, {"x": 380, "y": 20}]
    })
}

fn rows() -> Value {
    json!([{
        "toggle_label": {"x": 10, "y": 200, "w": 60, "h": 20},
        "boarding_pet_slot": {"x": 100, "y": 200, "w": 100, "h": 100},
        "feeder_strip": {"x": 100, "y": 320, "w": 400, "h": 40},
        "stacks": 4,
        "pet_slot_empty_png": true
    }])
}

#[test]
fn preview_derives_rows_slots_counts_and_bag_centres() {
    let (_dir, mut state, _frame_id) = harness(1200, 900);
    let response = call(
        &mut state,
        "2",
        "calibration.pet.preview",
        json!({
            "draft": {
                "rows": [{"feeder_strip": {"x": 0, "y": 100, "w": 200, "h": 40}, "stacks": 2}],
                "bag_grid": [0, 0, 80, 80],
            }
        }),
    );
    assert!(response.ok, "{:?}", response.error);
    let result = response.result.unwrap();
    assert_eq!(result["rows"][0]["slot_boxes"].as_array().unwrap().len(), 2);
    assert_eq!(
        result["rows"][0]["count_regions"].as_array().unwrap().len(),
        2
    );
    assert_eq!(result["bag_centres"].as_array().unwrap().len(), 64);
    assert!(result["problem"].is_null());
}

#[test]
fn save_writes_the_calibration_half_and_crops_the_empty_slot() {
    let (dir, mut state, frame_id) = harness(1200, 900);
    let response = call(
        &mut state,
        "2",
        "calibration.pet.save",
        json!({
            "frame_id": frame_id,
            "rows": rows(),
            "points": points(),
            "bag": {"grid": [0, 0, 240, 240], "slot": [0, 0, 30, 30]},
            "feeder_count_left_fraction": 0.4,
            "feeder_count_min_score": 0.75,
            "display": {"dpi_scale": 1.5, "monitor_dpi": 144, "screen": {"w": 1920, "h": 1080}},
        }),
    );
    assert!(response.ok, "save: {:?}", response.error);
    let result = response.result.expect("a result");
    assert_eq!(result["rev"], 1);
    assert_eq!(result["block"]["slots"][0]["stacks"], 4);
    assert!(
        result["block"]["slots"][0]["pet_slot_empty_png"]
            .as_str()
            .is_some(),
        "the empty-slot crop is inline base64"
    );
    assert_eq!(result["block"]["page_tabs"][0], json!([300, 20]));
    assert_eq!(result["artifacts"][0]["file"], "calib_pet.png");
    let local = dir.read("config/local.yaml");
    assert!(local.contains("slots"), "{local}");
    assert!(local.contains("toggle_label"), "{local}");
    assert!(local.contains("page_tabs"), "{local}");
    assert!(dir.exists("config/calib_pet.png"));
    let events = state.take_queued_events();
    assert_eq!(events[0].data["block"], "pet");
    assert!(
        events
            .iter()
            .any(|event| event.data["block"] == "calibration")
    );
}

#[test]
fn save_refuses_more_than_four_rows() {
    let (_dir, mut state, frame_id) = harness(1200, 900);
    let row = rows().as_array().unwrap()[0].clone();
    let response = call(
        &mut state,
        "2",
        "calibration.pet.save",
        json!({"frame_id": frame_id, "rows": vec![row; 5], "points": points()}),
    );
    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(error.message.contains("at most 4"), "{}", error.message);
}

#[test]
fn save_refuses_a_tiny_row_mark_with_the_field_named() {
    let (_dir, mut state, frame_id) = harness(1200, 900);
    let mut row = rows().as_array().unwrap()[0].clone();
    row["toggle_label"] = json!({"x": 10, "y": 200, "w": 3, "h": 20});
    let response = call(
        &mut state,
        "2",
        "calibration.pet.save",
        json!({"frame_id": frame_id, "rows": [row], "points": points()}),
    );
    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert_eq!(error.detail["field"], "rows");
}

#[test]
fn test_read_refuses_an_unknown_frame_shape_before_touching_the_game() {
    let (_dir, mut state, _frame_id) = harness(1200, 900);
    // No rows → the argument check refuses before OCR/window work.
    let response = call(&mut state, "2", "calibration.pet.test_read", json!({}));
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_request");
}

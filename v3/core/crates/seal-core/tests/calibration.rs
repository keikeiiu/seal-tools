//! `calibration.save` / `calibration.fetch` through the dispatcher (`PROTOCOL.md` §4,
//! `CALIBRATION.md` §1/§5), on a temp root so the real config folder is never touched.

mod common;

use common::TestDir;
use seal_core::dispatch::{CoreState, handle};
use seal_proto::Request;
use serde_json::{Value, json};

/// A fresh root with the `config/` directory the store writes into.
fn root() -> TestDir {
    let dir = TestDir::new();
    std::fs::create_dir_all(dir.path().join("config")).expect("makes config dir");
    dir
}

fn hello_request() -> Request {
    Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"}))
}

/// A handshaken state on a fresh root, plus the frame id it holds. The frame is 800×600 unless the
/// test needs pixels (`with_png`).
fn harness(width: u32, height: u32) -> (TestDir, CoreState, String) {
    let dir = root();
    let mut state = CoreState::new(dir.path().to_path_buf());
    let response = handle(&mut state, hello_request());
    assert!(response.ok, "hello: {:?}", response.error);
    let frame_id = state.frames.insert(json!({
        "format": "png",
        "width": width,
        "height": height,
        "origin": {"x": 0, "y": 0},
        "data": "",
    }));
    (dir, state, frame_id)
}

fn png_frame(width: u32, height: u32) -> (String, String) {
    // BGRA in, as the capture blit produces it (encode_png swaps to RGBA).
    let pixels = vec![255u8; (width * height * 4) as usize];
    let png = seal_win::pure::encode_png(width, height, &pixels).expect("encodes a frame");
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png);
    (data, String::new())
}

#[test]
fn a_rect_mark_is_written_to_local_yaml_and_answers_its_derived_pitch() {
    let (dir, mut state, frame_id) = harness(800, 600);

    let response = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "bag_grid",
                "region": {"x": 0, "y": 0, "w": 800, "h": 600},
            }),
        ),
    );

    assert!(response.ok, "save: {:?}", response.error);
    let result = response.result.expect("save carries a result");
    assert_eq!(result["rev"], 1, "a save consumes one rev");
    assert_eq!(result["kind"], "bag_grid");
    assert_eq!(result["mark"], json!([0, 0, 800, 600]));
    assert_eq!(result["derived"]["pitch"]["w"], json!(100.0));
    assert_eq!(result["derived"]["pitch"]["h"], json!(75.0));
    assert_eq!(result["artifact"], Value::Null);

    let local = dir.read("config/local.yaml");
    assert!(
        local.contains("bag_grid"),
        "the mark reached the file: {local}"
    );
    assert!(local.contains("800"), "the mark's numbers reached the file");

    let events = state.take_queued_events();
    assert_eq!(events.len(), 1, "one config.changed for the write");
    assert_eq!(events[0].event, "config.changed");
    assert_eq!(events[0].data["block"], "buy_sell");
    assert_eq!(events[0].data["rev"], 1);
    assert_eq!(
        events[0].data["values"]["bag_grid"],
        json!([0, 0, 800, 600])
    );
}

#[test]
fn a_point_mark_is_stored_as_a_pair() {
    let (_dir, mut state, frame_id) = harness(800, 600);

    let response = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "focus_point",
                "point": {"x": 100, "y": 200},
            }),
        ),
    );

    assert!(response.ok, "save: {:?}", response.error);
    let result = response.result.expect("save carries a result");
    assert_eq!(result["mark"], json!([100, 200]));
    assert_eq!(result["derived"], json!({}));
}

#[test]
fn the_result_gem_crop_is_written_as_a_png_artifact() {
    let dir = root();
    let mut state = CoreState::new(dir.path().to_path_buf());
    handle(&mut state, hello_request());
    let (data, _) = png_frame(2, 2);
    let frame_id = state.frames.insert(json!({
        "format": "png",
        "width": 2,
        "height": 2,
        "origin": {"x": 0, "y": 0},
        "data": data,
    }));

    let response = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "result_gem_area",
                "region": {"x": 1, "y": 1, "w": 1, "h": 1},
            }),
        ),
    );

    assert!(response.ok, "save: {:?}", response.error);
    let result = response.result.expect("save carries a result");
    let artifact = &result["artifact"];
    assert_eq!(artifact["file"], "calib_gem_result.png");
    assert_eq!(artifact["width"], 1);
    assert_eq!(artifact["height"], 1);

    let path = dir.path().join("config").join("calib_gem_result.png");
    let bytes = std::fs::read(&path).expect("the crop exists");
    let (width, height, _) = seal_win::pure::decode_png(&bytes).expect("the crop is a PNG");
    assert_eq!((width, height), (1, 1), "the artifact is exactly the crop");
}

#[test]
fn a_bag_slot_that_disagrees_with_the_grid_is_refused_and_not_written() {
    let (dir, mut state, frame_id) = harness(800, 600);

    let grid = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "bag_grid",
                "region": {"x": 0, "y": 0, "w": 800, "h": 600},
            }),
        ),
    );
    assert!(grid.ok, "grid: {:?}", grid.error);

    let slot = handle(
        &mut state,
        Request::new(
            "3",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "bag_slot",
                "region": {"x": 0, "y": 0, "w": 4, "h": 4},
            }),
        ),
    );

    assert!(!slot.ok, "a 4×4 slot cannot belong to an 800×600 8×8 bag");
    let error = slot.error.expect("an error");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("does not agree"),
        "{}",
        error.message
    );
    let local = dir.read("config/local.yaml");
    assert!(
        !local.contains("bag_slot"),
        "a refused save writes nothing: {local}"
    );
    assert_eq!(state.rev, 1, "a refused save consumes no rev");
}

#[test]
fn unknown_kinds_are_refused_by_name_with_the_list() {
    let (_dir, mut state, frame_id) = harness(800, 600);

    let response = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({"frame_id": frame_id, "kind": "nope", "region": {"x": 0, "y": 0, "w": 1, "h": 1}}),
        ),
    );

    assert!(!response.ok);
    let error = response.error.expect("an error");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("unknown calibration kind"),
        "{}",
        error.message
    );
    let kinds = error.detail["kinds"].as_array().expect("the list travels");
    assert!(kinds.iter().any(|kind| kind == "bag_grid"));
}

#[test]
fn a_frame_older_than_the_cache_is_refused_with_a_recapture_line() {
    let (_dir, mut state, _frame_id) = harness(800, 600);

    let response = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({
                "frame_id": "f99",
                "kind": "bag_grid",
                "region": {"x": 0, "y": 0, "w": 8, "h": 8},
            }),
        ),
    );

    assert!(!response.ok);
    let error = response.error.expect("an error");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("unknown frame_id"),
        "{}",
        error.message
    );
    assert!(error.message.contains("capture the window again"));
}

#[test]
fn fetch_returns_the_stored_mark_and_the_crop() {
    let dir = root();
    let mut state = CoreState::new(dir.path().to_path_buf());
    handle(&mut state, hello_request());
    let (data, _) = png_frame(2, 2);
    let frame_id = state.frames.insert(json!({
        "format": "png",
        "width": 2,
        "height": 2,
        "origin": {"x": 0, "y": 0},
        "data": data,
    }));
    let saved = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "result_gem_area",
                "region": {"x": 0, "y": 0, "w": 2, "h": 2},
            }),
        ),
    );
    assert!(saved.ok, "save: {:?}", saved.error);

    let fetched = handle(
        &mut state,
        Request::new("3", "calibration.fetch", json!({"kind": "result_gem_area"})),
    );

    assert!(fetched.ok, "fetch: {:?}", fetched.error);
    let result = fetched.result.expect("fetch carries a result");
    assert_eq!(result["mark"], json!([0, 0, 2, 2]));
    assert_eq!(result["artifact"]["file"], "calib_gem_result.png");
    assert!(
        result["artifact"]["png_base64"].as_str().is_some(),
        "the crop travels back for the calibrator to show"
    );
    assert_eq!(result["rev"], 1);
}

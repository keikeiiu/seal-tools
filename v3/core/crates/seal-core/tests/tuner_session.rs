//! G02 `calibration.tuner.check` / `.save` through the dispatcher, on a temp root.
//!
//! The artifact/annotation tests use a synthetic PNG frame; OCR itself is not exercised here
//! because the ONNX models are not committed (the `seal-ocr` suite loads them from
//! `SEAL_OCR_MODELS` when present). `save` deliberately needs no OCR — v2's Save Tuner did not run
//! an OCR pass either — so the write path is fully testable.

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

fn boxes() -> Value {
    json!({
        "grade": {"x": 120, "y": 100, "w": 40, "h": 20},
        "attr": {"x": 100, "y": 130, "w": 200, "h": 60},
        "remaining": {"x": 110, "y": 200, "w": 150, "h": 20},
    })
}

#[test]
fn save_derives_the_geometry_writes_the_block_and_annotates_the_frame() {
    let (dir, mut state, frame_id) = harness(800, 600);
    let response = call(
        &mut state,
        "2",
        "calibration.tuner.save",
        json!({
            "frame_id": frame_id,
            "boxes": boxes(),
            "display": {"dpi_scale": 1.5, "monitor_dpi": 144, "screen": {"w": 1920, "h": 1080}},
        }),
    );
    assert!(response.ok, "save: {:?}", response.error);
    let result = response.result.expect("a result");
    assert_eq!(result["rev"], 1);
    assert_eq!(
        result["derived"]["region"],
        json!({"left": 100, "top": 100, "width": 200, "height": 120})
    );
    assert_eq!(result["derived"]["row_height"], 20, "attr height / 3");
    assert_eq!(
        result["block"]["ocr"]["grade_area"],
        json!({"x1": 20, "y1": 0, "x2": 60, "y2": 20})
    );
    assert_eq!(result["block"]["calibration"]["dpi_scale"], 1.5);
    assert_eq!(
        result["block"]["calibration"]["client_size"],
        json!([800, 600])
    );
    assert_eq!(result["artifact"]["file"], "calib_tuner.png");

    let local = dir.read("config/local.yaml");
    assert!(
        local.contains("grade_area"),
        "the geometry reached the file: {local}"
    );
    assert!(
        local.contains("measured_at"),
        "the environment stamp reached the file"
    );
    assert!(dir.exists("config/calib_tuner.png"));

    let events = state.take_queued_events();
    assert_eq!(events.len(), 2, "one config.changed per changed block");
    assert_eq!(events[0].data["block"], "tuner");
    assert_eq!(events[1].data["block"], "calibration");

    // The artifact is the frame with the bands drawn: the grade box's top-left is green.
    let png = std::fs::read(dir.path().join("config/calib_tuner.png")).unwrap();
    let (width, _, rgba) = seal_win::pure::decode_png(&png).unwrap();
    assert_eq!(width, 800);
    let at = |x: u32, y: u32| {
        let index = ((y * width + x) * 4) as usize;
        [rgba[index], rgba[index + 1], rgba[index + 2]]
    };
    assert_eq!(at(120, 100), [0, 200, 0], "the grade band is green");
    assert_eq!(at(100, 130), [0, 120, 255], "the attribute band is blue");
}

#[test]
fn save_accepts_a_measured_row_height_and_an_optional_spring_point() {
    let (dir, mut state, frame_id) = harness(800, 600);
    let response = call(
        &mut state,
        "2",
        "calibration.tuner.save",
        json!({
            "frame_id": frame_id,
            "boxes": boxes(),
            "row_height": 19,
            "spring_point": {"x": 500, "y": 400},
        }),
    );
    assert!(response.ok, "save: {:?}", response.error);
    let result = response.result.expect("a result");
    assert_eq!(result["derived"]["row_height"], 19);
    assert_eq!(result["block"]["spring_point"], json!([500, 400]));
    let local = dir.read("config/local.yaml");
    assert!(local.contains("spring_point"), "{local}");
}

#[test]
fn a_stray_or_missing_box_is_refused_with_the_field_named() {
    let (_dir, mut state, frame_id) = harness(800, 600);
    let response = call(
        &mut state,
        "2",
        "calibration.tuner.save",
        json!({
            "frame_id": frame_id,
            "boxes": {
                "grade": {"x": 120, "y": 100, "w": 2, "h": 20},
                "attr": {"x": 100, "y": 130, "w": 200, "h": 60},
                "remaining": {"x": 110, "y": 200, "w": 150, "h": 20},
            },
        }),
    );
    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert_eq!(error.detail["field"], "boxes");
    assert!(error.message.contains("only 2x20"), "{}", error.message);

    let response = call(
        &mut state,
        "3",
        "calibration.tuner.save",
        json!({"frame_id": frame_id, "boxes": {"grade": {"x": 0, "y": 0, "w": 20, "h": 20}}}),
    );
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().detail["field"], "boxes");
}

#[test]
fn check_refuses_a_frame_id_it_did_not_cache() {
    let (_dir, mut state, _frame_id) = harness(800, 600);
    let response = call(
        &mut state,
        "2",
        "calibration.tuner.check",
        json!({"frame_id": "f99", "boxes": boxes()}),
    );
    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("unknown frame_id"),
        "{}",
        error.message
    );
}

#[test]
fn check_with_boxes_uses_the_same_derivation_as_save() {
    // With no OCR models in the checkout this settles at ocr_unavailable, but the derivation must
    // already be validated before any OCR/model work: a tiny box refuses even without models.
    let (_dir, mut state, frame_id) = harness(800, 600);
    let response = call(
        &mut state,
        "2",
        "calibration.tuner.check",
        json!({
            "frame_id": frame_id,
            "boxes": {
                "grade": {"x": 0, "y": 0, "w": 3, "h": 20},
                "attr": {"x": 100, "y": 130, "w": 200, "h": 60},
                "remaining": {"x": 110, "y": 200, "w": 150, "h": 20},
            },
        }),
    );
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_request");
}

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

// ── G18: result-box colour diagnostic ─────────────────────────────────────────────────────────

fn frame_id(state: &mut CoreState, bgra: [u8; 4], width: u32, height: u32) -> String {
    let pixels = bgra.repeat((width * height) as usize);
    let png = seal_win::pure::encode_png(width, height, &pixels).expect("a PNG");
    let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png);
    state.frames.insert(json!({
        "format": "png", "width": width, "height": height,
        "origin": {"x": 0, "y": 0}, "data": data,
    }))
}

fn check(state: &mut CoreState, id: &str, args: Value) -> Response {
    handle(
        state,
        Request::new(id, "calibration.gem.check_result", args),
    )
}

#[test]
fn a_matching_signature_reads_empty_with_zero_distance() {
    let (_dir, mut state) = state();
    let frame = frame_id(&mut state, [0, 0, 255, 255], 100, 100); // red box, BGRA
    let area = json!({"x": 10, "y": 10, "w": 20, "h": 20});
    let first = check(&mut state, "2", json!({"frame_id": frame, "area": area}));
    assert!(first.ok, "check: {:?}", first.error);
    let first = first.result.expect("a result");
    assert_eq!(first["verdict"], "no_reference");
    assert!(first["distance"].is_null());
    assert!(first["crop_png_base64"].as_str().unwrap().len() > 10);

    // Feed the composition back as the empty reference: the same box must now read empty.
    state.config["gem"]["empty_signature"] = first["composition"].clone();
    let second = check(&mut state, "3", json!({"frame_id": frame, "area": area}));
    let second = second.result.expect("a result");
    assert_eq!(second["verdict"], "empty");
    assert!(second["distance"].as_f64().unwrap() < 1e-9);
    assert!(
        second["spread"].as_f64().unwrap() > 0.5,
        "a red box has a wide channel spread"
    );
    assert_eq!(second["dominant"], "reddom");
}

#[test]
fn a_different_composition_reads_has_gem() {
    let (_dir, mut state) = state();
    let frame = frame_id(&mut state, [255, 255, 255, 255], 100, 100); // white
    state.config["gem"]["empty_signature"] = json!({"mean_v": 0.0, "mean_sat": 1.0, "colored_fraction": 1.0, "mean_r": 1.0, "mean_g": 0.0, "mean_b": 0.0});
    let response = check(
        &mut state,
        "2",
        json!({"frame_id": frame, "area": {"x": 0, "y": 0, "w": 10, "h": 10}}),
    );
    assert!(response.ok, "{:?}", response.error);
    let result = response.result.unwrap();
    assert_eq!(result["verdict"], "has_gem");
    assert!(result["distance"].as_f64().unwrap() > 0.18);
}

#[test]
fn a_missing_area_and_unknown_frame_are_refused() {
    let (_dir, mut state) = state();
    let missing = check(&mut state, "2", json!({}));
    assert!(!missing.ok);
    assert_eq!(missing.error.unwrap().detail["field"], "area");

    state.config["gem"]["result_gem_area"] = json!([10, 10, 20, 20]);
    let unknown = check(&mut state, "3", json!({"frame_id": "f99"}));
    assert!(!unknown.ok);
    assert!(unknown.error.unwrap().message.contains("unknown frame_id"));
}

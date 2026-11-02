//! G04 `calibration.gem.save` + the reference-gallery echo through `calibration.fetch`.

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
    state.config = json!({"gem": {"move_mode": "arduino", "empty_mode": "stop"}});
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

fn frame(state: &mut CoreState) -> String {
    let pixels = [20u8, 20u8, 220u8, 255u8].repeat(100 * 100);
    let png = seal_win::pure::encode_png(100, 100, &pixels).expect("a PNG");
    let data = base64::engine::general_purpose::STANDARD.encode(png);
    state.frames.insert(json!({
        "format": "png", "width": 100, "height": 100,
        "origin": {"x": 0, "y": 0}, "data": data,
    }))
}

#[test]
fn a_coordinates_only_save_writes_the_marks_and_preserves_other_gem_fields() {
    let (dir, mut state) = state();
    let response = call(
        &mut state,
        "2",
        "calibration.gem.save",
        json!({
            "points": {"N": [10, 20], "Register": [30, 40]},
            "resources": [[50, 60], [70, 80], [90, 100]],
            "result_area": [100, 110, 20, 30]
        }),
    );
    assert!(response.ok, "save: {:?}", response.error);
    let result = response.result.expect("a result");
    assert_eq!(
        result["block"]["gem"]["grade_positions"]["N"],
        json!([10, 20])
    );
    assert_eq!(
        result["block"]["gem"]["result_gem_area"],
        json!([100, 110, 20, 30])
    );
    assert_eq!(result["block"]["gem"]["move_mode"], "arduino");
    assert_eq!(result["artifacts"], json!([]), "no frame = no artifact");
    let local = dir.read("config/local.yaml");
    assert!(local.contains("grade_positions"), "{local}");
}

#[test]
fn a_frame_save_writes_both_natural_size_artifacts_and_a_gallery_entry() {
    let (dir, mut state) = state();
    let frame_id = frame(&mut state);
    let response = call(
        &mut state,
        "2",
        "calibration.gem.save",
        json!({
            "frame_id": frame_id,
            "points": {"N": [10, 20], "Combine": [30, 40]},
            "resources": [[50, 60], [70, 80], [90, 100]],
            "result_area": [20, 20, 10, 10],
            "empty": true
        }),
    );
    assert!(response.ok, "save: {:?}", response.error);
    let result = response.result.expect("a result");
    assert_eq!(result["artifacts"].as_array().unwrap().len(), 2);
    assert!(result["derived"]["empty_signature"].is_object());
    assert_eq!(
        result["derived"]["result_area"],
        json!({"x": 20, "y": 20, "w": 10, "h": 10})
    );

    for file in ["calib_gem.png", "calib_gem_result.png"] {
        let path = dir.path().join("config").join(file);
        assert!(path.is_file(), "{file} was written");
        let bytes = std::fs::read(path).unwrap();
        let (width, height, _) = seal_win::pure::decode_png(&bytes).unwrap();
        assert_eq!(
            (width, height),
            if file == "calib_gem_result.png" {
                (10, 10)
            } else {
                (100, 100)
            }
        );
    }

    // A restart-shaped fetch: the gallery carries both files and their bytes.
    let fetched = call(
        &mut state,
        "3",
        "calibration.fetch",
        json!({"kind": "bag_grid"}),
    );
    assert!(fetched.ok, "{:?}", fetched.error);
    let gallery = fetched.result.unwrap()["gallery"].clone();
    let names: Vec<&str> = gallery
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect();
    assert!(names.contains(&"gem"), "{names:?}");
    assert!(names.contains(&"gem_result"), "{names:?}");
    for entry in gallery.as_array().unwrap() {
        let file = entry["file"].as_str().unwrap();
        let path = dir.path().join("config").join(file);
        let expected =
            base64::engine::general_purpose::STANDARD.encode(std::fs::read(path).unwrap());
        assert_eq!(entry["png_base64"], expected, "{file} bytes are stable");
    }
}

#[test]
fn a_malformed_point_or_resource_list_is_refused_before_any_write() {
    let (_dir, mut state) = state();
    let bad_point = call(
        &mut state,
        "2",
        "calibration.gem.save",
        json!({"points": {"N": [1]}}),
    );
    assert!(!bad_point.ok);
    assert_eq!(bad_point.error.unwrap().detail["field"], "points");

    let bad_resources = call(
        &mut state,
        "3",
        "calibration.gem.save",
        json!({"resources": [[1, 2], [3, 4]]}),
    );
    assert!(!bad_resources.ok);
    assert_eq!(bad_resources.error.unwrap().detail["field"], "resources");
}

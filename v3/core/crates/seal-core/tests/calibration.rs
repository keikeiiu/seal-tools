//! `calibration.save` / `calibration.fetch` through the dispatcher (`PROTOCOL.md` §4,
//! `CALIBRATION.md` §1/§5), on a temp root so the real config folder is never touched.

mod common;

use common::TestDir;
use seal_core::dispatch::{CoreState, handle};
use seal_proto::{Error, Request};
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
fn a_point_mark_is_stored_as_a_pair_at_the_path_the_shop_reads() {
    let (dir, mut state, frame_id) = harness(800, 600);

    let response = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "scroll_point",
                "point": {"x": 100, "y": 200},
            }),
        ),
    );

    assert!(response.ok, "save: {:?}", response.error);
    let result = response.result.expect("save carries a result");
    assert_eq!(result["kind"], "scroll_point");
    assert_eq!(result["mark"], json!([100, 200]));
    assert_eq!(result["derived"], json!({}));

    let local = dir.read("config/local.yaml");
    assert!(
        local.contains("scroll_point"),
        "the kind writes the path the live shop reads: {local}"
    );
    assert!(
        !local.contains("focus_point"),
        "the old dead field is gone: {local}"
    );
    let settings = seal_core::shop::settings(&state.config);
    assert_eq!(
        settings.scroll_point,
        Some(vec![100, 200]),
        "the shop settings resolve the saved scroll point"
    );
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
                "empty": true,
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

    // `empty:true` also samples the six-feature signature from the same crop; it is the weaker
    // reference the gem run uses when the crop cannot be compared.
    let signature = &state.config["gem"]["empty_signature"];
    assert_eq!(signature["mean_v"], json!(1.0), "{signature}");
    assert_eq!(signature["mean_r"], json!(1.0), "{signature}");
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
fn a_focus_point_request_is_refused_by_its_replacement_name() {
    let (dir, mut state, frame_id) = harness(800, 600);

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

    assert!(!response.ok, "focus_point must not be accepted silently");
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(error.message.contains("scroll_point"), "{}", error.message);
    assert!(error.message.contains("focus_point"), "{}", error.message);
    assert_eq!(error.detail["kind"], "focus_point");
    let kinds = error.detail["kinds"].as_array().expect("the list travels");
    assert!(kinds.iter().any(|kind| kind == "scroll_point"));
    assert!(!kinds.iter().any(|kind| kind == "focus_point"));
    assert_eq!(state.rev, 0, "a refused save consumes no rev");
    assert!(
        !std::fs::read_to_string(dir.path().join("config/local.yaml"))
            .unwrap_or_default()
            .contains("focus_point"),
        "a refused save writes nothing"
    );

    let fetched = handle(
        &mut state,
        Request::new("3", "calibration.fetch", json!({"kind": "focus_point"})),
    );
    assert!(!fetched.ok, "fetch refuses it too");
    assert!(
        fetched
            .error
            .expect("a refusal")
            .message
            .contains("scroll_point"),
        "fetch names the replacement as well"
    );
}

#[test]
fn max_button_round_trips_and_uses_the_point_validation() {
    let (dir, mut state, frame_id) = harness(800, 600);

    let saved = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "max_button",
                "point": {"x": 700, "y": 500},
            }),
        ),
    );
    assert!(saved.ok, "save: {:?}", saved.error);
    let result = saved.result.expect("save carries a result");
    assert_eq!(result["kind"], "max_button");
    assert_eq!(result["mark"], json!([700, 500]));
    let local = dir.read("config/local.yaml");
    assert!(local.contains("max_button"), "{local}");
    let settings = seal_core::shop::settings(&state.config);
    assert_eq!(settings.max_button, Some(vec![700, 500]));

    let fetched = handle(
        &mut state,
        Request::new("3", "calibration.fetch", json!({"kind": "max_button"})),
    );
    assert!(fetched.ok, "fetch: {:?}", fetched.error);
    let result = fetched.result.expect("fetch carries a result");
    assert_eq!(result["mark"], json!([700, 500]));
    assert_eq!(result["artifact"], Value::Null);

    // The same point validation the other point kinds use: a non-integer coordinate...
    let bad = handle(
        &mut state,
        Request::new(
            "4",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "max_button",
                "point": {"x": "700", "y": 500},
            }),
        ),
    );
    assert!(!bad.ok);
    assert!(
        bad.error.expect("a refusal").message.contains("integer"),
        "the shared point message"
    );

    // ...and a point outside the captured frame.
    let outside = handle(
        &mut state,
        Request::new(
            "5",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "max_button",
                "point": {"x": 801, "y": 500},
            }),
        ),
    );
    assert!(!outside.ok);
    assert!(
        outside
            .error
            .expect("a refusal")
            .message
            .contains("does not fit"),
        "the shared frame check"
    );
}

#[test]
fn tuner_region_is_refused_with_its_withdrawal_reason() {
    let (dir, mut state, frame_id) = harness(800, 600);

    let response = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "tuner_region",
                "region": {"x": 0, "y": 0, "w": 100, "h": 100},
            }),
        ),
    );

    assert!(
        !response.ok,
        "a single rect must not claim to be a tuner region"
    );
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(error.message.contains("withdrawn"), "{}", error.message);
    assert!(error.message.contains("three-box"), "{}", error.message);
    assert!(error.message.contains("hand-edit"), "{}", error.message);
    assert_eq!(error.detail["kind"], "tuner_region");
    assert!(
        !std::fs::read_to_string(dir.path().join("config/local.yaml"))
            .unwrap_or_default()
            .contains("tuner_region"),
        "a refused save writes nothing"
    );
    assert_eq!(state.rev, 0, "a refused save consumes no rev");

    let fetched = handle(
        &mut state,
        Request::new("3", "calibration.fetch", json!({"kind": "tuner_region"})),
    );
    assert!(!fetched.ok);
    assert!(
        fetched
            .error
            .expect("a refusal")
            .message
            .contains("withdrawn"),
        "fetch names the reason too"
    );
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
                "empty": true,
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

#[test]
fn a_result_gem_save_without_the_empty_verdict_is_refused() {
    let (dir, mut state, frame_id) = harness(800, 600);
    let (data, _) = png_frame(2, 2);
    let frame_id = {
        let _ = frame_id;
        state.frames.insert(json!({
            "format": "png",
            "width": 2,
            "height": 2,
            "origin": {"x": 0, "y": 0},
            "data": data,
        }))
    };

    let response = handle(
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

    assert!(!response.ok, "the crop must not be written on a guess");
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("empty") && error.message.contains("result box"),
        "the line tells the player to say whether it is empty: {}",
        error.message
    );
    assert!(
        !dir.path().join("config/calib_gem_result.png").exists(),
        "a refused save writes no crop"
    );
    assert_eq!(state.rev, 0, "a refused save consumes no rev");
    assert!(state.config["gem"]["empty_signature"].is_null());
}

#[test]
fn a_wrongly_typed_empty_verdict_is_refused() {
    let (_dir, mut state, frame_id) = harness(800, 600);

    let response = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "result_gem_area",
                "region": {"x": 0, "y": 0, "w": 1, "h": 1},
                "empty": "yes",
            }),
        ),
    );

    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(error.message.contains("true or false"), "{}", error.message);
}

#[test]
fn an_empty_verdict_on_another_kind_is_refused_by_name() {
    let (_dir, mut state, frame_id) = harness(800, 600);

    let response = handle(
        &mut state,
        Request::new(
            "2",
            "calibration.save",
            json!({
                "frame_id": frame_id,
                "kind": "scroll_point",
                "point": {"x": 100, "y": 200},
                "empty": true,
            }),
        ),
    );

    assert!(!response.ok, "empty means nothing for a point mark");
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("empty") && error.message.contains("result_gem_area"),
        "{}",
        error.message
    );
    assert_eq!(state.rev, 0);
}

#[test]
fn empty_false_samples_the_signature_and_writes_no_crop() {
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
                "region": {"x": 0, "y": 0, "w": 2, "h": 2},
                "empty": false,
            }),
        ),
    );

    assert!(response.ok, "save: {:?}", response.error);
    let result = response.result.expect("save carries a result");
    assert_eq!(result["mark"], json!([0, 0, 2, 2]));
    assert_eq!(
        result["artifact"],
        Value::Null,
        "the busy box's crop must not become the empty reference"
    );
    assert!(
        !dir.path().join("config/calib_gem_result.png").exists(),
        "empty:false writes no crop"
    );

    let signature = &state.config["gem"]["empty_signature"];
    assert_eq!(
        signature["mean_v"],
        json!(1.0),
        "the sampled six-feature signature is in the merged config: {signature}"
    );
    assert_eq!(signature["colored_fraction"], json!(0.0));
    let local = dir.read("config/local.yaml");
    assert!(
        local.contains("empty_signature"),
        "the signature reached local.yaml: {local}"
    );
    assert!(
        state.config["gem"]["result_gem_area"] == json!([0, 0, 2, 2]),
        "the area is written even when the crop is not"
    );
}

// ── named-instance marks (`PROTOCOL.md` §12.8) ──────────────────────────────────────────────

/// A merged view + local file carrying four pet rows, three page tabs and every gem point, so a
/// targeted write can prove it preserved the instance's siblings and the array order.
fn targeted_state() -> (TestDir, CoreState, String) {
    let dir = root();
    let mut state = CoreState::new(dir.path().to_path_buf());
    handle(&mut state, hello_request());
    let local = json!({
        "pet": {
            "menu_button": [10, 10],
            "feed_icon": [20, 20],
            "close_button": [30, 30],
            "page_tabs": [[1, 1], [2, 2], [3, 3]],
            "bag_grid": [0, 0, 400, 400],
            "bag_slot": [0, 0, 40, 40],
            "slots": [
                {
                    "toggle_label": [100, 100, 20, 20],
                    "boarding_pet_slot": [200, 200, 30, 40],
                    "feeder_strip": [300, 300, 100, 20],
                    "stacks": 2,
                    "pet_slot_empty_png": "ROW1CROP",
                    "boarding_running": true
                },
                {
                    "toggle_label": [110, 110, 20, 20],
                    "boarding_pet_slot": [210, 210, 30, 40],
                    "feeder_strip": [310, 310, 100, 20],
                    "stacks": 5,
                    "pet_slot_empty_png": "ROW2CROP",
                    "boarding_running": false
                },
                {
                    "toggle_label": [120, 120, 20, 20],
                    "boarding_pet_slot": [220, 220, 30, 40],
                    "feeder_strip": [320, 320, 100, 20],
                    "stacks": 2,
                    "pet_slot_empty_png": "ROW3CROP",
                    "boarding_running": true
                },
                {
                    "toggle_label": [130, 130, 20, 20],
                    "boarding_pet_slot": [230, 230, 30, 40],
                    "stacks": 5,
                    "pet_slot_empty_png": "ROW4CROP",
                    "boarding_running": false
                }
            ]
        },
        "gem": {
            "grade_positions": {
                "N": [10, 10],
                "G": [20, 20],
                "DG": [30, 30],
                "Register": [40, 40],
                "Combine": [50, 50]
            },
            "resource_gems": [[60, 60], [70, 70], [80, 80]]
        }
    });
    dir.write(
        "config/local.yaml",
        &serde_json::to_string(&local).expect("the fixture serialises"),
    );
    state.config = local;
    let frame_id = state.frames.insert(json!({
        "format": "png",
        "width": 800,
        "height": 600,
        "origin": {"x": 0, "y": 0},
        "data": "",
    }));
    (dir, state, frame_id)
}

fn save(state: &mut CoreState, frame_id: &str, body: Value) -> Value {
    let mut body = body;
    body["frame_id"] = json!(frame_id);
    let response = handle(state, Request::new("save", "calibration.save", body));
    assert!(response.ok, "save failed: {:?}", response.error);
    response.result.expect("save carries a result")
}

fn save_error(state: &mut CoreState, frame_id: &str, body: Value) -> Error {
    let mut body = body;
    body["frame_id"] = json!(frame_id);
    let response = handle(state, Request::new("save", "calibration.save", body));
    assert!(!response.ok, "expected a refusal: {response:?}");
    response.error.expect("a refusal")
}

fn fetch(state: &mut CoreState, body: Value) -> Value {
    let response = handle(state, Request::new("fetch", "calibration.fetch", body));
    assert!(response.ok, "fetch failed: {:?}", response.error);
    response.result.expect("fetch carries a result")
}

fn fetch_error(state: &mut CoreState, body: Value) -> Error {
    let response = handle(state, Request::new("fetch", "calibration.fetch", body));
    assert!(!response.ok, "expected a fetch refusal: {response:?}");
    response.error.expect("a refusal")
}

#[test]
fn pet_point_and_bag_marks_round_trip_their_own_paths() {
    let (_dir, mut state, frame_id) = targeted_state();

    for (kind, point, path) in [
        ("pet_menu", [11, 12], "pet.menu_button"),
        ("pet_feed", [21, 22], "pet.feed_icon"),
        ("pet_close", [31, 32], "pet.close_button"),
    ] {
        let result = save(
            &mut state,
            &frame_id,
            json!({"kind": kind, "point": {"x": point[0], "y": point[1]}}),
        );
        assert_eq!(result["kind"], kind);
        assert_eq!(result["mark"], json!(point));
        assert!(result["target"].is_null(), "{kind} takes no target");
        let fetched = fetch(&mut state, json!({"kind": kind}));
        assert_eq!(fetched["mark"], json!(point));
        assert_eq!(fetched["target"], Value::Null);
        let local = state.store.load_local().expect("local reads");
        assert_eq!(
            path.split_once('.')
                .map(|(block, key)| local[block][key].clone())
                .expect("a two-part path"),
            json!(point),
            "the YAML path {path}"
        );
    }

    for (kind, rect, block_key) in [
        ("pet_bag_grid", [0, 0, 400, 400], "bag_grid"),
        ("pet_bag_slot", [0, 0, 40, 40], "bag_slot"),
    ] {
        let result = save(
            &mut state,
            &frame_id,
            json!({"kind": kind, "region": {"x": rect[0], "y": rect[1], "w": rect[2], "h": rect[3]}}),
        );
        assert_eq!(result["mark"], json!(rect));
        let fetched = fetch(&mut state, json!({"kind": kind}));
        assert_eq!(fetched["mark"], json!(rect));
        let local = state.store.load_local().expect("local reads");
        assert_eq!(
            local["pet"][block_key],
            json!(rect),
            "pet's own {block_key}"
        );
        assert!(
            local
                .get("buy_sell")
                .and_then(|b| b.get(block_key))
                .is_none(),
            "the pet bag is not the shop's bag"
        );
    }
}

#[test]
fn pet_page_tab_targets_are_one_based_names_and_refuse_unconfigured_tabs() {
    let (_dir, mut state, frame_id) = targeted_state();

    let result = save(
        &mut state,
        &frame_id,
        json!({"kind": "pet_page_tab", "target": "ITEM2", "point": {"x": 222, "y": 333}}),
    );
    assert_eq!(result["target"], "ITEM2");
    assert_eq!(result["mark"], json!([222, 333]));

    let local = state.store.load_local().expect("local reads");
    assert_eq!(local["pet"]["page_tabs"][0], json!([1, 1]));
    assert_eq!(local["pet"]["page_tabs"][1], json!([222, 333]));
    assert_eq!(local["pet"]["page_tabs"][2], json!([3, 3]));
    assert_eq!(state.config["pet"]["page_tabs"][1], json!([222, 333]));

    let fetched = fetch(
        &mut state,
        json!({"kind": "pet_page_tab", "target": "ITEM2"}),
    );
    assert_eq!(fetched["target"], "ITEM2");
    assert_eq!(fetched["mark"], json!([222, 333]));
    state.config["pet"]["page_tabs"] = json!([[1, 1], [2, 2]]);
    let error = save_error(
        &mut state,
        &frame_id,
        json!({"kind": "pet_page_tab", "target": "ITEM3", "point": {"x": 1, "y": 1}}),
    );
    assert!(
        error.message.contains("page tab ITEM3 isn't configured"),
        "{}",
        error.message
    );

    let error = save_error(
        &mut state,
        &frame_id,
        json!({"kind": "pet_page_tab", "point": {"x": 1, "y": 1}}),
    );
    assert!(error.message.contains("target") && error.message.contains("pet_page_tab"));
    let error = save_error(
        &mut state,
        &frame_id,
        json!({"kind": "pet_page_tab", "target": "ITEM4", "point": {"x": 1, "y": 1}}),
    );
    assert!(
        error.message.contains("ITEM1, ITEM2, ITEM3"),
        "{}",
        error.message
    );
    let error = save_error(
        &mut state,
        &frame_id,
        json!({"kind": "pet_page_tab", "target": "TAB1", "point": {"x": 1, "y": 1}}),
    );
    assert!(
        error.message.contains("ITEM1, ITEM2, ITEM3"),
        "an unknown target names the allowed values: {}",
        error.message
    );
}

#[test]
fn pet_row_marks_write_their_own_key_and_preserve_the_row() {
    let (_dir, mut state, frame_id) = targeted_state();

    // Three different marks on the same row, each a different key.
    let toggle = save(
        &mut state,
        &frame_id,
        json!({"kind": "pet_toggle", "target": "2", "region": {"x": 111, "y": 112, "w": 21, "h": 22}}),
    );
    assert_eq!(toggle["target"], "2");
    assert_eq!(toggle["mark"], json!([111, 112, 21, 22]));
    let boarding = save(
        &mut state,
        &frame_id,
        json!({"kind": "pet_boarding_slot", "target": "2", "region": {"x": 211, "y": 212, "w": 31, "h": 42}}),
    );
    let strip = save(
        &mut state,
        &frame_id,
        json!({"kind": "pet_feeder_strip", "target": "2", "region": {"x": 311, "y": 312, "w": 101, "h": 22}}),
    );
    assert_eq!(boarding["target"], "2");
    assert_eq!(strip["target"], "2");

    let local = state.store.load_local().expect("local reads");
    let row = &local["pet"]["slots"][1];
    assert_eq!(row["toggle_label"], json!([111, 112, 21, 22]));
    assert_eq!(row["boarding_pet_slot"], json!([211, 212, 31, 42]));
    assert_eq!(row["feeder_strip"], json!([311, 312, 101, 22]));
    assert_eq!(row["stacks"], 5, "the row's stacks survive");
    assert_eq!(
        row["pet_slot_empty_png"], "ROW2CROP",
        "the row's crop survives"
    );
    assert_eq!(
        row["boarding_running"], false,
        "the row's run state survives"
    );
    assert_eq!(
        local["pet"]["slots"].as_array().map(Vec::len),
        Some(4),
        "the array length and order are untouched"
    );
    // The rows before and after are exactly the fixture's.
    assert_eq!(local["pet"]["slots"][0]["stacks"], 2);
    assert_eq!(local["pet"]["slots"][2]["stacks"], 2);
    assert_eq!(local["pet"]["slots"][3]["stacks"], 5);
    assert_eq!(state.config["pet"]["slots"][1]["stacks"], 5);
    assert_eq!(
        state.config["pet"]["slots"][1]["pet_slot_empty_png"],
        "ROW2CROP"
    );

    // Every mark fetches back by its target, and the untouched row 4 is null.
    for (kind, mark) in [
        ("pet_toggle", json!([111, 112, 21, 22])),
        ("pet_boarding_slot", json!([211, 212, 31, 42])),
        ("pet_feeder_strip", json!([311, 312, 101, 22])),
    ] {
        let fetched = fetch(&mut state, json!({"kind": kind, "target": "2"}));
        assert_eq!(fetched["mark"], mark, "{kind}");
        assert_eq!(fetched["target"], "2");
    }
    let untouched = fetch(
        &mut state,
        json!({"kind": "pet_feeder_strip", "target": "4"}),
    );
    assert_eq!(untouched["mark"], Value::Null, "not calibrated yet");

    state.config["pet"]["slots"]
        .as_array_mut()
        .expect("slots")
        .truncate(3);
    let error = save_error(
        &mut state,
        &frame_id,
        json!({"kind": "pet_toggle", "target": "4", "region": {"x": 1, "y": 1, "w": 2, "h": 2}}),
    );
    assert!(
        error.message.contains("pet row 4 isn't configured"),
        "{}",
        error.message
    );
    let error = save_error(
        &mut state,
        &frame_id,
        json!({"kind": "pet_toggle", "target": "0", "region": {"x": 1, "y": 1, "w": 2, "h": 2}}),
    );
    assert!(
        error.message.contains("\"1\", \"2\", \"3\", \"4\""),
        "an unknown row names the allowed values: {}",
        error.message
    );
}

#[test]
fn gem_grade_and_resource_targets_map_names_and_numbers_to_the_arrays() {
    let (_dir, mut state, frame_id) = targeted_state();

    let grade = save(
        &mut state,
        &frame_id,
        json!({"kind": "gem_grade", "target": "DG", "point": {"x": 33, "y": 44}}),
    );
    assert_eq!(grade["target"], "DG");
    assert_eq!(grade["mark"], json!([33, 44]));
    let resource = save(
        &mut state,
        &frame_id,
        json!({"kind": "gem_resource", "target": "2", "point": {"x": 77, "y": 88}}),
    );
    assert_eq!(resource["target"], "2");

    let local = state.store.load_local().expect("local reads");
    assert_eq!(local["gem"]["grade_positions"]["DG"], json!([33, 44]));
    assert_eq!(local["gem"]["grade_positions"]["N"], json!([10, 10]));
    assert_eq!(local["gem"]["resource_gems"][0], json!([60, 60]));
    assert_eq!(local["gem"]["resource_gems"][1], json!([77, 88]));
    assert_eq!(local["gem"]["resource_gems"][2], json!([80, 80]));
    assert_eq!(
        state.config["gem"]["grade_positions"]["DG"],
        json!([33, 44])
    );

    for (kind, target) in [("gem_grade", "DG"), ("gem_resource", "2")] {
        let fetched = fetch(&mut state, json!({"kind": kind, "target": target}));
        assert_eq!(fetched["target"], target);
        assert!(fetched["mark"].is_array(), "{kind}");
    }

    let error = save_error(
        &mut state,
        &frame_id,
        json!({"kind": "gem_grade", "target": "SG", "point": {"x": 1, "y": 1}}),
    );
    assert!(
        error.message.contains("N, G, DG, Register, Combine"),
        "{}",
        error.message
    );
    let error = save_error(
        &mut state,
        &frame_id,
        json!({"kind": "gem_resource", "target": "4", "point": {"x": 1, "y": 1}}),
    );
    assert!(
        error.message.contains("\"1\", \"2\", \"3\""),
        "{}",
        error.message
    );
    state.config["gem"]["resource_gems"] = json!([[60, 60], [70, 70]]);
    let error = save_error(
        &mut state,
        &frame_id,
        json!({"kind": "gem_resource", "target": "3", "point": {"x": 1, "y": 1}}),
    );
    assert!(
        error.message.contains("gem resource 3 isn't configured"),
        "{}",
        error.message
    );
}

#[test]
fn the_target_field_rules_are_pinned() {
    let (_dir, mut state, frame_id) = targeted_state();

    // A targeted kind refuses a save/fetch without one, naming the field and the kind.
    for error in [
        save_error(
            &mut state,
            &frame_id,
            json!({"kind": "pet_toggle", "region": {"x": 1, "y": 1, "w": 2, "h": 2}}),
        ),
        fetch_error(&mut state, json!({"kind": "gem_grade"})),
    ] {
        assert!(
            error.message.contains("target") && error.message.contains("\"target\""),
            "{}",
            error.message
        );
    }

    // A non-targeted kind refuses a target, naming the field.
    for error in [
        save_error(
            &mut state,
            &frame_id,
            json!({"kind": "scroll_point", "point": {"x": 1, "y": 2}, "target": "1"}),
        ),
        fetch_error(&mut state, json!({"kind": "bag_grid", "target": "1"})),
    ] {
        assert!(
            error.message.contains("target") && error.message.contains("not accepted"),
            "{}",
            error.message
        );
    }

    // A target that is the wrong JSON type is refused as a string field.
    let error = save_error(
        &mut state,
        &frame_id,
        json!({"kind": "pet_toggle", "target": 2, "region": {"x": 1, "y": 1, "w": 2, "h": 2}}),
    );
    assert!(
        error.message.contains("must be a string"),
        "{}",
        error.message
    );

    // The supported list carries the new kinds.
    let error = save_error(&mut state, &frame_id, json!({"kind": "nope"}));
    let kinds = error.detail["kinds"].as_array().expect("the list travels");
    for kind in [
        "pet_menu",
        "pet_page_tab",
        "pet_toggle",
        "gem_grade",
        "gem_resource",
    ] {
        assert!(kinds.iter().any(|value| value == kind), "missing {kind}");
    }
}

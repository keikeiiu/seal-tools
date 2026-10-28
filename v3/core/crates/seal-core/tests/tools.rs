//! The tool surface over the real stdio protocol.
//!
//! The production registry is an explicit placeholder until the tools and their effects are ported,
//! so these tests pin the protocol-visible refusals and the shutdown ordering; the running-run
//! events are tested in process (`tests/supervisor.rs`) with injected fake tools. The e2e file also
//! proves responses and events are separate whole lines — `shutdown` answers first and `core.stopping`
//! is the next line.

mod common;

use common::{Session, config_root};
use serde_json::json;

#[test]
fn an_unknown_tool_id_is_a_bad_request() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let response = session.call("tool.start", json!({"tool": "hover"}));

    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("hover"),
        "the line names the id: {}",
        error.message
    );
    assert_eq!(error.detail["tool"], "hover");
}

#[test]
fn a_tool_start_without_an_id_is_a_bad_request() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let response = session.call("tool.start", json!({}));

    assert!(!response.ok);
    assert_eq!(response.error.expect("a refusal").code, "bad_request");
}

#[test]
fn stopping_when_nothing_runs_is_not_running_not_a_failure() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let response = session.call("tool.stop", json!({}));

    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "not_running");
    assert_eq!(error.message, "nothing is running");
}

#[test]
fn a_tool_without_its_calibration_is_refused_by_its_own_plan() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    // Every id now has a loop; a start is refused by the tool's own plan, not by a placeholder. The
    // pet feeder is the resident reader and its `ready` rule wants the 目錄 marks, which this fixture
    // (deliberately minimal) does not carry — the domain's line, not a "not ported" one.
    let response = session.call("tool.start", json!({"tool": "pet"}));

    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("Calibrate tab"),
        "the domain's calibration line: {}",
        error.message
    );
    assert!(
        !error.message.contains("not available in this build yet"),
        "there is no unported tool left: {}",
        error.message
    );
}

#[test]
fn shutdown_answers_first_then_stops_and_exits_cleanly() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let response = session.call("shutdown", json!({}));
    assert!(
        response.ok,
        "shutdown is acknowledged before the process stops"
    );

    // The next line is the event, whole and separate from the response above.
    let event = session.event();
    assert_eq!(event["event"], "core.stopping");
    assert_eq!(event["v"], 1);
    assert_eq!(event["data"]["reason"], "shutdown");
}

#[test]
fn eof_also_stops_the_core_with_the_stopping_event() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    // Closing stdin is the documented shutdown; Drop asserts the exit status. The core still says
    // why it is leaving, with EOF as the reason.
    session.close_stdin();
    let event = session.event();
    assert_eq!(event["event"], "core.stopping");
    assert_eq!(event["data"]["reason"], "eof");
}
#[test]
fn run_arguments_on_a_tool_that_does_not_take_them_are_a_bad_request_naming_the_field() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    // Deterministic and hardware-free: rejected before the supervisor is touched.
    let response = session.call("tool.start", json!({"tool": "spammer", "preset": "potion"}));
    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("preset"),
        "the line names the field: {}",
        error.message
    );
    assert_eq!(error.detail["field"], "preset");

    let response = session.call("tool.start", json!({"tool": "tuner", "count": 3}));
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("count"),
        "the line names the field: {}",
        error.message
    );
    assert_eq!(error.detail["field"], "count");
}

#[test]
fn a_buy_count_below_one_is_refused_not_clamped() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let response = session.call(
        "tool.start",
        json!({"tool": "buy", "preset": "potion", "count": 0}),
    );

    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("count") && error.message.contains("at least 1"),
        "the refusal says what is wrong: {}",
        error.message
    );
    assert_eq!(error.detail["field"], "count");
}

#[test]
fn an_unknown_tool_start_field_is_a_bad_request_naming_it() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let response = session.call(
        "tool.start",
        json!({"tool": "buy", "preset": "potion", "count": 1, "gift": true}),
    );

    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("gift"),
        "the line names the unknown field: {}",
        error.message
    );
    assert_eq!(error.detail["field"], "gift");
}

#[test]
fn a_buy_start_without_a_preset_reaches_the_domain_line() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    // The start itself is accepted (buy is served; its plan is built in `create`), and the missing
    // preset is the domain's own line in the run's finish event — deterministic, before any board
    // or window is opened.
    let response = session.call("tool.start", json!({"tool": "buy"}));
    assert!(response.ok, "the start is accepted: {:?}", response.error);

    let mut finished = None;
    for _ in 0..10 {
        let event = session.event();
        assert_ne!(event["event"], "core.stopping");
        if event["event"] == "tool.finished" {
            finished = Some(event);
            break;
        }
    }
    let finished = finished.expect("the run finishes with the refusal");
    assert_eq!(finished["data"]["tool"], "buy");
    assert_eq!(finished["data"]["reason"], "error");
    assert_eq!(finished["data"]["message"], "No buy preset selected.");
}
#[test]
fn a_wrongly_typed_run_argument_is_a_bad_request_naming_the_field() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let response = session.call(
        "tool.start",
        json!({"tool": "buy", "preset": "potion", "count": "3"}),
    );
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(
        error.message.contains("count"),
        "the line names the field: {}",
        error.message
    );
    assert_eq!(error.detail["field"], "count");

    let response = session.call("tool.start", json!({"tool": "buy", "preset": 7}));
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert_eq!(error.detail["field"], "preset");
    assert!(
        error.message.contains("preset"),
        "the line names the field: {}",
        error.message
    );
}

#[test]
fn sell_slots_are_validated_at_the_dispatch_before_the_supervisor() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    for (case, args) in [
        ("wrong tool", json!({"tool": "spammer", "slots": [1]})),
        ("buy is not sell", json!({"tool": "buy", "slots": [1]})),
        ("non-array", json!({"tool": "sell", "slots": "1"})),
        ("non-integer", json!({"tool": "sell", "slots": [1, "2"]})),
        ("float", json!({"tool": "sell", "slots": [1.5]})),
        ("out of range", json!({"tool": "sell", "slots": [64]})),
        ("negative", json!({"tool": "sell", "slots": [-1]})),
        ("empty", json!({"tool": "sell", "slots": []})),
    ] {
        let response = session.call("tool.start", args);
        assert!(!response.ok, "{case}: the start must be refused");
        let error = response.error.expect("a refusal");
        assert_eq!(error.code, "bad_request", "{case}");
        assert_eq!(error.detail["field"], "slots", "{case}");
        assert!(
            error.message.contains("slots"),
            "{case}: the line names the field: {}",
            error.message
        );
    }
}

#[test]
fn a_sell_start_without_slots_reaches_the_domains_selection_line() {
    let root = config_root();
    // Every shop mark is calibrated, but the migrated config has no selection and the start carries
    // no slots: the start is accepted (the per-run selection is run data) and the domain's own
    // line arrives as the run's finish error, not as a bad_request from the dispatcher.
    root.write(
        "config/local.yaml",
        "buy_sell:\n  bag_grid: [0, 0, 800, 600]\n  bag_slot: [0, 0, 100, 75]\n  shop_rows: 5\n  shop_region: [900, 400, 300, 250]\n  scroll_point: [1000, 420]\n  max_button: [1200, 700]\n  sell_slots: [3, 1]\n  sell_cap: 5\n",
    );
    let before = root.read("config/local.yaml");
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let response = session.call("tool.start", json!({"tool": "sell"}));
    assert!(response.ok, "the start is accepted: {:?}", response.error);

    let mut finished = None;
    for _ in 0..20 {
        let event = session.event();
        assert_ne!(event["event"], "core.stopping");
        if event["event"] == "tool.finished" {
            finished = Some(event);
            break;
        }
    }
    let finished = finished.expect("the run finishes with the domain line");
    assert_eq!(finished["data"]["tool"], "sell");
    assert_eq!(finished["data"]["reason"], "error");
    assert_eq!(
        finished["data"]["message"],
        "No slots are selected to sell."
    );
    assert_eq!(
        root.read("config/local.yaml"),
        before,
        "a tool start never writes the config"
    );
}

#[test]
fn an_over_cap_sell_selection_finishes_with_the_domain_refusal() {
    let root = config_root();
    root.write(
        "config/local.yaml",
        "buy_sell:\n  bag_grid: [0, 0, 800, 600]\n  bag_slot: [0, 0, 100, 75]\n  shop_rows: 5\n  shop_region: [900, 400, 300, 250]\n  scroll_point: [1000, 420]\n  max_button: [1200, 700]\n  sell_cap: 2\n",
    );
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    // Three distinct cells after dedupe, cap 2: the start validates (shape/range are fine), and the
    // domain's over-cap line arrives as the run's finish error before any click.
    let response = session.call("tool.start", json!({"tool": "sell", "slots": [1, 1, 2, 3]}));
    assert!(response.ok, "the start is accepted: {:?}", response.error);

    let mut finished = None;
    for _ in 0..20 {
        let event = session.event();
        assert_ne!(event["event"], "core.stopping");
        if event["event"] == "tool.finished" {
            finished = Some(event);
            break;
        }
    }
    let finished = finished.expect("the run finishes with the domain line");
    assert_eq!(finished["data"]["tool"], "sell");
    assert_eq!(finished["data"]["reason"], "error");
    assert_eq!(
        finished["data"]["message"],
        "3 slots selected but the per-run cap is 2. Raise the cap on purpose, or narrow the selection."
    );
}

#[test]
fn pointer_arguments_are_validated_before_any_window_or_board_work() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    for (cmd, args, field) in [
        ("pointer.place", json!({}), "x"),
        ("pointer.place", json!({"x": 10}), "y"),
        ("pointer.click", json!({"x": -1, "y": 20}), "x"),
        ("pointer.click", json!({"x": 10, "y": -1}), "y"),
        ("pointer.place", json!({"x": 1.5, "y": 20}), "x"),
        ("pointer.click", json!({"x": 10, "y": "20"}), "y"),
    ] {
        let response = session.call(cmd, args);
        assert!(!response.ok, "{cmd} must be refused");
        let error = response.error.expect("a refusal");
        assert_eq!(error.code, "bad_request", "{cmd}");
        assert_eq!(error.detail["field"], field, "{cmd}");
    }
}

#[test]
fn pointer_with_a_missing_game_window_is_window_not_found() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    for cmd in ["pointer.place", "pointer.click"] {
        let response = session.call(cmd, json!({"x": 10, "y": 20}));
        assert!(!response.ok, "{cmd} needs the game window");
        let error = response.error.expect("a refusal");
        assert_eq!(error.code, "window_not_found", "{cmd}: {}", error.message);
    }
}

#[test]
fn board_details_is_a_read_only_shape_without_opening_a_port() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    // A missing board is a normal choice state; details never opens the serial port, so the minimal
    // fixture root (no board) can answer it without any hardware or board_unavailable refusal.
    let result = session.result("board.details", json!({}));
    assert!(result["baud"].is_number());
    assert!(result["expected"]["vid"].is_string());
    assert!(result["expected"]["pids"].is_array());
    assert!(result["expected"]["port"].is_null());
    assert!(result["ports"].is_array());
    let state = result["choice"]["state"].as_str().expect("a choice state");
    assert!(
        matches!(state, "ready" | "named_missing" | "not_found"),
        "unexpected choice state: {state}"
    );
    assert!(result["choice"]["port"].is_null() || result["choice"]["port"].is_string());
    assert!(result["choice"]["refusal"].is_null() || result["choice"]["refusal"].is_string());
    assert!(
        result["firmware"].is_null(),
        "not checked yet on a fresh session"
    );
}

const PET_LOCAL: &str = "\
foreign:
  keep: me
pet:
  menu_button: [1, 1]
  feed_icon: [2, 2]
  close_button: [3, 3]
  bag_grid: [0, 0, 400, 400]
  bag_slot: [0, 0, 40, 40]
  page_tabs: [[100, 100]]
  food_slots: [[0, 0], [0, 1], [0, 2], [0, 3]]
  food_slots_used: 0
  food_load_mode: right_click
  return_slot: [0, 5]
  queue: []
  max_button: [900, 500]
  items_per_minute: 3
  wait_after_empty_minutes: 5
  slots:
    - enabled: true
      toggle_label: [10, 20, 100, 30]
      boarding_pet_slot: [10, 60, 100, 80]
      feeder_strip: [110, 20, 200, 80]
      stacks: 2
      boarding_running: false
      pet_slot_empty_png: iVBORw0KGgo=
";

#[test]
fn pet_preflight_answers_the_two_checklists_read_only() {
    let root = config_root();
    root.write("config/local.yaml", PET_LOCAL);
    let before = root.read("config/local.yaml");
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let result = session.result("pet.preflight", json!({}));

    assert!(result["ready"].is_boolean());
    assert!(result["calibration"].is_array());
    assert!(result["session"].is_array());
    assert!(result["lines"].is_array());
    assert!(
        result["calibration"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["label"] == "Boarding bag grid" && item["ok"] == true)
    );
    assert!(
        result["session"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["label"] == "food cells")
    );
    assert_eq!(
        root.read("config/local.yaml"),
        before,
        "preflight writes nothing"
    );
}

#[test]
fn pet_set_boarding_writes_one_row_field_keeps_the_file_and_emits_one_event() {
    let root = config_root();
    root.write("config/local.yaml", PET_LOCAL);
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let (response, before) =
        session.call_note_order("pet.set_boarding", json!({"row": 1, "running": true}));
    assert!(before.is_empty(), "the event must NOT precede the response");
    assert!(response.ok, "the tick succeeds: {:?}", response.error);
    let result = response.result.expect("a result");

    assert_eq!(result["rev"], 1, "the A1 writer bumps the shared revision");
    assert_eq!(result["block"]["slots"][0]["boarding_running"], true);
    assert_eq!(result["block"]["return_slot"], json!([0, 5]));

    let local = root.read("config/local.yaml");
    assert!(local.contains("boarding_running: true"), "{local}");
    assert!(
        local.contains("keep: me"),
        "foreign block preserved: {local}"
    );
    assert!(
        local.contains("return_slot:"),
        "rest of pet preserved: {local}"
    );

    let event = session.event();
    assert_eq!(event["event"], "config.changed");
    assert_eq!(event["data"]["rev"], 1);
    assert_eq!(event["data"]["block"], "pet");
    assert_eq!(
        event["data"]["values"]["slots"][0]["boarding_running"],
        true
    );

    // Exactly one: the command after the event must observe none queued before its response.
    let (next, extra) = session.call_note_order("pet.preflight", json!({}));
    assert!(next.ok, "the session stays usable: {:?}", next.error);
    assert!(
        extra.is_empty(),
        "exactly one config.changed for one boarding tick: {extra:?}"
    );
    let (after, extra_after) = session.call_note_order("pet.preflight", json!({}));
    assert!(after.ok);
    assert!(
        extra_after.is_empty(),
        "no second event is observable: {extra_after:?}"
    );
}

#[test]
fn pet_set_boarding_refuses_a_bad_row_or_type_with_the_field_named() {
    let root = config_root();
    root.write("config/local.yaml", PET_LOCAL);
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let out_of_range = session.call("pet.set_boarding", json!({"row": 2, "running": true}));
    assert!(!out_of_range.ok);
    let error = out_of_range.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert_eq!(error.detail["field"], "row");
    assert!(error.message.contains("out of range"), "{}", error.message);

    let not_bool = session.call("pet.set_boarding", json!({"row": 1, "running": "yes"}));
    assert!(!not_bool.ok);
    let error = not_bool.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert_eq!(error.detail["field"], "running");
}

#[cfg(windows)]
#[test]
fn a_blocked_pet_set_boarding_write_is_io_logged_and_leaves_the_file_alone() {
    use std::os::windows::fs::OpenOptionsExt;

    let root = config_root();
    root.write("config/local.yaml", PET_LOCAL);
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));
    let before = root.read("config/local.yaml");

    // Hold local.yaml with no sharing at all: the core's atomic temp+rename must fail, exercising
    // the same store path a real locked/blocked file hits. (The core already read it at startup.)
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(root.path_of("config/local.yaml"))
        .expect("the test holds local.yaml");

    let (response, before_response) =
        session.call_note_order("pet.set_boarding", json!({"row": 1, "running": true}));
    assert!(
        before_response.is_empty(),
        "a failed write must not emit an event before its response: {before_response:?}"
    );
    assert!(!response.ok, "the write must fail while the file is locked");
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "io");
    assert!(
        error.message.contains("cannot read") || error.message.contains("could not"),
        "the core line explains the I/O failure: {}",
        error.message
    );

    drop(held);
    assert_eq!(
        root.read("config/local.yaml"),
        before,
        "the file is untouched"
    );
    // The dispatcher is still alive and answers normally after the failed write; no config.changed
    // is observable before that next response either (the sink returns before it can queue one).
    let (preflight, extra) = session.call_note_order("pet.preflight", json!({}));
    assert!(
        preflight.ok,
        "the session stays usable: {:?}",
        preflight.error
    );
    assert!(
        extra.is_empty(),
        "the I/O failure emits no config.changed: {extra:?}"
    );
    let (again, extra_again) = session.call_note_order("pet.preflight", json!({}));
    assert!(again.ok);
    assert!(
        extra_again.is_empty(),
        "no late event after the failure: {extra_again:?}"
    );
}

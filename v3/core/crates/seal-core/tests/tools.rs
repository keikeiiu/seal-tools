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
        "buy_sell:\n  bag_grid: [0, 0, 800, 600]\n  bag_slot: [0, 0, 100, 75]\n  shop_rows: 5\n  shop_region: [900, 400, 300, 250]\n  scroll_point: [1000, 420]\n  max_button: [1200, 700]\n  sell_cap: 5\n",
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

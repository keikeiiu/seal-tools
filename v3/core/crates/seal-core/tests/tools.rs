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
        error.message.contains("Calibrate Pet"),
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

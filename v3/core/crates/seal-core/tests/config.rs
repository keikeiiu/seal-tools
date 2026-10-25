//! The config commands over the real stdio protocol: `hello` carries the merged config and the
//! session revision; `config.apply`/`config.reload`/`ui.window` write through the seal-config store
//! (`PROTOCOL.md` §4 and §7).
//!
//! Every test spawns the real binary against a temporary root and speaks one JSON line per message,
//! which is the same transport the Flutter shell will use. The fixtures are written by the test —
//! never copied from a player's `local.yaml`.

mod common;

use common::{Session, config_root};
use serde_json::json;

#[test]
fn hello_carries_the_merged_config_and_the_session_revision() {
    let root = config_root();
    let mut session = Session::start(root.path());

    let result = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    assert_eq!(result["protocol"], 1);
    assert_eq!(
        result["rev"], 0,
        "a session revision starts at 0 for this core process"
    );
    assert_eq!(
        result["root"].as_str(),
        Some(root.path().to_string_lossy().as_ref()),
        "the shell is told which root the core bound to"
    );
    // The base and the machine overlay are merged: the shell mirrors this, it does not read files.
    assert_eq!(result["config"]["window"]["title"], "TEST_TITLE");
    assert_eq!(result["config"]["tuner"]["guard_px"], 8);
}

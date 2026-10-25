//! The config commands over the real stdio protocol: `hello` carries the merged config and the
//! session revision; `config.apply`/`config.reload`/`config.validate`/`ui.window` work through the
//! seal-config store (`PROTOCOL.md` §4 and §7).
//!
//! Every test spawns the real binary against a temporary root and speaks one JSON line per message,
//! which is the same transport the Flutter shell will use. The fixtures are written by the test —
//! never copied from a player's `local.yaml`.

mod common;

use common::{Session, config_root};
use seal_config::LOCAL_HEADER;
use serde_json::{Value, json};

/// Parses a file the core wrote, so an assertion is about the store's output and not about string
/// matching.
fn yaml_file(root: &common::TestDir, name: &str) -> Value {
    seal_config::yaml::parse(&root.read(name)).expect("the core writes valid YAML")
}

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

#[test]
fn apply_merges_only_the_given_blocks_and_answers_with_them() {
    let root = config_root();
    let defaults_before = root.read_bytes("config/defaults.yaml");
    let mut session = Session::start(root.path());
    let hello = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));
    let rev = hello["rev"].as_u64().expect("hello carries the rev");

    let result = session.result(
        "config.apply",
        json!({
            "rev": rev,
            "sections": {"quest": {"active": "run1", "guard_px": 12}},
        }),
    );

    // The answer is the merged sections, not the whole config — the shell only owns a few blocks.
    assert_eq!(result["rev"], rev + 1);
    assert_eq!(result["sections"]["quest"]["active"], "run1");
    assert_eq!(result["sections"]["quest"]["guard_px"], 12);
    assert!(
        result["sections"].get("tuner").is_none(),
        "a block the shell did not send is not echoed back"
    );

    // The write went to the player's file, under the constant header, and the shipped template was
    // not touched.
    let written = root.read("config/local.yaml");
    assert!(
        written.starts_with(LOCAL_HEADER),
        "a save writes the header"
    );
    let local = yaml_file(&root, "config/local.yaml");
    assert_eq!(local["quest"]["active"], "run1");
    assert_eq!(local["quest"]["guard_px"], 12);
    // The machine block that was already there is untouched.
    assert_eq!(local["tuner"]["guard_px"], 8);
    assert_eq!(root.read_bytes("config/defaults.yaml"), defaults_before);
}

#[test]
fn apply_merges_field_wise_so_a_half_written_block_survives() {
    // The pet feeder persists run state mid-run; a tab save must not delete it. The UI sends only
    // the static half it owns, and the core merges field-wise into the current block.
    let root = config_root();
    root.append(
        "config/local.yaml",
        "pet:\n  boarding_window_may_be_open: true\n  close_button_png: close.png\n",
    );
    let mut session = Session::start(root.path());
    let hello = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let result = session.result(
        "config.apply",
        json!({
            "rev": hello["rev"],
            "sections": {"pet": {"food_load_mode": "drag"}},
        }),
    );

    let pet = &result["sections"]["pet"];
    assert_eq!(pet["food_load_mode"], "drag");
    assert_eq!(pet["boarding_window_may_be_open"], true);
    assert_eq!(pet["close_button_png"], "close.png");

    let local = yaml_file(&root, "config/local.yaml");
    assert_eq!(local["pet"]["boarding_window_may_be_open"], true);
    assert_eq!(local["pet"]["close_button_png"], "close.png");
    assert_eq!(local["pet"]["food_load_mode"], "drag");
}

#[test]
fn a_stale_revision_is_refused_as_a_conflict_with_the_current_one() {
    let root = config_root();
    let mut session = Session::start(root.path());
    let hello = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));
    let first_rev = hello["rev"].clone();
    session.result(
        "config.apply",
        json!({"rev": first_rev, "sections": {"quest": {"active": "first"}}}),
    );
    let after_first = root.read_bytes("config/local.yaml");

    // The shell saved once, the core bumped the rev; the same old rev must not be replayed blindly.
    let response = session.call(
        "config.apply",
        json!({"rev": first_rev, "sections": {"quest": {"active": "second"}}}),
    );

    assert!(!response.ok);
    let error = response.error.expect("a refusal carries an error");
    assert_eq!(error.code, "conflict");
    assert_eq!(
        error.detail["rev"], 1,
        "the detail carries the current session revision"
    );
    // Refused means not written: the first save's file is intact.
    assert_eq!(root.read_bytes("config/local.yaml"), after_first);
}

#[test]
fn reload_sees_a_hand_edit_and_bumps_the_revision() {
    let root = config_root();
    let mut session = Session::start(root.path());
    let hello = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));
    assert_eq!(hello["config"]["quest"]["active"], "base");

    // The player edits the file by hand while the core runs.
    root.write(
        "config/local.yaml",
        "tuner:\n  guard_px: 8\nquest:\n  active: handedit\n",
    );

    let result = session.result("config.reload", json!({}));

    assert_eq!(result["rev"], 1, "a reload is a change the shell must see");
    assert_eq!(result["config"]["quest"]["active"], "handedit");
    assert_eq!(
        result["config"]["tuner"]["guard_px"], 8,
        "reload is the same three-layer merge, not a raw file read"
    );
}

#[test]
fn ui_window_persists_the_window_block_into_local_yaml() {
    let root = config_root();
    let mut session = Session::start(root.path());
    let hello = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let result = session.result(
        "ui.window",
        json!({"left": 100, "top": 50, "width": 900, "height": 700, "pinned": true}),
    );
    assert_eq!(result, json!({}), "ui.window answers an empty result");

    // The protocol's `height` is v2's `expanded_height`: the ui block keeps the shape a v2
    // local.yaml already has, which the domain model mirrors.
    let local = yaml_file(&root, "config/local.yaml");
    assert_eq!(local["ui"]["left"], 100);
    assert_eq!(local["ui"]["top"], 50);
    assert_eq!(local["ui"]["width"], 900);
    assert_eq!(local["ui"]["expanded_height"], 700);
    assert_eq!(local["ui"]["pinned"], true);
    assert!(
        local["ui"].get("height").is_none(),
        "the stored block uses the v2 field name"
    );
    // The existing machine block is still there.
    assert_eq!(local["tuner"]["guard_px"], 8);

    // A write is a revision change: the shell's pre-write rev is now stale.
    let conflict = session.call(
        "config.apply",
        json!({"rev": hello["rev"], "sections": {"quest": {"active": "x"}}}),
    );
    assert!(!conflict.ok);
    assert_eq!(conflict.error.expect("a refusal").code, "conflict");

    // ...and the bumped rev can be read back through reload.
    let reloaded = session.result("config.reload", json!({}));
    assert_eq!(reloaded["rev"], 2);
}

#[test]
fn a_non_owned_section_is_refused_by_apply_and_validate() {
    let root = config_root();
    let before = root.read_bytes("config/local.yaml");
    let mut session = Session::start(root.path());
    let hello = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let apply = session.call(
        "config.apply",
        json!({"rev": hello["rev"], "sections": {"my_notes": {"monitor": "left"}}}),
    );
    assert!(!apply.ok);
    assert_eq!(apply.error.expect("a refusal").code, "bad_request");

    let validate = session.call(
        "config.validate",
        json!({"sections": {"my_notes": {"monitor": "left"}}}),
    );
    assert!(!validate.ok);
    assert_eq!(validate.error.expect("a refusal").code, "bad_request");

    // Nothing was written by either.
    assert_eq!(root.read_bytes("config/local.yaml"), before);
}

#[test]
fn validate_accepts_a_clean_owned_section() {
    // A quest block with no sequence entries has nothing for the Save gate to refuse; the empty
    // arrays are the protocol's "clean" answer, not a claim that no rules ran.
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let result = session.result(
        "config.validate",
        json!({"sections": {"quest": {"active": "run1"}}}),
    );

    assert_eq!(result["errors"], json!([]));
    assert_eq!(result["warnings"], json!([]));
}

#[test]
fn apply_and_validate_refuse_malformed_arguments() {
    let root = config_root();
    let mut session = Session::start(root.path());
    let hello = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));
    let rev = hello["rev"].clone();

    // No sections at all.
    let response = session.call("config.apply", json!({"rev": rev}));
    assert_eq!(response.error.expect("a refusal").code, "bad_request");

    // Sections is not a mapping.
    let response = session.call("config.apply", json!({"rev": rev, "sections": []}));
    assert_eq!(response.error.expect("a refusal").code, "bad_request");

    // A section is not a mapping.
    let response = session.call(
        "config.apply",
        json!({"rev": rev, "sections": {"quest": "not a block"}}),
    );
    assert_eq!(response.error.expect("a refusal").code, "bad_request");

    // A missing or non-numeric rev.
    for args in [
        json!({"sections": {"quest": {}}}),
        json!({"rev": "1", "sections": {"quest": {}}}),
        json!({"rev": -1, "sections": {"quest": {}}}),
    ] {
        let response = session.call("config.apply", args);
        assert_eq!(response.error.expect("a refusal").code, "bad_request");
    }

    // validate has the same shape checks.
    for args in [
        json!({}),
        json!({"sections": []}),
        json!({"sections": {"quest": 7}}),
    ] {
        let response = session.call("config.validate", args);
        assert_eq!(response.error.expect("a refusal").code, "bad_request");
    }
}

#[test]
fn ui_window_refuses_malformed_arguments() {
    let root = config_root();
    let mut session = Session::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));
    let before = root.read_bytes("config/local.yaml");

    // Not an object at all.
    let response = session.call("ui.window", json!([100, 50]));
    assert_eq!(response.error.expect("a refusal").code, "bad_request");

    // A required field is missing.
    let response = session.call("ui.window", json!({"left": 100, "top": 50}));
    assert_eq!(response.error.expect("a refusal").code, "bad_request");

    // A field has the wrong type.
    let response = session.call(
        "ui.window",
        json!({"left": "100", "top": 50, "width": 900, "height": 700, "pinned": true}),
    );
    assert_eq!(response.error.expect("a refusal").code, "bad_request");

    assert_eq!(root.read_bytes("config/local.yaml"), before);
}

#[test]
fn a_config_command_before_hello_is_not_ready() {
    let root = config_root();
    let mut session = Session::start(root.path());

    let response = session.call("config.apply", json!({"rev": 0, "sections": {}}));

    assert!(!response.ok);
    assert_eq!(response.error.expect("a refusal").code, "not_ready");
}

#[test]
fn a_failed_apply_leaves_the_revision_alone() {
    // A refused call must not consume a revision, or every refusal would force a reload.
    let root = config_root();
    let mut session = Session::start(root.path());
    let hello = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let response = session.call(
        "config.apply",
        json!({"rev": hello["rev"], "sections": {"my_notes": {}}}),
    );
    assert!(!response.ok);

    // The same rev still works for a real save.
    let result = session.result(
        "config.apply",
        json!({"rev": hello["rev"], "sections": {"quest": {"active": "after-refusal"}}}),
    );
    assert_eq!(result["rev"], 1);
}

/// The Quest tab's Save gate, end to end: `validate` answers with the domain's exact lines; `apply`
/// refuses with the same lines, writes nothing, and does not consume the revision it was sent.
#[test]
fn a_sequence_entry_naming_a_missing_flow_is_refused_without_writing() {
    let root = config_root();
    let before = root.read_bytes("config/local.yaml");
    let mut session = Session::start(root.path());
    let hello = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));
    let rev = hello["rev"].clone();

    let dangling = json!({
        "active": "run1",
        "presets": {"run1": {"steps": []}},
        "active_sequence": "seq1",
        "sequences": {"seq1": {"entries": [{"preset": "ghost", "times": 1}]}},
    });
    let expected_lines = json!([
        "\"seq1\" entry 1 names the flow \"ghost\", which does not exist",
        "Pick an existing flow for each entry, or remove it, then Save again.",
    ]);

    let validated = session.result(
        "config.validate",
        json!({"sections": {"quest": dangling.clone()}}),
    );
    assert_eq!(validated["errors"], expected_lines);
    assert_eq!(validated["warnings"], json!([]));

    let refused = session.call(
        "config.apply",
        json!({"rev": rev, "sections": {"quest": dangling}}),
    );
    assert!(!refused.ok);
    let error = refused.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert_eq!(error.detail["errors"], expected_lines);
    // Nothing was written and nothing was consumed — the same rev still saves.
    assert_eq!(root.read_bytes("config/local.yaml"), before);
    let accepted = session.result(
        "config.apply",
        json!({"rev": rev, "sections": {"quest": {
            "active": "run1",
            "presets": {"run1": {"steps": []}},
            "sequences": {},
        }}}),
    );
    assert_eq!(accepted["rev"], 1);
}

/// The Hotkeys tab's rule, end to end: a code with no key name is one line per bad box, quoting the
/// name the tab would show; apply refuses and writes nothing.
#[test]
fn invalid_hotkey_codes_are_refused_one_line_per_field_without_writing() {
    let root = config_root();
    let before = root.read_bytes("config/local.yaml");
    let mut session = Session::start(root.path());
    let hello = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let settings = json!({"hotkeys": {
        "start": 5,
        "quit": 7,
        "advance_grade": 71,
        "pause": 20,
    }});
    let expected_lines = json!([
        "\"0x5\" is not a key. Use F1–24, Esc, CapsLock, Space, Tab, Enter, or a single letter/digit.",
        "\"0x7\" is not a key. Use F1–24, Esc, CapsLock, Space, Tab, Enter, or a single letter/digit.",
    ]);

    let validated = session.result(
        "config.validate",
        json!({"sections": {"settings": settings.clone()}}),
    );
    assert_eq!(validated["errors"], expected_lines);

    let refused = session.call(
        "config.apply",
        json!({"rev": hello["rev"], "sections": {"settings": settings}}),
    );
    assert!(!refused.ok);
    let error = refused.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert_eq!(error.detail["errors"], expected_lines);
    assert_eq!(root.read_bytes("config/local.yaml"), before);
}

/// The clean direction end to end: a `settings` save that passes the rules — valid hotkey codes and
/// a known variant — is written and consumes a revision, so the gate refuses only what is wrong.
#[test]
fn a_valid_settings_save_succeeds_and_bumps_the_revision() {
    let root = config_root();
    let defaults_before = root.read_bytes("config/defaults.yaml");
    let mut session = Session::start(root.path());
    let hello = session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));

    let result = session.result(
        "config.apply",
        json!({"rev": hello["rev"], "sections": {"settings": {
            "hotkeys": {"start": 123, "quit": 122, "advance_grade": 71, "pause": 20},
            "game": {"variant": "us"},
        }}}),
    );

    assert_eq!(result["rev"], 1);
    let local = yaml_file(&root, "config/local.yaml");
    assert_eq!(local["settings"]["hotkeys"]["start"], 123);
    assert_eq!(local["settings"]["game"]["variant"], "us");
    assert_eq!(root.read_bytes("config/defaults.yaml"), defaults_before);
}

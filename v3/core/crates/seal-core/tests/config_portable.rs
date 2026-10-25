//! `config.apply`'s portable half and `config_invalid`, over the real protocol (`PROTOCOL.md` §4/§6/§7).
//!
//! These are the ruling's shape/refusal tests, written before the code:
//!
//! - a `settings` section is the portable half: written through the settings PROJECTION (not as a raw
//!   block), one rev with any owned blocks, and the result echoes the effective sections for the names
//!   in the request, in the request's structure;
//! - the variant is resolved and its dictionary LOADED before anything is written (v2's
//!   `SwitchVariant` order) — a missing dictionary is `io`, nothing written, no rev consumed;
//! - `config.reload` meeting an invalid on-disk config answers `config_invalid` with the validator's
//!   lines in `detail.errors` and keeps the session's config;
//! - `config.validate` runs the same effective merge, so a portable-only request is validated rather
//!   than rejected.

mod common;

use common::{TestDir, config_root};
use serde_json::{Value, json};

fn hello(session: &mut common::Session) -> Value {
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}))
}

fn rev(session: &mut common::Session) -> Value {
    hello(session)["rev"].clone()
}

/// A root whose config names two game clients, each with its own dictionary; `tw` is active.
/// `attributes.us.yaml` is deliberately NOT written: the caller decides when it exists.
fn variant_root(with_us: bool) -> TestDir {
    let root = TestDir::new();
    root.write("config/defaults.yaml", common::BASE_CONFIG);
    root.write("config/local.yaml", "settings:\n  game:\n    variant: tw\n");
    root.write("config/attributes.tw.yaml", "per_level_stats:\n  - tw\n");
    if with_us {
        root.write("config/attributes.us.yaml", "per_level_stats:\n  - us\n");
    }
    root
}

#[test]
fn apply_accepts_a_settings_section_and_writes_the_projection() {
    let root = config_root();
    let mut session = common::Session::start(root.path());
    hello(&mut session);

    let response = session.call(
        "config.apply",
        json!({
            "rev": 0,
            "sections": {"settings": {"window": {"title": "PORTABLE_TITLE"}}},
        }),
    );

    assert!(
        response.ok,
        "a settings section is part of config.apply: {:?}",
        response.error
    );
    let result = response.result.expect("a result");
    assert_eq!(result["rev"], 1, "one rev bump covers the portable write");
    // The echo is the EFFECTIVE section for the requested name, in the request's structure.
    assert_eq!(
        result["sections"]["settings"]["window"]["title"],
        "PORTABLE_TITLE"
    );

    // The write went through the settings PROJECTION: the local file carries the portable half (the
    // fixture's local.yaml had no hotkeys/arduino of its own, so their presence proves projection).
    let local = root.read("config/local.yaml");
    assert!(local.contains("settings:"), "{local}");
    assert!(local.contains("PORTABLE_TITLE"), "{local}");
    assert!(
        local.contains("hotkeys:"),
        "projection carries the whole portable shape: {local}"
    );
    assert!(local.contains("arduino:"), "{local}");

    // The merged view the shell gets next agrees.
    assert_eq!(
        hello(&mut session)["config"]["window"]["title"],
        "PORTABLE_TITLE"
    );
}

#[test]
fn apply_carries_owned_and_portable_sections_together_in_one_rev() {
    let root = config_root();
    let mut session = common::Session::start(root.path());
    hello(&mut session);

    let response = session.call(
        "config.apply",
        json!({
            "rev": 0,
            "sections": {
                "tuner": {"guard_px": 9},
                "settings": {"hotkeys": {"start": 66}},
            },
        }),
    );

    assert!(response.ok, "{:?}", response.error);
    let result = response.result.expect("a result");
    assert_eq!(result["rev"], 1);
    // Owned at top level, portable under `settings`, each the effective value.
    assert_eq!(result["sections"]["tuner"]["guard_px"], 9);
    assert_eq!(result["sections"]["settings"]["hotkeys"]["start"], 66);

    let local = root.read("config/local.yaml");
    assert!(local.contains("guard_px: 9"), "{local}");
    assert!(local.contains("start: 66"), "{local}");
}

#[test]
fn a_settings_section_that_is_not_an_object_is_a_bad_request() {
    let root = config_root();
    let mut session = common::Session::start(root.path());
    hello(&mut session);

    let response = session.call(
        "config.apply",
        json!({"rev": 0, "sections": {"settings": "nope"}}),
    );

    assert!(!response.ok);
    assert_eq!(response.error.expect("a refusal").code, "bad_request");
}

#[test]
fn validate_uses_the_same_effective_merge_for_portable_sections() {
    let root = config_root();
    let mut session = common::Session::start(root.path());
    hello(&mut session);

    // A portable-only request is validated against the overlay, not rejected as an unknown section.
    let clean = session.result(
        "config.validate",
        json!({"sections": {"settings": {"hotkeys": {"start": 66}}}}),
    );
    assert_eq!(clean["errors"], json!([]));

    // A bad portable value still refuses: an unknown client names the known ones in v2's line.
    let refused = session.result(
        "config.validate",
        json!({"sections": {"settings": {"game": {"variant": "nope"}}}}),
    );
    let errors = refused["errors"].as_array().expect("errors is an array");
    assert!(!errors.is_empty(), "{refused}");
    assert!(
        errors.iter().any(|line| line
            .as_str()
            .is_some_and(|line| line.contains("nope") || line.contains("variant"))),
        "{errors:?}"
    );
}

#[test]
fn config_validate_still_rejects_an_unknown_owned_block() {
    let root = config_root();
    let mut session = common::Session::start(root.path());
    hello(&mut session);

    let response = session.call("config.validate", json!({"sections": {"not_a_block": {}}}));

    assert!(!response.ok);
    assert_eq!(response.error.expect("a refusal").code, "bad_request");
}

#[test]
fn a_variant_change_through_apply_writes_nothing_when_its_dictionary_is_missing() {
    let root = variant_root(false);
    let mut session = common::Session::start(root.path());
    hello(&mut session);
    let local_before = root.read_bytes("config/local.yaml");
    let rev_before = rev(&mut session);

    let response = session.call(
        "config.apply",
        json!({
            "rev": rev_before,
            "sections": {"settings": {"game": {"variant": "us"}}},
        }),
    );

    // v2's SwitchVariant order: resolve → validate → LOAD → write. The load fails, so nothing is
    // written and no rev is consumed.
    assert!(!response.ok, "a missing dictionary must refuse the switch");
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "io", "the loader's line is an io failure");
    assert!(
        error.message.contains("attributes.us.yaml"),
        "the line names the missing dictionary: {}",
        error.message
    );
    assert_eq!(
        root.read_bytes("config/local.yaml"),
        local_before,
        "the local file must be untouched"
    );
    assert_eq!(rev(&mut session), rev_before, "no rev consumed");

    // Once the dictionary exists the same request goes through — proving the refusal was the load,
    // not the shape.
    root.write("config/attributes.us.yaml", "per_level_stats:\n  - us\n");
    let response = session.call(
        "config.apply",
        json!({
            "rev": rev_before,
            "sections": {"settings": {"game": {"variant": "us"}}},
        }),
    );
    assert!(response.ok, "{:?}", response.error);
    assert!(
        root.read("config/local.yaml").contains("us"),
        "the variant landed in local.yaml"
    );
}

#[test]
fn a_variant_change_through_apply_echoes_the_effective_section() {
    let root = variant_root(true);
    let mut session = common::Session::start(root.path());
    hello(&mut session);

    let response = session.call(
        "config.apply",
        json!({
            "rev": 0,
            "sections": {"settings": {"game": {"variant": "us"}}},
        }),
    );

    assert!(response.ok, "{:?}", response.error);
    let result = response.result.expect("a result");
    // The effective `game` section, not just the overlay the shell sent.
    assert_eq!(result["sections"]["settings"]["game"]["variant"], "us");
    assert!(
        result["sections"]["settings"]["game"]["variants"]["us"]["attributes"]
            .as_str()
            .is_some_and(|name| name == "attributes.us.yaml"),
        "the effective section carries the variants table: {result}"
    );
}

#[test]
fn reload_with_an_invalid_on_disk_config_answers_config_invalid_and_keeps_the_session() {
    let root = config_root();
    let mut session = common::Session::start(root.path());
    let before = hello(&mut session);
    assert_eq!(before["config"]["window"]["title"], "TEST_TITLE");

    // An invalid on-disk config: no arduino, no tuner geometry, no gem section — v2's initialisers
    // make those default objects, and the ported rules refuse them.
    root.write("config/defaults.yaml", "window:\n  title: TEST_TITLE\n");

    let response = session.call("config.reload", json!({}));
    assert!(!response.ok, "an invalid config must not reload silently");
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "config_invalid");
    let errors = error.detail["errors"].as_array().expect("detail.errors");
    assert!(!errors.is_empty());
    assert!(
        errors.iter().any(|line| line
            .as_str()
            .is_some_and(|line| line.to_lowercase().contains("tuner"))),
        "the validator's lines are carried: {errors:?}"
    );

    // The session kept what it had.
    let after = hello(&mut session);
    assert_eq!(after["config"], before["config"]);
    assert_eq!(after["rev"], before["rev"]);
}

#[test]
fn apply_writes_nothing_when_the_projection_cannot_be_built() {
    // The two writes of apply used to be independent: the owned block landed, then the projection
    // could fail and answer `io` with the file HALF-SAVED. Remove the template the projection is
    // built from, after boot, and pin PROTOCOL §4's nothing-written promise.
    let root = config_root();
    let mut session = common::Session::start(root.path());
    hello(&mut session);
    let local_before = root.read_bytes("config/local.yaml");
    let rev_before = rev(&mut session);

    std::fs::remove_file(root.path_of("config/defaults.yaml")).expect("remove the template");

    let response = session.call(
        "config.apply",
        json!({
            "rev": rev_before,
            "sections": {
                "tuner": {"guard_px": 9},
                "settings": {"window": {"title": "X"}},
            },
        }),
    );

    assert!(!response.ok, "a write that cannot complete must refuse");
    assert_eq!(
        root.read_bytes("config/local.yaml"),
        local_before,
        "nothing may be half-written"
    );
    assert_eq!(rev(&mut session), rev_before, "no rev consumed");
}

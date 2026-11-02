//! Increment 0 of PLAN-PORTABLE-SETUP: the registry must cover every path of the REAL typed overlay.
//!
//! The unit tests in `seal_config::registry` pin the table's own consistency. This test walks the
//! decoded v2-era fixture (the largest real-shaped config the repo keeps: calibration, portable
//! `settings:` and every personal block) through `LocalOverrides` -> JSON, then asserts every leaf
//! path has an owner in the new class × domain layout. A key added to the typed model without a home
//! fails here, before a future loader can silently drop it.

use seal_config::registry::{self, Class};
use seal_domain::config::LocalOverrides;
use serde_json::Value;

const FIXTURE: &str = include_str!("../../seal-domain/tests/fixtures/v2-era-config.json");

/// Every leaf path in an object tree. Arrays are leaves: the registry owns fields/arrays wholesale,
/// and walking their indices would make ownership depend on the player's row count.
fn leaf_paths(value: &Value, prefix: &mut Vec<String>, out: &mut Vec<String>) {
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (key, child) in map {
                prefix.push(key.clone());
                leaf_paths(child, prefix, out);
                prefix.pop();
            }
        }
        Value::Object(_) => out.push(prefix.join(".")),
        Value::Array(_) | Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => {
            out.push(prefix.join("."));
        }
    }
}

#[test]
fn every_typed_overlay_leaf_has_one_owner_in_the_new_layout() {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("the fixture is JSON");
    let local: LocalOverrides =
        serde_json::from_value(fixture).expect("the v2-era fixture decodes into the typed overlay");
    let value = serde_json::to_value(local).expect("the typed overlay serialises");

    let mut leaves = Vec::new();
    leaf_paths(&value, &mut Vec::new(), &mut leaves);
    assert!(
        leaves.len() > 100,
        "the fixture walks a real config: {leaves:?}"
    );

    let unowned = leaves
        .iter()
        .filter(|path| registry::owner_of_model(path).is_none())
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        unowned.is_empty(),
        "typed-model leaves with no owner in the new layout: {unowned:?}"
    );

    // The class is the portability answer, so spot-check the two halves the player asked about.
    for path in &leaves {
        let file = registry::owner_of_model(path).expect("asserted above");
        if let Some(stripped) = path.strip_prefix("settings.") {
            if stripped == "legacy_folded" {
                assert_eq!(file.path, "meta.yaml", "the fold marker is bookkeeping");
            } else {
                assert_eq!(
                    file.class,
                    Class::Setup,
                    "{path} is portable and must live under setup/: {file:?}"
                );
            }
        }
    }

    for path in ["calibration.dpi_scale", "tuner.ocr.region", "ui.left"] {
        let file = registry::owner_of_model(path).expect("a machine path has an owner");
        assert_eq!(
            file.class,
            Class::Machine,
            "{path} is measured on this machine"
        );
    }
    assert_eq!(
        registry::owner_of_model("spammer.presets").unwrap().class,
        Class::Setup,
        "the spammer rotations travel"
    );
}

#[test]
fn every_data_file_owns_something_and_no_shipped_file_claims_config() {
    for file in registry::FILES {
        if file.path.ends_with('/') {
            continue;
        }
        match file.class {
            Class::Setup | Class::Machine => {
                assert!(
                    !file.owns.is_empty(),
                    "{} is a data file but owns no paths",
                    file.path
                );
            }
            Class::Defaults | Class::Release | Class::User => {
                assert!(
                    file.owns.is_empty(),
                    "{} must not claim typed config paths",
                    file.path
                );
            }
        }
    }
}

//! Increment 1 of PLAN-PORTABLE-SETUP: the per-file format layer's behaviour.
//!
//! Every test writes into Cargo's target scratch (the `TestDir` helper), never the player's tree.

mod common;

use common::TestDir;
use seal_config::file;
use seal_config::registry::{self, Class, SCHEMA_VERSION};
use serde_json::json;

fn file_for(path: &str) -> &'static registry::FileDef {
    registry::FILES
        .iter()
        .find(|file| file.path == path)
        .expect("a registry entry")
}

fn temp_files(dir: &TestDir) -> Vec<String> {
    std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            std::path::Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("tmp"))
        })
        .collect()
}

#[test]
fn a_write_then_read_round_trips_unknown_keys_and_strips_the_reserved_key() {
    let dir = TestDir::new();
    let tuner = file_for("setup/tuner.yaml");
    let content = json!({
        "tuner": {"target_grade": "DG", "grade_order": ["N", "G", "DG"]},
        "a_future_key": {"nested": 1, "list": [1, 2, 3]},
    });

    file::write(dir.path(), tuner, &content).expect("a setup file writes");
    let text = dir.read("setup/tuner.yaml");
    assert!(
        text.starts_with("# "),
        "the file carries its header: {text:?}"
    );
    assert!(
        text.contains("schema_version: 1"),
        "the reserved key is written: {text}"
    );
    assert!(
        text.contains("unknown keys are kept") || text.contains("KEPT"),
        "the header says unknown keys are kept: {text}"
    );

    let read = file::read(dir.path(), tuner)
        .expect("reading it back succeeds")
        .expect("the file exists");
    assert_eq!(
        read, content,
        "the reserved key is not exposed to the config"
    );
    assert!(
        read.get("schema_version").is_none(),
        "the reserved key is stripped"
    );
    assert!(
        temp_files(&dir).is_empty(),
        "an atomic write leaves no temp file"
    );
}

#[test]
fn a_file_without_a_schema_version_is_read_as_version_one() {
    let dir = TestDir::new();
    dir.write("setup/gem.yaml", "gem:\n  grades: [N, G]\n");
    let gem = file_for("setup/gem.yaml");
    let read = file::read(dir.path(), gem).unwrap().expect("present");
    assert_eq!(read, json!({"gem": {"grades": ["N", "G"]}}));
}

#[test]
fn a_newer_schema_version_is_refused_loudly() {
    let dir = TestDir::new();
    dir.write(
        "machine/pet.yaml",
        "schema_version: 99\npet:\n  items_per_minute: 3\n",
    );
    let pet = file_for("machine/pet.yaml");
    let error = file::read(dir.path(), pet).expect_err("a newer file is refused");
    let message = error.to_string();
    assert!(message.contains("machine/pet.yaml"), "{message}");
    assert!(
        message.contains("99") && message.contains(&SCHEMA_VERSION.to_string()),
        "the message names found and supported versions: {message}"
    );
}

#[test]
fn a_missing_file_is_not_an_error() {
    let dir = TestDir::new();
    let missing = file_for("machine/tooltip.yaml");
    assert_eq!(file::read(dir.path(), missing).unwrap(), None);
}

#[test]
fn a_malformed_or_non_mapping_file_is_a_named_parse_error() {
    let dir = TestDir::new();
    let common = file_for("setup/common.yaml");

    dir.write("setup/common.yaml", "window: [unclosed\n");
    let error = file::read(dir.path(), common).expect_err("broken YAML is loud");
    assert!(error.to_string().contains("setup/common.yaml"), "{error}");

    dir.write("setup/common.yaml", "- just\n- a list\n");
    let error = file::read(dir.path(), common).expect_err("a list is not a data file");
    assert!(
        error.to_string().contains("mapping"),
        "the message says what shape was wrong: {error}"
    );
}

#[test]
fn a_refused_write_leaves_the_old_file_whole() {
    let dir = TestDir::new();
    let tooltip = file_for("machine/tooltip.yaml");
    let first = json!({"tooltip": {"width": 320}});
    file::write(dir.path(), tooltip, &first).unwrap();
    let before = dir.read("machine/tooltip.yaml");

    let error = file::write(dir.path(), tooltip, &json!(["not a mapping"]))
        .expect_err("a non-mapping content is refused");
    assert!(error.to_string().contains("machine/tooltip.yaml"));
    assert_eq!(
        dir.read("machine/tooltip.yaml"),
        before,
        "old file untouched"
    );
    assert!(
        temp_files(&dir).is_empty(),
        "no temp file survives a refused write"
    );
}

#[test]
fn the_header_answers_whether_and_who() {
    for path in ["setup/tuner.yaml", "machine/tuner.yaml"] {
        let def = file_for(path);
        let header = file::header(def);
        assert!(
            header.contains("PORTABLE") || header.contains("PER-PC"),
            "{path}: {header}"
        );
        assert!(
            header.contains("tuner"),
            "{path}: the domain is named: {header}"
        );
        assert!(
            header.contains("schema_version") && header.contains("unknown keys"),
            "{path}: {header}"
        );
    }
}

#[test]
fn the_registry_class_decides_which_kind_of_header_is_written() {
    let dir = TestDir::new();
    let setup = file_for("setup/spammer.yaml");
    file::write(dir.path(), setup, &json!({"spammer": {}})).unwrap();
    let setup_text = dir.read("setup/spammer.yaml");
    assert!(setup_text.contains("PORTABLE"), "{setup_text}");
    assert_eq!(setup.class, Class::Setup);

    let machine = file_for("machine/ui.yaml");
    file::write(dir.path(), machine, &json!({"ui": {}})).unwrap();
    let machine_text = dir.read("machine/ui.yaml");
    assert!(machine_text.contains("PER-PC"), "{machine_text}");
    assert_eq!(machine.class, Class::Machine);
}

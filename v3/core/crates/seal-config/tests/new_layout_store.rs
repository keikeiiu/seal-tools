//! The live-path switch: `ConfigStore` reads and writes the class × domain tree once `meta.yaml`
//! marks a migrated install, while a dev tree without it keeps the legacy single-file path.
//!
//! These tests build a small new-layout tree and exercise the same API the core uses: `load`,
//! `load_local`, `update_local` and `persist_pet_run_state`. The legacy suite keeps covering the
//! old path (its tests carry no `meta.yaml`).

mod common;

use common::TestDir;
use seal_config::file;
use seal_config::registry::{self, FileDef};
use seal_config::{ConfigStore, PetRunStatePatch};
use serde_json::{Value, json};

fn def(path: &str) -> &'static FileDef {
    registry::FILES
        .iter()
        .find(|file| file.path == path)
        .expect("a registry entry")
}

fn write(dir: &TestDir, path: &str, content: &Value) {
    file::write(dir.path(), def(path), content).expect("seeds a file");
}

fn tree() -> TestDir {
    let dir = TestDir::new();
    std::fs::create_dir_all(dir.path()).unwrap();
    // A shipped install carries the template; a checkout may not.
    dir.write(
        "defaults.yaml.example",
        "window:\n  title: DEFAULT\ntuner:\n  target_grade: N\n  ocr:\n    row_height: 10\n",
    );
    write(
        &dir,
        "meta.yaml",
        &json!({"layout_version": 1, "legacy_folded": true}),
    );
    write(
        &dir,
        "setup/common.yaml",
        &json!({"window": {"title": "TW_LIVE"}, "hotkeys": {"start": 123}}),
    );
    write(
        &dir,
        "machine/tuner.yaml",
        &json!({"tuner": {"ocr": {"row_height": 19}}}),
    );
    write(
        &dir,
        "machine/ui.yaml",
        &json!({"ui": {"left": 10.0, "top": 20.0}}),
    );
    dir
}

#[test]
fn the_new_layout_loads_the_merged_shape_and_the_legacy_view() {
    let dir = tree();
    let store = ConfigStore::new(dir.path());
    let config = store.load().expect("the new tree loads");
    assert_eq!(config["window"]["title"], "TW_LIVE");
    assert_eq!(config["hotkeys"]["start"], 123);
    assert_eq!(config["tuner"]["target_grade"], "N", "base kept");
    assert_eq!(config["tuner"]["ocr"]["row_height"], 19, "machine override");
    assert!(!dir.exists("local.yaml"), "new mode never seeds local.yaml");

    let local = store.load_local().expect("the overlay view");
    assert_eq!(local["settings"]["window"]["title"], "TW_LIVE");
    assert_eq!(local["settings"]["hotkeys"]["start"], 123);
    assert_eq!(local["tuner"]["ocr"]["row_height"], 19);
    assert_eq!(local["ui"]["left"], 10.0);
    assert_eq!(local["settings"]["legacy_folded"], true);
}

#[test]
fn an_update_writes_only_the_files_owning_the_changed_blocks() {
    let dir = tree();
    let store = ConfigStore::new(dir.path());
    store
        .update_local(|local| {
            local["settings"]["window"]["title"] = json!("NEW_TITLE");
            local["settings"]["tuner"]["target_grade"] = json!("DG");
            local["tuner"]["ocr"]["row_height"] = json!(19);
        })
        .expect("an update routes");
    assert_eq!(
        file::read(dir.path(), def("setup/common.yaml"))
            .unwrap()
            .unwrap()["window"]["title"],
        "NEW_TITLE"
    );
    assert_eq!(
        file::read(dir.path(), def("setup/tuner.yaml"))
            .unwrap()
            .unwrap()["tuner"]["target_grade"],
        "DG"
    );
    let ui = file::read(dir.path(), def("machine/ui.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(ui["ui"]["left"], 10.0, "an untouched file stays");
    assert!(
        !dir.exists(".seal-txn"),
        "a single-file update does not create the transaction scratch"
    );
    assert!(!dir.exists("local.yaml"));
    let reloaded = store.load().expect("reloads");
    assert_eq!(reloaded["window"]["title"], "NEW_TITLE");
}

#[test]
fn a_two_block_update_uses_the_transaction_and_lands_in_both_files() {
    let dir = tree();
    let store = ConfigStore::new(dir.path());
    store
        .update_local(|local| {
            local["settings"]["pet"] = json!({"items_per_minute": 5});
            local["pet"] = json!({"slots": [{"stacks": 2, "enabled": true}]});
        })
        .expect("a two-file update");
    assert_eq!(
        file::read(dir.path(), def("setup/pet.yaml"))
            .unwrap()
            .unwrap()["pet"]["items_per_minute"],
        5
    );
    assert_eq!(
        file::read(dir.path(), def("machine/pet.yaml"))
            .unwrap()
            .unwrap()["pet"]["slots"][0]["stacks"],
        2
    );
    assert!(!dir.exists(".seal-txn"));
}

#[test]
fn the_pet_run_state_persists_through_the_routing() {
    let dir = tree();
    let store = ConfigStore::new(dir.path());
    write(
        &dir,
        "machine/pet.yaml",
        &json!({"pet": {"slots": [{"enabled": true, "stacks": 2}]}}),
    );
    store
        .persist_pet_run_state(&PetRunStatePatch {
            food_slots_used: Some(1),
            food_slots: Some(vec![(1, 1)]),
            boarding_running: vec![(0, true)],
            close_button_png: None,
            boarding_window_may_be_open: Some(true),
        })
        .expect("the run state writes");
    let pet = file::read(dir.path(), def("machine/pet.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(
        pet["pet"]["food_slots_used"], 0,
        "replacing the food cells resets the used count"
    );
    assert_eq!(pet["pet"]["slots"][0]["boarding_running"], true);
    assert_eq!(pet["pet"]["boarding_window_may_be_open"], true);
}

#[test]
fn a_shipped_install_with_local_yaml_migrates_on_the_first_load() {
    let dir = TestDir::new();
    std::fs::create_dir_all(dir.path()).unwrap();
    dir.write(
        "defaults.yaml.example",
        "window:\n  title: DEFAULT\ntuner:\n  ocr:\n    row_height: 10\n",
    );
    dir.write("local.yaml.example", "window:\n  title: DEFAULT\n");
    dir.write(
        "local.yaml",
        "settings:\n  window:\n    title: TW_LIVE\ntuner:\n  ocr:\n    row_height: 19\n",
    );
    let store = ConfigStore::new(dir.path());
    let config = store.load().expect("the first load migrates and reads");
    assert_eq!(config["window"]["title"], "TW_LIVE");
    assert_eq!(config["tuner"]["ocr"]["row_height"], 19);
    assert!(!dir.exists("local.yaml"), "the old file is retired");
    assert!(dir.exists("meta.yaml"), "the layout marker is written");
    assert!(dir.exists("setup/common.yaml"));
    assert!(dir.exists("machine/tuner.yaml"));
    // A second load is a plain new-layout read (no second backup).
    let backups = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".migration-backup-")
        })
        .count();
    assert_eq!(backups, 1);
    store.load().expect("the second load is clean");
}

#[test]
fn a_newer_file_is_refused_by_the_live_loader() {
    let dir = tree();
    dir.write(
        "machine/tuner.yaml",
        "schema_version: 99\ntuner:\n  ocr:\n    row_height: 19\n",
    );
    let store = ConfigStore::new(dir.path());
    let error = store.load().expect_err("a newer file is loud");
    assert!(error.to_string().contains("machine/tuner.yaml"), "{error}");
}

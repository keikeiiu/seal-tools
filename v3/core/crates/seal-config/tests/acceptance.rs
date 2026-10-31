//! Increment 8 of PLAN-PORTABLE-SETUP: the acceptance claims, expressed as tests.
//!
//! - a copied `setup/` reproduces the portable setup on a second root; `machine/` is absent there
//!   and the calibration values are absent (the tools recalibrate as today);
//! - replacing the shipped template/core never touches the player-state tree; a later save changes
//!   ONLY the file that owns the edited value;
//! - a two-file save leaves no transaction scratch behind.

mod common;

use common::TestDir;
use seal_config::ConfigStore;
use seal_config::file;
use seal_config::registry::{self, FileDef};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;

fn def(path: &str) -> &'static FileDef {
    registry::FILES
        .iter()
        .find(|file| file.path == path)
        .expect("a registry entry")
}

fn write(dir: &TestDir, path: &str, content: Value) {
    file::write(dir.path(), def(path), &content).expect("seeds a file");
}

fn template() -> &'static str {
    "window:\n  title: DEFAULT\nbuy_sell:\n  sell_cap: 10\ntuner:\n  target_grade: N\n"
}

fn tree() -> TestDir {
    let dir = TestDir::new();
    dir.write("defaults.yaml.example", template());
    write(
        &dir,
        "meta.yaml",
        json!({"layout_version": 1, "legacy_folded": true}),
    );
    write(
        &dir,
        "setup/common.yaml",
        json!({"window": {"title": "TW_LIVE"}, "hotkeys": {"start": 123}}),
    );
    write(
        &dir,
        "setup/buy_sell.yaml",
        json!({"buy_sell": {"presets": {"Potion": {"scroll": 0, "row": 2, "count": 2}}}}),
    );
    write(
        &dir,
        "machine/display.yaml",
        json!({"calibration": {"dpi_scale": 1.5, "monitor_dpi": 144}}),
    );
    write(
        &dir,
        "machine/pet.yaml",
        json!({"pet": {"slots": [{"enabled": true, "stacks": 4}]}}),
    );
    dir
}

/// Every file in the tree, keyed by its `/`-joined relative path, so nested setup/ and machine/
/// files are compared too.
fn snapshot(dir: &TestDir) -> BTreeMap<String, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        current: &std::path::Path,
        out: &mut BTreeMap<String, Vec<u8>>,
    ) {
        for entry in fs::read_dir(current).unwrap().flatten() {
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                walk(root, &path, out);
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(relative, fs::read(&path).unwrap());
        }
    }
    let mut files = BTreeMap::new();
    walk(dir.path(), dir.path(), &mut files);
    files
}

#[test]
fn a_copied_setup_reproduces_the_portable_half_and_carries_no_calibration() {
    let source = tree();
    let target = TestDir::new();
    target.write("defaults.yaml.example", template());
    // Copy ONLY the portable folder and the bookkeeping marker a migrated install carries.
    fs::create_dir_all(target.path().join("setup")).unwrap();
    for entry in fs::read_dir(source.path().join("setup")).unwrap().flatten() {
        fs::copy(
            entry.path(),
            target.path().join("setup").join(entry.file_name()),
        )
        .unwrap();
    }
    fs::copy(
        source.path().join("meta.yaml"),
        target.path().join("meta.yaml"),
    )
    .unwrap();

    let store = ConfigStore::new(target.path());
    let config = store.load().expect("the copied setup loads");
    assert_eq!(config["window"]["title"], "TW_LIVE");
    assert_eq!(config["hotkeys"]["start"], 123);
    assert_eq!(config["buy_sell"]["presets"]["Potion"]["count"], 2);
    assert!(
        config.pointer("/calibration").is_none(),
        "no display environment travels with setup/"
    );
    assert!(
        config.pointer("/pet/slots").is_none(),
        "no machine pet rows travel with setup/"
    );
    assert!(
        !target.path().join("machine").exists(),
        "the copied root has no machine folder"
    );
}

#[test]
fn replacing_the_template_and_loading_writes_nothing_in_the_player_tree() {
    let dir = tree();
    let before = snapshot(&dir);
    dir.write(
        "defaults.yaml.example",
        "window:\n  title: NEW_DEFAULT\nbuy_sell:\n  sell_cap: 20\ntuner:\n  target_grade: N\n",
    );
    let store = ConfigStore::new(dir.path());
    let loaded = store.load().expect("the new template loads");
    assert_eq!(
        loaded["buy_sell"]["sell_cap"], 20,
        "a default the player never set arrives from the new template"
    );
    let after = snapshot(&dir);
    for name in before.keys() {
        if name == "defaults.yaml.example" {
            continue;
        }
        assert_eq!(before[name], after[name], "{name} was not rewritten");
    }
    assert_eq!(
        loaded["window"]["title"], "TW_LIVE",
        "the player's setup value still wins over the new default"
    );
}

#[test]
fn saving_one_preset_changes_only_the_file_that_owns_it() {
    let dir = tree();
    let store = ConfigStore::new(dir.path());
    let before = snapshot(&dir);
    store
        .update_local(|local| {
            local["settings"]["buy_sell"]["presets"]["Potion"]["count"] = json!(3);
        })
        .expect("the preset save routes");
    let after = snapshot(&dir);
    let changed: Vec<&String> = before
        .keys()
        .filter(|name| before[*name] != after[*name])
        .collect();
    assert_eq!(changed, vec!["setup/buy_sell.yaml"], "one file changed");
    let reloaded = store.load().unwrap();
    assert_eq!(reloaded["buy_sell"]["presets"]["Potion"]["count"], 3);
    assert!(
        !dir.exists(".seal-txn"),
        "the save left no transaction scratch"
    );
}

#[test]
fn a_two_file_save_keeps_the_tree_transactional() {
    let dir = tree();
    let store = ConfigStore::new(dir.path());
    store
        .update_local(|local| {
            local["settings"]["pet"] = json!({"items_per_minute": 5});
            local["pet"] = json!({"slots": [{"enabled": true, "stacks": 2}]});
        })
        .expect("a two-file save");
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
    assert!(!dir.exists(".seal-txn"), "the transaction cleaned up");
}

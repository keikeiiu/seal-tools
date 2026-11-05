//! Increment 4 of PLAN-PORTABLE-SETUP: the two-file transaction and crash recovery.
//!
//! The normal path is exercised through `write_sections`; the crash paths are simulated by building
//! the journal the transaction would have left (the format is documented in `transaction.rs`) and
//! then calling `recover` — the same function a boot/read calls. Recovery always rolls back.

mod common;

use common::TestDir;
use seal_config::file;
use seal_config::layout::{load, write_sections};
use seal_config::registry::{FILES, FileDef};
use seal_config::transaction::{JOURNAL_DIR, recover};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::Path;

fn def(path: &str) -> &'static FileDef {
    FILES
        .iter()
        .find(|file| file.path == path)
        .expect("a registry entry")
}

fn seed(dir: &TestDir, path: &str, content: &Value) {
    file::write(dir.path(), def(path), content).expect("seeds a file");
}

fn sections(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect()
}

fn bytes(dir: &TestDir, path: &str) -> Vec<u8> {
    fs::read(dir.path_of(path)).expect("a file")
}

/// Build the journal an interrupted commit leaves, with both targets' old bytes as backups and both
/// new texts as temps. Returns the transaction directory.
fn interrupted(dir: &TestDir, writes: &[(&str, Value)], first_renamed: bool) -> std::path::PathBuf {
    let directory = dir.path().join(JOURNAL_DIR).join("txn-test");
    fs::create_dir_all(&directory).unwrap();
    let mut entries = Vec::new();
    for (index, (path, content)) in writes.iter().enumerate() {
        let target = dir.path_of(path);
        let existed = target.is_file();
        if existed {
            fs::copy(&target, directory.join(format!("{index}.bak"))).unwrap();
        }
        let def = def(path);
        fs::write(
            directory.join(format!("{index}.tmp")),
            file::encode(def, content).unwrap(),
        )
        .unwrap();
        entries.push(json!({
            "path": path,
            "backup": format!("{index}.bak"),
            "temp": format!("{index}.tmp"),
            "existed": existed,
        }));
        if index == 0 && first_renamed {
            fs::rename(directory.join("0.tmp"), &target).unwrap();
        }
    }
    fs::write(
        directory.join("journal.json"),
        serde_json::to_string_pretty(&json!({"v": 1, "files": entries})).unwrap(),
    )
    .unwrap();
    directory
}

#[test]
fn a_two_file_save_writes_both_files_and_leaves_no_journal() {
    let dir = TestDir::new();
    seed(
        &dir,
        "setup/pet.yaml",
        &json!({"pet": {"items_per_minute": 3}}),
    );
    seed(&dir, "machine/pet.yaml", &json!({"pet": {"slots": []}}));

    let written = write_sections(
        dir.path(),
        &sections(&[
            ("settings", json!({"pet": {"items_per_minute": 5}})),
            ("pet", json!({"slots": [{"stacks": 2}]})),
        ]),
    )
    .unwrap();
    assert!(written.changed("setup/pet.yaml"));
    assert!(written.changed("machine/pet.yaml"));
    assert!(
        !Path::new(&dir.path().join(JOURNAL_DIR)).exists(),
        "a clean commit removes the transaction directory"
    );
    let setup = file::read(dir.path(), def("setup/pet.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(setup["pet"]["items_per_minute"], 5);
    let machine = file::read(dir.path(), def("machine/pet.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(machine["pet"]["slots"][0]["stacks"], 2);
}

#[test]
fn a_crash_after_the_first_rename_rolls_both_files_back() {
    let dir = TestDir::new();
    seed(
        &dir,
        "setup/common.yaml",
        &json!({"window": {"title": "OLD"}}),
    );
    seed(
        &dir,
        "machine/tuner.yaml",
        &json!({"tuner": {"ocr": {"row_height": 10}}}),
    );
    let old_common = bytes(&dir, "setup/common.yaml");
    let old_tuner = bytes(&dir, "machine/tuner.yaml");

    interrupted(
        &dir,
        &[
            ("setup/common.yaml", json!({"window": {"title": "NEW"}})),
            (
                "machine/tuner.yaml",
                json!({"tuner": {"ocr": {"row_height": 19}}}),
            ),
        ],
        true,
    );

    assert_eq!(recover(dir.path()).unwrap(), 1);
    assert_eq!(
        bytes(&dir, "setup/common.yaml"),
        old_common,
        "first file rolled back"
    );
    assert_eq!(
        bytes(&dir, "machine/tuner.yaml"),
        old_tuner,
        "second file old"
    );
    assert!(!dir.path().join(JOURNAL_DIR).join("txn-test").exists());
    assert_eq!(recover(dir.path()).unwrap(), 0, "recovery is idempotent");
}

#[test]
fn a_crash_before_any_rename_rolls_back_to_the_same_bytes() {
    let dir = TestDir::new();
    seed(
        &dir,
        "setup/common.yaml",
        &json!({"window": {"title": "OLD"}}),
    );
    let old = bytes(&dir, "setup/common.yaml");
    interrupted(
        &dir,
        &[("setup/common.yaml", json!({"window": {"title": "NEW"}}))],
        false,
    );
    recover(dir.path()).unwrap();
    assert_eq!(bytes(&dir, "setup/common.yaml"), old);
}

#[test]
fn recovery_removes_a_target_that_did_not_exist_before_the_transaction() {
    let dir = TestDir::new();
    // Simulate a first rename that created machine/tuner.yaml from nothing (`existed:false`).
    let directory = dir.path().join(JOURNAL_DIR).join("txn-new");
    fs::create_dir_all(&directory).unwrap();
    seed(
        &dir,
        "machine/tuner.yaml",
        &json!({"tuner": {"ocr": {"row_height": 19}}}),
    );
    fs::write(
        directory.join("journal.json"),
        serde_json::to_string_pretty(&json!({
            "v": 1,
            "files": [{
                "path": "machine/tuner.yaml",
                "backup": "0.bak",
                "temp": "0.tmp",
                "existed": false
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    recover(dir.path()).unwrap();
    assert!(
        !dir.path().join("machine/tuner.yaml").exists(),
        "the new file is removed"
    );
}

#[test]
fn a_loader_read_heals_an_interrupted_save_first() {
    let dir = TestDir::new();
    seed(
        &dir,
        "setup/common.yaml",
        &json!({"window": {"title": "OLD"}}),
    );
    seed(
        &dir,
        "machine/tuner.yaml",
        &json!({"tuner": {"ocr": {"row_height": 10}}}),
    );
    interrupted(
        &dir,
        &[
            ("setup/common.yaml", json!({"window": {"title": "NEW"}})),
            (
                "machine/tuner.yaml",
                json!({"tuner": {"ocr": {"row_height": 19}}}),
            ),
        ],
        true,
    );
    let loaded = load(dir.path(), &json!({"window": {"title": "DEFAULT"}})).unwrap();
    assert_eq!(loaded.config["window"]["title"], "OLD");
    assert_eq!(loaded.config["tuner"]["ocr"]["row_height"], 10);
    assert!(!dir.path().join(JOURNAL_DIR).join("txn-test").exists());
}

#[test]
fn a_committed_transaction_is_cleaned_up_without_rolling_back() {
    let dir = TestDir::new();
    seed(
        &dir,
        "setup/common.yaml",
        &json!({"window": {"title": "NEW"}}),
    );
    let directory = dir.path().join(JOURNAL_DIR).join("txn-done");
    fs::create_dir_all(&directory).unwrap();
    // The commit wrote the new target and marked the journal done; only cleanup was interrupted.
    fs::write(directory.join("journal.done"), "{}").unwrap();
    fs::write(directory.join("0.bak"), "old").unwrap();

    assert_eq!(recover(dir.path()).unwrap(), 1);
    assert_eq!(
        file::read(dir.path(), def("setup/common.yaml"))
            .unwrap()
            .unwrap()["window"]["title"],
        "NEW",
        "a committed transaction must never be rolled back"
    );
    assert!(!directory.exists(), "the committed scratch is cleaned up");
}

#[test]
fn a_missing_backup_refuses_recovery_instead_of_half_restoring() {
    let dir = TestDir::new();
    seed(
        &dir,
        "setup/common.yaml",
        &json!({"window": {"title": "NEW"}}),
    );
    let directory = dir.path().join(JOURNAL_DIR).join("txn-missing-bak");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("journal.json"),
        serde_json::to_string_pretty(&json!({
            "v": 1,
            "files": [{
                "path": "setup/common.yaml",
                "backup": "0.bak",
                "temp": "0.tmp",
                "existed": true
            }]
        }))
        .unwrap(),
    )
    .unwrap();

    let error = recover(dir.path()).expect_err("a missing backup is not restorable");
    assert!(error.to_string().contains("partial save"), "{error}");
    assert!(
        directory.join("journal.json").exists(),
        "the journal stays for a later, complete recovery attempt"
    );
}

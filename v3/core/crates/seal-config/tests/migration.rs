//! Increment 5 of PLAN-PORTABLE-SETUP: the one-time migration from today's `local.yaml`.

mod common;

use common::TestDir;
use seal_config::file;
use seal_config::layout::load;
use seal_config::migrate::{LAYOUT_VERSION, migrate};
use seal_config::registry;
use serde_json::{Value, json};
use std::fs;

fn base() -> Value {
    json!({
        "window": {"title": "DEFAULT"},
        "tuner": {"target_grade": "N"},
        "gem": {"grades": ["N", "G"]},
        "buy_sell": {"sell_cap": 10}
    })
}

fn local_text() -> &'static str {
    r#"calibration:
  dpi_scale: 1.5
  monitor_dpi: 144
  screen: [1920, 1080]
  client_size: [1200, 600]
tuner:
  ocr:
    region: {left: 100, top: 100, width: 200, height: 120}
    row_height: 19
  target_grade: DG
arduino:
  port: COM5
ui:
  left: 10.0
buy_sell:
  bag_grid: [0, 0, 240, 240]
  presets:
    Potion: {scroll: 0, row: 2, count: 2}
  sell_cap: 12
spammer:
  active: buffs
  presets:
    buffs: {"1": 1.2}
settings:
  window: {title: TW_LIVE}
  tuner: {target_grade: DG}
  pet: {items_per_minute: 3}
  legacy_folded: true
my_notes:
  favourite: seal
"#
}

fn migrated_root() -> TestDir {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join("config")).unwrap();
    dir.write("config/local.yaml", local_text());
    // The shipped template is untouched by the migration; layout::load takes it as the base.
    migrate(dir.path().join("config").as_path(), &base()).expect("migration succeeds");
    dir
}

#[test]
fn the_migration_splits_the_old_file_into_the_new_tree_and_keeps_a_backup() {
    let dir = migrated_root();
    let root = dir.path().join("config");

    assert!(
        !root.join("local.yaml").exists(),
        "the old file is retired only after verification"
    );
    let common = file::read(&root, def("setup/common.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(common["window"]["title"], "TW_LIVE");
    assert!(
        common
            .get("arduino")
            .and_then(|arduino| arduino.get("port"))
            .is_none(),
        "the COM port stays machine-only"
    );
    let arduino = file::read(&root, def("machine/arduino.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(arduino["arduino"]["port"], "COM5");
    let display = file::read(&root, def("machine/display.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(display["calibration"]["dpi_scale"], 1.5);
    let tuner = file::read(&root, def("machine/tuner.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(tuner["tuner"]["ocr"]["row_height"], 19);
    let setup_tuner = file::read(&root, def("setup/tuner.yaml")).unwrap().unwrap();
    assert_eq!(setup_tuner["tuner"]["target_grade"], "DG");
    let buy_setup = file::read(&root, def("setup/buy_sell.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(buy_setup["buy_sell"]["presets"]["Potion"]["count"], 2);
    let buy_machine = file::read(&root, def("machine/buy_sell.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(buy_machine["buy_sell"]["bag_grid"], json!([0, 0, 240, 240]));

    let meta = file::read(&root, def("meta.yaml")).unwrap().unwrap();
    assert_eq!(meta["layout_version"], LAYOUT_VERSION);
    assert_eq!(meta["legacy_folded"], true);

    let user = fs::read_to_string(root.join("user.yaml")).unwrap();
    assert!(
        user.contains("my_notes"),
        "foreign blocks move verbatim: {user}"
    );
    assert!(user.contains("seal"));

    let backups: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".migration-backup-")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    let backup_local = backups[0].path().join("local.yaml");
    assert_eq!(
        fs::read_to_string(backup_local).unwrap(),
        local_text(),
        "the rollback copy is the exact old file"
    );

    let loaded = load(&root, &base()).unwrap();
    assert_eq!(loaded.config["window"]["title"], "TW_LIVE");
    assert_eq!(loaded.config["tuner"]["ocr"]["row_height"], 19);
    assert_eq!(loaded.config["calibration"]["monitor_dpi"], 144);
}

#[test]
fn a_second_migration_is_a_no_op() {
    let dir = migrated_root();
    let root = dir.path().join("config");
    let second = migrate(&root, &base()).expect("a second run is safe");
    assert!(second.is_none(), "already-migrated trees are left alone");
    let backups = fs::read_dir(&root)
        .unwrap()
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".migration-backup-")
        })
        .count();
    assert_eq!(backups, 1, "no duplicate backup");
}

#[test]
fn a_crash_after_retirement_but_before_the_marker_is_recognised() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join("config")).unwrap();
    file::write(
        dir.path().join("config").as_path(),
        def("setup/common.yaml"),
        &json!({"window": {"title": "OLD"}}),
    )
    .unwrap();
    let root = dir.path().join("config");
    assert!(!root.join("local.yaml").is_file());
    let report = migrate(&root, &base()).expect("the tree is finished, not migrated twice");
    assert!(report.is_none());
    let meta = file::read(&root, def("meta.yaml")).unwrap().unwrap();
    assert_eq!(meta["layout_version"], LAYOUT_VERSION);
}

#[test]
fn a_tree_without_local_yaml_and_without_setup_files_is_left_alone() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join("config")).unwrap();
    let root = dir.path().join("config");
    assert!(migrate(&root, &base()).unwrap().is_none());
    assert!(!root.join("meta.yaml").exists());
}

fn def(path: &str) -> &'static registry::FileDef {
    registry::FILES
        .iter()
        .find(|file| file.path == path)
        .expect("a registry entry")
}

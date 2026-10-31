//! Increment 3 of PLAN-PORTABLE-SETUP: one save path per file.
//!
//! These tests prove the routing contract the shell's `config.apply` payload will use: each block's
//! fields land in the file that owns them, one atomic write per file, other files untouched, and an
//! unowned path is refused before any write.

mod common;

use common::TestDir;
use seal_config::file;
use seal_config::layout::{ChangeSummary, write_sections};
use seal_config::registry::{self, FILES, FileDef};
use serde_json::{Map, Value, json};

fn def(path: &str) -> &'static FileDef {
    FILES
        .iter()
        .find(|file| file.path == path)
        .expect("a registry entry")
}

fn sections(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect()
}

fn seed(dir: &TestDir, path: &str, content: Value) {
    file::write(dir.path(), def(path), &content).expect("seeds a file");
}

#[test]
fn one_save_routes_each_field_to_its_file_and_leaves_every_other_file_alone() {
    let dir = TestDir::new();
    seed(
        &dir,
        "setup/common.yaml",
        json!({"window": {"title": "OLD"}, "hotkeys": {"start": 123}}),
    );
    seed(&dir, "machine/ui.yaml", json!({"ui": {"left": 10.0}}));

    let written = write_sections(
        dir.path(),
        &sections(&[
            (
                "settings",
                json!({
                    "window": {"title": "NEW"},
                    "tuner": {"target_grade": "DG"},
                    "legacy_folded": true
                }),
            ),
            (
                "buy_sell",
                json!({
                    "presets": {"Potion": {"scroll": 0, "row": 2, "count": 2}},
                    "bag_grid": [0, 0, 80, 80]
                }),
            ),
            ("tuner", json!({"ocr": {"row_height": 19}})),
        ]),
    )
    .expect("a routable save");

    let mut expected = vec![
        "machine/buy_sell.yaml",
        "machine/tuner.yaml",
        "meta.yaml",
        "setup/buy_sell.yaml",
        "setup/common.yaml",
        "setup/tuner.yaml",
    ];
    expected.sort_unstable();
    assert_eq!(written.files, expected);
    assert!(!written.changed("machine/ui.yaml"));

    let common = file::read(dir.path(), def("setup/common.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(common["window"]["title"], "NEW");
    assert_eq!(
        common["hotkeys"]["start"], 123,
        "a key the save doesn't mention survives"
    );
    let tuner = file::read(dir.path(), def("machine/tuner.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(tuner["tuner"]["ocr"]["row_height"], 19);
    let setup_tuner = file::read(dir.path(), def("setup/tuner.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(setup_tuner["tuner"]["target_grade"], "DG");
    let meta = file::read(dir.path(), def("meta.yaml")).unwrap().unwrap();
    assert_eq!(meta["legacy_folded"], true);
    let buy_setup = file::read(dir.path(), def("setup/buy_sell.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(buy_setup["buy_sell"]["presets"]["Potion"]["count"], 2);
    let buy_machine = file::read(dir.path(), def("machine/buy_sell.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(buy_machine["buy_sell"]["bag_grid"], json!([0, 0, 80, 80]));

    let ui = file::read(dir.path(), def("machine/ui.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(
        ui["ui"]["left"], 10.0,
        "the untouched file is byte-equivalent"
    );
}

#[test]
fn a_whole_block_file_routes_by_its_block_owner() {
    let dir = TestDir::new();
    let written = write_sections(
        dir.path(),
        &sections(&[(
            "spammer",
            json!({"active": "buffs", "presets": {"buffs": {"1": 1.2}}}),
        )]),
    )
    .unwrap();
    assert_eq!(written.files, vec!["setup/spammer.yaml"]);
    let spammer = file::read(dir.path(), def("setup/spammer.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(spammer["spammer"]["active"], "buffs");
}

#[test]
fn an_unowned_path_is_refused_before_anything_is_written() {
    let dir = TestDir::new();
    let error = write_sections(dir.path(), &sections(&[("mystery", json!({"x": 1}))]))
        .expect_err("an unowned block is refused");
    assert!(
        matches!(error, seal_config::ConfigError::SectionNotOwned(_)),
        "{error:?}"
    );
    assert!(
        registry::FILES.iter().all(|file| !dir.exists(file.path)),
        "the refusal wrote no file"
    );
}

#[test]
fn an_empty_payload_writes_nothing() {
    let dir = TestDir::new();
    let written: ChangeSummary = write_sections(dir.path(), &Map::new()).unwrap();
    assert!(written.files.is_empty());
    assert!(
        registry::FILES.iter().all(|file| !dir.exists(file.path)),
        "nothing was created"
    );
}

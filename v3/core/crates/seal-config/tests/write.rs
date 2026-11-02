//! The writer: the constant header, foreign-block preservation, the gated read-modify-write, the
//! atomic replace, and the portable `settings:` projection.
//!
//! Ported from v2's `ConfigLoaderTests`: `BothSavesWriteAHeaderSayingWhatTheyDestroy`,
//! `HeaderDoesNotDisturbTheRoundTrip`, `ATopLevelBlockTheAppDoesNotOwnSurvivesASave`,
//! `ABlockTheAppOwnsIsStillRewritten`, `TwoWritersEachKeepTheOthersBlock`,
//! `SaveSettingsPreservesEveryPortableField`, `SaveSettingsWritesLocalYamlAndLeavesDefaultsAlone`,
//! `APartialSettingsBlockLeavesTheRestStanding`, `SaveSettingsLeavesTheSpammerBlockAlone`, the
//! adoption pair (`LoadAdoptsSpammerPresetsLeftInDefaultsIntoLocalYaml`,
//! `LegacyFlatKeysReachLocalYamlThroughAdoption`) and `AFoldThatCannotBeWritten…`'s litter rule.

mod common;

use std::sync::Arc;
use std::thread;

use common::{EMPTY_LOCAL, TestDir, parse_yaml, repo_config_text, set_readonly};
use seal_config::{ConfigError, ConfigStore, LOCAL_HEADER};
use serde_json::{Map, Value, json};

fn seeded(dir: &TestDir) -> ConfigStore {
    dir.copy_repo("defaults.yaml", "defaults.yaml");
    dir.copy_repo("local.yaml.example", "local.yaml");
    ConfigStore::new(dir.path())
}

fn sections(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect()
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
fn a_save_writes_the_constant_header_first() {
    let dir = TestDir::new();
    let store = seeded(&dir);

    // A no-op change: the point is the header the write emits, not what it carries.
    store.update_local(|_| {}).unwrap();

    let written = dir.read("local.yaml");
    assert!(
        written.starts_with(LOCAL_HEADER),
        "a header, not bare content"
    );
    assert!(written.contains("REWRITES this whole file"));
    assert!(written.contains("KEPT"));
    assert!(!written.contains("DROPPED"));

    // The header must not be parsed as content, and must not disturb the round trip.
    let parsed = parse_yaml(&written);
    assert_eq!(parsed.pointer("/tuner/ocr/region/left"), Some(&json!(1140)));
    assert!(
        temp_files(&dir).is_empty(),
        "a save leaves no temp file behind"
    );
}

#[test]
fn a_foreign_block_survives_a_save_verbatim() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    let foreign = "# My own notes about this machine, which nothing should delete.\n\
                   my_notes:\n  monitor: \"left\"\n  notes:\n  - and a list\n";
    dir.append("local.yaml", &format!("\n{foreign}"));

    store
        .update_local(|local| {
            local["ui"] = json!({"left": 222, "pinned": true});
        })
        .unwrap();

    let written = dir.read("local.yaml");
    assert!(
        written.contains("# My own notes about this machine"),
        "the comment travels with the block"
    );
    // Quoted in the input and quoted in the output: the block is carried as TEXT, not parsed and
    // re-emitted, so a re-serialisation cannot drop the quotes or the list style.
    assert!(written.contains("monitor: \"left\""));
    assert!(written.contains("- and a list"));
    let parsed = parse_yaml(&written);
    assert_eq!(parsed.pointer("/ui/left"), Some(&json!(222)));
}

#[test]
fn a_foreign_block_is_written_once_not_twice() {
    // The block is preserved by appending its raw TEXT after the serialised body. If the parsed body
    // still carries the block, it is emitted a second time — unquoted, without its comment, with a
    // duplicate top-level key that serde_norway tolerates (last wins), so a presence assertion alone
    // cannot see the defect. It is counted here instead.
    let dir = TestDir::new();
    let store = seeded(&dir);
    dir.append(
        "local.yaml",
        "\n# My own notes about this machine, which nothing should delete.\n\
         my_notes:\n  monitor: \"left\"\n  notes:\n  - and a list\n",
    );

    store.update_local(|_| {}).unwrap();

    let written = dir.read("local.yaml");
    assert_eq!(
        written.matches("my_notes:").count(),
        1,
        "the foreign block must not also be re-serialised into the body:\n{written}"
    );
    assert_eq!(
        written.matches("monitor: \"left\"").count(),
        1,
        "only the raw text carries the author's quoting:\n{written}"
    );
}

#[test]
fn a_second_save_does_not_grow_the_foreign_block() {
    // The first duplicate is written to disk, and the next save re-reads that file: the foreign walk
    // then finds TWO copies and appends both, so the growth is compounding rather than a one-off.
    let dir = TestDir::new();
    let store = seeded(&dir);
    dir.append(
        "local.yaml",
        "\n# My own notes about this machine, which nothing should delete.\n\
         my_notes:\n  monitor: \"left\"\n",
    );

    store
        .apply_sections(&sections(&[("ui", json!({"left": 1}))]))
        .unwrap();
    store
        .apply_sections(&sections(&[("ui", json!({"left": 2}))]))
        .unwrap();

    let written = dir.read("local.yaml");
    assert_eq!(
        written.matches("my_notes:").count(),
        1,
        "the block stayed one copy across two saves:\n{written}"
    );
    assert_eq!(
        written.matches("# My own notes about this machine").count(),
        1
    );
    assert_eq!(parse_yaml(&written).pointer("/ui/left"), Some(&json!(2)));
}

#[test]
fn a_block_the_app_owns_is_rewritten() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    dir.append("local.yaml", "\nui:\n  left: 111\n  pinned: false\n");

    store
        .update_local(|local| {
            local["ui"] = json!({"left": 222, "pinned": true});
        })
        .unwrap();

    let parsed = parse_yaml(&dir.read("local.yaml"));
    assert_eq!(parsed.pointer("/ui/left"), Some(&json!(222)));
    assert_eq!(parsed.pointer("/ui/pinned"), Some(&json!(true)));
}

#[test]
fn an_unmodelled_key_inside_an_owned_block_is_kept_not_dropped() {
    // v2 re-serialised from a typed projection, so a key it did not model inside an owned block was
    // silently dropped — the failure class CONFIG.md documents. The v3 config is opaque JSON and the
    // writer merges the block it was given, so there is no projection to drop a key: it survives,
    // and LOCAL_HEADER says so (the one deliberately reworded sentence of the ported header).
    let dir = TestDir::new();
    let store = seeded(&dir);
    dir.append("local.yaml", "\nui:\n  left: 1\n  future_key: keep\n");

    store
        .update_local(|local| {
            local["ui"]["left"] = json!(2);
        })
        .unwrap();

    let parsed = parse_yaml(&dir.read("local.yaml"));
    assert_eq!(parsed.pointer("/ui/left"), Some(&json!(2)));
    assert_eq!(parsed.pointer("/ui/future_key"), Some(&json!("keep")));
}

#[test]
fn a_save_creates_local_yaml_when_it_is_absent() {
    let dir = TestDir::new();
    let store = ConfigStore::new(dir.path());
    assert!(!dir.exists("local.yaml"));

    store
        .update_local(|local| {
            local["ui"] = json!({"left": 1});
        })
        .unwrap();

    let written = dir.read("local.yaml");
    assert!(written.starts_with(LOCAL_HEADER));
    assert_eq!(parse_yaml(&written).pointer("/ui/left"), Some(&json!(1)));
}

#[test]
fn an_unparsable_local_fails_the_save_rather_than_being_overwritten() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    let broken = "window: [unclosed\n";
    dir.write("local.yaml", broken);
    let before = dir.read_bytes("local.yaml");

    let error = store.update_local(|_| {}).unwrap_err();
    match error {
        ConfigError::Parse { path, .. } => assert!(path.ends_with("local.yaml")),
        other => panic!("expected a parse failure naming the file, got {other}"),
    }
    assert_eq!(dir.read_bytes("local.yaml"), before);
}

#[test]
fn a_failed_write_reports_the_path_and_leaves_no_litter() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    let local_path = dir.path_of("local.yaml");
    let before = dir.read_bytes("local.yaml");
    // A read-only local.yaml stands in for the medium the plan names: the atomic replace cannot
    // happen, so the write fails the way it would on a read-only stick or a full disk.
    set_readonly(&local_path, true);

    let error = store.update_local(|_| {}).unwrap_err();
    set_readonly(&local_path, false);

    match error {
        ConfigError::Write { path, .. } => assert!(path.ends_with("local.yaml")),
        other => panic!("expected a write failure naming the file, got {other}"),
    }
    // The old state is intact and the failed write left nothing for the next writer to trip over.
    assert_eq!(dir.read_bytes("local.yaml"), before);
    assert!(temp_files(&dir).is_empty(), "{:?}", temp_files(&dir));
}

#[test]
fn two_sequential_writers_keep_each_others_block() {
    let dir = TestDir::new();
    let store = seeded(&dir);

    // The first writer saves the portable half it owns.
    let config = store.load().unwrap();
    store.save_settings(&config).unwrap();

    // The second, which loaded nothing and knows only its own block.
    store
        .apply_sections(&sections(&[("ui", json!({"left": 456, "pinned": true}))]))
        .unwrap();

    let local = store.load_local().unwrap();
    assert_eq!(local.pointer("/ui/left"), Some(&json!(456)));
    assert!(local.pointer("/settings/tuner/guard_px").is_some());
    // And the merged config sees both.
    let merged = store.load().unwrap();
    assert_eq!(merged.pointer("/ui/left"), Some(&json!(456)));
    assert_eq!(merged.pointer("/tuner/guard_px"), Some(&json!(8)));
}

#[test]
fn two_concurrent_writers_keep_each_others_block() {
    let dir = TestDir::new();
    let store = Arc::new(seeded(&dir));

    let ui_writer = {
        let store = Arc::clone(&store);
        thread::spawn(move || {
            for left in 0..30 {
                store
                    .apply_sections(&sections(&[("ui", json!({"left": left}))]))
                    .unwrap();
            }
        })
    };
    let quest_writer = {
        let store = Arc::clone(&store);
        thread::spawn(move || {
            for guard_px in 0..30 {
                store
                    .apply_sections(&sections(&[("quest", json!({"guard_px": guard_px}))]))
                    .unwrap();
            }
        })
    };
    ui_writer.join().unwrap();
    quest_writer.join().unwrap();

    // Before the gated read-modify-write the second writer saved the snapshot it had loaded
    // earlier and deleted the first one's block — silently, from two threads that both reported
    // success.
    let local = store.load_local().unwrap();
    assert_eq!(local.pointer("/ui/left"), Some(&json!(29)));
    assert_eq!(local.pointer("/quest/guard_px"), Some(&json!(29)));
}

#[test]
fn apply_sections_merges_field_wise_and_returns_the_merged_block() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    // The pet feeder persists run state mid-run; a tab save must not delete it.
    dir.append(
        "local.yaml",
        "\npet:\n  boarding_window_may_be_open: true\n  close_button_png: close.png\n  action_wait_ms: 500\n",
    );

    let merged = store
        .apply_sections(&sections(&[(
            "pet",
            json!({"food_load_mode": "drag", "action_wait_ms": 700}),
        )]))
        .unwrap();

    let pet = merged.get("pet").expect("the merged pet block comes back");
    assert_eq!(
        pet.pointer("/boarding_window_may_be_open"),
        Some(&json!(true))
    );
    assert_eq!(pet.pointer("/close_button_png"), Some(&json!("close.png")));
    assert_eq!(pet.pointer("/action_wait_ms"), Some(&json!(700)));
    assert_eq!(pet.pointer("/food_load_mode"), Some(&json!("drag")));

    // And the file says the same thing, merged field-wise into the block that was there.
    let local = store.load_local().unwrap();
    assert_eq!(
        local.pointer("/pet/boarding_window_may_be_open"),
        Some(&json!(true))
    );
}

#[test]
fn apply_sections_adds_a_block_the_file_does_not_have_yet() {
    let dir = TestDir::new();
    let store = seeded(&dir);

    let merged = store
        .apply_sections(&sections(&[(
            "calibration",
            json!({"dpi_scale": 1.5, "monitor_dpi": 144}),
        )]))
        .unwrap();

    assert_eq!(
        merged
            .get("calibration")
            .and_then(|value| value.get("dpi_scale")),
        Some(&json!(1.5))
    );
    let local = store.load_local().unwrap();
    assert_eq!(local.pointer("/calibration/monitor_dpi"), Some(&json!(144)));
}

#[test]
fn apply_sections_refuses_a_block_the_app_does_not_own() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    dir.append("local.yaml", "\n# mine\nmy_notes:\n  monitor: \"left\"\n");
    let before = dir.read_bytes("local.yaml");

    let error = store
        .apply_sections(&sections(&[("my_notes", json!({"monitor": "right"}))]))
        .unwrap_err();
    match error {
        ConfigError::SectionNotOwned(name) => assert_eq!(name, "my_notes"),
        other => panic!("expected the foreign section to be refused, got {other}"),
    }
    // Nothing was written: rewriting a block the writer carries as raw text would duplicate it.
    assert_eq!(dir.read_bytes("local.yaml"), before);
}

#[test]
fn save_settings_projects_only_portable_fields_into_the_settings_block() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    let defaults_before = dir.read_bytes("defaults.yaml");
    let mut config = store.load().unwrap();
    assert!(
        config.pointer("/tuner/ocr").is_some(),
        "the machine half is loaded"
    );

    config["tuner"]["target_grade"] = json!("S");
    config["tuner"]["guard_px"] = json!(5);
    config["game"]["variant"] = json!("");
    config["window"]["title"] = json!("MY_OWN_TITLE");
    config["hotkeys"]["start"] = json!(65);
    store.save_settings(&config).unwrap();

    let local = parse_yaml(&dir.read("local.yaml"));
    assert_eq!(
        local.pointer("/settings/tuner/target_grade"),
        Some(&json!("S"))
    );
    assert_eq!(local.pointer("/settings/tuner/guard_px"), Some(&json!(5)));
    // The section is carried WHOLE: a lone leaf would drag the other defaults in with it.
    assert_eq!(
        local.pointer("/settings/tuner/max_retries"),
        Some(&json!(999_999))
    );
    // ...but only as far as the template's shape: a machine-only field cannot leak into the
    // portable half.
    assert!(local.pointer("/settings/tuner/ocr").is_none());
    assert!(local.pointer("/settings/gem/grade_positions").is_none());
    assert_eq!(
        local.pointer("/settings/window/title"),
        Some(&json!("MY_OWN_TITLE"))
    );
    assert_eq!(local.pointer("/settings/hotkeys/start"), Some(&json!(65)));

    // The machine block is untouched, the shipped template is never written, and the merged config
    // reads the new values back.
    assert_eq!(local.pointer("/tuner/ocr/region/left"), Some(&json!(1140)));
    assert_eq!(dir.read_bytes("defaults.yaml"), defaults_before);
    let reloaded = store.load().unwrap();
    assert_eq!(reloaded.pointer("/tuner/target_grade"), Some(&json!("S")));
    assert_eq!(reloaded.pointer("/tuner/guard_px"), Some(&json!(5)));
    assert_eq!(
        reloaded.pointer("/window/title"),
        Some(&json!("MY_OWN_TITLE"))
    );
    assert_eq!(reloaded.pointer("/hotkeys/start"), Some(&json!(65)));
    assert_eq!(
        reloaded.pointer("/tuner/ocr/region/left"),
        Some(&json!(1140))
    );
}

#[test]
fn save_settings_preserves_the_fold_marker_and_unknown_settings() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    let local_example = dir.read("local.yaml");
    dir.write(
        "local.yaml",
        &format!("{local_example}\nsettings:\n  legacy_folded: true\n  custom: keep-me\n"),
    );

    let config = store.load().unwrap();
    store.save_settings(&config).unwrap();

    let local = parse_yaml(&dir.read("local.yaml"));
    assert_eq!(local.pointer("/settings/legacy_folded"), Some(&json!(true)));
    assert_eq!(local.pointer("/settings/custom"), Some(&json!("keep-me")));
}

#[test]
fn save_settings_leaves_the_spammer_block_alone() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    dir.append(
        "local.yaml",
        "\nspammer:\n  active: Personal\n  presets:\n    Personal:\n      '*0': 0.2\n",
    );

    let config = store.load().unwrap();
    assert_eq!(config.pointer("/spammer/active"), Some(&json!("Personal")));
    store.save_settings(&config).unwrap();

    // The preset is still in its own block...
    let local = store.load_local().unwrap();
    assert_eq!(
        local.pointer("/spammer/presets/Personal/*0"),
        Some(&json!(0.2))
    );
    // ...and the settings block carries no spammer key, so a personal rotation can never reach the
    // portable half and from there a release.
    assert!(local.pointer("/settings/spammer").is_none());
}

#[test]
fn adoption_moves_presets_left_in_the_base_into_local_yaml() {
    let dir = TestDir::new();
    let mut tracked = repo_config_text("defaults.yaml");
    tracked
        .push_str("spammer:\n  active: Knight0-9\n  presets:\n    Knight0-9:\n      '*0': 0.2\n");
    dir.write("defaults.yaml", &tracked);
    dir.write("local.yaml", EMPTY_LOCAL);
    let store = ConfigStore::new(dir.path());

    let config = store.load().unwrap();
    assert_eq!(config.pointer("/spammer/active"), Some(&json!("Knight0-9")));
    assert_eq!(
        config.pointer("/spammer/presets/Knight0-9/*0"),
        Some(&json!(0.2))
    );

    // Adopted into the player's own file, once, because the next save of the shipped file would
    // have deleted the only copy.
    let local = store.load_local().unwrap();
    assert_eq!(local.pointer("/spammer/active"), Some(&json!("Knight0-9")));
    assert_eq!(
        local.pointer("/spammer/presets/Knight0-9/*0"),
        Some(&json!(0.2))
    );
}

#[test]
fn adoption_carries_legacy_flat_keys_through_to_local_yaml() {
    let dir = TestDir::new();
    let mut tracked = repo_config_text("defaults.yaml");
    tracked.push_str("spammer:\n  keys:\n    '*0': 0.25\n");
    dir.write("defaults.yaml", &tracked);
    dir.write("local.yaml", EMPTY_LOCAL);
    let store = ConfigStore::new(dir.path());

    let config = store.load().unwrap();
    assert_eq!(
        config.pointer("/spammer/presets/default/*0"),
        Some(&json!(0.25))
    );

    let local = store.load_local().unwrap();
    assert_eq!(
        local.pointer("/spammer/presets/default/*0"),
        Some(&json!(0.25))
    );
}

#[test]
fn adoption_leaves_a_local_spammer_block_alone_and_is_once_only() {
    let dir = TestDir::new();
    let mut tracked = repo_config_text("defaults.yaml");
    tracked
        .push_str("spammer:\n  active: Knight0-9\n  presets:\n    Knight0-9:\n      '*0': 0.2\n");
    dir.write("defaults.yaml", &tracked);
    dir.write(
        "local.yaml",
        "# mine\nspammer:\n  active: Personal\n  presets:\n    Personal:\n      F1: 1.5\n",
    );
    let before = dir.read_bytes("local.yaml");
    let store = ConfigStore::new(dir.path());

    let config = store.load().unwrap();
    assert_eq!(config.pointer("/spammer/active"), Some(&json!("Personal")));

    // The guard is the block's presence: another process may have adopted in the gap, and writing
    // ours blind would replace the winner's presets with our copy.
    assert_eq!(dir.read_bytes("local.yaml"), before);
}

#[test]
fn adoption_folds_a_legacy_startup_step_into_the_object_shape() {
    let dir = TestDir::new();
    let mut tracked = repo_config_text("defaults.yaml");
    tracked.push_str(
        "spammer:\n  active: Knight0-9\n  presets:\n    Knight0-9:\n      '*0': 0.2\n  \
         startup_step:\n    Knight0-9: 0.15\n",
    );
    dir.write("defaults.yaml", &tracked);
    dir.write("local.yaml", EMPTY_LOCAL);
    let store = ConfigStore::new(dir.path());

    store.load().unwrap();
    let local = store.load_local().unwrap();
    assert_eq!(
        local.pointer("/spammer/startup/Knight0-9/step"),
        Some(&json!(0.15))
    );
    // The legacy bare shape is never written again.
    assert!(local.pointer("/spammer/startup_step").is_none());
}

// ——— the single-update composition: `config.apply`'s owned write AND settings projection ———

#[test]
fn apply_sections_with_settings_writes_both_halves_in_one_update() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    let mut config = store.load().unwrap();
    // The caller's effective config: the owned overlay AND the portable change, as seal-core computes
    // them before the write.
    config["window"]["title"] = json!("PORTABLE_TITLE");
    if let Some(tuner) = config.get_mut("tuner").and_then(Value::as_object_mut) {
        tuner.insert("guard_px".to_owned(), json!(9));
    }
    let merged_config = config.clone();

    let returned = store
        .apply_sections_with_settings(
            &sections(&[("tuner", json!({"guard_px": 9}))]),
            &merged_config,
        )
        .expect("the composed write succeeds");

    assert_eq!(returned["tuner"]["guard_px"], 9);
    let local = parse_yaml(&dir.read("local.yaml"));
    // Both halves landed in the SAME file, in one read-modify-write.
    assert_eq!(local["tuner"]["guard_px"], 9);
    assert_eq!(local["settings"]["window"]["title"], "PORTABLE_TITLE");
    // The shipped template is untouched and no temp file was left behind.
    assert_eq!(dir.read("defaults.yaml"), repo_config_text("defaults.yaml"));
    assert!(temp_files(&dir).is_empty(), "{:?}", temp_files(&dir));
}

#[test]
fn a_composed_update_with_a_foreign_name_writes_nothing() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    let before = dir.read("local.yaml");

    let error = store
        .apply_sections_with_settings(&sections(&[("not_owned", json!({"x": 1}))]), &json!({}))
        .expect_err("a foreign block is refused");

    assert!(
        matches!(error, ConfigError::SectionNotOwned(_)),
        "{error:?}"
    );
    assert_eq!(dir.read("local.yaml"), before, "nothing was written");
}

#[test]
fn a_composed_update_whose_projection_cannot_be_built_writes_nothing() {
    let dir = TestDir::new();
    let store = seeded(&dir);
    // The projection is built from the shipped template, BEFORE the update: a missing template is a
    // failure with the file byte-identical, which is what makes PROTOCOL §4's promise true.
    std::fs::remove_file(dir.path().join("defaults.yaml")).unwrap();
    let before = dir.read("local.yaml");

    let error = store
        .apply_sections_with_settings(
            &sections(&[("tuner", json!({"guard_px": 9}))]),
            &json!({"window": {"title": "X"}}),
        )
        .expect_err("the template is gone");

    assert!(
        matches!(
            error,
            ConfigError::MissingFile(_) | ConfigError::Read { .. }
        ),
        "{error:?}"
    );
    assert_eq!(dir.read("local.yaml"), before, "nothing was written");
}

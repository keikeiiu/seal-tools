//! The three layers: the shipped base, the `local.yaml` overlay, and the OCR dictionary.
//!
//! Ported from v2's `ConfigLoaderTests` (`LoadMergesLocalOverridesIntoDefaults`,
//! `APartialSettingsBlockLeavesTheRestStanding`, `AFreshInstallWithOnlyTheShippedTemplateLoads`,
//! `LoadMissingLocalYamlThrowsConfigException`, `LoadAttributesParsesDictionary`, the variant
//! resolution pair, and the spammer shape migration). The fixtures are the repository's own
//! committed `v2/config/` files, so the real key set is exercised rather than a stub.

mod common;

use common::{EMPTY_LOCAL, TestDir, parse_yaml, repo_config_dir, repo_config_text, set_readonly};
use seal_config::{ConfigError, ConfigStore};
use serde_json::{Value, json};

#[test]
fn load_merges_the_base_and_the_overlay() {
    let dir = TestDir::new();
    dir.copy_repo("defaults.yaml", "defaults.yaml");
    dir.copy_repo("local.yaml.example", "local.yaml");

    let config = ConfigStore::new(dir.path()).load().unwrap();

    // The portable defaults, from the shipped template.
    assert_eq!(config.pointer("/window/title"), Some(&json!("TW_LIVE")));
    assert_eq!(config.pointer("/arduino/vid"), Some(&json!(9025)));
    assert_eq!(config.pointer("/tuner/target_grade"), Some(&json!("DG")));
    assert_eq!(config.pointer("/tuner/max_retries"), Some(&json!(999_999)));
    assert_eq!(
        config.pointer("/tuner/timing/click_enter_delay"),
        Some(&json!(0.4))
    );

    // Machine-specific values, from local.yaml.
    assert_eq!(config.pointer("/tuner/ocr/region/left"), Some(&json!(1140)));
    assert_eq!(
        config.pointer("/tuner/ocr/region/height"),
        Some(&json!(320))
    );
    assert_eq!(config.pointer("/tuner/ocr/row_height"), Some(&json!(25)));
    assert_eq!(
        config.pointer("/gem/grade_positions/N/0"),
        Some(&json!(727))
    );
    assert_eq!(
        config.pointer("/gem/grade_positions/N/1"),
        Some(&json!(696))
    );
    assert_eq!(config.pointer("/gem/empty_distance"), Some(&json!(0.18)));

    // The OCR dictionary is a separate file, not a layer of the config.
    assert!(config.get("attributes").is_none());
}

#[test]
fn the_shipped_template_is_the_base_when_it_exists() {
    let dir = TestDir::new();
    let template = repo_config_text("defaults.yaml");
    dir.write("defaults.yaml.example", &template);
    dir.write("local.yaml", EMPTY_LOCAL);

    let config = ConfigStore::new(dir.path()).load().unwrap();
    assert_eq!(config.pointer("/window/title"), Some(&json!("TW_LIVE")));
}

#[test]
fn the_checkout_reads_the_tracked_defaults_when_there_is_no_template() {
    let dir = TestDir::new();
    let mut tracked = repo_config_text("defaults.yaml");
    tracked.push_str("\n# a checkout edit, which is the base in a repo\n");
    tracked = tracked.replace("target_grade: DG", "target_grade: SG");
    dir.write("defaults.yaml", &tracked);
    dir.write("local.yaml", EMPTY_LOCAL);

    let config = ConfigStore::new(dir.path()).load().unwrap();
    assert_eq!(config.pointer("/tuner/target_grade"), Some(&json!("SG")));
}

#[test]
fn a_fresh_install_seeds_local_from_the_example() {
    let dir = TestDir::new();
    dir.write("defaults.yaml.example", &repo_config_text("defaults.yaml"));
    dir.copy_repo("local.yaml.example", "local.yaml.example");

    let config = ConfigStore::new(dir.path()).load().unwrap();

    // Seeded, not merely read: the file is there afterwards, verbatim through the atomic writer.
    assert!(dir.exists("local.yaml"));
    assert_eq!(
        dir.read("local.yaml"),
        repo_config_text("local.yaml.example")
    );
    assert_eq!(config.pointer("/tuner/ocr/region/left"), Some(&json!(1140)));
}

#[test]
fn a_missing_local_with_no_example_is_loud() {
    let dir = TestDir::new();
    dir.copy_repo("defaults.yaml", "defaults.yaml");

    let error = ConfigStore::new(dir.path()).load().unwrap_err();
    match error {
        ConfigError::MissingFile(path) => assert!(path.ends_with("local.yaml")),
        other => panic!("expected a missing local.yaml, got {other}"),
    }
    assert!(!dir.exists("local.yaml"));
}

#[test]
fn load_local_reads_only_the_overlay() {
    let dir = TestDir::new();
    dir.copy_repo("defaults.yaml", "defaults.yaml");
    dir.copy_repo("local.yaml.example", "local.yaml");

    let local = ConfigStore::new(dir.path()).load_local().unwrap();
    assert_eq!(local.pointer("/tuner/ocr/region/left"), Some(&json!(1140)));
    // The overlay carries no portable sections of its own: those live in the base.
    assert!(local.get("window").is_none());
}

#[test]
fn a_partial_settings_block_leaves_the_rest_standing() {
    let dir = TestDir::new();
    dir.copy_repo("defaults.yaml", "defaults.yaml");
    dir.write(
        "local.yaml",
        "settings:\n  tuner:\n    guard_px: 33\n  gem:\n    colored_gap_min: 0\n",
    );

    let config = ConfigStore::new(dir.path()).load().unwrap();

    assert_eq!(config.pointer("/tuner/guard_px"), Some(&json!(33)));
    // Stated as ZERO on purpose: 0 is a legal colored_gap_min, so a "non-empty" guard would have
    // read a deliberate value as silence and quietly left the shipped one in place.
    assert_eq!(config.pointer("/gem/colored_gap_min"), Some(&json!(0)));
    // And every value the block does not mention is untouched.
    assert_eq!(config.pointer("/tuner/max_retries"), Some(&json!(999_999)));
    assert_eq!(config.pointer("/tuner/spring_mode"), Some(&json!("hid")));
    assert_eq!(config.pointer("/tuner/timing/ocr_delay"), Some(&json!(0.8)));
    assert_eq!(config.pointer("/gem/move_mode"), Some(&json!("arduino")));
    assert_eq!(config.pointer("/window/title"), Some(&json!("TW_LIVE")));
    assert_eq!(config.pointer("/hotkeys/start"), Some(&json!(123)));
    assert_eq!(config.pointer("/pet/items_per_minute"), Some(&json!(3)));
}

#[test]
fn the_machine_half_keeps_the_last_word_over_the_settings_block() {
    let dir = TestDir::new();
    dir.copy_repo("defaults.yaml", "defaults.yaml");
    dir.write(
        "local.yaml",
        "settings:\n  tuner:\n    guard_px: 33\ntuner:\n  guard_px: 8\n",
    );

    let config = ConfigStore::new(dir.path()).load().unwrap();
    assert_eq!(config.pointer("/tuner/guard_px"), Some(&json!(8)));
    // The machine block is a partial overlay, not a replacement: the rest of the section survives.
    assert_eq!(config.pointer("/tuner/target_grade"), Some(&json!("DG")));
}

#[test]
fn blocks_of_the_players_own_are_ignored_on_load() {
    let dir = TestDir::new();
    dir.copy_repo("defaults.yaml", "defaults.yaml");
    dir.write(
        "local.yaml",
        "# A block nothing reads.\nmy_notes:\n  monitor: left\n  notes:\n  - and a list\n",
    );

    let config = ConfigStore::new(dir.path()).load().unwrap();
    assert!(config.get("my_notes").is_none());
    assert_eq!(config.pointer("/window/title"), Some(&json!("TW_LIVE")));
}

#[test]
fn legacy_flat_spammer_keys_migrate_into_a_default_preset() {
    let dir = TestDir::new();
    let mut tracked = repo_config_text("defaults.yaml");
    tracked.push_str("spammer:\n  keys:\n    '*0': 0.25\n    'F1': 5.0\n");
    dir.write("defaults.yaml", &tracked);
    dir.write("local.yaml", EMPTY_LOCAL);

    let config = ConfigStore::new(dir.path()).load().unwrap();
    assert_eq!(config.pointer("/spammer/active"), Some(&json!("default")));
    assert_eq!(
        config.pointer("/spammer/presets/default/*0"),
        Some(&json!(0.25))
    );
    assert_eq!(
        config.pointer("/spammer/presets/default/F1"),
        Some(&json!(5.0))
    );
    assert!(config.pointer("/spammer/keys").is_none());
}

#[test]
fn the_active_variant_resolves_the_window_title() {
    let dir = TestDir::new();
    let template = repo_config_text("defaults.yaml");
    dir.write("defaults.yaml", &template);
    dir.write("local.yaml", EMPTY_LOCAL);
    let store = ConfigStore::new(dir.path());

    // As shipped: tw, resolving to the title every tool reads.
    assert_eq!(
        store.load().unwrap().pointer("/window/title"),
        Some(&json!("TW_LIVE"))
    );

    // Point it at the US client and the same field follows; there is no second title.
    dir.write(
        "defaults.yaml",
        &template.replace("  variant: tw", "  variant: us"),
    );
    assert_eq!(
        store.load().unwrap().pointer("/window/title"),
        Some(&json!("US_LIVE"))
    );

    // Case is the variants map's business, not the player's.
    dir.write(
        "defaults.yaml",
        &template.replace("  variant: tw", "  variant: TW"),
    );
    assert_eq!(
        store.load().unwrap().pointer("/window/title"),
        Some(&json!("TW_LIVE"))
    );

    // An unset or UNKNOWN variant changes NOTHING: the configured title has to stand.
    for replacement in ["  variant: nope", "  variant: \"\""] {
        dir.write(
            "defaults.yaml",
            &template.replace("  variant: tw", replacement),
        );
        assert_eq!(
            store.load().unwrap().pointer("/window/title"),
            Some(&json!("TW_LIVE")),
            "replacement {replacement:?}"
        );
    }
}

#[test]
fn load_attributes_parses_the_committed_dictionary() {
    let dir = TestDir::new();
    dir.copy_repo("attributes.yaml", "attributes.yaml");

    let attributes = ConfigStore::new(dir.path()).load_attributes().unwrap();

    assert_eq!(
        attributes.pointer("/attributes/0/name"),
        Some(&json!("攻擊力"))
    );
    let per_level_stats = attributes
        .pointer("/per_level_stats")
        .and_then(Value::as_array)
        .expect("per_level_stats is a list");
    assert!(per_level_stats.iter().any(|stat| stat == "力量"));
    let text_fixes = attributes
        .pointer("/text_fixes/whole")
        .and_then(Value::as_array)
        .expect("text_fixes.whole is a list");
    assert!(!text_fixes.is_empty(), "the dictionary carries text fixes");
}

#[test]
fn a_missing_dictionary_is_seeded_from_its_template() {
    let dir = TestDir::new();
    dir.write(
        "attributes.us.yaml.example",
        "attributes: []\nper_level_stats: []\nnegative_attr_markers: []\ntext_fixes:\n  whole: []\n",
    );
    let store = ConfigStore::new(dir.path());

    let attributes = store.load_attributes_named("attributes.us.yaml").unwrap();
    assert_eq!(attributes.pointer("/attributes"), Some(&json!([])));
    // Seeded, not merely read: the file is there afterwards, and it is the one that gets filled.
    assert!(dir.exists("attributes.us.yaml"));

    // Second direction: once it exists it is the player's file, and a seed never overwrites it.
    dir.write(
        "attributes.us.yaml",
        "attributes:\n  - {name: \"Mine\", category: \"atk\", variants: [\"Mine\"]}\n",
    );
    let attributes = store.load_attributes_named("attributes.us.yaml").unwrap();
    assert_eq!(
        attributes.pointer("/attributes/0/name"),
        Some(&json!("Mine"))
    );
}

#[test]
fn a_missing_dictionary_with_no_template_is_loud() {
    let dir = TestDir::new();
    dir.write("local.yaml", EMPTY_LOCAL);

    let error = ConfigStore::new(dir.path())
        .load_attributes_named("attributes.us.yaml")
        .unwrap_err();
    match error {
        ConfigError::MissingFile(path) => assert!(path.ends_with("attributes.us.yaml")),
        other => panic!("expected the missing dictionary to be named, got {other}"),
    }
}

#[test]
fn a_parse_failure_names_the_file() {
    let dir = TestDir::new();
    dir.write("defaults.yaml", "window: [unclosed\n");
    dir.write("local.yaml", EMPTY_LOCAL);

    let error = ConfigStore::new(dir.path()).load().unwrap_err();
    let message = error.to_string();
    assert!(message.contains("defaults.yaml"), "{message}");
    assert!(message.contains("failed to parse"), "{message}");
}

#[test]
fn the_fixture_files_are_found_by_the_repository_walk() {
    // Guards the helper the other tests depend on: if the walk broke, this names it instead of
    // every fixture test failing with a confusing parse error.
    assert!(repo_config_dir().join("defaults.yaml").is_file());
    assert!(repo_config_dir().join("local.yaml.example").is_file());
}

#[test]
fn the_written_settings_block_is_readable_yaml() {
    // A save's output has to be a file the next load can read; this pins the shape without a save
    // (the writer tests cover the writer itself).
    let dir = TestDir::new();
    dir.copy_repo("defaults.yaml", "defaults.yaml");
    let text = repo_config_text("local.yaml.example");
    dir.write("local.yaml", &text);
    let parsed = parse_yaml(&text);
    assert_eq!(parsed.pointer("/tuner/ocr/row_height"), Some(&json!(25)));

    // A read-only local.yaml is a read the loader must still be able to do.
    set_readonly(&dir.path_of("local.yaml"), true);
    let config = ConfigStore::new(dir.path()).load().unwrap();
    assert_eq!(config.pointer("/window/title"), Some(&json!("TW_LIVE")));
    set_readonly(&dir.path_of("local.yaml"), false);
}

#[test]
fn a_hand_edited_empty_body_is_still_a_null_in_the_merge() {
    // `tuner:` with no body loads as null, and the merge must read that as "not stated" rather
    // than as an empty object that blanks the section.
    let dir = TestDir::new();
    dir.copy_repo("defaults.yaml", "defaults.yaml");
    dir.write("local.yaml", "tuner:\ngem:\nui:\n");

    let config = ConfigStore::new(dir.path()).load().unwrap();
    assert_eq!(config.pointer("/tuner/target_grade"), Some(&json!("DG")));
    assert_eq!(config.pointer("/gem/start_grade"), Some(&json!("N")));
}

//! The one-time fold: an install that has taken an update gets the shipped template beside the
//! player's own old `defaults.yaml`, and the first launch moves the values they CHANGED into
//! `local.yaml`'s `settings:` block once, then stops reading the old file.
//!
//! Ported from v2's `ConfigLoaderTests` fold section: `TheFoldCarriesOnlyWhatThePlayerChanged`,
//! `TheFoldMarksItselfDoneEvenWhenNothingDiffers`, `TheFoldRunsOnceAndTheLegacyFileIsNeverReadAgain`,
//! `AChangedShippedDefaultIsCarriedAcrossAsThePlayersOwn`, `TheFoldLeavesTheMachineHalfOfLocalYaml`,
//! `AFoldThatCannotBeWrittenStillLoadsTheLegacyValues`,
//! `ACheckoutWithNoShippedTemplateReadsExactlyWhatItDidBefore`,
//! `AFreshInstallWithOnlyTheShippedTemplateLoads`, plus the rules `PLAN-DEPLOY.md` §4/§6 add: only
//! leaves the legacy file states are folded, a section is written whole, and the marker is set in
//! the same write even when nothing differs.

mod common;

use common::{TestDir, parse_yaml, repo_config_text, set_readonly};
use seal_config::{ConfigError, ConfigStore};
use serde_json::{Value, json};

/// The shipped template as this release packages it. The repo tracks it as `defaults.yaml`; the zip
/// carries it as `defaults.yaml.example`, which is the file an updated install reads as its base.
fn shipped() -> String {
    repo_config_text("defaults.yaml")
}

/// An install AFTER an update has landed: the shipped template beside the player's own legacy
/// `defaults.yaml`, and a seeded `local.yaml`. `legacy` defaults to the shipped text, so a test names
/// only the values it wants the player to have changed.
fn install(dir: &TestDir, shipped_text: &str, legacy: Option<&str>) {
    dir.write("defaults.yaml.example", shipped_text);
    dir.write("defaults.yaml", legacy.unwrap_or(shipped_text));
    dir.copy_repo("local.yaml.example", "local.yaml");
}

fn temp_files(dir: &TestDir) -> Vec<String> {
    std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect()
}

/// True when a settings section carries no value at all. This writer emits an unset value as a
/// `null`, so "the fold did not bring this section across" is tested as "carries nothing", not as
/// "the key is absent".
fn carries_nothing(node: Option<&Value>) -> bool {
    match node {
        None | Some(Value::Null) => true,
        Some(Value::Object(map)) => map.is_empty(),
        Some(Value::Array(values)) => values.is_empty(),
        Some(_) => false,
    }
}

#[test]
fn the_fold_carries_only_what_the_player_changed() {
    let shipped_text = shipped();
    let dir = TestDir::new();
    install(
        &dir,
        &shipped_text,
        Some(&shipped_text.replace("target_grade: DG", "target_grade: S")),
    );
    let legacy_before = dir.read_bytes("defaults.yaml");
    let store = ConfigStore::new(dir.path());

    let config = store.load().unwrap();
    assert_eq!(config.pointer("/tuner/target_grade"), Some(&json!("S")));

    let local = parse_yaml(&dir.read("local.yaml"));
    let settings = local.get("settings");
    assert!(settings.is_some(), "the fold writes the settings block");
    // The section holding the change was carried WHOLE — the apply side assigns the object, so a
    // lone `target_grade` leaf would have dragged the other twelve tuner defaults in with it.
    assert!(!carries_nothing(local.pointer("/settings/tuner")));
    assert_eq!(
        local.pointer("/settings/tuner/target_grade"),
        Some(&json!("S"))
    );
    assert_eq!(
        local.pointer("/settings/tuner/ocr_retries"),
        Some(&json!(3))
    );
    // And the sections that do NOT differ are not carried at all, which is the point of the whole
    // design: they go on tracking the shipped template, so a default a later release changes still
    // reaches this machine.
    for section in [
        "window",
        "reference_window",
        "game",
        "arduino",
        "hotkeys",
        "gem",
        "pet",
    ] {
        assert!(
            carries_nothing(local.pointer(&format!("/settings/{section}"))),
            "{section} should not have been carried"
        );
    }

    // The legacy file is the rollback copy: never written, never moved, never deleted.
    assert_eq!(dir.read_bytes("defaults.yaml"), legacy_before);
}

#[test]
fn the_fold_marks_itself_done_even_when_nothing_differs() {
    // A player who never changed a value: nothing to carry, and the marker still has to be set — or
    // the next release's launch would fold this template against that release's example and pin
    // every default it changed as the player's own.
    let shipped_text = shipped();
    let dir = TestDir::new();
    install(&dir, &shipped_text, None);
    ConfigStore::new(dir.path()).load().unwrap();

    let local = parse_yaml(&dir.read("local.yaml"));
    assert_eq!(local.pointer("/settings/legacy_folded"), Some(&json!(true)));
    assert!(carries_nothing(local.pointer("/settings/tuner")));
    assert!(carries_nothing(local.pointer("/settings/window")));
}

#[test]
fn the_fold_runs_once_and_the_legacy_file_is_never_read_again() {
    let shipped_text = shipped();
    let dir = TestDir::new();
    install(
        &dir,
        &shipped_text,
        Some(&shipped_text.replace("target_grade: DG", "target_grade: S")),
    );
    let store = ConfigStore::new(dir.path());

    assert_eq!(
        store.load().unwrap().pointer("/tuner/target_grade"),
        Some(&json!("S"))
    );
    let after_first_load = dir.read_bytes("local.yaml");

    // A second launch writes nothing at all: the marker means the legacy file is not read.
    assert_eq!(
        store.load().unwrap().pointer("/tuner/target_grade"),
        Some(&json!("S"))
    );
    assert_eq!(dir.read_bytes("local.yaml"), after_first_load);

    // Which is decisive with the legacy file GONE: the value cannot have come from it.
    std::fs::remove_file(dir.path_of("defaults.yaml")).unwrap();
    assert_eq!(
        store.load().unwrap().pointer("/tuner/target_grade"),
        Some(&json!("S"))
    );
}

#[test]
fn a_changed_shipped_default_is_carried_across_as_the_players_own() {
    // The fold's approximation, pinned so nobody later mistakes it for a bug: it reads "the player
    // changed it" as "it differs from the shipped template", so a default a RELEASE changed, under
    // a key this player never touched, is carried across as if they had chosen it. The direction is
    // the safe one — a saved value beats a silent behaviour change.
    let shipped_now = shipped().replace("target_grade: DG", "target_grade: S");
    let legacy_from_the_old_release = shipped();
    let dir = TestDir::new();
    install(&dir, &shipped_now, Some(&legacy_from_the_old_release));

    assert_eq!(
        ConfigStore::new(dir.path())
            .load()
            .unwrap()
            .pointer("/tuner/target_grade"),
        Some(&json!("DG"))
    );
}

#[test]
fn the_fold_leaves_the_machine_half_of_local_yaml_alone() {
    let shipped_text = shipped();
    let dir = TestDir::new();
    install(
        &dir,
        &shipped_text,
        Some(&shipped_text.replace("target_grade: DG", "target_grade: S")),
    );
    let store = ConfigStore::new(dir.path());
    let before = store.load_local().unwrap();
    let ocr_left = before.pointer("/tuner/ocr/region/left").cloned();
    let grade_positions = before
        .pointer("/gem/grade_positions")
        .and_then(Value::as_object)
        .map(|positions| positions.len());

    store.load().unwrap();

    // The fold writes local.yaml, which is also where this machine's calibration lives — the one
    // thing in the install that cannot be regenerated from the repo.
    let after = store.load_local().unwrap();
    assert_eq!(after.pointer("/tuner/ocr/region/left").cloned(), ocr_left);
    assert_eq!(
        after
            .pointer("/gem/grade_positions")
            .and_then(Value::as_object)
            .map(|positions| positions.len()),
        grade_positions
    );
    assert!(after.get("settings").is_some());
}

#[test]
fn a_fold_that_cannot_be_written_still_loads_the_legacy_values() {
    let shipped_text = shipped();
    let dir = TestDir::new();
    install(
        &dir,
        &shipped_text,
        Some(&shipped_text.replace("target_grade: DG", "target_grade: S")),
    );
    let local_path = dir.path_of("local.yaml");
    let untouched = dir.read_bytes("local.yaml");
    // A read-only local.yaml stands in for the medium the plan names: the atomic replace cannot
    // happen, so the fold fails the way it would on a read-only stick or a full disk.
    set_readonly(&local_path, true);

    let config = ConfigStore::new(dir.path()).load().unwrap();
    set_readonly(&local_path, false);

    // Nothing throws, and the legacy values are still in force for this session from the fallback
    // layer...
    assert_eq!(config.pointer("/tuner/target_grade"), Some(&json!("S")));
    // ...the marker was not set, so the fold retries on the next launch...
    assert_eq!(dir.read_bytes("local.yaml"), untouched);
    // ...and the failed write left no litter.
    assert!(temp_files(&dir).is_empty(), "{:?}", temp_files(&dir));
}

#[test]
fn a_checkout_with_no_shipped_template_does_not_fold() {
    // No defaults.yaml.example: a repo checkout, and every install that has not taken an update.
    let dir = TestDir::new();
    let tracked = shipped().replace("target_grade: DG", "target_grade: S");
    dir.write("defaults.yaml", &tracked);
    dir.copy_repo("local.yaml.example", "local.yaml");
    let store = ConfigStore::new(dir.path());

    let config = store.load().unwrap();
    assert_eq!(config.pointer("/tuner/target_grade"), Some(&json!("S")));
    // No fold ran: without a shipped template there are not two files to compare.
    assert!(store.load_local().unwrap().get("settings").is_none());
}

#[test]
fn a_fresh_install_with_only_the_shipped_template_does_not_fold() {
    let dir = TestDir::new();
    dir.write("defaults.yaml.example", &shipped());
    dir.copy_repo("local.yaml.example", "local.yaml.example");
    let store = ConfigStore::new(dir.path());

    let config = store.load().unwrap();
    assert_eq!(config.pointer("/window/title"), Some(&json!("TW_LIVE")));
    // local.yaml was seeded, and no fold ran: there is no legacy file to fold.
    assert!(dir.exists("local.yaml"));
    assert!(store.load_local().unwrap().get("settings").is_none());
}

#[test]
fn an_already_folded_install_never_reads_the_legacy_file() {
    let shipped_text = shipped();
    let dir = TestDir::new();
    install(
        &dir,
        &shipped_text,
        Some(&shipped_text.replace("target_grade: DG", "target_grade: S")),
    );
    // The marker plus a value only the settings block could have supplied: if the legacy file were
    // still read, `S` would win; the marker is what makes the legacy values invisible.
    dir.append(
        "local.yaml",
        "\nsettings:\n  legacy_folded: true\n  tuner:\n    target_grade: XG\n",
    );
    let store = ConfigStore::new(dir.path());

    assert_eq!(
        store.load().unwrap().pointer("/tuner/target_grade"),
        Some(&json!("XG"))
    );
    std::fs::remove_file(dir.path_of("defaults.yaml")).unwrap();
    assert_eq!(
        store.load().unwrap().pointer("/tuner/target_grade"),
        Some(&json!("XG"))
    );
}

#[test]
fn the_fold_carries_only_leaves_the_legacy_file_states() {
    // An absent key folds nothing, so a class default can never enter local.yaml wearing the
    // player's name. The legacy file here states one tuner leaf; only that leaf is compared and only
    // that leaf lands.
    let shipped_text = shipped();
    let dir = TestDir::new();
    install(&dir, &shipped_text, Some("tuner:\n  target_grade: S\n"));

    let config = ConfigStore::new(dir.path()).load().unwrap();
    assert_eq!(config.pointer("/tuner/target_grade"), Some(&json!("S")));
    // The leaves the legacy file never stated keep tracking the template.
    assert_eq!(config.pointer("/tuner/max_retries"), Some(&json!(999999)));
    assert_eq!(config.pointer("/tuner/ocr_retries"), Some(&json!(3)));

    let local = parse_yaml(&dir.read("local.yaml"));
    assert_eq!(
        local.pointer("/settings/tuner/target_grade"),
        Some(&json!("S"))
    );
    assert!(
        local.pointer("/settings/tuner/ocr_retries").is_none(),
        "a leaf the legacy file does not state has no destination to land in"
    );
}

#[test]
fn the_fold_projects_onto_the_templates_shape() {
    // A hand-added machine field inside a portable section must not ride into the portable half: the
    // template decides which leaves are portable.
    let shipped_text = shipped();
    let legacy = shipped_text.replace(
        "  items_per_minute: 3",
        "  items_per_minute: 5\n  close_button_png: machine.png",
    );
    let dir = TestDir::new();
    install(&dir, &shipped_text, Some(&legacy));

    let config = ConfigStore::new(dir.path()).load().unwrap();
    assert_eq!(config.pointer("/pet/items_per_minute"), Some(&json!(5)));
    assert!(config.pointer("/pet/close_button_png").is_none());

    let local = parse_yaml(&dir.read("local.yaml"));
    assert_eq!(
        local.pointer("/settings/pet/items_per_minute"),
        Some(&json!(5))
    );
    assert!(local.pointer("/settings/pet/close_button_png").is_none());
}

#[test]
fn presets_left_in_the_legacy_file_are_adopted_into_local_yaml_in_the_same_load() {
    // Both once-only writes can happen on the same launch: the fold moves the portable settings, and
    // the existing adoption moves the personal rotations out of the old shipped file. The adoption's
    // guard then finds the spammer block populated and skips on a later launch.
    let shipped_text = shipped();
    let legacy = format!(
        "{shipped_text}spammer:\n  active: Knight0-9\n  presets:\n    Knight0-9:\n      '*0': 0.2\n"
    );
    let dir = TestDir::new();
    install(&dir, &shipped_text, Some(&legacy));
    let store = ConfigStore::new(dir.path());

    let config = store.load().unwrap();
    assert_eq!(config.pointer("/spammer/active"), Some(&json!("Knight0-9")));
    assert_eq!(
        config.pointer("/spammer/presets/Knight0-9/*0"),
        Some(&json!(0.2))
    );

    let local = parse_yaml(&dir.read("local.yaml"));
    assert_eq!(local.pointer("/settings/legacy_folded"), Some(&json!(true)));
    assert_eq!(
        local.pointer("/spammer/presets/Knight0-9/*0"),
        Some(&json!(0.2))
    );

    // The legacy file keeps its block as the rollback copy.
    assert!(dir.read("defaults.yaml").contains("Knight0-9"));
}

#[test]
fn a_fold_is_one_write_containing_the_sections_and_the_marker() {
    let shipped_text = shipped();
    let dir = TestDir::new();
    install(
        &dir,
        &shipped_text,
        Some(&shipped_text.replace("target_grade: DG", "target_grade: S")),
    );
    let store = ConfigStore::new(dir.path());

    store.load().unwrap();

    // One file, one header: the marker and the carried section arrived in the same write, so a
    // crash can never leave the marker set without its values (or the values without the marker).
    let written = dir.read("local.yaml");
    assert_eq!(written.matches("legacy_folded").count(), 1);
    assert!(written.contains("target_grade: S"));
    assert_eq!(written.matches(&seal_config::LOCAL_HEADER).count(), 1);
    assert_eq!(
        store
            .load_local()
            .unwrap()
            .pointer("/settings/legacy_folded"),
        Some(&json!(true))
    );
}

#[test]
fn a_second_fold_does_not_re_run_when_the_example_changes_later() {
    // A folder restored from before a release, or simply the next release: once the marker is set,
    // a changed template is a changed DEFAULT, not the player's value.
    let old_shipped = shipped();
    let dir = TestDir::new();
    install(
        &dir,
        &old_shipped,
        Some(&old_shipped.replace("target_grade: DG", "target_grade: S")),
    );
    let store = ConfigStore::new(dir.path());
    store.load().unwrap();
    let after_first_load = dir.read_bytes("local.yaml");

    // The next release changes a default in a section the first fold did NOT carry (only `tuner`
    // differed, so `hotkeys` still tracks the template). If the fold re-ran it would compare the
    // legacy file's 123 against the new template's 124 and pin 123 as the player's own.
    let new_shipped = old_shipped.replace("  start: 123", "  start: 124");
    dir.write("defaults.yaml.example", &new_shipped);
    let config = store.load().unwrap();

    assert_eq!(config.pointer("/tuner/target_grade"), Some(&json!("S")));
    assert_eq!(config.pointer("/hotkeys/start"), Some(&json!(124)));
    // No second write happened at all: the marker stopped the legacy file being read.
    assert_eq!(dir.read_bytes("local.yaml"), after_first_load);
    assert!(
        parse_yaml(&dir.read("local.yaml"))
            .pointer("/settings/hotkeys")
            .is_none()
    );
}

#[test]
fn a_fold_never_touches_a_foreign_block() {
    let shipped_text = shipped();
    let dir = TestDir::new();
    install(
        &dir,
        &shipped_text,
        Some(&shipped_text.replace("target_grade: DG", "target_grade: S")),
    );
    dir.append(
        "local.yaml",
        "\n# my own block\nmy_notes:\n  monitor: \"left\"\n",
    );

    ConfigStore::new(dir.path()).load().unwrap();

    let written = dir.read("local.yaml");
    assert!(written.contains("# my own block"));
    assert!(written.contains("monitor: \"left\""));
}

#[test]
fn the_fold_reports_a_write_failure_rather_than_throwing_a_typed_error() {
    // The fold is the one stated exception to "a write that cannot happen throws": it returns to the
    // caller as "not folded", and the load still succeeds. Pinned separately from the readonly test
    // so the contract, not just the file state, is asserted.
    let shipped_text = shipped();
    let dir = TestDir::new();
    install(
        &dir,
        &shipped_text,
        Some(&shipped_text.replace("target_grade: DG", "target_grade: S")),
    );
    set_readonly(&dir.path_of("local.yaml"), true);
    let result = ConfigStore::new(dir.path()).load();
    set_readonly(&dir.path_of("local.yaml"), false);
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn a_parse_failure_in_local_yaml_fails_the_fold_loudly() {
    // The fold's one caught failure is an unwritable destination. A local.yaml that cannot be parsed
    // is a different thing: it must be reported, never overwritten.
    let shipped_text = shipped();
    let dir = TestDir::new();
    install(
        &dir,
        &shipped_text,
        Some(&shipped_text.replace("target_grade: DG", "target_grade: S")),
    );
    let broken = "window: [unclosed\n";
    dir.write("local.yaml", broken);
    let before = dir.read_bytes("local.yaml");

    let error = ConfigStore::new(dir.path()).load().unwrap_err();
    assert!(matches!(error, ConfigError::Parse { .. }), "{error}");
    assert_eq!(dir.read_bytes("local.yaml"), before);
}

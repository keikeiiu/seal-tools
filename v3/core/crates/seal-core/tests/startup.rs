//! Startup validation (G4) and the variant dictionary switch (OPEN §3), wired for real.
//!
//! v2's `ConfigLoader.Load` ended with `ConfigValidator.Validate(AppConfig)` and a failure was a
//! `ConfigException` before OCR or the COM port existed. Here the same ported rules run in
//! `CoreState::load`, before any command is served; the outcome for the binary is a non-zero exit
//! with the v2 text on stderr (`PROTOCOL.md` §1's core-crash channel — there is no protocol shape for
//! "config invalid" yet, which is reported rather than invented).
//!
//! The dictionary half is the same all-or-nothing rule v2's `SwitchVariant` documents: the variant is
//! resolved first, its dictionary is loaded second, and the config is only left switched once that
//! load succeeded. A failed dictionary load must NOT silently keep the old one as if switched —
//! `reload` commits nothing on failure.

mod common;

use std::fs;
use std::process::Command;

use common::{BASE_ATTRIBUTES, TestDir, config_root};
use seal_core::dispatch::CoreState;
use seal_core::startup::{self, StartupError};
use serde_json::json;

/// A valid, calibrated fixture with a `game:` block naming two clients, each with its own dictionary
/// that differs in a field the test can see (`per_level_stats`).
fn variant_root() -> TestDir {
    let root = TestDir::new();
    root.write(
        "config/defaults.yaml",
        &format!(
            "{}
game:
  variant: tw
  variants:
    tw:
      title: \"Seal Online TW\"
      attributes: attributes.tw.yaml
    us:
      title: \"Seal Online US\"
      attributes: attributes.us.yaml
",
            common::BASE_CONFIG
        ),
    );
    root.write("config/local.yaml", "{}");
    root.write(
        "config/attributes.tw.yaml",
        "per_level_stats:\n  - tw_only\n",
    );
    root.write(
        "config/attributes.us.yaml",
        "per_level_stats:\n  - us_only\n",
    );
    root
}

#[test]
fn a_valid_config_boots_with_the_default_dictionary() {
    let root = config_root();

    let state = CoreState::load(root.path().to_path_buf()).expect("the fixture is valid");

    assert_eq!(state.dictionary, "attributes.yaml");
    assert_eq!(state.variant, None);
    assert!(
        state.attributes.attributes.is_empty(),
        "the empty dictionary parses as empty, not as an error"
    );
}

#[test]
fn an_invalid_config_is_refused_with_v2s_exception_text() {
    // The old minimal fixture: no arduino, no tuner geometry, no gem section. v2's initialisers make
    // those DEFAULT objects, and the rules refuse them.
    let root = TestDir::new();
    root.write("config/defaults.yaml", "window:\n  title: TEST_TITLE\n");
    root.write("config/local.yaml", "{}");
    root.write("config/attributes.yaml", BASE_ATTRIBUTES);

    let error = match CoreState::load(root.path().to_path_buf()) {
        Ok(_) => panic!("an invalid config must not load"),
        Err(error) => error,
    };

    let StartupError::Invalid { lines, .. } = &error else {
        panic!("expected validation lines, got {error:?}");
    };
    assert!(!lines.is_empty());
    let message = error.message();
    assert!(
        message.starts_with("Config validation failed:"),
        "{message}"
    );
    assert!(
        message.contains("Check defaults.yaml and local.yaml."),
        "{message}"
    );
    // The v2 rule set's own lines are carried, not a summary.
    assert!(
        lines
            .iter()
            .any(|line| line.to_lowercase().contains("tuner")),
        "the tuner failures are listed: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.to_lowercase().contains("arduino")),
        "the arduino failure is listed: {lines:?}"
    );
}

#[test]
fn a_bare_section_is_skipped_the_way_v2_skips_it() {
    // v2's reflection walk skips a null section (`ConfigValidator.cs:84`); the merged JSON keeps the
    // null, and the mapping must not turn it into a default object that then fails.
    let (input, _) = startup::validation_input(&json!({"window": null}));
    assert_eq!(input.window, None);

    let (absent, _) = startup::validation_input(&json!({}));
    assert_eq!(absent.window, Some(Default::default()));
}

#[test]
fn an_unknown_variant_changes_nothing() {
    let active = startup::active(&json!({
        "game": {"variant": "nope", "variants": {"tw": {"attributes": "attributes.tw.yaml"}}}
    }));

    assert_eq!(active.variant, None);
    assert_eq!(active.dictionary, "attributes.yaml");
}

#[test]
fn the_variant_name_matches_without_case() {
    let active = startup::active(&json!({
        "game": {"variant": "TW", "variants": {"tw": {"attributes": "attributes.tw.yaml"}}}
    }));

    assert_eq!(active.variant.as_deref(), Some("tw"));
    assert_eq!(active.dictionary, "attributes.tw.yaml");
}

#[test]
fn the_variant_selects_its_own_dictionary() {
    let root = variant_root();

    let state = CoreState::load(root.path().to_path_buf()).expect("the variant fixture is valid");

    assert_eq!(state.variant.as_deref(), Some("tw"));
    assert_eq!(state.dictionary, "attributes.tw.yaml");
    assert_eq!(state.attributes.per_level_stats, vec!["tw_only"]);
    // The title came from the variant (seal-config resolves it), not the base window block.
    assert_eq!(state.config["window"]["title"], "Seal Online TW");
}

#[test]
fn a_missing_dictionary_refuses_to_boot_with_the_loaders_line() {
    let root = config_root();
    fs::remove_file(root.path_of("config/attributes.yaml")).expect("remove the dictionary");

    let error = match CoreState::load(root.path().to_path_buf()) {
        Ok(_) => panic!("a missing dictionary must not boot"),
        Err(error) => error,
    };

    assert!(
        matches!(error, StartupError::Dictionary { .. }),
        "{error:?}"
    );
    assert!(
        error.message().contains("attributes.yaml"),
        "the line names the file: {}",
        error.message()
    );
}

#[test]
fn reload_switches_the_dictionary_with_the_variant() {
    let root = variant_root();
    let mut state = CoreState::load(root.path().to_path_buf()).expect("boots on tw");
    assert_eq!(state.attributes.per_level_stats, vec!["tw_only"]);

    root.write("config/local.yaml", "settings:\n  game:\n    variant: us\n");
    state.reload().expect("the us dictionary exists");

    assert_eq!(state.variant.as_deref(), Some("us"));
    assert_eq!(state.dictionary, "attributes.us.yaml");
    assert_eq!(state.attributes.per_level_stats, vec!["us_only"]);
}

#[test]
fn a_failed_reload_keeps_the_working_dictionary_rather_than_half_switching() {
    let root = variant_root();
    let mut state = CoreState::load(root.path().to_path_buf()).expect("boots on tw");

    // Point the config at the US variant and remove its dictionary: the load fails, and nothing —
    // variant, dictionary, config or attributes — may be half-applied.
    root.write("config/local.yaml", "settings:\n  game:\n    variant: us\n");
    fs::remove_file(root.path_of("config/attributes.us.yaml")).expect("remove us dictionary");

    let error = state.reload().expect_err("the dictionary is gone");

    assert!(
        matches!(error, StartupError::Dictionary { .. }),
        "{error:?}"
    );
    assert_eq!(state.variant.as_deref(), Some("tw"));
    assert_eq!(state.dictionary, "attributes.tw.yaml");
    assert_eq!(state.attributes.per_level_stats, vec!["tw_only"]);
    assert_eq!(state.config["game"]["variant"], "tw");
}

#[test]
fn an_invalid_config_exits_non_zero_with_the_v2_text_on_stderr() {
    // The protocol has no shape for a config the core refuses to boot on, so the defined outcome is
    // §1's crash channel: nothing on stdout (the shell is waiting for a hello response), the reason
    // on stderr, and a non-zero exit.
    let root = TestDir::new();
    root.write("config/defaults.yaml", "window:\n  title: TEST_TITLE\n");
    root.write("config/local.yaml", "{}");
    root.write("config/attributes.yaml", BASE_ATTRIBUTES);

    let output = Command::new(env!("CARGO_BIN_EXE_seal-core"))
        .arg("--root")
        .arg(root.path())
        .output()
        .expect("the core binary runs");

    assert!(!output.status.success(), "an invalid config must not boot");
    assert!(
        output.stdout.is_empty(),
        "no protocol bytes before the handshake: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Config validation failed:"), "{stderr}");
    assert!(
        stderr.contains("Check defaults.yaml and local.yaml."),
        "{stderr}"
    );
}

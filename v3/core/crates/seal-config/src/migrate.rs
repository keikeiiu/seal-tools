//! Increment 5 of PLAN-PORTABLE-SETUP: the one-time migration from today's `local.yaml`.
//!
//! The migration is additive and inspectable, and it is the ONLY code that writes the new tree for
//! an existing install (`config.apply` keeps using the old store until the switch):
//!
//! 1. back up the whole `config/` folder to `config/.migration-backup-<stamp>/`;
//! 2. split `local.yaml` through the same writer routing increment 3 uses — machine blocks to
//!    `machine/`, portable `settings:` to `setup/`, whole-block files to their one owner;
//! 3. move every top-level block the app does not own to `user.yaml` (verbatim data; the core never
//!    reads it);
//! 4. load the new tree through the increment-2 loader and verify it succeeds;
//! 5. retire the old `local.yaml` (it is already inside the backup), then write `meta.yaml` with the
//!    fold/layout markers LAST, so a crash before the marker makes the retry run again rather than
//!    leaving a half-tagged tree.
//!
//! Idempotence: a `meta.yaml` with `layout_version` means "already migrated" and the function is a
//! no-op. A crash after retirement but before the marker is recognised by "no local.yaml and setup
//! files exist" and only writes the marker. Base64 crops stay inline until increment 6.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use crate::error::ConfigError;
use crate::file;
use crate::layout;
use crate::registry;
use crate::yaml;

/// The layout version stamped into `meta.yaml`.
pub const LAYOUT_VERSION: u64 = 1;
const LOCAL: &str = "local.yaml";
const META: &str = "meta.yaml";
const USER: &str = "user.yaml";
const BACKUP_PREFIX: &str = ".migration-backup-";

/// What a migration did (or that it was already done).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    pub backup: PathBuf,
    pub files: Vec<&'static str>,
    pub user_blocks: Vec<String>,
    /// Crops that could not be decoded and stayed inline (lossless fallback, never silent).
    pub warnings: Vec<String>,
}

fn stamp() -> String {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    format!("{seconds:x}-{sequence:x}")
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        if name_text.starts_with(BACKUP_PREFIX) || name_text == crate::transaction::JOURNAL_DIR {
            continue;
        }
        let target = to.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Increment 6: move the four inline crop groups into `config/artifacts/*.png` and leave small
/// `artifacts/<name>.png` references in the config. A crop whose base64 does not decode is LEFT
/// INLINE and named in the warnings: losing a player's picture to a broken transition is worse than
/// a large machine file.
fn extract_crops(root: &Path, local: &mut Map<String, Value>) -> Vec<String> {
    let mut warnings = Vec::new();
    let Some(pet) = local.get_mut("pet").and_then(Value::as_object_mut) else {
        return warnings;
    };
    for (field, name) in [
        ("close_button_png", "pet_close_button.png"),
        ("food_icon_png", "pet_food_icon.png"),
    ] {
        extract_one(root, pet, field, name, &mut warnings);
    }
    if let Some(slots) = pet.get_mut("slots").and_then(Value::as_array_mut) {
        for (index, slot) in slots.iter_mut().enumerate() {
            if let Some(object) = slot.as_object_mut() {
                extract_one(
                    root,
                    object,
                    "pet_slot_empty_png",
                    &format!("pet_slot_empty_{}.png", index + 1),
                    &mut warnings,
                );
            }
        }
    }
    if let Some(queue) = pet.get_mut("queue").and_then(Value::as_array_mut) {
        for (index, entry) in queue.iter_mut().enumerate() {
            if let Some(object) = entry.as_object_mut() {
                extract_one(
                    root,
                    object,
                    "png",
                    &format!("pet_queue_{}.png", index + 1),
                    &mut warnings,
                );
            }
        }
    }
    warnings
}

fn extract_one(
    root: &Path,
    object: &mut Map<String, Value>,
    field: &str,
    name: &str,
    warnings: &mut Vec<String>,
) {
    let Some(value) = object.get(field).and_then(Value::as_str) else {
        return;
    };
    let value = value.to_owned();
    if value.trim().is_empty() || value.starts_with(crate::ARTIFACT_PREFIX) {
        return;
    }
    let payload = value.rsplit(',').next().unwrap_or(&value).trim();
    let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, payload)
    else {
        warnings.push(format!(
            "pet.{field} is not valid base64; it was kept inline in machine/pet.yaml"
        ));
        return;
    };
    let path = root.join(crate::ARTIFACT_PREFIX).join(name);
    match file::write_bytes(&path, &bytes) {
        Ok(()) => {
            object.insert(
                field.to_owned(),
                Value::String(format!("{}{name}", crate::ARTIFACT_PREFIX)),
            );
        }
        Err(error) => warnings.push(format!(
            "pet.{field} could not be written to artifacts/{name}: {error}; it was kept inline"
        )),
    }
}

fn meta(folded: bool) -> Value {
    json!({
        "layout_version": LAYOUT_VERSION,
        "legacy_folded": folded,
        "migrated_at": stamp(),
    })
}

/// True when the overlay carried the once-only fold marker into the migration. `ConfigStore::load`
/// runs the legacy fold before calling this function, so a shipped install answers `true`; a caller
/// that migrates an unfolded tree directly stamps `false`, which is the truth rather than a claim
/// (HW-F3).
fn fold_marker(local: &Map<String, Value>) -> bool {
    local
        .get("settings")
        .and_then(|settings| settings.get("legacy_folded"))
        == Some(&Value::Bool(true))
}

/// True when `meta.yaml` already says the tree was migrated.
fn already_migrated(root: &Path) -> Result<bool, ConfigError> {
    let Some(meta) = file::read(root, def(META)?)? else {
        return Ok(false);
    };
    Ok(meta
        .get("layout_version")
        .and_then(Value::as_u64)
        .is_some_and(|version| version >= LAYOUT_VERSION))
}

/// The registry entry for one of this module's constant file names. `MissingFile` is the honest
/// error if the registry ever loses one: the migration names only files it ships.
fn def(path: &str) -> Result<&'static registry::FileDef, ConfigError> {
    registry::FILES
        .iter()
        .find(|file| file.path == path)
        .ok_or_else(|| ConfigError::MissingFile(PathBuf::from(path)))
}

fn setup_files_exist(root: &Path) -> bool {
    registry::portable_files().any(|file| file::path_of(root, file).is_file())
}

fn write_user_blocks(root: &Path, blocks: &Map<String, Value>) -> Result<(), ConfigError> {
    if blocks.is_empty() {
        return Ok(());
    }
    let yaml = yaml::to_string(&Value::Object(blocks.clone())).map_err(|message| {
        ConfigError::Serialize {
            path: root.join(USER),
            message,
        }
    })?;
    let text = format!(
        "# ── Your own notes / extra blocks — the core never reads this file ───────────────\n\
         # Moved here by the one-time layout migration. Anything you add survives every save.\n\
         \n{yaml}"
    );
    file::write_text(&root.join(USER), &text)
}

/// Migrate one config root. `base` is the parsed shipped default template. Returns `None` when the
/// tree was already migrated.
///
/// The new tree is verified BEFORE the old file is retired: it must load back to the same values the
/// legacy loader served, and a mismatch restores every written file from the backup and refuses with
/// the differing JSON pointers (HW-F4).
///
/// # Errors
///
/// Returns [`ConfigError`] when the backup copy, a read or parse of `local.yaml`/the template, a
/// routed write, or the verify-before-retire comparison fails. A crop that cannot be decoded does
/// not fail the migration: it stays inline and is reported in [`MigrationReport::warnings`].
pub fn migrate(root: &Path, base: &Value) -> Result<Option<MigrationReport>, ConfigError> {
    if already_migrated(root)? {
        return Ok(None);
    }
    let local_path = root.join(LOCAL);
    if !local_path.is_file() {
        // A crash after retirement but before the marker: the tree exists, only tag it.
        if setup_files_exist(root) {
            file::write(root, def(META)?, &meta(true))?;
        }
        return Ok(None);
    }

    let backup = root.join(format!("{BACKUP_PREFIX}{}", stamp()));
    copy_tree(root, &backup).map_err(|source| ConfigError::Write {
        path: backup.clone(),
        source,
    })?;

    let text = fs::read_to_string(&local_path).map_err(|source| ConfigError::Read {
        path: local_path.clone(),
        source,
    })?;
    let local = yaml::parse(&text).map_err(|message| ConfigError::Parse {
        path: local_path.clone(),
        message,
    })?;
    let mut local = match local {
        Value::Null => Map::new(),
        Value::Object(local) => local,
        _ => {
            return Err(ConfigError::Parse {
                path: local_path,
                message: "local.yaml must be a YAML mapping".to_owned(),
            });
        }
    };

    let warnings = extract_crops(root, &mut local);
    // The marker the new tree will carry: what the overlay actually states, not a hard-coded claim.
    let folded = fold_marker(&local);
    let owned = crate::OWNED_LOCAL_BLOCKS;
    let mut sections = Map::new();
    let mut foreign = Map::new();
    for (key, value) in &local {
        if owned.contains(&key.as_str()) || key == "settings" {
            sections.insert(key.clone(), value.clone());
        } else {
            foreign.insert(key.clone(), value.clone());
        }
    }
    // The fold marker is bookkeeping, not a portable setting; keep it out of the routed sections.
    if let Some(settings) = sections.get_mut("settings").and_then(Value::as_object_mut) {
        settings.remove("legacy_folded");
    }
    let written = layout::write_sections(root, &sections)?;
    write_user_blocks(root, &foreign)?;

    // Verify BEFORE retiring the old file: the new tree must load back to the settings the old
    // loader served. `layout::load` always starts from the base, so "it loaded" proves nothing; the
    // comparison is what catches a routing gap that dropped a block (HW-F4). The expected value is
    // computed from the SAME overlay the router saw — crops already rewritten to artifact
    // references, so that documented difference cannot hide a real loss.
    let mut written_files = written.files;
    if !foreign.is_empty() {
        written_files.push(USER);
    }
    let expected = crate::store::legacy_merged_config(base, &Value::Object(local));
    let loaded = layout::load(root, base)?;
    let losses = lost_values(&expected, &loaded.config);
    if !losses.is_empty() {
        let rollback_failed = restore_from_backup(root, &backup, &written_files);
        let message = if rollback_failed.is_empty() {
            String::new()
        } else {
            format!(
                " The previous files could not all be restored: {}.",
                rollback_failed.join("; ")
            )
        };
        return Err(ConfigError::MigrationMismatch {
            differences: losses,
            backup,
            message,
        });
    }

    // Retire the old file: it is already inside the backup directory (copy_tree ran first).
    fs::remove_file(&local_path).map_err(|source| ConfigError::Write {
        path: local_path,
        source,
    })?;
    file::write(root, def(META)?, &meta(folded))?;
    let mut files = written_files;
    files.push(META);
    files.sort_unstable();
    Ok(Some(MigrationReport {
        backup,
        files,
        user_blocks: foreign.keys().cloned().collect(),
        warnings,
    }))
}

/// How many losses one refusal names. Enough to identify the shape of the bug, few enough to stay
/// readable in the startup line.
const MAX_DIFFERENCES: usize = 8;

/// The values the old loader served that the new tree does NOT reproduce — the migration's
/// verify-before-retire comparison (HW-F4). A missing or changed value is a loss and refuses the
/// migration; an EXTRA value is not: the router may keep a setting the current template no longer
/// lists, and refusing an upgrade because the new tree preserved more than the old one served would
/// strand the player's own data. Object keys are walked, and an array with a different length is
/// reported at the array rather than at every shifted index.
fn lost_values(expected: &Value, actual: &Value) -> Vec<String> {
    let mut losses = Vec::new();
    collect_losses(expected, actual, "", &mut losses);
    losses
}

fn collect_losses(expected: &Value, actual: &Value, pointer: &str, out: &mut Vec<String>) {
    if out.len() >= MAX_DIFFERENCES {
        return;
    }
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            for (key, value) in expected {
                let child = format!("{pointer}/{key}");
                match actual.get(key) {
                    Some(actual) => collect_losses(value, actual, &child, out),
                    None => out.push(format!("{child}: missing from the migrated tree")),
                }
                if out.len() >= MAX_DIFFERENCES {
                    return;
                }
            }
        }
        (Value::Array(expected), Value::Array(actual)) => {
            if expected.len() != actual.len() {
                out.push(format!(
                    "{}: {} entries became {}",
                    label(pointer),
                    expected.len(),
                    actual.len()
                ));
                return;
            }
            for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
                collect_losses(expected, actual, &format!("{pointer}/{index}"), out);
                if out.len() >= MAX_DIFFERENCES {
                    return;
                }
            }
        }
        // An extra key on a new side that is not an object reaches this arm too; it is not a loss,
        // so only an expected value that changed is reported.
        _ if expected != actual => out.push(format!(
            "{}: expected {expected}, the migrated tree loaded {actual}",
            label(pointer)
        )),
        _ => {}
    }
}

fn label(pointer: &str) -> &str {
    if pointer.is_empty() {
        "(root)"
    } else {
        pointer
    }
}

/// Put every file the migration wrote back the way the backup holds it, so a refused migration
/// leaves the tree as the old loader saw it. A file the backup does not have did not exist before,
/// so it is removed; the backup itself stays untouched as the record. Failures are collected rather
/// than aborting, because the returned error already carries the mismatch that caused this.
fn restore_from_backup(root: &Path, backup: &Path, files: &[&str]) -> Vec<String> {
    let mut failures = Vec::new();
    for path in files {
        let target = root.join(path);
        let source = backup.join(path);
        let result = if source.is_file() {
            fs::copy(&source, &target).map(|_| ())
        } else {
            match fs::remove_file(&target) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        };
        if let Err(error) = result {
            failures.push(format!("{path}: {error}"));
        }
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_and_array_losses_are_named_by_their_json_pointer() {
        let expected = json!({
            "hotkeys": {"start": 99},
            "gem": {"grades": ["N", "G"]},
            "window": {"title": "OLD"},
        });
        let actual = json!({
            "hotkeys": {"start": 0},
            "gem": {"grades": ["N"]},
            "window": {"title": "OLD", "extra": true},
            "unexpected": 1,
        });

        let losses = lost_values(&expected, &actual);

        assert!(
            losses.iter().any(|line| line.starts_with("/hotkeys/start")),
            "{losses:?}"
        );
        assert!(
            losses.iter().any(|line| line.starts_with("/gem/grades:")),
            "an array length change is one line, not every shifted index: {losses:?}"
        );
        assert!(
            !losses.iter().any(|line| line.starts_with("/window/extra")),
            "a value the new tree preserves that the old one did not serve is not a loss: {losses:?}"
        );
        assert!(
            !losses.iter().any(|line| line.starts_with("/unexpected")),
            "an extra top-level block is not a loss either: {losses:?}"
        );
        assert!(
            !losses.iter().any(|line| line.starts_with("/window/title")),
            "an equal field produces no noise: {losses:?}"
        );
    }

    #[test]
    fn an_identical_tree_has_no_losses() {
        let value = json!({"a": [1, {"b": null}], "c": "x"});
        assert_eq!(lost_values(&value, &value.clone()), Vec::<String>::new());
    }

    #[test]
    fn a_wildly_diverged_tree_is_capped_so_the_error_stays_readable() {
        let expected = Value::Object(
            (0..50)
                .map(|key| (key.to_string(), Value::from(key)))
                .collect(),
        );
        let actual = json!({});
        let losses = lost_values(&expected, &actual);
        assert_eq!(losses.len(), MAX_DIFFERENCES);
    }
}

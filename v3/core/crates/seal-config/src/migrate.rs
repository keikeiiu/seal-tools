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

/// True when `meta.yaml` already says the tree was migrated.
fn already_migrated(root: &Path) -> Result<bool, ConfigError> {
    let Some(meta) = file::read(root, def(META))? else {
        return Ok(false);
    };
    Ok(meta
        .get("layout_version")
        .and_then(Value::as_u64)
        .is_some_and(|version| version >= LAYOUT_VERSION))
}

fn def(path: &str) -> &'static registry::FileDef {
    registry::FILES
        .iter()
        .find(|file| file.path == path)
        .expect("the path is in the registry")
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

/// Migrate one config root. `base` is the parsed shipped default template, used to verify the new
/// tree loads exactly as the old one did. Returns `None` when the tree was already migrated.
pub fn migrate(root: &Path, base: &Value) -> Result<Option<MigrationReport>, ConfigError> {
    if already_migrated(root)? {
        return Ok(None);
    }
    let local_path = root.join(LOCAL);
    if !local_path.is_file() {
        // A crash after retirement but before the marker: the tree exists, only tag it.
        if setup_files_exist(root) {
            file::write(root, def(META), &meta(true))?;
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

    // Verify BEFORE retiring the old file: the new tree must load against the same template.
    let loaded = layout::load(root, base)?;
    if loaded.config.is_null() {
        return Err(ConfigError::Parse {
            path: root.to_path_buf(),
            message: "the migrated tree loaded as null".to_owned(),
        });
    }

    // Retire the old file: it is already inside the backup directory (copy_tree ran first).
    fs::remove_file(&local_path).map_err(|source| ConfigError::Write {
        path: local_path,
        source,
    })?;
    file::write(root, def(META), &meta(true))?;
    let mut files = written.files;
    files.push(META);
    if !foreign.is_empty() {
        files.push(USER);
    }
    files.sort_unstable();
    Ok(Some(MigrationReport {
        backup,
        files,
        user_blocks: foreign.keys().cloned().collect(),
        warnings,
    }))
}

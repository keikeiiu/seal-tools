//! Increment 2 of PLAN-PORTABLE-SETUP: the multi-file loader.
//!
//! The live `local.yaml` store is still the served path (increments 3–5 move the writer and the
//! migration); this module is the read half of the new layout and is what those increments test
//! against. Rules, from the registry:
//!
//! - load order is the registry's, never filesystem order: shipped defaults first, then `setup/`,
//!   then `machine/`. A missing file contributes nothing, exactly like a key absent from today's
//!   overlay;
//! - merging is the same field-wise overlay every config path already uses: objects merge, scalars
//!   and lists REPLACE, and `null` means "not stated";
//! - one file owns one path, so the order only matters between the shipped base and a stated
//!   override; no two data files write the same key;
//! - a malformed file, or one whose `schema_version` is newer, fails the load loudly with its path —
//!   the loader never guesses and never half-loads;
//! - a `.yaml` in `config/setup/` or `config/machine/` that the registry does not name is reported
//!   as a warning rather than silently ignored (a typo'd file would otherwise look like a save that
//!   did nothing);
//! - the active game variant resolves `window.title` last, the same single-field rewrite the old
//!   loader performs, so no call site has to know a variant exists.

use std::path::Path;

use serde_json::{Map, Value};

use crate::error::ConfigError;
use crate::file;
use crate::registry;

/// What the new layout read: the merged config (exactly the shape `hello` serves today) and any
/// advisory warnings the shell/log should show.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Loaded {
    pub config: Value,
    pub warnings: Vec<String>,
}

/// Load the new tree over a shipped base. `base` is the parsed `defaults.yaml.example`; the caller
/// keeps owning which template is current (the old store's template/adoption rules are unchanged
/// until the migration increment).
///
/// # Errors
///
/// Returns [`ConfigError`] when an interrupted save cannot be recovered, when a registered file
/// cannot be read or parsed, when its shape is wrong, or when its `schema_version` is newer than
/// this core understands.
pub fn load(root: &Path, base: &Value) -> Result<Loaded, ConfigError> {
    crate::transaction::recover(root)?;
    let mut config = base.clone();
    if !config.is_object() {
        config = Value::Object(Map::new());
    }
    let mut warnings = Vec::new();
    for def in registry::load_order() {
        if def.class != registry::Class::Setup && def.class != registry::Class::Machine {
            continue;
        }
        let Some(contents) = file::read(root, def)? else {
            continue;
        };
        merge_overlay(&mut config, &contents);
    }
    warnings.extend(unknown_files(root));
    resolve_variant(&mut config);
    Ok(Loaded { config, warnings })
}

/// Field-wise merge with the WRITE deletion rule: objects recurse, everything else replaces, `null`
/// is silence, and the named RECORD MAPS are replaced WHOLE. A tab deletes an entry by omitting it
/// from `presets`/`sequences`; a field-wise merge would keep the old key and resurrect it on reload
/// (HW-F1). These are the only maps whose contents are player-owned records rather than a fixed
/// schema, so only they take the whole-map rule.
pub fn merge_with_policy(target: &mut Value, overlay: &Value) {
    merge_at(target, overlay);
}

const RECORD_MAPS: &[&str] = &["presets", "sequences"];

fn merge_at(target: &mut Value, overlay: &Value) {
    match overlay {
        Value::Object(overlay_object) => match target {
            Value::Object(target_object) => {
                for (key, value) in overlay_object {
                    if value.is_null() {
                        continue;
                    }
                    if RECORD_MAPS.contains(&key.as_str()) && value.is_object() {
                        // Replacement, not merge: an omission inside a record map is a deletion.
                        target_object.insert(key.clone(), value.clone());
                        continue;
                    }
                    match target_object.get_mut(key) {
                        Some(existing) => merge_at(existing, value),
                        None => {
                            target_object.insert(key.clone(), value.clone());
                        }
                    }
                }
            }
            _ => *target = overlay.clone(),
        },
        Value::Null => {}
        _ => *target = overlay.clone(),
    }
}

fn merge_overlay(target: &mut Value, overlay: &Value) {
    merge_with_policy(target, overlay);
}

/// `.yaml` files in the two layout folders that no registry entry names. A file the migration or a
/// hand-edit added is still readable; ignoring it silently would make the player think a save went
/// nowhere, so it is reported.
fn unknown_files(root: &Path) -> Vec<String> {
    let mut warnings = Vec::new();
    for folder in ["setup", "machine"] {
        let Ok(entries) = std::fs::read_dir(root.join(folder)) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                is_yaml(&name).then(|| format!("{folder}/{name}"))
            })
            .collect();
        names.sort();
        for name in names {
            if !registry::FILES.iter().any(|def| def.path == name) {
                warnings.push(format!(
                    "{name} is not in the config registry, so it was not loaded (a typo in the file \
                     name, or a file from a newer layout?)"
                ));
            }
        }
    }
    warnings
}

/// Case-insensitive `.yaml` test. Windows hands back whatever case the player saved under, and the
/// loader would not load `Setup.YAML` either, so the "unregistered file" warning must not miss it.
fn is_yaml(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("yaml"))
}

/// The one post-merge rewrite: the active variant supplies `window.title`, exactly as the old
/// loader's variant step does. A config stating no variant keeps the title it already has.
fn resolve_variant(config: &mut Value) {
    let Some(variant) = config
        .pointer("/game/variant")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    let Some(title) = config
        .pointer("/game/variants")
        .and_then(|variants| variants.get(&variant))
        .and_then(|variant| variant.get("title"))
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    let Some(object) = config.as_object_mut() else {
        return;
    };
    let window = object
        .entry("window".to_owned())
        .or_insert_with(json_object);
    if let Some(window) = window.as_object_mut() {
        window.insert("title".to_owned(), Value::String(title));
    }
}

fn json_object() -> Value {
    Value::Object(Map::new())
}

/// What one routed save wrote: the files that changed, in registry order. Increment 3 has no
/// two-file transaction yet — a caller saving the Setup/Pet cards will get both files written
/// sequentially — but the routing itself is the contract every later increment tests against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeSummary {
    pub files: Vec<&'static str>,
}

impl ChangeSummary {
    #[must_use]
    pub fn changed(&self, path: &str) -> bool {
        self.files.contains(&path)
    }
}

/// Route a `config.apply`-shaped payload (block name → block value) to the files that own each
/// field, one atomic write per file. `settings` is the OLD portable overlay spelling: its sections
/// route to `setup/` files at the top level, and `legacy_folded` routes to `meta.yaml`.
///
/// A path no registry entry owns is refused with [`ConfigError::SectionNotOwned`] before ANY file is
/// written, so routing cannot half-apply a save. Fields inside one block may route to different
/// files (`buy_sell.presets` setup, `buy_sell.bag_grid` machine); a field with no specific owner
/// falls back to its block's owner, which is how `calibration`/`spammer`/`quest` route wholesale.
///
/// # Errors
///
/// Returns [`ConfigError::SectionNotOwned`] when a block or field has no registry owner (nothing is
/// written in that case), and the read/encode/write or transaction errors of the writes that follow.
pub fn write_sections(
    root: &Path,
    sections: &Map<String, Value>,
) -> Result<ChangeSummary, ConfigError> {
    let mut groups: std::collections::BTreeMap<&'static str, (&'static registry::FileDef, Value)> =
        std::collections::BTreeMap::new();
    // Validate and route into per-file fragments first. A failure leaves the tree untouched.
    for (block, value) in sections {
        if block == "settings" {
            route_settings(block, value, &mut groups)?;
        } else {
            route_block(block, value, &mut groups)?;
        }
    }
    if groups.is_empty() {
        return Ok(ChangeSummary::default());
    }
    let mut writes = Vec::with_capacity(groups.len());
    for (def, fragment) in groups.values() {
        let mut content = match file::read(root, def)? {
            Some(content) => content,
            None => Value::Object(Map::new()),
        };
        merge_overlay(&mut content, fragment);
        writes.push(crate::transaction::PendingWrite { file: def, content });
    }
    // One file is one atomic write. Two files (the Setup/Pet card saves) go through the journal so a
    // crash between the renames restores BOTH files rather than leaving a half-applied save.
    if writes.len() == 1 {
        file::write(root, writes[0].file, &writes[0].content)?;
    } else {
        crate::transaction::commit(root, &writes)?;
    }
    Ok(ChangeSummary {
        files: writes.iter().map(|write| write.file.path).collect(),
    })
}

fn route_settings(
    block: &str,
    value: &Value,
    groups: &mut std::collections::BTreeMap<&'static str, (&'static registry::FileDef, Value)>,
) -> Result<(), ConfigError> {
    let Some(sections) = value.as_object() else {
        return Err(ConfigError::SectionNotOwned(block.to_owned()));
    };
    for (section, section_value) in sections {
        if section == "legacy_folded" {
            // `meta.yaml` holds the fold marker at its top level, not under a `meta:` block. Its
            // registry entry is the owner of this one legacy-model path.
            let Some(meta_def) = registry::owner_of("settings.legacy_folded") else {
                return Err(ConfigError::SectionNotOwned(
                    "settings.legacy_folded".to_owned(),
                ));
            };
            let meta = groups
                .entry(meta_def.path)
                .or_insert_with(|| (meta_def, Value::Object(Map::new())));
            if let Some(meta) = meta.1.as_object_mut() {
                meta.insert("legacy_folded".to_owned(), section_value.clone());
            }
            continue;
        }
        let Some(fields) = section_value.as_object() else {
            route_field(section, section, section_value.clone(), groups)?;
            continue;
        };
        for (field, field_value) in fields {
            let canonical = format!("{section}.{field}");
            route_canonical(&canonical, section, field, field_value.clone(), groups)?;
        }
    }
    Ok(())
}

fn route_block(
    block: &str,
    value: &Value,
    groups: &mut std::collections::BTreeMap<&'static str, (&'static registry::FileDef, Value)>,
) -> Result<(), ConfigError> {
    let Some(fields) = value.as_object() else {
        route_field(block, block, value.clone(), groups)?;
        return Ok(());
    };
    if fields.is_empty() {
        // An empty section writes nothing; it states no values.
        return Ok(());
    }
    for (field, field_value) in fields {
        let canonical = format!("{block}.{field}");
        route_canonical(&canonical, block, field, field_value.clone(), groups)?;
    }
    Ok(())
}

/// Resolve the owner of a canonical path, preferring the most specific registry claim and falling
/// back to the block's own owner (a whole-block file such as `machine/display.yaml`).
fn route_canonical(
    canonical: &str,
    block: &str,
    field: &str,
    value: Value,
    groups: &mut std::collections::BTreeMap<&'static str, (&'static registry::FileDef, Value)>,
) -> Result<(), ConfigError> {
    let owner = registry::owner_of(canonical).or_else(|| registry::owner_of(block));
    let Some(owner) = owner else {
        return Err(ConfigError::SectionNotOwned(canonical.to_owned()));
    };
    insert_fragment(groups, owner, block, field, value);
    Ok(())
}

fn route_field(
    block: &str,
    field: &str,
    value: Value,
    groups: &mut std::collections::BTreeMap<&'static str, (&'static registry::FileDef, Value)>,
) -> Result<(), ConfigError> {
    let Some(owner) = registry::owner_of(block) else {
        return Err(ConfigError::SectionNotOwned(block.to_owned()));
    };
    insert_fragment(groups, owner, block, field, value);
    Ok(())
}

fn insert_fragment(
    groups: &mut std::collections::BTreeMap<&'static str, (&'static registry::FileDef, Value)>,
    owner: &'static registry::FileDef,
    block: &str,
    field: &str,
    value: Value,
) {
    let fragment = groups
        .entry(owner.path)
        .or_insert_with(|| (owner, Value::Object(Map::new())));
    let Some(fragment) = fragment.1.as_object_mut() else {
        return;
    };
    let block_value = fragment
        .entry(block.to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(block_value) = block_value.as_object_mut() {
        block_value.insert(field.to_owned(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn scratch() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join(format!("seal-config-layout-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write(root: &Path, name: &str, text: &str) {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn base() -> Value {
        json!({
            "window": {"title": "DEFAULT_TITLE"},
            "game": {
                "variant": "tw",
                "variants": {"tw": {"title": "TW_LIVE"}}
            },
            "tuner": {
                "target_grade": "N",
                "ocr": {"row_height": 10, "grade_area": {"x1": 1, "y1": 2, "x2": 3, "y2": 4}}
            },
            "buy_sell": {"presets": {}, "sell_cap": 10}
        })
    }

    #[test]
    fn setup_then_machine_merge_field_wise_and_the_variant_resolves_last() {
        let root = scratch();
        write(
            &root,
            "setup/common.yaml",
            "game:\n  variant: tw\nhotkeys:\n  start: 123\n",
        );
        write(
            &root,
            "machine/tuner.yaml",
            "tuner:\n  ocr:\n    row_height: 19\n",
        );
        let loaded = load(&root, &base()).unwrap();
        assert_eq!(loaded.config["hotkeys"]["start"], 123, "setup applied");
        assert_eq!(
            loaded.config["tuner"]["ocr"]["row_height"], 19,
            "machine applied"
        );
        assert_eq!(
            loaded.config["tuner"]["ocr"]["grade_area"]["x1"], 1,
            "a field the machine file does not state keeps the base"
        );
        assert_eq!(
            loaded.config["window"]["title"], "TW_LIVE",
            "the variant resolves the title after every file"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_missing_tree_is_just_the_shipped_base_with_the_variant_resolved() {
        let root = scratch();
        let loaded = load(&root, &base()).unwrap();
        let mut expected = base();
        expected["window"]["title"] = json!("TW_LIVE");
        assert_eq!(loaded.config, expected);
        assert!(
            loaded.warnings.is_empty(),
            "a clean load reports no warnings"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_malformed_file_fails_loudly_with_its_path() {
        let root = scratch();
        write(&root, "setup/common.yaml", "window: [unclosed\n");
        let error = load(&root, &base()).expect_err("malformed is loud");
        assert!(error.to_string().contains("setup/common.yaml"), "{error}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_newer_schema_version_is_refused_by_the_loader() {
        let root = scratch();
        write(
            &root,
            "machine/tuner.yaml",
            "schema_version: 99\ntuner: {}\n",
        );
        let error = load(&root, &base()).expect_err("a newer version is refused");
        assert!(error.to_string().contains("schema_version 99"), "{error}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_unregistered_yaml_file_is_a_warning_not_a_silent_ignore() {
        let root = scratch();
        write(&root, "setup/common.yaml", "hotkeys: {start: 1}\n");
        write(&root, "setup/legacy.yaml", "whatever: true\n");
        write(&root, "machine/notes.txt", "not yaml");
        let loaded = load(&root, &base()).unwrap();
        assert_eq!(loaded.warnings.len(), 1, "{:?}", loaded.warnings);
        assert!(loaded.warnings[0].contains("setup/legacy.yaml"));
        assert_eq!(loaded.config["hotkeys"]["start"], 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn every_registered_data_file_exists_in_the_registry_order() {
        let order: Vec<&str> = registry::load_order().map(|def| def.path).collect();
        let first_setup = order
            .iter()
            .position(|path| path.starts_with("setup/"))
            .expect("setup files load");
        let first_machine = order
            .iter()
            .position(|path| path.starts_with("machine/"))
            .expect("machine files load");
        assert!(first_setup < first_machine);
        assert_eq!(order[0], "defaults.yaml.example");
    }
}

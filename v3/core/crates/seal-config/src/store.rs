//! Reading and writing the config folder: the three layers, the seed, the fold, and the gated
//! read-modify-write every save goes through.
//!
//! One [`ConfigStore`] is one `config/` directory. It holds no config in memory: every read goes to
//! disk and every write re-reads under the save gate, which is v2's `UpdateLocal` shape and the fix
//! for the lost-block bug that file paid for four times.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde_json::{Map, Value};

use crate::OWNED_LOCAL_BLOCKS;
use crate::error::ConfigError;
use crate::yaml;
use crate::{file, registry};

const DEFAULTS: &str = "defaults.yaml";
const DEFAULTS_TEMPLATE: &str = "defaults.yaml.example";
const LOCAL: &str = "local.yaml";
const LOCAL_EXAMPLE: &str = "local.yaml.example";
const ATTRIBUTES: &str = "attributes.yaml";
const SETTINGS: &str = "settings";
const LEGACY_FOLDED: &str = "legacy_folded";
const META: &str = "meta.yaml";

/// The machine-side pet RUN state a live feeder may update mid-run. This is deliberately not a
/// whole `pet` block: a field the run does not own can never be touched by this writer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PetRunStatePatch {
    pub food_slots_used: Option<i32>,
    /// Zero-based `(page, cell)` pairs replacing `pet.food_slots`; when present the writer also
    /// resets `food_slots_used` to 0 (the set changed, so a carried count would skip/repeat cells).
    pub food_slots: Option<Vec<(usize, usize)>>,
    /// `(row index, boarding_running)` in the existing `pet.slots` order.
    pub boarding_running: Vec<(usize, bool)>,
    /// `Some(None)` clears a learned crop; `Some(Some(png))` sets it.
    pub close_button_png: Option<Option<String>>,
    pub boarding_window_may_be_open: Option<bool>,
}

impl PetRunStatePatch {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.food_slots_used.is_none()
            && self.food_slots.is_none()
            && self.boarding_running.is_empty()
            && self.close_button_png.is_none()
            && self.boarding_window_may_be_open.is_none()
    }
}

/// One writer at a time, and a temp name nobody else can be holding.
///
/// v2 made the save gate a `static` object for exactly this reason: two `ConfigLoader` instances
/// (the launcher's UI thread and a tool's own state write) must not interleave. The gate spans the
/// WHOLE read-modify-write — a caller says what to change, never what the whole file should
/// contain — so the second writer sees the first writer's block instead of deleting it.
static SAVE_GATE: Mutex<()> = Mutex::new(());

/// The `config/` folder, read and written the way v2's `ConfigLoader` does it.
///
/// Cheap to clone (it is a path); clones share the process-wide save gate, which is the property
/// that makes two store objects safe together. Writers from two PROCESSES are not serialised here —
/// v2's gate is in-process too, and `PLAN-DEPLOY.md` §6.7 records the two-installs rule.
#[derive(Debug, Clone)]
pub struct ConfigStore {
    dir: PathBuf,
}

impl ConfigStore {
    /// Points at a `config/` directory. Nothing is read or created until a method is called.
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: config_dir.into(),
        }
    }

    /// The `config/` directory this store reads and writes.
    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.dir
    }

    /// The path of a named file inside the config folder, for callers that own sibling files (the
    /// calibration PNGs, the OCR models).
    #[must_use]
    pub fn path_of(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    /// Loads the three layers and returns the merged config as opaque JSON.
    ///
    /// Order, weakest source first, exactly as `ConfigLoader.Load` documents it:
    ///
    /// 1. the **base** — `defaults.yaml.example` when it exists (an install that has taken an
    ///    update), otherwise `defaults.yaml` (a checkout);
    /// 2. the **legacy layer** — a `defaults.yaml` beside the template, read ONCE, and only until
    ///    the fold has moved the player's own values into `local.yaml`;
    /// 3. `local.yaml` — first its `settings:` block (the portable half), then its machine blocks
    ///    (this machine's own measurements), so the machine keeps the last word;
    /// 4. the active **game variant** resolves `window.title`, so every reader keeps using one
    ///    field and no call site has to know a variant exists.
    ///
    /// A fresh install seeds `local.yaml` from `local.yaml.example` first, through the same atomic
    /// writer as every other save.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the base/template or `local.yaml` cannot be read or parsed,
    /// when the one-time fold fails with anything but a write error, or when the portable layout's
    /// load fails.
    /// Loads the config — see [`Self::load_detailed`] — and returns the merged value alone. Callers
    /// that can show an advisory line (the shell's log, the core's startup) should use
    /// [`Self::load_detailed`] so a quarantined machine file is not silent (HW-F15).
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] for the same reasons as [`Self::load_detailed`].
    pub fn load(&self) -> Result<Value, ConfigError> {
        Ok(self.load_detailed()?.config)
    }

    /// Loads the config and the advisory warnings the load produced. The warnings today are a
    /// machine file that could not be parsed and was quarantined aside (HW-F15); a load that
    /// succeeds with one has already moved the bad file, so the tool surfaces normally and the
    /// player is told what happened.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the base/template or `local.yaml` cannot be read or parsed,
    /// when the one-time fold fails with anything but a write error, or when the portable layout's
    /// load fails.
    pub fn load_detailed(&self) -> Result<crate::layout::Loaded, ConfigError> {
        // A SHIPPED install (it carries the template) with a legacy local.yaml is migrated on the
        // next boot: the migration backs the folder up and writes the LAST marker, so this is the
        // one-time switch. A development checkout has only defaults.yaml and stays on the legacy
        // path, which is what keeps the repo's own tree from being rewritten.
        // The shipped install is recognisable by BOTH templates: the zip stages defaults.yaml as
        // defaults.yaml.example and always carries local.yaml.example. Requiring both keeps a test
        // fixture or a bare checkout from being migrated behind the owner's back.
        if self.path_of(DEFAULTS_TEMPLATE).is_file()
            && self.path_of(LOCAL_EXAMPLE).is_file()
            && self.path_of(LOCAL).is_file()
            && !self.is_new_layout()
        {
            let base = self.read_yaml(DEFAULTS_TEMPLATE)?;
            // The legacy fold has to happen BEFORE the migration: the migration routes local.yaml
            // and retires it, so anything still only in defaults.yaml would never be read again
            // (HW-F3).
            self.fold_legacy_before_migration(&base)?;
            crate::migrate::migrate(&self.dir, &base)?;
        }
        // The portable layout (PLAN-PORTABLE-SETUP) is the live path once `meta.yaml` says the tree
        // was migrated. A dev tree without it keeps the legacy single-file path below.
        if self.is_new_layout() {
            let base = self.load_base_template()?;
            return crate::layout::load(&self.dir, &base);
        }
        // THE BASE. An install that has taken an update has the shipped template — the zip carries
        // the defaults as config/defaults.yaml.example precisely so that extracting an update
        // cannot cover the player's own defaults.yaml — and reads that. A checkout has only the
        // tracked file, so a dev run reads exactly what it read before.
        let base_is_shipped = self.path_of(DEFAULTS_TEMPLATE).is_file();
        let base_name = if base_is_shipped {
            DEFAULTS_TEMPLATE
        } else {
            DEFAULTS
        };
        let mut base = self.read_yaml(base_name)?;

        // First run on a new machine: seed the machine-specific config from the example template.
        // Through the same writer as every other save, not a copy: a copy interrupted halfway
        // leaves a TRUNCATED local.yaml that loads as a valid empty config, and the player then
        // runs on v1 starting coordinates without an error anywhere.
        let local_path = self.path_of(LOCAL);
        if !local_path.is_file() {
            let example = self.path_of(LOCAL_EXAMPLE);
            if example.is_file() {
                let text = Self::read_text(&example)?;
                Self::write_atomically_locked(&local_path, &text)?;
            }
        }
        if !local_path.is_file() {
            return Err(ConfigError::MissingFile(local_path));
        }

        let mut local = self.read_yaml(LOCAL)?;
        if !local.is_object() {
            // An empty or comment-only document deserialises to null; a fresh overlay is an empty
            // mapping, never a null the merge would have to keep special-casing.
            local = Value::Object(Map::new());
        }

        // THE LEGACY FILE. On an install that has taken an update, this is the player's own old
        // defaults.yaml — the only copy of every setting they ever saved, because the old save path
        // wrote the whole portable projection into it. It is read ONCE: the fold below moves those
        // values into local.yaml, and from then on the marker stops this file being read at all.
        let mut legacy =
            if base_is_shipped && !is_folded(&local) && self.path_of(DEFAULTS).is_file() {
                Some(self.read_yaml(DEFAULTS)?)
            } else {
                None
            };

        // Normalise both files the same way, because the fold compares like with like: a
        // pre-presets flat `spammer.keys` becomes a preset in memory first, so the comparison never
        // meets two shapes of the same setting.
        migrate_spammer_presets(&mut base);
        if let Some(legacy) = legacy.as_mut() {
            migrate_spammer_presets(legacy);
        }

        // THE FOLD, once. It compares section by section and writes only what the player changed,
        // so a default a later release changes still arrives for every key they never touched.
        let mut folded = false;
        if let Some(legacy) = &legacy {
            folded = self.fold_into_local(legacy, &base)?;
        }

        // A successful fold has just written local.yaml, so the copy read above is stale. Re-read
        // it, or the player's own values arrive only on the NEXT launch.
        if folded {
            local = self.read_yaml(LOCAL)?;
            if !local.is_object() {
                local = Value::Object(Map::new());
            }
        }

        let mut config = base;
        // The template's own top-level sections are the portable shape (`CONFIG.md`: everything in
        // the shipped file is "the same on every machine"). Taking it from the file rather than a
        // hard-coded field list is what keeps the fold, the save projection and this apply from
        // drifting apart.
        let portable = portable_sections(&config);

        // The legacy file as a VALUES LAYER — the pre-fold behaviour, kept only as the fallback for
        // a fold that could not be written (read-only medium, full disk). It sits BELOW local.yaml,
        // which is why it is applied before the settings block and the machine half.
        if let Some(legacy) = &legacy
            && !folded
        {
            apply_portable_layer(&mut config, legacy);
        }

        // Presets used to be saved into the shipped file. Adopt whatever the old file still holds
        // into local.yaml before a save can rewrite it away — the same once-only migration v2 runs.
        let adopt_source = legacy.as_ref().unwrap_or(&config);
        self.adopt_spammer_presets(adopt_source, &mut local)?;

        // The portable layer first — the layer the shipped file used to be — then this machine's
        // own measurements on top of it.
        apply_settings(&mut config, &local, &portable);
        apply_machine_blocks(&mut config, &local);

        resolve_variant(&mut config);

        // The legacy path has no per-file tree, so nothing there can be quarantined; the warnings
        // slot exists so both paths answer the same shape.
        Ok(crate::layout::Loaded {
            config,
            warnings: Vec::new(),
        })
    }

    /// Parses `local.yaml` on its own — the overlay, not the merged config. `Value::Null` for an
    /// empty or comment-only document, exactly as the file reads.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::MissingFile`] when `local.yaml` is absent on the legacy layout, and
    /// [`ConfigError`] when the file (or the portable layout behind it) cannot be read or parsed.
    pub fn load_local(&self) -> Result<Value, ConfigError> {
        if self.is_new_layout() {
            return self.new_layout_view();
        }
        let path = self.path_of(LOCAL);
        if !path.is_file() {
            return Err(ConfigError::MissingFile(path));
        }
        self.read_yaml(LOCAL)
    }

    /// Loads the OCR dictionary — the active game variant's, when the caller names one. With no
    /// name it is the TW dictionary, `attributes.yaml`, which is what a config with no variant uses.
    ///
    /// A missing file is SEEDED from `<name>.example` when that template exists, then read; with no
    /// template either the failure is loud, because an absent dictionary matches nothing and would
    /// read every attribute as "no attribute", silently — indistinguishable from the game having
    /// changed.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the dictionary cannot be read or parsed, and
    /// [`ConfigError::MissingFile`] when it is absent and has no `<name>.example` template either.
    pub fn load_attributes(&self) -> Result<Value, ConfigError> {
        self.load_attributes_named(ATTRIBUTES)
    }

    /// [`Self::load_attributes`] for a named dictionary (a game variant's, e.g.
    /// `attributes.us.yaml`).
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the dictionary cannot be read or parsed, and
    /// [`ConfigError::MissingFile`] when it is absent and has no `<name>.example` template either.
    pub fn load_attributes_named(&self, name: &str) -> Result<Value, ConfigError> {
        let path = self.path_of(name);
        if !path.is_file() {
            let example = self.path_of(&format!("{name}.example"));
            if example.is_file() {
                let text = Self::read_text(&example)?;
                Self::write_atomically_locked(&path, &text)?;
            } else {
                return Err(ConfigError::MissingFile(path));
            }
        }
        self.read_yaml(name)
    }

    /// Reads `local.yaml`, applies `mutate`, and writes it back — all under the save gate, which is
    /// the entire point of the method.
    ///
    /// The callback therefore sees the file as it is NOW rather than as it was when some earlier
    /// load ran, and it may run more than once in a process's lifetime but never concurrently. A
    /// file that cannot be PARSED fails the save rather than being overwritten — the read comes
    /// first, so an unreadable `local.yaml` says so instead of being replaced by whatever the last
    /// caller had in mind.
    ///
    /// Everything the application does not own is carried through the save verbatim (see
    /// [`crate::foreign`]), and the constant header is prepended to every write.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the existing file cannot be read or parsed, or when the atomic
    /// write fails.
    pub fn update_local<F: FnOnce(&mut Value)>(&self, mutate: F) -> Result<(), ConfigError> {
        self.update_local_checked(|local| {
            mutate(local);
            Ok(())
        })
    }

    /// The same one-writer read-modify-write, but the mutator may refuse the write. A refused
    /// mutation leaves the file untouched and the gate still covers the whole read/write.
    ///
    /// # Errors
    ///
    /// Returns the mutator's own [`ConfigError`] unchanged, or the read/parse/atomic-write error of
    /// the save it wraps.
    pub fn update_local_checked<F>(&self, mutate: F) -> Result<(), ConfigError>
    where
        F: FnOnce(&mut Value) -> Result<(), ConfigError>,
    {
        let _gate = lock_save_gate();
        if self.is_new_layout() {
            return self.update_new_layout(mutate);
        }
        let path = self.path_of(LOCAL);
        let existing = if path.is_file() {
            Self::read_text(&path)?
        } else {
            String::new()
        };
        let foreign = crate::foreign::foreign_blocks(&existing);
        let mut local = if existing.is_empty() {
            Value::Object(Map::new())
        } else {
            yaml::parse(&existing).map_err(|message| ConfigError::Parse {
                path: path.clone(),
                message,
            })?
        };
        // Only the blocks this app owns travel through the object; a foreign block travels as raw
        // text, appended after the body. Retaining only the owned keys is the v2-equivalent of
        // `Deserialize<LocalOverrides>` dropping what the class does not model — and it is
        // load-bearing here rather than cosmetic: without it the parsed foreign blocks are ALSO
        // re-serialised into the body and then appended again as text, so every save duplicates
        // them (and the next save duplicates the duplicates).
        match &mut local {
            Value::Object(local_object) => {
                local_object.retain(|key, _| crate::foreign::is_owned_block(key));
            }
            _ => local = Value::Object(Map::new()),
        }

        mutate(&mut local)?;

        let body = yaml::to_string(&local).map_err(|message| ConfigError::Serialize {
            path: path.clone(),
            message,
        })?;
        let mut text =
            String::with_capacity(crate::LOCAL_HEADER.len() + body.len() + foreign.len());
        text.push_str(crate::LOCAL_HEADER);
        text.push_str(&body);
        text.push_str(&foreign);
        Self::write_atomically(&path, &text)
    }

    /// True when the config folder is the class × domain tree (a migrated install). The marker is
    /// `meta.yaml`'s `layout_version`; its presence is the switch the migration writes LAST.
    fn is_new_layout(&self) -> bool {
        self.path_of(META).is_file()
    }

    /// The shipped base for the new layout: the template when an install carries one, else the
    /// tracked defaults (a checkout). The merged read itself is `layout::load`.
    fn load_base_template(&self) -> Result<Value, ConfigError> {
        let name = if self.path_of(DEFAULTS_TEMPLATE).is_file() {
            DEFAULTS_TEMPLATE
        } else {
            DEFAULTS
        };
        self.read_yaml(name)
    }

    /// The new tree seen through the OLD overlay shape (`settings:` + machine blocks). Every
    /// `update_local` closure in the core speaks this shape; presenting it here is what lets the
    /// writer switch to per-file routing without rewriting dozens of call sites.
    ///
    /// A corrupt machine file is quarantined here too, so a save or an overlay read is not blocked
    /// by the same file that would have blocked startup (HW-F15). The warning belongs to the load
    /// path ([`Self::load_detailed`]); this view has no slot for it.
    fn new_layout_view(&self) -> Result<Value, ConfigError> {
        let mut view = Map::new();
        let mut settings = Map::new();
        for def in registry::portable_files() {
            let Some(content) = file::read_or_quarantine(&self.dir, def)?.0 else {
                continue;
            };
            if let Some(blocks) = content.as_object() {
                for (key, value) in blocks {
                    match settings.get_mut(key) {
                        Some(slot) => merge_overlay(slot, value),
                        None => {
                            settings.insert(key.clone(), value.clone());
                        }
                    }
                }
            }
        }
        if let Some(meta_def) = registry::FILES.iter().find(|def| def.path == META)
            && let Some(meta) = file::read(&self.dir, meta_def)?
            && let Some(folded) = meta.get(LEGACY_FOLDED)
        {
            settings.insert(LEGACY_FOLDED.to_owned(), folded.clone());
        }
        if !settings.is_empty() {
            view.insert(SETTINGS.to_owned(), Value::Object(settings));
        }
        for def in registry::machine_files() {
            let Some(content) = file::read_or_quarantine(&self.dir, def)?.0 else {
                continue;
            };
            let Value::Object(blocks) = content else {
                continue;
            };
            for (key, value) in blocks {
                match view.get_mut(&key) {
                    Some(slot) => merge_overlay(slot, &value),
                    None => {
                        view.insert(key, value);
                    }
                }
            }
        }
        Ok(Value::Object(view))
    }

    /// Read the old overlay shape, run the caller's mutation, and route only the CHANGED owned
    /// blocks back to their files (one atomic write per file, or the two-file transaction). A
    /// top-level block removal is not expressible through the field-wise writer and is refused
    /// rather than silently left behind.
    fn update_new_layout<F>(&self, mutate: F) -> Result<(), ConfigError>
    where
        F: FnOnce(&mut Value) -> Result<(), ConfigError>,
    {
        let before = self.new_layout_view()?;
        let mut after = before.clone();
        mutate(&mut after)?;
        let before_object = before.as_object().cloned().unwrap_or_default();
        let after_object = after.as_object().cloned().unwrap_or_default();
        let mut sections = Map::new();
        for (key, value) in &after_object {
            if before_object.get(key) == Some(value) {
                continue;
            }
            let before_value = before_object.get(key);
            if key == SETTINGS {
                if let Some(settings) = changed_fields(before_value, value) {
                    sections.insert(key.clone(), settings);
                }
                continue;
            }
            if let Some(changed) = changed_fields(before_value, value) {
                sections.insert(key.clone(), changed);
            }
        }
        for key in before_object.keys() {
            if !after_object.contains_key(key) {
                return Err(ConfigError::SectionNotOwned(format!(
                    "removal of top-level block {key} through the portable writer"
                )));
            }
        }
        if sections.is_empty() {
            return Ok(());
        }
        crate::layout::write_sections(&self.dir, &sections)?;
        Ok(())
    }

    /// Persist one pet run-state update through the same gate/read-modify-write as every other
    /// save. Field-wise by construction: only the fields named in `patch` are touched, inside the
    /// existing `pet` block; the rest of `local.yaml` (foreign text included) is carried as usual.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Parse`] when `local.yaml`'s `pet` shape cannot hold the patch (not a
    /// mapping, not a list of rows), and the read/atomic-write errors of the save itself.
    pub fn persist_pet_run_state(&self, patch: &PetRunStatePatch) -> Result<(), ConfigError> {
        if patch.is_empty() {
            return Ok(());
        }
        let local_path = self.path_of(LOCAL);
        self.update_local_checked(|local| {
            let Some(local_object) = local.as_object_mut() else {
                return Err(ConfigError::Parse {
                    path: local_path.clone(),
                    message: "local.yaml does not contain a mapping".to_owned(),
                });
            };
            let pet = local_object
                .entry("pet".to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
            let Some(pet) = pet.as_object_mut() else {
                return Err(ConfigError::Parse {
                    path: local_path.clone(),
                    message: "pet is not a mapping".to_owned(),
                });
            };

            if let Some(used) = patch.food_slots_used {
                pet.insert("food_slots_used".to_owned(), Value::from(used));
            }
            if let Some(cells) = &patch.food_slots {
                let slots: Vec<Value> = cells
                    .iter()
                    .map(|(page, cell)| Value::Array(vec![Value::from(*page), Value::from(*cell)]))
                    .collect();
                pet.insert("food_slots".to_owned(), Value::Array(slots));
                // Replacing the set is exactly the moment the old used-cell count stops meaning
                // anything (v2's PetScanFood invariant): reset it in the same atomic write.
                pet.insert("food_slots_used".to_owned(), Value::from(0));
            }
            if !patch.boarding_running.is_empty() {
                let slots = pet
                    .entry("slots".to_owned())
                    .or_insert_with(|| Value::Array(Vec::new()));
                let Some(slots) = slots.as_array_mut() else {
                    return Err(ConfigError::Parse {
                        path: local_path.clone(),
                        message: "pet.slots is not a list".to_owned(),
                    });
                };
                for (row, running) in &patch.boarding_running {
                    let Some(slot) = slots.get_mut(*row) else {
                        return Err(ConfigError::Parse {
                            path: local_path.clone(),
                            message: format!("pet.slots has no row {}", row + 1),
                        });
                    };
                    let Some(slot) = slot.as_object_mut() else {
                        return Err(ConfigError::Parse {
                            path: local_path.clone(),
                            message: format!("pet.slots row {} is not a mapping", row + 1),
                        });
                    };
                    slot.insert("boarding_running".to_owned(), Value::Bool(*running));
                }
            }
            if let Some(close) = &patch.close_button_png {
                match close {
                    Some(png) => {
                        pet.insert("close_button_png".to_owned(), Value::String(png.clone()));
                    }
                    None => {
                        pet.remove("close_button_png");
                    }
                }
            }
            if let Some(open) = patch.boarding_window_may_be_open {
                pet.insert("boarding_window_may_be_open".to_owned(), Value::Bool(open));
            }
            Ok(())
        })
    }

    /// Writes a binary artifact (a calibration crop) beside the config files, atomically.
    ///
    /// The text writer carries text only; a screenshot must never be half-written, so this is the
    /// same temp-and-rename shape on the same directory (the rename cannot cross a volume). A name
    /// is a bare file name: the caller says which artifact, never a path.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Write`] when `name` is not a bare file name, or when the config
    /// directory, the temp file, or the rename over the destination fails.
    pub fn write_artifact(&self, name: &str, bytes: &[u8]) -> Result<PathBuf, ConfigError> {
        if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
            return Err(ConfigError::Write {
                path: self.dir.join(name),
                source: io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "an artifact name is a bare file name",
                ),
            });
        }
        fs::create_dir_all(&self.dir).map_err(|source| ConfigError::Write {
            path: self.dir.clone(),
            source,
        })?;
        let path = self.dir.join(name);
        // One writer for every save, so the artifact cannot be half-written either (HW-F5).
        file::atomic_replace(&path, bytes)?;
        Ok(path)
    }

    /// Merges the given top-level blocks field-wise into `local.yaml` and returns the merged value
    /// of each block, which is what the protocol's `config.apply` answers with.
    ///
    /// Only blocks the application owns can be written: a foreign block is carried as raw text, so
    /// rewriting one from JSON would duplicate it. A section the file does not have yet is added.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::SectionNotOwned`] for a block the app does not own (nothing is written
    /// in that case), and the read/parse/atomic-write errors of the save itself.
    pub fn apply_sections(
        &self,
        sections: &Map<String, Value>,
    ) -> Result<Map<String, Value>, ConfigError> {
        for name in sections.keys() {
            if !crate::foreign::is_owned_block(name) {
                return Err(ConfigError::SectionNotOwned(name.clone()));
            }
        }

        let mut merged = Map::new();
        self.update_local(|local| {
            let Some(local_object) = local.as_object_mut() else {
                return;
            };
            for (name, value) in sections {
                match local_object.get_mut(name) {
                    Some(slot) => merge_overlay(slot, value),
                    None => {
                        local_object.insert(name.clone(), value.clone());
                    }
                }
                merged.insert(
                    name.clone(),
                    local_object.get(name).cloned().unwrap_or(Value::Null),
                );
            }
        })?;
        Ok(merged)
    }

    /// Applies the owned blocks AND the portable `settings:` projection in ONE `local.yaml`
    /// read-modify-write.
    ///
    /// `config.apply` used to call [`Self::apply_sections`] and [`Self::save_settings`] one after the
    /// other: the owned write could succeed and the projection then fail, leaving the file HALF-SAVED
    /// — which is exactly what `PROTOCOL.md` §4's "nothing written, no rev consumed" promise forbids.
    /// Here both changes happen inside the same update, so a failure at any point writes nothing. The
    /// projection is built BEFORE the update (its template read can fail), and the owned merge is the
    /// same rule [`Self::apply_sections`] uses; both halves reuse the code rather than duplicating it.
    ///
    /// `config` is the effective merged config the caller validated and wants projected — the same
    /// value [`Self::save_settings`] takes. Returns the merged value of each owned block, like
    /// [`Self::apply_sections`], so a caller can answer with it.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::SectionNotOwned`] for a block the app does not own (nothing is
    /// written), when the settings projection cannot read its template, or when the save fails.
    pub fn apply_sections_with_settings(
        &self,
        sections: &Map<String, Value>,
        config: &Value,
    ) -> Result<Map<String, Value>, ConfigError> {
        for name in sections.keys() {
            if !crate::foreign::is_owned_block(name) {
                return Err(ConfigError::SectionNotOwned(name.clone()));
            }
        }

        // Built before the update so a template that cannot be read leaves the file untouched.
        let projection = self.projection_for(config)?;

        let mut merged = Map::new();
        self.update_local(|local| {
            let Some(local_object) = local.as_object_mut() else {
                return;
            };

            for (name, value) in sections {
                match local_object.get_mut(name) {
                    Some(slot) => merge_overlay(slot, value),
                    None => {
                        local_object.insert(name.clone(), value.clone());
                    }
                }
                merged.insert(
                    name.clone(),
                    local_object.get(name).cloned().unwrap_or(Value::Null),
                );
            }

            let settings = local_object
                .entry(SETTINGS.to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
            if !settings.is_object() {
                *settings = Value::Object(Map::new());
            }
            if let Value::Object(settings) = settings {
                for (name, value) in &projection {
                    settings.insert(name.clone(), value.clone());
                }
            }
        })?;
        Ok(merged)
    }

    /// The settings projection for `config`: the portable sections of the shipped template, taken
    /// from `config` and reduced to the template's shape. Factored out so
    /// [`Self::save_settings`] and [`Self::apply_sections_with_settings`] cannot project differently.
    fn projection_for(&self, config: &Value) -> Result<Map<String, Value>, ConfigError> {
        let base_is_shipped = self.path_of(DEFAULTS_TEMPLATE).is_file();
        let base_name = if base_is_shipped {
            DEFAULTS_TEMPLATE
        } else {
            DEFAULTS
        };
        let mut base = self.read_yaml(base_name)?;
        // `ui.always_on_top` is the shell's portable window preference, introduced after the shipped
        // template was written. Its shape is added here explicitly so the projection can carry it
        // without making every unknown `settings` key writable.
        ensure_portable_ui(&mut base);
        Ok(portable_projection(config, &base))
    }
    /// Writes the portable half of `config` into `local.yaml`'s `settings:` block — v2's
    /// `SaveSettings`, the save path that replaced writing the shipped file.
    ///
    /// The block is MUTATED rather than replaced: the fold's marker and anything this release does
    /// not know about survive, which is what makes "fold once" true across saves. Sections are
    /// copied WHOLE, projected onto the shipped template's shape, so a machine-only field (such as
    /// `tuner.ocr`) can never leak into the portable half — and the spammer block is deliberately
    /// not projected at all: a personal rotation must never reach a released template.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the shipped template cannot be read or parsed, or when the
    /// `local.yaml` read-modify-write fails.
    pub fn save_settings(&self, config: &Value) -> Result<(), ConfigError> {
        let projection = self.projection_for(config)?;

        self.update_local(|local| {
            let Some(local_object) = local.as_object_mut() else {
                return;
            };
            let settings = local_object
                .entry(SETTINGS.to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
            if !settings.is_object() {
                *settings = Value::Object(Map::new());
            }
            if let Value::Object(settings) = settings {
                for (name, value) in projection {
                    // Replaced, not field-merged: the apply side assigns the section, so half a
                    // section would drag the other half's defaults in with it.
                    settings.insert(name, value);
                }
            }
        })
    }

    /// Moves the player's own portable values out of the legacy defaults file and into
    /// `local.yaml`'s `settings:` block, ONCE, and reports whether the write succeeded.
    ///
    /// Reading is per leaf, writing is per SECTION: a section is carried whole as soon as any of its
    /// leaves differs, because the apply side assigns the section — `hotkeys.start` on its own would
    /// carry the default 0 for the other three keys.
    ///
    /// 'Differs from the shipped template' is how it reads 'the player changed it', and the direction
    /// it errs in is the safe one: a default a release changed under a key nobody touched is
    /// preserved rather than silently applied.
    ///
    /// The legacy file is never written, moved or deleted: it is the rollback copy and the record of
    /// what the player had. A write that cannot happen returns `false` rather than throwing: the
    /// marker stays unset, so the caller keeps reading the legacy file for the session and the fold
    /// retries on the next launch. Any other failure — a local.yaml that cannot be parsed — is a real
    /// error and is reported, never overwritten.
    fn fold_into_local(&self, legacy: &Value, base: &Value) -> Result<bool, ConfigError> {
        let carried = fold_projection(legacy, base);

        let result = self.update_local(|local| {
            let Some(local_object) = local.as_object_mut() else {
                return;
            };
            let settings = local_object
                .entry(SETTINGS.to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
            if !settings.is_object() {
                *settings = Value::Object(Map::new());
            }
            if let Value::Object(settings) = settings {
                for (name, value) in carried {
                    // Replaced, not field-merged: v2's MergeSettings assigns the section.
                    settings.insert(name, value);
                }
                // Unconditional, and in the SAME write. A machine that never changed a value must
                // still stop folding, or the next launch would fold the old template against a
                // newer release's defaults and pin that release's defaults as the player's own.
                settings.insert(LEGACY_FOLDED.to_owned(), Value::Bool(true));
            }
        });

        match result {
            Ok(()) => Ok(true),
            // The one stated exception to "a write that cannot happen throws" (PLAN-DEPLOY §6.5).
            Err(ConfigError::Write { .. }) => Ok(false),
            Err(other) => Err(other),
        }
    }

    /// Folds the legacy `defaults.yaml` into `local.yaml` BEFORE the layout migration runs.
    ///
    /// The migration routes whatever `local.yaml` holds and then retires it; it never reads the old
    /// shipped file. On an install that upgrades from a pre-fold release the player's settings
    /// (hotkeys, tuner behaviour, presets) live ONLY in that file, so folding afterwards can never
    /// happen — there is no legacy read left on the new layout. This is the pre-step that makes the
    /// migration lossless (HW-F3).
    ///
    /// A failed fold write is a real error here, not the tolerated `false` of the per-session path:
    /// proceeding would retire the only copy of the values.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the legacy file or `local.yaml` cannot be read, or when the fold
    /// write fails.
    fn fold_legacy_before_migration(&self, base: &Value) -> Result<(), ConfigError> {
        if !self.path_of(DEFAULTS).is_file() {
            // Nothing to fold; the migration's marker then truthfully says "folded".
            return Ok(());
        }
        let mut legacy = self.read_yaml(DEFAULTS)?;
        migrate_spammer_presets(&mut legacy);
        let mut base = base.clone();
        migrate_spammer_presets(&mut base);
        if !self.fold_into_local(&legacy, &base)? {
            return Err(ConfigError::Write {
                path: self.path_of(LOCAL),
                source: io::Error::other(
                    "the one-time fold of defaults.yaml could not be written, so the layout \
                     migration was not started and nothing was retired",
                ),
            });
        }
        // The presets used to live in the shipped file; take them along in the same window the fold
        // opens, or the migration's save would leave them behind (HW-F3).
        let mut local = self.read_yaml(LOCAL)?;
        self.adopt_spammer_presets(&legacy, &mut local)
    }

    /// Adopts spammer presets left in the old shipped file into `local.yaml`, at most once.
    ///
    /// Presets used to be saved into the shipped `defaults.yaml`. The save path stopped writing them,
    /// so the first save on any other tab rewrote that file without a spammer block — and on a
    /// machine that only ever ran the older build those presets existed nowhere else, so the save
    /// deleted them outright. The guard is the block's presence in `local.yaml`: after the first
    /// adoption, including on a first run seeded from an example with no spammer block, the block is
    /// non-null and this does nothing. The write re-checks the guard against the file as it stands
    /// NOW, because another process can have adopted in the gap since this load read it, and writing
    /// ours blind would replace the winner's presets with our copy.
    ///
    /// A hand-written legacy `startup_step` folds into the object shape first, so the copy that is
    /// adopted carries one representation rather than two; the old key is then never written again.
    fn adopt_spammer_presets(&self, source: &Value, local: &mut Value) -> Result<(), ConfigError> {
        if local
            .get("spammer")
            .is_some_and(|spammer| !spammer.is_null())
        {
            return Ok(());
        }
        let Some(source_spammer) = source.get("spammer").and_then(Value::as_object) else {
            return Ok(());
        };
        let has_presets = source_spammer
            .get("presets")
            .and_then(Value::as_object)
            .is_some_and(|presets| !presets.is_empty());
        if !has_presets {
            return Ok(());
        }

        let mut adopted = Map::new();
        for key in ["active", "presets", "priority", "combos", "startup"] {
            if let Some(value) = source_spammer.get(key) {
                adopted.insert(key.to_owned(), value.clone());
            }
        }
        if let Some(steps) = source_spammer
            .get("startup_step")
            .and_then(Value::as_object)
        {
            let startup = adopted
                .entry("startup".to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
            if !startup.is_object() {
                *startup = Value::Object(Map::new());
            }
            if let Some(startup) = startup.as_object_mut() {
                for (name, step) in steps {
                    let entry = startup
                        .entry(name.clone())
                        .or_insert_with(|| Value::Object(Map::new()));
                    if !entry.is_object() {
                        *entry = Value::Object(Map::new());
                    }
                    if let Some(entry) = entry.as_object_mut() {
                        entry.insert("step".to_owned(), step.clone());
                    }
                }
            }
        }

        let mut in_file: Option<Value> = None;
        self.update_local(|fresh| {
            let Some(fresh_object) = fresh.as_object_mut() else {
                return;
            };
            if fresh_object
                .get("spammer")
                .is_none_or(serde_json::Value::is_null)
            {
                fresh_object.insert("spammer".to_owned(), Value::Object(adopted.clone()));
            }
            in_file = fresh_object.get("spammer").cloned();
        })?;

        // The session follows the file, so even the process that lost that race reads the presets
        // that are actually stored rather than a second set of its own.
        if let Some(spammer) = in_file
            && let Some(local_object) = local.as_object_mut()
        {
            local_object.insert("spammer".to_owned(), spammer);
        }
        Ok(())
    }

    fn read_text(path: &Path) -> Result<String, ConfigError> {
        fs::read_to_string(path).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                ConfigError::MissingFile(path.to_owned())
            } else {
                ConfigError::Read {
                    path: path.to_owned(),
                    source,
                }
            }
        })
    }

    fn read_yaml(&self, name: &str) -> Result<Value, ConfigError> {
        let path = self.path_of(name);
        if !path.is_file() {
            return Err(ConfigError::MissingFile(path));
        }
        let text = Self::read_text(&path)?;
        yaml::parse(&text).map_err(|message| ConfigError::Parse { path, message })
    }

    /// The atomic writer: a unique temp name beside the destination, an fsync, then a replace that
    /// waits out a transient holder. The one implementation lives in [`file::atomic_replace`] so the
    /// legacy store and the portable tree cannot drift apart on durability (HW-F5).
    ///
    /// Callers that do not already hold [`SAVE_GATE`] use [`Self::write_atomically_locked`]; the
    /// gate is not reentrant, so this one is the inner half.
    fn write_atomically(path: &Path, text: &str) -> Result<(), ConfigError> {
        file::atomic_replace(path, text.as_bytes())
    }

    fn write_atomically_locked(path: &Path, text: &str) -> Result<(), ConfigError> {
        let _gate = lock_save_gate();
        Self::write_atomically(path, text)
    }
}

/// Takes the process-wide save gate. A poisoned lock (a writer panicked mid-write) is recovered
/// rather than rethrown: the file's atomic replace means the panic left either the old file or the
/// whole new one, and refusing every later save would outlast the bug.
fn lock_save_gate() -> MutexGuard<'static, ()> {
    SAVE_GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// True when `local.yaml`'s `settings:` block carries the once-only fold marker.
fn is_folded(local: &Value) -> bool {
    local
        .get(SETTINGS)
        .and_then(|settings| settings.get(LEGACY_FOLDED))
        == Some(&Value::Bool(true))
}

/// A pre-presets flat `spammer.keys` map becomes the `"default"` preset, and the legacy key is
/// removed so nothing reads two shapes of the same setting (v2's `MigrateSpammerPresets`).
fn migrate_spammer_presets(config: &mut Value) {
    let Some(spammer) = config.get_mut("spammer").and_then(Value::as_object_mut) else {
        return;
    };
    let has_presets = spammer
        .get("presets")
        .and_then(Value::as_object)
        .is_some_and(|presets| !presets.is_empty());
    if !has_presets {
        let keys = spammer
            .get("keys")
            .and_then(Value::as_object)
            .filter(|keys| !keys.is_empty())
            .cloned();
        if let Some(keys) = keys {
            let mut presets = Map::new();
            presets.insert("default".to_owned(), Value::Object(keys));
            spammer.insert("presets".to_owned(), Value::Object(presets));
            spammer.insert("active".to_owned(), Value::String("default".to_owned()));
        }
    }
    // v2 sets the legacy property to null; nothing writes it again, so removing it is the same
    // statement without leaving a bare `keys:` in every merged config.
    spammer.remove("keys");
}

/// The portable top-level sections: everything the shipped template carries, except the two names
/// that are never portable by construction — `spammer` (personal rotations must never ship) and
/// `settings` (where the projection itself lives). `ui` is the one post-template portable section:
/// its shape is added by [`ensure_portable_ui`] so a hand-written or applied
/// `settings.ui.always_on_top` is read even on an install whose shipped template predates it.
fn portable_sections(base: &Value) -> Vec<String> {
    let mut sections: Vec<String> = base
        .as_object()
        .map(|sections| {
            sections
                .keys()
                .filter(|name| is_portable_section(name))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if !sections.iter().any(|name| name == "ui") {
        sections.push("ui".to_owned());
    }
    sections
}

/// The portable shape of the post-template setting leaves: `ui.always_on_top`, the shell's
/// `ui.quick_dock` preference and the two run-window preferences (`collapse_on_run_start`,
/// `return_after_run`). The projection still reduces the merged config to these leaves, so the
/// machine `ui` block's geometry can never enter the portable `settings:` half.
fn ensure_portable_ui(base: &mut Value) {
    let Some(object) = base.as_object_mut() else {
        return;
    };
    let ui = object
        .entry("ui".to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(ui_object) = ui.as_object_mut() else {
        return;
    };
    ui_object
        .entry("always_on_top".to_owned())
        .or_insert(Value::Bool(false));
    ui_object
        .entry("collapse_on_run_start".to_owned())
        .or_insert(Value::Bool(true));
    ui_object
        .entry("return_after_run".to_owned())
        .or_insert(Value::Bool(false));
    ui_object
        .entry("theme".to_owned())
        .or_insert(Value::String("dark".to_owned()));
    ui_object.entry("quick_dock".to_owned()).or_insert_with(|| {
        let mut shape = Map::new();
        shape.insert(
            "placement".to_owned(),
            Value::String("right_rail".to_owned()),
        );
        shape.insert("size".to_owned(), Value::String("standard".to_owned()));
        shape.insert("collapsed".to_owned(), Value::Bool(false));
        shape.insert(
            "behavior".to_owned(),
            Value::String("auto_collapse".to_owned()),
        );
        // Lists are a single leaf to the projection. The tiles default mirrors the shell's shipped
        // quick-action list so a config that never customised the dock still exposes all of them;
        // an explicit empty list is the player's own choice and is preserved as written.
        shape.insert(
            "tiles".to_owned(),
            Value::Array(
                [
                    "spammer",
                    "buy",
                    "sell",
                    "quest",
                    "gem",
                    "tuner",
                    "holdspace",
                ]
                .into_iter()
                .map(|id| Value::String(id.to_owned()))
                .collect(),
            ),
        );
        shape.insert("pins".to_owned(), Value::Array(Vec::new()));
        Value::Object(shape)
    });
    // A hand-edited/malformed scalar here is not repaired: the projection simply has no destination
    // below it, so a broken `quick_dock` stays out of the portable half like any other bad value.
}

fn is_portable_section(name: &str) -> bool {
    name != "spammer" && name != SETTINGS
}

/// The merged config the LEGACY loader would serve from `local` over `base` — the "old" half of the
/// migration's verify-before-retire comparison (HW-F4). Pure: no disk.
///
/// `local` is the overlay the migration is routing, with its crops already rewritten to artifact
/// references, so both sides of the comparison are in the same shape. The legacy `defaults.yaml`
/// layer is not applied here because the migration only runs after
/// [`ConfigStore::fold_legacy_before_migration`] has folded it into `local`.
pub(crate) fn legacy_merged_config(base: &Value, local: &Value) -> Value {
    let mut config = base.clone();
    let portable = portable_sections(&config);
    apply_settings(&mut config, local, &portable);
    apply_machine_blocks(&mut config, local);
    resolve_variant(&mut config);
    config
}

/// Applies the `settings:` block (the portable half) onto the merged config, in the template's
/// section order. A section the block does not mention is left exactly as the base loaded it.
fn apply_settings(config: &mut Value, local: &Value, portable: &[String]) {
    let Some(settings) = local.get(SETTINGS).and_then(Value::as_object) else {
        return;
    };
    let Some(config_object) = config.as_object_mut() else {
        return;
    };
    for name in portable {
        let Some(value) = settings.get(name) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        match config_object.get_mut(name) {
            Some(slot) => merge_overlay(slot, value),
            None => {
                config_object.insert(name.clone(), value.clone());
            }
        }
    }
}

/// Applies this machine's own blocks on top, in the owned-block order. The order matters only when
/// both name the same field, and then the machine half keeps the last word — which is what an
/// overlay of measured values means.
fn apply_machine_blocks(config: &mut Value, local: &Value) {
    let Some(config_object) = config.as_object_mut() else {
        return;
    };
    for name in OWNED_LOCAL_BLOCKS {
        if *name == SETTINGS {
            continue;
        }
        let Some(value) = local.get(*name) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        match config_object.get_mut(*name) {
            Some(slot) => merge_overlay(slot, value),
            None => {
                config_object.insert((*name).to_owned(), value.clone());
            }
        }
    }
}

/// The legacy fallback layer: the portable values the old file states, applied onto the base. Only
/// leaves the TEMPLATE shapes are carried, so a hand-added machine field cannot ride in.
fn apply_portable_layer(config: &mut Value, legacy: &Value) {
    let Some(legacy_object) = legacy.as_object() else {
        return;
    };
    let carried: Vec<(String, Value)> = config
        .as_object()
        .map(|base| {
            base.iter()
                .filter(|(name, _)| is_portable_section(name))
                .filter_map(|(name, shape)| {
                    let source = legacy_object.get(name)?;
                    if source.is_null() {
                        return None;
                    }
                    Some((name.clone(), project_onto_shape(shape, source)))
                })
                .collect()
        })
        .unwrap_or_default();
    let Some(config_object) = config.as_object_mut() else {
        return;
    };
    for (name, value) in carried {
        if let Some(slot) = config_object.get_mut(&name) {
            merge_overlay(slot, &value);
        }
    }
}

/// The filled section from [`portable_projection`]: each portable section of the merged config,
/// rewritten to the template's shape so the `settings:` block holds exactly the portable fields.
fn portable_projection(config: &Value, base: &Value) -> Map<String, Value> {
    let mut projection = Map::new();
    let (Some(config_object), Some(base_object)) = (config.as_object(), base.as_object()) else {
        return projection;
    };
    for (name, shape) in base_object {
        if !is_portable_section(name) {
            continue;
        }
        let Some(value) = config_object.get(name) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        projection.insert(name.clone(), project_onto_shape(shape, value));
    }
    projection
}

/// Reduces `source` to the leaves `shape` has a destination for. Reading is per leaf, writing is
/// per section: a leaf the legacy file does not state stays absent (so a class default can never
/// enter `local.yaml` wearing the player's name), and a leaf the template does not shape is dropped
/// (it is machine state, not portable).
fn project_onto_shape(shape: &Value, source: &Value) -> Value {
    match shape {
        Value::Object(shape_object) => {
            let mut projected = Map::new();
            if let Some(source_object) = source.as_object() {
                for (key, shape_value) in shape_object {
                    let Some(source_value) = source_object.get(key) else {
                        continue;
                    };
                    if source_value.is_null() {
                        continue;
                    }
                    projected.insert(key.clone(), project_onto_shape(shape_value, source_value));
                }
            }
            Value::Object(projected)
        }
        // A scalar or a list is one value: the file states it or it does not, and there is nothing
        // below it to project.
        _ => source.clone(),
    }
}

/// True when the projected section differs from the shipped shape at any leaf both of them name.
fn project_has_difference(shape: &Value, projected: &Value) -> bool {
    match (shape, projected) {
        (Value::Object(shape_object), Value::Object(projected_object)) => {
            projected_object.iter().any(|(key, value)| {
                match shape_object.get(key) {
                    Some(shape_value) => project_has_difference(shape_value, value),
                    // No destination in the template: the leaf has nowhere to land.
                    None => true,
                }
            })
        }
        _ => shape != projected,
    }
}

/// The sections the fold carries: a portable section is carried WHOLE as soon as any leaf the legacy
/// file states differs from the shipped template's value for it. A section where nothing differs is
/// not carried at all, so an untouched key goes on tracking the released template.
fn fold_projection(legacy: &Value, base: &Value) -> Map<String, Value> {
    let mut carried = Map::new();
    let (Some(legacy_object), Some(base_object)) = (legacy.as_object(), base.as_object()) else {
        return carried;
    };
    for (name, shape) in base_object {
        // Only the portable projection is folded: the spammer's rotations are personal and reach
        // local.yaml through the adoption instead, and `settings` is where this write lands.
        if !is_portable_section(name) {
            continue;
        }
        let Some(source) = legacy_object.get(name) else {
            continue;
        };
        if source.is_null() {
            continue;
        }
        let projected = project_onto_shape(shape, source);
        if project_has_difference(shape, &projected) {
            carried.insert(name.clone(), projected);
        }
    }
    carried
}

/// Only the fields that actually changed between two versions of a block. The writer routes
/// field-by-field, so this is what keeps a preset save from rewriting an untouched sibling file
/// (the whole block would route to every file that owns any of its fields).
fn changed_fields(before: Option<&Value>, after: &Value) -> Option<Value> {
    match (before, after) {
        (Some(Value::Object(before_object)), Value::Object(after_object)) => {
            let mut changed = Map::new();
            for (key, value) in after_object {
                if before_object.get(key) == Some(value) {
                    continue;
                }
                let before_field = before_object.get(key);
                let field = match (before_field, value) {
                    (Some(Value::Object(_)), Value::Object(_)) => {
                        changed_fields(before_field, value).unwrap_or_else(|| value.clone())
                    }
                    _ => value.clone(),
                };
                changed.insert(key.clone(), field);
            }
            (!changed.is_empty()).then(|| Value::Object(changed))
        }
        (Some(before), after) if before == after => None,
        (_, after) => Some(after.clone()),
    }
}

/// Recursive field-wise overlay: objects merge, everything else replaces, and `null` means "not
/// stated" — the YAML spelling of v2's unset optionals, so it leaves the value below it standing.
fn merge_overlay(target: &mut Value, overlay: &Value) {
    crate::layout::merge_with_policy(target, overlay);
}

/// THE VARIANT RESOLVES THE TITLE, once, here — `window.title` stays the single value every tool
/// reads, so nothing downstream has to know a variant exists.
///
/// An unset or unknown variant changes NOTHING, deliberately: a file written before variants
/// existed carries only `window.title`, and an unrecognised name must not blank the target. The
/// variants table is case-insensitive, so a hand-typed `TW` works.
fn resolve_variant(config: &mut Value) {
    let Some(game) = config.get("game") else {
        return;
    };
    let Some(variant_name) = game
        .get("variant")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return;
    };
    let Some(variants) = game.get("variants").and_then(Value::as_object) else {
        return;
    };
    let Some(variant) = variants
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(variant_name))
        .map(|(_, variant)| variant)
    else {
        return;
    };
    let Some(title) = variant
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| !title.is_empty())
    else {
        return;
    };
    let title = title.to_owned();

    let Some(config_object) = config.as_object_mut() else {
        return;
    };
    if !config_object.get("window").is_some_and(Value::is_object) {
        config_object.insert("window".to_owned(), Value::Object(Map::new()));
    }
    if let Some(window) = config_object
        .get_mut("window")
        .and_then(Value::as_object_mut)
    {
        window.insert("title".to_owned(), Value::String(title));
    }
}

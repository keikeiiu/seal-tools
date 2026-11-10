//! The config-file registry: **one class × one domain per file** (PLAN-PORTABLE-SETUP §1–§6).
//!
//! This is increment 0 of the portable-setup layout: it names every file, its class (does it travel?),
//! its domain (which tool owns it?), the schema version it carries, and the config PATHS it owns.
//! Nothing reads or writes these files yet — the current `local.yaml` loader is untouched — so this
//! table is the single place a later loader/writer routes by, and a wrong or missing owner fails a
//! test before it can fail a save.
//!
//! Ownership is by longest dotted prefix: `pet.slots` is owned by `machine/pet.yaml`, while
//! `pet.items_per_minute` is claimed by the more specific `setup/pet.yaml`; the wider machine claim is
//! deliberate for that whole block. Every registered path must have exactly one owner at its most
//! specific length; the model-coverage test in `seal-core/tests/config_registry.rs` walks the real
//! typed overlay and fails if any leaf is unowned.
//!
//! ## Assumed verdict for this work
//!
//! The plan is a proposal whose §14 questions the player answers; this implementation takes the
//! document's own recommendation (layout **C**, `config/setup` + `config/machine` + `config/artifacts`,
//! `user.yaml`, per-file `schema_version: 1`, artifacts extracted later) and leaves comments and the
//! database question as the plan records them (comments deferred; SQLite never the store of record).
//! If the player's verdict differs, this module is the one table to change.

/// What a file's contents mean for portability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Shipped with the release; never player state.
    Defaults,
    /// Player-owned, portable — copy this to another PC (`setup/`).
    Setup,
    /// Measured on THIS PC — never copied, safe to regenerate (`machine/`, `artifacts/`).
    Machine,
    /// The player's own notes/blocks; the core never reads or writes it.
    User,
    /// Release-maintained read-only data (OCR dictionaries).
    Release,
}

/// Which tool (or shared app) owns a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Common,
    Tuner,
    Gem,
    Spammer,
    Quest,
    BuySell,
    Pet,
    Display,
    Tooltip,
    Ui,
    Arduino,
    Meta,
    Attributes,
    Artifacts,
}

/// One config file (or file-like directory where noted) in the registry and load order.
#[derive(Debug, Clone, Copy)]
pub struct FileDef {
    /// Path under `config/`, using `/` on every platform; a trailing `/` marks a directory.
    pub path: &'static str,
    pub class: Class,
    pub domain: Domain,
    /// The reserved `schema_version` this file is written with; `None` for shipped/read-only files
    /// the core does not write.
    pub schema_version: Option<u8>,
    /// The dotted paths this file owns. `settings.legacy_folded` is called out because the legacy
    /// marker lives under the old overlay's `settings:` in the typed model but is `meta.yaml` state.
    pub owns: &'static [&'static str],
}

/// The per-file layout version (PLAN-PORTABLE-SETUP §6/§8). One number: a file that states a newer
/// version must be refused loudly by the future loader rather than guessed at.
pub const SCHEMA_VERSION: u8 = 1;

/// Every player-visible config file in **load order** (PLAN-PORTABLE-SETUP §6): shipped defaults,
/// then the portable setup files, then the machine files, then the bookkeeping/notes files. Files
/// absent on disk simply contribute nothing (missing = "not stated").
pub const FILES: &[FileDef] = &[
    // ── shipped base and release data ──────────────────────────────────────────────────────────
    FileDef {
        path: "defaults.yaml.example",
        class: Class::Defaults,
        domain: Domain::Common,
        schema_version: None,
        owns: &[],
    },
    FileDef {
        path: "attributes.yaml",
        class: Class::Release,
        domain: Domain::Attributes,
        schema_version: None,
        owns: &[],
    },
    FileDef {
        path: "attributes.us.yaml.example",
        class: Class::Release,
        domain: Domain::Attributes,
        schema_version: None,
        owns: &[],
    },
    // ── portable setup: copy this folder to another PC ────────────────────────────────────────
    FileDef {
        path: "setup/common.yaml",
        class: Class::Setup,
        domain: Domain::Common,
        schema_version: Some(SCHEMA_VERSION),
        owns: &[
            "window",
            "game",
            "reference_window",
            "hotkeys",
            "ocr",
            "arduino.vid",
            "arduino.pid",
            "arduino.baud",
        ],
    },
    FileDef {
        path: "setup/tuner.yaml",
        class: Class::Setup,
        domain: Domain::Tuner,
        schema_version: Some(SCHEMA_VERSION),
        owns: &[
            "tuner.grade_order",
            "tuner.target_grade",
            "tuner.max_retries",
            "tuner.save_captures",
            "tuner.ocr_retries",
            "tuner.spring_mode",
            "tuner.mouse_guard",
            "tuner.guard_px",
            "tuner.recenter_max",
            "tuner.models",
            "tuner.timing",
            "tuner.grade_colors",
            "tuner.filter",
        ],
    },
    FileDef {
        path: "setup/gem.yaml",
        class: Class::Setup,
        domain: Domain::Gem,
        schema_version: Some(SCHEMA_VERSION),
        owns: &[
            "gem.grades",
            "gem.start_grade",
            "gem.empty_mode",
            "gem.empty_streak",
            "gem.colored_gap_min",
            "gem.save_empty_captures",
            "gem.move_mode",
        ],
    },
    FileDef {
        path: "setup/pet.yaml",
        class: Class::Setup,
        domain: Domain::Pet,
        schema_version: Some(SCHEMA_VERSION),
        owns: &["pet.items_per_minute", "pet.pet_slot_occupied_above"],
    },
    FileDef {
        path: "setup/spammer.yaml",
        class: Class::Setup,
        domain: Domain::Spammer,
        schema_version: Some(SCHEMA_VERSION),
        owns: &["spammer"],
    },
    FileDef {
        path: "setup/quest.yaml",
        class: Class::Setup,
        domain: Domain::Quest,
        schema_version: Some(SCHEMA_VERSION),
        owns: &["quest"],
    },
    FileDef {
        path: "setup/buy_sell.yaml",
        class: Class::Setup,
        domain: Domain::BuySell,
        schema_version: Some(SCHEMA_VERSION),
        owns: &[
            "buy_sell.presets",
            "buy_sell.sell_cap",
            // Game property, not a measurement: it ships as a default in defaults.yaml and must
            // never land in machine/. The owner is setup so a config write can route it.
            "buy_sell.shop_rows",
        ],
    },
    // ── machine: measured on this PC, never copied ───────────────────────────────────────────
    FileDef {
        path: "machine/display.yaml",
        class: Class::Machine,
        domain: Domain::Display,
        schema_version: Some(SCHEMA_VERSION),
        owns: &["calibration"],
    },
    FileDef {
        path: "machine/tuner.yaml",
        class: Class::Machine,
        domain: Domain::Tuner,
        schema_version: Some(SCHEMA_VERSION),
        owns: &["tuner.ocr", "tuner.spring_point"],
    },
    FileDef {
        path: "machine/gem.yaml",
        class: Class::Machine,
        domain: Domain::Gem,
        schema_version: Some(SCHEMA_VERSION),
        owns: &[
            "gem.grade_positions",
            "gem.movements",
            "gem.result_gem_area",
            "gem.resource_gems",
            "gem.empty_signature",
            "gem.empty_distance",
        ],
    },
    FileDef {
        path: "machine/buy_sell.yaml",
        class: Class::Machine,
        domain: Domain::BuySell,
        schema_version: Some(SCHEMA_VERSION),
        owns: &[
            "buy_sell.bag_grid",
            "buy_sell.bag_slot",
            "buy_sell.shop_region",
            "buy_sell.scroll_point",
            "buy_sell.max_button",
            "buy_sell.sell_slots",
            "buy_sell.shop_icons",
        ],
    },
    FileDef {
        path: "machine/pet.yaml",
        class: Class::Machine,
        domain: Domain::Pet,
        schema_version: Some(SCHEMA_VERSION),
        owns: &["pet"],
    },
    FileDef {
        path: "machine/tooltip.yaml",
        class: Class::Machine,
        domain: Domain::Tooltip,
        schema_version: Some(SCHEMA_VERSION),
        owns: &["tooltip"],
    },
    FileDef {
        path: "machine/ui.yaml",
        class: Class::Machine,
        domain: Domain::Ui,
        schema_version: Some(SCHEMA_VERSION),
        owns: &["ui"],
    },
    FileDef {
        path: "machine/arduino.yaml",
        class: Class::Machine,
        domain: Domain::Arduino,
        schema_version: Some(SCHEMA_VERSION),
        owns: &["arduino.port"],
    },
    // ── bookkeeping and player notes (not part of the merged load) ──────────────────────────
    FileDef {
        path: "meta.yaml",
        class: Class::Machine,
        domain: Domain::Meta,
        schema_version: Some(SCHEMA_VERSION),
        // The typed `LocalOverrides` still spells the old fold marker under its legacy
        // `settings.legacy_folded`; the new layout keeps it in meta.yaml and never in setup/.
        owns: &["meta", "settings.legacy_folded"],
    },
    FileDef {
        path: "user.yaml",
        class: Class::User,
        domain: Domain::Common,
        schema_version: None,
        owns: &[],
    },
    FileDef {
        path: "artifacts/",
        class: Class::Machine,
        domain: Domain::Artifacts,
        schema_version: None,
        owns: &[],
    },
];

/// The files a future loader merges, in order: the shipped template first, then setup, then machine.
/// `meta.yaml`, `user.yaml`, attributes and artifacts are deliberately not config inputs.
pub fn load_order() -> impl Iterator<Item = &'static FileDef> {
    FILES
        .iter()
        .filter(|file| !file.path.ends_with('/'))
        .filter(|file| matches!(file.class, Class::Defaults | Class::Setup | Class::Machine))
        .filter(|file| file.domain != Domain::Meta)
}

/// True when `path` is `prefix` or lies under it (`prefix.` boundary; a bare string prefix would let
/// `pet` claim `petitioners`).
fn is_under(prefix: &str, path: &str) -> bool {
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

/// The most specific file owning `path`, or `None` when no file claims it. Model paths use the
/// `settings.` spelling of the OLD overlay; callers that walk `seal_domain` should use
/// [`owner_of_model`], which strips that prefix before lookup.
#[must_use]
pub fn owner_of(path: &str) -> Option<&'static FileDef> {
    let mut best: Option<(&'static FileDef, usize)> = None;
    for file in FILES {
        for owned in file.owns {
            if is_under(owned, path) {
                let length = owned.len();
                if best.is_none_or(|(_, current)| length > current) {
                    best = Some((file, length));
                }
            }
        }
    }
    best.map(|(file, _)| file)
}

/// Owner of a path in the TYPED model's spelling: `settings.window.title` is
/// `setup/common.yaml`, `settings.legacy_folded` is `meta.yaml`, everything else is unchanged.
pub fn owner_of_model(path: &str) -> Option<&'static FileDef> {
    if let Some(file) = owner_of(path) {
        return Some(file);
    }
    path.strip_prefix("settings.").and_then(owner_of)
}

/// The `setup/` files in copy order; this is what a future "export my setup" copies.
pub fn portable_files() -> impl Iterator<Item = &'static FileDef> {
    FILES.iter().filter(|file| file.class == Class::Setup)
}

/// The machine files, i.e. everything a copied setup must NOT carry.
pub fn machine_files() -> impl Iterator<Item = &'static FileDef> {
    FILES
        .iter()
        .filter(|file| file.class == Class::Machine && !file.path.ends_with('/'))
}

/// Pairs of owned prefixes where one is nested under another. Nesting is allowed (the more specific
/// owner wins), but every such pair is intentional and pinned by a test so a new broad claim cannot
/// silently shadow a tool file.
#[must_use]
pub fn overlapping_owners() -> Vec<(&'static str, &'static str)> {
    let mut overlaps = Vec::new();
    for (index, left) in FILES.iter().enumerate() {
        for right in &FILES[index + 1..] {
            for left_path in left.owns {
                for right_path in right.owns {
                    if is_under(left_path, right_path) || is_under(right_path, left_path) {
                        let (first, second) = if left.path <= right.path {
                            (left.path, right.path)
                        } else {
                            (right.path, left.path)
                        };
                        overlaps.push((first, second));
                    }
                }
            }
        }
    }
    overlaps.sort_unstable();
    overlaps.dedup();
    overlaps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_file_path_is_unique_and_classed() {
        let mut paths = FILES.iter().map(|file| file.path).collect::<Vec<_>>();
        paths.sort_unstable();
        let before = paths.len();
        paths.dedup();
        assert_eq!(before, paths.len(), "a file path appears twice");

        for file in FILES {
            let expected = match file.class {
                Class::Defaults | Class::Release | Class::User => None,
                Class::Setup | Class::Machine => Some(SCHEMA_VERSION),
            };
            if file.path.ends_with('/') {
                assert_eq!(file.class, Class::Machine);
                continue;
            }
            assert_eq!(
                file.schema_version, expected,
                "{}: written data files state schema_version {SCHEMA_VERSION}",
                file.path
            );
            assert!(!file.path.starts_with('/'), "relative path");
        }
    }

    #[test]
    fn the_load_order_is_defaults_setup_then_machine() {
        let order = load_order().map(|file| file.path).collect::<Vec<_>>();
        assert_eq!(order.first(), Some(&"defaults.yaml.example"));
        let setup_end = order.iter().rposition(|path| path.starts_with("setup/"));
        let machine_start = order.iter().position(|path| path.starts_with("machine/"));
        assert!(setup_end.is_some() && machine_start.is_some());
        assert!(
            setup_end.unwrap() < machine_start.unwrap(),
            "setup comes before machine: {order:?}"
        );
        assert!(!order.contains(&"meta.yaml"));
        assert!(!order.contains(&"user.yaml"));
    }

    #[test]
    fn the_only_nested_claims_are_the_documented_pet_and_marker_cases() {
        // `machine/pet.yaml` owns the whole pet block; `setup/pet.yaml` owns the two portable
        // preferences inside it. `meta.yaml` owns one legacy-`settings` marker, which no setup file
        // claims. Anything else nested would shadow a tool file silently.
        let expected = vec![("machine/pet.yaml", "setup/pet.yaml")];
        assert_eq!(overlapping_owners(), expected);
    }

    #[test]
    fn the_typed_model_paths_route_to_the_right_files() {
        assert_eq!(
            owner_of_model("settings.window.title").unwrap().path,
            "setup/common.yaml"
        );
        assert_eq!(
            owner_of_model("settings.legacy_folded").unwrap().path,
            "meta.yaml"
        );
        assert_eq!(
            owner_of_model("tuner.ocr.region").unwrap().path,
            "machine/tuner.yaml"
        );
        assert_eq!(
            owner_of_model("tuner.target_grade").unwrap().path,
            "setup/tuner.yaml"
        );
        assert_eq!(
            owner_of_model("pet.items_per_minute").unwrap().path,
            "setup/pet.yaml"
        );
        assert_eq!(
            owner_of_model("pet.slots").unwrap().path,
            "machine/pet.yaml"
        );
        assert_eq!(owner_of_model("unknown.block").map(|file| file.path), None);
    }

    #[test]
    fn a_bare_prefix_does_not_claim_a_longer_name() {
        assert!(owner_of("pets").is_none(), "pet must not claim pets");
        assert!(owner_of("tooltipx").is_none());
    }
}

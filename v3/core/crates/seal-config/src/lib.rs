//! The three-layer config file, ported from v2's `ConfigLoader`.
//!
//! `defaults.yaml.example` is the base, `local.yaml` is the per-machine overlay (with the one-time
//! fold marker), `attributes.yaml` is the read-only OCR dictionary. The writer keeps the constant
//! header and every foreign top-level block verbatim, and writes atomically — those are the
//! measured rules from `v2/docs/spec/CONFIG.md` and `PLAN-V3-CORE.md` §6.1.
//!
//! The YAML crate is `serde_norway`, a maintained fork of the deprecated `serde_yaml`; it is used
//! behind this crate alone, so swapping it stays one file's work.
//!
//! Three v3 choices are worth stating because they are not literal line-by-line ports:
//!
//! * **The config is opaque JSON** (`PROTOCOL.md` §7). The loader merges YAML documents as
//!   `serde_json::Value` trees rather than through a typed `AppConfig`; the YAML↔JSON boundary is
//!   [`yaml`] and nothing else in the crate names the YAML crate.
//! * **The portable sections are derived from the template itself**, not from a hard-coded field
//!   list: every top-level section the shipped `defaults.yaml.example` carries is portable (the
//!   template is the definition of "the same on every machine" — `CONFIG.md`). A save projects the
//!   merged config onto that shape, which is how a machine-only field such as `tuner.ocr` cannot
//!   leak into the `settings:` block.
//! * **Merging is a recursive field-wise overlay, null means "not stated"**. v2's typed apply had
//!   a mix of whole-object and per-field assignments; the observable rule it documented was "a key
//!   the block does not mention leaves the shipped one standing, and a value it states as empty is
//!   a choice" (`ConfigLoader.ApplySettings`). A recursive overlay is that rule without the type
//!   list, and `null` in YAML is how v2's writer spells "unset", so it is treated as silence.

pub mod error;
pub mod file;
pub mod foreign;
pub mod layout;
pub mod registry;
pub mod store;
pub mod yaml;

pub use error::ConfigError;
pub use layout::{Loaded, load as load_layout};
pub use registry::{Class, Domain, FILES, FileDef, SCHEMA_VERSION};
pub use store::{ConfigStore, PetRunStatePatch};

/// The top-level blocks of `local.yaml` this application owns and rewrites, ported verbatim from
/// `ConfigLoader.OwnedLocalBlocks`. Every other top-level block is foreign: carried through a save
/// verbatim, comments included, so a hand-added or future block survives (`CONFIG.md`).
pub const OWNED_LOCAL_BLOCKS: &[&str] = &[
    "calibration",
    "tuner",
    "gem",
    "arduino",
    "ui",
    "spammer",
    "buy_sell",
    "pet",
    "tooltip",
    "quest",
    "settings",
];

/// The header every save writes above the serialised content, ported from
/// `ConfigLoader.LocalHeader` (the string literals are kept one per line so the constant is
/// independent of the checkout's line endings).
///
/// One sentence is deliberately reworded, and the reason is worth keeping: v2 rebuilt every owned
/// block from a typed projection, so a key it did not model was silently DROPPED — four recorded
/// losses. In v3 the config is opaque JSON and the writer merges the values it was given, so no key
/// disappears inside an owned block. The header therefore says such a key is KEPT. Leaving v2's
/// sentence would put a false warning in the player's own file. The rest is byte-identical, and its
/// other half — comments cannot survive a save — is still true and still said.
pub const LOCAL_HEADER: &str = concat!(
    "# ── This machine's configuration ──────────────────────────────────────────────────\n",
    "# Two halves, and they travel differently.\n",
    "#\n",
    "# The MEASURED half: coordinates, the display environment, the Arduino port override, and the\n",
    "# calibration images beside this file. True only on THIS machine.\n",
    "#\n",
    "# The `settings:` block: the PORTABLE half — the same values on every machine. It lives here\n",
    "# rather than in defaults.yaml because this file is YOURS. defaults.yaml is the template\n",
    "# publish.bat ships, so anything saved there would be replaced by the next update. A hand-added\n",
    "# spammer preset is personal too, and that is why presets live here as well.\n",
    "#\n",
    "# Gitignored, and excluded from the public zip by publish.bat — do not commit it or share it.\n",
    "#\n",
    "# READ THIS BEFORE EDITING. A save REWRITES this whole file's owned blocks — the ones named in\n",
    "# docs/spec/CONFIG.md — and leaves any other TOP-LEVEL block alone, so a block of your own survives.\n",
    "# Inside an owned block a key this build does not read is KEPT: a save merges the values it was\n",
    "# given rather than rebuilding the block from a fixed list. Comments are still lost everywhere,\n",
    "# though: the writer serialises values and cannot carry prose. Keep a copy if you edit.\n",
    "#\n",
    "# Documented in docs/spec/CONFIG.md; calibrating is docs/spec/CALIBRATION.md.\n",
    "\n",
);

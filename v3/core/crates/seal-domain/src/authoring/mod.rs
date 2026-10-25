//! The authoring rules: the ones the v2 launcher kept inline in `MainWindow.xaml.cs`, moved into the
//! domain as pure functions returning structured errors.
//!
//! `PLAN-V3-CORE.md` §4.1: "The authoring rules the launcher currently carries — Quest Save refusing
//! a flow/sequence entry that names a missing flow and listing it, Duplicate/rename/re-point,
//! preset-name and hotkey-name validation, game-variant switching, Setup's blank-client-size refusal
//! … move into `seal-domain` and are exercised through `config.validate`/`config.apply`. **No widget
//! keeps a rule**, which is the failure class this plan exists to remove."
//!
//! Two properties here are the point rather than a nicety:
//!
//! - **Every refusal is structured and carries the player-facing message.** A rule that returns a
//!   bare bool puts the wording back in the widget, where it can disagree with another widget's copy
//!   of the same rule — the three hand-rolled pickers v2 had to collapse into one.
//! - **Nothing here performs I/O.** These run inside `config.validate` and `config.apply`, upstream
//!   of any file write, so an authoring error can never leave a half-written config.

mod hotkey;
mod name;
mod quest;
mod setup;
mod variant;

pub use hotkey::{
    HotkeyError, HotkeyNameError, HotkeyNames, parse_hotkeys, parse_virtual_key, virtual_key_name,
};
pub use name::{NameError, PresetNoun, copy_name, validate_name};
pub use quest::{
    DanglingEntry, PresetAction, PresetError, QuestSaveError, check_saveable,
    dangling_sequence_entries, duplicate_flow, duplicate_sequence, quest_step_complaint,
    remove_flow, rename_flow, rename_sequence,
};
pub use setup::{ClientSize, ClientSizeError, ClientSizeField, validate_client_size};
pub use variant::{VariantError, VariantSwitch, plan_variant_switch, resolve_window_title};

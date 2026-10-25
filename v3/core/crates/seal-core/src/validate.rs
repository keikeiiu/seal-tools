//! The authoring rules a tab's Save runs, applied to a COPY of the merged config.
//!
//! `PROTOCOL.md` §4's `config.validate` is the shell's pre-Save gate, and `config.apply` runs the
//! same checks before it writes — `PLAN-V3-CORE.md` §4.1: "No widget keeps a rule". The rules
//! themselves are pure functions in `seal-domain::authoring`; this module only decides which of them
//! a given `sections` payload would exercise, and turns their structured refusals into the
//! player-facing lines the protocol carries (one line per array entry).
//!
//! Which rules run is a SAVE question, not a run question:
//!
//! - `quest` — the Quest tab's Save gate ([`check_saveable`]): every sequence entry must name a flow
//!   that exists. A dangling entry is silent on screen and the run would silently skip it.
//! - `settings.hotkeys` — the Hotkeys tab's Save parses the four names it shows back to key codes
//!   ([`parse_hotkeys`]). The config stores codes, so the names are reconstructed with
//!   [`virtual_key_name`] — the same text the tab displays — and every box that cannot be parsed
//!   back is refused. This is the only way that rule fits a config section: the section carries VK
//!   codes, not the player's typed text.
//! - `settings.game.variant` — the Setup tab's client switch resolves the name before committing
//!   ([`plan_variant_switch`]): an unknown client is refused with the known ones listed.
//!
//! Deliberately NOT wired here, and why:
//!
//! - The picker's name rules ([`seal_domain::authoring::validate_name`] and the duplicate / rename /
//!   delete operations) act on a picker SELECTION, not on a saved section — a config block cannot
//!   carry a stale selection or a duplicate map key.
//! - Setup's blank-client-size rule (`validate_client_size`) validates the text boxes the Detect
//!   button measured; no protocol section carries that text, so there is nothing to run it on.
//! - The quest step rule (`quest_step_complaint`) is enforced when a RUN starts, not by the tab's
//!   Save; it belongs to `tool.start`, not to a config write.
//!
//! Warnings: no rule in `seal-domain` currently has an advisory result — every one refuses — so the
//! `warnings` array is the fixed protocol shape with nothing to put in it yet.

use seal_domain::authoring::{
    HotkeyNames, check_saveable, parse_hotkeys, plan_variant_switch, virtual_key_name,
};
use seal_domain::config::{GameConfig, HotkeysConfig, QuestConfig};
use serde_json::{Map, Value};

use crate::dispatch::CoreState;

const SETTINGS: &str = "settings";
const LEGACY_FOLDED: &str = "legacy_folded";

/// What a Save would report: player-facing lines, one per array entry.
#[derive(Debug, Default)]
pub struct Outcome {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Runs the rules a save of `sections` would run, against a copy of the current merged config.
///
/// The copy is what makes this side-effect free: the given sections are laid onto it exactly the way
/// the merge will lay them onto the file (an owned top-level block replaces that block, `settings`
/// overlays the portable sections field-wise), and only the copy is inspected. Nothing is written.
pub fn validate_sections(state: &CoreState, sections: &Map<String, Value>) -> Outcome {
    let effective = effective_config(&state.config, sections);
    let settings = sections.get(SETTINGS).and_then(Value::as_object);
    let mut outcome = Outcome::default();

    if sections.contains_key("quest") {
        outcome.errors.extend(quest_errors(&effective));
    }
    if let Some(settings) = settings {
        if settings.contains_key("hotkeys") {
            outcome.errors.extend(hotkey_errors(&effective));
        }
        if settings
            .get("game")
            .and_then(Value::as_object)
            .is_some_and(|game| game.contains_key("variant"))
        {
            outcome.errors.extend(variant_errors(&effective, settings));
        }
    }

    outcome
}

/// The merged config as it would read after this save — on a copy, so a rule can never touch the
/// session view or the file.
fn effective_config(current: &Value, sections: &Map<String, Value>) -> Value {
    let mut config = current.clone();
    let Some(config_object) = config.as_object_mut() else {
        return config;
    };

    for (name, value) in sections {
        if name == SETTINGS {
            let Some(settings) = value.as_object() else {
                continue;
            };
            for (section, section_value) in settings {
                // The fold marker is data in the settings block, not a config section.
                if section == LEGACY_FOLDED || section_value.is_null() {
                    continue;
                }
                match config_object.get_mut(section) {
                    Some(target) => merge_overlay(target, section_value),
                    None => {
                        config_object.insert(section.clone(), section_value.clone());
                    }
                }
            }
        } else {
            // A tab saves its whole working copy of the block it owns; the merge replaces the block.
            config_object.insert(name.clone(), value.clone());
        }
    }

    config
}

/// Recursive field-wise overlay, the same rule `seal-config`'s load applies: objects merge, a value
/// that is stated (including `0` and `""`) replaces, and `null` means "not stated". It is repeated
/// here rather than exposed from the config crate because it is the protocol's merge semantics, not
/// a file detail — a validation that saw different values from the reload would be lying to the
/// shell.
fn merge_overlay(target: &mut Value, overlay: &Value) {
    match overlay {
        Value::Object(overlay_object) => match target {
            Value::Object(target_object) => {
                for (key, value) in overlay_object {
                    if value.is_null() {
                        continue;
                    }
                    match target_object.get_mut(key) {
                        Some(slot) => merge_overlay(slot, value),
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

/// The Quest tab's Save gate. Every dangling sequence entry is one line, and the domain's closing
/// instruction is its own line — the same wording the v2 barrier showed, split for the protocol's
/// one-line-per-entry contract.
fn quest_errors(config: &Value) -> Vec<String> {
    let Some(quest) = config.get("quest") else {
        return Vec::new();
    };
    let quest: QuestConfig = match serde_json::from_value(quest.clone()) {
        Ok(quest) => quest,
        Err(error) => return vec![malformed("quest", &error)],
    };

    match check_saveable(&quest) {
        Ok(()) => Vec::new(),
        Err(error) => error.message().lines().map(str::to_owned).collect(),
    }
}

/// The Hotkeys tab's Save rule, applied to the names the tab would show for the stored codes.
fn hotkey_errors(config: &Value) -> Vec<String> {
    let Some(hotkeys) = config.get("hotkeys") else {
        return Vec::new();
    };
    let hotkeys: HotkeysConfig = match serde_json::from_value(hotkeys.clone()) {
        Ok(hotkeys) => hotkeys,
        Err(error) => return vec![malformed("hotkeys", &error)],
    };

    let start = shown_hotkey(hotkeys.start);
    let quit = shown_hotkey(hotkeys.quit);
    let advance_grade = shown_hotkey(hotkeys.advance_grade);
    let pause = shown_hotkey(hotkeys.pause);
    let names = HotkeyNames {
        start: &start,
        quit: &quit,
        advance_grade: &advance_grade,
        pause: &pause,
    };

    match parse_hotkeys(names) {
        Ok(_) => Vec::new(),
        Err(errors) => errors.iter().map(|error| error.message()).collect(),
    }
}

/// The name the Hotkeys tab shows for one stored code (`VkName`): the value the player edits, and
/// therefore the text the rule quotes when it refuses. A code outside the VK name table reads as
/// `0x…`, which cannot be parsed back — exactly what makes such a code unsaveable.
fn shown_hotkey(code: i32) -> String {
    match u32::try_from(code) {
        Ok(vk) => virtual_key_name(vk),
        Err(_) => code.to_string(),
    }
}

/// The Setup tab's client switch: the variant must be one of the configured clients.
fn variant_errors(config: &Value, settings: &Map<String, Value>) -> Vec<String> {
    let requested = settings
        .get("game")
        .and_then(Value::as_object)
        .and_then(|game| game.get("variant"));
    let Some(requested) = requested else {
        return Vec::new();
    };
    if requested.is_null() {
        return Vec::new();
    }
    let Some(requested) = requested.as_str() else {
        return vec!["the game block is malformed: variant must be a string".to_owned()];
    };

    let game: GameConfig = match config.get("game") {
        Some(game) => match serde_json::from_value(game.clone()) {
            Ok(game) => game,
            Err(error) => return vec![malformed("game", &error)],
        },
        None => GameConfig::default(),
    };

    match plan_variant_switch(&game, requested) {
        Ok(_) => Vec::new(),
        Err(error) => vec![error.message()],
    }
}

fn malformed(section: &str, error: &serde_json::Error) -> String {
    format!("the {section} block is malformed: {error}")
}

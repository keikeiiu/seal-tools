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
//! - `settings.game.variant` — the Setup tab's client switch resolves the name before committing
//!   ([`plan_variant_switch`]): an unknown client is refused with the known ones listed.
//!
//! Deliberately NOT wired here, and why:
//!
//! - **Hotkeys.** The config stores virtual-key CODES, and v2 had no rule for them: `GetAsyncKeyState`
//!   accepts any VK code, and `ConfigValidator.cs` has no hotkey check, so a hand-edited config with
//!   a code that has no display name (`0x05`) ran fine. The Hotkeys tab's name rule
//!   (`parse_hotkeys`) validated the TEXT the player typed into the four boxes, before it became a
//!   code — that is an input-editing rule, and no protocol section carries the typed text. A
//!   typed-name validation command is the open item; refusing a stored code here would be a
//!   behaviour regression dressed as validation.
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

use seal_domain::authoring::{check_saveable, plan_variant_switch};
use seal_domain::config::{GameConfig, QuestConfig};
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
/// the merge will lay them onto the file (an owned top-level block merges FIELD-WISE into the current
/// block, `settings` overlays the portable sections field-wise — PROTOCOL §7), and only the copy is
/// inspected. Nothing is written.
pub fn validate_sections(state: &CoreState, sections: &Map<String, Value>) -> Outcome {
    let effective = effective_config(&state.config, sections);
    let settings = sections.get(SETTINGS).and_then(Value::as_object);
    let mut outcome = Outcome::default();

    if sections.contains_key("quest") {
        outcome.errors.extend(quest_errors(&effective));
    }
    if let Some(settings) = settings
        && settings
            .get("game")
            .and_then(Value::as_object)
            .is_some_and(|game| game.contains_key("variant"))
    {
        outcome.errors.extend(variant_errors(&effective, settings));
    }

    outcome
}

/// The merged config as it would read after this save — on a copy, so a rule can never touch the
/// session view or the file.
pub(crate) fn effective_config(current: &Value, sections: &Map<String, Value>) -> Value {
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
            // PROTOCOL §7's rule: the write merges an owned block FIELD-WISE into the current block,
            // so the copy a rule judges must be the config the write actually produces. Replacing the
            // block here would judge a partial save against a block it never creates — the bug this
            // exists to close (OPEN.md §3).
            match config_object.get_mut(name) {
                Some(target) => merge_overlay(target, value),
                None => {
                    config_object.insert(name.clone(), value.clone());
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// PROTOCOL §7's merge rule is FIELD-WISE for owned blocks too: the writer merges into the
    /// current block, so the copy a rule judges must keep the fields the request did not state. A
    /// replace-copy would judge `{"pet": {"food_load_mode": "drag"}}` and lose the feeder's run
    /// state — a config the write will never produce.
    #[test]
    fn an_owned_block_merges_field_wise_like_the_writer() {
        let current = json!({
            "pet": {"food_load_mode": "right_click", "boarding_window_may_be_open": true},
        });
        let mut sections = Map::new();
        sections.insert("pet".to_owned(), json!({"food_load_mode": "drag"}));

        let effective = effective_config(&current, &sections);

        assert_eq!(effective["pet"]["food_load_mode"], "drag");
        assert_eq!(
            effective["pet"]["boarding_window_may_be_open"], true,
            "the fields the request did not state are still there"
        );
    }

    /// A stated value (including 0 and "") replaces; `null` means "not stated", exactly the loader's
    /// rule — the copy must not invent a different one.
    #[test]
    fn a_null_leaf_is_not_a_statement() {
        let current = json!({"quest": {"active": "run1", "guard_px": 12}});
        let mut sections = Map::new();
        sections.insert("quest".to_owned(), json!({"guard_px": null, "active": ""}));

        let effective = effective_config(&current, &sections);

        assert_eq!(effective["quest"]["guard_px"], 12, "null is not stated");
        assert_eq!(
            effective["quest"]["active"], "",
            "an empty string IS stated"
        );
    }
}

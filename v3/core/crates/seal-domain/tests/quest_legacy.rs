//! The save-path regression for migrated v2 quest values (task-67).
//!
//! `config.apply {quest: {active: ...}}` validates the quest block through the domain's authoring
//! rule. A v2 file can carry a numeric/bool step scalar (`value: 1`), which used to fail the block
//! decode and refuse every save. This test decodes the local block and runs the same domain rule the
//! validator uses, without the malformed-decode refusal.

use std::collections::BTreeMap;

use seal_domain::authoring::check_saveable;
use seal_domain::config::{LocalQuest, QuestConfig};
use serde_json::json;

#[test]
fn a_v2_scalar_quest_value_does_not_block_a_save() {
    let local: LocalQuest = serde_json::from_value(json!({
        "active": "god8",
        "presets": {
            "god8": {
                "initial_wait_seconds": 10.0,
                "loops": 1,
                "steps": [
                    {"action": "key", "value": 1, "delay_seconds": 0.25},
                    {"action": "enter", "value": null, "delay_seconds": 0.25}
                ]
            }
        }
    }))
    .expect("a migrated local quest block decodes");

    let preset = local
        .presets
        .as_ref()
        .and_then(|presets| presets.get("god8"))
        .expect("the flow survives the decode");
    assert_eq!(preset.steps[0].value.as_deref(), Some("1"));
    assert_eq!(preset.steps[1].value, None);

    // The exact rule `config.validate`/`config.apply` runs on the quest section: no malformed
    // decode error can reach it because the shape above now decodes first.
    let quest = QuestConfig {
        active: "god8".to_owned(),
        presets: BTreeMap::from([("god8".to_owned(), preset.clone())]),
        ..QuestConfig::default()
    };
    assert!(check_saveable(&quest).is_ok());
}

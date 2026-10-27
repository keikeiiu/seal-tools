//! The sanitized v2-era config replay proof (task-67): the committed fixture is a representative
//! migrated `local.yaml` -- generic geometry, no player values, no real crops -- and must decode
//! through the typed config and the readers the v3 tools use.

use seal_domain::authoring::check_saveable;
use seal_domain::config::{LocalOverrides, QuestConfig};
use serde_json::Value;

const FIXTURE: &str = include_str!("fixtures/v2-era-config.json");

#[test]
fn a_sanitized_v2_era_config_decodes_into_the_typed_local_overlay() {
    let config: Value = serde_json::from_str(FIXTURE).expect("the fixture is JSON");
    let local: LocalOverrides =
        serde_json::from_value(config.clone()).expect("the v2-era overlay decodes");

    let ocr = local
        .tuner
        .as_ref()
        .and_then(|tuner| tuner.ocr.as_ref())
        .expect("tuner OCR geometry");
    assert_eq!(ocr.region.as_ref().map(|region| region.left), Some(100));
    assert_eq!(ocr.grade_area.as_ref().map(|area| area.x2), Some(100));
    assert_eq!(ocr.grade_y, vec![5, 45]);
    assert_eq!(ocr.attr_y, vec![60, 200]);
    assert_eq!(ocr.remaining_y, vec![210, 230]);
    assert_eq!(ocr.attr_x, vec![5, 275]);
    assert_eq!(ocr.remaining_x, vec![5, 275]);
    assert_eq!(ocr.row_height, 19);

    let pet = local.pet.as_ref().expect("pet block");
    assert_eq!(pet.page_tabs.as_ref().map(Vec::len), Some(3));
    assert_eq!(pet.slots.as_ref().map(Vec::len), Some(2));
    assert_eq!(pet.queue.as_ref().map(Vec::len), Some(2));
    assert_eq!(pet.food_slots.as_ref().map(Vec::len), Some(2));
    assert_eq!(pet.return_slot.as_deref(), Some(&[0, 5][..]));
    assert_eq!(pet.food_load_mode.as_deref(), Some("right_click"));

    let buy = local.buy_sell.as_ref().expect("buy/sell block");
    assert_eq!(buy.bag_grid.as_deref(), Some(&[0, 0, 800, 600][..]));
    assert_eq!(buy.shop_rows, Some(5));
    assert_eq!(buy.sell_slots.as_deref(), Some(&[1, 2][..]));
    assert_eq!(buy.sell_cap, Some(5));

    let gem = local.gem.as_ref().expect("gem block");
    assert_eq!(
        gem.result_gem_area.as_deref(),
        Some(&[100, 100, 200, 60][..])
    );
    assert_eq!(
        gem.grade_positions
            .as_ref()
            .map(|positions| positions.len()),
        Some(5)
    );
    assert_eq!(gem.resource_gems.as_ref().map(Vec::len), Some(3));

    let spammer = local.spammer.as_ref().expect("spammer block");
    assert_eq!(
        spammer.priority.as_ref().map(|map| map["run"].len()),
        Some(2)
    );
    assert_eq!(
        spammer.startup.as_ref().map(|map| map["run"].step),
        Some(0.1)
    );

    assert_eq!(
        local
            .calibration
            .as_ref()
            .and_then(|c| c.measured_at.as_deref()),
        Some("2026-01-01T00:00:00")
    );
    assert_eq!(local.ui.as_ref().and_then(|ui| ui.width), Some(900.0));
    assert_eq!(
        local
            .settings
            .as_ref()
            .and_then(|s| s.reference_window.as_ref())
            .map(|w| w.width),
        Some(1926)
    );
}

#[test]
fn the_v2_numeric_quest_step_value_decodes_and_saves() {
    let local: LocalOverrides = serde_json::from_str(FIXTURE).expect("the fixture decodes");
    let quest = local.quest.expect("quest block");
    let step = &quest.presets.as_ref().expect("presets")["god8"].steps[0];
    assert_eq!(step.value.as_deref(), Some("1"));
    assert_eq!(
        quest.presets.as_ref().expect("presets")["god8"].steps[1].value,
        None
    );

    let config = QuestConfig {
        active: "god8".to_owned(),
        presets: quest.presets.unwrap(),
        ..QuestConfig::default()
    };
    assert!(
        check_saveable(&config).is_ok(),
        "the save rule sees no malformed body"
    );
}

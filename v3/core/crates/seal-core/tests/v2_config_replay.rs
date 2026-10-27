//! Core-side replay proof for the sanitized v2-era config fixture (task-67): the same pointed
//! config is fed through the readers the tuner, pet, buy/sell, gem and spammer tools actually use.

use std::path::Path;

use seal_core::{gem, pet, shop, spammer, tuner};
use seal_domain::config::LocalOverrides;
use seal_domain::geometry::Rect;
use serde_json::Value;

const FIXTURE: &str = include_str!("../../seal-domain/tests/fixtures/v2-era-config.json");

#[test]
fn the_sanitized_v2_era_config_replays_through_the_tool_readers() {
    let config: Value = serde_json::from_str(FIXTURE).expect("the fixture is JSON");
    let local: LocalOverrides =
        serde_json::from_value(config.clone()).expect("the overlay decodes");

    // Tuner: the OCR region and spring point survive into the plan the tool runs.
    let tuner = tuner::plan(&config).expect("tuner plan");
    assert_eq!(tuner.region, Rect::new(100, 200, 280, 232));
    assert_eq!(
        tuner.spring,
        Some(seal_domain::geometry::Point::new(500, 400))
    );
    assert!(local.tuner.as_ref().and_then(|t| t.ocr.as_ref()).is_some());

    // Pet: every configured row/queue/food slot is seen by the plan.
    let pet = pet::plan(&config, Path::new(".")).expect("pet plan");
    assert_eq!(pet.rows.len(), 2);
    assert_eq!(pet.queue.len(), 2);
    assert_eq!(pet.food_slots.len(), 2);
    assert_eq!(
        pet.rows[0].boarding_pet_slot,
        Some(Rect::new(10, 60, 100, 80))
    );
    assert_eq!(pet.setup.food_load_mode, "right_click");

    // Buy/sell: the saved geometry and the v2-era selected slots are the ones the plan sees.
    let shop = shop::settings(&config);
    assert_eq!(shop.bag_grid, Some(vec![0, 0, 800, 600]));
    assert_eq!(shop.shop_rows, 5);
    assert_eq!(shop.sell_slots, vec![1, 2]);
    assert!(shop::sell_plan(&config).is_ok(), "sell plan resolves");

    // Gem: the saved grade/resource marks and result area survive.
    let gem = gem::plan(&config).expect("gem plan");
    assert_eq!(gem.result_area, Some(Rect::new(100, 100, 200, 60)));
    assert_eq!(gem.settings.grade_positions.len(), 5);
    assert_eq!(gem.settings.resource_gems.len(), 3);

    // Spammer: the active preset and its priority/combos/startup are the configured ones.
    let (configured, active, rotation) = spammer::rotation(&config);
    assert!(configured);
    assert_eq!(active, "run");
    let rotation = rotation.expect("the active rotation resolves");
    assert_eq!(rotation.priority, vec!["E".to_owned(), "Q".to_owned()]);
    assert_eq!(rotation.combos.len(), 1);
    assert_eq!(rotation.startup.step, 0.1);
}

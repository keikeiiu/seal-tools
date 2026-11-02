//! Core-side replay proof for the sanitized v2-era config fixture (task-67): the same pointed
//! config is fed through the readers the tuner, pet, buy/sell, gem and spammer tools actually use.

use std::path::Path;

use seal_core::{gem, pet, shop, spammer, tuner};
use seal_domain::config::LocalOverrides;
use seal_domain::geometry::Rect;
use seal_ocr::modes::GradeArea;
use serde_json::Value;

const FIXTURE: &str = include_str!("../../seal-domain/tests/fixtures/v2-era-config.json");

#[test]
fn the_sanitized_v2_era_config_replays_through_the_tool_readers() {
    let config: Value = serde_json::from_str(FIXTURE).expect("the fixture is JSON");
    let local: LocalOverrides =
        serde_json::from_value(config.clone()).expect("the overlay decodes");

    // Tuner: the OCR region, the region-relative v2 grade box and the spring point reach the
    // geometry/plan the tool runs. The fixture's box must be inside its region.
    let geometry = seal_core::dispatch::ocr_geometry(&config);
    assert_eq!(
        geometry.grade_area,
        Some(GradeArea {
            x1: 10,
            y1: 5,
            x2: 100,
            y2: 45
        })
    );
    assert_eq!(geometry.grade_y, Some((5, 45)));

    let mut without_area = config.clone();
    without_area["tuner"]["ocr"]
        .as_object_mut()
        .expect("ocr object")
        .remove("grade_area");
    assert_eq!(
        seal_core::dispatch::ocr_geometry(&without_area).grade_area,
        None,
        "an absent box leaves the grade_y fallback in place"
    );
    assert_eq!(
        seal_core::dispatch::ocr_geometry(&without_area).grade_y,
        Some((5, 45))
    );

    let mut outside_region = config.clone();
    outside_region["tuner"]["ocr"]["grade_area"]["x2"] = serde_json::json!(9999);
    assert_eq!(
        seal_core::dispatch::ocr_geometry(&outside_region).grade_area,
        None,
        "an out-of-region box is invalid and falls back"
    );

    for (field, value) in [
        ("x2", serde_json::json!(5)),
        ("x1", serde_json::json!(-1)),
        ("y2", serde_json::json!(9999)),
    ] {
        let mut invalid = config.clone();
        invalid["tuner"]["ocr"]["grade_area"][field] = value;
        assert_eq!(
            seal_core::dispatch::ocr_geometry(&invalid).grade_area,
            None,
            "an invalid {field} falls back to grade_y"
        );
    }

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

    // Buy/sell: the saved geometry survives; the v2-era sell_slots field is model/migration data,
    // NOT a run selection. A start with no `slots` must answer the domain's own refusal.
    let shop = shop::settings(&config);
    assert_eq!(shop.bag_grid, Some(vec![0, 0, 800, 600]));
    assert_eq!(shop.shop_rows, 5);
    assert!(
        shop.sell_slots.is_empty(),
        "the run settings never adopt the saved field"
    );
    assert_eq!(
        local
            .buy_sell
            .as_ref()
            .and_then(|buy| buy.sell_slots.as_deref()),
        Some(&[1, 2][..]),
        "the typed model keeps the field for migration compatibility"
    );
    let refusal = shop::sell_plan(&config).expect_err("a saved selection is not run data");
    assert_eq!(refusal.message, "No slots are selected to sell.");
    let plan = shop::sell_plan_with_slots(&config, Some(&[1, 2])).expect("run slots plan");
    assert_eq!(plan.done_message, "Sold 2 slot(s).");

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

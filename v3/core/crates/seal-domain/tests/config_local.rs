//! The config model, pinned against `v2/SealTools.Core/Config/ConfigLoader.cs` and
//! `AppConfig.cs`.
//!
//! `local.yaml` compatibility is a hard requirement (`PLAN-V3-CORE.md` §5), so the fixtures here are
//! the shapes those files actually contain: the local overlay's eleven blocks plus the portable
//! `settings:` mirror. The JSON is the same data model the YAML loader feeds (serde has one model
//! for both); the YAML half of the proof lives in `seal-config`, which owns the file I/O.
//!
//! The values are synthetic or taken from the SHIPPED `v2/config/defaults.yaml` template. Nothing
//! here comes from `v2/config/local.yaml`, which is the player's personal state.

// The full `local.yaml` fixture is one deeply nested `json!` literal; the default macro recursion
// budget is not enough for it.
#![recursion_limit = "256"]

use seal_domain::config::{
    BuyPreset, GameConfig, HotkeysConfig, LocalOverrides, LocalSettings, QuestConfig, QuestPreset,
    QuestSequence, QuestStep, SpammerCombo, SpammerStartup, TooltipConfig,
};
use serde_json::{Value, json};

/// A fully populated `local.yaml`, in the exact field spellings v2 writes via
/// `UnderscoredNamingConvention`. Every field is present so the round-trip below proves the model
/// keeps all of them — the "silently dropped field" bug this file's doc comment says it paid for
/// four times.
///
/// One `json!` table by design: the point is a single hand-checkable mirror of the file, and splitting
/// it would hide which block a missing field came from.
#[allow(
    clippy::too_many_lines,
    reason = "the fixture is one hand-checkable mirror of the whole local.yaml file"
)]
fn full_local_shape() -> Value {
    json!({
        "calibration": {
            "dpi_scale": 1.5,
            "monitor_dpi": 144,
            "screen": [3840, 2160],
            "client_size": [2865, 1789],
            "measured_at": "2026-10-25 19:00:00"
        },
        "tuner": {
            "ocr": {
                "region": {"left": 1140, "top": 840, "width": 300, "height": 320},
                "grade_area": {"x1": 149, "y1": 1, "x2": 232, "y2": 44},
                "grade_y": [1, 44],
                "attr_y": [42, 140],
                "remaining_y": [190, 235],
                "attr_x": [10, 20],
                "remaining_x": [30, 40],
                "row_height": 25
            },
            "spring_point": [300, 400]
        },
        "gem": {
            "grade_positions": {"N": [727, 696], "G": [777, 696]},
            "movements": {
                "radio_to_register": {"register": [10, 20], "combine": [30, 40]},
                "register_combine": [1, 2],
                "combine_register": [3, 4],
                "grade_next": [5, 6],
                "grade_prev": [7, 8],
                "register_slot1": [9, 10],
                "slot1_slot2": [11, 12],
                "slot2_slot3": [13, 14],
                "slot3_to_n": [15, 16],
                "slot3_to_g": [17, 18],
                "slot3_to_dg": [19, 20]
            },
            "result_gem_area": [610, 800, 220, 60],
            "resource_gems": [[450, 696], [520, 696]],
            "empty_signature": {
                "mean_v": 0.3, "mean_sat": 0.1, "colored_fraction": 0.05,
                "mean_r": 0.3, "mean_g": 0.3, "mean_b": 0.3
            },
            "empty_distance": 0.18
        },
        "arduino": {"port": "COM5"},
        "ui": {
            "left": 100.0, "top": 200.0, "width": 900.0,
            "expanded_height": 700.0, "pinned": true
        },
        "spammer": {
            "active": "default",
            "presets": {"default": {"*1": 0.5, "*q": 2.0}},
            "priority": {"default": ["*1"]},
            "combos": {"default": [{"keys": ["*1", "*2"], "gap": 0.8}]},
            "startup": {"default": {"lead_in": 1.5, "step": 0.2}},
            "startup_step": {"default": 0.3}
        },
        "buy_sell": {
            "bag_grid": [10, 20, 500, 400],
            "bag_slot": [10, 20, 40, 40],
            "shop_rows": 10,
            "shop_region": [600, 200, 300, 400],
            "scroll_point": [700, 300],
            "max_button": [800, 500],
            "presets": {"spring": {"scroll": 2, "row": 1, "count": 5}},
            "sell_slots": [0, 1],
            "sell_cap": 16
        },
        "pet": {
            "menu_button": [50, 60],
            "feed_icon": [70, 80],
            "close_button": [90, 100],
            "close_button_png": "iVBORw0KGgo=",
            "boarding_window_may_be_open": true,
            "page_tabs": [[1, 2], [3, 4]],
            "bag_grid": [10, 20, 500, 400],
            "bag_slot": [10, 20, 40, 40],
            "food_slots": [[0, 5], [0, 6]],
            "food_slots_used": 2,
            "food_icon_png": "iVBORw0KGgo=",
            "action_wait_ms": 1200,
            "wait_after_empty_minutes": 5,
            "food_load_mode": "drag",
            "feeder_count_left_fraction": 0.6,
            "feeder_count_min_score": 0.7,
            "reload_on_start": true,
            "return_slot": [0, 7],
            "max_button": [800, 500],
            "slots": [{
                "enabled": true,
                "toggle_label": [1, 2, 3, 4],
                "boarding_pet_slot": [5, 6, 7, 8],
                "feeder_strip": [9, 10, 11, 12],
                "stacks": 5,
                "pet_slot_empty_png": "iVBORw0KGgo=",
                "boarding_running": true
            }],
            "queue": [{
                "label": "pet A",
                "rect": [1, 2, 3, 4],
                "png": "iVBORw0KGgo=",
                "species": "dragon"
            }]
        },
        "tooltip": {
            "offset_x": -20,
            "offset_y": 30,
            "width": 200,
            "height": 120,
            "hover_delay_ms": 1200
        },
        "quest": {
            "active": "daily",
            "presets": {"daily": {
                "initial_wait_seconds": 10.0,
                "loops": 2,
                "steps": [
                    {"action": "click", "value": null, "delay_seconds": 0.25},
                    {"action": "key", "value": "1", "delay_seconds": 0.2}
                ]
            }},
            "active_sequence": "nightly",
            "sequences": {"nightly": {
                "initial_wait_seconds": 12.0,
                "loops": 3,
                "entries": [{"preset": "daily", "times": 2}]
            }},
            "guard_px": 8
        },
        "settings": {
            "window": {"title": "TW_LIVE"},
            "reference_window": {"width": 1926, "height": 1232},
            "game": {
                "variant": "tw",
                "variants": {
                    "tw": {"title": "TW_LIVE", "attributes": "attributes.yaml"},
                    "us": {"title": "US_LIVE", "attributes": "attributes.us.yaml"}
                }
            },
            "arduino": {"vid": 9025, "pid": [32822, 32823], "baud": 115_200},
            "hotkeys": {"start": 123, "quit": 122, "advance_grade": 71, "pause": 20},
            "tuner": {
                "grade_order": ["N", "G", "DG"],
                "target_grade": "DG",
                "max_retries": 999_999,
                "save_captures": false,
                "ocr_retries": 3,
                "spring_mode": "hid",
                "mouse_guard": "stop",
                "guard_px": 8,
                "recenter_max": 20,
                "models": {"detector": "d.onnx", "recognizer": "r.onnx", "classifier": "c.onnx"},
                "timing": {"click_enter_delay": 0.4, "ocr_delay": 0.8},
                "grade_colors": {
                    "yellow": {"rg_gap": 20, "gb_gap": 20, "r_min": 120, "g_min": 120},
                    "blue": {"bg_gap": 25, "br_gap": 25, "b_min": 80},
                    "red": {"rg_gap": 40, "rb_gap": 40, "r_min": 150},
                    "purple": {"rb_gap": 30, "bg_gap": 30, "r_min": 80, "b_min": 80},
                    "white": {"r_min": 180, "g_min": 180, "b_min": 180},
                    "decision": {
                        "total_min": 10, "dg_yellow": 80, "g_blue": 60, "xg_red": 60,
                        "sg_purple": 60, "n_white": 100, "n_yellow_max": 60, "n_blue_max": 60
                    }
                },
                "filter": {
                    "enabled": true,
                    "match_mode": "per_attr",
                    "require_grade": "DG",
                    "rules": [{"name": "atk", "count": 2, "min": 1, "max": 1}],
                    "override_rules": []
                }
            },
            "gem": {
                "grades": ["N", "G", "DG"],
                "start_grade": "N",
                "empty_mode": "advance_grade_clear",
                "empty_streak": 2,
                "colored_gap_min": 40,
                "save_empty_captures": false,
                "move_mode": "arduino"
            },
            "pet": {"items_per_minute": 3, "pet_slot_occupied_above": 0.05},
            "legacy_folded": true
        }
    })
}

#[test]
fn a_full_local_yaml_round_trips_with_v2_field_names() {
    let fixture = full_local_shape();

    let local: LocalOverrides = serde_json::from_value(fixture.clone()).unwrap();

    // Spot-check the deepest fields, so a decode that silently substituted defaults still fails.
    let calibration = local.calibration.as_ref().unwrap();
    assert_eq!(calibration.dpi_scale, Some(1.5));
    assert_eq!(calibration.monitor_dpi, Some(144));
    assert_eq!(calibration.client_size, Some(vec![2865, 1789]));

    let ocr = local.tuner.as_ref().unwrap().ocr.as_ref().unwrap();
    assert_eq!(ocr.region.as_ref().unwrap().width, 300);
    assert_eq!(ocr.grade_area.as_ref().unwrap().x2, 232);
    assert_eq!(
        local.tuner.as_ref().unwrap().spring_point,
        Some(vec![300, 400])
    );

    let gem = local.gem.as_ref().unwrap();
    assert_eq!(gem.grade_positions.as_ref().unwrap()["N"], vec![727, 696]);
    assert_eq!(gem.movements.as_ref().unwrap().slot3_to_dg, vec![19, 20]);
    assert_eq!(gem.empty_signature.as_ref().unwrap().mean_sat, 0.1);
    assert_eq!(gem.empty_distance, Some(0.18));

    let ui = local.ui.as_ref().unwrap();
    assert_eq!(ui.expanded_height, Some(700.0));
    assert!(ui.pinned);

    let spammer = local.spammer.as_ref().unwrap();
    assert_eq!(spammer.presets.as_ref().unwrap()["default"]["*q"], 2.0);
    assert_eq!(spammer.priority.as_ref().unwrap()["default"], vec!["*1"]);
    assert_eq!(spammer.combos.as_ref().unwrap()["default"][0].gap, 0.8);
    assert_eq!(spammer.startup.as_ref().unwrap()["default"].lead_in, 1.5);
    assert_eq!(spammer.startup_step.as_ref().unwrap()["default"], 0.3);

    let pet = local.pet.as_ref().unwrap();
    assert_eq!(pet.food_slots_used, Some(2));
    assert_eq!(pet.slots.as_ref().unwrap()[0].stacks, Some(5));
    assert_eq!(pet.slots.as_ref().unwrap()[0].boarding_running, Some(true));
    assert_eq!(
        pet.queue.as_ref().unwrap()[0].species.as_deref(),
        Some("dragon")
    );

    let quest = local.quest.as_ref().unwrap();
    assert_eq!(
        quest.presets.as_ref().unwrap()["daily"].steps[1]
            .value
            .as_deref(),
        Some("1")
    );
    assert_eq!(
        quest.sequences.as_ref().unwrap()["nightly"].entries[0].times,
        2
    );

    let settings = local.settings.as_ref().unwrap();
    assert_eq!(settings.legacy_folded, Some(true));
    assert_eq!(settings.window.as_ref().unwrap().title, "TW_LIVE");
    assert_eq!(
        settings.game.as_ref().unwrap().variants.as_ref().unwrap()["us"].title,
        "US_LIVE"
    );
    assert_eq!(settings.hotkeys.as_ref().unwrap().advance_grade, 71);
    assert_eq!(
        settings
            .tuner
            .as_ref()
            .unwrap()
            .grade_colors
            .as_ref()
            .unwrap()
            .decision
            .as_ref()
            .unwrap()
            .n_white,
        100
    );
    assert_eq!(
        settings.gem.as_ref().unwrap().empty_mode.as_deref(),
        Some("advance_grade_clear")
    );
    assert_eq!(settings.pet.as_ref().unwrap().items_per_minute, Some(3));

    // The whole model, back out: if a field were dropped by the projection, this is where it shows.
    assert_eq!(serde_json::to_value(&local).unwrap(), fixture);
}

/// "Every guard is 'is it PRESENT', never 'is it non-empty'" (`ConfigLoader.ApplySettings`): an empty
/// block is a partial write, so a field it does not mention must stay absent rather than become a
/// zero.
#[test]
fn a_partial_overlay_leaves_every_unstated_field_absent() {
    let local: LocalOverrides = serde_json::from_value(json!({
        "pet": {"food_slots_used": 3},
        "settings": {"pet": {"pet_slot_occupied_above": 0.09}}
    }))
    .unwrap();

    let pet = local.pet.as_ref().unwrap();
    assert_eq!(pet.food_slots_used, Some(3));
    assert_eq!(pet.food_slots, None);

    let settings = local.settings.as_ref().unwrap();
    assert_eq!(
        settings.pet.as_ref().unwrap().pet_slot_occupied_above,
        Some(0.09)
    );
    assert!(settings.window.is_none());
    assert_eq!(settings.legacy_folded, None);
}

/// `IgnoreUnmatchedProperties` in v2: a key nothing models is ignored on load (and dropped on the
/// next save — that is the documented loss). A load must not fail on one.
#[test]
fn unknown_keys_are_tolerated_at_every_level() {
    let local: LocalOverrides = serde_json::from_value(json!({
        "a_top_level_block_from_a_later_release": {"anything": true},
        "tooltip": {"offset_x": 1, "offset_y": 2, "width": 3, "height": 4, "a_new_key": "x"},
        "quest": {"presets": {"daily": {"loops": 4, "future_field": [1, 2]}}}
    }))
    .unwrap();

    assert_eq!(local.tooltip.as_ref().unwrap().width, 3);
    assert_eq!(
        local.quest.as_ref().unwrap().presets.as_ref().unwrap()["daily"].loops,
        4
    );
}

/// A hand-edited `run:` with no body is a null dictionary VALUE in v2, and `TryGetValue` then reports
/// the name as present. v2 repairs it to an empty body (`QuestConfig.NormaliseNullBodies`); a `preset:`
/// with no value is the same defect one level down (`QuestConfigTests`). The model repairs both at
/// the decode, so no reader has to guard again.
#[test]
fn value_less_quest_bodies_load_as_empty_bodies_not_nulls() {
    let quest: QuestConfig = serde_json::from_value(json!({
        "active": "run",
        "presets": {"run": null},
        "active_sequence": "seq",
        "sequences": {"seq": null}
    }))
    .unwrap();

    assert_eq!(quest.presets.len(), 1);
    let run = &quest.presets["run"];
    assert_eq!(run.steps, [] as [seal_domain::config::QuestStep; 0]);
    assert_eq!(run.loops, 1);

    assert_eq!(quest.sequences.len(), 1);
    assert_eq!(
        quest.sequences["seq"].entries,
        [] as [seal_domain::config::QuestSequenceEntry; 0]
    );
    // The key is KEPT rather than dropped: dropping would rename the player's flow.
    assert!(quest.presets.contains_key("run"));
}

/// A bare `entries:` list and a bare `-` list item are both nulls `YamlDotNet` produces from a
/// hand-edited file. The list is "no entries"; an entry is reported as a blank name by the Save gate
/// rather than crashing the load (`QuestConfigTests` pins both).
#[test]
fn value_less_sequence_lists_and_entries_load_as_blanks() {
    let quest: QuestConfig = serde_json::from_value(json!({
        "sequences": {
            "null_list": {"entries": null},
            "null_entry": {"entries": [null, {"preset": "a"}]}
        },
        "presets": {"a": {"steps": null}}
    }))
    .unwrap();

    assert_eq!(
        quest.sequences["null_list"].entries,
        [] as [seal_domain::config::QuestSequenceEntry; 0]
    );
    assert_eq!(quest.sequences["null_entry"].entries.len(), 2);
    assert_eq!(quest.sequences["null_entry"].entries[0].preset, "");
    assert_eq!(quest.sequences["null_entry"].entries[1].preset, "a");
    assert_eq!(
        quest.presets["a"].steps,
        [] as [seal_domain::config::QuestStep; 0]
    );
}

/// v2's initializers, ported: a config file that omits a key gets the value the C# property starts
/// with (`ConfigLoaderTests.TheSettingsMirrorCarriesExactlyTheShippedTemplatesFields` guards the
/// shipped template; these are the code-side defaults).
#[test]
fn omitted_fields_keep_the_initialisers_v2_gives_them() {
    let preset: QuestPreset = serde_json::from_value(json!({})).unwrap();
    assert_eq!(preset.initial_wait_seconds, 10.0);
    assert_eq!(preset.loops, 1);
    assert_eq!(preset.steps, [] as [seal_domain::config::QuestStep; 0]);

    let step: QuestStep = serde_json::from_value(json!({})).unwrap();
    assert_eq!(step.action, "click");
    assert_eq!(step.delay_seconds, 0.25);
    assert_eq!(step.value, None);

    let sequence: QuestSequence = serde_json::from_value(json!({})).unwrap();
    assert_eq!(sequence.initial_wait_seconds, 10.0);
    assert_eq!(sequence.loops, 1);

    let combo: SpammerCombo = serde_json::from_value(json!({"keys": ["1", "2"]})).unwrap();
    assert_eq!(combo.gap, 0.8);

    let startup: SpammerStartup = serde_json::from_value(json!({})).unwrap();
    assert_eq!(startup.lead_in, 0.0);
    assert_eq!(startup.step, 0.0);

    let buy: BuyPreset = serde_json::from_value(json!({})).unwrap();
    assert_eq!(buy.count, 1);
    assert_eq!(buy.scroll, 0);
    assert_eq!(buy.row, 0);

    let tooltip: TooltipConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(tooltip.hover_delay_ms, 1200);

    let hotkeys: HotkeysConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(hotkeys.start, 0);
    assert_eq!(hotkeys.pause, 0);

    let quest: QuestConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(quest.active, "default");
    assert_eq!(quest.active_sequence, "");
    assert_eq!(quest.guard_px, 8);

    let settings: LocalSettings = serde_json::from_value(json!({})).unwrap();
    assert_eq!(settings.legacy_folded, None);
}

/// The two shipped clients are the `GameConfig` initializer's, and the lookup is case-insensitive
/// because v2's dictionary is `StringComparer.OrdinalIgnoreCase` — a hand-typed `TW` must work.
#[test]
fn the_game_variant_table_is_the_shipped_one_and_lookup_ignores_case() {
    let mut game = GameConfig::default();
    // Unset means "not configured": `window.title` stands, which is what every pre-variant file says
    // (the C# initialiser is an empty string, not `tw` — the shipped defaults.yaml is what sets tw).
    assert!(game.active().is_none());
    assert_eq!(game.variants["tw"].title, "TW_LIVE");
    assert_eq!(game.variants["us"].attributes, "attributes.us.yaml");

    game.variant = "tw".to_owned();
    assert_eq!(game.active().unwrap().title, "TW_LIVE");

    game.variant = "US".to_owned();
    assert_eq!(game.active().unwrap().title, "US_LIVE");
    assert_eq!(
        game.active().unwrap().attributes,
        "attributes.us.yaml".to_owned()
    );

    game.variant = "  us  ".to_owned();
    assert_eq!(game.active().unwrap().title, "US_LIVE");

    // Unset and unknown both mean "the configured window.title stands" — never blank the target.
    game.variant = String::new();
    assert!(game.active().is_none());
    game.variant = "nope".to_owned();
    assert!(game.active().is_none());
}

/// `TooltipConfig.IsSet`: an offset means nothing without a size, and a size without an offset reads
/// the wrong place (`ConfigLoader.ApplyOverrides` only applies a set tooltip block).
#[test]
fn a_tooltip_is_set_only_when_it_has_a_size() {
    let mut tooltip = TooltipConfig::default();
    assert!(!tooltip.is_set());

    tooltip.width = 200;
    tooltip.height = 120;
    assert!(tooltip.is_set());
    assert_eq!(tooltip.offset_x, 0);
}

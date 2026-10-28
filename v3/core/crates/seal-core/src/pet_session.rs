//! Pet session increment 1: the read-only preflight and the single-row boarding tick.
//!
//! v2's Pet tab had two checklists: Calibrate Pet's machine marks, and this tab's own "Setup so far
//! — this tab" plus the Ready-to-run verdict (`MainWindow.xaml.cs:8009-8039,8308-8354`). Preflight
//! is the data half of that surface. It opens no window, no board and no OCR; it only reads the
//! merged config. `pet.set_boarding` is the one write: it goes through the A1 field-precise writer
//! (`ConfigStore::persist_pet_run_state`), so a tick cannot race the feeder's own mid-run updates or
//! replace the rest of the pet block.

use seal_domain::tools::pet::cycle_minutes;
use serde_json::{Value, json};

use crate::pet::setup;

/// The read-only preflight answer. `ready` is the overall run readiness: v2's Ready-to-run lines are
/// empty AND the run's calibration marks are present. The two lists are facts, never error codes; a
/// malformed/absent block still answers cleanly with `ok:false` items.
pub fn preflight(config: &Value) -> Value {
    let setup = setup(config);
    let food_used = config
        .pointer("/pet/food_slots_used")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let round: i32 = setup.slots.iter().map(|row| row.stacks.max(1)).sum();

    let mut calibration = Vec::new();
    let mut mark = |ok: bool, label: &str, detail: Option<String>| {
        calibration.push(item(ok, label, detail));
    };
    mark(
        point_ok(&setup.menu_button),
        "Pet menu point",
        Some(if point_ok(&setup.menu_button) {
            "marked".to_owned()
        } else {
            "not marked — save it on the Calibrate tab".to_owned()
        }),
    );
    mark(
        point_ok(&setup.feed_icon),
        "Pet feed icon point",
        Some(if point_ok(&setup.feed_icon) {
            "marked".to_owned()
        } else {
            "not marked — save it on the Calibrate tab".to_owned()
        }),
    );
    mark(
        point_ok(&setup.close_button),
        "Boarding X point",
        Some(if point_ok(&setup.close_button) {
            "marked".to_owned()
        } else {
            "not marked — save it on the Calibrate tab".to_owned()
        }),
    );
    mark(
        rect_ok(&setup.bag_grid),
        "Boarding bag grid",
        Some(if rect_ok(&setup.bag_grid) {
            "marked".to_owned()
        } else {
            "not marked — save it on the Calibrate tab".to_owned()
        }),
    );
    mark(
        rect_ok(&setup.bag_slot),
        "Boarding bag slot",
        Some(if rect_ok(&setup.bag_slot) {
            "marked".to_owned()
        } else {
            "not marked — save it on the Calibrate tab".to_owned()
        }),
    );
    mark(
        !setup.page_tabs.is_empty(),
        "Bag page tabs",
        Some(if setup.page_tabs.is_empty() {
            "not marked — save the page tab points on the Calibrate tab".to_owned()
        } else {
            format!("{} page tab(s) marked", setup.page_tabs.len())
        }),
    );
    let max = setup.max_button.is_some() || setup.buy_sell_max_button.is_some();
    mark(
        max,
        "MAX button",
        Some(if max {
            "marked".to_owned()
        } else {
            "not marked — save the MAX button on the Calibrate tab".to_owned()
        }),
    );
    for (index, row) in setup.slots.iter().enumerate() {
        if !row.enabled {
            continue;
        }
        mark(
            rect_ok(&row.toggle_label),
            &format!("Row {} start/end button", index + 1),
            Some(row_mark_detail(row.toggle_label.as_deref())),
        );
        let slot = row_rect(config, index, "boarding_pet_slot");
        mark(
            rect_ok(&slot),
            &format!("Row {} boarding slot", index + 1),
            Some(row_mark_detail(slot.as_deref())),
        );
        mark(
            rect_ok(&row.feeder_strip),
            &format!("Row {} food strip", index + 1),
            Some(row_mark_detail(row.feeder_strip.as_deref())),
        );
        let empty = config
            .pointer(&format!("/pet/slots/{index}/pet_slot_empty_png"))
            .and_then(Value::as_str)
            .is_some_and(|png| !png.trim().is_empty());
        mark(
            empty,
            &format!("Row {} empty-slot reference", index + 1),
            Some(if empty {
                "captured".to_owned()
            } else {
                "no reference stored — capture the empty pet slot".to_owned()
            }),
        );
    }
    let calibration_ok = calibration
        .iter()
        .all(|item| item["ok"] == Value::Bool(true));

    let mut session = Vec::new();
    let return_ok = point_ok(&setup.return_slot);
    session.push(item(
        return_ok,
        "return slot",
        Some(match &setup.return_slot {
            Some(slot) if slot.len() == 2 => format!("page {}, cell {}", slot[0] + 1, slot[1]),
            _ => "not marked".to_owned(),
        }),
    ));
    session.push(item(
        !setup.food_slots.is_empty(),
        "food cells",
        Some(format!(
            "{} marked, {food_used} used{}",
            setup.food_slots.len(),
            if round > 0 {
                format!(" (one round loads {round})")
            } else {
                String::new()
            }
        )),
    ));
    session.push(item(
        setup.queue_len > 0,
        "queued pet icons",
        Some(if setup.queue_len == 0 {
            "0 (the return slot is used instead)".to_owned()
        } else {
            setup.queue_len.to_string()
        }),
    ));
    session.push(item(
        !setup.food_load_mode.trim().is_empty(),
        "food load mode",
        Some(setup.food_load_mode.clone()),
    ));
    for (index, _row) in setup.slots.iter().enumerate() {
        let running = row_boarding_running(config, index);
        session.push(item(
            running,
            &format!("row {} boarding", index + 1),
            Some(if running {
                "yes".to_owned()
            } else {
                "no — tick it if it is feeding".to_owned()
            }),
        ));
    }
    session.push(item(
        true,
        "board firmware",
        Some("not checked by preflight — normal".to_owned()),
    ));

    // The exact v2 Ready-to-run conditions (`RefreshPetReady:8308-8354`).
    let mut lines = Vec::new();
    if setup.slots.is_empty() {
        lines.push(
            "no breeding rows configured — there is nothing for the feeder to run".to_owned(),
        );
    }
    if !return_ok && setup.queue_len == 0 {
        lines.push(
            "neither the return slot nor a pet icon is marked — nothing says where the pet is"
                .to_owned(),
        );
    }
    if setup.food_slots.is_empty() {
        lines.push("no food cells are marked".to_owned());
    }
    if round > 0 && !setup.food_slots.is_empty() && (setup.food_slots.len() as i32) < round {
        lines.push(format!(
            "{} food cell(s) marked but one round of all {} row(s) loads {round}",
            setup.food_slots.len(),
            setup.slots.len()
        ));
    }
    if setup.queue_len == 0 {
        lines.push(
            "no pet icon queued — the tool will use the return slot instead of looking for the pet \
             by icon"
                .to_owned(),
        );
    }
    if !calibration_ok {
        lines.push(
            "calibration is incomplete — the run will refuse until the missing marks are saved"
                .to_owned(),
        );
    }
    let ready = lines.is_empty();
    if ready {
        let cycles: Vec<String> = setup
            .slots
            .iter()
            .map(|row| {
                format!(
                    "{}",
                    cycle_minutes(
                        row.stacks,
                        config
                            .pointer("/pet/items_per_minute")
                            .and_then(Value::as_i64)
                            .unwrap_or(3) as i32,
                        config
                            .pointer("/pet/wait_after_empty_minutes")
                            .and_then(Value::as_i64)
                            .unwrap_or(5) as i32,
                    )
                )
            })
            .collect();
        lines.push(format!(
            "This tab is ready. {} row(s), reloading every {} min.",
            setup.slots.len(),
            cycles.join(" / ")
        ));
    }

    json!({
        "ready": ready,
        "calibration": calibration,
        "session": session,
        "lines": lines,
    })
}

/// The boarding-tick arguments, 1-BASED row per the protocol. The caller checks the row exists.
pub fn boarding_arguments(args: &Value) -> Result<(usize, bool), (String, String)> {
    let Some(row) = args.get("row").and_then(Value::as_i64) else {
        return Err((
            "pet.set_boarding needs an integer \"row\" (1-based)".to_owned(),
            "row".to_owned(),
        ));
    };
    let Ok(row) = usize::try_from(row) else {
        return Err((
            "pet.set_boarding's \"row\" must be at least 1".to_owned(),
            "row".to_owned(),
        ));
    };
    if row == 0 {
        return Err((
            "pet.set_boarding's \"row\" must be at least 1".to_owned(),
            "row".to_owned(),
        ));
    }
    let Some(running) = args.get("running").and_then(Value::as_bool) else {
        return Err((
            "pet.set_boarding needs a boolean \"running\"".to_owned(),
            "running".to_owned(),
        ));
    };
    Ok((row - 1, running))
}

fn item(ok: bool, label: &str, detail: Option<String>) -> Value {
    match detail {
        Some(detail) => json!({"ok": ok, "label": label, "detail": detail}),
        None => json!({"ok": ok, "label": label}),
    }
}

fn point_ok(point: &Option<Vec<i32>>) -> bool {
    point.as_ref().is_some_and(|point| point.len() == 2)
}

fn rect_ok(rect: &Option<Vec<i32>>) -> bool {
    rect.as_ref()
        .is_some_and(|rect| rect.len() == 4 && rect[2] > 0 && rect[3] > 0)
}

fn row_rect(config: &Value, index: usize, field: &str) -> Option<Vec<i32>> {
    config
        .pointer(&format!("/pet/slots/{index}/{field}"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_i64)
                .filter_map(|value| i32::try_from(value).ok())
                .collect()
        })
}

fn row_boarding_running(config: &Value, index: usize) -> bool {
    config
        .pointer(&format!("/pet/slots/{index}/boarding_running"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn row_mark_detail(rect: Option<&[i32]>) -> String {
    match rect {
        Some(rect) if rect.len() == 4 => {
            format!("[{}, {}, {}, {}]", rect[0], rect[1], rect[2], rect[3])
        }
        Some(_) => "malformed mark".to_owned(),
        None => "not marked — save it on the Calibrate tab".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config(rows: usize) -> Value {
        let slots: Vec<Value> = (0..rows)
            .map(|_| {
                json!({
                    "enabled": true,
                    "toggle_label": [10, 20, 100, 30],
                    "boarding_pet_slot": [10, 60, 100, 80],
                    "feeder_strip": [110, 20, 200, 80],
                    "stacks": 2,
                    "boarding_running": true,
                    "pet_slot_empty_png": "iVBORw0KGgo=",
                })
            })
            .collect();
        json!({
            "buy_sell": {"max_button": [900, 500]},
            "pet": {
                "menu_button": [1, 1], "feed_icon": [2, 2], "close_button": [3, 3],
                "bag_grid": [0, 0, 400, 400], "bag_slot": [0, 0, 40, 40],
                "page_tabs": [[100, 100]], "slots": slots,
                "food_slots": [[0, 0], [0, 1], [0, 2], [0, 3]],
                "food_slots_used": 1, "food_load_mode": "right_click",
                "return_slot": [0, 5], "queue": [{"label": "p", "png": "x"}],
                "items_per_minute": 3, "wait_after_empty_minutes": 5,
            }
        })
    }

    #[test]
    fn a_complete_setup_is_ready_and_names_the_cycle_minutes() {
        let answer = preflight(&valid_config(1));
        assert_eq!(answer["ready"], true, "{answer}");
        assert_eq!(
            answer["lines"][0],
            "This tab is ready. 1 row(s), reloading every 205 min."
        );
        assert!(
            answer["calibration"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["ok"] == true)
        );
        assert!(
            answer["session"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["label"] == "food cells")
        );
    }

    #[test]
    fn zero_rows_no_food_and_no_queue_are_v2s_ready_lines() {
        let mut config = valid_config(0);
        config["pet"]["food_slots"] = json!([]);
        config["pet"]["queue"] = json!([]);
        let answer = preflight(&config);
        assert_eq!(answer["ready"], false);
        let lines = answer["lines"].to_string();
        assert!(lines.contains("no breeding rows configured"));
        assert!(lines.contains("no food cells are marked"));
        assert!(lines.contains("no pet icon queued"));
    }

    #[test]
    fn no_return_and_no_queue_is_refused_even_when_food_exists() {
        let mut config = valid_config(1);
        config["pet"]["return_slot"] = Value::Null;
        config["pet"]["queue"] = json!([]);
        let answer = preflight(&config);
        assert_eq!(answer["ready"], false);
        assert!(
            answer["lines"]
                .to_string()
                .contains("neither the return slot nor a pet icon")
        );
    }

    #[test]
    fn too_few_food_cells_for_one_round_is_refused_with_both_numbers() {
        let mut config = valid_config(2);
        config["pet"]["food_slots"] = json!([[0, 0], [0, 1]]);
        let answer = preflight(&config);
        assert_eq!(answer["ready"], false);
        assert!(
            answer["lines"][0]
                .as_str()
                .unwrap()
                .contains("2 food cell(s)")
        );
        assert!(answer["lines"][0].as_str().unwrap().contains("loads 4"));
    }

    #[test]
    fn a_missing_mark_is_a_calibration_fact_and_makes_ready_false() {
        let mut config = valid_config(1);
        config["pet"]["bag_grid"] = Value::Null;
        let answer = preflight(&config);
        assert_eq!(answer["ready"], false);
        assert!(
            answer["calibration"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| { item["label"] == "Boarding bag grid" && item["ok"] == false })
        );
    }

    #[test]
    fn boarding_arguments_are_one_based_and_named() {
        assert_eq!(
            boarding_arguments(&json!({"row": 1, "running": true})),
            Ok((0, true))
        );
        assert_eq!(
            boarding_arguments(&json!({"row": 2, "running": false})),
            Ok((1, false))
        );
        assert_eq!(
            boarding_arguments(&json!({"row": 0, "running": true}))
                .unwrap_err()
                .1,
            "row"
        );
        assert_eq!(
            boarding_arguments(&json!({"row": 1})).unwrap_err().1,
            "running"
        );
        assert_eq!(
            boarding_arguments(&json!({"row": "1", "running": true}))
                .unwrap_err()
                .1,
            "row"
        );
    }
}

//! Pet session increment 1: the read-only preflight and the single-row boarding tick.
//!
//! v2's Pet tab had two checklists: Calibrate Pet's machine marks, and this tab's own "Setup so far
//! — this tab" plus the Ready-to-run verdict (`MainWindow.xaml.cs:8009-8039,8308-8354`). Preflight
//! is the data half of that surface. It opens no window, no board and no OCR; it only reads the
//! merged config. `pet.set_boarding` is the one write: it goes through the A1 field-precise writer
//! (`ConfigStore::persist_pet_run_state`), so a tick cannot race the feeder's own mid-run updates or
//! replace the rest of the pet block.

use seal_domain::geometry::Point;
use seal_domain::tools::pet::{FeederLayout, cycle_minutes};
use seal_proto::{Error, Request, Response};
use serde_json::{Value, json};

use crate::dispatch::CoreState;
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
    // The run only needs tabs for the pages its marks point at (`decision.rs:839-869`), and each
    // such tab must carry a 2-point mark; a non-empty `page_tabs` vector is not enough.
    let mut used_pages: Vec<usize> = Vec::new();
    if let Some(return_slot) = &setup.return_slot
        && return_slot.len() == 2
        && return_slot[0] >= 0
    {
        used_pages.push(return_slot[0] as usize);
    }
    for cell in &setup.food_slots {
        if cell.len() == 2 && cell[0] >= 0 {
            used_pages.push(cell[0] as usize);
        }
    }
    used_pages.sort_unstable();
    used_pages.dedup();
    let missing_pages: Vec<usize> = used_pages
        .iter()
        .copied()
        .filter(|page| {
            setup
                .page_tabs
                .get(*page)
                .is_none_or(|mark| mark.len() != 2)
        })
        .collect();
    let pages_ok = missing_pages.is_empty();
    let page_detail = if used_pages.is_empty() {
        "no pages are in use by the current marks".to_owned()
    } else if pages_ok {
        format!("{}/{} marked", used_pages.len(), used_pages.len())
    } else {
        let used = used_pages
            .iter()
            .map(|page| (page + 1).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let missing = missing_pages
            .iter()
            .map(|page| (page + 1).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "used pages: {used} — {} {missing} missing a mark",
            if missing_pages.len() == 1 {
                "page"
            } else {
                "pages"
            }
        )
    };
    mark(pages_ok, "Bag page tabs", Some(page_detail));
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
        let food_strip_ok = if setup.food_load_mode.eq_ignore_ascii_case("drag") {
            // The run's own derivation, not a bare rect check: a malformed strip, a zero stack count
            // and a strip that yields no boxes must all agree with Start.
            FeederLayout::slot_boxes(row.feeder_strip.as_deref(), row.stacks).is_some()
        } else {
            true
        };
        mark(
            food_strip_ok,
            &format!("Row {} food strip", index + 1),
            Some(if setup.food_load_mode.eq_ignore_ascii_case("drag") {
                if food_strip_ok {
                    "drawn".to_owned()
                } else {
                    "not drawn — drag mode has to name the box it drops into".to_owned()
                }
            } else {
                "not needed in right-click mode".to_owned()
            }),
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
    // Only the marks Start actually refuses on count towards `ready`. The boarding-slot and
    // empty-slot-reference rows are useful calibration facts but not start gates (`ready` in
    // `decision.rs`); keeping them out of this predicate is what makes preflight agree with Start
    // in BOTH directions, instead of claiming a refusal for an optional crop.
    let drag = setup.food_load_mode.eq_ignore_ascii_case("drag");
    let row_toggles_ok = setup
        .slots
        .iter()
        .filter(|row| row.enabled)
        .all(|row| rect_ok(&row.toggle_label));
    let row_strips_ok =
        !drag
            || setup.slots.iter().filter(|row| row.enabled).all(|row| {
                FeederLayout::slot_boxes(row.feeder_strip.as_deref(), row.stacks).is_some()
            });
    let calibration_ok = point_ok(&setup.menu_button)
        && point_ok(&setup.feed_icon)
        && point_ok(&setup.close_button)
        && rect_ok(&setup.bag_grid)
        && rect_ok(&setup.bag_slot)
        && pages_ok
        && (setup.max_button.is_some() || setup.buy_sell_max_button.is_some())
        && row_toggles_ok
        && row_strips_ok;

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
    if !setup.slots.is_empty() && !setup.slots.iter().any(|row| row.enabled) {
        lines.push("every row is unticked — nothing to feed".to_owned());
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
        // v2 formats EACH row (`"205 min"`) and joins those strings, so the unit appears once per
        // row. The numbers use the run's guarded `cycle_minutes` rather than v2's raw
        // LoadMinutes+WaitAfterEmpty on purpose: the preview should show what the run will use (the
        // max(5, …) guard and the 60-minute fallback are part of that truth).
        let cycles: Vec<String> = setup
            .slots
            .iter()
            .map(|row| {
                format!(
                    "{} min",
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
            "This tab is ready. {} row(s), reloading every {}.",
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

    #[test]
    fn the_ready_line_puts_min_after_each_rows_cycle() {
        let answer = preflight(&valid_config(2));
        assert_eq!(answer["ready"], true, "{answer}");
        assert_eq!(
            answer["lines"][0],
            "This tab is ready. 2 row(s), reloading every 205 min / 205 min."
        );
    }

    #[test]
    fn every_used_page_needs_a_two_point_tab_mark_but_unused_pages_do_not() {
        let mut config = valid_config(1);
        config["pet"]["food_slots"] = json!([[1, 0]]);
        config["pet"]["return_slot"] = Value::Null;
        let missing = preflight(&config);
        assert_eq!(missing["ready"], false);
        let page_item = missing["calibration"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["label"] == "Bag page tabs")
            .expect("the page item");
        assert_eq!(page_item["ok"], false);
        assert!(
            page_item["detail"]
                .as_str()
                .unwrap()
                .contains("used pages: 2 — page 2 missing a mark"),
            "{page_item}"
        );

        // A page nobody's marks point at is irrelevant to Start and must not fail preflight.
        let mut config = valid_config(1);
        config["pet"]["page_tabs"] = json!([[100, 100]]);
        config["pet"]["food_slots"] = json!([[0, 0], [0, 1], [0, 2], [0, 3]]);
        assert_eq!(preflight(&config)["ready"], true);
    }

    #[test]
    fn food_strip_is_a_gate_only_in_drag_mode_and_uses_the_run_derivation() {
        let mut drag = valid_config(1);
        drag["pet"]["food_load_mode"] = json!("drag");
        drag["pet"]["slots"][0]["feeder_strip"] = Value::Null;
        let answer = preflight(&drag);
        assert_eq!(answer["ready"], false);
        let strip = answer["calibration"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["label"] == "Row 1 food strip")
            .expect("the strip item");
        assert_eq!(strip["ok"], false);
        assert!(strip["detail"].as_str().unwrap().contains("drag mode"));

        let mut right_click = valid_config(1);
        right_click["pet"]["slots"][0]["feeder_strip"] = Value::Null;
        let answer = preflight(&right_click);
        assert_eq!(answer["ready"], true, "{answer}");
        let strip = answer["calibration"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["label"] == "Row 1 food strip")
            .expect("the strip item");
        assert_eq!(strip["ok"], true);
        assert_eq!(strip["detail"], "not needed in right-click mode");
    }

    #[test]
    fn every_row_unticked_is_a_readiness_line() {
        let mut config = valid_config(1);
        config["pet"]["slots"][0]["enabled"] = json!(false);
        let answer = preflight(&config);
        assert_eq!(answer["ready"], false);
        assert!(
            answer["lines"]
                .to_string()
                .contains("every row is unticked — nothing to feed")
        );
    }

    #[test]
    fn optional_empty_slot_and_boarding_slot_crops_are_facts_not_start_gates() {
        let mut config = valid_config(1);
        config["pet"]["slots"][0]["boarding_pet_slot"] = Value::Null;
        config["pet"]["slots"][0]["pet_slot_empty_png"] = Value::Null;
        let answer = preflight(&config);
        // The run's `ready` neither reads nor refuses these crops; showing them as failed checks
        // while `ready` stays true is the parity rule.
        assert_eq!(answer["ready"], true, "{answer}");
        assert!(
            answer["calibration"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| { item["label"] == "Row 1 boarding slot" && item["ok"] == false })
        );
    }

    #[test]
    fn malformed_or_absent_pet_config_answers_facts_without_panicking() {
        for config in [json!({}), json!({"pet": null}), json!({"pet": []})] {
            let answer = preflight(&config);
            assert_eq!(answer["ready"], false);
            assert!(answer["calibration"].is_array());
            assert!(answer["session"].is_array());
            assert!(answer["lines"].is_array());
        }
    }

    #[test]
    fn the_board_firmware_item_is_informational() {
        let answer = preflight(&valid_config(1));
        let board = answer["session"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["label"] == "board firmware")
            .expect("the board item");
        assert_eq!(board["ok"], true);
        assert_eq!(board["detail"], "not checked by preflight — normal");
    }
}

// ── G22: put a bag page up before a scan ────────────────────────────────────────────────────────

/// `pet.select_page {"page":N}`: click the calibrated ITEM tab for 1-based page `N`. The shell
/// hides itself around the call (the same contract as a capture/scan); this only performs the one
/// inert closed-loop click under the tool input lease, so the scan commands can rely on the page
/// being up instead of documenting a caveat.
///
/// The tab is ONE marked point (`pet.page_tabs[N-1]`); v2's tabs are ABSOLUTE ("clicking one lands
/// on that page whatever page you were on"), which is why there is one mark per page and no
/// next/previous count to lose.
pub fn select_page(state: &CoreState, request: &Request) -> Response {
    let Some(page) = request.args.get("page").and_then(Value::as_i64) else {
        return Response::err(
            &request.id,
            Error::new(
                "bad_request",
                "pet.select_page needs a 1-based page number",
                json!({"field": "page"}),
            ),
        );
    };
    if page < 1 {
        return Response::err(
            &request.id,
            Error::new(
                "bad_request",
                "pet.select_page's page must be at least 1",
                json!({"field": "page"}),
            ),
        );
    }
    let tabs = state
        .config
        .pointer("/pet/page_tabs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let index = usize::try_from(page - 1).unwrap_or(usize::MAX);
    let Some(tab) = tabs.get(index).and_then(Value::as_array) else {
        return Response::err(
            &request.id,
            Error::new(
                "bad_request",
                &format!("bag page {page} has no calibrated tab — mark it on Calibrate Pet first"),
                json!({"field": "page", "pages": tabs.len()}),
            ),
        );
    };
    let (Some(x), Some(y)) = (
        tab.first().and_then(Value::as_i64),
        tab.get(1).and_then(Value::as_i64),
    ) else {
        return Response::err(
            &request.id,
            Error::new(
                "bad_request",
                &format!("bag page {page}'s tab mark is malformed — re-mark it on Calibrate Pet"),
                json!({"field": "page"}),
            ),
        );
    };
    let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) else {
        return Response::err(
            &request.id,
            Error::new(
                "bad_request",
                &format!("bag page {page}'s tab point is outside the i32 range"),
                json!({"field": "page"}),
            ),
        );
    };
    if let Some(response) =
        crate::tooltip_session::check_input_lease(state, &request.id, request.cmd.as_str())
    {
        return response;
    }
    match crate::pointer::system_perform(&state.config, Point::new(x, y), true) {
        Ok(()) => Response::ok(
            &request.id,
            json!({
                "page": page,
                "point": [x, y],
                "line": format!("clicked the ITEM{page} tab at ({x}, {y})"),
            }),
        ),
        Err(crate::pointer::PointerFailure::Window(error)) => {
            crate::dispatch::win_error_response(&request.id, error)
        }
        Err(crate::pointer::PointerFailure::Board(error)) => {
            crate::dispatch::board_error_response(&request.id, error)
        }
        Err(crate::pointer::PointerFailure::Effects(line)) => Response::err(
            &request.id,
            Error::new("board_unavailable", &line, json!({})),
        ),
    }
}

#[cfg(test)]
mod page_tests {
    use super::*;

    #[test]
    fn a_page_with_no_tab_mark_is_refused_before_any_click() {
        let mut state = crate::dispatch::CoreState::new(std::env::temp_dir());
        state.config = json!({"pet": {"page_tabs": [[10, 20]]}});
        let response = select_page(
            &state,
            &Request::new("1", "pet.select_page", json!({"page": 2})),
        );
        assert!(!response.ok);
        let error = response.error.expect("a refusal");
        assert_eq!(error.code, "bad_request");
        assert_eq!(error.detail["field"], "page");
        assert!(error.message.contains("page 2"), "{}", error.message);
    }
}

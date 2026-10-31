//! Buy/Sell calibration/session helpers (G19/G05): the pure derived preview, the read-only
//! pre-flight checklist, and v2's Test scroll.
//!
//! - `calibration.buysell.preview` never writes: it answers v2's overlay numbers (64 bag centres,
//!   the derived row centres, the pitch and any disagreement/problem) so the shell draws the CORE's
//!   geometry instead of re-deriving it.
//! - `buy_sell.preflight` is the Buy/Sell tab's "Setup so far" checklist, modelled on the Pet tab's
//!   preflight: facts with ok/label/detail plus a ready verdict, never a code error.
//! - `calibration.shop.test_scroll` is the one live action: click the inert focus point, then send
//!   the firmware's wheel command. It refuses more than the firmware's 400-notch guard, and it takes
//!   the same input lease the tools do.

use seal_domain::geometry::Point;
use seal_domain::tools::shop::{BagGrid, ShopGeometry};
use seal_proto::{Error, Request, Response};
use seal_tools::runner::Effects;
use serde_json::{Value, json};

use crate::board::{self, Settings};
use crate::dispatch::{CoreState, board_error_response, win_error_response};
use crate::effects::system_effects;
use crate::pointer::game_display;
use crate::tooltip_session::check_input_lease;

fn bad(id: &str, message: impl Into<String>, field: &str) -> Response {
    Response::err(
        id,
        Error::new("bad_request", &message.into(), json!({"field": field})),
    )
}

fn ints(value: Option<&Value>) -> Option<Vec<i32>> {
    value.and_then(|value| {
        value
            .as_array()?
            .iter()
            .map(|item| i32::try_from(item.as_i64()?).ok())
            .collect()
    })
}

fn points(value: &[Point]) -> Vec<Value> {
    value
        .iter()
        .map(|point| json!([point.x, point.y]))
        .collect()
}

/// `calibration.buysell.preview {"draft":{...}}`: pure derived geometry.
pub fn preview(_state: &CoreState, request: &Request) -> Response {
    let draft = request.args.get("draft").cloned().unwrap_or_default();
    let bag_grid = ints(draft.get("bag_grid"));
    let bag_slot = ints(draft.get("bag_slot"));
    let shop_region = ints(draft.get("shop_region"));
    let rows = draft
        .get("shop_rows")
        .and_then(Value::as_i64)
        .and_then(|rows| i32::try_from(rows).ok())
        .unwrap_or(ShopGeometry::DEFAULT_ROWS);
    let centres = BagGrid::centres(bag_grid.as_deref());
    let row_centres = ShopGeometry::row_centres(shop_region.as_deref(), rows);
    let pitch = bag_grid
        .as_deref()
        .and_then(BagGrid::pitch)
        .map(|(x, y)| json!({"w": x, "h": y}));
    let row_pitch = ShopGeometry::row_pitch(shop_region.as_deref(), rows);
    let problem = BagGrid::disagreement(bag_grid.as_deref(), bag_slot.as_deref())
        .or_else(|| ShopGeometry::problem(shop_region.as_deref(), rows).map(str::to_owned));
    Response::ok(
        &request.id,
        json!({
            "bag_centres": points(&centres),
            "row_centres": points(&row_centres),
            "pitch": pitch,
            "row_pitch": row_pitch,
            "problem": problem,
        }),
    )
}

fn item(ok: bool, label: &str, detail: &str) -> Value {
    json!({"ok": ok, "label": label, "detail": detail})
}

fn rect_ok(value: Option<&Vec<i32>>) -> bool {
    value.is_some_and(|rect| rect.len() == 4 && rect[2] > 0 && rect[3] > 0)
}

fn point_ok(value: Option<&Vec<i32>>) -> bool {
    value.is_some_and(|point| point.len() == 2)
}

/// `buy_sell.preflight {}`: the tab's checklist plus readiness. Read-only.
pub fn preflight(config: &Value) -> Value {
    let bag_grid = ints(config.pointer("/buy_sell/bag_grid"));
    let bag_slot = ints(config.pointer("/buy_sell/bag_slot"));
    let shop_region = ints(config.pointer("/buy_sell/shop_region"));
    let scroll_point = ints(config.pointer("/buy_sell/scroll_point"));
    let max_button = ints(config.pointer("/buy_sell/max_button"));
    let shop_rows = config
        .pointer("/buy_sell/shop_rows")
        .and_then(Value::as_i64)
        .and_then(|rows| i32::try_from(rows).ok())
        .unwrap_or(ShopGeometry::DEFAULT_ROWS);
    let sell_cap = config
        .pointer("/buy_sell/sell_cap")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let presets = config
        .pointer("/buy_sell/presets")
        .and_then(Value::as_object)
        .map_or(0, serde_json::Map::len);
    let mark = |ok: bool, label: &str| {
        item(
            ok,
            label,
            if ok {
                "marked"
            } else {
                "not marked - save it on the Calibrate tab"
            },
        )
    };
    let mut calibration = vec![
        mark(rect_ok(bag_grid.as_ref()), "Bag grid"),
        mark(rect_ok(bag_slot.as_ref()), "Bag slot"),
        mark(rect_ok(shop_region.as_ref()), "Shop list region"),
        mark(point_ok(scroll_point.as_ref()), "Focus/scroll point"),
        mark(point_ok(max_button.as_ref()), "MAX button"),
    ];
    if let Some(problem) = BagGrid::disagreement(bag_grid.as_deref(), bag_slot.as_deref()) {
        calibration.push(item(false, "Bag grid vs slot pitch", &problem));
    }
    let mut session = vec![
        item(presets > 0, "buy presets", &presets.to_string()),
        item(
            sell_cap > 0,
            "sell cap",
            if sell_cap > 0 { "set" } else { "not set" },
        ),
        item(
            shop_rows > 0,
            "shop rows",
            &ShopGeometry::DEFAULT_ROWS.to_string(),
        ),
    ];
    let mut lines = Vec::new();
    if !rect_ok(bag_grid.as_ref()) || !rect_ok(bag_slot.as_ref()) {
        lines.push("the bag grid and slot are not calibrated".to_owned());
    }
    if !rect_ok(shop_region.as_ref()) {
        lines.push("the shop list region is not calibrated".to_owned());
    }
    if !point_ok(scroll_point.as_ref()) {
        lines.push("the focus point is not calibrated".to_owned());
    }
    if presets == 0 {
        lines.push("no buy presets configured".to_owned());
    }
    if sell_cap <= 0 {
        lines.push("the sell cap is not set".to_owned());
    }
    if let Some(problem) = BagGrid::disagreement(bag_grid.as_deref(), bag_slot.as_deref()) {
        lines.push(problem);
    }
    session.push(item(
        lines.is_empty(),
        "ready",
        if lines.is_empty() { "yes" } else { "no" },
    ));
    json!({"ready": lines.is_empty(), "calibration": calibration, "session": session, "lines": lines})
}

/// `calibration.shop.test_scroll {"notches":3,"up":true}`: the focus click plus one wheel scroll.
pub fn test_scroll(state: &CoreState, request: &Request) -> Response {
    let Some(notches) = request.args.get("notches").and_then(Value::as_i64) else {
        return bad(
            &request.id,
            "calibration.shop.test_scroll needs integer notches",
            "notches",
        );
    };
    let notches = match i32::try_from(notches) {
        Ok(notches) if (1..=ShopGeometry::MAX_SCROLL_NOTCHES).contains(&notches) => notches,
        _ => {
            return bad(
                &request.id,
                format!(
                    "a test scroll is 1..={} notches; {notches} is beyond the firmware guard",
                    ShopGeometry::MAX_SCROLL_NOTCHES
                ),
                "notches",
            );
        }
    };
    let up = request
        .args
        .get("up")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let focus =
        ints(state.config.pointer("/buy_sell/scroll_point")).filter(|values| values.len() == 2);
    let Some(focus) = focus else {
        return bad(
            &request.id,
            "The focus point isn't calibrated - draw the Buy/Sell focus point on the Calibrate tab first.",
            "scroll_point",
        );
    };
    if let Some(response) = check_input_lease(state, &request.id, request.cmd.as_str()) {
        return response;
    }
    let display = match game_display(&state.config) {
        Ok(display) => display,
        Err(error) => return win_error_response(&request.id, error),
    };
    let settings = Settings::from_config(&state.config);
    let (port, transport) = match board::open_board(&settings) {
        Ok(opened) => opened,
        Err(error) => return board_error_response(&request.id, error),
    };
    let mut effects = system_effects(transport, display);
    let focus = Point::new(focus[0], focus[1]);
    if let Err(error) = effects.click_at(focus) {
        return Response::err(
            &request.id,
            Error::new("board_unavailable", error.message(), json!({})),
        );
    }
    if let Err(error) = effects.wheel(notches, !up) {
        return Response::err(
            &request.id,
            Error::new("board_unavailable", error.message(), json!({})),
        );
    }
    Response::ok(
        &request.id,
        json!({
            "line": format!(
                "clicked the focus point at ({}, {}) and scrolled {notches} notch(es) {} through {port}",
                focus.x, focus.y, if up { "up" } else { "down" }
            ),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preview_derives_the_v2_geometry() {
        let state = CoreState::new(std::env::temp_dir());
        let response = preview(
            &state,
            &Request::new(
                "1",
                "calibration.buysell.preview",
                json!({"draft": {
                    "bag_grid": [0, 0, 800, 400],
                    "bag_slot": [0, 0, 100, 50],
                    "shop_region": [100, 300, 400, 400],
                    "shop_rows": 10
                }}),
            ),
        );
        assert!(response.ok, "{:?}", response.error);
        let result = response.result.unwrap();
        assert_eq!(result["bag_centres"].as_array().unwrap().len(), 64);
        assert_eq!(result["row_centres"].as_array().unwrap().len(), 10);
        assert_eq!(result["pitch"], json!({"w": 100, "h": 50}));
        assert_eq!(result["row_pitch"], 40.0);
        assert!(result["problem"].is_null());
    }
}

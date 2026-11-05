//! `calibration.tooltip.*`: the hover-panel calibration session (G01, PLAN-V3-CALIBRATION §5.2).
//!
//! The panel is anchored to the POINTER, so one offset serves every hover read in the game: the
//! calibrator parks the cursor on an item, waits for the panel, captures the screen, and the player
//! drags a box round the panel. The offset is the box minus the point the CORE placed the cursor on,
//! which is why the tool places it rather than reading it. The focus click uses the already-calibrated
//! buy/sell `scroll_point`, an inert place; clicking the hover point itself would act on the item
//! (on a bagged pet it switches the equipped pet).
//!
//! Three commands, one mechanism:
//! - `hover_capture` places, clicks focus, hovers, waits and returns the captured frame + display;
//! - `save` validates and writes the one `tooltip` block (offset/size/delay), one writer/rev/event;
//! - `test` hovers again and OCRs the configured region, so the player can see whether the read is
//!   useful BEFORE anything is built on it.
//!
//! Everything touching the game takes the same input-lease snapshot the tools do (`tool_busy` naming
//! the holder). The shell hides itself around these commands through the host channel, exactly as it
//! does for `capture`.

use seal_domain::config::TooltipConfig;
use seal_domain::geometry::{Point, Rect};
use seal_proto::{Error, Request, Response};
use serde_json::{Value, json};

use base64::Engine as _;

use crate::dispatch::{
    CoreState, board_error_response, ensure_ocr, frame_rgba, win_error_response,
};

/// v2's measured default (`TooltipConfig.HoverDelayMs`); a field because the first 700 ms read the
/// bag instead of the panel.
pub const DEFAULT_HOVER_DELAY_MS: u64 = 700;
/// The UI only accepts 100+ (`MainWindow`'s hover-delay field); the protocol bounds it so a typo
/// cannot make a capture command sleep for minutes.
pub const MAX_HOVER_DELAY_MS: u64 = 10_000;
/// The smallest saved box v2 accepts (`TooltipMouseUp`): anything smaller is a stray drag, not a panel.
pub const MIN_PANEL_PX: i32 = 20;
/// v2's settle after the focus click (`FocusThenHover`), before moving onto the item.
const FOCUS_SETTLE_MS: u64 = 400;

fn fail(id: &str, message: impl Into<String>, detail: Value) -> Response {
    Response::err(id, Error::new("bad_request", &message.into(), detail))
}

fn pointer_failure(id: &str, failure: crate::pointer::PointerFailure) -> Response {
    match failure {
        crate::pointer::PointerFailure::Window(error) => win_error_response(id, error),
        crate::pointer::PointerFailure::Board(error) => board_error_response(id, error),
        crate::pointer::PointerFailure::Effects(line) => {
            Response::err(id, Error::new("board_unavailable", &line, json!({})))
        }
    }
}

/// The shared input-lease snapshot: a running foreground tool (or the resident feeder holding the
/// game) refuses the calibration action with the same `tool_busy` line a start gets. `Some` means the
/// action is over before it touched the game.
pub(crate) fn check_input_lease(state: &CoreState, id: &str, command: &str) -> Option<Response> {
    let supervisor = state.supervisor.as_ref()?;
    match supervisor.input_lease() {
        Ok(None) => None,
        Ok(Some(line)) => Some(Response::err(
            id,
            Error::new(
                "tool_busy",
                &line,
                json!({"line": line, "command": command}),
            ),
        )),
        Err(line) => Some(Response::err(id, Error::new("internal", &line, json!({})))),
    }
}

fn point_argument(args: &Value, command: &str) -> Result<Point, (String, &'static str)> {
    let Some(point) = args.get("point").and_then(Value::as_object) else {
        return Err((
            format!(
                "calibration.{command} needs point {{x, y}} (a client-relative physical point)"
            ),
            "point",
        ));
    };
    let x = point.get("x").and_then(Value::as_i64);
    let y = point.get("y").and_then(Value::as_i64);
    match (x, y) {
        (Some(x), Some(y)) if x >= 0 && y >= 0 => Ok(Point::new(
            i32::try_from(x).unwrap_or(i32::MAX),
            i32::try_from(y).unwrap_or(i32::MAX),
        )),
        _ => Err((
            format!("calibration.{command}'s point needs non-negative integer x and y"),
            "point",
        )),
    }
}

fn hover_delay_argument(
    args: &Value,
    config: &Value,
    command: &str,
) -> Result<u64, (String, &'static str)> {
    if let Some(value) = args.get("hover_delay_ms") {
        let delay = value.as_u64().filter(|delay| *delay <= MAX_HOVER_DELAY_MS);
        return match delay {
            Some(delay) => Ok(delay),
            None => Err((
                format!(
                    "calibration.{command}'s hover_delay_ms must be an integer between 0 and \
                     {MAX_HOVER_DELAY_MS}"
                ),
                "hover_delay_ms",
            )),
        };
    }
    let configured = config
        .pointer("/tooltip/hover_delay_ms")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_HOVER_DELAY_MS);
    Ok(configured.min(MAX_HOVER_DELAY_MS))
}

/// The configured focus point: v2's buy/sell `scroll_point`, reused so there is one inert point to
/// calibrate, not two. Missing is a named prerequisite, never a silent hover over the item.
fn focus_point(config: &Value) -> Result<Point, (String, &'static str)> {
    let values = config.pointer("/buy_sell/scroll_point").and_then(|value| {
        let list = value.as_array()?;
        if list.len() != 2 {
            return None;
        }
        Some((list[0].as_i64()?, list[1].as_i64()?))
    });
    match values {
        Some((x, y)) if x >= 0 && y >= 0 => Ok(Point::new(
            i32::try_from(x).unwrap_or(i32::MAX),
            i32::try_from(y).unwrap_or(i32::MAX),
        )),
        _ => Err((
            "No focus point is calibrated. Both tools click one inert point at the start of a run \
             to give the game focus — mark it on Calibrate Buy/Sell first. It must NOT be the item \
             being hovered: that click would act on it."
                .to_owned(),
            "scroll_point",
        )),
    }
}

fn tooltip_config(config: &Value) -> TooltipConfig {
    config
        .get("tooltip")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

/// The configured panel region for a placed point: the point plus the calibrated offset/size
/// (CALIBRATION §1: physical, client-relative, never scaled).
pub(crate) fn region_for(point: Point, tooltip: &TooltipConfig) -> Rect {
    Rect::new(
        point.x + tooltip.offset_x,
        point.y + tooltip.offset_y,
        tooltip.width,
        tooltip.height,
    )
}

fn place_focus_then_hover(
    config: &Value,
    id: &str,
    focus: Point,
    point: Point,
    hover_delay_ms: u64,
) -> Option<Response> {
    // The click is only for FOCUS: it lands on the inert scroll point, never on the hovered item.
    if let Err(error) = crate::pointer::system_perform(config, focus, true) {
        return Some(pointer_failure(id, error));
    }
    std::thread::sleep(std::time::Duration::from_millis(FOCUS_SETTLE_MS));
    if let Err(error) = crate::pointer::system_perform(config, point, false) {
        return Some(pointer_failure(id, error));
    }
    std::thread::sleep(std::time::Duration::from_millis(hover_delay_ms.max(200)));
    None
}

/// `calibration.tooltip.hover_capture {"point":{x,y},"hover_delay_ms":700}`: place (closed loop),
/// click the inert focus point, hover the point, wait, capture the client, and cache the frame so the
/// shell's next `calibration.save`/`tooltip.save` can reference it. The tool placed the cursor, so the
/// coordinate the offset is measured from is one it chose — nothing is read back from the screen.
pub fn hover_capture(state: &mut CoreState, request: &Request) -> Response {
    let point = match point_argument(&request.args, "tooltip.hover_capture") {
        Ok(point) => point,
        Err((message, field)) => {
            return fail(&request.id, message, json!({"field": field}));
        }
    };
    let delay = match hover_delay_argument(&request.args, &state.config, "tooltip.hover_capture") {
        Ok(delay) => delay,
        Err((message, field)) => {
            return fail(&request.id, message, json!({"field": field}));
        }
    };
    if let Some(response) = check_input_lease(state, &request.id, request.cmd.as_str()) {
        return response;
    }
    let focus = match focus_point(&state.config) {
        Ok(focus) => focus,
        Err((message, field)) => {
            return fail(&request.id, message, json!({"field": field}));
        }
    };
    let allow_covered = match crate::win::parse_allow_covered(&request.args) {
        Ok(allow) => allow,
        Err(error) => return win_error_response(&request.id, error),
    };
    let title = match crate::win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return win_error_response(&request.id, error),
    };
    if let Some(response) = place_focus_then_hover(&state.config, &request.id, focus, point, delay)
    {
        return response;
    }
    let (frame, display) = match crate::win::capture_with(&title, None, allow_covered) {
        Ok(result) => result,
        Err(error) => return win_error_response(&request.id, error),
    };
    let frame_id = state.frames.insert(frame);
    let Some(frame) = state.frames.get_value(&frame_id) else {
        return Response::err(
            &request.id,
            Error::new(
                "internal",
                "the captured frame left the frame cache",
                json!({}),
            ),
        );
    };
    Response::ok(
        &request.id,
        json!({
            "frame": frame,
            "display": display,
            "placed": {"x": point.x, "y": point.y},
            "line": format!(
                "Placed the cursor at ({}, {}) and captured the panel. Drag a box around the panel \
                 — the offset is measured from where the tool put the cursor.",
                point.x, point.y
            ),
        }),
    )
}

/// `calibration.tooltip.save`: validate and write the ONE `tooltip` block through the single writer;
/// rev bump + one `config.changed {block:"tooltip"}` after the response.
pub fn save(state: &mut CoreState, request: &Request) -> Response {
    let Some(offset_x) = request.args.get("offset_x").and_then(Value::as_i64) else {
        return fail(
            &request.id,
            "calibration.tooltip.save needs offset_x (the panel's left edge minus the hover point)",
            json!({"field": "offset_x"}),
        );
    };
    let Some(offset_y) = request.args.get("offset_y").and_then(Value::as_i64) else {
        return fail(
            &request.id,
            "calibration.tooltip.save needs offset_y (the panel's top edge minus the hover point)",
            json!({"field": "offset_y"}),
        );
    };
    let width = request.args.get("width").and_then(Value::as_i64);
    let height = request.args.get("height").and_then(Value::as_i64);
    let (Some(width), Some(height)) = (width, height) else {
        return fail(
            &request.id,
            "calibration.tooltip.save needs width and height (the panel box)",
            json!({"field": "width"}),
        );
    };
    if width < i64::from(MIN_PANEL_PX) || height < i64::from(MIN_PANEL_PX) {
        return fail(
            &request.id,
            format!(
                "The panel box is {width}x{height} px — too small. Drag a box at least \
                 {MIN_PANEL_PX}x{MIN_PANEL_PX} px around the LARGEST panel you care about (a pet's \
                 is bigger than a food item's)."
            ),
            json!({"field": "width"}),
        );
    }
    let delay = match hover_delay_argument(&request.args, &state.config, "tooltip.save") {
        Ok(delay) => delay,
        Err((message, field)) => {
            return fail(&request.id, message, json!({"field": field}));
        }
    };
    let block = json!({
        "offset_x": offset_x,
        "offset_y": offset_y,
        "width": width,
        "height": height,
        "hover_delay_ms": delay,
    });
    if let Err(error) = state.store.update_local(|local| {
        crate::calibration::set_tooltip_block(local, block.clone());
    }) {
        return Response::err(&request.id, Error::new("io", &error.to_string(), json!({})));
    }
    if let Some(object) = state.config.as_object_mut() {
        object.insert("tooltip".to_owned(), block.clone());
    }
    state.bump_revision();
    state.queue_config_changed("tooltip");
    Response::ok(&request.id, json!({"rev": state.rev, "block": block}))
}

/// `calibration.tooltip.test {"point":{x,y},"hover_delay_ms":700,"debug":true}`: hover the point again
/// and OCR the configured panel region. Read-only; the answer is what the player needs to tune the
/// delay or see the offset is wrong BEFORE anything is built on it.
#[allow(
    clippy::too_many_lines,
    reason = "the tooltip test keeps its capture, scan and evidence sequence together"
)]
pub fn test(state: &mut CoreState, request: &Request) -> Response {
    let tooltip = tooltip_config(&state.config);
    if !tooltip.is_set() {
        return fail(
            &request.id,
            "The tooltip panel isn't calibrated — hover the point, capture, drag a box around the \
             panel and Save first.",
            json!({"field": "tooltip"}),
        );
    }
    let point = match point_argument(&request.args, "tooltip.test") {
        Ok(point) => point,
        Err((message, field)) => {
            return fail(&request.id, message, json!({"field": field}));
        }
    };
    let delay = match hover_delay_argument(&request.args, &state.config, "tooltip.test") {
        Ok(delay) => delay,
        Err((message, field)) => {
            return fail(&request.id, message, json!({"field": field}));
        }
    };
    let debug = request
        .args
        .get("debug")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    if let Some(response) = check_input_lease(state, &request.id, request.cmd.as_str()) {
        return response;
    }
    let focus = match focus_point(&state.config) {
        Ok(focus) => focus,
        Err((message, field)) => {
            return fail(&request.id, message, json!({"field": field}));
        }
    };
    // OCR is loaded BEFORE the game is touched, exactly like `pet.test_read_panel`.
    if let Some(response) = ensure_ocr(state, &request.id) {
        return response;
    }
    let title = match crate::win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return win_error_response(&request.id, error),
    };
    if let Some(response) = place_focus_then_hover(&state.config, &request.id, focus, point, delay)
    {
        return response;
    }
    let region = region_for(point, &tooltip);
    let allow_covered = match crate::win::parse_allow_covered(&request.args) {
        Ok(allow) => allow,
        Err(error) => return win_error_response(&request.id, error),
    };
    let (frame, _display) = match crate::win::capture_with(&title, Some(region), allow_covered) {
        Ok(result) => result,
        Err(error) => return win_error_response(&request.id, error),
    };
    let (width, height, rgba) = match frame_rgba(&frame) {
        Ok(image) => image,
        Err(message) => {
            return Response::err(&request.id, Error::new("internal", &message, json!({})));
        }
    };
    let lines = {
        let Some(pipeline) = state.ocr.as_mut() else {
            return Response::err(
                &request.id,
                Error::new("internal", "the OCR pipeline is not loaded", json!({})),
            );
        };
        match pipeline.scan_rgba(width, height, &rgba) {
            Ok(scan) => scan
                .lines
                .into_iter()
                .map(|line| json!({"text": line.text, "score": f64::from(line.confidence)}))
                .collect::<Vec<Value>>(),
            Err(error) => {
                return Response::err(
                    &request.id,
                    Error::new("internal", &error.message, json!({})),
                );
            }
        }
    };
    let debug_path = if debug {
        let path = frame
            .get("data")
            .and_then(Value::as_str)
            .and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok())
            .and_then(|png| {
                state
                    .logs
                    .write_read(&format!("tooltip_{}.png", crate::logs::read_stamp()), &png)
            });
        path.map_or(Value::Null, Value::String)
    } else {
        Value::Null
    };
    Response::ok(
        &request.id,
        json!({
            "lines": lines,
            "region": {
                "x": region.x,
                "y": region.y,
                "width": region.w,
                "height": region.h,
            },
            "debug_path": debug_path,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Value {
        json!({
            "buy_sell": {"scroll_point": [10, 20]},
            "tooltip": {
                "offset_x": -8,
                "offset_y": -6,
                "width": 320,
                "height": 210,
                "hover_delay_ms": 700,
            }
        })
    }

    #[test]
    fn the_panel_region_is_point_plus_the_calibrated_offset() {
        let tooltip = tooltip_config(&config());
        assert_eq!(
            region_for(Point::new(100, 200), &tooltip),
            Rect::new(92, 194, 320, 210)
        );
    }

    #[test]
    fn missing_focus_point_is_a_named_prerequisite() {
        let error = focus_point(&json!({"buy_sell": {}})).unwrap_err();
        assert_eq!(error.1, "scroll_point");
        assert!(error.0.contains("focus point"), "{}", error.0);
    }

    #[test]
    fn hover_delay_defaults_from_config_and_bounds_the_argument() {
        assert_eq!(
            hover_delay_argument(&json!({}), &config(), "x").unwrap(),
            700
        );
        assert_eq!(
            hover_delay_argument(&json!({"hover_delay_ms": 1200}), &config(), "x").unwrap(),
            1200
        );
        assert_eq!(
            hover_delay_argument(&json!({"hover_delay_ms": 10_001}), &config(), "x")
                .unwrap_err()
                .1,
            "hover_delay_ms"
        );
    }

    #[test]
    fn a_point_argument_needs_two_non_negative_integers() {
        assert!(point_argument(&json!({"point": {"x": 3, "y": 4}}), "x").is_ok());
        assert_eq!(
            point_argument(&json!({"point": {"x": -1, "y": 4}}), "x")
                .unwrap_err()
                .1,
            "point"
        );
        assert_eq!(
            point_argument(&json!({"point": {"x": 1}}), "x")
                .unwrap_err()
                .1,
            "point"
        );
    }
}

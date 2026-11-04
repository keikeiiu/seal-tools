//! `calibration.gem.moves.save`: v2's tuned movement editor (G16, PLAN-V3-CALIBRATION §5.2).
//!
//! The Gem Composer's tuned route is raw HID counts, one `[dx, dy]` per leg. v2's editor parsed
//! every leg with a silent `0` fallback, so a typo produced a route that moved nowhere; this command
//! REFUSES a malformed leg instead, naming the route. It writes only `gem.movements` inside the
//! `gem` block, leaving the coordinates, empty reference and portable behavior untouched, through
//! the same single-writer/rev/config.changed path every other calibration save uses.
//!
//! A partial payload merges into the existing movements: the editor can save one leg at a time
//! without erasing the others. An empty payload is refused rather than performing a no-op save.

use base64::Engine as _;
use seal_domain::config::ColorComposition;
use seal_domain::geometry::Rect;
use seal_proto::{Error, Request, Response};
use serde_json::{Value, json};

use crate::dispatch::{CoreState, frame_rgba, win_error_response};

/// Every route v2's Gem Composer has. The order is the order the editor shows.
const ROUTES: &[&str] = &[
    "register_combine",
    "combine_register",
    "grade_next",
    "grade_prev",
    "register_slot1",
    "slot1_slot2",
    "slot2_slot3",
    "slot3_to_n",
    "slot3_to_g",
    "slot3_to_dg",
];
const RADIO_MAP: &str = "radio_to_register";

fn fail(id: &str, message: impl Into<String>, detail: Value) -> Response {
    Response::err(id, Error::new("bad_request", &message.into(), detail))
}

/// One `[dx, dy]` leg. Integers, exactly two, and bounded so a pasted coordinate cannot become an
/// enormous HID burst by accident (`±9999` is far beyond any real move).
fn leg(name: &str, value: &Value) -> Result<Vec<i32>, String> {
    let Some(array) = value.as_array() else {
        return Err(format!(
            "gem.movements.{name} must be a [dx, dy] pair of integers"
        ));
    };
    if array.len() != 2 {
        return Err(format!(
            "gem.movements.{name} has {} numbers; a route is exactly two (dx and dy)",
            array.len()
        ));
    }
    let mut pair = Vec::with_capacity(2);
    for number in array {
        let Some(number) = number.as_i64() else {
            return Err(format!("gem.movements.{name} needs integer dx and dy"));
        };
        let Ok(number) = i32::try_from(number) else {
            return Err(format!("gem.movements.{name} is outside the i32 range"));
        };
        if !(-9999..=9999).contains(&number) {
            return Err(format!(
                "gem.movements.{name} has {number}; a movement count must be between -9999 and 9999"
            ));
        }
        pair.push(number);
    }
    Ok(pair)
}

/// Parse and merge the payload into the existing movements object, refusing malformed input before
/// any write. Returns the merged block.
fn movements_argument(args: &Value, existing: Option<&Value>) -> Result<Value, (String, String)> {
    let Some(payload) = args.get("movements").and_then(Value::as_object) else {
        return Err((
            "calibration.gem.moves.save needs a movements object".to_owned(),
            "movements".to_owned(),
        ));
    };
    if payload.is_empty() {
        return Err((
            "calibration.gem.moves.save needs at least one route; nothing was written".to_owned(),
            "movements".to_owned(),
        ));
    }
    let mut merged = existing
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (name, value) in payload {
        if name == RADIO_MAP {
            let Some(map) = value.as_object() else {
                return Err((
                    "gem.movements.radio_to_register must be an object of [dx, dy] pairs"
                        .to_owned(),
                    name.clone(),
                ));
            };
            let mut routes = serde_json::Map::new();
            for (target, leg_value) in map {
                let pair = leg(&format!("radio_to_register.{target}"), leg_value)
                    .map_err(|message| (message, name.clone()))?;
                routes.insert(target.clone(), json!(pair));
            }
            merged.insert(name.clone(), Value::Object(routes));
            continue;
        }
        if !ROUTES.contains(&name.as_str()) {
            return Err((
                format!(
                    "unknown gem movement route {name:?}; known routes are {} and {RADIO_MAP}",
                    ROUTES.join(", ")
                ),
                name.clone(),
            ));
        }
        let pair = leg(name, value).map_err(|message| (message, name.clone()))?;
        merged.insert(name.clone(), json!(pair));
    }
    Ok(Value::Object(merged))
}

/// `calibration.gem.moves.save {"movements": {route: [dx, dy], ...}}`.
pub fn save(state: &mut CoreState, request: &Request) -> Response {
    let existing = state.config.pointer("/gem/movements").cloned();
    let movements = match movements_argument(&request.args, existing.as_ref()) {
        Ok(movements) => movements,
        Err((message, field)) => return fail(&request.id, message, json!({"field": field})),
    };
    let movements_for_write = movements.clone();
    if let Err(error) = state.store.update_local(|local| {
        if !local.get("gem").is_some_and(Value::is_object) {
            local["gem"] = json!({});
        }
        if let Some(gem) = local.get_mut("gem").and_then(Value::as_object_mut) {
            gem.insert("movements".to_owned(), movements_for_write.clone());
        }
    }) {
        return Response::err(&request.id, Error::new("io", &error.to_string(), json!({})));
    }
    if let Some(gem) = state.config.get_mut("gem").and_then(Value::as_object_mut) {
        gem.insert("movements".to_owned(), movements.clone());
    }
    state.bump_revision();
    state.queue_config_changed("gem");
    Response::ok(
        &request.id,
        json!({"rev": state.rev, "block": {"movements": movements}}),
    )
}

// ── G18: the result-box colour diagnostic ──────────────────────────────────────────────────────

/// v2's result-box colour sampler (`GemCheckResultColour` / `GemTestResult`). READ-ONLY: it crops
/// the result box out of a cached frame (or one fresh capture), reports the six-feature colour
/// composition, the distance to the saved empty reference, the empty/has-gem verdict, and the two
/// hint fields v2 showed (`spread`, `dominant`). It never clicks, writes or moves the cursor.
fn area_argument(value: &Value) -> Option<Rect> {
    let object = value.as_object()?;
    let get = |name: &str| {
        object
            .get(name)
            .and_then(Value::as_i64)
            .and_then(|number| i32::try_from(number).ok())
    };
    let (x, y, w, h) = (get("x")?, get("y")?, get("w")?, get("h")?);
    (w > 0 && h > 0).then_some(Rect::new(x, y, w, h))
}

fn saved_area(config: &Value) -> Option<Rect> {
    let array = config.pointer("/gem/result_gem_area")?.as_array()?;
    if array.len() != 4 {
        return None;
    }
    let mut numbers = [0i32; 4];
    for (index, item) in array.iter().enumerate() {
        numbers[index] = i32::try_from(item.as_i64()?).ok()?;
    }
    (numbers[2] > 0 && numbers[3] > 0)
        .then(|| Rect::new(numbers[0], numbers[1], numbers[2], numbers[3]))
}

fn composition_json(c: &ColorComposition) -> Value {
    json!({
        "mean_v": c.mean_v,
        "mean_sat": c.mean_sat,
        "colored_fraction": c.colored_fraction,
        "mean_r": c.mean_r,
        "mean_g": c.mean_g,
        "mean_b": c.mean_b,
    })
}

fn spread(c: &ColorComposition) -> f64 {
    c.mean_r.max(c.mean_g).max(c.mean_b) - c.mean_r.min(c.mean_g).min(c.mean_b)
}

fn dominant(c: &ColorComposition) -> &'static str {
    if spread(c) < 0.08 {
        return "neutral / pale";
    }
    if c.mean_r >= c.mean_g && c.mean_r >= c.mean_b {
        "reddom"
    } else if c.mean_g >= c.mean_r && c.mean_g >= c.mean_b {
        "greendom"
    } else {
        "bluedom"
    }
}

fn frame_for(state: &CoreState, request: &Request) -> Result<Value, Box<Response>> {
    if let Some(frame_id) = request.args.get("frame_id").and_then(Value::as_str) {
        return state.frames.get(frame_id).cloned().ok_or_else(|| {
            Box::new(fail(
                &request.id,
                format!("unknown frame_id {frame_id:?} — capture the window again"),
                json!({}),
            ))
        });
    }
    let title = match crate::win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return Err(Box::new(win_error_response(&request.id, error))),
    };
    crate::win::capture(&title, None)
        .map(|(frame, _display)| frame)
        .map_err(|error| Box::new(win_error_response(&request.id, error)))
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "frame pixel coordinates and counts are far inside the range these conversions can represent"
)]
fn crop_area(frame: &Value, area: Rect) -> Result<(u32, u32, Vec<u8>, String), String> {
    let (width, height, rgba) = frame_rgba(frame).map_err(|message| message.clone())?;
    let origin = frame
        .get("origin")
        .and_then(Value::as_object)
        .map_or((0, 0), |origin| {
            (
                i32::try_from(origin.get("x").and_then(Value::as_i64).unwrap_or(0)).unwrap_or(0),
                i32::try_from(origin.get("y").and_then(Value::as_i64).unwrap_or(0)).unwrap_or(0),
            )
        });
    let local = Rect::new(area.x - origin.0, area.y - origin.1, area.w, area.h);
    let (crop_w, crop_h, cropped) =
        seal_win::pure::crop_rgba(width as u32, height as u32, &rgba, local)
            .map_err(|error| format!("the result box could not be cropped: {error}"))?;
    let (pixels, _) = cropped.as_chunks::<4>();
    let mut bgra = Vec::with_capacity(cropped.len());
    for pixel in pixels {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    let png = seal_win::pure::encode_png(crop_w, crop_h, &bgra)
        .map_err(|error| format!("the result crop could not be encoded: {error}"))?;
    Ok((
        crop_w,
        crop_h,
        cropped,
        base64::engine::general_purpose::STANDARD.encode(png),
    ))
}

/// `calibration.gem.check_result {"frame_id":?, "area":?}`: v2's Check Result Colour and Sample
/// result gem folded into one read-only answer.
pub fn check_result(state: &CoreState, request: &Request) -> Response {
    let area = request
        .args
        .get("area")
        .and_then(area_argument)
        .or_else(|| saved_area(&state.config));
    let Some(area) = area else {
        return fail(
            &request.id,
            "calibration.gem.check_result needs an area, or gem.result_gem_area calibrated",
            json!({"field": "area"}),
        );
    };
    let frame = match frame_for(state, request) {
        Ok(frame) => frame,
        Err(response) => return *response,
    };
    let (width, height, rgba, crop_png_base64) = match crop_area(&frame, area) {
        Ok(crop) => crop,
        Err(message) => return fail(&request.id, message, json!({"field": "area"})),
    };
    let colored_gap_min = state
        .config
        .pointer("/gem/colored_gap_min")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(40);
    let composition = seal_domain::tools::gem::analyze_composition(
        &rgba,
        width as usize,
        height as usize,
        colored_gap_min,
    );
    let signature = state
        .config
        .pointer("/gem/empty_signature")
        .and_then(|value| serde_json::from_value::<ColorComposition>(value.clone()).ok());
    // TOOL-F15: this path is the COLOUR-SIGNATURE one, so it uses the signature distance — the old
    // single `empty_distance` key seeded it (0.18). The pixel-diff threshold (`empty_diff_max`, 0.01)
    // never applies here: the two metrics are on different scales.
    let empty_signature_distance = state
        .config
        .pointer("/gem/empty_signature_distance")
        .or_else(|| state.config.pointer("/gem/empty_distance"))
        .and_then(Value::as_f64)
        .unwrap_or(seal_domain::tools::gem::DEFAULT_EMPTY_SIGNATURE_DISTANCE);
    let (distance, verdict) = match &signature {
        Some(signature) => {
            let distance = seal_domain::tools::gem::composition_distance(&composition, signature);
            let empty = seal_domain::tools::gem::result_box_is_empty(
                &seal_domain::tools::gem::EmptyRead::Composition(composition),
                Some(signature),
                seal_domain::tools::gem::DEFAULT_EMPTY_DIFF_MAX,
                empty_signature_distance,
            );
            (json!(distance), if empty { "empty" } else { "has_gem" })
        }
        None => (Value::Null, "no_reference"),
    };
    Response::ok(
        &request.id,
        json!({
            "composition": composition_json(&composition),
            "distance": distance,
            "verdict": verdict,
            "spread": spread(&composition),
            "dominant": dominant(&composition),
            "crop_png_base64": crop_png_base64,
        }),
    )
}

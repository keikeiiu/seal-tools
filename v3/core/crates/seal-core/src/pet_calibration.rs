//! `calibration.pet.*`: the Calibrate Pet session (G03, PLAN-V3-CALIBRATION §5.2).
//!
//! The player marks, per row, the boarding toggle, the pet's slot and ONE strip across its food
//! slots; the core divides the strip (`FeederLayout`), derives each count and ETA region, crops the
//! marked pet slot as the empty reference, and writes only the CALIBRATION half of the `pet` block.
//! The queue, food cells, run flags and portable preferences are preserved.
//!
//! - `preview` is pure: it derives slot boxes/count regions/ETA regions (and the 64 bag centres)
//!   from a draft so the shell can draw the overlay before saving.
//! - `save` takes one frame id, up to four rows and the shared points; it writes the marks, the
//!   optional empty-slot crops (base64, from the frame), the calibration environment and an
//!   annotated `calib_pet.png`, in one local write and one revision.
//! - `test_read` is the READ-ONLY live companion: one capture, then each ticked row/slot is read at
//!   v2's upscale ladder (3×/9×/24×) until a count passes the score gate. It never writes config.

use std::time::Instant;

use base64::Engine as _;
use seal_domain::geometry::{Point, Rect};
use seal_domain::tools::pet::FeederLayout;
use seal_domain::tools::shop::BagGrid;
use seal_proto::{Error, Request, Response};
use serde_json::{Map, Value, json};

use crate::dispatch::{CoreState, ensure_ocr, frame_rgba, win_error_response};

const MAX_ROWS: usize = 4;
const MIN_DRAG_PX: i32 = 10;
const DEFAULT_LEFT_FRACTION: f64 = 0.40;
const DEFAULT_MIN_SCORE: f64 = 0.75;
const ARTIFACT: &str = "calib_pet.png";
/// v2 `PetTool.CountReadUpscales`: the ladder the run walks; the Test read shares it.
const UPSCALES: [u32; 3] = [3, 9, 24];

fn fail(id: &str, message: impl Into<String>, detail: Value) -> Response {
    Response::err(id, Error::new("bad_request", &message.into(), detail))
}

fn rect(value: Option<&Value>, field: &str) -> Result<Rect, (String, &'static str)> {
    let Some(value) = value else {
        return Err((format!("{field} is missing (a {{x,y,w,h}} box)"), "rows"));
    };
    let object = value
        .as_object()
        .ok_or_else(|| (format!("{field} must be an object {{x,y,w,h}}"), "rows"))?;
    let number = |name: &str| object.get(name).and_then(Value::as_i64);
    let (Some(x), Some(y), Some(w), Some(h)) = (number("x"), number("y"), number("w"), number("h"))
    else {
        return Err((format!("{field} needs integer x, y, w and h"), "rows"));
    };
    let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
        i32::try_from(x),
        i32::try_from(y),
        i32::try_from(w),
        i32::try_from(h),
    ) else {
        return Err((format!("{field} is outside the i32 range"), "rows"));
    };
    if w < MIN_DRAG_PX || h < MIN_DRAG_PX {
        return Err((
            format!("{field} is only {w}x{h} px — drag a real box (at least {MIN_DRAG_PX} px)"),
            "rows",
        ));
    }
    Ok(Rect::new(x, y, w, h))
}

/// A `{x,y,w,h}` box as the stored `[x,y,w,h]` form (drafts use the object form the protocol's
/// other calibration commands use; saved config uses the array).
fn object_rect(value: &Value) -> Option<Vec<i32>> {
    let object = value.as_object()?;
    let get = |name: &str| i32::try_from(object.get(name)?.as_i64()?).ok();
    Some(vec![get("x")?, get("y")?, get("w")?, get("h")?])
}

fn int_array(value: Option<&Value>) -> Option<Vec<i32>> {
    value.and_then(|value| {
        value
            .as_array()?
            .iter()
            .map(|item| i32::try_from(item.as_i64()?).ok())
            .collect()
    })
}

fn stacks(value: Option<&Value>) -> Result<i32, (String, &'static str)> {
    let Some(stacks) = value.and_then(Value::as_i64) else {
        return Err(("rows[].stacks is required (1..=5)".to_owned(), "rows"));
    };
    let Ok(stacks) = i32::try_from(stacks) else {
        return Err(("rows[].stacks is outside the i32 range".to_owned(), "rows"));
    };
    if !(1..=5).contains(&stacks) {
        return Err((
            format!("rows[].stacks is {stacks}; it must be between 1 and 5"),
            "rows",
        ));
    }
    Ok(stacks)
}

fn rows(value: &Value) -> Result<&Vec<Value>, (String, &'static str)> {
    match value.get("rows").and_then(Value::as_array) {
        Some(rows) if (1..=MAX_ROWS).contains(&rows.len()) => Ok(rows),
        Some(rows) if rows.is_empty() => Err(("no rows were ticked".to_owned(), "rows")),
        Some(rows) => Err((
            format!(
                "{} rows were sent; at most {MAX_ROWS} are supported",
                rows.len()
            ),
            "rows",
        )),
        None => Err(("rows must be an array".to_owned(), "rows")),
    }
}

fn fraction(value: &Value, field: &str) -> Result<f64, (String, &'static str)> {
    let number = value.get(field).and_then(Value::as_f64);
    match number {
        Some(number) if (0.0..=1.0).contains(&number) => Ok(number),
        _ => Err((
            format!("{field} must be a fraction between 0 and 1"),
            "rows",
        )),
    }
}

/// One draft row's derivation, used by preview (no frame) and save (frame for crops/artifact).
fn derive_row(row: &Value, left_fraction: f64) -> Value {
    let strip = int_array(row.get("feeder_strip"))
        .or_else(|| row.get("feeder_strip").and_then(object_rect));
    let stacks = stacks(row.get("stacks")).ok().unwrap_or(2);
    let slot_boxes = FeederLayout::slot_boxes(strip.as_deref(), stacks);
    let count_regions = slot_boxes.as_ref().map(|boxes| {
        boxes
            .iter()
            .filter_map(|slot| FeederLayout::read_region(Some(slot), left_fraction))
            .collect::<Vec<_>>()
    });
    let eta = FeederLayout::eta_region(strip.as_deref());
    let mut problems = Vec::new();
    if strip.is_none() || !BagGrid::is_valid_rect(strip.as_deref()) {
        problems.push("feeder_strip is missing or invalid");
    }
    if !(1..=5).contains(&stacks) {
        problems.push("stacks must be between 1 and 5");
    }
    json!({
        "slot_boxes": slot_boxes,
        "count_regions": count_regions,
        "eta_region": eta,
        "problems": problems,
    })
}

/// `calibration.pet.preview`: pure derivation for the overlay/checklist.
pub fn preview(_state: &CoreState, request: &Request) -> Response {
    let draft = request.args.get("draft").cloned().unwrap_or_default();
    let draft_rows = match rows(&draft) {
        Ok(rows) => rows,
        Err((message, field)) => {
            return fail(&request.id, message, json!({"field": field}));
        }
    };
    let left_fraction =
        fraction(&draft, "feeder_count_left_fraction").unwrap_or(DEFAULT_LEFT_FRACTION);
    let derived_rows = draft_rows
        .iter()
        .map(|row| derive_row(row, left_fraction))
        .collect::<Vec<Value>>();
    let bag_grid = int_array(draft.get("bag_grid"));
    let centres = BagGrid::centres(bag_grid.as_deref())
        .into_iter()
        .map(|point| json!([point.x, point.y]))
        .collect::<Vec<Value>>();
    let problem = if bag_grid.is_none() {
        Some("bag_grid is not calibrated")
    } else {
        None
    };
    Response::ok(
        &request.id,
        json!({
            "rows": derived_rows,
            "bag_centres": centres,
            "problem": problem,
        }),
    )
}

fn existing_slot(config: &Value, index: usize) -> Value {
    config
        .pointer("/pet/slots")
        .and_then(Value::as_array)
        .and_then(|slots| slots.get(index))
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn origin_of(frame: &Value) -> Point {
    frame
        .get("origin")
        .and_then(Value::as_object)
        .map(|origin| {
            Point::new(
                i32::try_from(origin.get("x").and_then(Value::as_i64).unwrap_or(0)).unwrap_or(0),
                i32::try_from(origin.get("y").and_then(Value::as_i64).unwrap_or(0)).unwrap_or(0),
            )
        })
        .unwrap_or(Point::new(0, 0))
}

fn shifted(rect: Rect, origin: Point) -> Rect {
    Rect::new(rect.x - origin.x, rect.y - origin.y, rect.w, rect.h)
}

fn crop_rgba(frame: &Value, rect: Rect) -> Result<(u32, u32, Vec<u8>, String), String> {
    let (width, height, rgba) = frame_rgba(frame).map_err(|message| message.to_string())?;
    let origin = origin_of(frame);
    let local = Rect::new(rect.x - origin.x, rect.y - origin.y, rect.w, rect.h);
    let (crop_w, crop_h, cropped) =
        seal_win::pure::crop_rgba(width as u32, height as u32, &rgba, local)
            .map_err(|error| format!("the pet slot could not be cropped: {error}"))?;
    let (pixels, _) = cropped.as_chunks::<4>();
    let mut bgra = Vec::with_capacity(cropped.len());
    for pixel in pixels {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    let png = seal_win::pure::encode_png(crop_w, crop_h, &bgra)
        .map_err(|error| format!("the pet slot crop could not be encoded: {error}"))?;
    Ok((
        crop_w,
        crop_h,
        png.clone(),
        base64::engine::general_purpose::STANDARD.encode(png),
    ))
}

fn frame_from(state: &CoreState, request: &Request) -> Result<Value, Box<Response>> {
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

/// `calibration.pet.save`: marks + empty-slot crops + environment + annotation, one write, one rev.
pub fn save(state: &mut CoreState, request: &Request) -> Response {
    let rows = match rows(&request.args) {
        Ok(rows) => rows,
        Err((message, field)) => {
            return fail(&request.id, message, json!({"field": field}));
        }
    };
    let left_fraction =
        fraction(&request.args, "feeder_count_left_fraction").unwrap_or(DEFAULT_LEFT_FRACTION);
    let min_score = request
        .args
        .get("feeder_count_min_score")
        .and_then(Value::as_f64)
        .filter(|score| (0.0..=1.0).contains(score))
        .unwrap_or(DEFAULT_MIN_SCORE);
    let frame = match frame_from(state, request) {
        Ok(frame) => frame,
        Err(response) => return *response,
    };
    let origin = origin_of(&frame);
    let points = request.args.get("points").cloned().unwrap_or_default();
    let menu = match rect(points.get("menu"), "points.menu") {
        Ok(rect) => rect,
        Err((message, field)) => return fail(&request.id, message, json!({"field": field})),
    };
    let feed = match rect(points.get("feed"), "points.feed") {
        Ok(rect) => rect,
        Err((message, field)) => return fail(&request.id, message, json!({"field": field})),
    };
    let close = match rect(points.get("close"), "points.close") {
        Ok(rect) => rect,
        Err((message, field)) => return fail(&request.id, message, json!({"field": field})),
    };
    let page_tabs = match page_tabs(points.get("page_tabs")) {
        Ok(tabs) => tabs,
        Err((message, field)) => return fail(&request.id, message, json!({"field": field})),
    };
    let bag = request.args.get("bag").cloned().unwrap_or_default();
    let bag_grid = int_array(bag.get("grid")).filter(|grid| BagGrid::is_valid_rect(Some(grid)));
    let bag_slot = int_array(bag.get("slot")).filter(|slot| BagGrid::is_valid_rect(Some(slot)));

    let mut slot_entries = Vec::new();
    let mut drawings: Vec<crate::artifacts::Shape> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let toggle = match rect(
            row.get("toggle_label"),
            &format!("rows[{index}].toggle_label"),
        ) {
            Ok(rect) => rect,
            Err((message, field)) => return fail(&request.id, message, json!({"field": field})),
        };
        let boarding = match rect(
            row.get("boarding_pet_slot"),
            &format!("rows[{index}].boarding_pet_slot"),
        ) {
            Ok(rect) => rect,
            Err((message, field)) => return fail(&request.id, message, json!({"field": field})),
        };
        let strip = match rect(
            row.get("feeder_strip"),
            &format!("rows[{index}].feeder_strip"),
        ) {
            Ok(rect) => rect,
            Err((message, field)) => return fail(&request.id, message, json!({"field": field})),
        };
        let stacks = match stacks(row.get("stacks")) {
            Ok(stacks) => stacks,
            Err((message, field)) => return fail(&request.id, message, json!({"field": field})),
        };
        let crop = row
            .get("pet_slot_empty_png")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let previous = existing_slot(&state.config, index);
        let mut entry = json!({
            "enabled": previous.get("enabled").cloned().unwrap_or(json!(true)),
            "toggle_label": [toggle.x, toggle.y, toggle.w, toggle.h],
            "boarding_pet_slot": [boarding.x, boarding.y, boarding.w, boarding.h],
            "feeder_strip": [strip.x, strip.y, strip.w, strip.h],
            "stacks": stacks,
        });
        if let Some(running) = previous.get("boarding_running") {
            entry["boarding_running"] = running.clone();
        }
        if crop {
            match crop_rgba(&frame, boarding) {
                Ok((_, _, png, _)) => {
                    // Increment 6: crops live in config/artifacts/ and the config carries a small
                    // reference; a failed artifact write refuses the save rather than leaving a
                    // row whose reference picture is missing.
                    match crate::pet_artifacts::write_reference(
                        state.store.config_dir(),
                        &format!("pet_slot_empty_{}.png", index + 1),
                        &png,
                    ) {
                        Ok(reference) => {
                            entry["pet_slot_empty_png"] = Value::String(reference);
                        }
                        Err(error) => {
                            return Response::err(
                                &request.id,
                                Error::new(
                                    "io",
                                    &format!("the pet-slot crop could not be written: {error}"),
                                    json!({}),
                                ),
                            );
                        }
                    }
                }
                Err(message) => return fail(&request.id, message, json!({"field": "rows"})),
            }
        } else if let Some(previous) = previous.get("pet_slot_empty_png") {
            entry["pet_slot_empty_png"] = previous.clone();
        }
        slot_entries.push(entry);
        drawings.push(crate::artifacts::Shape::Rect {
            rect: shifted(toggle, origin),
            color: crate::artifacts::Color::Green,
        });
        drawings.push(crate::artifacts::Shape::Rect {
            rect: shifted(boarding, origin),
            color: crate::artifacts::Color::Blue,
        });
        drawings.push(crate::artifacts::Shape::Rect {
            rect: shifted(strip, origin),
            color: crate::artifacts::Color::Orange,
        });
    }
    for (rect, color) in [
        (menu, crate::artifacts::Color::Yellow),
        (feed, crate::artifacts::Color::Magenta),
        (close, crate::artifacts::Color::White),
    ] {
        drawings.push(crate::artifacts::Shape::Dot {
            point: Point::new(
                rect.x + rect.w / 2 - origin.x,
                rect.y + rect.h / 2 - origin.y,
            ),
            color,
            radius: 5,
        });
    }
    let encoded = frame
        .get("data")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let png = match base64::engine::general_purpose::STANDARD.decode(encoded) {
        Ok(png) => png,
        Err(error) => {
            return fail(
                &request.id,
                format!("the frame's PNG data is not base64: {error}"),
                json!({}),
            );
        }
    };
    let annotated = match crate::artifacts::annotate_png(&png, &drawings) {
        Ok(png) => png,
        Err(error) => {
            return fail(
                &request.id,
                format!("the frame could not be annotated: {error}"),
                json!({}),
            );
        }
    };
    let artifact = match state.store.write_artifact(ARTIFACT, &annotated) {
        Ok(path) => json!({"file": ARTIFACT, "path": path.display().to_string()}),
        Err(error) => {
            return Response::err(&request.id, Error::new("io", &error.to_string(), json!({})));
        }
    };

    let environment = request.args.get("display").map(|display| {
        json!({
            "dpi_scale": display.get("dpi_scale"),
            "monitor_dpi": display.get("monitor_dpi"),
            "screen": display.get("screen"),
            "client_size": [frame.get("width"), frame.get("height")],
            "measured_at": crate::events::utc_now_rfc3339(),
        })
    });
    let mut block = Map::new();
    block.insert(
        "menu_button".to_owned(),
        json!([menu.x, menu.y, menu.w, menu.h]),
    );
    block.insert(
        "feed_icon".to_owned(),
        json!([feed.x, feed.y, feed.w, feed.h]),
    );
    block.insert(
        "close_button".to_owned(),
        json!([close.x, close.y, close.w, close.h]),
    );
    block.insert("page_tabs".to_owned(), Value::Array(page_tabs));
    if let Some(grid) = &bag_grid {
        block.insert("bag_grid".to_owned(), json!(grid));
    }
    if let Some(slot) = &bag_slot {
        block.insert("bag_slot".to_owned(), json!(slot));
    }
    block.insert(
        "feeder_count_left_fraction".to_owned(),
        json!(left_fraction),
    );
    block.insert("feeder_count_min_score".to_owned(), json!(min_score));
    block.insert("slots".to_owned(), Value::Array(slot_entries.clone()));
    let block = Value::Object(block);
    let written = state.store.update_local(|local| {
        if !local.get("pet").is_some_and(Value::is_object) {
            local["pet"] = json!({});
        }
        if let Some(pet) = local.get_mut("pet").and_then(Value::as_object_mut) {
            for (key, value) in block.as_object().into_iter().flatten() {
                pet.insert(key.clone(), value.clone());
            }
        }
        if let Some(environment) = &environment {
            crate::calibration::set_environment_block(local, environment.clone());
        }
    });
    if let Err(error) = written {
        return Response::err(&request.id, Error::new("io", &error.to_string(), json!({})));
    }
    if let Some(pet) = state.config.get_mut("pet").and_then(Value::as_object_mut) {
        for (key, value) in block.as_object().into_iter().flatten() {
            pet.insert(key.clone(), value.clone());
        }
    }
    if let Some(environment) = &environment
        && let Some(object) = state.config.as_object_mut()
    {
        object.insert("calibration".to_owned(), environment.clone());
    }
    state.bump_revision();
    state.queue_config_changed("pet");
    if environment.is_some() {
        state.queue_config_changed("calibration");
    }
    Response::ok(
        &request.id,
        json!({
            "rev": state.rev,
            "block": block,
            "artifacts": [artifact],
            "derived": {"rows": rows.iter().map(|row| derive_row(row, left_fraction)).collect::<Vec<Value>>()},
            "warnings": [],
        }),
    )
}

fn page_tabs(value: Option<&Value>) -> Result<Vec<Value>, (String, &'static str)> {
    let Some(tabs) = value.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (index, tab) in tabs.iter().enumerate() {
        let object = tab.as_object().ok_or_else(|| {
            (
                format!("points.page_tabs[{index}] must be a point"),
                "points",
            )
        })?;
        let get = |name: &str| {
            object
                .get(name)
                .and_then(Value::as_i64)
                .and_then(|value| i32::try_from(value).ok())
                .filter(|value| *value >= 0)
        };
        let (Some(x), Some(y)) = (get("x"), get("y")) else {
            return Err((
                format!("points.page_tabs[{index}] needs non-negative integer x and y"),
                "points",
            ));
        };
        out.push(json!([x, y]));
    }
    Ok(out)
}

/// `calibration.pet.test_read`: one live capture, then the ticked rows' count regions read at v2's
/// upscale ladder. READ-ONLY; models/capture errors are the usual refusals.
pub fn test_read(state: &mut CoreState, request: &Request) -> Response {
    let draft_rows = match request.args.get("rows").and_then(Value::as_array) {
        Some(rows) if !rows.is_empty() && rows.len() <= MAX_ROWS => rows.clone(),
        _ => {
            return fail(
                &request.id,
                "test_read needs 1..=4 rows with feeder_strip/stacks",
                json!({"field": "rows"}),
            );
        }
    };
    let left_fraction =
        fraction(&request.args, "feeder_count_left_fraction").unwrap_or(DEFAULT_LEFT_FRACTION);
    let min_score = request
        .args
        .get("feeder_count_min_score")
        .and_then(Value::as_f64)
        .unwrap_or(DEFAULT_MIN_SCORE);
    if let Some(response) = ensure_ocr(state, &request.id) {
        return response;
    }
    let started = Instant::now();
    let title = match crate::win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return win_error_response(&request.id, error),
    };
    let (frame, _display) = match crate::win::capture(&title, None) {
        Ok(result) => result,
        Err(error) => return win_error_response(&request.id, error),
    };
    let mut result_rows = Vec::new();
    for (index, row) in draft_rows.iter().enumerate() {
        let strip = int_array(row.get("feeder_strip"));
        let stacks = stacks(row.get("stacks")).unwrap_or(2);
        let Some(boxes) = FeederLayout::slot_boxes(strip.as_deref(), stacks) else {
            result_rows.push(json!({
                "row": index + 1,
                "slots": [],
                "problem": "feeder_strip is missing or invalid",
            }));
            continue;
        };
        let mut slots = Vec::new();
        for (slot_index, slot) in boxes.iter().enumerate() {
            let Some(region) = FeederLayout::read_region(Some(slot), left_fraction) else {
                slots.push(json!({"index": slot_index + 1, "reads": [], "count": null}));
                continue;
            };
            let (reads, count) = read_count(state, &frame, region, min_score);
            slots.push(json!({
                "index": slot_index + 1,
                "reads": reads,
                "count": count,
            }));
        }
        let eta = FeederLayout::eta_region(strip.as_deref());
        result_rows.push(json!({"row": index + 1, "slots": slots, "eta_region": eta}));
    }
    Response::ok(
        &request.id,
        json!({
            "rows": result_rows,
            "hidden_ms": started.elapsed().as_millis() as u64,
        }),
    )
}

/// Read one count region at the 3×/9×/24× ladder; the first rung whose best line passes the score
/// gate wins (v2 `CountReadUpscales`). The returned `reads` keeps every rung's best line.
fn read_count(
    state: &mut CoreState,
    frame: &Value,
    region: Vec<i32>,
    min_score: f64,
) -> (Vec<Value>, Option<i32>) {
    let rect = Rect::new(region[0], region[1], region[2], region[3]);
    let Ok((width, height, rgba, _)) = crop_rgba(frame, rect) else {
        return (Vec::new(), None);
    };
    let mut reads = Vec::new();
    let pipeline = state.ocr.as_mut().expect("the OCR preflight loaded it");
    for upscale in std::iter::once(1).chain(UPSCALES) {
        let (scaled_w, scaled_h, scaled) = upscale_rgba(width, height, &rgba, upscale);
        let Ok(scan) = pipeline.scan_rgba(scaled_w as usize, scaled_h as usize, &scaled) else {
            continue;
        };
        let best = scan
            .lines
            .iter()
            .max_by(|left, right| left.confidence.total_cmp(&right.confidence));
        if let Some(line) = best {
            reads.push(json!({
                "text": line.text,
                "score": f64::from(line.confidence),
                "upscale": upscale,
            }));
            let pairs = vec![(line.text.clone(), f64::from(line.confidence))];
            if let Some(count) = seal_domain::tools::pet::FeederCount::parse(&pairs, min_score) {
                return (reads, Some(count));
            }
        }
    }
    (reads, None)
}

/// Nearest-neighbour upscale of one RGBA image (v2's ladder only needs more pixels for the detector;
/// no smoothing is claimed). Public for the ladder's unit test.
pub fn upscale_rgba(width: u32, height: u32, rgba: &[u8], factor: u32) -> (u32, u32, Vec<u8>) {
    let factor = factor.max(1);
    let scaled_w = width * factor;
    let scaled_h = height * factor;
    let mut out = Vec::with_capacity((scaled_w * scaled_h * 4) as usize);
    for y in 0..scaled_h {
        let source_y = y / factor;
        for x in 0..scaled_w {
            let source_x = x / factor;
            let index = ((source_y * width + source_x) * 4) as usize;
            out.extend_from_slice(&rgba[index..index + 4]);
        }
    }
    (scaled_w, scaled_h, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_upscaled_image_is_factor_squared_bigger_and_repeats_pixels() {
        let image = vec![1, 2, 3, 255, 4, 5, 6, 255];
        let (width, height, scaled) = upscale_rgba(2, 1, &image, 3);
        assert_eq!((width, height), (6, 3));
        assert_eq!(scaled.len(), image.len() * 9);
        assert_eq!(&scaled[0..4], &image[0..4]);
        assert_eq!(&scaled[4..8], &image[0..4]);
    }

    #[test]
    fn a_preview_derives_the_slot_boxes_count_regions_and_eta() {
        let state = CoreState::new(std::env::temp_dir());
        let response = preview(
            &state,
            &Request::new(
                "1",
                "calibration.pet.preview",
                json!({"draft": {"rows": [
                    {"feeder_strip": {"x": 0, "y": 100, "w": 200, "h": 40}, "stacks": 2}
                ], "bag_grid": [0, 0, 80, 80]}}),
            ),
        );
        assert!(response.ok, "{:?}", response.error);
        let result = response.result.unwrap();
        assert_eq!(result["rows"][0]["slot_boxes"].as_array().unwrap().len(), 2);
        assert_eq!(
            result["rows"][0]["count_regions"].as_array().unwrap().len(),
            2
        );
        assert!(result["rows"][0]["eta_region"].is_array());
        assert_eq!(result["bag_centres"].as_array().unwrap().len(), 64);
        assert!(result["problem"].is_null());
    }
}

//! `calibration.tuner.*`: v2's three-box tuner calibration session (G02, PLAN-V3-CALIBRATION §5.2).
//!
//! The player drags three boxes on a captured frame — the grade letter, the three attribute lines
//! and the spring-count row — and the core derives the OCR region and its region-relative bands
//! (`seal_domain::tools::tuner::geometry_from_boxes`). `check` runs the same OCR the tuner run runs
//! and answers the raw read, the dictionary-matched attributes and the filter verdict, so the player
//! can verify the boxes BEFORE saving. `save` writes `tuner.ocr` (and `tuner.spring_point` when
//! given), stamps the measured environment, annotates the frame and writes `calib_tuner.png`.
//!
//! v2 parity decisions kept here: the region is the bounding box of the boxes (a window move does not
//! invalidate the marks); `row_height` is the measured pitch when `check` measured one (the shell
//! passes it back on save), else `attr.height / 3`; a box smaller than 4 px is a stray drag; the
//! three band colours are green/blue/orange; the artifact is a full-client frame with the bands
//! drawn, so it is a reference, not a second set of coordinates.
//!
//! `check` is READ-ONLY. It needs OCR models; without them the answer is the usual
//! `ocr_unavailable`, before anything is written. `save` does NOT need OCR: the geometry is derived
//! from the boxes, exactly as v2's Save Tuner did.

use seal_domain::config::OcrGeometry;
use seal_domain::geometry::Rect;
use seal_domain::ocr::AttrMatcher;
use seal_ocr::pipeline::OcrPipeline;
use seal_proto::{Error, Request, Response};
use serde_json::{Value, json};

use crate::dispatch::{CoreState, ensure_ocr, win_error_response};
use crate::tooltip_session::check_input_lease;

const MIN_BOX_PX: i32 = 4;
const MIN_REGION_PX: i32 = 20;
const ARTIFACT: &str = "calib_tuner.png";

fn fail(id: &str, message: impl Into<String>, detail: Value) -> Response {
    Response::err(id, Error::new("bad_request", &message.into(), detail))
}

fn internal(id: &str, message: &str) -> Response {
    Response::err(id, Error::new("internal", message, json!({})))
}

/// One dragged box. `x`/`y` are client-relative physical pixels, matching every calibration mark.
fn box_argument(args: &Value, name: &str) -> Result<Rect, (String, &'static str)> {
    let Some(value) = args.get("boxes").and_then(|boxes| boxes.get(name)) else {
        return Err((
            format!("calibration.tuner needs boxes.{name} {{x, y, w, h}}"),
            "boxes",
        ));
    };
    let Some(object) = value.as_object() else {
        return Err((format!("boxes.{name} must be an object"), "boxes"));
    };
    let number = |field: &str| object.get(field).and_then(Value::as_i64);
    let (Some(x), Some(y), Some(w), Some(h)) = (number("x"), number("y"), number("w"), number("h"))
    else {
        return Err((format!("boxes.{name} needs integer x, y, w and h"), "boxes"));
    };
    let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
        i32::try_from(x),
        i32::try_from(y),
        i32::try_from(w),
        i32::try_from(h),
    ) else {
        return Err((format!("boxes.{name} is outside the i32 range"), "boxes"));
    };
    if w < MIN_BOX_PX || h < MIN_BOX_PX {
        return Err((
            format!(
                "boxes.{name} is only {w}x{h} px — drag a real box (at least {MIN_BOX_PX} px on \
                 each side)"
            ),
            "boxes",
        ));
    }
    Ok(Rect::new(x, y, w, h))
}

/// The three boxes plus the derived geometry, or the first refusal with its field.
fn derive(args: &Value) -> Result<(OcrGeometry, Rect, Rect, Rect), (String, &'static str)> {
    let grade = box_argument(args, "grade")?;
    let attr = box_argument(args, "attr")?;
    let remaining = box_argument(args, "remaining")?;
    let measured = args
        .get("row_height")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok());
    let geometry = seal_domain::tools::tuner::geometry_from_boxes(grade, attr, remaining, measured);
    let region = geometry.region.unwrap_or_default();
    if region.width < MIN_REGION_PX || region.height < MIN_REGION_PX {
        return Err((
            format!(
                "the three boxes cover only {}x{} px — drag real rectangles",
                region.width, region.height
            ),
            "boxes",
        ));
    }
    Ok((geometry, grade, attr, remaining))
}

fn frame_origin(frame: &Value) -> Option<seal_domain::geometry::Point> {
    let origin = frame.get("origin")?.as_object()?;
    Some(seal_domain::geometry::Point::new(
        i32::try_from(origin.get("x")?.as_i64()?).ok()?,
        i32::try_from(origin.get("y")?.as_i64()?).ok()?,
    ))
}

/// The pixels of `region` inside `frame`, which may be the whole client or a region capture: the
/// frame's origin says where it starts, and the crop is refused when the region falls outside it.
fn region_pixels(frame: &Value, region: Rect) -> Result<(u32, u32, Vec<u8>), String> {
    let encoded = frame
        .get("data")
        .and_then(Value::as_str)
        .filter(|data| !data.is_empty())
        .ok_or_else(|| "the frame carries no PNG data to read".to_owned())?;
    let png = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
        .map_err(|error| format!("the frame's PNG data is not base64: {error}"))?;
    let (width, height, rgba) = seal_win::pure::decode_png(&png)
        .map_err(|error| format!("the frame is not a PNG: {error}"))?;
    let origin = frame_origin(frame).unwrap_or(seal_domain::geometry::Point::new(0, 0));
    let local = Rect::new(region.x - origin.x, region.y - origin.y, region.w, region.h);
    if local.x < 0 || local.y < 0 || local.right() > width as i32 || local.bottom() > height as i32
    {
        return Err("the OCR region does not fit inside the captured frame".to_owned());
    }
    seal_win::pure::crop_rgba(width, height, &rgba, local)
        .map_err(|error| format!("the OCR region could not be cropped: {error}"))
}

/// The OCR-side geometry (`seal_ocr::modes`) for a derived `OcrGeometry`, through the same producer
/// `ocr.scan` uses (so `grade_area` validity and the fallback rules cannot drift).
fn mode_geometry(geometry: &OcrGeometry) -> seal_ocr::modes::OcrGeometry {
    let config = json!({"tuner": {"ocr": geometry}});
    crate::dispatch::ocr_geometry(&config)
}

fn y_of(item: &seal_ocr::pipeline::RecognizedLine) -> f64 {
    item.box_quad
        .0
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min)
}

fn x_of(item: &seal_ocr::pipeline::RecognizedLine) -> f64 {
    item.box_quad
        .0
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min)
}

fn max_x_of(item: &seal_ocr::pipeline::RecognizedLine) -> f64 {
    item.box_quad
        .0
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max)
}

fn in_band(value: f64, band: &[i32]) -> bool {
    match band {
        [low, high] => value >= f64::from(*low) && value <= f64::from(*high),
        _ => true,
    }
}

/// v2's remaining-count read: the X+Y-filtered rows in `remaining_y`/`remaining_x`, highest
/// confidence first, first plausible number (1..=99999) wins. The regex is replaced by "collect the
/// ASCII digits", which is the same rule on the cleaned text.
fn read_remaining(
    items: &[seal_ocr::pipeline::RecognizedLine],
    geometry: &OcrGeometry,
) -> Option<u32> {
    let mut candidates: Vec<&seal_ocr::pipeline::RecognizedLine> = items
        .iter()
        .filter(|item| {
            in_band(y_of(item), &geometry.remaining_y)
                && (geometry.remaining_x.len() != 2
                    || (x_of(item) >= f64::from(geometry.remaining_x[0])
                        && max_x_of(item) <= f64::from(geometry.remaining_x[1])))
                && item.confidence > 0.4
        })
        .collect();
    candidates.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for item in candidates {
        let digits: String = item.text.chars().filter(char::is_ascii_digit).collect();
        if let Ok(value) = digits.parse::<u32>()
            && (1..=99_999).contains(&value)
        {
            return Some(value);
        }
    }
    None
}

fn filter_config(config: &Value) -> seal_domain::config::FilterConfig {
    config
        .get("tuner")
        .and_then(|tuner| tuner.get("filter"))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn matched_json(matched: &[seal_domain::ocr::MatchedAttr]) -> Vec<Value> {
    matched
        .iter()
        .map(|item| {
            json!({
                "name": item.name,
                "category": item.category,
                "value": item.value,
                "raw": item.raw,
            })
        })
        .collect()
}

fn filter_json(
    filter: &seal_domain::config::FilterConfig,
    matched: &[seal_domain::ocr::MatchedAttr],
) -> Value {
    if !filter.enabled {
        return json!({"enabled": false, "passed": Value::Null, "reason": Value::Null, "hits": []});
    }
    let verdict = AttrMatcher::check_filter(matched, filter);
    json!({
        "enabled": true,
        "passed": verdict.passed,
        "reason": verdict.reason,
        "hits": verdict.hits.iter().map(|hit| json!({
            "rule": hit.rule,
            "found": hit.found,
            "value": hit.value,
            "override": hit.is_override,
        })).collect::<Vec<Value>>(),
    })
}

/// Run the check on a frame. `geometry` is the derived one; `frame` may be the whole client or a
/// region capture. Returns the result object or a response-shaped refusal.
fn check_frame(
    state: &mut CoreState,
    id: &str,
    geometry: &OcrGeometry,
    frame: &Value,
) -> Result<Value, Box<Response>> {
    if let Some(response) = ensure_ocr(state, id) {
        return Err(Box::new(response));
    }
    let configured = geometry.region.unwrap_or_default();
    let region = Rect::new(
        configured.left,
        configured.top,
        configured.width,
        configured.height,
    );
    let (width, height, rgba) =
        region_pixels(frame, region).map_err(|message| Box::new(fail(id, message, json!({}))))?;
    let pipeline: &mut OcrPipeline = state.ocr.as_mut().expect("the OCR preflight loaded it");
    let scan = match pipeline.scan_rgba(width as usize, height as usize, &rgba) {
        Ok(scan) => scan,
        Err(error) => return Err(Box::new(internal(id, &error.message))),
    };
    let modes = mode_geometry(geometry);
    let grade = seal_ocr::modes::read_grade(&scan.items, &modes);
    let attributes = seal_ocr::modes::read_attributes(&scan.items, &state.attributes, &modes);
    let remaining = read_remaining(&scan.items, geometry);
    let measured_row_height = seal_ocr::modes::measure_line_spacing(&scan.items);
    let matcher = AttrMatcher::new(&state.attributes);
    let matched = matcher.match_attributes(&attributes);
    let filter = filter_config(&state.config);

    let mut warnings: Vec<String> = Vec::new();
    if measured_row_height.is_none() {
        warnings.push(
            "only one attribute line was recognised, so no line pitch could be measured; the saved \
             row_height stays attr height / 3"
                .to_owned(),
        );
    }
    if attributes.iter().all(Vec::is_empty) {
        warnings.push("no attribute line was recognised inside the attr box".to_owned());
    }
    if remaining.is_none() {
        warnings.push("no spring count was recognised inside the remaining box".to_owned());
    }

    Ok(json!({
        "geometry": geometry,
        "measured_row_height": measured_row_height,
        "grade": grade,
        "remaining": remaining,
        "attributes": attributes,
        "matched": matched_json(&matched),
        "filter": filter_json(&filter, &matched),
        "warnings": warnings,
    }))
}

/// `calibration.tuner.check`: pure read for the three boxes (and `row_height` when the shell has a
/// measured pitch). No write, no revision.
pub fn check(state: &mut CoreState, request: &Request) -> Response {
    let (geometry, _grade, _attr, _remaining) = match derive(&request.args) {
        Ok(parts) => parts,
        Err((message, field)) => {
            return fail(&request.id, message, json!({"field": field}));
        }
    };
    let frame = match cached_or_live_frame(state, &request.id, &request.args) {
        Ok(frame) => frame,
        Err(response) => return *response,
    };
    match check_frame(state, &request.id, &geometry, &frame) {
        Ok(result) => Response::ok(&request.id, result),
        Err(response) => *response,
    }
}

/// A cached frame by `frame_id`, or a fresh full-client capture when the shell did not capture one.
/// The hide/show around the capture is the shell's contract (as for `capture`).
fn cached_or_live_frame(state: &CoreState, id: &str, args: &Value) -> Result<Value, Box<Response>> {
    if let Some(frame_id) = args.get("frame_id").and_then(Value::as_str) {
        return state.frames.get(frame_id).cloned().ok_or_else(|| {
            Box::new(fail(
                id,
                format!("unknown frame_id {frame_id:?} — capture the window again"),
                json!({}),
            ))
        });
    }
    let title = match crate::win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return Err(Box::new(win_error_response(id, error))),
    };
    crate::win::capture(&title, None)
        .map(|(frame, _display)| frame)
        .map_err(|error| Box::new(win_error_response(id, error)))
}

/// The display environment a `save` records. `display` is the shell's env-shaped payload
/// (`dpi_scale`, `monitor_dpi`, `screen`), never a full `DisplayInfo`; `client_size` is the captured
/// frame's own dimensions, which is exactly what the calibration block stores.
fn environment(display: Option<&Value>, frame: &Value) -> Option<Value> {
    let display = display?;
    let width = frame.get("width").and_then(Value::as_u64).unwrap_or(0);
    let height = frame.get("height").and_then(Value::as_u64).unwrap_or(0);
    Some(json!({
        "dpi_scale": display.get("dpi_scale"),
        "monitor_dpi": display.get("monitor_dpi"),
        "screen": display.get("screen"),
        "client_size": [width, height],
        "measured_at": crate::events::utc_now_rfc3339(),
    }))
}

/// `calibration.tuner.save`: derive the geometry, annotate the frame, write `tuner.ocr` /
/// `tuner.spring_point` / the calibration environment in ONE local write, bump the revision, echo
/// the written block and queue the `config.changed` events after the response.
pub fn save(state: &mut CoreState, request: &Request) -> Response {
    let (geometry, grade, attr, remaining) = match derive(&request.args) {
        Ok(parts) => parts,
        Err((message, field)) => {
            return fail(&request.id, message, json!({"field": field}));
        }
    };
    if let Some(response) = check_input_lease(state, &request.id, request.cmd.as_str()) {
        return response;
    }
    let frame = match cached_or_live_frame(state, &request.id, &request.args) {
        Ok(frame) => frame,
        Err(response) => return *response,
    };
    // Draw the reference screenshot BEFORE the block write: a bad frame refuses the save rather
    // than leaving a calibration whose picture is missing.
    let encoded = frame
        .get("data")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let png = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded) {
        Ok(bytes) => bytes,
        Err(error) => {
            return fail(
                &request.id,
                format!("the frame's PNG data is not base64: {error}"),
                json!({}),
            );
        }
    };
    let origin = frame_origin(&frame).unwrap_or(seal_domain::geometry::Point::new(0, 0));
    let shifted = |rect: Rect| Rect::new(rect.x - origin.x, rect.y - origin.y, rect.w, rect.h);
    let annotated = crate::artifacts::annotate_png(
        &png,
        &[
            crate::artifacts::Shape::Rect {
                rect: shifted(grade),
                color: crate::artifacts::Color::Green,
            },
            crate::artifacts::Shape::Rect {
                rect: shifted(attr),
                color: crate::artifacts::Color::Blue,
            },
            crate::artifacts::Shape::Rect {
                rect: shifted(remaining),
                color: crate::artifacts::Color::Orange,
            },
        ],
    );
    let annotated = match annotated {
        Ok(bytes) => bytes,
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

    let spring = match request.args.get("spring_point") {
        None => None,
        Some(value) => {
            let point = value.as_object().and_then(|point| {
                Some((
                    i32::try_from(point.get("x")?.as_i64()?).ok()?,
                    i32::try_from(point.get("y")?.as_i64()?).ok()?,
                ))
            });
            match point {
                Some((x, y)) if x >= 0 && y >= 0 => Some(json!([x, y])),
                _ => {
                    return fail(
                        &request.id,
                        "spring_point needs non-negative integer x and y",
                        json!({"field": "spring_point"}),
                    );
                }
            }
        }
    };
    let environment = environment(request.args.get("display"), &frame);
    let mut block = json!({"ocr": geometry.clone()});
    if let Some(spring) = &spring {
        block["spring_point"] = spring.clone();
    }
    if let Some(environment) = &environment {
        block["calibration"] = environment.clone();
    }
    let block_for_write = block.clone();
    let written = state.store.update_local(|local| {
        set_tuner_ocr(
            local,
            block_for_write.get("ocr").cloned().unwrap_or_default(),
        );
        if let Some(spring) = block_for_write.get("spring_point") {
            set_tuner_spring(local, spring.clone());
        }
        if let Some(environment) = block_for_write.get("calibration") {
            crate::calibration::set_environment_block(local, environment.clone());
        }
    });
    if let Err(error) = written {
        return Response::err(&request.id, Error::new("io", &error.to_string(), json!({})));
    }
    // Keep the in-memory merged view identical to the file, or the next run reads the old geometry.
    if let Some(object) = state.config.as_object_mut() {
        let tuner = object.entry("tuner").or_insert_with(|| json!({}));
        if let Some(tuner) = tuner.as_object_mut() {
            if let Some(ocr) = block.get("ocr") {
                tuner.insert("ocr".to_owned(), ocr.clone());
            }
            if let Some(spring) = block.get("spring_point") {
                tuner.insert("spring_point".to_owned(), spring.clone());
            }
        }
        if let Some(environment) = block.get("calibration") {
            object.insert("calibration".to_owned(), environment.clone());
        }
    }
    state.bump_revision();
    state.queue_config_changed("tuner");
    if environment.is_some() {
        state.queue_config_changed("calibration");
    }
    Response::ok(
        &request.id,
        json!({
            "rev": state.rev,
            "block": block,
            "derived": geometry,
            "artifact": artifact,
        }),
    )
}

fn set_tuner_ocr(local: &mut Value, ocr: Value) {
    if !local.get("tuner").is_some_and(Value::is_object) {
        local["tuner"] = json!({});
    }
    if let Some(tuner) = local.get_mut("tuner").and_then(Value::as_object_mut) {
        tuner.insert("ocr".to_owned(), ocr);
    }
}

fn set_tuner_spring(local: &mut Value, spring: Value) {
    if !local.get("tuner").is_some_and(Value::is_object) {
        local["tuner"] = json!({});
    }
    if let Some(tuner) = local.get_mut("tuner").and_then(Value::as_object_mut) {
        tuner.insert("spring_point".to_owned(), spring);
    }
}

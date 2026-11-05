//! `calibration.gem.save`: v2's Calibrate Gem save (G04's missing artifact half).
//!
//! Two shapes, one command:
//!
//! - **Coordinates-only** (no frame): writes the grade/Register/Combine points, the three resource
//!   points and the result box, leaving the empty signature alone (v2's "Save Coordinates").
//! - **Full save** (frame): the same marks PLUS the result box sampled from the frame. `empty:true`
//!   stores the sampled colour composition as `gem.empty_signature` (the empty reference the result
//!   check compares against); `empty:false` keeps the existing reference. It writes two natural-size
//!   artifacts — `calib_gem.png` (the frame with every point dotted and the result box outlined) and
//!   `calib_gem_result.png` (the result box crop) — so the reference gallery has something to show
//!   after a restart.
//!
//! A refused save writes nothing: every coordinate is validated, the crop/annotation is built, and
//! only then does the one `gem.movements`-style local write happen.

use base64::Engine as _;
use seal_domain::config::ColorComposition;
use seal_domain::geometry::{Point, Rect};
use seal_domain::tools::gem::analyze_composition;
use seal_proto::{Error, Request, Response};
use serde_json::{Map, Value, json};

use crate::dispatch::{CoreState, frame_rgba};

const ARTIFACT: &str = "calib_gem.png";
const RESULT_ARTIFACT: &str = "calib_gem_result.png";
const POINT_NAMES: &[&str] = &["N", "G", "DG", "Register", "Combine"];

fn fail(id: &str, message: impl Into<String>, field: &str) -> Response {
    Response::err(
        id,
        Error::new("bad_request", &message.into(), json!({"field": field})),
    )
}

fn point(value: &Value, name: &str) -> Result<Point, String> {
    let Some(values) = value.as_array() else {
        return Err(format!("{name} must be a [x, y] point"));
    };
    if values.len() != 2 {
        return Err(format!("{name} must be exactly two numbers [x, y]"));
    }
    let x = values[0]
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| format!("{name} needs integer x and y"))?;
    let y = values[1]
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| format!("{name} needs integer x and y"))?;
    Ok(Point::new(x, y))
}

fn rect(value: &Value, name: &str) -> Result<Rect, String> {
    let Some(values) = value.as_array() else {
        return Err(format!("{name} must be [x, y, w, h]"));
    };
    if values.len() != 4 {
        return Err(format!("{name} must be exactly four numbers [x, y, w, h]"));
    }
    let mut numbers = [0i32; 4];
    for (index, item) in values.iter().enumerate() {
        numbers[index] = item
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| format!("{name} needs integer x, y, w and h"))?;
    }
    if numbers[2] <= 0 || numbers[3] <= 0 {
        return Err(format!("{name} must have a positive width and height"));
    }
    Ok(Rect::new(numbers[0], numbers[1], numbers[2], numbers[3]))
}

/// The parsed save draft.
struct Draft {
    points: Vec<(String, Point)>,
    resources: Option<Vec<Point>>,
    result_area: Option<Rect>,
    frame_id: Option<String>,
    empty: Option<bool>,
}

fn draft(args: &Value) -> Result<Draft, (String, String)> {
    let mut points = Vec::new();
    if let Some(object) = args.get("points").and_then(Value::as_object) {
        for (name, value) in object {
            if !POINT_NAMES.contains(&name.as_str()) {
                return Err((
                    format!("unknown gem point {name:?}; known points are {POINT_NAMES:?}"),
                    "points".to_owned(),
                ));
            }
            let at = point(value, name).map_err(|message| (message, "points".to_owned()))?;
            points.push((name.clone(), at));
        }
    }
    let resources = match args.get("resources") {
        None => None,
        Some(value) => {
            let Some(values) = value.as_array() else {
                return Err((
                    "resources must be an array of three [x, y] points".to_owned(),
                    "resources".to_owned(),
                ));
            };
            if values.len() != 3 {
                return Err((
                    format!("resources needs exactly three points; got {}", values.len()),
                    "resources".to_owned(),
                ));
            }
            let mut out = Vec::with_capacity(3);
            for (index, value) in values.iter().enumerate() {
                out.push(
                    point(value, &format!("Resource{}", index + 1))
                        .map_err(|message| (message, "resources".to_owned()))?,
                );
            }
            Some(out)
        }
    };
    let result_area = match args.get("result_area") {
        None => None,
        Some(value) => Some(
            rect(value, "result_area").map_err(|message| (message, "result_area".to_owned()))?,
        ),
    };
    let empty = match args.get("empty") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(flag)) => Some(*flag),
        Some(_) => {
            return Err((
                "empty must be true, false or null".to_owned(),
                "empty".to_owned(),
            ));
        }
    };
    if points.is_empty() && resources.is_none() && result_area.is_none() {
        return Err((
            "calibration.gem.save needs at least one point, the resources or the result area"
                .to_owned(),
            "points".to_owned(),
        ));
    }
    Ok(Draft {
        points,
        resources,
        result_area,
        frame_id: args
            .get("frame_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        empty,
    })
}

fn frame_for(state: &CoreState, id: &str, draft: &Draft) -> Result<Option<Value>, Box<Response>> {
    let Some(frame_id) = &draft.frame_id else {
        return Ok(None);
    };
    state.frames.get_value(frame_id).map(Some).ok_or_else(|| {
        Box::new(fail(
            id,
            format!("unknown frame_id {frame_id:?} - capture the window again"),
            "frame_id",
        ))
    })
}

fn composition(c: &ColorComposition) -> Value {
    json!({
        "mean_v": c.mean_v,
        "mean_sat": c.mean_sat,
        "colored_fraction": c.colored_fraction,
        "mean_r": c.mean_r,
        "mean_g": c.mean_g,
        "mean_b": c.mean_b,
    })
}

/// `calibration.gem.save {frame_id?, points?, resources?, result_area?, empty?}`.
#[allow(
    clippy::cast_possible_truncation,
    reason = "frame pixel coordinates and counts are far inside the range these conversions can represent"
)]
#[allow(
    clippy::too_many_lines,
    reason = "one calibration command keeps its validation, path resolution and write in one place"
)]
pub fn save(state: &mut CoreState, request: &Request) -> Response {
    let draft = match draft(&request.args) {
        Ok(draft) => draft,
        Err((message, field)) => return fail(&request.id, message, &field),
    };
    let frame = match frame_for(state, &request.id, &draft) {
        Ok(frame) => frame,
        Err(response) => return *response,
    };
    let origin = frame
        .as_ref()
        .and_then(|frame| frame.get("origin"))
        .and_then(Value::as_object)
        .map_or((0, 0), |origin| {
            (
                i32::try_from(origin.get("x").and_then(Value::as_i64).unwrap_or(0)).unwrap_or(0),
                i32::try_from(origin.get("y").and_then(Value::as_i64).unwrap_or(0)).unwrap_or(0),
            )
        });
    let mut empty_signature = None;
    let mut artifacts = Vec::new();
    if let Some(frame) = &frame {
        let png = frame
            .get("data")
            .and_then(Value::as_str)
            .and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok());
        let Some(png) = png else {
            return fail(&request.id, "the frame has no base64 PNG data", "frame_id");
        };
        if let Some(area) = draft.result_area {
            let (width, height, rgba) = match frame_rgba(frame) {
                Ok(image) => image,
                Err(message) => {
                    return Response::err(&request.id, Error::new("io", &message, json!({})));
                }
            };
            let (crop_w, crop_h, crop) = match seal_win::pure::crop_rgba(
                width as u32,
                height as u32,
                &rgba,
                Rect::new(area.x - origin.0, area.y - origin.1, area.w, area.h),
            ) {
                Ok(crop) => crop,
                Err(error) => {
                    return fail(
                        &request.id,
                        format!("the result area could not be cropped: {error}"),
                        "result_area",
                    );
                }
            };
            let composition = analyze_composition(
                &crop,
                crop_w as usize,
                crop_h as usize,
                state
                    .config
                    .pointer("/gem/colored_gap_min")
                    .and_then(Value::as_i64)
                    .and_then(|value| i32::try_from(value).ok())
                    .unwrap_or(40),
            );
            let result_png = match encode_crop(crop_w, crop_h, &crop) {
                Ok(png) => png,
                Err(message) => {
                    return Response::err(&request.id, Error::new("io", &message, json!({})));
                }
            };
            if let Ok(path) = state.store.write_artifact(RESULT_ARTIFACT, &result_png) {
                artifacts
                    .push(json!({"file": RESULT_ARTIFACT, "path": path.display().to_string()}));
            }
            if draft.empty == Some(true) {
                empty_signature = Some(composition);
            }
        }
        let mut shapes = Vec::new();
        for (_, at) in &draft.points {
            shapes.push(crate::artifacts::Shape::Dot {
                point: Point::new(at.x - origin.0, at.y - origin.1),
                color: crate::artifacts::Color::Yellow,
                radius: 6,
            });
        }
        if let Some(resources) = &draft.resources {
            for at in resources {
                shapes.push(crate::artifacts::Shape::Dot {
                    point: Point::new(at.x - origin.0, at.y - origin.1),
                    color: crate::artifacts::Color::Yellow,
                    radius: 6,
                });
            }
        }
        if let Some(area) = draft.result_area {
            shapes.push(crate::artifacts::Shape::Rect {
                rect: Rect::new(area.x - origin.0, area.y - origin.1, area.w, area.h),
                color: crate::artifacts::Color::Magenta,
            });
        }
        match crate::artifacts::annotate_png(&png, &shapes) {
            Ok(annotated) => match state.store.write_artifact(ARTIFACT, &annotated) {
                Ok(path) => {
                    artifacts.push(json!({"file": ARTIFACT, "path": path.display().to_string()}));
                }
                Err(error) => {
                    return Response::err(
                        &request.id,
                        Error::new("io", &error.to_string(), json!({})),
                    );
                }
            },
            Err(error) => {
                return fail(
                    &request.id,
                    format!("the frame could not be annotated: {error}"),
                    "frame_id",
                );
            }
        }
    }
    let points = draft.points.clone();
    let resources = draft.resources.clone();
    let result_area = draft.result_area;
    let signature = empty_signature;
    if let Err(error) = state.store.update_local(|local| {
        if !local.get("gem").is_some_and(Value::is_object) {
            local["gem"] = json!({});
        }
        let Some(gem) = local.get_mut("gem").and_then(Value::as_object_mut) else {
            return;
        };
        let mut positions = gem
            .remove("grade_positions")
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        for (name, at) in &points {
            positions.insert(name.clone(), json!([at.x, at.y]));
        }
        if !positions.is_empty() {
            gem.insert("grade_positions".to_owned(), Value::Object(positions));
        }
        if let Some(resources) = &resources {
            gem.insert(
                "resource_gems".to_owned(),
                Value::Array(resources.iter().map(|at| json!([at.x, at.y])).collect()),
            );
        }
        if let Some(area) = result_area {
            gem.insert(
                "result_gem_area".to_owned(),
                json!([area.x, area.y, area.w, area.h]),
            );
        }
        if let Some(signature) = &signature {
            gem.insert("empty_signature".to_owned(), composition(signature));
        }
    }) {
        return Response::err(&request.id, Error::new("io", &error.to_string(), json!({})));
    }
    let mut gem = state
        .config
        .get("gem")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(object) = gem.as_object_mut() {
        let mut positions = object
            .get("grade_positions")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (name, at) in &points {
            positions.insert(name.clone(), json!([at.x, at.y]));
        }
        if !positions.is_empty() {
            object.insert("grade_positions".to_owned(), Value::Object(positions));
        }
        if let Some(resources) = &resources {
            object.insert(
                "resource_gems".to_owned(),
                Value::Array(resources.iter().map(|at| json!([at.x, at.y])).collect()),
            );
        }
        if let Some(area) = result_area {
            object.insert(
                "result_gem_area".to_owned(),
                json!([area.x, area.y, area.w, area.h]),
            );
        }
        if let Some(signature) = &signature {
            object.insert("empty_signature".to_owned(), composition(signature));
        }
    }
    state.config["gem"] = gem.clone();
    state.bump_revision();
    state.queue_config_changed("gem");
    let mut derived = Map::new();
    if let Some(area) = result_area {
        derived.insert(
            "result_area".to_owned(),
            json!({"x": area.x, "y": area.y, "w": area.w, "h": area.h}),
        );
    }
    if let Some(signature) = &empty_signature {
        derived.insert("empty_signature".to_owned(), composition(signature));
    }
    Response::ok(
        &request.id,
        json!({
            "rev": state.rev,
            "block": {"gem": gem},
            "derived": Value::Object(derived),
            "artifacts": artifacts,
        }),
    )
}

fn encode_crop(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut bgra = Vec::with_capacity(rgba.len());
    for pixel in rgba.as_chunks::<4>().0 {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    seal_win::pure::encode_png(width, height, &bgra)
        .map_err(|error| format!("the result crop could not be encoded: {error}"))
}

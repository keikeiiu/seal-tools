//! `calibration.save` / `calibration.fetch`: the measured half of `local.yaml` and its crop
//! artifacts.
//!
//! Why this is a command and not a file the shell writes: the calibrator draws on a CAPTURE, the
//! core owns the frame cache (`PROTOCOL.md` §8), and the marks land in `local.yaml` through the one
//! write gate (`CONFIG.md` §1). The shell sends the mark it drew and the id of the frame it drew on;
//! the core turns that into the file write, the derived values, and — for the kinds whose artifact is
//! a picture — the cropped PNG (`CALIBRATION.md` §5).
//!
//! The space is pinned by `CALIBRATION.md` §1: physical pixels, client-relative, never scaled. The
//! kinds are a table; a kind this build does not know is refused by name with the list, never
//! silently ignored.
//!
//! Two invariants:
//!
//! - **A refused save writes nothing and consumes no rev.** Values are validated and the artifact is
//!   cropped before the first byte reaches `local.yaml`.
//! - **The bag slot must agree with the bag grid.** They are two independent measurements of the same
//!   pitch; a save whose numbers cannot both be right is a bad drag, not something to average
//!   (`CALIBRATION.md` §1). The artifact is written first, then the marks: an orphan PNG is harmless,
//!   a mark that references a missing crop is not.

use base64::Engine as _;
use seal_domain::config::CalibrationInfo;
use seal_domain::geometry::{DisplayInfo, Rect};
use seal_domain::tools::gem::analyze_composition;
use seal_proto::{Error, Request, Response};
use serde_json::{Map, Value, json};

use crate::dispatch::CoreState;

/// The shape a mark has: a rectangle or a single point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Rect,
    Point,
}

/// Whether a kind is one mark or a NAMED INSTANCE selected by the protocol's `target` (§12.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Targets {
    None,
    /// `pet.page_tabs[index]`, wire `ITEM1`..`ITEM3`.
    PageTab,
    /// `pet.slots[index].<row_key>`, wire `"1"`..`"4"`.
    PetRow,
    /// `gem.grade_positions[<name>]`, wire v2's own names.
    GemGrade,
    /// `gem.resource_gems[index]`, wire `"1"`..`"3"`.
    GemResource,
}

/// One calibratable mark: its protocol name, its shape, where it lives in `local.yaml`, the artifact
/// crop it owns (if any), and whether it names one instance of an array/map.
#[derive(Debug)]
pub struct KindDef {
    pub name: &'static str,
    pub shape: Shape,
    /// The path inside `local.yaml`'s owned body, e.g. `["buy_sell", "bag_grid"]`; for a targeted kind
    /// this is the CONTAINER (the array or map) the instance lives in.
    pub path: &'static [&'static str],
    /// The crop artifact's bare file name under `config/`, when the kind has one.
    pub artifact: Option<&'static str>,
    /// Named-instance selection (`PROTOCOL.md` §12.8); `None` for a single mark.
    pub targets: Targets,
    /// For a pet-row mark: the key written on `pet.slots[i]` (`toggle_label` etc.).
    pub row_key: Option<&'static str>,
}

/// The kinds this build knows. Adding one is a table row, plus whatever derives from it.
pub const KINDS: &[KindDef] = &[
    KindDef {
        name: "bag_grid",
        shape: Shape::Rect,
        path: &["buy_sell", "bag_grid"],
        artifact: None,
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "bag_slot",
        shape: Shape::Rect,
        path: &["buy_sell", "bag_slot"],
        artifact: None,
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "shop_list",
        shape: Shape::Rect,
        path: &["buy_sell", "shop_region"],
        artifact: None,
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "scroll_point",
        shape: Shape::Point,
        path: &["buy_sell", "scroll_point"],
        artifact: None,
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "max_button",
        shape: Shape::Point,
        path: &["buy_sell", "max_button"],
        artifact: None,
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "result_gem_area",
        shape: Shape::Rect,
        path: &["gem", "result_gem_area"],
        artifact: Some("calib_gem_result.png"),
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "spring_point",
        shape: Shape::Point,
        path: &["tuner", "spring_point"],
        artifact: None,
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "pet_menu",
        shape: Shape::Point,
        path: &["pet", "menu_button"],
        artifact: None,
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "pet_feed",
        shape: Shape::Point,
        path: &["pet", "feed_icon"],
        artifact: None,
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "pet_close",
        shape: Shape::Point,
        path: &["pet", "close_button"],
        artifact: None,
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "pet_page_tab",
        shape: Shape::Point,
        path: &["pet", "page_tabs"],
        artifact: None,
        targets: Targets::PageTab,
        row_key: None,
    },
    KindDef {
        name: "pet_bag_grid",
        shape: Shape::Rect,
        path: &["pet", "bag_grid"],
        artifact: None,
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "pet_bag_slot",
        shape: Shape::Rect,
        path: &["pet", "bag_slot"],
        artifact: None,
        targets: Targets::None,
        row_key: None,
    },
    KindDef {
        name: "pet_toggle",
        shape: Shape::Rect,
        path: &["pet", "slots"],
        artifact: None,
        targets: Targets::PetRow,
        row_key: Some("toggle_label"),
    },
    KindDef {
        name: "pet_boarding_slot",
        shape: Shape::Rect,
        path: &["pet", "slots"],
        artifact: None,
        targets: Targets::PetRow,
        row_key: Some("boarding_pet_slot"),
    },
    KindDef {
        name: "pet_feeder_strip",
        shape: Shape::Rect,
        path: &["pet", "slots"],
        artifact: None,
        targets: Targets::PetRow,
        row_key: Some("feeder_strip"),
    },
    KindDef {
        name: "gem_grade",
        shape: Shape::Point,
        path: &["gem", "grade_positions"],
        artifact: None,
        targets: Targets::GemGrade,
        row_key: None,
    },
    KindDef {
        name: "gem_resource",
        shape: Shape::Point,
        path: &["gem", "resource_gems"],
        artifact: None,
        targets: Targets::GemResource,
        row_key: None,
    },
];
/// Kinds this build refuses BY NAME instead of accepting them. `focus_point` is the audit's wrong path
/// (nothing reads `buy_sell.focus_point`; the live shop reads `scroll_point`), and `tuner_region` is
/// withdrawn until v2's three-box derivation exists — a single rect wrote a shape the loader could
/// not parse. The reason travels in the refusal so the player is not left guessing.
fn withdrawn_line(name: &str) -> Option<&'static str> {
    match name {
        "focus_point" => Some(
            "calibration kind \"focus_point\" was replaced by \"scroll_point\" — save scroll_point \
             instead; buy_sell.focus_point is read by nothing.",
        ),
        "tuner_region" => Some(
            "calibration kind \"tuner_region\" was withdrawn: v2 derived the OCR region from three \
             dragged boxes and that three-box session is not built yet, so a single rect cannot \
             become a usable tuner.ocr region — hand-edit the tuner OCR geometry until it is.",
        ),
        _ => None,
    }
}

/// The kind whose pitch the bag-slot measurement must agree with.
const BAG_GRID: &str = "bag_grid";

/// How far the slot's implied pitch may differ from the grid's, in pixels of the whole 8×8 bag.
/// Two independent drags of the same thing are never identical; they are either the same block or a
/// bad drag, and one pixel of rounding is the first case.
const BAG_PITCH_TOLERANCE_PX: f64 = 1.0;

pub fn kind(name: &str) -> Option<&'static KindDef> {
    KINDS.iter().find(|kind| kind.name == name)
}

fn supported() -> Value {
    Value::Array(
        KINDS
            .iter()
            .map(|kind| Value::String(kind.name.to_owned()))
            .collect(),
    )
}

fn fail(id: &str, message: impl Into<String>, detail: Value) -> Response {
    Response::err(id, Error::new("bad_request", &message.into(), detail))
}

fn io_fail(id: &str, message: impl Into<String>) -> Response {
    Response::err(id, Error::new("io", &message.into(), json!({})))
}

fn integer(value: &Value, name: &str) -> Option<i32> {
    value
        .get(name)
        .and_then(Value::as_i64)
        .and_then(|number| i32::try_from(number).ok())
}

fn rect_argument(args: &Value) -> Result<Rect, String> {
    let region = args
        .get("region")
        .ok_or_else(|| "a rect mark needs region {x,y,w,h}".to_owned())?;
    let number = |name: &str| {
        integer(region, name).ok_or_else(|| format!("region needs an integer {name:?}"))
    };
    let rect = Rect::new(number("x")?, number("y")?, number("w")?, number("h")?);
    if rect.w <= 0 || rect.h <= 0 {
        return Err("a rect mark needs a positive w and h".to_owned());
    }
    Ok(rect)
}

fn allowed_targets(targets: Targets) -> &'static str {
    match targets {
        Targets::None => "",
        Targets::PageTab => "ITEM1, ITEM2, ITEM3",
        Targets::PetRow => "\"1\", \"2\", \"3\", \"4\"",
        Targets::GemGrade => "N, G, DG, Register, Combine",
        Targets::GemResource => "\"1\", \"2\", \"3\"",
    }
}

/// One named instance on the wire (`echo`) and the 0-based array index or map key it maps to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Instance {
    Index(usize),
    Key(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    echo: String,
    instance: Instance,
}

fn carried_len(config: &Value, def: &KindDef) -> usize {
    config
        .get(def.path[0])
        .and_then(|block| block.get(def.path[1]))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

/// Parses `target` for one command (`save`/`fetch`), validating it against the instances the config
/// actually carries. Returns `None` for a non-targeted kind, whose refusals name the field.
fn target_argument(
    def: &KindDef,
    args: &Value,
    command: &str,
    config: &Value,
) -> Result<Option<Target>, String> {
    if def.targets == Targets::None {
        if args.get("target").is_some() {
            return Err(format!(
                "calibration.{command}'s \"target\" is not accepted by {}",
                def.name
            ));
        }
        return Ok(None);
    }
    let Some(value) = args.get("target") else {
        return Err(format!(
            "calibration.{command} for {} needs \"target\" (allowed: {})",
            def.name,
            allowed_targets(def.targets)
        ));
    };
    let Some(text) = value.as_str() else {
        return Err(format!(
            "calibration.{command}'s \"target\" must be a string (allowed: {})",
            allowed_targets(def.targets)
        ));
    };
    let unknown = || {
        format!(
            "unknown target {text:?} for {} (allowed: {})",
            def.name,
            allowed_targets(def.targets)
        )
    };
    let instance = match def.targets {
        Targets::PageTab => {
            let index = text
                .strip_prefix("ITEM")
                .and_then(|number| number.parse::<usize>().ok())
                .filter(|index| (1..=3).contains(index))
                .ok_or_else(unknown)?;
            let carried = carried_len(config, def);
            if index > carried {
                return Err(format!(
                    "page tab {text} isn't configured (the config carries {carried})"
                ));
            }
            Instance::Index(index - 1)
        }
        Targets::PetRow => {
            let row = text
                .parse::<usize>()
                .ok()
                .filter(|row| (1..=4).contains(row))
                .ok_or_else(unknown)?;
            let carried = carried_len(config, def);
            if row > carried {
                return Err(format!(
                    "pet row {row} isn't configured (the config carries {carried})"
                ));
            }
            Instance::Index(row - 1)
        }
        Targets::GemGrade => {
            if !["N", "G", "DG", "Register", "Combine"].contains(&text) {
                return Err(unknown());
            }
            let configured = config
                .get("gem")
                .and_then(|gem| gem.get("grade_positions"))
                .and_then(Value::as_object)
                .is_some_and(|positions| positions.contains_key(text));
            if !configured {
                return Err(format!("gem grade {text:?} isn't configured"));
            }
            Instance::Key(text.to_owned())
        }
        Targets::GemResource => {
            let index = text
                .parse::<usize>()
                .ok()
                .filter(|index| (1..=3).contains(index))
                .ok_or_else(unknown)?;
            let carried = carried_len(config, def);
            if index > carried {
                return Err(format!(
                    "gem resource {text} isn't configured (the config carries {carried})"
                ));
            }
            Instance::Index(index - 1)
        }
        Targets::None => unreachable!("handled above"),
    };
    Ok(Some(Target {
        echo: text.to_owned(),
        instance,
    }))
}

fn instance_container<'a>(root: &'a Value, def: &KindDef) -> Option<&'a Value> {
    root.get(def.path[0])?.get(def.path[1])
}

/// The stored mark for one named instance: the array element itself (page tabs, resources), the
/// map value (gem grades), or the pet row's own key.
fn instance_mark<'a>(root: &'a Value, def: &KindDef, target: &Target) -> Option<&'a Value> {
    let container = instance_container(root, def)?;
    match &target.instance {
        Instance::Index(index) => {
            let item = container.as_array()?.get(*index)?;
            match def.row_key {
                Some(key) => item.get(key),
                None => Some(item),
            }
        }
        Instance::Key(key) => container.as_object()?.get(key),
    }
}

/// Writes ONE named instance. A pet-row mark inserts only `row_key` on `slots[i]`, so the row's
/// stacks, crops, `boarding_running` and its other two marks survive; arrays keep their order/length.
fn set_instance_mark(root: &mut Value, def: &KindDef, target: &Target, value: Value) {
    if !root.is_object() {
        *root = json!({});
    }
    let root_object = root.as_object_mut().expect("just made an object");
    let block = root_object.entry(def.path[0]).or_insert_with(|| json!({}));
    if !block.is_object() {
        *block = json!({});
    }
    let block_object = block.as_object_mut().expect("just made an object");
    let container = block_object.entry(def.path[1]).or_insert_with(|| json!({}));
    match &target.instance {
        Instance::Index(index) => {
            if !container.is_array() {
                *container = json!([]);
            }
            let array = container.as_array_mut().expect("just made an array");
            while array.len() <= *index {
                array.push(Value::Null);
            }
            if let Some(key) = def.row_key {
                if !array[*index].is_object() {
                    array[*index] = json!({});
                }
                if let Some(row) = array[*index].as_object_mut() {
                    row.insert(key.to_owned(), value);
                }
            } else {
                array[*index] = value;
            }
        }
        Instance::Key(key) => {
            if !container.is_object() {
                *container = json!({});
            }
            if let Some(map) = container.as_object_mut() {
                map.insert(key.clone(), value);
            }
        }
    }
}

/// so the player must say whether the box was empty), meaningless for every other kind.
fn empty_argument(def: &KindDef, args: &Value) -> Result<Option<bool>, String> {
    let Some(value) = args.get("empty") else {
        if def.name == "result_gem_area" {
            return Err(
                "calibration.save for result_gem_area needs \"empty\": is the result box empty \
                 (true) or holding a gem (false)?"
                    .to_owned(),
            );
        }
        return Ok(None);
    };
    if def.name != "result_gem_area" {
        return Err(format!(
            "calibration.save's \"empty\" is only accepted by result_gem_area, not {}",
            def.name
        ));
    }
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| "calibration.save's \"empty\" must be true or false".to_owned())
}

/// `gem.colored_gap_min` (0-255): the channel gap above which a pixel counts as distinctly coloured.
/// Same default as the gem plan (`gem.rs`), so calibration samples exactly what the run measures.
fn colored_gap_min(config: &Value) -> i32 {
    config
        .pointer("/gem/colored_gap_min")
        .and_then(Value::as_i64)
        .and_then(|number| i32::try_from(number).ok())
        .unwrap_or(40)
}

fn point_argument(args: &Value) -> Result<(i32, i32), String> {
    let point = args
        .get("point")
        .ok_or_else(|| "a point mark needs point {x,y}".to_owned())?;
    let number =
        |name: &str| integer(point, name).ok_or_else(|| format!("point needs an integer {name:?}"));
    Ok((number("x")?, number("y")?))
}

fn set_path(root: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() {
        *root = value;
        return;
    }
    if !root.is_object() {
        *root = json!({});
    }
    let mut cursor = root;
    for step in &path[..path.len() - 1] {
        let object = cursor.as_object_mut().expect("just made an object");
        cursor = object
            .entry((*step).to_owned())
            .or_insert_with(|| json!({}));
        if !cursor.is_object() {
            *cursor = json!({});
        }
    }
    if let Value::Object(object) = cursor {
        object.insert(path[path.len() - 1].to_owned(), value);
    }
}

fn get_path<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cursor = root;
    for step in path {
        cursor = cursor.get(*step)?;
    }
    Some(cursor)
}

fn four_numbers(value: &Value) -> Option<[i32; 4]> {
    let array = value.as_array()?;
    if array.len() != 4 {
        return None;
    }
    let mut numbers = [0i32; 4];
    for (index, item) in array.iter().enumerate() {
        numbers[index] = i32::try_from(item.as_i64()?).ok()?;
    }
    Some(numbers)
}

/// Write the tooltip block through the one local writer (the `calibration.tooltip.save` command).
pub(crate) fn set_tooltip_block(local: &mut Value, block: Value) {
    set_path(local, &["tooltip"], block);
}

/// Run `calibration.save`.
pub fn save(state: &mut CoreState, request: &Request) -> Response {
    let args = &request.args;
    let Some(kind_name) = args.get("kind").and_then(Value::as_str) else {
        return fail(
            &request.id,
            "calibration.save needs kind",
            json!({"kinds": supported()}),
        );
    };
    if let Some(line) = withdrawn_line(kind_name) {
        return fail(
            &request.id,
            line,
            json!({"kind": kind_name, "kinds": supported()}),
        );
    }
    let Some(def) = kind(kind_name) else {
        return fail(
            &request.id,
            format!("unknown calibration kind {kind_name:?}"),
            json!({"kinds": supported()}),
        );
    };
    // G06: optional compare-and-set. Absent keeps the legacy last-write-wins path; present and
    // stale is refused BEFORE validation, artifacts or the first write, carrying the current rev and
    // the block the save would have touched (the config.apply conflict shape).
    if let Some(rev) = match args.get("rev") {
        None => None,
        Some(value) => match value.as_u64() {
            Some(rev) => Some(rev),
            None => {
                return fail(
                    &request.id,
                    "calibration.save's \"rev\" must be a non-negative integer",
                    json!({"kind": kind_name}),
                );
            }
        },
    } && rev != state.rev
    {
        return calibration_conflict(state, &request.id, def);
    }

    let empty = match empty_argument(def, args) {
        Ok(empty) => empty,
        Err(message) => return fail(&request.id, message, json!({"kind": kind_name})),
    };
    let target = match target_argument(def, args, "save", &state.config) {
        Ok(target) => target,
        Err(message) => return fail(&request.id, message, json!({"kind": kind_name})),
    };
    let Some(frame_id) = args.get("frame_id").and_then(Value::as_str) else {
        return fail(
            &request.id,
            "calibration.save needs frame_id (the capture the mark was drawn on)",
            json!({}),
        );
    };
    let Some(frame) = state.frames.get(frame_id) else {
        return fail(
            &request.id,
            format!("unknown frame_id {frame_id:?} — capture the window again"),
            json!({}),
        );
    };
    let frame_width = frame.get("width").and_then(Value::as_u64).unwrap_or(0) as i32;
    let frame_height = frame.get("height").and_then(Value::as_u64).unwrap_or(0) as i32;

    let (mark, rect) = match def.shape {
        Shape::Rect => {
            let rect = match rect_argument(args) {
                Ok(rect) => rect,
                Err(message) => return fail(&request.id, message, json!({"kind": kind_name})),
            };
            if rect.right() > frame_width || rect.bottom() > frame_height {
                return fail(
                    &request.id,
                    "the mark does not fit inside the captured frame",
                    json!({
                        "kind": kind_name,
                        "frame": {"w": frame_width, "h": frame_height},
                        "mark": [rect.x, rect.y, rect.w, rect.h],
                    }),
                );
            }
            (json!([rect.x, rect.y, rect.w, rect.h]), Some(rect))
        }
        Shape::Point => {
            let (x, y) = match point_argument(args) {
                Ok(point) => point,
                Err(message) => return fail(&request.id, message, json!({"kind": kind_name})),
            };
            if x < 0 || y < 0 || x > frame_width || y > frame_height {
                return fail(
                    &request.id,
                    "the point does not fit inside the captured frame",
                    json!({
                        "kind": kind_name,
                        "frame": {"w": frame_width, "h": frame_height},
                        "point": [x, y],
                    }),
                );
            }
            (json!([x, y]), None)
        }
    };

    // Two independent measurements of the same block: they must agree, or the save is refused.
    if def.name == "bag_slot"
        && let Some(rect) = rect
        && let Some(grid) = get_path(&state.config, &["buy_sell", BAG_GRID]).and_then(four_numbers)
    {
        let grid_pitch_w = grid[2] as f64 / 8.0;
        let grid_pitch_h = grid[3] as f64 / 8.0;
        if (rect.w as f64 - grid_pitch_w).abs() > BAG_PITCH_TOLERANCE_PX
            || (rect.h as f64 - grid_pitch_h).abs() > BAG_PITCH_TOLERANCE_PX
        {
            return fail(
                &request.id,
                "the bag slot does not agree with the bag grid's pitch",
                json!({
                    "kind": kind_name,
                    "slot": [rect.x, rect.y, rect.w, rect.h],
                    "grid": grid,
                    "pitch": {"w": grid_pitch_w, "h": grid_pitch_h},
                }),
            );
        }
    }

    // The crop is produced BEFORE the first byte is written, so a decode/crop failure refuses the
    // whole save rather than leaving a mark whose reference picture is missing. `result_gem_area`
    // always samples the six-feature colour signature here too: that is the weaker empty reference
    // used when the crop is absent (`empty:false`), and it must come from the same pixels.
    let mut artifact = Value::Null;
    let mut signature = Value::Null;
    if let Some(name) = def.artifact {
        let Some(rect) = rect else {
            return fail(
                &request.id,
                "this kind's artifact needs a rect mark",
                json!({"kind": kind_name}),
            );
        };
        let Some(encoded) = frame.get("data").and_then(Value::as_str) else {
            return fail(
                &request.id,
                "the frame carries no PNG data to crop",
                json!({"frame_id": frame_id}),
            );
        };
        let png = match base64::engine::general_purpose::STANDARD.decode(encoded) {
            Ok(bytes) => bytes,
            Err(error) => {
                return fail(
                    &request.id,
                    format!("the frame's PNG data is not base64: {error}"),
                    json!({"frame_id": frame_id}),
                );
            }
        };
        let (width, height, rgba) = match seal_win::pure::decode_png(&png) {
            Ok(pixels) => pixels,
            Err(error) => {
                return fail(
                    &request.id,
                    format!("the frame's picture cannot be decoded: {error}"),
                    json!({"frame_id": frame_id}),
                );
            }
        };
        let (crop_w, crop_h, cropped) = match seal_win::pure::crop_rgba(width, height, &rgba, rect)
        {
            Ok(crop) => crop,
            Err(error) => {
                return fail(
                    &request.id,
                    format!("the mark cannot be cropped out of the frame: {error}"),
                    json!({"frame_id": frame_id}),
                );
            }
        };
        if def.name == "result_gem_area" {
            let composition = analyze_composition(
                &cropped,
                crop_w as usize,
                crop_h as usize,
                colored_gap_min(&state.config),
            );
            signature = json!({
                "mean_v": composition.mean_v,
                "mean_sat": composition.mean_sat,
                "colored_fraction": composition.colored_fraction,
                "mean_r": composition.mean_r,
                "mean_g": composition.mean_g,
                "mean_b": composition.mean_b,
            });
        }
        // `empty:false` means the box is NOT the empty reference: the area is saved, the signature is
        // sampled, and NO crop is written (a busy-box crop would poison the strong empty test).
        if empty != Some(false) {
            let out = match seal_win::pure::encode_png(crop_w, crop_h, &cropped) {
                Ok(png) => png,
                Err(error) => {
                    return io_fail(&request.id, format!("cannot encode the crop: {error}"));
                }
            };
            match state.store.write_artifact(name, &out) {
                Ok(path) => {
                    artifact = json!({
                        "file": name,
                        "path": path.display().to_string(),
                        "width": crop_w,
                        "height": crop_h,
                    });
                }
                Err(error) => return io_fail(&request.id, error.to_string()),
            }
        }
    }

    let derived = match def.name {
        "bag_grid" => match rect {
            Some(rect) => json!({
                "pitch": {"w": rect.w as f64 / 8.0, "h": rect.h as f64 / 8.0},
            }),
            None => json!({}),
        },
        _ => json!({}),
    };

    // The marks and the environment travel in ONE local.yaml write. `measured_at` is stamped on
    // EVERY calibration save (G07) — the mark was measured now even when the shell sent no display
    // environment — and the block as written is echoed back so the shell never re-derives it.
    let mark_for_write = mark.clone();
    let display = args.get("display").cloned();
    let measured_at = crate::events::utc_now_rfc3339();
    let mut calibration_echo = Value::Null;
    if let Err(error) = state.store.update_local(|local| {
        if let Some(target) = &target {
            set_instance_mark(local, def, target, mark_for_write);
        } else {
            set_path(local, def.path, mark_for_write);
        }
        if !signature.is_null() {
            set_path(local, &["gem", "empty_signature"], signature.clone());
        }
        if !local.get("calibration").is_some_and(Value::is_object) {
            set_path(local, &["calibration"], json!({}));
        }
        if let Some(display) = &display {
            let environment = json!({
                "dpi_scale": display.get("dpi_scale"),
                "monitor_dpi": display.get("monitor_dpi"),
                "screen": display.get("screen"),
            });
            for (key, value) in environment.as_object().into_iter().flatten() {
                if !value.is_null() {
                    set_path(local, &["calibration", key], value.clone());
                }
            }
            set_path(
                local,
                &["calibration", "client_size"],
                json!([frame_width, frame_height]),
            );
        }
        set_path(local, &["calibration", "measured_at"], json!(measured_at));
        calibration_echo = local.get("calibration").cloned().unwrap_or(Value::Null);
    }) {
        return io_fail(&request.id, error.to_string());
    }
    // Keep the in-memory merged view identical to the file for later commands/conflicts.
    if let Some(object) = state.config.as_object_mut() {
        object.insert("calibration".to_owned(), calibration_echo.clone());
    }

    // The merged view the shell reads stays the merge: apply the same mark to the in-memory config
    // rather than re-reading the files (no dictionary load is involved in a calibration mark).
    if let Some(target) = &target {
        // A named instance is mutated in place, so its siblings and the array order survive.
        set_instance_mark(&mut state.config, def, target, mark.clone());
    } else {
        let mut block = json!({});
        set_path(&mut block, &def.path[1..], mark.clone());
        if !signature.is_null() {
            set_path(&mut block, &["empty_signature"], signature.clone());
        }
        let mut sections = Map::new();
        sections.insert(def.path[0].to_owned(), block);
        state.config = crate::validate::effective_config(&state.config, &sections);
    }
    state.bump_revision();
    state.queue_config_changed(def.path[0]);

    let mut result = json!({
        "rev": state.rev,
        "kind": def.name,
        "mark": mark,
        "derived": derived,
        "artifact": artifact,
        "calibration": calibration_echo,
    });
    if let Some(target) = &target {
        result["target"] = json!(target.echo);
    }
    Response::ok(&request.id, result)
}

/// Run `calibration.fetch`: the stored mark and, when the kind has one, the stored crop.
pub fn fetch(state: &CoreState, request: &Request) -> Response {
    let Some(kind_name) = request.args.get("kind").and_then(Value::as_str) else {
        return fail(
            &request.id,
            "calibration.fetch needs kind",
            json!({"kinds": supported()}),
        );
    };
    if let Some(line) = withdrawn_line(kind_name) {
        return fail(
            &request.id,
            line,
            json!({"kind": kind_name, "kinds": supported()}),
        );
    }
    let Some(def) = kind(kind_name) else {
        return fail(
            &request.id,
            format!("unknown calibration kind {kind_name:?}"),
            json!({"kinds": supported()}),
        );
    };
    let target = match target_argument(def, &request.args, "fetch", &state.config) {
        Ok(target) => target,
        Err(message) => return fail(&request.id, message, json!({"kind": kind_name})),
    };
    let local = match state.store.load_local() {
        Ok(local) => local,
        Err(error) => return io_fail(&request.id, error.to_string()),
    };
    let mark = match &target {
        Some(target) => instance_mark(&local, def, target)
            .cloned()
            .unwrap_or(Value::Null),
        None => get_path(&local, def.path).cloned().unwrap_or(Value::Null),
    };
    let artifact = match def.artifact {
        Some(name) => {
            let path = state.store.path_of(name);
            if path.is_file() {
                match std::fs::read(&path) {
                    Ok(bytes) => json!({
                        "file": name,
                        "png_base64": base64::engine::general_purpose::STANDARD.encode(bytes),
                    }),
                    Err(error) => {
                        return io_fail(
                            &request.id,
                            format!("cannot read {}: {error}", path.display()),
                        );
                    }
                }
            } else {
                Value::Null
            }
        }
        None => Value::Null,
    };
    let mut result = json!({
        "kind": def.name,
        "mark": mark,
        "artifact": artifact,
        "rev": state.rev,
    });
    if let Some(target) = &target {
        result["target"] = json!(target.echo);
    }
    Response::ok(&request.id, result)
}

/// A stale `calibration.save`: refuse without writing, carrying the current rev and the block the
/// save would have touched (the `config.apply` conflict shape, G06).
fn calibration_conflict(state: &CoreState, id: &str, def: &KindDef) -> Response {
    let block = def.path[0];
    let mut sections = Map::new();
    sections.insert(
        block.to_owned(),
        state.config.get(block).cloned().unwrap_or(Value::Null),
    );
    Response::err(
        id,
        Error::new(
            "conflict",
            "calibration changed while you were editing",
            json!({"rev": state.rev, "sections": sections, "kind": def.name}),
        ),
    )
}

/// `calibration.kinds {}`: the core-owned catalog of every calibratable mark, so the shell stops
/// copying the table and a new kind appears without a shell edit (G08).
pub fn kinds(request: &Request) -> Response {
    Response::ok(&request.id, catalog())
}

pub(crate) fn catalog() -> Value {
    let kinds = KINDS
        .iter()
        .map(|def| {
            json!({
                "name": def.name,
                "shape": match def.shape {
                    Shape::Rect => "rect",
                    Shape::Point => "point",
                },
                "block": def.path[0],
                "path": def.path,
                "artifact": def.artifact,
                "targets": match def.targets {
                    Targets::None => "none",
                    Targets::PageTab => "page_tab",
                    Targets::PetRow => "pet_row",
                    Targets::GemGrade => "gem_grade",
                    Targets::GemResource => "gem_resource",
                },
                "row_key": def.row_key,
            })
        })
        .collect::<Vec<Value>>();
    json!({"kinds": kinds, "withdrawn": withdrawn_catalog()})
}

fn withdrawn_catalog() -> Value {
    let mut entries = Vec::new();
    for name in ["focus_point", "tuner_region"] {
        if let Some(reason) = withdrawn_line(name) {
            entries.push(json!({"name": name, "reason": reason}));
        }
    }
    Value::Array(entries)
}

/// `calibration.fetch_all {}`: every stored mark plus the environment block in ONE local read, so
/// opening the workspace is one request rather than ~35 fetches (G08). Artifacts stay per-kind (only
/// one kind has one today) and are read best-effort like `fetch` does.
pub fn fetch_all(state: &CoreState, request: &Request) -> Response {
    let local = match state.store.load_local() {
        Ok(local) => local,
        Err(error) => return io_fail(&request.id, error.to_string()),
    };
    let calibration = match local.get("calibration").cloned() {
        Some(value) if value.is_object() => value,
        _ => Value::Null,
    };
    let mut marks = Vec::new();
    for def in KINDS {
        let mut targets = target_echoes(def, &state.config);
        if targets.is_empty() {
            targets.push(String::new());
        }
        for echo in targets {
            let targeted = !echo.is_empty();
            let mark = if targeted {
                instance_value_for_echo(&local, def, &echo)
                    .cloned()
                    .unwrap_or(Value::Null)
            } else {
                get_path(&local, def.path).cloned().unwrap_or(Value::Null)
            };
            let artifact = match def.artifact {
                Some(name) => match artifact_json(state, name) {
                    Ok(value) => value,
                    Err(message) => return io_fail(&request.id, message),
                },
                None => Value::Null,
            };
            marks.push(json!({
                "kind": def.name,
                "target": if targeted { Value::String(echo) } else { Value::Null },
                "mark": mark,
                "artifact": artifact,
            }));
        }
    }
    Response::ok(
        &request.id,
        json!({
            "rev": state.rev,
            "calibration": calibration,
            "marks": marks,
            "withdrawn": withdrawn_catalog(),
        }),
    )
}

/// The instances a targeted kind carries in the CURRENT merged config, on the wire's own form.
fn target_echoes(def: &KindDef, config: &Value) -> Vec<String> {
    match def.targets {
        Targets::None => Vec::new(),
        Targets::PageTab => (1..=carried_len(config, def))
            .map(|index| format!("ITEM{index}"))
            .collect(),
        Targets::PetRow | Targets::GemResource => (1..=carried_len(config, def))
            .map(|index| index.to_string())
            .collect(),
        Targets::GemGrade => config
            .get("gem")
            .and_then(|gem| gem.get("grade_positions"))
            .and_then(Value::as_object)
            .map(|positions| positions.keys().cloned().collect())
            .unwrap_or_default(),
    }
}

/// One named instance's stored mark, addressed by its WIRE echo (`ITEM2`, `"3"`, `N`).
fn instance_value_for_echo<'a>(root: &'a Value, def: &KindDef, echo: &str) -> Option<&'a Value> {
    let container = instance_container(root, def)?;
    match def.targets {
        Targets::GemGrade => container.get(echo),
        Targets::PetRow => {
            let index = echo.parse::<usize>().ok()? - 1;
            let row = container.get(index)?;
            row.get(def.row_key?)
        }
        _ => {
            let index = echo
                .strip_prefix("ITEM")
                .unwrap_or(echo)
                .parse::<usize>()
                .ok()?
                - 1;
            container.get(index)
        }
    }
}

fn artifact_json(state: &CoreState, name: &str) -> Result<Value, String> {
    let path = state.store.path_of(name);
    if !path.is_file() {
        return Ok(Value::Null);
    }
    match std::fs::read(&path) {
        Ok(bytes) => Ok(json!({
            "file": name,
            "png_base64": base64::engine::general_purpose::STANDARD.encode(bytes),
        })),
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

/// `calibration.check {}`: one canonical Detect answer (G09). The rule is the domain's
/// [`seal_domain::calibration::differences`]; this function only measures the live side and shapes
/// the answer. A closed game is NOT an error — `live: null` and a note say why, and `matches` stays
/// null because nothing was compared.
pub fn check(state: &CoreState, request: &Request) -> Response {
    let local = match state.store.load_local() {
        Ok(local) => local,
        Err(error) => return io_fail(&request.id, error.to_string()),
    };
    let stored = match local.get("calibration").cloned() {
        Some(value) if value.is_object() => Some(value),
        _ => None,
    };
    let title = match crate::win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return crate::dispatch::win_error_response(&request.id, error),
    };
    let (live, note) = match crate::win::measure(&title) {
        Ok(value) => (value.get("display").cloned(), None),
        Err(crate::win::CommandError::NotFound { .. }) => (None, Some("game window not open")),
        Err(error) => return crate::dispatch::win_error_response(&request.id, error),
    };
    Response::ok(
        &request.id,
        check_answer(stored.as_ref(), live.as_ref(), note),
    )
}

/// The Detect answer, pure and testable: stored/live/differences/matches/note. `matches` is null
/// unless BOTH sides exist (nothing stored = nothing to match; game closed = nothing compared).
pub(crate) fn check_answer(
    stored: Option<&Value>,
    live: Option<&Value>,
    note: Option<&str>,
) -> Value {
    let stored_view = stored.filter(|value| value.is_object()).map(|value| {
        json!({
            "scale": value.get("dpi_scale").cloned().unwrap_or(Value::Null),
            "monitor_dpi": value.get("monitor_dpi").cloned().unwrap_or(Value::Null),
            "screen": value.get("screen").cloned().unwrap_or(Value::Null),
            "client_size": value.get("client_size").cloned().unwrap_or(Value::Null),
            "measured_at": value.get("measured_at").cloned().unwrap_or(Value::Null),
        })
    });
    let stored_info =
        stored.and_then(|value| serde_json::from_value::<CalibrationInfo>(value.clone()).ok());
    let live_info =
        live.and_then(|value| serde_json::from_value::<DisplayInfo>(value.clone()).ok());
    let differences = match (&stored_info, &live_info) {
        (Some(stored), Some(live)) => seal_domain::calibration::differences(stored, live),
        _ => Vec::new(),
    };
    let matches = match (&stored_info, &live_info) {
        (Some(_), Some(_)) => Some(differences.is_empty()),
        _ => None,
    };
    json!({
        "live": live,
        "stored": stored_view,
        "differences": differences,
        "matches": matches,
        "note": note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_distinct_name_and_a_known_shape() {
        let mut names: Vec<&str> = KINDS.iter().map(|kind| kind.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), KINDS.len(), "kind names are unique");
        for def in KINDS {
            assert!(!def.path.is_empty(), "{} has a local path", def.name);
            assert!(
                matches!(def.path[0], "buy_sell" | "gem" | "tuner" | "pet"),
                "{} lives in an owned local block",
                def.name
            );
        }
    }

    #[test]
    fn a_mark_is_written_at_its_local_path_and_read_back() {
        let mut local = json!({"tuner": {"ocr": {"row_height": 25}}});
        set_path(&mut local, &["tuner", "ocr", "region"], json!([1, 2, 3, 4]));
        assert_eq!(
            get_path(&local, &["tuner", "ocr", "region"]),
            Some(&json!([1, 2, 3, 4]))
        );
        assert_eq!(
            get_path(&local, &["tuner", "ocr", "row_height"]),
            Some(&json!(25)),
            "a sibling in the same block survives the write"
        );
    }

    #[test]
    fn the_bag_grid_derives_its_pitch() {
        let rect = Rect::new(0, 0, 800, 600);
        let derived = json!({"pitch": {"w": rect.w as f64 / 8.0, "h": rect.h as f64 / 8.0}});
        assert_eq!(derived["pitch"]["w"], json!(100.0));
        assert_eq!(derived["pitch"]["h"], json!(75.0));
    }

    /// `CalibrationCheck` input on 1920×1080 at 1.5×: physical client 1200×600, logical 800×400.
    fn live_display() -> Value {
        let display = DisplayInfo::from_rects(
            Rect::new(10, 20, 1200, 600),
            Rect::new(10, 20, 1200, 600),
            Rect::new(10, 20, 800, 400),
            seal_domain::geometry::Size::new(1920, 1080),
        );
        serde_json::to_value(display).expect("DisplayInfo serialises")
    }

    #[test]
    fn check_matches_when_the_stored_environment_matches_the_live_one() {
        let live = live_display();
        let stored = json!({
            "dpi_scale": 1.5,
            "monitor_dpi": 144,
            "screen": [1920, 1080],
            "client_size": [1200, 600],
            "measured_at": "2026-10-30T12:00:00Z",
        });
        let answer = check_answer(Some(&stored), Some(&live), None);
        assert_eq!(answer["matches"], true);
        assert_eq!(answer["differences"], json!([]));
        assert_eq!(answer["live"]["scale"], 1.5);
        assert_eq!(answer["stored"]["scale"], 1.5);
        assert_eq!(answer["stored"]["measured_at"], "2026-10-30T12:00:00Z");
    }

    #[test]
    fn check_lists_every_moved_field_in_v2s_order() {
        let live = live_display();
        let stored = json!({
            "dpi_scale": 1.25,
            "screen": [2560, 1440],
            "client_size": [1000, 600],
        });
        let answer = check_answer(Some(&stored), Some(&live), None);
        assert_eq!(answer["matches"], false);
        let differences = answer["differences"].as_array().expect("an array");
        assert_eq!(differences.len(), 3, "{differences:?}");
        assert!(differences[0].as_str().unwrap().starts_with("scale "));
        assert!(differences[1].as_str().unwrap().starts_with("client "));
        assert!(differences[2].as_str().unwrap().starts_with("screen "));
    }

    #[test]
    fn check_is_not_an_error_without_a_stored_calibration_or_a_live_one() {
        let live = live_display();
        let nothing_stored = check_answer(None, Some(&live), None);
        assert!(nothing_stored["stored"].is_null());
        assert!(nothing_stored["matches"].is_null());
        assert_eq!(nothing_stored["differences"], json!([]));

        let game_closed = check_answer(
            Some(&json!({"dpi_scale": 1.5})),
            None,
            Some("game window not open"),
        );
        assert!(game_closed["live"].is_null());
        assert!(game_closed["matches"].is_null());
        assert_eq!(game_closed["note"], "game window not open");
    }

    #[test]
    fn the_catalog_names_every_kind_and_every_withdrawal() {
        let catalog = catalog();
        let kinds = catalog["kinds"].as_array().expect("kinds array");
        assert_eq!(kinds.len(), KINDS.len());
        assert!(kinds.iter().any(|kind| kind["name"] == "bag_grid"));
        assert!(kinds.iter().any(|kind| kind["targets"] == "pet_row"));
        let withdrawn = catalog["withdrawn"].as_array().expect("withdrawn array");
        assert!(
            withdrawn
                .iter()
                .any(|entry| entry["name"] == "tuner_region"),
            "{withdrawn:?}"
        );
    }
}

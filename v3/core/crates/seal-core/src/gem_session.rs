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

use seal_proto::{Error, Request, Response};
use serde_json::{Value, json};

use crate::dispatch::CoreState;

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

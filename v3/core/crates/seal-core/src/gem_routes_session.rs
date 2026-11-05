//! G17: the gem calibrator's live route/cycle tests.
//!
//! Both commands are v2's "does the move set actually land?" checks, and both execute REAL clicks
//! only after every prerequisite has been resolved:
//!
//! - `calibration.gem.test_route` clicks the SOURCE, performs the route (raw `D dx dy` for the tuned
//!   set or a closed-loop placement for the arduino set), then clicks the TARGET.
//! - `calibration.gem.cycle {confirm:true}` runs v2's 21-step N/G/DG cycle: N and G combine twice with
//!   the composer's deregister+register pair and cleared resource slots between, then DG combines once.
//!
//! Every calibrated point is resolved BEFORE the first click, so an uncalibrated point refuses the
//! whole action instead of leaving the game in a half-run state. Both take the input lease, use the
//! closed-loop pointer and log a diagnostic line.

use std::collections::BTreeMap;

use seal_domain::geometry::{DisplayInfo, Point};
use seal_domain::tools::gem::{GemRoutes, MoveMode};
use seal_proto::{Error, Request, Response};
use seal_tools::runner::Effects;
use serde_json::{Value, json};

use crate::board::{self, BoardFailure, Settings};
use crate::dispatch::CoreState;
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

fn grade_positions(config: &Value) -> BTreeMap<String, Vec<i32>> {
    config
        .pointer("/gem/grade_positions")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(name, value)| Some((name.clone(), ints(Some(value))?)))
                .collect()
        })
        .unwrap_or_default()
}

fn resources(config: &Value) -> Vec<Vec<i32>> {
    config
        .pointer("/gem/resource_gems")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| ints(Some(value)))
                .collect()
        })
        .unwrap_or_default()
}

fn point(config: &Value, name: &str) -> Option<Point> {
    GemRoutes::resolve(&grade_positions(config), &resources(config), name)
}

/// The tuned movement for a pair, mirroring the composer's routing (the `slot3_*` keys are the one
/// place the composer's route ids and `gem.movements` field names differ).
fn tuned_movement(config: &Value, from: &str, to: &str) -> Option<(i32, i32)> {
    let get = |name: &str| {
        let values = ints(config.pointer(&format!("/gem/movements/{name}")))?;
        (values.len() >= 2).then_some((values[0], values[1]))
    };
    match (from, to) {
        ("Register", "Combine") => get("register_combine"),
        ("Combine", "Register") => get("combine_register"),
        ("Register", "Resource1") => get("register_slot1"),
        ("Resource1", "Resource2") => get("slot1_slot2"),
        ("Resource2", "Resource3") => get("slot2_slot3"),
        ("Resource3", "N") => get("slot3_to_n"),
        ("Resource3", "G") => get("slot3_to_g"),
        ("Resource3", "DG") => get("slot3_to_dg"),
        ("N" | "G" | "DG", "Register") => config
            .pointer("/gem/movements/radio_to_register")
            .and_then(Value::as_object)
            .and_then(|map| map.get(from))
            .and_then(|value| ints(Some(value)))
            .and_then(|values| (values.len() >= 2).then_some((values[0], values[1]))),
        _ => None,
    }
}

/// The production effects type and the pair a command needs (effects + the measured display).
type RealEffects = crate::effects::BoardEffects<
    seal_board::serial::SerialTransport,
    crate::effects::WinCursor,
    seal_board::clock::RealClock,
>;
type OpenedEffects = (RealEffects, DisplayInfo);

fn open_effects(config: &Value, id: &str) -> Result<OpenedEffects, Box<Response>> {
    let display = game_display(config)
        .map_err(|error| Box::new(crate::dispatch::win_error_response(id, error)))?;
    let settings = Settings::from_config(config);
    let (_port, transport) = board::open_board(&settings).map_err(|error: BoardFailure| {
        Box::new(crate::dispatch::board_error_response(id, error))
    })?;
    Ok((system_effects(transport, display), display))
}

/// `calibration.gem.test_route {"move":"tuned"|"arduino","from":"N","to":"Register","tuned":{...}?}`.
#[allow(
    clippy::too_many_lines,
    reason = "the route test keeps its ordered click sequence and refusal paths together"
)]
pub fn test_route(state: &CoreState, request: &Request) -> Response {
    let Some(move_mode) = request.args.get("move").and_then(Value::as_str) else {
        return bad(
            &request.id,
            "calibration.gem.test_route needs \"move\": \"tuned\" or \"arduino\"",
            "move",
        );
    };
    let mode = match move_mode {
        "tuned" => MoveMode::Tuned,
        "arduino" => MoveMode::Arduino,
        other => {
            return bad(
                &request.id,
                format!("unknown gem move set {other:?}; use \"tuned\" or \"arduino\""),
                "move",
            );
        }
    };
    let Some(from_name) = request.args.get("from").and_then(Value::as_str) else {
        return bad(
            &request.id,
            "calibration.gem.test_route needs \"from\"",
            "from",
        );
    };
    let Some(to_name) = request.args.get("to").and_then(Value::as_str) else {
        return bad(&request.id, "calibration.gem.test_route needs \"to\"", "to");
    };
    let config = &state.config;
    let Some(from) = point(config, from_name) else {
        return bad(
            &request.id,
            format!("Start \"{from_name}\" isn't calibrated yet."),
            "from",
        );
    };
    let to = match mode {
        MoveMode::Tuned => {
            // The TARGET is clicked where the cursor lands; the point itself is not needed, but
            // v2 still checked the target was calibrated (`ResolveGemPoint`), which catches a wrong
            // pair before anything moves.
            if point(config, to_name).is_none() {
                return bad(
                    &request.id,
                    format!("Target \"{to_name}\" isn't calibrated yet."),
                    "to",
                );
            }
            None
        }
        MoveMode::Arduino => match point(config, to_name) {
            Some(target) => Some(target),
            None => {
                return bad(
                    &request.id,
                    format!("Target \"{to_name}\" isn't calibrated yet."),
                    "to",
                );
            }
        },
    };
    let tuned = match mode {
        MoveMode::Tuned => {
            let explicit = request
                .args
                .get("tuned")
                .and_then(Value::as_object)
                .and_then(|object| {
                    Some((
                        i32::try_from(object.get("dx")?.as_i64()?).ok()?,
                        i32::try_from(object.get("dy")?.as_i64()?).ok()?,
                    ))
                });
            let paired = tuned_movement(config, from_name, to_name);
            match explicit.or(paired) {
                Some((dx, dy)) => Some((dx, dy)),
                None => {
                    return bad(
                        &request.id,
                        format!(
                            "No raw movement for \"{from_name}\" -> \"{to_name}\". Pick a composer \
                             pair or pass tuned {{\"dx\":..,\"dy\":..}}."
                        ),
                        "tuned",
                    );
                }
            }
        }
        MoveMode::Arduino => None,
    };
    if let Some(response) = check_input_lease(state, &request.id, request.cmd.as_str()) {
        return response;
    }
    let (mut effects, _display) = match open_effects(config, &request.id) {
        Ok(opened) => opened,
        Err(response) => return *response,
    };
    let mut steps = Vec::new();
    if let Err(error) = effects.click_at(from) {
        return Response::err(
            &request.id,
            Error::new("board_unavailable", error.message(), json!({})),
        );
    }
    steps.push(
        json!({"action": "click", "point": from_name, "placement": {"x": from.x, "y": from.y}}),
    );
    match mode {
        MoveMode::Tuned => {
            let Some((dx, dy)) = tuned else {
                return Response::err(
                    &request.id,
                    Error::new("internal", "the tuned move was validated above", json!({})),
                );
            };
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Err(error) = effects.move_by(dx, dy) {
                return Response::err(
                    &request.id,
                    Error::new("board_unavailable", error.message(), json!({})),
                );
            }
            steps.push(json!({"action": "move", "dx": dx, "dy": dy}));
            std::thread::sleep(std::time::Duration::from_millis(300));
            if let Err(error) = effects.click_here() {
                return Response::err(
                    &request.id,
                    Error::new("board_unavailable", error.message(), json!({})),
                );
            }
            steps.push(json!({"action": "click", "point": to_name}));
        }
        MoveMode::Arduino => {
            let Some(target) = to else {
                return Response::err(
                    &request.id,
                    Error::new(
                        "internal",
                        "the target point was validated above",
                        json!({}),
                    ),
                );
            };
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Err(error) = effects.place_at(target) {
                return Response::err(
                    &request.id,
                    Error::new("board_unavailable", error.message(), json!({})),
                );
            }
            steps.push(json!({"action": "place", "point": to_name, "placement": {"x": target.x, "y": target.y}}));
            std::thread::sleep(std::time::Duration::from_millis(300));
            if let Err(error) = effects.click_here() {
                return Response::err(
                    &request.id,
                    Error::new("board_unavailable", error.message(), json!({})),
                );
            }
            steps.push(json!({"action": "click", "point": to_name}));
        }
    }
    let line = match mode {
        MoveMode::Tuned => {
            let Some((dx, dy)) = tuned else {
                return Response::err(
                    &request.id,
                    Error::new("internal", "the tuned move was validated above", json!({})),
                );
            };
            format!(
                "Test move: click \"{from_name}\" then D {dx} {dy} then click \"{to_name}\". Does it land?"
            )
        }
        MoveMode::Arduino => format!(
            "New move: placed on \"{from_name}\", clicked, then placed on \"{to_name}\", clicked. Does it land?"
        ),
    };
    Response::ok(
        &request.id,
        json!({"steps": steps, "line": line, "move": move_mode}),
    )
}

/// v2's 21-step full cycle (the arduino move set): N and G combine twice with the composer's
/// deregister+register pair and resource clears between, then DG combines once.
pub const CYCLE_STEPS: [(&str, &str); 21] = [
    ("click", "N"),
    ("click", "Register"),
    ("click", "Combine"),
    ("click", "Register"),
    ("click", "Register"),
    ("click", "Combine"),
    ("rclick", "Resource1"),
    ("rclick", "Resource2"),
    ("rclick", "Resource3"),
    ("click", "G"),
    ("click", "Register"),
    ("click", "Combine"),
    ("click", "Register"),
    ("click", "Register"),
    ("click", "Combine"),
    ("rclick", "Resource1"),
    ("rclick", "Resource2"),
    ("rclick", "Resource3"),
    ("click", "DG"),
    ("click", "Register"),
    ("click", "Combine"),
];

/// `calibration.gem.cycle {"confirm":true}`: every point pre-resolved, then the 21 steps.
#[must_use]
pub fn cycle(state: &CoreState, request: &Request) -> Response {
    if request.args.get("confirm") != Some(&Value::Bool(true)) {
        return bad(
            &request.id,
            "calibration.gem.cycle moves the game through all 21 clicks; send {\"confirm\":true} \
             to run it",
            "confirm",
        );
    }
    let config = &state.config;
    let mut targets = Vec::with_capacity(CYCLE_STEPS.len());
    for (action, name) in CYCLE_STEPS {
        let Some(at) = point(config, name) else {
            return bad(
                &request.id,
                format!("Full cycle: \"{name}\" isn't calibrated yet - nothing was clicked."),
                name,
            );
        };
        targets.push((action, name, at));
    }
    if let Some(response) = check_input_lease(state, &request.id, request.cmd.as_str()) {
        return response;
    }
    // TOOL-F10: hold the game for the whole 21-step sequence. A plain lease check can pass while
    // the resident feeder is merely armed; mid-cycle it would request and drive the same mouse.
    let _lease = match state.supervisor.as_ref() {
        Some(supervisor) => match supervisor.begin_calibration() {
            Ok(lease) => Some(lease),
            Err(line) => {
                return Response::err(
                    &request.id,
                    Error::new(
                        "tool_busy",
                        &line,
                        json!({"line": line, "command": request.cmd}),
                    ),
                );
            }
        },
        None => None,
    };
    let (mut effects, _display) = match open_effects(config, &request.id) {
        Ok(opened) => opened,
        Err(response) => return *response,
    };
    let mut steps = Vec::with_capacity(targets.len());
    for (index, (action, name, at)) in targets.iter().enumerate() {
        let result = if *action == "click" {
            effects.click_at(*at)
        } else {
            effects
                .place_at(*at)
                .and_then(|()| effects.right_click_here())
        };
        if let Err(error) = result {
            return Response::err(
                &request.id,
                Error::new(
                    "board_unavailable",
                    &format!(
                        "Full cycle stopped at {}/{} ({action} \"{name}\"): {}",
                        index + 1,
                        targets.len(),
                        error.message()
                    ),
                    json!({}),
                ),
            );
        }
        steps.push(json!({"index": index + 1, "action": action, "point": name}));
        std::thread::sleep(std::time::Duration::from_millis(if *name == "Combine" {
            800
        } else {
            500
        }));
    }
    state
        .logs
        .arduino_debug_line(&format!("test-cycle-arduino done steps={}", targets.len()));
    Response::ok(
        &request.id,
        json!({
            "steps": steps,
            "done": true,
            "log": format!("arduino_debug.txt: cycle done ({} steps)", targets.len()),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Value {
        json!({
            "gem": {
                "grade_positions": {"N": [10, 20], "G": [30, 40], "DG": [50, 60], "Register": [70, 80], "Combine": [90, 100]},
                "resource_gems": [[110, 120], [130, 140], [150, 160]],
                "movements": {
                    "radio_to_register": {"N": [1, 2]},
                    "register_combine": [3, 4]
                }
            }
        })
    }

    #[test]
    fn the_cycle_is_v2s_21_step_list() {
        assert_eq!(CYCLE_STEPS.len(), 21);
        assert_eq!(CYCLE_STEPS[0], ("click", "N"));
        assert_eq!(CYCLE_STEPS[20], ("click", "Combine"));
        assert_eq!(
            CYCLE_STEPS
                .iter()
                .filter(|(action, _)| *action == "rclick")
                .count(),
            6
        );
    }

    #[test]
    fn a_missing_confirm_is_refused_before_any_click() {
        let state = CoreState::new(std::env::temp_dir());
        let response = cycle(
            &state,
            &Request::new("1", "calibration.gem.cycle", json!({})),
        );
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().detail["field"], "confirm");
    }

    #[test]
    fn the_tuned_pair_uses_the_composer_routing() {
        let config = config();
        assert_eq!(tuned_movement(&config, "N", "Register"), Some((1, 2)));
        assert_eq!(tuned_movement(&config, "Register", "Combine"), Some((3, 4)));
        assert_eq!(tuned_movement(&config, "Combine", "Register"), None);
    }
}

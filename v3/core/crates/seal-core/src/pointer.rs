//! Calibration pointer actions: `pointer.place` and `pointer.click`.
//!
//! Both commands take a CLIENT-RELATIVE PHYSICAL point (the calibration coordinate contract) and
//! place the cursor with the SAME closed-loop board placement every tool uses
//! ([`BoardEffects::place_at`]/[`BoardEffects::click_at`]); the core never calls `SetCursorPos`.
//! This module owns only the protocol arguments and the production window/board wiring; the live
//! input lease is the supervisor's (`Supervisor::input_lease`), and session-wide leases/queueing
//! belong to the later calibration session increments.

use seal_domain::geometry::{DisplayInfo, Point};
use serde_json::Value;

use crate::board::{self, BoardFailure, Settings};
use crate::effects::{BoardEffects, system_effects};
use crate::win::{self, CommandError};

/// Why a pointer action could not run, in the terms the dispatcher maps to protocol codes.
#[derive(Debug)]
pub enum PointerFailure {
    /// The shared window/capture refusal: missing title, not found, not foreground, measure failure.
    Window(CommandError),
    /// The board could not be opened.
    Board(BoardFailure),
    /// The board/cursor placement or click failed after the port opened.
    Effects(String),
}

/// Parse the flat `{"x":<int>,"y":<int>}` pointer arguments. Both are required integers >= 0; a
/// malformed one names its own field (the dispatcher reports it as `bad_request`).
///
/// # Errors
///
/// Returns the player-facing line and field name when `x` or `y` is missing or not a non-negative integer.
pub fn point_arguments(cmd: &str, args: &Value) -> Result<Point, (String, String)> {
    let coordinate = |name: &str| -> Result<i32, (String, String)> {
        let Some(value) = args.get(name).and_then(Value::as_i64) else {
            return Err((
                format!("{cmd} needs an integer \"{name}\" (client-relative physical pixels)"),
                name.to_owned(),
            ));
        };
        let Ok(value) = i32::try_from(value) else {
            return Err((
                format!("{cmd}'s \"{name}\" must be between 0 and {}", i32::MAX),
                name.to_owned(),
            ));
        };
        if value < 0 {
            return Err((
                format!("{cmd}'s \"{name}\" must be at least 0"),
                name.to_owned(),
            ));
        }
        Ok(value)
    };
    Ok(Point::new(coordinate("x")?, coordinate("y")?))
}

/// The game's display measurement after the same window/foreground checks a tool read uses. A
/// missing window is `NotFound`; a covered game is `NotForeground`; the measure failure keeps its
/// typed Win32 shape. `DisplayInfo::cursor_target` is applied later by `BoardEffects`, so this is
/// the one place the physical/logical conversion happens.
///
/// # Errors
///
/// Returns the command error when the configured title is empty or the window cannot be found.
pub fn game_display(config: &Value) -> Result<DisplayInfo, CommandError> {
    let title = win::configured_title(config)?;
    let hwnd = seal_win::window::find_window(&title).ok_or(CommandError::NotFound {
        title: title.clone(),
    })?;
    if !seal_win::window::is_foreground(hwnd) {
        return Err(CommandError::NotForeground);
    }
    seal_win::window::measure(hwnd).map_err(CommandError::from)
}

/// Perform the pointer action through the existing effects placement. `click` only adds the bare
/// left click after a successful placement; a failed placement never clicks.
///
/// # Errors
///
/// Returns the line when the placement or the click write fails.
pub fn perform<T, C, K>(
    effects: &mut BoardEffects<T, C, K>,
    point: Point,
    click: bool,
) -> Result<(), String>
where
    T: seal_board::transport::Transport,
    C: seal_board::placement::Cursor,
    K: seal_board::clock::Clock,
{
    let result = if click {
        effects.click_at(point)
    } else {
        effects.place_at(point)
    };
    result.map_err(|error| error.message().to_owned())
}

/// The production path: window/foreground/measure, open the board, then place (and optionally click)
/// through the closed loop. The pointer command's input lease is checked by the dispatcher before
/// this is called.
///
/// # Errors
///
/// Returns the pointer failure when the window or the board cannot be opened, or the action fails.
#[cfg(windows)]
pub fn system_perform(config: &Value, point: Point, click: bool) -> Result<(), PointerFailure> {
    let display = game_display(config).map_err(PointerFailure::Window)?;
    let settings = Settings::from_config(config);
    let (_port, transport) = board::open_board(&settings).map_err(PointerFailure::Board)?;
    let mut effects = system_effects(transport, display);
    perform(&mut effects, point, click).map_err(PointerFailure::Effects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use seal_board::clock::Clock;
    use seal_board::placement::Cursor;
    use seal_board::transport::FakeTransport;
    use seal_domain::geometry::{Rect, Size};
    use serde_json::json;

    struct StepClock;

    impl Clock for StepClock {
        fn sleep_seconds(&self, _seconds: f64) {}
    }

    struct ScriptedCursor {
        positions: Vec<Option<(i32, i32)>>,
        index: usize,
    }

    impl ScriptedCursor {
        fn new(positions: Vec<Option<(i32, i32)>>) -> Self {
            Self {
                positions,
                index: 0,
            }
        }
    }

    impl Cursor for ScriptedCursor {
        fn position(&mut self) -> Option<(i32, i32)> {
            let position = self
                .positions
                .get(self.index.min(self.positions.len().saturating_sub(1)))
                .copied()
                .flatten();
            self.index += 1;
            position
        }
    }

    fn display() -> DisplayInfo {
        DisplayInfo::from_rects(
            Rect::new(100, 40, 820, 640),
            Rect::new(100, 50, 800, 600),
            Rect::new(100, 50, 800, 600),
            Size::new(1920, 1080),
        )
    }

    fn effects(
        transport: FakeTransport,
        cursor: ScriptedCursor,
    ) -> BoardEffects<FakeTransport, ScriptedCursor, StepClock> {
        BoardEffects::new(transport, cursor, StepClock, display())
    }

    #[test]
    fn pointer_coordinates_are_flat_non_negative_integers_and_name_their_field() {
        assert_eq!(
            point_arguments("pointer.place", &json!({"x": 10, "y": 20})).expect("valid"),
            Point::new(10, 20)
        );
        for (args, field) in [
            (json!({"y": 20}), "x"),
            (json!({"x": 10}), "y"),
            (json!({"x": -1, "y": 20}), "x"),
            (json!({"x": 10, "y": -1}), "y"),
            (json!({"x": "10", "y": 20}), "x"),
            (json!({"x": 1.5, "y": 20}), "x"),
        ] {
            let error = point_arguments("pointer.click", &args).expect_err("bad shape");
            assert_eq!(error.1, field, "{args}");
            assert!(error.0.contains(field), "{args}: {}", error.0);
        }
    }

    #[test]
    fn place_and_click_use_the_same_closed_loop_conversion_as_the_tools() {
        let mut place = effects(
            FakeTransport::new(),
            ScriptedCursor::new(vec![Some((0, 0)), Some((110, 70))]),
        );
        perform(&mut place, Point::new(10, 20), false).expect("places");
        assert_eq!(place.transport().writes(), vec!["D 110 70\n"]);

        let mut click = effects(
            FakeTransport::new(),
            ScriptedCursor::new(vec![Some((0, 0)), Some((110, 70))]),
        );
        perform(&mut click, Point::new(10, 20), true).expect("places then clicks");
        assert_eq!(click.transport().writes(), vec!["D 110 70\n", "C\n"]);
    }

    #[test]
    fn a_missing_game_window_is_the_shared_not_found_refusal() {
        let error = game_display(&json!({
            "window": {"title": "SEAL-TEST-NO-SUCH-WINDOW-7F3A"}
        }))
        .expect_err("no such window");
        assert!(matches!(error, CommandError::NotFound { .. }), "{error:?}");
    }
}

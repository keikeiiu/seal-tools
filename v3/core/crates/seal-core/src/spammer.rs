//! The first live tool: the skill spammer's runner adapter.
//!
//! `seal-domain` owns the decision half (`SpammerPlan`/`SpammerRun`); this module is the thin join
//! between it and `seal-tools`' runner. It resolves the ACTIVE preset out of the merged config,
//! refuses keys the board cannot send (fail-closed AT START rather than warning every tick — a
//! recorded divergence from v2, see `v3/core/README.md`), and turns each tick's presses into
//! `Decision`s. Nothing here chooses a key (the domain's) and nothing here performs an effect (the
//! runner's, through `Effects`).
//!
//! The cadence is v2's `SkillSpammer.Run` tick: `SleepCheck(0.02)`, so one tick is 20 ms.

use std::collections::VecDeque;
use std::time::Instant;

use seal_domain::config::{SpammerCombo, SpammerStartup};
use seal_domain::decision::{Decision, StopReason};
use seal_domain::tools::spammer::{
    SpammerPlan, SpammerRotation, SpammerRun, plan_or_stop, unsupported_key_message,
};
use seal_tools::runner::{Tool, ToolStatus};
use seal_tools::tool::{ToolDetail, ToolId};
use serde_json::Value;

/// v2's `SkillSpammer.Run` tick (`SleepCheck(0.02)`).
pub const TICK_SECONDS: f64 = 0.02;

/// Where the run's monotonic time comes from. The real one is a `Instant`; tests step it.
pub trait Now {
    fn seconds(&self) -> f64;
}

/// The production clock: seconds since the run's object was made.
pub struct RealNow(Instant);

impl RealNow {
    #[must_use]
    pub fn start() -> Self {
        Self(Instant::now())
    }
}

impl Now for RealNow {
    fn seconds(&self) -> f64 {
        self.0.elapsed().as_secs_f64()
    }
}

/// The active rotation out of the merged config: `spammer.presets[spammer.active]` in FILE ORDER
/// (the loader preserves map order, which is the order the player sees), plus that preset's
/// priority/combos/startup. `presets_configured` and `active` travel separately because the domain's
/// refusal line differs between "no presets at all" and "that name is not here".
///
/// `problems` are values the config has but the run cannot use (TOOL-F12): a cooldown that is not a
/// number was silently treated as `0.0`, which makes the key due on every tick — about fifty presses a
/// second. They are player lines; [`plan`] refuses to start while one is present.
#[derive(Debug, Clone)]
pub struct Rotation {
    pub presets_configured: bool,
    pub active: String,
    pub rotation: Option<SpammerRotation>,
    pub problems: Vec<String>,
}

/// Resolve the active preset, keeping the values the config could not use as refusal lines.
pub fn rotation(config: &Value) -> Rotation {
    let mut problems = Vec::new();
    let spammer = config.get("spammer");
    let presets = spammer
        .and_then(|block| block.get("presets"))
        .and_then(Value::as_object);
    let presets_configured = presets.is_some_and(|presets| !presets.is_empty());
    let active = spammer
        .and_then(|block| block.get("active"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned();
    let Some(body) = presets
        .and_then(|presets| presets.get(&active))
        .and_then(Value::as_object)
    else {
        return Rotation {
            presets_configured,
            active,
            rotation: None,
            problems,
        };
    };
    if body.is_empty() {
        return Rotation {
            presets_configured,
            active,
            rotation: None,
            problems,
        };
    }

    let mut keys: Vec<(String, f64)> = Vec::with_capacity(body.len());
    for (key, cooldown) in body {
        match cooldown.as_f64() {
            Some(seconds) => keys.push((key.clone(), seconds)),
            None => problems.push(format!(
                "Spammer key '{key}' has no cooldown — give it a number of seconds on the Spammer tab \
                 (a value that is not a number fires the key on every tick)."
            )),
        }
    }
    let branch = |name: &str| {
        spammer
            .and_then(|block| block.get(name))
            .and_then(Value::as_object)
            .and_then(|map| map.get(&active))
            .cloned()
    };
    let priority: Vec<String> = branch("priority")
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let combos: Vec<SpammerCombo> = branch("combos")
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let startup: SpammerStartup = branch("startup")
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();

    Rotation {
        presets_configured,
        active: active.clone(),
        rotation: Some(SpammerRotation {
            keys,
            priority,
            combos,
            startup,
        }),
        problems,
    }
}

/// The run's plan, or the refusal line v2's `Run()` shows.
///
/// TOOL-F12: a cooldown the run cannot honour (not a number, zero, negative, or shorter than one
/// tick) refuses the START by name instead of pressing that key fifty times a second.
///
/// # Errors
///
/// Returns the refusal line when no preset is configured, the active preset is missing, or a
/// cooldown cannot be used.
pub fn plan(config: &Value) -> Result<SpammerPlan, String> {
    let read = rotation(config);
    if let Some(problem) = read.problems.first() {
        return Err(problem.clone());
    }
    let plan = plan_or_stop(read.presets_configured, &read.active, read.rotation)
        .map_err(|reason| reason.message)?;
    match plan.cooldown_problems().first() {
        Some(problem) => Err(problem.clone()),
        None => Ok(plan),
    }
}

/// The keys the firmware cannot send, one v2 warning line each. The whole rotation is checked at
/// START: a run that would silently drop half its rotation is worse than a refused start, and the
/// player can fix the preset before anything is pressed.
#[must_use]
pub fn unsupported_keys(config: &Value) -> Vec<String> {
    let Some(rotation) = rotation(config).rotation else {
        return Vec::new();
    };
    rotation
        .keys
        .iter()
        .filter_map(|(key, _)| unsupported_key_message(key))
        .collect()
}

/// The spammer as the runner sees it: one `decide` per step, presses queued in tick order.
pub struct SpammerTool {
    run: SpammerRun,
    now: Box<dyn Now + Send>,
    queue: VecDeque<Decision>,
    /// The card's `Current`: the last key the loop pressed (v2 `state.Current`).
    last_key: Option<String>,
    /// The card's `Cycle`: presses the ticks produced, v2's accepted-press count. The runner performs
    /// every queued press and a refused effect ends the run, so a tick's presses are the sent ones.
    presses: i32,
}

impl SpammerTool {
    #[must_use]
    pub fn new(plan: &SpammerPlan, now: Box<dyn Now + Send>) -> Self {
        let started = now.seconds();
        Self {
            run: SpammerRun::start(plan, started),
            now,
            queue: VecDeque::new(),
            last_key: None,
            presses: 0,
        }
    }

    /// The card's `detail`: v2's `Cycle` + `Current` (`SkillSpammer.cs:308-339`), in `FormatStatus`'s
    /// order. Before the first press nothing has happened yet, so the list is empty; the lead-in shows
    /// through [`Self::status`]'s message and countdown instead.
    fn card_detail(&self) -> Vec<ToolDetail> {
        let mut detail = Vec::new();
        if self.presses > 0 {
            detail.push(ToolDetail::labeled("Cycle", self.presses.to_string()));
        }
        if let Some(key) = &self.last_key {
            detail.push(ToolDetail::labeled("Current", key.clone()));
        }
        detail
    }
}

impl Tool for SpammerTool {
    fn id(&self) -> ToolId {
        ToolId::Spammer
    }

    fn status(&self) -> ToolStatus {
        // v2 reported the lead-in through its schedule line + `NextActionAt`; v3's equivalents are the
        // message and `countdown_s` (the card counts it down). Wording from `SkillSpammer.cs:121`.
        if let Some(remaining) = self.run.lead_in_remaining(self.now.seconds()) {
            return ToolStatus {
                message: "lead-in — starting shortly".to_owned(),
                // WHOLE seconds: the loop ticks every 20 ms, and a continuously changing countdown
                // would republish `state` on every tick. The card counts the second down itself.
                countdown_s: Some(remaining.ceil().max(0.0)),
                rows: Vec::new(),
                detail: Vec::new(),
            };
        }
        ToolStatus {
            message: String::new(),
            countdown_s: None,
            rows: Vec::new(),
            detail: self.card_detail(),
        }
    }

    fn decide(&mut self) -> Decision {
        if let Some(decision) = self.queue.pop_front() {
            return decision;
        }

        let tick = self.run.tick(self.now.seconds());
        if let Some(press) = tick.presses.last() {
            self.last_key = Some(press.key.clone());
        }
        self.presses = self
            .presses
            .saturating_add(i32::try_from(tick.presses.len()).unwrap_or(i32::MAX));
        for press in &tick.presses {
            let Some(board_press) = press.board_press() else {
                // Unreachable when the start validated the rotation; kept as a terminal answer
                // rather than a panic if a config changes under a running session.
                return Decision::Stop(StopReason::error(
                    press
                        .unsupported_message()
                        .unwrap_or_else(|| "a spammer key cannot be sent".to_owned()),
                ));
            };
            self.queue.push_back(if board_press.fast {
                Decision::KeyFast(board_press.key)
            } else {
                Decision::Key(board_press.key)
            });
        }

        self.queue
            .pop_front()
            .unwrap_or(Decision::Wait(TICK_SECONDS))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::{Arc, Mutex};

    use seal_domain::decision::Key;

    use super::*;

    /// A clock the test steps by hand.
    struct StepNow(Cell<f64>);

    impl StepNow {
        fn new(seconds: f64) -> Self {
            Self(Cell::new(seconds))
        }
    }

    impl Now for StepNow {
        fn seconds(&self) -> f64 {
            self.0.get()
        }
    }

    /// A clock the test and the tool share, so the card can be read before and after the lead-in.
    struct SharedNow(Arc<Mutex<f64>>);

    impl Now for SharedNow {
        fn seconds(&self) -> f64 {
            *self.0.lock().expect("not poisoned")
        }
    }

    fn config(presets: &Value, active: &str) -> Value {
        serde_json::json!({
            "spammer": {
                "active": active,
                "presets": presets,
            }
        })
    }

    #[test]
    fn the_active_preset_is_read_in_file_order_with_its_cooldowns() {
        let config = config(
            &serde_json::json!({
                "buff": {"1": 1, "2": 0.5},
                "other": {"3": 2},
            }),
            "buff",
        );

        let read = rotation(&config);

        assert!(read.presets_configured, "two presets are configured");
        assert_eq!(read.active, "buff");
        assert!(read.problems.is_empty(), "{:?}", read.problems);
        let rotation = read.rotation.expect("the active preset resolved");
        assert_eq!(
            rotation.keys,
            vec![("1".to_owned(), 1.0), ("2".to_owned(), 0.5)],
            "file order is the rotation order"
        );
    }

    #[test]
    fn a_preset_that_is_not_there_gets_the_look_up_line() {
        let config = config(&serde_json::json!({"buff": {"1": 1}}), "missing");

        let line = plan(&config).expect_err("no such preset");

        assert!(line.contains("was not found"), "{line}");
        assert!(line.contains("Spammer tab"), "{line}");
    }

    #[test]
    fn no_presets_at_all_gets_the_add_one_line() {
        let config = config(&serde_json::json!({}), "");

        let line = plan(&config).expect_err("no presets");

        assert!(line.contains("No spammer presets"), "{line}");
    }

    #[test]
    fn unsendable_keys_are_all_named_before_a_run_starts() {
        let config = config(
            &serde_json::json!({"buff": {"1": 1, "ab": 2, "F13": 3}}),
            "buff",
        );

        let lines = unsupported_keys(&config);

        assert_eq!(
            lines.len(),
            2,
            "two of the three keys are unsendable: {lines:?}"
        );
        assert!(lines.iter().any(|line| line.contains("'ab'")), "{lines:?}");
        assert!(lines.iter().any(|line| line.contains("'F13'")), "{lines:?}");
    }

    #[test]
    fn the_card_detail_reports_the_lead_in_then_the_current_key_and_cycle() {
        let config = serde_json::json!({
            "spammer": {
                "active": "buff",
                "presets": {"buff": {"1": 5.0, "2": 5.0}},
                "startup": {"buff": {"lead_in": 2.0, "step": 0.0}},
            }
        });
        let plan = plan(&config).expect("a plan");
        let clock = Arc::new(Mutex::new(0.0));
        let mut tool = SpammerTool::new(&plan, Box::new(SharedNow(Arc::clone(&clock))));

        // During the lead-in v2 showed its schedule line + countdown, and no facts yet.
        let lead_in = tool.status();
        assert_eq!(lead_in.message, "lead-in — starting shortly");
        assert!(
            lead_in
                .countdown_s
                .is_some_and(|seconds| (seconds - 2.0).abs() < 1e-9),
            "{:?}",
            lead_in.countdown_s
        );
        assert!(lead_in.detail.is_empty(), "{:?}", lead_in.detail);

        // Past the lead-in the opening presses go out: v2's Cycle (accepted presses) and Current
        // (the last key) reach the card.
        *clock.lock().expect("not poisoned") = 2.5;
        let _first_press = tool.decide();
        let running = tool.status();
        assert!(running.countdown_s.is_none());
        assert_eq!(
            running.detail,
            vec![
                ToolDetail::labeled("Cycle", "2"),
                ToolDetail::labeled("Current", "2"),
            ],
            "both due keys were pressed, the last one named"
        );
    }

    #[test]
    fn a_tick_turns_presses_into_decisions_then_waits() {
        // A cooldown above the tick is what makes the key STOP being due; a cooldown below the tick
        // is refused at start (TOOL-F12), so there is no "always due" preset left to wait on.
        let config = config(&serde_json::json!({"buff": {"1": 5.0}}), "buff");
        let plan = plan(&config).expect("a plan");
        let mut tool = SpammerTool::new(&plan, Box::new(StepNow::new(0.0)));

        // The opening presses the only key, then a tick with nothing due waits v2's 20 ms.
        assert_eq!(tool.decide(), Decision::Key(Key::Char('1')));
        assert_eq!(tool.decide(), Decision::Wait(TICK_SECONDS));
    }

    #[test]
    fn a_starred_key_is_the_fast_tap_decision() {
        // The shortest cooldown the tick can honour; a zero one is refused at start (TOOL-F12).
        let config = config(&serde_json::json!({"buff": {"*1": 0.02}}), "buff");
        let plan = plan(&config).expect("a plan");
        let mut tool = SpammerTool::new(&plan, Box::new(StepNow::new(0.0)));

        assert_eq!(tool.decide(), Decision::KeyFast(Key::Char('1')));
    }

    /// TOOL-F12: a cooldown the run cannot honour refuses the START and names the key. Before this, a
    /// cooldown written as a string (or 0, or negative) became "due every tick" — about fifty presses
    /// a second, and under a priority order the other keys never fired at all.
    #[test]
    fn a_cooldown_that_is_not_a_number_or_too_short_refuses_the_start() {
        let text = config(&serde_json::json!({"buff": {"1": "fast"}}), "buff");
        let line = plan(&text).expect_err("a string cooldown cannot start");
        assert!(line.contains("Spammer key '1' has no cooldown"), "{line}");

        let zero = config(&serde_json::json!({"buff": {"1": 0.0, "2": 2.0}}), "buff");
        let line = plan(&zero).expect_err("a zero cooldown cannot start");
        assert!(line.contains("Spammer key '1'"), "{line}");
        assert!(line.contains("at least 0.02 s"), "{line}");

        let negative = config(&serde_json::json!({"buff": {"1": -1.0}}), "buff");
        let line = plan(&negative).expect_err("a negative cooldown cannot start");
        assert!(line.contains("Spammer key '1'"), "{line}");

        // One tick is the shortest cooldown the 20 ms loop can honour.
        let shortest = config(&serde_json::json!({"buff": {"1": 0.02}}), "buff");
        assert!(plan(&shortest).is_ok());
    }
}

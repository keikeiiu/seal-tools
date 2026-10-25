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
use seal_tools::runner::Tool;
use seal_tools::tool::ToolId;
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
pub fn rotation(config: &Value) -> (bool, String, Option<SpammerRotation>) {
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
        return (presets_configured, active, None);
    };
    if body.is_empty() {
        return (presets_configured, active, None);
    }

    let keys: Vec<(String, f64)> = body
        .iter()
        .map(|(key, cooldown)| (key.clone(), cooldown.as_f64().unwrap_or(0.0)))
        .collect();
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

    (
        presets_configured,
        active.clone(),
        Some(SpammerRotation {
            keys,
            priority,
            combos,
            startup,
        }),
    )
}

/// The run's plan, or the refusal line v2's `Run()` shows.
pub fn plan(config: &Value) -> Result<SpammerPlan, String> {
    let (presets_configured, active, rotation) = rotation(config);
    plan_or_stop(presets_configured, &active, rotation).map_err(|reason| reason.message)
}

/// The keys the firmware cannot send, one v2 warning line each. The whole rotation is checked at
/// START: a run that would silently drop half its rotation is worse than a refused start, and the
/// player can fix the preset before anything is pressed.
pub fn unsupported_keys(config: &Value) -> Vec<String> {
    let (_, _, rotation) = rotation(config);
    let Some(rotation) = rotation else {
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
}

impl SpammerTool {
    pub fn new(plan: &SpammerPlan, now: Box<dyn Now + Send>) -> Self {
        let started = now.seconds();
        Self {
            run: SpammerRun::start(plan, started),
            now,
            queue: VecDeque::new(),
        }
    }
}

impl Tool for SpammerTool {
    fn id(&self) -> ToolId {
        ToolId::Spammer
    }

    fn decide(&mut self) -> Decision {
        if let Some(decision) = self.queue.pop_front() {
            return decision;
        }

        let tick = self.run.tick(self.now.seconds());
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

    fn config(presets: Value, active: &str) -> Value {
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
            serde_json::json!({
                "buff": {"1": 1, "2": 0.5},
                "other": {"3": 2},
            }),
            "buff",
        );

        let (configured, active, rotation) = rotation(&config);

        assert!(configured, "two presets are configured");
        assert_eq!(active, "buff");
        let rotation = rotation.expect("the active preset resolved");
        assert_eq!(
            rotation.keys,
            vec![("1".to_owned(), 1.0), ("2".to_owned(), 0.5)],
            "file order is the rotation order"
        );
    }

    #[test]
    fn a_preset_that_is_not_there_gets_the_look_up_line() {
        let config = config(serde_json::json!({"buff": {"1": 1}}), "missing");

        let line = plan(&config).expect_err("no such preset");

        assert!(line.contains("was not found"), "{line}");
        assert!(line.contains("Spammer tab"), "{line}");
    }

    #[test]
    fn no_presets_at_all_gets_the_add_one_line() {
        let config = config(serde_json::json!({}), "");

        let line = plan(&config).expect_err("no presets");

        assert!(line.contains("No spammer presets"), "{line}");
    }

    #[test]
    fn unsendable_keys_are_all_named_before_a_run_starts() {
        let config = config(
            serde_json::json!({"buff": {"1": 1, "ab": 2, "F13": 3}}),
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
    fn a_tick_turns_presses_into_decisions_then_waits() {
        // A cooldown above zero is what makes the key STOP being due: a zero-cooldown key is due every
        // tick, which is a legitimate preset (a key with no cooldown) rather than the wait case.
        let config = config(serde_json::json!({"buff": {"1": 5.0}}), "buff");
        let plan = plan(&config).expect("a plan");
        let mut tool = SpammerTool::new(&plan, Box::new(StepNow::new(0.0)));

        // The opening presses the only key, then a tick with nothing due waits v2's 20 ms.
        assert_eq!(tool.decide(), Decision::Key(Key::Char('1')));
        assert_eq!(tool.decide(), Decision::Wait(TICK_SECONDS));
    }

    #[test]
    fn a_starred_key_is_the_fast_tap_decision() {
        let config = config(serde_json::json!({"buff": {"*1": 0.0}}), "buff");
        let plan = plan(&config).expect("a plan");
        let mut tool = SpammerTool::new(&plan, Box::new(StepNow::new(0.0)));

        assert_eq!(tool.decide(), Decision::KeyFast(Key::Char('1')));
    }
}

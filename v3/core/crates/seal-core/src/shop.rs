//! The Shop tools (sell and buy): pure TRANSACTION-PLAN executors.
//!
//! Neither shop tool reads the screen. v2's `ShopTool` is geometry plus an ordered transaction: the
//! bag grid and shop rows are DERIVED from two dragged calibrations (64 hand-marked points would be
//! 64 chances to be wrong), the order and measured waits live in `seal-domain`'s `ShopPlan`, and this
//! module only turns each [`ShopStep`] into a runner decision and counts the done line.
//!
//! Two notes, both recorded rather than hidden:
//!
//! - **Buy takes its parameters from the RUN** (`PROTOCOL.md` §4 note / §12.6): `tool.start` carries
//!   `preset` and `count`, a missing `count` uses the preset's configured count, and an explicit count
//!   below 1 is refused at the argument layer. `buy_plan` resolves them against `buy_sell.presets`;
//!   without a preset it answers the domain's own "No buy preset selected." line, and an unknown name
//!   the domain's line naming it.
//! - **Selling is the one irreversible thing here**, so the slot selection is filtered, de-duplicated,
//!   highest-index-first and capped by the domain BEFORE any click; this loop only executes what that
//!   plan says.

use std::collections::BTreeMap;

use seal_domain::config::BuyPreset;
use seal_domain::decision::{Decision, StopReason};
use seal_domain::tools::shop::{ClickButton, ShopPlan, ShopSettings, ShopStep};
use seal_tools::runner::{Tool, ToolStatus};
use seal_tools::tool::ToolId;
use serde_json::Value;

/// The merged config's `buy_sell` block, as the domain's decision half wants it.
pub fn settings(config: &Value) -> ShopSettings {
    let block = config.get("buy_sell");
    let ints = |name: &str| -> Option<Vec<i32>> {
        block
            .and_then(|block| block.get(name))
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_i64)
                    .filter_map(|value| i32::try_from(value).ok())
                    .collect()
            })
    };
    let integer = |name: &str| -> Option<i32> {
        block
            .and_then(|block| block.get(name))
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
    };

    ShopSettings {
        bag_grid: ints("bag_grid"),
        bag_slot: ints("bag_slot"),
        shop_rows: integer("shop_rows").unwrap_or(0),
        shop_region: ints("shop_region"),
        scroll_point: ints("scroll_point"),
        max_button: ints("max_button"),
        presets: block
            .and_then(|block| block.get("presets"))
            .cloned()
            .and_then(|value| serde_json::from_value::<BTreeMap<String, BuyPreset>>(value).ok())
            .unwrap_or_default(),
        sell_slots: ints("sell_slots").unwrap_or_default(),
        sell_cap: integer("sell_cap").unwrap_or(0),
    }
}

/// One SELL pass, validated: the domain refuses an uncalibrated grid, a missing MAX/focus point, an
/// empty selection, or a selection above the per-run cap BEFORE this returns.
pub fn sell_plan(config: &Value) -> Result<ShopPlan, StopReason> {
    ShopPlan::sell(&settings(config))
}

/// One BUY pass, from the RUN arguments (`PROTOCOL.md` §4 / §12.6): `preset` names a
/// `buy_sell.presets` key and `count` is how many transactions; a missing `count` uses the preset
/// configured count, never a hardcoded 1. `preset` None is v2 domain own "No buy preset selected."
/// line; an unknown name is the domain "no longer exists" line naming it.
///
/// A `count` present and below 1 is refused HERE: the domain clamps to one for its own callers, but
/// the protocol says an explicit number the player gave is refused rather than silently changed.
pub fn buy_plan(
    config: &Value,
    preset: Option<&str>,
    count: Option<i32>,
) -> Result<ShopPlan, StopReason> {
    if count.is_some_and(|count| count < 1) {
        return Err(StopReason::error("The buy count must be at least 1."));
    }
    let settings = settings(config);
    let count = count.unwrap_or_else(|| {
        preset
            .and_then(|name| settings.presets.get(name))
            .map_or(1, |preset| preset.count)
    });
    ShopPlan::buy(&settings, preset, count)
}

/// The plan as the runner sees it: one decision per step, then the done line.
pub struct ShopTool {
    id: ToolId,
    plan: ShopPlan,
    index: usize,
    message: String,
}

impl ShopTool {
    pub fn new(id: ToolId, plan: ShopPlan) -> Self {
        Self {
            id,
            plan,
            index: 0,
            message: String::new(),
        }
    }

    fn describe(step: &ShopStep, index: usize, total: usize) -> String {
        let what = match step {
            ShopStep::Place(_) => "moving the cursor",
            ShopStep::Click(ClickButton::Left) => "clicking",
            ShopStep::Click(ClickButton::Right) => "right-clicking",
            ShopStep::Enter => "pressing Enter",
            ShopStep::Scroll { .. } => "scrolling the list",
            ShopStep::Wait(_) => "waiting for the game",
        };
        format!("step {index}/{total}: {what}")
    }
}

impl Tool for ShopTool {
    fn id(&self) -> ToolId {
        self.id
    }

    fn status(&self) -> ToolStatus {
        ToolStatus {
            message: self.message.clone(),
            countdown_s: None,
            rows: Vec::new(),
        }
    }

    fn decide(&mut self) -> Decision {
        if self.index >= self.plan.steps.len() {
            return Decision::Stop(StopReason::done(self.plan.done_message.clone()));
        }
        let step = self.plan.steps[self.index].clone();
        self.index += 1;
        self.message = Self::describe(&step, self.index, self.plan.steps.len());

        match step {
            ShopStep::Place(point) => Decision::Place(point),
            ShopStep::Click(ClickButton::Left) => Decision::ClickHere,
            ShopStep::Click(ClickButton::Right) => Decision::RightClickHere,
            ShopStep::Enter => Decision::Enter,
            ShopStep::Scroll { notches, down } => Decision::Wheel { notches, down },
            ShopStep::Wait(seconds) => Decision::Wait(seconds),
        }
    }
}

#[cfg(test)]
mod tests {
    use seal_domain::decision::StopKind;

    use super::*;

    fn config() -> Value {
        serde_json::json!({
            "buy_sell": {
                "bag_grid": [0, 0, 800, 600],
                "bag_slot": [0, 0, 100, 75],
                "shop_rows": 5,
                "shop_region": [900, 400, 300, 250],
                "scroll_point": [1000, 420],
                "max_button": [1200, 700],
                "presets": {"potion": {"scroll": 2, "row": 1, "count": 3}},
                "sell_slots": [3, 1],
                "sell_cap": 5
            }
        })
    }

    #[test]
    fn the_settings_read_the_machine_half_and_its_geometry() {
        let settings = settings(&config());

        assert_eq!(settings.bag_grid, Some(vec![0, 0, 800, 600]));
        assert_eq!(settings.shop_rows, 5);
        assert_eq!(settings.scroll_point, Some(vec![1000, 420]));
        assert_eq!(settings.sell_cap, 5);
        assert_eq!(settings.sell_slots, vec![3, 1]);
        assert_eq!(
            settings.presets.get("potion").map(|preset| preset.row),
            Some(1)
        );
    }

    #[test]
    fn a_sell_pass_is_focus_then_each_selected_slot_high_first() {
        let plan = sell_plan(&config()).expect("the plan validates");
        let mut tool = ShopTool::new(ToolId::Sell, plan);

        // Focus: place the scroll point, wait, left click, wait.
        assert_eq!(
            tool.decide(),
            Decision::Place(seal_domain::geometry::Point::new(1000, 420))
        );
        assert_eq!(tool.decide(), Decision::Wait(0.35));
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.45));

        // First transaction: slot 3's centre, then the MAX button, each with its right/left click.
        let mut rights = 0;
        let mut enters = 0;
        let mut clicks = 0;
        let mut done = None;
        for _ in 0..200 {
            match tool.decide() {
                Decision::RightClickHere => rights += 1,
                Decision::Enter => enters += 1,
                Decision::ClickHere => clicks += 1,
                Decision::Place(_) | Decision::Wait(_) => {}
                Decision::Stop(reason) => {
                    done = Some(reason);
                    break;
                }
                other => panic!("the sell plan never sends {other:?}"),
            }
        }

        assert_eq!(rights, 2, "one right-click per sold slot");
        assert_eq!(
            clicks, 2,
            "one MAX left click per sold slot (the focus one is counted above)"
        );
        assert_eq!(enters, 4, "Enter twice per transaction");
        let done = done.expect("the pass finishes");
        assert_eq!(done.kind, StopKind::Done);
        assert_eq!(done.message, "Sold 2 slot(s).");
    }

    #[test]
    fn a_sell_pass_with_no_selected_slots_is_refused_with_the_domain_line() {
        let mut config = config();
        config["buy_sell"]["sell_slots"] = serde_json::json!([]);

        let reason = sell_plan(&config).expect_err("nothing selected");

        assert_eq!(reason.message, "No slots are selected to sell.");
    }

    #[test]
    fn a_buy_pass_without_a_preset_is_the_domain_own_line() {
        let reason = buy_plan(&config(), None, None).expect_err("no preset chosen");

        assert_eq!(reason.message, "No buy preset selected.");
    }

    #[test]
    fn a_buy_pass_uses_the_run_count_and_names_the_preset() {
        let plan = buy_plan(&config(), Some("potion"), Some(5)).expect("the plan validates");
        assert_eq!(plan.done_message, "Bought 5x 'potion'.");

        // One transaction per run count: each extra buy adds the same steps, so the deltas between
        // counts must be equal (this is the run number, not the preset's configured count).
        let three = buy_plan(&config(), Some("potion"), Some(3)).expect("the plan validates");
        let one = buy_plan(&config(), Some("potion"), Some(1)).expect("the plan validates");
        assert_eq!(
            plan.steps.len() - three.steps.len(),
            three.steps.len() - one.steps.len(),
            "one transaction per run count"
        );
    }

    #[test]
    fn a_missing_count_uses_the_preset_configured_count() {
        let configured = buy_plan(&config(), Some("potion"), None).expect("the plan validates");
        let explicit = buy_plan(&config(), Some("potion"), Some(3)).expect("the plan validates");

        assert_eq!(configured.done_message, "Bought 3x 'potion'.");
        assert_eq!(configured.steps, explicit.steps);
        assert_eq!(configured.done_message, explicit.done_message);
    }

    #[test]
    fn an_unknown_preset_names_the_chosen_preset() {
        let reason = buy_plan(&config(), Some("ghost"), Some(1)).expect_err("no such preset");

        assert_eq!(reason.message, "Buy preset 'ghost' no longer exists.");
    }

    #[test]
    fn a_count_below_one_is_refused_at_the_argument_layer() {
        for count in [0, -1, i32::MIN] {
            let reason = buy_plan(&config(), Some("potion"), Some(count))
                .expect_err("an explicit count below 1 is refused");
            assert_eq!(
                reason.message, "The buy count must be at least 1.",
                "{count}"
            );
        }
    }

    #[test]
    fn a_scroll_step_becomes_the_wheel_decision() {
        let plan = ShopPlan {
            steps: vec![ShopStep::Scroll {
                notches: 3,
                down: true,
            }],
            done_message: "scrolled".to_owned(),
        };
        let mut tool = ShopTool::new(ToolId::Buy, plan);

        assert_eq!(
            tool.decide(),
            Decision::Wheel {
                notches: 3,
                down: true
            }
        );
        assert!(matches!(tool.decide(), Decision::Stop(_)));
    }

    #[test]
    fn the_status_names_the_step_it_is_about_to_perform() {
        let plan = ShopPlan {
            steps: vec![ShopStep::Enter],
            done_message: "done".to_owned(),
        };
        let mut tool = ShopTool::new(ToolId::Sell, plan);

        tool.decide();
        assert_eq!(tool.status().message, "step 1/1: pressing Enter");
    }
}

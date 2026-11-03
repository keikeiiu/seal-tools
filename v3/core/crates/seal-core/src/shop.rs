//! The Shop tools (sell and buy): pure TRANSACTION-PLAN executors.
//!
//! Neither shop tool reads the screen. v2's `ShopTool` is geometry plus an ordered transaction: the
//! bag grid and shop rows are DERIVED from two dragged calibrations (64 hand-marked points would be
//! 64 chances to be wrong), the order and measured waits live in `seal-domain`'s `ShopPlan`, and this
//! module only turns each [`ShopStep`] into a runner decision and counts the done line.
//!
//! Two notes, both recorded rather than hidden:
//!
//! - **Both tools take their run parameters from `tool.start`** (`PROTOCOL.md` §4 note / §12.6):
//!   `buy` carries `preset` and `count`; `sell` carries `slots`, its ONLY selection for that run.
//!   Absent slots mean an empty selection (never the migrated `buy_sell.sell_slots` field), and
//!   nothing is ever written back to config. A missing buy `count` uses the preset's configured count, an explicit count below 1 is refused at
//!   the argument layer, and `buy_plan` resolves the preset against `buy_sell.presets`; without a
//!   preset it answers the domain's own "No buy preset selected." line, and an unknown name the
//!   domain's line naming it.
//! - **Selling is the one irreversible thing here**, so the slot selection is filtered, de-duplicated,
//!   highest-index-first and capped by the domain BEFORE any click; this loop only executes what that
//!   plan says.

use std::collections::BTreeMap;

use seal_domain::config::BuyPreset;
use seal_domain::decision::{Decision, StopReason};
use seal_domain::tools::shop::{ClickButton, ShopPlan, ShopSettings, ShopStep};
use seal_tools::runner::{Tool, ToolStatus};
use seal_tools::tool::{ToolDetail, ToolId};
use serde_json::Value;

/// The merged config's `buy_sell` block as the DOMAIN decision half wants it. Deliberately does
/// NOT read `buy_sell.sell_slots`: that persisted field is migration data with no writer, and a sell
/// run's selection comes only from `tool.start`'s `slots` argument. `ShopSettings::sell_slots` is
/// therefore left empty here and filled at the run seam; the typed model keeps the config field for
/// compatibility only.
#[must_use]
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
        sell_slots: Vec::new(),
        sell_cap: integer("sell_cap").unwrap_or(0),
    }
}

/// One SELL pass with no run argument: an EMPTY selection, never the persisted config field, so the
/// domain's own "No slots are selected to sell." line is the answer.
///
/// # Errors
///
/// Returns the domain's stop reason when the shop grid or one of its marks is not calibrated.
pub fn sell_plan(config: &Value) -> Result<ShopPlan, StopReason> {
    sell_plan_with_slots(config, None)
}

/// One SELL pass, validated: the domain refuses an uncalibrated grid, a missing MAX/focus point, an
/// empty selection, or a selection above the per-run cap BEFORE this returns.
///
/// `slots` is the per-run selection from `tool.start` (`PROTOCOL.md` §4 / §12.6 ruling 6). It is the
/// ONLY source of a sell run's selection: absent means empty, not the migrated config field, and
/// nothing is ever written back. The domain does the filtering, dedupe, sort and `sell_cap`.
///
/// # Errors
///
/// Returns the domain's stop reason when the shop grid or one of its marks is not calibrated.
pub fn sell_plan_with_slots(config: &Value, slots: Option<&[i32]>) -> Result<ShopPlan, StopReason> {
    let mut settings = settings(config);
    settings.sell_slots = slots.unwrap_or_default().to_vec();
    ShopPlan::sell(&settings)
}

/// The supervisor's cheap availability gate for SELL. The per-run selection arrives later with
/// `tool.start`, so this validates the calibration/geometry half with a probe selection instead of
/// consulting the config's `sell_slots`: the selection is run data, not availability. A start with
/// no selection still reaches `create`, where the domain answers its own "No slots are selected to
/// sell." line.
///
/// # Errors
///
/// Returns the domain's stop reason when no selection can be sold.
pub fn sell_available(config: &Value) -> Result<(), StopReason> {
    let mut settings = settings(config);
    settings.sell_slots = vec![0];
    settings.sell_cap = settings.sell_cap.max(1);
    ShopPlan::sell(&settings).map(|_| ())
}

/// One BUY pass, from the RUN arguments (`PROTOCOL.md` §4 / §12.6): `preset` names a
/// `buy_sell.presets` key and `count` is how many transactions; a missing `count` uses the preset
/// configured count, never a hardcoded 1. `preset` None is v2 domain own "No buy preset selected."
/// line; an unknown name is the domain "no longer exists" line naming it.
///
/// A `count` present and below 1 is refused HERE: the domain clamps to one for its own callers, but
/// the protocol says an explicit number the player gave is refused rather than silently changed.
///
/// # Errors
///
/// Returns the domain's stop reason when the preset, the count or one of the marks is not usable.
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
    /// The step indices that START a transaction (its right click), in plan order. v2 set `Cycle` to
    /// the transaction's number (`ShopTool.cs:168,212`) and `Current` to its item/slot line; the plan
    /// steps are the only per-transaction marks this tool receives, so they are derived here.
    transactions: Vec<usize>,
    /// The buy preset's name for the `Current` line, from the plan's own done line. `None` for a sell
    /// plan (and for an unparseable buy line), which then names the position only.
    item: Option<String>,
}

impl ShopTool {
    #[must_use]
    pub fn new(id: ToolId, plan: ShopPlan) -> Self {
        let transactions = plan
            .steps
            .iter()
            .enumerate()
            .filter(|(_, step)| matches!(step, ShopStep::Click(ClickButton::Right)))
            .map(|(index, _)| index)
            .collect();
        let item = buy_item(&plan.done_message);
        Self {
            id,
            plan,
            index: 0,
            message: String::new(),
            transactions,
            item,
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

    /// v2's `Cycle` + `Current` (`ShopTool.cs:168-169,212-213`).
    ///
    /// `Cycle` is the transaction being worked (1-based; empty before the first right click). `Current`
    /// is v2's sentence where the plan can still supply it: a BUY plan's done line names its preset, so
    /// the line reads `"{preset} {n}/{total}"`; a SELL plan carries neither the selection nor the bag
    /// indices (`ShopPlan` is steps + a done line), so its line names the position only — completing it
    /// needs the domain plan to carry the selection, which is outside this increment's write scope.
    fn card_detail(&self) -> Vec<ToolDetail> {
        let total = self.transactions.len();
        let cycle = self
            .transactions
            .iter()
            .filter(|start| **start < self.index)
            .count();
        if cycle == 0 || total == 0 {
            return Vec::new();
        }
        let current = if self.id == ToolId::Buy {
            self.item.as_ref().map_or_else(
                || format!("{cycle}/{total}"),
                |item| format!("{item} {cycle}/{total}"),
            )
        } else {
            format!("slot ({cycle}/{total})")
        };
        vec![
            ToolDetail::labeled("Cycle", cycle.to_string()),
            ToolDetail::labeled("Current", current),
        ]
    }
}

/// The buy preset's name out of the plan's own done line (`Bought {count}x '{preset}'.`).
///
/// `ShopPlan` has no preset field, and the domain plan and the registry are outside this increment's
/// write scope, so the word the run already reports is the carrier. A line this does not recognise
/// yields `None`, and the card falls back to the position only — never a wrong name.
fn buy_item(done_message: &str) -> Option<String> {
    let rest = done_message.strip_prefix("Bought ")?;
    let (_, named) = rest.split_once("x '")?;
    let (name, _) = named.split_once("'.")?;
    (!name.is_empty()).then(|| name.to_owned())
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
            detail: self.card_detail(),
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
        assert!(
            settings.sell_slots.is_empty(),
            "the saved field is not adopted into the run shape"
        );
        assert_eq!(
            settings.presets.get("potion").map(|preset| preset.row),
            Some(1)
        );
    }

    #[test]
    fn a_sell_pass_is_focus_then_each_selected_slot_high_first() {
        let plan = sell_plan_with_slots(&config(), Some(&[3, 1])).expect("the plan validates");
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
    fn a_sell_run_without_slots_ignores_the_config_selection_and_refuses() {
        // v3.0.1 could have left a stale selection here; a start with no `slots` must not sell it.
        let config = config();
        assert_eq!(config["buy_sell"]["sell_slots"], serde_json::json!([3, 1]));

        let reason = sell_plan(&config).expect_err("a saved selection is not run data");

        assert_eq!(reason.message, "No slots are selected to sell.");
    }

    #[test]
    fn a_sell_run_uses_the_start_slots_and_ignores_the_config_field() {
        let config = config(); // the stale config field is [3, 1]
        let plan =
            sell_plan_with_slots(&config, Some(&[5, 1, 5])).expect("the run selection validates");

        assert_eq!(plan.done_message, "Sold 2 slot(s).", "dedupe, then cap");
        let centres = seal_domain::tools::shop::BagGrid::centres(Some(&[0, 0, 800, 600]));
        let max = seal_domain::geometry::Point::new(1200, 700);
        let places: Vec<_> = plan
            .steps
            .iter()
            .filter_map(|step| match step {
                ShopStep::Place(point) => Some(*point),
                _ => None,
            })
            .collect();
        assert_eq!(
            places,
            vec![
                seal_domain::geometry::Point::new(1000, 420),
                centres[5],
                max,
                centres[1],
                max
            ],
            "focus, then the highest selected cell first; the config's [3,1] is not used"
        );
    }

    #[test]
    fn a_sell_run_without_slots_never_uses_the_config_selection() {
        let mut config = config();
        config["buy_sell"]["sell_slots"] = serde_json::json!([9, 8, 7]);
        let error = sell_plan_with_slots(&config, None)
            .expect_err("a saved selection is migration data, not a run fallback");
        assert_eq!(error.message, "No slots are selected to sell.");

        // The per-run selection is the only thing that can make a sell plan.
        let plan =
            sell_plan_with_slots(&config, Some(&[2])).expect("an explicit run selection validates");
        assert_eq!(plan.done_message, "Sold 1 slot(s).");
    }

    #[test]
    fn the_availability_gate_ignores_the_selection_but_still_checks_calibration() {
        let mut unselected = config();
        unselected["buy_sell"]["sell_slots"] = serde_json::json!([]);
        assert!(
            sell_available(&unselected).is_ok(),
            "the selection arrives with the start, so the saved field is not a preflight refusal"
        );

        let mut uncalibrated = unselected.clone();
        uncalibrated["buy_sell"]["bag_grid"] = serde_json::Value::Null;
        let reason = sell_available(&uncalibrated).expect_err("the grid is still required");
        assert!(reason.message.contains("bag grid"), "{}", reason.message);
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

    /// Walk the plan until the next transaction's right click has been performed — the step v2
    /// counted as the transaction's start.
    fn walk_to_right_click(tool: &mut ShopTool) {
        for _ in 0..200 {
            if tool.decide() == Decision::RightClickHere {
                return;
            }
        }
        panic!("the plan never reached a transaction's right click");
    }

    #[test]
    fn a_buy_card_carries_the_transaction_cycle_and_the_preset_progress() {
        let plan = buy_plan(&config(), Some("potion"), Some(3)).expect("the plan validates");
        let mut tool = ShopTool::new(ToolId::Buy, plan);

        assert!(
            tool.status().detail.is_empty(),
            "before the first transaction there is no cycle to report"
        );

        walk_to_right_click(&mut tool);
        assert_eq!(
            tool.status().detail,
            vec![
                ToolDetail::labeled("Cycle", "1"),
                ToolDetail::labeled("Current", "potion 1/3"),
            ],
            "v2's Current is the preset's progress through this run"
        );

        walk_to_right_click(&mut tool);
        assert_eq!(
            tool.status().detail,
            vec![
                ToolDetail::labeled("Cycle", "2"),
                ToolDetail::labeled("Current", "potion 2/3"),
            ]
        );
    }

    #[test]
    fn a_sell_card_carries_the_transaction_cycle_and_the_position() {
        let plan = sell_plan_with_slots(&config(), Some(&[3, 1])).expect("two slots");
        let mut tool = ShopTool::new(ToolId::Sell, plan);

        walk_to_right_click(&mut tool);
        assert_eq!(
            tool.status().detail,
            vec![
                ToolDetail::labeled("Cycle", "1"),
                ToolDetail::labeled("Current", "slot (1/2)"),
            ],
            "the bag slot number is not carried by ShopPlan, so the position is named"
        );
    }
}

//! The spammer's decision half: the walk order, the combo rule, the opening, and the tick.
//!
//! `v2/SealTools.Tests/SpammerOrderTests.cs` is the oracle for the order/combination rules and is
//! translated here almost line for line. The tick behaviour (opening stagger, lead-in, preemption,
//! combos owning a tick) comes from `SkillSpammer.cs`'s run loop, which v2 could not test without a
//! serial port — that gap is the reason this crate exists (`PLAN-V3-CORE.md` §2.1).
//!
//! One thing is DEFINED rather than ported: the input is an ordered `keys: Vec<(name, cooldown)>`
//! because the walk's "dictionary order" is the order the config file lists a preset's keys in, and
//! v2's `Dictionary` preserved it. Rust's `BTreeMap` would sort it, so the rotation keeps the order
//! explicitly and the caller (the config merge) must not sort.

use seal_domain::config::SpammerCombo;
use seal_domain::decision::StopReason;
use seal_domain::tools::spammer::{
    BoardPress, SpammerPlan, SpammerPress, SpammerRotation, SpammerRun, order_for, plan_or_stop,
    respell, runnable_combos, singles, unsupported_key_message,
};

fn combo(keys: &[&str], gap: f64) -> SpammerCombo {
    SpammerCombo {
        keys: keys.iter().map(|key| (*key).to_owned()).collect(),
        gap,
    }
}

fn names(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|key| (*key).to_owned()).collect()
}

fn rotation(keys: &[(&str, f64)], priority: &[&str], combos: Vec<SpammerCombo>) -> SpammerRotation {
    SpammerRotation {
        keys: keys
            .iter()
            .map(|(key, cooldown)| ((*key).to_owned(), *cooldown))
            .collect(),
        priority: names(priority),
        combos,
        startup: Default::default(),
    }
}

// ── the walk order (v2's SpammerOrderTests) ─────────────────────────────────────────────────────

#[test]
fn named_keys_come_first_in_the_order_given() {
    let order = order_for(&names(&["q", "w", "e", "t"]), &names(&["e", "q"]));

    assert_eq!(order, names(&["e", "q", "w", "t"]));
}

/// The unnamed ones are the "press when we are able to" half. They must still be walked — a priority
/// list that silently dropped them would turn a filler key off entirely.
#[test]
fn unnamed_keys_follow_in_config_order() {
    let order = order_for(&names(&["t", "u", "v", "q"]), &names(&["q"]));

    assert_eq!(order, names(&["q", "t", "u", "v"]));
}

/// A typo in the order must not stop the rotation, and must not invent a key either.
#[test]
fn a_name_matching_no_key_is_skipped() {
    let order = order_for(&names(&["q", "w"]), &names(&["nope", "w"]));

    assert_eq!(order, names(&["w", "q"]));
}

/// First position wins. Taking the LAST occurrence instead would let a duplicated name silently move
/// a key down the rotation.
#[test]
fn a_repeated_name_is_taken_once_at_its_first_position() {
    let order = order_for(&names(&["q", "w", "e"]), &names(&["e", "q", "e"]));

    assert_eq!(order, names(&["e", "q", "w"]));
}

/// The additive promise: a preset written before any of this existed has no order and must walk
/// exactly as it did.
#[test]
fn no_order_given_walks_in_config_order() {
    let order = order_for(&names(&["q", "w", "e"]), &[]);

    assert_eq!(order, names(&["q", "w", "e"]));
}

/// Every key the preset has must appear exactly once, whatever the order is — a duplicate would press
/// the same key twice in one tick.
#[test]
fn every_key_appears_exactly_once() {
    let order = order_for(
        &names(&["q", "w", "e", "t"]),
        &names(&["t", "t", "ghost", "q"]),
    );

    assert_eq!(order, names(&["t", "q", "w", "e"]));
    assert_eq!(order.len(), 4);
}

// ── combos (v2's SpammerOrderTests) ────────────────────────────────────────────────────────────

/// THE LOCKED PAIR: a key a combo names must not also be walkable on its own.
#[test]
fn keys_owned_by_a_combo_are_not_pressed_on_their_own() {
    let singles = singles(
        &names(&["1", "2", "3", "4", "q"]),
        &names(&["1", "2", "3", "4"]),
        &[combo(&["1", "2"], 0.8), combo(&["3", "4"], 0.8)],
    );

    assert_eq!(singles, names(&["q"]));
}

#[test]
fn with_no_combos_every_key_stands_alone() {
    let singles = singles(&names(&["q", "w", "e"]), &names(&["e"]), &[]);

    assert_eq!(singles, names(&["e", "q", "w"]));
}

#[test]
fn unnamed_keys_keep_their_precedence() {
    let singles = singles(
        &names(&["q", "w", "e", "1", "2"]),
        &names(&["e", "q"]),
        &[combo(&["1", "2"], 0.8)],
    );

    assert_eq!(singles, names(&["e", "q", "w"]));
}

/// HALF A COMBO IS WORSE THAN NONE: it would fire the first skill, spend its cooldown, and leave the
/// effect un-triggered.
#[test]
fn a_combo_needs_at_least_two_keys() {
    let runnable = runnable_combos(&names(&["1", "2"]), &[combo(&["1"], 0.8)]);

    assert!(runnable.is_empty());
}

#[test]
fn a_combo_naming_a_key_the_preset_lacks_is_dropped() {
    let runnable = runnable_combos(&names(&["1", "2"]), &[combo(&["1", "nope"], 0.8)]);

    assert!(runnable.is_empty());
}

#[test]
fn runnable_combos_keep_their_listed_order() {
    let runnable = runnable_combos(
        &names(&["1", "2", "3", "4"]),
        &[combo(&["3", "4"], 0.8), combo(&["1", "2"], 0.8)],
    );

    assert_eq!(runnable.len(), 2);
    assert_eq!(runnable[0].keys, names(&["3", "4"]));
    assert_eq!(runnable[1].keys, names(&["1", "2"]));
}

// ── respelling a reference (v2's SpammerOrderTests) ───────────────────────────────────────────

/// THE BUG THIS EXISTS FOR: the rows editor writes every key as `*name`, so saving a preset a
/// hand-edited config spelled bare renames its keys — and `priority`/`combos` name keys, so the
/// references were pruned against names that had merely changed shape.
#[test]
fn a_reference_follows_its_key_when_only_the_star_differs() {
    assert_eq!(
        respell(&names(&["F4", "F5"]), &names(&["*F4", "*F5"])),
        names(&["*F4", "*F5"])
    );
}

#[test]
fn a_starred_reference_follows_a_bare_key() {
    assert_eq!(respell(&names(&["*F4"]), &names(&["F4"])), names(&["F4"]));
}

/// A name that matches NEITHER spelling is left alone: "correcting" it would rewrite a typo into some
/// key that happens to exist.
#[test]
fn a_name_that_matches_nothing_is_left_alone() {
    assert_eq!(
        respell(&names(&["ghost", "F4"]), &names(&["*F4", "*F5"])),
        names(&["ghost", "*F4"])
    );
}

#[test]
fn a_name_already_spelled_right_is_unchanged() {
    assert_eq!(
        respell(&names(&["*1", "*2"]), &names(&["*1", "*2"])),
        names(&["*1", "*2"])
    );
}

// ── selecting a rotation (SkillSpammer.Run's head) ────────────────────────────────────────────

/// A missing active preset is refused with v2's message, and the two cases are told apart: no presets
/// at all is a different instruction from "the one you picked is gone".
#[test]
fn no_configured_preset_is_refused_with_its_own_message() {
    let stop = plan_or_stop(false, "default", None).unwrap_err();

    assert_eq!(
        stop.message,
        "No spammer presets are configured — add one in the Spammer tab."
    );
    assert_eq!(stop.kind, seal_domain::decision::StopKind::Error);
}

#[test]
fn a_missing_active_preset_names_it() {
    let stop = plan_or_stop(true, "nightly", None).unwrap_err();

    assert_eq!(
        stop.message,
        "The active preset 'nightly' was not found — pick one in the Spammer tab."
    );
}

/// An empty preset is the same refusal: v2's `ActiveKeys` is an empty map, and pressing a different
/// rotation silently would be worse than pressing nothing.
#[test]
fn an_empty_preset_is_refused_like_a_missing_one() {
    let stop = plan_or_stop(true, "empty", Some(rotation(&[], &[], vec![]))).unwrap_err();

    assert_eq!(
        stop.message,
        "The active preset 'empty' was not found — pick one in the Spammer tab."
    );
}

#[test]
fn the_plan_carries_the_walk_the_combos_and_the_preempt_flag() {
    let plan = SpammerPlan::new(rotation(
        &[("1", 5.0), ("2", 10.0), ("q", 3.0)],
        &["q"],
        vec![combo(&["1", "2"], 0.8)],
    ));

    assert_eq!(plan.combos.len(), 1);
    assert_eq!(plan.singles, names(&["q"]));
    assert!(plan.preempt);
    assert_eq!(plan.cooldown("2"), Some(10.0));
}

/// "No order given" is NOT "nothing outranks anything": an empty priority keeps the old
/// send-everything-due behaviour, which is why preemption is opt-in per preset.
#[test]
fn an_empty_priority_does_not_turn_preemption_on() {
    let plan = SpammerPlan::new(rotation(&[("q", 1.0)], &[], vec![]));

    assert!(!plan.preempt);
}

// ── the run: opening, lead-in, preemption, combos ────────────────────────────────────────────

fn plan(keys: &[(&str, f64)], priority: &[&str], combos: Vec<SpammerCombo>) -> SpammerPlan {
    SpammerPlan::new(rotation(keys, priority, combos))
}

#[test]
fn every_key_starts_due_so_a_fresh_run_casts_immediately() {
    let plan = plan(&[("q", 5.0), ("w", 10.0)], &[], vec![]);
    let mut run = SpammerRun::start(&plan, 0.0);

    let tick = run.tick(0.0);

    // No order means no preemption, so every due key goes out in the walk order in one tick.
    assert_eq!(tick.keys(), names(&["q", "w"]));
}

#[test]
fn a_pressed_key_waits_its_cooldown() {
    let plan = plan(&[("q", 5.0)], &[], vec![]);
    let mut run = SpammerRun::start(&plan, 0.0);

    assert_eq!(run.tick(0.0).keys(), names(&["q"]));
    assert!(run.tick(1.0).is_idle());
    assert!(run.tick(4.99).is_idle());
    assert_eq!(run.tick(5.0).keys(), names(&["q"]));
}

/// PREEMPT — one press per tick, the rest HELD. A due key that is outranked keeps its cooldown
/// untouched and goes out on a later tick, so it is delayed rather than skipped.
#[test]
fn preemption_presses_one_key_and_delays_the_rest() {
    let plan = plan(&[("q", 5.0), ("w", 10.0)], &["q"], vec![]);
    let mut run = SpammerRun::start(&plan, 0.0);

    assert_eq!(run.tick(0.0).keys(), names(&["q"]));
    // w is due but was held; q is now cooling, so w goes out on the next tick.
    assert_eq!(run.tick(0.02).keys(), names(&["w"]));
    assert!(run.tick(0.04).is_idle());
}

/// The lead-in is the window to put the game in front: nothing is pressed at all until it is over.
#[test]
fn the_lead_in_presses_nothing_until_it_is_over() {
    let mut plan = plan(&[("q", 5.0)], &[], vec![]);
    plan.startup.lead_in = 2.0;
    let mut run = SpammerRun::start(&plan, 0.0);

    assert!(run.tick(0.0).is_idle());
    assert!(run.tick(1.99).is_idle());
    assert_eq!(run.lead_in_remaining(0.0), Some(2.0));
    assert_eq!(run.lead_in_remaining(1.5), Some(0.5));
    assert_eq!(run.lead_in_remaining(2.0), None);
    assert_eq!(run.tick(2.0).keys(), names(&["q"]));
}

/// `startupStep` spaces the OPENING presses — due all at once, a rotation with animations fires its
/// whole opening into itself and the game swallows all but the first.
#[test]
fn the_opening_presses_are_staggered_by_the_startup_step() {
    let mut plan = plan(&[("q", 5.0), ("w", 5.0)], &[], vec![]);
    plan.startup.step = 1.0;
    let mut run = SpammerRun::start(&plan, 0.0);

    assert_eq!(run.tick(0.0).keys(), names(&["q"]));
    assert!(run.tick(0.5).is_idle());
    assert_eq!(run.tick(1.0).keys(), names(&["w"]));
}

/// A combo is the unit: its keys share one opening offset, and the combo's own gap spaces the presses
/// inside it. Nothing else is pressed while it drains — that silence is the point.
#[test]
fn a_combo_owns_the_tick_until_it_drains() {
    let plan = plan(
        &[("1", 5.0), ("2", 10.0), ("q", 3.0)],
        &[],
        vec![combo(&["1", "2"], 0.8)],
    );
    let mut run = SpammerRun::start(&plan, 0.0);

    // Tick 0 starts the combo and presses its first step. q is due but waits.
    assert_eq!(run.tick(0.0).keys(), names(&["1"]));
    // Mid-gap: nothing, not even the filler.
    assert!(run.tick(0.5).is_idle());
    // The gap elapses: the second step.
    assert_eq!(run.tick(0.8).keys(), names(&["2"]));
    // Combo drained; q is due and goes out.
    assert_eq!(run.tick(1.6).keys(), names(&["q"]));
}

/// A combo is ready only when EVERY key in it is off cooldown — starting a pair that cannot finish
/// would spend the first skill and miss the effect.
#[test]
fn a_combo_waits_for_its_last_key_to_come_off_cooldown() {
    let plan = plan(
        &[("1", 5.0), ("2", 10.0)],
        &[],
        vec![combo(&["1", "2"], 0.8)],
    );
    let mut run = SpammerRun::start(&plan, 0.0);

    assert_eq!(run.tick(0.0).keys(), names(&["1"]));
    assert_eq!(run.tick(0.8).keys(), names(&["2"]));
    // Key 1 is due but key 2 is not; the pair must not start.
    assert!(run.tick(5.0).is_idle());
    assert!(run.tick(10.0).is_idle());
    // Both are due: the pair restarts.
    assert_eq!(run.tick(10.8).keys(), names(&["1"]));
}

/// A fresh run does not pay the lead-in again when it is resumed from the gate — v2 re-opens through
/// the rotation's own rules (so expired cooldowns do not all fire at once) but not the positioning
/// window, because the game is already in front.
#[test]
fn resuming_staggers_again_but_does_not_pay_the_lead_in() {
    let mut plan = plan(&[("q", 5.0), ("w", 5.0)], &[], vec![]);
    plan.startup.lead_in = 30.0;
    plan.startup.step = 1.0;
    let mut run = SpammerRun::resume(&plan, 100.0);

    assert_eq!(run.lead_in_remaining(100.0), None);
    assert_eq!(run.tick(100.0).keys(), names(&["q"]));
    assert_eq!(run.tick(101.0).keys(), names(&["w"]));
}

// ── keys the firmware cannot send ────────────────────────────────────────────────────────────

#[test]
fn a_configured_key_resolves_to_the_boards_press() {
    let plan = plan(&[("*1", 5.0), ("F4", 5.0)], &[], vec![]);
    let mut run = SpammerRun::start(&plan, 0.0);
    let tick = run.tick(0.0);

    assert_eq!(
        tick.presses[0].board_press(),
        Some(BoardPress {
            key: seal_domain::decision::Key::character('1').unwrap(),
            fast: true
        })
    );
    assert_eq!(
        tick.presses[1].board_press(),
        Some(BoardPress {
            key: seal_domain::decision::Key::function(4).unwrap(),
            fast: false
        })
    );
}

/// v2 refuses an unsendable key at the board instead of sending it, and says so ONCE per key:
/// "Spammer key 'F13' unsupported — use one letter or digit, or F1–F12."
#[test]
fn an_unsendable_key_carries_the_warning_v2_shows() {
    let plan = plan(&[("F13", 5.0)], &[], vec![]);
    let mut run = SpammerRun::start(&plan, 0.0);
    let tick = run.tick(0.0);

    let press: &SpammerPress = &tick.presses[0];
    assert_eq!(press.board_press(), None);
    assert_eq!(
        press.unsupported_message().as_deref(),
        Some("Spammer key 'F13' unsupported — use one letter or digit, or F1–F12.")
    );
}

#[test]
fn the_warning_strips_the_fast_tap_star_like_v2_does() {
    assert_eq!(
        unsupported_key_message("*nope").as_deref(),
        Some("Spammer key 'nope' unsupported — use one letter or digit, or F1–F12.")
    );
    assert_eq!(unsupported_key_message("*1"), None);
    assert_eq!(unsupported_key_message("F12"), None);
    assert_eq!(unsupported_key_message("q"), None);
}

#[test]
fn a_stop_reason_is_available_for_a_dead_port() {
    let stop = StopReason::error("Arduino disconnected — stopped (the port is gone)");

    assert!(stop.message.starts_with("Arduino disconnected — stopped ("));
}

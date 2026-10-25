//! The Quest executor's decision half: what a run will execute, every pre-click refusal, the guard,
//! and the two ending lines.
//!
//! `QuestTool.cs` is the source; the step RULES it calls (`QuestAction.Complaint`) are already ported
//! in `authoring` and exercised by `tests/authoring_quest.rs`, so this file is about the plan and the
//! guard around them.

use seal_domain::config::{
    QuestConfig, QuestPreset, QuestSequence, QuestSequenceEntry, QuestStep, quest_action,
};
use seal_domain::decision::StopKind;
use seal_domain::geometry::Point;
use seal_domain::tools::quest::{
    QuestCommand, QuestPlan, QuestPlanKind, guard_problem, guard_reference, plan as build_plan,
    positioning_text, step_label, stopped_in_sequence, stopped_on_loop,
};

fn step(action: &str, value: Option<&str>, delay: f64) -> QuestStep {
    QuestStep {
        action: action.to_owned(),
        value: value.map(|value| value.to_owned()),
        delay_seconds: delay,
    }
}

fn preset(loops: i32, wait: f64, steps: Vec<QuestStep>) -> QuestPreset {
    QuestPreset {
        initial_wait_seconds: wait,
        loops,
        steps,
    }
}

fn entry(flow: &str, times: i32) -> QuestSequenceEntry {
    QuestSequenceEntry {
        preset: flow.to_owned(),
        times,
    }
}

fn sequence(loops: i32, wait: f64, entries: Vec<QuestSequenceEntry>) -> QuestSequence {
    QuestSequence {
        initial_wait_seconds: wait,
        loops,
        entries,
    }
}

/// A quest config with two usable flows and one two-entry sequence.
fn quest() -> QuestConfig {
    let mut quest = QuestConfig {
        active: "daily".to_owned(),
        active_sequence: String::new(),
        guard_px: 8,
        ..QuestConfig::default()
    };
    quest.presets.insert(
        "daily".to_owned(),
        preset(
            2,
            3.0,
            vec![
                step(quest_action::CLICK, None, 0.25),
                step(quest_action::KEY, Some("1"), 0.2),
                step(quest_action::WAIT, None, 0.5),
                step(quest_action::ENTER, None, 0.1),
            ],
        ),
    );
    quest.presets.insert(
        "nightly".to_owned(),
        preset(1, 5.0, vec![step(quest_action::RIGHT_CLICK, None, 0.3)]),
    );
    quest.sequences.insert(
        "all".to_owned(),
        sequence(2, 9.0, vec![entry("daily", 3), entry("nightly", 2)]),
    );
    quest
}

fn error_of(result: Result<QuestPlan, seal_domain::decision::StopReason>) -> (StopKind, String) {
    match result {
        Err(stop) => (stop.kind, stop.message),
        Ok(_) => panic!("expected the run to be refused"),
    }
}

// ── one flow ────────────────────────────────────────────────────────────────────────────────

#[test]
fn one_flow_plans_its_schedule_positioning_loops_and_steps() {
    let plan = build_plan(&quest(), None, None, None).unwrap();

    assert_eq!(
        plan.kind,
        QuestPlanKind::Flow {
            name: "daily".to_owned()
        }
    );
    assert_eq!(plan.schedule, "flow \"daily\" · 4 step(s) × 2 loop(s)");
    assert_eq!(plan.positioning_seconds, 3.0);
    assert_eq!(plan.loops, 2);
    assert_eq!(plan.guard_px, 8);
    assert_eq!(plan.passes.len(), 1);
    assert_eq!(plan.passes[0].label, "");
    assert_eq!(plan.passes[0].flow, "daily");

    let steps = &plan.passes[0].steps;
    assert_eq!(steps[0].command, QuestCommand::Click);
    assert_eq!(steps[0].wait_after_seconds, 0.25);
    assert_eq!(steps[0].label, "click (0.25s)");
    assert_eq!(steps[1].command, QuestCommand::Key('1'));
    assert_eq!(steps[1].label, "key 1 (0.2s)");
    assert_eq!(steps[2].command, QuestCommand::NoInput);
    assert_eq!(steps[2].label, "wait (0.5s)");
    assert_eq!(steps[3].command, QuestCommand::Enter);
    assert_eq!(steps[3].label, "enter (0.1s)");
}

/// Test one loop passes `loops_override = Some(1)` and runs the SAME plan — which is why the test
/// cannot drift from a real run.
#[test]
fn the_loops_override_wins_for_a_flow() {
    let plan = build_plan(&quest(), None, None, Some(1)).unwrap();
    assert_eq!(plan.loops, 1);
    assert_eq!(plan.schedule, "flow \"daily\" · 4 step(s) × 1 loop(s)");
}

/// A flow's own loop count is clamped to at least one; an override is taken as given (v2 passes the
/// tab's Test-one-loop 1 through unchanged).
#[test]
fn a_flows_loop_count_is_at_least_one_but_an_override_is_not_clamped() {
    let mut quest = quest();
    quest.presets.insert(
        "daily".to_owned(),
        preset(0, 3.0, vec![step(quest_action::CLICK, None, 0.25)]),
    );
    assert_eq!(build_plan(&quest, None, None, None).unwrap().loops, 1);

    assert_eq!(build_plan(&quest, None, None, Some(0)).unwrap().loops, 0);
}

#[test]
fn a_flow_with_no_steps_is_refused() {
    let mut quest = quest();
    quest
        .presets
        .insert("daily".to_owned(), preset(1, 3.0, Vec::new()));

    let (kind, message) = error_of(build_plan(&quest, None, None, None));
    assert_eq!(kind, StopKind::Error);
    assert_eq!(
        message,
        "The flow \"daily\" has no steps — add some on the Quest tab."
    );
}

/// A step that cannot be sent is refused BEFORE anything is clicked, and the refusal names the step
/// number the player has to go and fix.
#[test]
fn an_unsendable_step_is_refused_with_its_number() {
    let mut quest = quest();
    quest.presets.insert(
        "daily".to_owned(),
        preset(
            1,
            3.0,
            vec![
                step(quest_action::CLICK, None, 0.25),
                step(quest_action::KEY, None, 0.2),
            ],
        ),
    );

    let (_, message) = error_of(build_plan(&quest, None, None, None));
    assert_eq!(
        message,
        "Flow \"daily\" step 2: a key step with no character would send nothing at all."
    );
}

/// A missing active flow is the empty-preset case v2 had: the refusal is the "no steps" line, not a
/// different flow's actions.
#[test]
fn a_missing_active_flow_is_refused_as_an_empty_flow() {
    let mut quest = quest();
    quest.active = "gone".to_owned();

    let (_, message) = error_of(build_plan(&quest, None, None, None));
    assert_eq!(
        message,
        "The flow \"gone\" has no steps — add some on the Quest tab."
    );
}

// ── a sequence ──────────────────────────────────────────────────────────────────────────────

#[test]
fn a_sequence_plans_one_positioning_window_and_a_flattened_pass_list() {
    let plan = build_plan(&quest(), Some("all"), None, None).unwrap();

    assert_eq!(
        plan.kind,
        QuestPlanKind::Sequence {
            name: "all".to_owned()
        }
    );
    assert_eq!(
        plan.schedule,
        "sequence \"all\" · 2 × (daily×3 + nightly×2)"
    );
    // ONE positioning window, from the sequence: a flow's own wait means "when I am run alone".
    assert_eq!(plan.positioning_seconds, 9.0);
    assert_eq!(plan.loops, 2);

    let flows: Vec<&str> = plan.passes.iter().map(|pass| pass.flow.as_str()).collect();
    assert_eq!(flows, vec!["daily", "daily", "daily", "nightly", "nightly"]);
    assert_eq!(plan.passes[0].label, "daily 1/3 · ");
    assert_eq!(plan.passes[3].label, "nightly 1/2 · ");
}

/// Every entry and every flow it names is checked BEFORE anything is clicked: a sequence is longer
/// than a flow, so a mistake in it costs more.
#[test]
fn a_sequence_with_no_entries_is_refused() {
    let mut quest = quest();
    quest
        .sequences
        .insert("all".to_owned(), sequence(1, 9.0, Vec::new()));

    let (_, message) = error_of(build_plan(&quest, Some("all"), None, None));
    assert_eq!(
        message,
        "The sequence \"all\" has no entries — add some on the Quest tab."
    );
}

#[test]
fn a_sequence_naming_a_missing_flow_is_refused_by_name() {
    let mut quest = quest();
    quest
        .sequences
        .insert("all".to_owned(), sequence(1, 9.0, vec![entry("gone", 1)]));

    let (_, message) = error_of(build_plan(&quest, Some("all"), None, None));
    assert_eq!(
        message,
        "The sequence \"all\" names a flow \"gone\" that does not exist."
    );
}

#[test]
fn an_invalid_flow_inside_a_sequence_is_refused_before_the_run() {
    let mut quest = quest();
    quest
        .presets
        .insert("broken".to_owned(), preset(1, 0.0, Vec::new()));
    quest
        .sequences
        .insert("all".to_owned(), sequence(1, 9.0, vec![entry("broken", 1)]));

    let (_, message) = error_of(build_plan(&quest, Some("all"), None, None));
    assert_eq!(
        message,
        "The flow \"broken\" has no steps — add some on the Quest tab."
    );
}

/// A sequence's times and master loops are clamped to at least one.
#[test]
fn a_sequences_times_and_master_loops_are_at_least_one() {
    let mut quest = quest();
    quest
        .sequences
        .insert("all".to_owned(), sequence(0, 9.0, vec![entry("daily", 0)]));

    let plan = build_plan(&quest, Some("all"), None, None).unwrap();
    assert_eq!(plan.loops, 1);
    assert_eq!(plan.passes.len(), 1);
    // The loops override is clamped for a sequence too (v2's `Math.Max(1, ...)`).
    assert_eq!(
        build_plan(&quest, Some("all"), None, Some(0))
            .unwrap()
            .loops,
        1
    );
}

// ── what the card chose vs what the tab holds ──────────────────────────────────────────────

#[test]
fn a_card_sequence_choice_that_is_gone_is_refused_with_v2s_line() {
    let (_, message) = error_of(build_plan(&quest(), Some("gone"), None, None));
    assert_eq!(
        message,
        "There is no sequence called \"gone\" — pick another on the card, or reload the config."
    );
}

#[test]
fn a_card_flow_choice_that_is_gone_is_refused_with_v2s_line() {
    let (_, message) = error_of(build_plan(&quest(), None, Some("gone"), None));
    assert_eq!(
        message,
        "There is no flow called \"gone\" — pick another on the card, or reload the config."
    );
}

/// The card's sequence choice outranks the card's flow choice AND the tab; a blank choice counts as
/// absent (`IsNullOrWhiteSpace`).
#[test]
fn the_card_choice_outranks_the_tab_and_a_blank_choice_is_absent() {
    let plan = build_plan(&quest(), Some("all"), Some("nightly"), None).unwrap();
    assert!(matches!(plan.kind, QuestPlanKind::Sequence { .. }));

    let plan = build_plan(&quest(), Some("  "), Some("nightly"), None).unwrap();
    assert_eq!(
        plan.kind,
        QuestPlanKind::Flow {
            name: "nightly".to_owned()
        }
    );
}

/// The tab's active sequence is used when one is chosen; an empty OR unknown name falls back to the
/// single flow, which is the shape of every config written before sequences existed.
#[test]
fn the_tabs_active_sequence_is_used_or_falls_back_to_the_single_flow() {
    let mut quest = quest();
    quest.active_sequence = "all".to_owned();
    assert!(matches!(
        build_plan(&quest, None, None, None).unwrap().kind,
        QuestPlanKind::Sequence { .. }
    ));

    quest.active_sequence = "nope".to_owned();
    assert!(matches!(
        build_plan(&quest, None, None, None).unwrap().kind,
        QuestPlanKind::Flow { .. }
    ));

    quest.active_sequence = "  ".to_owned();
    assert!(matches!(
        build_plan(&quest, None, None, None).unwrap().kind,
        QuestPlanKind::Flow { .. }
    ));
}

// ── the guard ───────────────────────────────────────────────────────────────────────────────

/// The guard's reference is read STRICTLY: an unreadable cursor ends the run rather than clicking on
/// at a place nothing is watching.
#[test]
fn an_unreadable_reference_cursor_stops_the_run_before_anything_is_clicked() {
    let stop = guard_reference(None).unwrap_err();
    assert_eq!(stop.kind, StopKind::Error);
    assert_eq!(
        stop.message,
        "The cursor couldn't be read, so it can't be watched — stopping rather than clicking at a \
         place nothing is checking."
    );
    assert_eq!(
        guard_reference(Some(Point { x: 10, y: 20 })).unwrap(),
        Point { x: 10, y: 20 }
    );
}

/// The drift is the larger axis, in the same logical space as the reference, and it is `>` — a
/// cursor exactly guard_px away has not moved.
#[test]
fn a_cursor_that_moved_more_than_the_guard_distance_stops_the_run() {
    let start = Point { x: 100, y: 100 };

    assert_eq!(guard_problem(start, Some(Point { x: 108, y: 92 }), 8), None);

    let stop = guard_problem(start, Some(Point { x: 109, y: 100 }), 8).unwrap();
    assert_eq!(stop.kind, StopKind::Guard);
    assert_eq!(
        stop.message,
        "The mouse moved — it is at (109,100) and started at (100,100)."
    );
}

#[test]
fn a_cursor_that_stops_being_readable_ends_the_run() {
    let stop = guard_problem(Point { x: 1, y: 2 }, None, 8).unwrap();
    assert_eq!(
        stop.message,
        "The cursor stopped being readable, so it can't be watched any more."
    );
}

// ── the messages a run ends with ───────────────────────────────────────────────────────────

#[test]
fn the_positioning_countdown_counts_whole_seconds_up() {
    let plan = build_plan(&quest(), None, None, None).unwrap();
    assert_eq!(plan.positioning_counts(), vec![3, 2, 1]);
    assert_eq!(positioning_text(3), "move the mouse into place — 3s");

    let mut quest = quest();
    quest.presets.insert(
        "daily".to_owned(),
        preset(1, 2.3, vec![step(quest_action::CLICK, None, 0.25)]),
    );
    assert_eq!(
        build_plan(&quest, None, None, None)
            .unwrap()
            .positioning_counts(),
        vec![3, 2, 1]
    );

    quest.presets.insert(
        "daily".to_owned(),
        preset(1, 0.0, vec![step(quest_action::CLICK, None, 0.25)]),
    );
    assert!(
        build_plan(&quest, None, None, None)
            .unwrap()
            .positioning_counts()
            .is_empty()
    );
}

#[test]
fn a_completed_run_reports_v2s_done_line() {
    assert_eq!(
        build_plan(&quest(), None, None, None)
            .unwrap()
            .done_message(),
        "DONE — 2 loop(s) of \"daily\""
    );
    assert_eq!(
        build_plan(&quest(), Some("all"), None, None)
            .unwrap()
            .done_message(),
        "DONE — 2 master loop(s) of \"all\""
    );
}

/// A guard stop keeps the guard's sentence and appends where the run was — the two v2 lines, one for
/// the single flow and one for the sequence.
#[test]
fn a_stopped_run_says_how_far_it_got() {
    let reason = "The mouse moved — it is at (9,9) and started at (0,0).";

    assert_eq!(
        stopped_on_loop(reason, 2, 5),
        "The mouse moved — it is at (9,9) and started at (0,0). Stopped on loop 2 of 5."
    );
    assert_eq!(
        stopped_in_sequence(reason, "daily", 2, 3, 1, 4),
        "The mouse moved — it is at (9,9) and started at (0,0). Stopped during daily 2/3 of master \
         loop 1/4."
    );
}

#[test]
fn step_labels_are_v2s_short_forms() {
    assert_eq!(
        step_label(&step(quest_action::RIGHT_CLICK, None, 0.3)),
        "right click (0.3s)"
    );
    // The delay is "0.##": 0.25 keeps both decimals, 1 prints as "1".
    assert_eq!(
        step_label(&step(quest_action::CLICK, None, 0.25)),
        "click (0.25s)"
    );
    assert_eq!(
        step_label(&step(quest_action::CLICK, None, 1.0)),
        "click (1s)"
    );
}

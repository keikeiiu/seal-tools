//! The tuner's decision half: what one OCR read means, what stops the run, and what the two guard
//! paths do.
//!
//! `GradeCyclesTests.cs` is the v2 oracle for the per-grade tally and is translated line for line.
//! The stop conditions come from `SealTuner.Run`'s run loop, which v2 could not test without a serial
//! port and a game window — the messages below are that loop's own log lines, ported verbatim
//! (`PROTOCOL.md` §5: a finished tool's message is the one v2 writes to its log).

use seal_domain::config::{FilterConfig, FilterRule};
use seal_domain::decision::{StopKind, StopReason};
use seal_domain::geometry::Point;
use seal_domain::ocr::MatchedAttr;
use seal_domain::tools::tuner::{
    GradeCycles, MouseGuard, MouseGuardStep, START_COUNTDOWN, SpringMode, SpringStep,
    TunerDecision, TunerOutcome, TunerRead, TunerRun, TunerSettings, UNCONFIRMED_READ_MESSAGE,
};

fn settings() -> TunerSettings {
    TunerSettings {
        grade_order: ["N", "G", "DG"].iter().map(|g| (*g).to_owned()).collect(),
        target_grade: "DG".to_owned(),
        max_retries: 100,
        spring_mode: SpringMode::Manual,
        mouse_guard: MouseGuard::Off,
        guard_px: 8,
        recenter_max: 20,
        filter: FilterConfig::default(),
    }
}

/// A CONFIRMED read: the decision half refuses PARTIAL reads now, so the default fixture carries the
/// three named attribute rows a real scan produces.
fn read(grade: Option<&str>, remaining: Option<i32>) -> TunerRead {
    TunerRead {
        grade: grade.map(std::borrow::ToOwned::to_owned),
        remaining,
        attributes: vec![
            attr("STR", Some(1)),
            attr("DEX", Some(1)),
            attr("INT", Some(1)),
        ],
    }
}

/// A PARTIAL read: grade and spring count were read but one attribute row was lost — the issue #16
/// shape that used to be decided on and counted as a repeat.
fn partial_read() -> TunerRead {
    TunerRead {
        grade: Some("G".to_owned()),
        remaining: Some(9),
        attributes: vec![attr("STR", Some(1)), attr("DEX", Some(1))],
    }
}

fn attr(name: &str, value: Option<i32>) -> MatchedAttr {
    MatchedAttr {
        name: name.to_owned(),
        category: String::new(),
        value,
        extra: None,
        raw: Vec::new(),
    }
}

fn rule(name: &str, count: i32, min: Option<i32>, max: Option<i32>) -> FilterRule {
    FilterRule {
        name: name.to_owned(),
        count,
        min,
        max,
    }
}

// ── the per-grade tally (`GradeCyclesTests`, translated) ─────────────────────────────────────

/// The grades the composer runs them in.
fn grades() -> Vec<String> {
    ["N", "G", "DG"].iter().map(|g| (*g).to_owned()).collect()
}

#[test]
fn every_cycle_is_counted_against_the_grade_it_ran_on() {
    let mut cycles = GradeCycles::new();

    cycles.add("N");
    cycles.add("N");
    cycles.add("G");
    cycles.add("N");

    assert_eq!(cycles.count_for("N"), 3);
    assert_eq!(cycles.count_for("G"), 1);
    assert_eq!(cycles.count_for("DG"), 0);
}

#[test]
fn a_new_run_starts_from_zero() {
    let mut cycles = GradeCycles::new();
    cycles.add("N");
    cycles.add("G");

    cycles.reset();

    assert_eq!(cycles.count_for("N"), 0);
    assert_eq!(cycles.count_for("G"), 0);
}

#[test]
fn the_card_shows_a_row_per_grade_in_run_order() {
    let mut cycles = GradeCycles::new();
    cycles.add("N");
    cycles.add("N");
    cycles.add("G");

    let rows = cycles.rows(&grades());

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].name, "N");
    assert_eq!(rows[0].status, "2 cycles");
    // Singular, because "1 cycles" reads as a bug.
    assert_eq!(rows[1].status, "1 cycle");
    assert_eq!(rows[2].status, "0 cycles");
}

#[test]
fn the_summary_line_carries_every_grade_and_the_total() {
    let mut cycles = GradeCycles::new();
    for _ in 0..12 {
        cycles.add("N");
    }
    for _ in 0..8 {
        cycles.add("G");
    }
    for _ in 0..3 {
        cycles.add("DG");
    }

    assert_eq!(cycles.summary(&grades()), "N 12 · G 8 · DG 3 · 23 total");
}

#[test]
fn a_grade_the_run_never_reached_still_appears_at_zero() {
    let mut cycles = GradeCycles::new();
    cycles.add("N");

    assert_eq!(cycles.summary(&grades()), "N 1 · G 0 · DG 0 · 1 total");
}

#[test]
fn the_total_comes_from_the_grades_so_the_two_cannot_disagree() {
    let mut cycles = GradeCycles::new();
    cycles.add("N");
    cycles.add("DG");

    assert_eq!(cycles.total(&grades()), 2);
}

// ── one read, one decision ──────────────────────────────────────────────────────────────────

#[test]
fn reaching_the_target_grade_with_no_filter_ends_the_run() {
    let mut run = TunerRun::new(settings());

    let outcome = run.decide(&read(Some("DG"), Some(50)));

    let stop = match outcome.decision {
        TunerDecision::Stop(stop) => stop,
        TunerDecision::Retry => panic!("expected a stop, got a retry"),
    };
    assert_eq!(stop.kind, StopKind::Done);
    assert_eq!(stop.message, ">>> DG REACHED (no filter) <<<");
}

/// A read whose attribute satisfies a filter rule: the fixture both filter-passing tests use.
fn read_matching_the_filter(grade: &str, remaining: i32) -> TunerRead {
    let mut read = read(Some(grade), Some(remaining));
    read.attributes = vec![
        attr("增加傷害", Some(1)),
        attr("STR", Some(1)),
        attr("DEX", Some(1)),
    ];
    read
}

fn passing_filter() -> FilterConfig {
    FilterConfig {
        enabled: true,
        match_mode: "any".to_owned(),
        require_grade: None,
        rules: vec![rule("增加傷害", 1, None, None)],
        override_rules: Vec::new(),
    }
}

#[test]
fn reaching_the_target_grade_with_a_passing_filter_says_so() {
    let mut settings = settings();
    settings.filter = passing_filter();
    let mut run = TunerRun::new(settings);

    let stop = stop_of(run.decide(&read_matching_the_filter("DG", 50)));

    assert_eq!(stop.message, ">>> DG REACHED + FILTER MATCH <<<");
}

/// The filter can be the only stop condition: `require_grade` absent means "any grade the filter
/// passes", and that branch exists because a player may want an attribute roll rather than a grade.
#[test]
fn a_passing_filter_without_a_grade_requirement_ends_the_run() {
    let mut settings = settings();
    settings.filter = passing_filter();
    let mut run = TunerRun::new(settings);

    let stop = stop_of(run.decide(&read_matching_the_filter("N", 50)));

    assert_eq!(
        stop.message,
        ">>> FILTER MATCHED at grade N (no grade requirement) <<<"
    );
}

/// A grade below the target with a failing filter is a retry, not a stop.
#[test]
fn a_grade_below_target_while_the_filter_fails_continues() {
    let mut settings = settings();
    settings.filter.enabled = true;
    let mut run = TunerRun::new(settings);

    assert_eq!(
        run.decide(&read(Some("G"), Some(50))).decision,
        TunerDecision::Retry
    );
}

/// With `require_grade` set, a passing filter is not enough on its own — the grade has to reach it.
#[test]
fn a_passing_filter_below_the_required_grade_continues() {
    let mut settings = settings();
    settings.filter = passing_filter();
    settings.filter.require_grade = Some("DG".to_owned());
    let mut run = TunerRun::new(settings);

    assert_eq!(
        run.decide(&read_matching_the_filter("G", 50)).decision,
        TunerDecision::Retry
    );
}

/// The filter's `require_grade` replaces `target_grade` when it names a grade — unless it is the
/// literal `"false"` a hand-edited config may carry.
#[test]
fn a_require_grade_of_false_falls_back_to_the_target() {
    let mut settings = settings();
    settings.filter.require_grade = Some("false".to_owned());
    let mut run = TunerRun::new(settings);

    let stop = stop_of(run.decide(&read(Some("DG"), Some(50))));
    assert_eq!(stop.message, ">>> DG REACHED (no filter) <<<");
}

/// A stopped run records the filter's sentence for the card, or "disabled" when it is off.
#[test]
fn the_filter_status_is_the_reason_or_disabled() {
    let mut settings = settings();
    settings.filter.enabled = true;
    settings.filter.rules = vec![rule("增加傷害", 1, None, None)];
    let mut run = TunerRun::new(settings);

    let outcome = run.decide(&read(Some("N"), Some(50)));
    assert_eq!(outcome.filter_status(), "no rules matched");

    let mut off = TunerRun::new(self::settings());
    assert_eq!(
        off.decide(&read(Some("N"), Some(50))).filter_status(),
        "disabled"
    );
}

/// A scan that recognised NOTHING is a failure, not a result to compare: it would otherwise count as
/// the first repeat of the empty signature the run starts with and trip "same result x3" after only
/// two failed reads.
#[test]
fn a_total_read_failure_stops_and_does_not_count_as_a_repeat() {
    let mut run = TunerRun::new(settings());

    let stop = stop_of(run.decide(&TunerRead::default()));

    assert_eq!(stop.kind, StopKind::Error);
    assert_eq!(
        stop.message,
        "Read failed — nothing recognised in the OCR region."
    );
    assert_eq!(run.consecutive_repeats(), 0);
}

/// Issue #16: after `tuner.ocr_retries` the core hands the decision half a PARTIAL read; deciding on
/// it and counting repeats produced "same result x3" while springs were still on the panel. The first
/// partial read stops with the explicit line, before any repeat is counted.
#[test]
fn a_partial_read_stops_with_the_confirming_error_and_never_reaches_the_repeat_detector() {
    let mut run = TunerRun::new(settings());
    let partial = partial_read();

    let stop = stop_of(run.decide(&partial));

    assert_eq!(stop.kind, StopKind::Error);
    assert_eq!(stop.message, UNCONFIRMED_READ_MESSAGE);
    assert_eq!(
        run.consecutive_repeats(),
        0,
        "a partial read is not a repeat"
    );
    // Repeating the SAME partial read cannot build a chain either: every one is refused.
    for _ in 0..2 {
        assert_eq!(
            stop_of(run.decide(&partial)).message,
            UNCONFIRMED_READ_MESSAGE
        );
    }
    assert_eq!(run.consecutive_repeats(), 0);
}

/// The domain's half of v2's `IsConfirmed` (it lives here with the check): grade, spring count and
/// three named attribute rows are all required; every missing piece is a partial read.
#[test]
fn a_read_is_confirmed_only_with_grade_every_row_and_remaining() {
    let complete = read(Some("DG"), Some(100));
    assert!(complete.is_confirmed());
    let mut missing_remaining = complete.clone();
    missing_remaining.remaining = None;
    assert!(!missing_remaining.is_confirmed());
    let mut two_rows = complete;
    two_rows.attributes.truncate(2);
    assert!(!two_rows.is_confirmed());
    let mut unnamed = read(Some("DG"), Some(100));
    unnamed.attributes[0].name.clear();
    assert!(!unnamed.is_confirmed());
}

/// Three identical results in a row is a stuck game: the same springs never came back.
#[test]
fn three_identical_results_stop_the_run() {
    let mut run = TunerRun::new(settings());

    assert_eq!(
        run.decide(&read(Some("G"), Some(50))).decision,
        TunerDecision::Retry
    );
    assert_eq!(
        run.decide(&read(Some("G"), Some(50))).decision,
        TunerDecision::Retry
    );
    let stop = stop_of(run.decide(&read(Some("G"), Some(50))));
    assert_eq!(
        stop.message,
        ">>> STUCK / OUT OF SPRINGS (same result x3) <<<"
    );
}

/// The repeat signature is grade + remaining + the attribute line, so a changed number is NOT a
/// repeat even when the grade is the same.
#[test]
fn a_changed_attribute_value_breaks_the_repeat_chain() {
    let mut run = TunerRun::new(settings());
    let mut first = read(Some("G"), Some(50));
    first.attributes = vec![
        attr("增加傷害", Some(1)),
        attr("STR", Some(1)),
        attr("DEX", Some(1)),
    ];
    let mut changed = read(Some("G"), Some(50));
    changed.attributes = vec![
        attr("增加傷害", Some(2)),
        attr("STR", Some(1)),
        attr("DEX", Some(1)),
    ];

    assert_eq!(run.decide(&first).decision, TunerDecision::Retry);
    assert_eq!(run.decide(&changed).decision, TunerDecision::Retry);
    assert_eq!(run.consecutive_repeats(), 0);
}

/// Out of springs is its own stop, and it is checked BEFORE the repeat counter — the run that just
/// spent its last spring should say so rather than "stuck".
#[test]
fn running_out_of_springs_is_reported_as_its_own_stop() {
    let mut run = TunerRun::new(settings());
    run.begin_attempt();

    let stop = stop_of(run.decide(&read(Some("G"), Some(0))));
    assert_eq!(stop.message, ">>> OUT OF SPRINGS <<<");

    let mut negative = TunerRun::new(settings());
    negative.begin_attempt();
    assert_eq!(
        stop_of(negative.decide(&read(Some("G"), Some(-1)))).message,
        ">>> OUT OF SPRINGS <<<"
    );

    // A missing remaining count is not "zero": the read is PARTIAL, so it stops with the confirming
    // line rather than claiming the springs ran out.
    let mut unknown = TunerRun::new(settings());
    unknown.begin_attempt();
    let missing = stop_of(unknown.decide(&read(Some("G"), None)));
    assert_eq!(missing.kind, StopKind::Error);
    assert_eq!(missing.message, UNCONFIRMED_READ_MESSAGE);
    assert_ne!(missing.message, ">>> OUT OF SPRINGS <<<");
}

/// Reaching the target outranks running out of springs: the read is the answer, whatever is left.
#[test]
fn reaching_the_target_grade_outranks_the_spring_count() {
    let mut run = TunerRun::new(settings());

    let stop = stop_of(run.decide(&read(Some("DG"), Some(0))));
    assert_eq!(stop.message, ">>> DG REACHED (no filter) <<<");
}

#[test]
fn the_retry_ceiling_stops_the_run() {
    let mut settings = settings();
    settings.max_retries = 2;
    let mut run = TunerRun::new(settings);

    run.begin_attempt();
    assert_eq!(
        run.decide(&read(Some("G"), Some(50))).decision,
        TunerDecision::Retry
    );
    run.begin_attempt();
    let stop = stop_of(run.decide(&read(Some("G"), Some(50))));
    assert_eq!(stop.message, "[!] Max 2");
}

/// A grade name that is in neither the order nor the target list is the v2 quirk: both indexes are
/// -1, and `-1 >= -1`, so the read counts as a reached target. Pinned so a "cleanup" has to argue
/// with the oracle rather than silently change when the tuner stops.
#[test]
fn an_unknown_grade_name_counts_as_reached_when_the_target_is_also_unknown() {
    let mut settings = settings();
    settings.grade_order = Vec::new();
    settings.target_grade = "DG".to_owned();
    let mut run = TunerRun::new(settings);

    let stop = stop_of(run.decide(&read(Some("mystery"), Some(50))));
    assert_eq!(stop.message, ">>> mystery REACHED (no filter) <<<");
}

#[test]
fn the_countdown_length_is_v2s() {
    assert_eq!(START_COUNTDOWN, 5);
}

// ── the mouse guard ─────────────────────────────────────────────────────────────────────────

fn guard_settings(mode: MouseGuard) -> TunerSettings {
    TunerSettings {
        spring_mode: SpringMode::Hid,
        mouse_guard: mode,
        guard_px: 8,
        recenter_max: 20,
        ..settings()
    }
}

const SPRING: Point = Point { x: 300, y: 400 };

#[test]
fn a_guard_that_is_off_never_looks_at_the_cursor() {
    let settings = guard_settings(MouseGuard::Off);
    let step = seal_domain::tools::tuner::mouse_guard_step(
        &settings,
        Some(SPRING),
        Some(Point { x: 999, y: 999 }),
        0,
    );

    assert_eq!(step, MouseGuardStep::Proceed { drift_free: false });
}

/// ACKNOWLEDGED FAIL-OPEN: no calibrated spring point, or a window that has gone away, turns the
/// guard OFF rather than tripping it. The failure is recorded in v2's backlog, not fixed.
#[test]
fn the_guard_cannot_judge_without_a_point_or_a_cursor_and_fails_open() {
    let settings = guard_settings(MouseGuard::Stop);

    assert_eq!(
        seal_domain::tools::tuner::mouse_guard_step(&settings, None, Some(SPRING), 0),
        MouseGuardStep::Proceed { drift_free: false }
    );
    assert_eq!(
        seal_domain::tools::tuner::mouse_guard_step(&settings, Some(SPRING), None, 0),
        MouseGuardStep::Proceed { drift_free: false }
    );
}

/// The drift is measured in the LOGICAL cursor space against the harness's logical target, and it is
/// `>` the guard distance, not `>=` — a cursor sitting exactly `guard_px` away has not drifted.
#[test]
fn a_cursor_within_the_guard_distance_has_not_drifted() {
    let settings = guard_settings(MouseGuard::Stop);

    assert_eq!(
        seal_domain::tools::tuner::mouse_guard_step(
            &settings,
            Some(SPRING),
            Some(Point { x: 308, y: 392 }),
            0
        ),
        MouseGuardStep::Proceed { drift_free: true }
    );

    let stop = seal_domain::tools::tuner::mouse_guard_step(
        &settings,
        Some(SPRING),
        Some(Point { x: 309, y: 400 }),
        0,
    );
    assert_eq!(
        stop,
        MouseGuardStep::Stop(StopReason::guard("mouse moved — stopped"))
    );
}

#[test]
fn stop_mode_halts_on_the_first_drift_with_its_message() {
    let settings = guard_settings(MouseGuard::Stop);
    let step = seal_domain::tools::tuner::mouse_guard_step(
        &settings,
        Some(SPRING),
        Some(Point { x: 300, y: 500 }),
        0,
    );

    let MouseGuardStep::Stop(halt) = step else {
        panic!("expected a stop, got {step:?}");
    };
    assert_eq!(halt.kind, StopKind::Guard);
    assert_eq!(halt.message, "mouse moved — stopped");
}

#[test]
fn recenter_mode_puts_the_cursor_back_and_keeps_going() {
    let settings = guard_settings(MouseGuard::Recenter);
    let step = seal_domain::tools::tuner::mouse_guard_step(
        &settings,
        Some(SPRING),
        Some(Point { x: 300, y: 500 }),
        0,
    );

    assert_eq!(step, MouseGuardStep::Recenter);
}

/// A recentre that fails is not "carry on": the message says the cursor would not move.
#[test]
fn a_failed_recentre_stops_with_the_reason() {
    let settings = guard_settings(MouseGuard::Recenter);

    let step = seal_domain::tools::tuner::after_recenter(&settings, Err("no such window"), 1);

    let MouseGuardStep::Stop(halt) = step else {
        panic!("expected a stop, got {step:?}");
    };
    assert_eq!(halt.message, "cursor wouldn't re-centre — no such window");
}

/// A cursor that has to be put back more than `recenter_max` times in a row is a fight the run should
/// not keep having.
#[test]
fn a_runaway_recentre_count_stops_the_run() {
    let settings = guard_settings(MouseGuard::Recenter);

    assert_eq!(
        seal_domain::tools::tuner::after_recenter(&settings, Ok(()), 20),
        MouseGuardStep::Proceed { drift_free: false }
    );
    let step = seal_domain::tools::tuner::after_recenter(&settings, Ok(()), 21);
    assert_eq!(
        step,
        MouseGuardStep::Stop(StopReason::guard("cursor keeps drifting — stopped"))
    );
}

/// The guard is compared against the strings the config holds; anything that is not `off` or `stop`
/// takes the `recenter` branch, exactly as v2's if/else does.
#[test]
fn the_guard_modes_parse_the_way_v2_compares_them() {
    assert_eq!(MouseGuard::parse("off"), MouseGuard::Off);
    assert_eq!(MouseGuard::parse("stop"), MouseGuard::Stop);
    assert_eq!(MouseGuard::parse("recenter"), MouseGuard::Recenter);
    assert_eq!(MouseGuard::parse("anything else"), MouseGuard::Recenter);
    assert_eq!(SpringMode::parse("hid"), SpringMode::Hid);
    assert_eq!(SpringMode::parse("manual"), SpringMode::Manual);
    assert_eq!(SpringMode::parse("HID"), SpringMode::Manual);
}

// ── placing the cursor on the spring (spring_mode: hid) ─────────────────────────────────────

#[test]
fn manual_spring_mode_needs_no_placement() {
    let step = seal_domain::tools::tuner::spring_step(&settings(), None);
    assert_eq!(step, SpringStep::NotNeeded);
}

/// `hid` mode with no calibrated point must refuse rather than click blind.
#[test]
fn hid_mode_without_a_calibrated_point_refuses_with_the_instruction() {
    let settings = guard_settings(MouseGuard::Off);
    let step = seal_domain::tools::tuner::spring_step(&settings, None);

    let SpringStep::Stop(halt) = step else {
        panic!("expected a stop, got {step:?}");
    };
    assert_eq!(
        halt.message,
        "spring_mode is hid but tuner.spring_point isn't calibrated — save the Spring point on the Calibrate tab"
    );
}

#[test]
fn hid_mode_places_the_cursor_at_the_calibrated_point() {
    let settings = guard_settings(MouseGuard::Off);
    let step = seal_domain::tools::tuner::spring_step(&settings, Some(SPRING));

    assert_eq!(step, SpringStep::Place { point: SPRING });
}

#[test]
fn the_two_placement_failures_carry_v2s_messages() {
    assert_eq!(
        seal_domain::tools::tuner::spring_window_missing().message,
        "game window not found — couldn't place the cursor on the spring"
    );
    assert_eq!(
        seal_domain::tools::tuner::spring_failed("5 steps, ended at (1,2)").message,
        "couldn't place the cursor on the spring — 5 steps, ended at (1,2)"
    );
}

fn stop_of(outcome: TunerOutcome) -> StopReason {
    match outcome.decision {
        TunerDecision::Stop(stop) => stop,
        TunerDecision::Retry => panic!("expected a stop, got a retry"),
    }
}

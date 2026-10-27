//! The pet visit decisions, translated from `FeederCountTests.cs`, `PetFeedingTests.cs`, and the
//! schedule/veto/fold/card/Ready logic of `PetTool.cs`.
//!
//! These are the messages and rules the resident feeder runs on, and the ones the Lead asked to have
//! ported without truncation: a run of this tool is unattended for days, so a wrong schedule or a
//! swallowed message is invisible until a pet has gone unfed.

use seal_domain::decision::StopKind;
use seal_domain::tools::pet::{
    FeederCount, FeedingLine, FoldResult, MAX_FAILURES, NO_PET_RETRY_MINUTES, PetFeeding,
    PetRowSetup, PetRowView, PetScheduleSettings, PetSetup, RETRY_MINUTES, ReloadStep, RowOutcome,
    RowReading, ScheduleRead, ScheduleSource, all_refused_message, bag_grid_missing_message,
    build_card, checking_rows_message, consider_veto, cycle_minutes, end_toggle_failed,
    every_row_failed_message, fold_back, load_items, load_minutes, next_action_minutes,
    no_pet_in_bag_message, no_way_to_find_pet_message, nothing_matched_message, pet_did_not_go_in,
    rate_for, ready, reload_step_message, schedule_for, stopped_before_reading_message,
    stopped_during, visit_opening_message, window_left_front_message, window_not_in_front_message,
};

// ── FeederCountTests ───────────────────────────────────────────────────────────────────────

fn scored(lines: &[(&str, f64)]) -> Vec<(String, f64)> {
    lines
        .iter()
        .map(|(text, score)| ((*text).to_owned(), *score))
        .collect()
}

#[test]
fn the_best_scoring_line_wins_not_the_first() {
    let lines = scored(&[("中", 0.13), ("300", 1.00), ("0", 0.56)]);
    assert_eq!(
        FeederCount::parse(&lines, FeederCount::MIN_SCORE),
        Some(300)
    );
}

#[test]
fn junk_below_the_gate_is_not_a_count() {
    let lines = scored(&[("0", 0.56), ("中", 0.51)]);
    assert_eq!(FeederCount::parse(&lines, FeederCount::MIN_SCORE), None);
}

#[test]
fn a_read_is_the_digits_of_the_line() {
    let cases = [
        ("300", 1.00, Some(300)),
        ("0", 0.90, Some(0)),
        ("84", 0.99, Some(84)),
        ("7", 0.95, Some(7)),
    ];
    for (text, score, expected) in cases {
        assert_eq!(
            FeederCount::parse(&scored(&[(text, score)]), FeederCount::MIN_SCORE),
            expected,
            "{text}"
        );
    }
}

/// A wrong count is a PLAUSIBLE number, so implausible ones are rejected rather than scheduled from.
#[test]
fn anything_implausible_is_rejected_rather_than_scheduled() {
    for text in ["1200", "999", "abc", ""] {
        assert_eq!(
            FeederCount::parse(&scored(&[(text, 1.00)]), FeederCount::MIN_SCORE),
            None,
            "{text}"
        );
    }
}

/// Real counts measured 0.90-1.00; everything the icon produced scored at most 0.63.
#[test]
fn the_gate_sits_inside_the_measured_gap() {
    // Bound through a variable so the assertion is about the value, not a folded constant.
    let gate = FeederCount::MIN_SCORE;
    assert!(gate > 0.63);
    assert!(gate < 0.90);
}

// ── PetFeedingTests ───────────────────────────────────────────────────────────────────────

const TABLE_FIXTURE: &str = "species,pet_id,pet_name,stage,wyz_first_level,feed_value_total,food_items,minutes\n\
线甲,1,X,6,5200,75400,2514,838\n\
线乙,2,Y,6,10000,145000,4834,1682\n\
线丙,3,Z,7,33000,478500,15950,3190\n";

fn table() -> Vec<FeedingLine> {
    PetFeeding::parse(TABLE_FIXTURE)
}

/// `+0` at 0 % is the table's own total — `wyz × 14.5`.
#[test]
fn a_new_pet_needs_the_whole_fourteen_point_five() {
    assert!((PetFeeding::remaining_value(5200.0, 0, 0.0) - 5200.0 * 14.5).abs() < 1e-3);
}

/// `+9` at 100 % is DONE — exactly zero, not "nearly".
#[test]
fn a_finished_pet_needs_nothing() {
    assert!(PetFeeding::remaining_value(5200.0, 9, 100.0).abs() < 1e-9);
}

/// A `+9` pet at 10 % has only its own bar to fill: `wyz × 1.9 × 0.9`.
#[test]
fn a_top_pet_with_a_partial_bar_needs_only_that_bar() {
    let expected = 5200.0 * 1.9 * 0.9;
    assert!((PetFeeding::remaining_value(5200.0, 9, 10.0) - expected).abs() < 1e-3);
}

/// The example that prompted all of this: stage-6 pet at `+9` 10 %, just under 99 minutes — not the
/// 505 the configured cycle would have waited.
#[test]
fn a_top_pet_is_revisited_in_about_an_hour_and_a_half_not_eight_hours() {
    let table = table();
    let line = PetFeeding::find(&table, Some("线甲"), 6);

    let minutes = PetFeeding::minutes_to_finish(line, 9, 10.0).unwrap();

    assert!((98.0..100.0).contains(&minutes), "minutes {minutes}");
}

#[test]
fn a_new_pet_needs_its_whole_run() {
    let table = table();
    let minutes = PetFeeding::minutes_to_finish(PetFeeding::find(&table, Some("线甲"), 6), 0, 0.0);

    assert!((minutes.unwrap() - 838.0).abs() < 1e-3);
}

/// An EXP% outside 0-100 is a misread: CLAMPED, because the safe direction is "done".
#[test]
fn an_impossible_exp_is_clamped_not_trusted() {
    assert!(
        (PetFeeding::remaining_value(5200.0, 9, 140.0)
            - PetFeeding::remaining_value(5200.0, 9, 100.0))
        .abs()
            < 1e-9
    );
    assert!(
        (PetFeeding::remaining_value(5200.0, 9, -20.0)
            - PetFeeding::remaining_value(5200.0, 9, 0.0))
        .abs()
            < 1e-9
    );
}

/// The stage-7 `.G` pets sit at 33000 and cannot be boarded; dropping them is what makes
/// `(species, stage)` a key.
#[test]
fn the_unboardable_pets_are_dropped_so_species_and_stage_is_a_key() {
    let table = table();

    assert!(!table.iter().any(|line| line.wyz == 33000.0));
    assert_eq!(table.len(), 2);
}

/// A species spans five stages or seven, so a stage outside its range is a wrongly named species —
/// answer `None` rather than picking a neighbour.
#[test]
fn a_stage_the_species_does_not_have_is_not_found() {
    let table = table();

    assert!(PetFeeding::find(&table, Some("线甲"), 7).is_none());
    assert!(PetFeeding::find(&table, Some("nonsense"), 6).is_none());
    assert!(PetFeeding::minutes_to_finish(None, 9, 10.0).is_none());
}

// ── the schedule (ScheduleFor) ────────────────────────────────────────────────────────────

struct ScheduleFixture {
    items_per_minute: i32,
    wait_after_empty: i32,
    reload_on_start: bool,
    stacks: i32,
}

fn fixture() -> ScheduleFixture {
    ScheduleFixture {
        items_per_minute: 3,
        wait_after_empty: 5,
        reload_on_start: false,
        stacks: 5,
    }
}

fn scheduled(
    fixture: &ScheduleFixture,
    boarding: bool,
    items_left: Option<i32>,
    completion: Option<f64>,
    measured_rate: Option<i32>,
) -> RowReading {
    let settings = PetScheduleSettings {
        items_per_minute: fixture.items_per_minute,
        wait_after_empty_minutes: fixture.wait_after_empty,
        reload_on_start: fixture.reload_on_start,
    };
    schedule_for(
        &ScheduleRead {
            boarding_running: boarding,
            items_left,
            completion_minutes: completion,
            rate: rate_for(measured_rate, fixture.items_per_minute),
            stacks: fixture.stacks,
            slot_occupied: true,
        },
        &settings,
    )
}

/// `LoadItemsFor` = stacks × 300, and minutes at the configured fallback rate — 5 stacks is 1500
/// items, 500 minutes at 3/min, and the cycle adds the wait past empty.
#[test]
fn the_load_and_cycle_arithmetic_is_per_row() {
    assert_eq!(load_items(5), 1500);
    assert_eq!(load_items(2), 600);
    assert_eq!(load_items(0), 300); // max(1, stacks)
    assert_eq!(load_minutes(5, 3), 500);
    assert_eq!(load_minutes(2, 3), 200);
    assert_eq!(cycle_minutes(5, 3, 5), 505.0);
    assert_eq!(cycle_minutes(2, 3, 5), 205.0);
    // A broken rate must not spin the loop.
    assert_eq!(load_minutes(5, 0), 0);
    assert_eq!(cycle_minutes(5, 0, 5), 60.0);
}

/// RELOAD EVERY ROW ON START makes every row due, and no reading can cancel that.
#[test]
fn reload_on_start_makes_every_row_due() {
    let mut fixture = fixture();
    fixture.reload_on_start = true;

    let reading = scheduled(&fixture, true, Some(1500), Some(40.0), None);

    assert_eq!(reading.next_minutes, 0.0);
    assert_eq!(reading.source, ScheduleSource::StartOverride);
}

/// A row that is not boarding reloads NOW — it needs a pet and a fill.
#[test]
fn a_non_boarding_row_is_due_now() {
    let reading = scheduled(&fixture(), false, None, None, None);

    assert_eq!(reading.next_minutes, 0.0);
    assert_eq!(reading.source, ScheduleSource::NotBoarding);
}

/// A count is food evidence: the reload lands just past the moment the tray runs dry, at THAT row's
/// rate, plus the wait past empty.
#[test]
fn a_count_schedules_past_the_moment_the_tray_runs_dry() {
    let reading = scheduled(&fixture(), true, Some(300), None, None);

    // 300 items at 3/min = 100 min, plus 5.
    assert!((reading.next_minutes - 105.0).abs() < 1e-9);
    assert_eq!(reading.source, ScheduleSource::CountRead);
}

/// The game's own rate line shortens the row's arithmetic when it names a faster eater.
#[test]
fn the_rows_own_rate_wins_over_the_configured_fallback() {
    let reading = scheduled(&fixture(), true, Some(300), None, Some(4));

    // 300 at 4/min = 75, plus 5.
    assert!((reading.next_minutes - 80.0).abs() < 1e-9);
}

/// ZERO IS RELOAD NOW, with no margin: a tray already reading zero IS past empty.
#[test]
fn an_empty_tray_is_due_now_with_no_margin() {
    let reading = scheduled(&fixture(), true, Some(0), None, None);

    assert_eq!(reading.next_minutes, 0.0);
    assert_eq!(reading.source, ScheduleSource::CountEmpty);
}

/// With no counts, the game's own completion line is a better answer than the cycle — but not food
/// evidence, and the source says so.
#[test]
fn without_counts_a_completion_line_beats_the_cycle() {
    let reading = scheduled(&fixture(), true, None, Some(14.0), None);

    assert!((reading.next_minutes - 19.0).abs() < 1e-9); // 14 + the 5 past empty
    assert_eq!(reading.source, ScheduleSource::CompletionRead);

    let unreadable = scheduled(&fixture(), true, None, None, None);
    assert_eq!(unreadable.next_minutes, 505.0);
    assert_eq!(unreadable.source, ScheduleSource::CycleFallback);
}

/// WHICHEVER RUNS OUT FIRST — the pet finishing outranks the food left in the tray, and the source
/// stays CountRead because a count above zero WAS read.
#[test]
fn the_pet_finishing_outranks_the_food() {
    let reading = scheduled(&fixture(), true, Some(1500), Some(14.0), None);

    assert!((reading.next_minutes - 19.0).abs() < 1e-9);
    assert_eq!(reading.source, ScheduleSource::CountRead);
}

/// The veto's first gate comes from the SAME pass: a slot that could not be read must keep meaning
/// "act".
#[test]
fn the_slot_occupied_flag_rides_along_with_the_reading() {
    let reading = schedule_for(
        &ScheduleRead {
            boarding_running: true,
            items_left: Some(300),
            completion_minutes: None,
            rate: 3,
            stacks: 5,
            slot_occupied: false, // the slot was not read as occupied
        },
        &PetScheduleSettings {
            items_per_minute: 3,
            wait_after_empty_minutes: 5,
            reload_on_start: false,
        },
    );

    assert!(!reading.slot_occupied);
    assert!(!consider_veto(&reading, 0).0);
}

// ── the veto (ConsiderVeto) ──────────────────────────────────────────────────────────────

fn reading(minutes: f64, source: ScheduleSource, occupied: bool) -> RowReading {
    RowReading {
        next_minutes: minutes,
        source,
        slot_occupied: occupied,
    }
}

/// POSITIVE-ONLY: a skip needs food SEEN — an occupied slot, a count, and more than the margin left.
#[test]
fn the_veto_only_fires_on_food_seen_and_more_than_the_margin() {
    let fresh = reading(45.0, ScheduleSource::CountRead, true);
    let (skip, note) = consider_veto(&fresh, 0);
    assert!(skip);
    assert!(note.contains("found food still in the feeder"), "{note}");

    // Exactly at the margin is NOT a skip — the visit to save would cost what the food is worth.
    let at_margin = reading(30.0, ScheduleSource::CountRead, true);
    assert!(!consider_veto(&at_margin, 0).0);
    assert!(
        consider_veto(&at_margin, 0)
            .1
            .contains("within the 30 min margin")
    );
}

/// Every other state acts: unknown must never cancel a reload, and an unfed pet cannot be undone.
#[test]
fn the_veto_never_fires_without_food_evidence() {
    let cases = [
        (
            reading(500.0, ScheduleSource::CycleFallback, true),
            "configured cycle is assumed",
        ),
        (
            reading(500.0, ScheduleSource::CompletionRead, true),
            "completion line was read",
        ),
        (
            reading(500.0, ScheduleSource::CountRead, false),
            "not read as occupied",
        ),
        (
            reading(500.0, ScheduleSource::StartOverride, true),
            "RELOAD EVERY ROW ON START",
        ),
        (
            reading(0.0, ScheduleSource::CountEmpty, true),
            "counts read zero",
        ),
    ];

    for (fresh, needle) in cases {
        let (skip, note) = consider_veto(&fresh, 0);
        assert!(!skip, "must not skip: {note}");
        assert!(
            note.contains(needle),
            "note `{note}` should mention `{needle}`"
        );
    }
}

/// A veto leaves the strike count untouched — the row proved it can be fed.
#[test]
fn standing_strikes_are_mentioned_but_do_not_change_the_veto() {
    let fresh = reading(45.0, ScheduleSource::CountRead, true);
    let (skip, note) = consider_veto(&fresh, 2);

    assert!(skip);
    assert!(note.contains("2 strike(s) standing"), "{note}");
}

// ── folding a visit back (FoldBack) ───────────────────────────────────────────────────────

/// The pet's own estimate and the food's load compete; the smaller wins, and the message says when the
/// PET is the reason.
#[test]
fn a_reloaded_row_schedules_whichever_runs_out_first() {
    let outcome = RowOutcome::Reloaded {
        pet_minutes: Some(98.0),
        landed_line_minutes: None,
    };

    let folded = fold_back("Row 1", &outcome, 0, 500, 5, "19:40");

    assert_eq!(folded.failures, 0);
    assert!((folded.next_minutes.unwrap() - 103.0).abs() < 1e-9);
    assert_eq!(
        folded.message,
        "Row 1 reloaded. Next 19:40. (the pet finishes first, in 98 min)"
    );
}

#[test]
fn a_reloaded_row_with_no_estimate_keeps_the_full_load() {
    let outcome = RowOutcome::Reloaded {
        pet_minutes: None,
        landed_line_minutes: None,
    };

    let folded = fold_back("Row 1", &outcome, 0, 500, 5, "11:00");

    assert!((folded.next_minutes.unwrap() - 505.0).abs() < 1e-9);
    assert_eq!(folded.message, "Row 1 reloaded. Next 11:00.");
}

/// The two non-failure waits say WHICH wait they are, and reset the strikes.
#[test]
fn a_deferred_open_and_an_empty_bag_are_waits_not_failures() {
    let deferred = fold_back("Row 2", &RowOutcome::Deferred, 2, 500, 5, "x");
    assert_eq!(deferred.failures, 0);
    assert!((deferred.next_minutes.unwrap() - RETRY_MINUTES).abs() < 1e-9);
    assert_eq!(
        deferred.message,
        "Row 2 is waiting for the boarding window to open."
    );

    let no_pet = fold_back("Row 3", &RowOutcome::NothingToBoard, 2, 500, 5, "x");
    assert_eq!(no_pet.failures, 0);
    assert!((no_pet.next_minutes.unwrap() - NO_PET_RETRY_MINUTES).abs() < 1e-9);
    assert_eq!(
        no_pet.message,
        "Row 3 is waiting for a pet that can be fed."
    );
}

/// Three strikes drops the ROW, not the run — the other rows carry on.
#[test]
fn a_row_that_fails_three_times_is_dropped_with_its_reason() {
    let failure = RowOutcome::Failed {
        error: "the filler didn't open".to_owned(),
    };

    let first = fold_back("Row 4", &failure, 0, 500, 5, "x");
    assert_eq!(first.failures, 1);
    assert_eq!(first.message, "Row 4 failed 1/3: the filler didn't open");
    assert!((first.next_minutes.unwrap() - RETRY_MINUTES).abs() < 1e-9);

    let third = fold_back("Row 4", &failure, 2, 500, 5, "x");
    assert_eq!(third.failures, MAX_FAILURES);
    assert!(third.next_minutes.is_none(), "the row is dropped");
    let log = third.dropped_log.unwrap();
    assert!(log.contains("given up on after 3 failures"), "{log}");
    assert!(log.contains("the filler didn't open"), "{log}");
    assert!(log.contains("the other rows carry on"), "{log}");
}

// ── the card (PublishSchedule) and the arbiter facts ────────────────────────────────────

fn view(name: &str, has_next: bool, boarding: bool, next: Option<f64>) -> PetRowView {
    PetRowView {
        name: name.to_owned(),
        has_next,
        boarding_running: boarding,
        next_minutes: next,
        pet_detail: None,
        growth_target: None,
        measured_rate: None,
    }
}

/// One line per row, in the order given, with the status from the same two facts the schedule uses —
/// and everything the player carries: the pet, the growth it is heading for, the row's own rate.
#[test]
fn the_card_lists_every_row_with_its_status_and_detail() {
    let mut first = view("Row 1", true, true, Some(500.0));
    first.pet_detail = Some("真蔚蓝凤凰  ·  stage 6, +7, 52.18%".to_owned());
    first.growth_target = Some(9);
    first.measured_rate = Some(3);
    let second = view("Row 2", true, false, Some(30.0));
    let third = view("Row 3", false, false, None);

    let card = build_card(&[first, second, third]);

    assert_eq!(card.rows.len(), 3);
    assert_eq!(card.rows[0].status, "boarding");
    assert_eq!(
        card.rows[0].detail.as_deref(),
        Some("真蔚蓝凤凰  ·  stage 6, +7, 52.18%  ·  → +9  ·  3/min (game line)")
    );
    assert_eq!(card.rows[1].status, "waiting for a reload");
    assert_eq!(card.rows[2].status, "given up");
    assert_eq!(card.boarding, vec!["Row 1"]);
    assert_eq!(card.next_row.as_deref(), Some("Row 2"));
    assert!((card.next_minutes.unwrap() - 30.0).abs() < 1e-9);
    assert_eq!(card.queued, 1);
    assert_eq!(
        card.schedule_line("19:40"),
        "boarding Row 1  ·  next Row 2 at 19:40  ·  1 more queued"
    );
}

#[test]
fn a_card_with_nothing_scheduled_says_so() {
    let card = build_card(&[view("Row 1", false, false, None)]);

    assert_eq!(card.schedule_line("00:00"), "no rows scheduled");
    assert!(card.next_row.is_none());
}

#[test]
fn nothing_boarding_is_named_as_such() {
    let card = build_card(&[view("Row 1", true, false, Some(12.0))]);

    assert_eq!(
        card.schedule_line("08:17"),
        "nothing boarding  ·  next Row 1 at 08:17"
    );
}

/// The arbiter's facts are plain data: does a row want the game NOW, and when is the next action.
#[test]
fn the_card_supplies_the_arbiter_facts_without_deciding_anything() {
    let due_now = build_card(&[view("Row 1", true, false, Some(0.0))]);
    let facts = due_now.facts();
    assert!(facts.wants_game_now);
    assert_eq!(facts.next_action_in_minutes, Some(0.0));

    let later = build_card(&[view("Row 1", true, false, Some(42.0))]);
    let facts = later.facts();
    assert!(!facts.wants_game_now);
    assert_eq!(facts.next_action_in_minutes, Some(42.0));

    let soonest = next_action_minutes(&[("Row 1".to_owned(), 40.0), ("Row 2".to_owned(), 5.0)]);
    assert_eq!(soonest, Some(("Row 2".to_owned(), 5.0)));
}

// ── Ready: the Start refusals ───────────────────────────────────────────────────────────

fn setup() -> PetSetup {
    PetSetup {
        menu_button: Some(vec![1, 1]),
        feed_icon: Some(vec![2, 2]),
        close_button: Some(vec![3, 3]),
        bag_grid: Some(vec![0, 0, 400, 400]),
        bag_slot: Some(vec![0, 0, 50, 50]),
        page_tabs: vec![vec![10, 10]],
        slots: vec![PetRowSetup {
            enabled: true,
            toggle_label: Some(vec![0, 0, 40, 20]),
            feeder_strip: Some(vec![0, 100, 300, 60]),
            stacks: 2,
        }],
        food_load_mode: "right_click".to_owned(),
        return_slot: Some(vec![0, 3]),
        queue_len: 0,
        food_slots: vec![vec![0, 5]],
        max_button: None,
        buy_sell_max_button: Some(vec![9, 9]),
    }
}

fn refusal(setup: &PetSetup) -> String {
    match ready(setup) {
        Err(stop) => {
            assert_eq!(stop.kind, StopKind::Error);
            stop.message
        }
        Ok(()) => panic!("expected the run to be refused"),
    }
}

#[test]
fn a_complete_setup_passes_ready() {
    assert!(ready(&setup()).is_ok());
}

#[test]
fn ready_names_each_missing_calibration_in_v2s_order() {
    let mut broken = setup();
    broken.menu_button = None;
    assert_eq!(
        refusal(&broken),
        "The pet menu button isn't calibrated — save the 目錄 (menu) point on the Calibrate tab."
    );

    let mut broken = setup();
    broken.feed_icon = None;
    assert_eq!(
        refusal(&broken),
        "The pet feed icon isn't calibrated — save the Pet feed icon point on the Calibrate tab."
    );

    let mut broken = setup();
    broken.close_button = None;
    assert_eq!(
        refusal(&broken),
        "The boarding window's X isn't calibrated — save the Boarding X point on the Calibrate tab."
    );

    let mut broken = setup();
    broken.bag_slot = None;
    assert_eq!(
        refusal(&broken),
        "The boarding bag's grid isn't calibrated — save the Boarding bag grid and Boarding bag slot on the Calibrate tab. (Its own, not the shop's.)"
    );

    let mut broken = setup();
    broken.page_tabs.clear();
    assert_eq!(
        refusal(&broken),
        "The bag page tabs aren't calibrated — save the Bag page tab points on the Calibrate tab."
    );

    let mut broken = setup();
    broken.slots.clear();
    assert_eq!(
        refusal(&broken),
        "No breeding row is set up — add it in config/local.yaml (the Pet tab is read-only in v3.0). Each row needs its start/end button and its pet slot marked on the Calibrate tab."
    );

    let mut broken = setup();
    broken.slots[0].enabled = false;
    assert_eq!(
        refusal(&broken),
        "Every row is unticked — nothing to feed. Tick the row in config/local.yaml (the Pet tab is read-only in v3.0)."
    );
}
#[test]
fn a_ticked_row_with_no_button_is_refused_but_an_unticked_one_is_left_alone() {
    let mut broken = setup();
    broken.slots[0].toggle_label = None;
    assert_eq!(
        refusal(&broken),
        "Row 1's start/end button isn't marked — save the Row start/end button for row 1 on the Calibrate tab. The tool has nothing to click for that row. Untick it in config/local.yaml (the Pet tab is read-only in v3.0) to leave it alone."
    );

    // The unticked row is not driven, so its missing marks must NOT block a run.
    let mut ignored = setup();
    ignored.slots[0].enabled = false;
    ignored.slots[0].feeder_strip = None;
    ignored.slots[0].toggle_label = None;
    ignored.slots[0].toggle_label = None;
    assert_eq!(
        refusal(&ignored),
        "Every row is unticked — nothing to feed. Tick the row in config/local.yaml (the Pet tab is read-only in v3.0)."
    );
}
/// A drag has to name the box it drops into, so the strip is required only in drag mode.
#[test]
fn a_drag_needs_the_food_strip_and_right_click_does_not() {
    let mut drag = setup();
    drag.food_load_mode = "drag".to_owned();
    drag.slots[0].feeder_strip = None;
    assert_eq!(
        refusal(&drag),
        "Row 1's food strip isn't drawn — save the Row food strip for row 1 on the Calibrate tab. The food load is set to drag, which has to name the box it drops into, and the strip is what says where the boxes are."
    );

    let mut right_click = setup();
    right_click.food_load_mode = "right_click".to_owned();
    right_click.slots[0].feeder_strip = None;
    assert!(ready(&right_click).is_ok());
}
#[test]
fn a_cell_on_an_uncalibrated_page_is_refused_before_the_run() {
    let mut broken = setup();
    broken.return_slot = Some(vec![3, 1]);
    assert_eq!(
        refusal(&broken),
        "The return slot is marked on bag page 4, but only 1 page tab(s) are calibrated — save the Bag page tab point on the Calibrate tab, or move the cell in config/local.yaml."
    );

    let mut untabbed = setup();
    untabbed.page_tabs = vec![vec![10, 10], Vec::new()];
    untabbed.food_slots = vec![vec![1, 5]];
    assert_eq!(
        refusal(&untabbed),
        "A food cell is marked on bag page 2, and that page's tab isn't calibrated — save the Bag page tab point ITEM2 on the Calibrate tab."
    );
}
#[test]
fn a_pet_needs_a_way_to_be_found_and_food_and_a_max() {
    let mut no_way = setup();
    no_way.return_slot = None;
    no_way.queue_len = 0;
    assert_eq!(
        refusal(&no_way),
        "Neither a return slot nor any queued pet icon is set — the tool has nothing to find the pet by. Set pet.return_slot in config/local.yaml (the Pet tab is read-only in v3.0), or add a pet icon to pet.queue."
    );

    let mut no_food = setup();
    no_food.food_slots.clear();
    assert_eq!(
        refusal(&no_food),
        "No food cells are marked — set pet.food_slots in config/local.yaml (the Pet tab is read-only in v3.0). Nothing says where the pet food lives."
    );

    let mut no_max = setup();
    no_max.buy_sell_max_button = None;
    assert_eq!(
        refusal(&no_max),
        "No MAX is calibrated — save the MAX button on the Calibrate tab."
    );
}
// ── the messages the visit and reload show ─────────────────────────────────────────────

#[test]
fn the_visit_opening_line_names_a_batch_or_the_row() {
    assert_eq!(
        visit_opening_message(&["Row 1".to_owned(), "Row 2".to_owned()]),
        "Opening the breeder for 2 rows…"
    );
    assert_eq!(
        visit_opening_message(&["Row 1".to_owned()]),
        "Row 1: opening the boarding window…"
    );
}

/// Each reload step names itself, so a failure does not read as "it opened and shut again".
#[test]
fn the_reload_steps_name_themselves_on_the_card() {
    let steps = [
        (ReloadStep::Reloading, "reloading…"),
        (ReloadStep::EndingBoarding, "ending boarding…"),
        (ReloadStep::PlacingPet, "placing the pet…"),
        (ReloadStep::LoadingFood, "loading the food…"),
        (ReloadStep::StartingBoarding, "starting boarding…"),
    ];
    for (step, text) in steps {
        assert_eq!(reload_step_message("Row 1", step), format!("Row 1: {text}"));
    }
}

#[test]
fn the_load_bearing_failure_messages_are_v2s() {
    assert!(end_toggle_failed("Row 2").starts_with(
        "Row 2: the pet did NOT come back out of the loader after pressing the end button"
    ));
    assert_eq!(
        pet_did_not_go_in(),
        "The pet did not go in after 3 right-clicks — the slot still looks empty. Nothing was \
         loaded. Check pet.return_slot in config/local.yaml (the Pet tab is read-only in v3.0)."
    );
    assert_eq!(
        all_refused_message(&["Knight0-9 at page 1 cell 4".to_owned()]),
        "Every pet that could be boarded reads +9/100%. Refused Knight0-9 at page 1 cell 4. Capture \
         the icon of a pet that can still be fed and add it to pet.queue in config/local.yaml (the \
         Pet tab is read-only in v3.0)."
    );
    assert_eq!(
        nothing_matched_message(),
        "The return slot holds no pet, and no queued icon matched one in the bag. Capture the icon of \
         a pet that can still be fed and add it to pet.queue in config/local.yaml (the Pet tab is \
         read-only in v3.0)."
    );
    assert_eq!(
        no_way_to_find_pet_message(),
        "Neither a queued pet icon nor a return slot is set — there is nothing to right-click. Set \
         pet.return_slot or add a pet icon to pet.queue in config/local.yaml (the Pet tab is read-only \
         in v3.0)."
    );
    assert_eq!(
        no_pet_in_bag_message("the queue has no icons in it"),
        "No queued pet is in the bag (the queue has no icons in it). Right-clicked nothing. If the \
         pet finished it was mailed, so this is the queue being empty of live pets — add the next \
         one's icon to pet.queue in config/local.yaml (the Pet tab is read-only in v3.0)."
    );
    assert_eq!(
        bag_grid_missing_message(),
        "The bag grid isn't calibrated — save the Boarding bag grid on the Calibrate tab; there is \
         nothing to right-click."
    );
}
#[test]
fn the_run_and_visit_state_messages_are_v2s() {
    assert_eq!(
        every_row_failed_message(),
        "Every row has failed repeatedly — stopped."
    );
    assert_eq!(
        stopped_before_reading_message(),
        "Stopped before it could read the rows."
    );
    assert_eq!(checking_rows_message(), "Checking what is already running…");
    assert_eq!(
        window_left_front_message(),
        "The game window left the front while reading the rows — trying again in 5 min."
    );
    assert_eq!(
        window_not_in_front_message("Row 1"),
        "The game window isn't in front — Row 1 keeps its place; nothing was clicked, and it is tried \
         again in 5 min."
    );
}

/// A stop inside a visit is the handover to the player: the reload was cut, and this process cannot
/// put the game back.
#[test]
fn a_stopped_visit_says_which_rows_may_not_have_been_fed() {
    assert_eq!(
        stopped_during(&["Row 1".to_owned()]),
        "Stopped during Row 1 — it may not have been fed. Check it in game."
    );
    assert_eq!(
        stopped_during(&["Row 1".to_owned(), "Row 3".to_owned()]),
        "Stopped during 2 rows (Row 1, Row 3) — some may not have been fed. Check them in game."
    );
}

/// The `drop` log is only for the log; the card message stays the strikes line.
#[test]
fn the_dropped_log_is_absent_until_the_row_is_dropped() {
    let failure = RowOutcome::Failed {
        error: "boom".to_owned(),
    };
    let first: FoldResult = fold_back("Row 1", &failure, 0, 500, 5, "x");
    assert!(first.dropped_log.is_none());

    // A wait never carries a drop log for a failure, and the failure never claims a next when dropped.
    let last = fold_back("Row 1", &failure, 2, 500, 5, "x");
    assert!(last.dropped_log.is_some());
}

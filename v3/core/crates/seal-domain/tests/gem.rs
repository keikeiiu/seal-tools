//! The gem composer's decision half: the route table, the two move sets, the empty-result-box
//! discriminator, and the start / combine / settle / advance turn sequence.
//!
//! `GemColorAnalyzerTests.DistanceIsEuclideanNotTheMeanThatHidAGem` is the v2 oracle for the
//! discriminator and is translated exactly. The turn sequence comes from `GemComposer.cs`'s run
//! loop, which v2 could not test without the game, a capture and a serial port.
//!
//! One thing is worth stating because it looks like a bug: the move set's TUNED route keys are
//! `slot3_n` / `slot3_g` / `slot3_dg` while `gem.movements` stores them as `slot3_to_n` etc. That is
//! v2's own naming (`GemRoutes.Route.Key` vs `MovementsConfig`), and both are kept:
//! `GemRoutes::destination` takes the route key, the movements lookup takes the grade.

use std::collections::BTreeMap;

use seal_domain::config::{ColorComposition, MovementsConfig};
use seal_domain::geometry::Point;
use seal_domain::tools::gem::{
    EmptyMode, EmptyRead, GemRoutes, GemRun, GemSettings, GemStep, GemTurn, MoveMode,
    analyze_composition, composition_distance, composition_is_empty, result_box_is_empty,
};

fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}

fn movements() -> MovementsConfig {
    MovementsConfig {
        radio_to_register: BTreeMap::from([
            ("N".to_owned(), vec![10, 20]),
            ("G".to_owned(), vec![11, 21]),
            ("DG".to_owned(), vec![12, 22]),
        ]),
        register_combine: vec![1, 2],
        combine_register: vec![3, 4],
        grade_next: vec![5, 6],
        grade_prev: vec![7, 8],
        register_slot1: vec![9, 10],
        slot1_slot2: vec![11, 12],
        slot2_slot3: vec![13, 14],
        slot3_to_n: vec![15, 16],
        slot3_to_g: vec![17, 18],
        slot3_to_dg: vec![19, 20],
    }
}

fn positions() -> BTreeMap<String, Vec<i32>> {
    BTreeMap::from([
        ("N".to_owned(), vec![100, 200]),
        ("G".to_owned(), vec![150, 200]),
        ("DG".to_owned(), vec![200, 200]),
        ("Register".to_owned(), vec![300, 300]),
        ("Combine".to_owned(), vec![400, 300]),
    ])
}

fn settings() -> GemSettings {
    GemSettings {
        grades: ["N", "G", "DG"].iter().map(|g| (*g).to_owned()).collect(),
        start_grade: "N".to_owned(),
        grade_positions: positions(),
        movements: movements(),
        move_mode: MoveMode::Tuned,
        resource_gems: vec![vec![500, 300], vec![520, 300], vec![540, 300]],
        empty_mode: EmptyMode::Stop,
        empty_distance: 0.18,
        empty_signature: Some(signature()),
        empty_streak: 2,
    }
}

fn signature() -> ColorComposition {
    ColorComposition {
        mean_v: 0.3,
        mean_sat: 0.1,
        colored_fraction: 0.05,
        mean_r: 0.3,
        mean_g: 0.3,
        mean_b: 0.3,
    }
}

fn steps_of(turn: GemTurn) -> Vec<GemStep> {
    assert!(turn.stop.is_none(), "unexpected stop: {:?}", turn.stop);
    turn.steps
}

// ── the route table (GemRoutes.cs) ─────────────────────────────────────────────────────────

#[test]
fn the_route_table_is_v2s_eleven_routes_in_its_order() {
    let keys: Vec<&str> = GemRoutes::ALL.iter().map(|route| route.key).collect();
    assert_eq!(
        keys,
        vec![
            "radio_N",
            "radio_G",
            "radio_DG",
            "register_combine",
            "combine_register",
            "register_slot1",
            "slot1_slot2",
            "slot2_slot3",
            "slot3_n",
            "slot3_g",
            "slot3_dg",
        ]
    );

    let first = GemRoutes::ALL[0];
    assert_eq!((first.from, first.to), ("N", "Register"));
    assert_eq!(first.label, "N → Register");
}

#[test]
fn a_route_ends_on_its_calibrated_destination_point() {
    assert_eq!(GemRoutes::destination("radio_N"), Some("Register"));
    assert_eq!(GemRoutes::destination("combine_register"), Some("Register"));
    assert_eq!(GemRoutes::destination("slot3_dg"), Some("DG"));
    assert_eq!(GemRoutes::destination("no_such_route"), None);
}

/// `Resolve`: grade/Register/Combine come from `gem.grade_positions`; Resource1..3 from
/// `gem.resource_gems` (1-based in the name, 0-based in the list).
#[test]
fn a_point_name_resolves_to_the_calibrated_offset() {
    let positions = positions();
    let resources = vec![vec![500, 300], vec![520, 300], vec![540, 300]];

    assert_eq!(
        GemRoutes::resolve(&positions, &resources, "Register"),
        Some(point(300, 300))
    );
    assert_eq!(
        GemRoutes::resolve(&positions, &resources, "Resource2"),
        Some(point(520, 300))
    );
    assert_eq!(
        GemRoutes::resolve(&positions, &resources, "Resource0"),
        None
    );
    assert_eq!(
        GemRoutes::resolve(&positions, &resources, "Resource4"),
        None
    );
    assert_eq!(GemRoutes::resolve(&positions, &resources, "SG"), None);
}

// ── the two move sets (GemRoutes.cs / GemComposer.Route) ───────────────────────────────────

/// "tuned" sends the hand-tuned counts from `gem.movements` as one `D dx dy`.
#[test]
fn tuned_mode_sends_the_configured_counts_for_a_route() {
    let mut settings = settings();
    settings.move_mode = MoveMode::Tuned;
    let mut run = GemRun::new(settings).unwrap();

    let turn = run.combine_turn().unwrap();

    assert_eq!(turn.steps[0], GemStep::Move { dx: 1, dy: 2 });
    assert_eq!(turn.steps[1], GemStep::Wait(0.2));
    assert_eq!(turn.steps[2], GemStep::ClickLeft);
    assert_eq!(turn.steps[3], GemStep::Wait(0.8));
}

/// "arduino" ignores the tuned counts and places the cursor on the route's destination point,
/// closed-loop — the same mechanism the calibrator's Test Click uses.
#[test]
fn arduino_mode_places_the_cursor_on_the_destination_point() {
    let mut settings = settings();
    settings.move_mode = MoveMode::Arduino;
    let mut run = GemRun::new(settings).unwrap();

    let turn = run.combine_turn().unwrap();

    // Register → Combine: the destination is the Combine point.
    assert_eq!(turn.steps[0], GemStep::Place(point(400, 300)));
    assert_eq!(turn.steps[2], GemStep::ClickLeft);
}

/// A tuned move that was never saved is refused with the instruction v2 shows, not sent as (0,0).
#[test]
fn a_tuned_move_that_was_never_saved_is_refused_with_the_instruction() {
    let mut settings = settings();
    settings.movements.register_combine = Vec::new();
    let mut run = GemRun::new(settings).unwrap();

    let err = run.combine_turn().unwrap_err();
    assert_eq!(
        err.message,
        "Move Register → Combine isn't saved yet — the tuned movements are hand-edited in gem.movements in config/local.yaml (the movement editor is not built in v3.0)."
    );
}

/// An uncalibrated point is refused with the point's name, in both move sets' path (the arduino set
/// resolves points, and a missing one must not click blind).
#[test]
fn an_uncalibrated_point_is_refused_with_its_name() {
    let mut settings = settings();
    settings.move_mode = MoveMode::Arduino;
    settings.grade_positions.remove("Combine");
    let mut run = GemRun::new(settings).unwrap();

    let err = run.combine_turn().unwrap_err();
    assert_eq!(
        err.message,
        "Move Register → Combine: 'Combine' isn't calibrated yet — save the Grade point 'Combine' on the Calibrate tab."
    );
}

/// An unknown route in arduino mode is v2's own failure path (it cannot derive a destination).
#[test]
fn an_unknown_route_is_refused_by_its_key() {
    let mut settings = settings();
    settings.move_mode = MoveMode::Arduino;
    let run = GemRun::new(settings).unwrap();

    let err = run
        .route_step("Move X → Y", "no_such_route", None)
        .unwrap_err();
    assert_eq!(err.message, "Move X → Y: unknown route 'no_such_route'.");
}

// ── the empty-result-box discriminator (GemColorAnalyzer / GemComposer) ───────────────────

/// The exact measured pair from v2's test: the old mean-of-six gave 0.100 (below the threshold, so a
/// gem read as empty); the Euclidean norm is 0.294, which 0.18 separates cleanly.
#[test]
fn the_composition_distance_is_euclidean_not_the_mean_that_hid_a_gem() {
    let empty = ColorComposition {
        mean_v: 0.859_564,
        mean_sat: 0.274_034,
        colored_fraction: 1.0,
        mean_r: 0.839_423,
        mean_g: 0.859_273,
        mean_b: 0.631_923,
    };
    let gem = ColorComposition {
        mean_v: 0.811_451,
        mean_sat: 0.414_936,
        colored_fraction: 0.934_937,
        mean_r: 0.640_172,
        mean_g: 0.716_273,
        mean_b: 0.638_671,
    };

    let distance = composition_distance(&empty, &gem);
    assert!((0.29..0.30).contains(&distance), "distance {distance}");
    assert!(!composition_is_empty(&gem, Some(&empty), 0.01));
    assert!(composition_is_empty(&empty, Some(&empty), 0.01));
    // A null reference means empty-detection has not been calibrated: never "empty".
    assert!(!composition_is_empty(&empty, None, 0.5));
}

/// The crop is the primary signal: when a diff fraction exists it alone decides, in the safe
/// direction for a missing reference.
#[test]
fn a_diff_fraction_at_or_below_the_threshold_is_empty() {
    assert!(result_box_is_empty(
        &EmptyRead::DiffFraction(0.0),
        Some(&signature()),
        0.18
    ));
    assert!(result_box_is_empty(
        &EmptyRead::DiffFraction(0.18),
        Some(&signature()),
        0.18
    ));
    assert!(!result_box_is_empty(
        &EmptyRead::DiffFraction(0.357),
        Some(&signature()),
        0.18
    ));
}

/// Without the crop, the weaker colour signature is the fallback; without either, the composer must
/// NOT advance on a missing reference.
#[test]
fn the_colour_signature_is_the_fallback_and_no_reference_is_never_empty() {
    let frame = signature();

    assert!(result_box_is_empty(
        &EmptyRead::Composition(frame),
        Some(&signature()),
        0.01
    ));
    assert!(!result_box_is_empty(
        &EmptyRead::Composition(frame),
        None,
        0.01
    ));
    assert!(!result_box_is_empty(&EmptyRead::NoReference, None, 0.01));
}

// ── the run: start, combine, settle, advance ──────────────────────────────────────────────

/// Start selects the grade and moves to Register: the single click on the grade button both
/// activates the window and presses the button, so there is no separate focus click.
#[test]
fn the_start_turn_selects_the_grade_then_moves_to_register() {
    let run = GemRun::new(settings()).unwrap();

    let steps = steps_of(run.select_grade_turn().unwrap());

    assert_eq!(
        steps,
        vec![
            GemStep::Place(point(100, 200)), // the N button
            GemStep::Wait(0.3),
            GemStep::ClickLeft,
            GemStep::Wait(0.5),
            GemStep::Move { dx: 10, dy: 20 }, // N → Register
            GemStep::Wait(0.3),
            GemStep::ClickLeft,
            GemStep::Wait(0.5),
        ]
    );
}

/// The start grade is resolved, and a name the grade list does not have starts at the first grade
/// (v2's `Math.Max(0, IndexOf(...))`).
#[test]
fn the_start_grade_falls_back_to_the_first_grade() {
    let mut from_unknown = settings();
    from_unknown.start_grade = "SG".to_owned();
    let run = GemRun::new(from_unknown).unwrap();
    assert_eq!(run.grade(), "N");

    let mut from_g = settings();
    from_g.start_grade = "G".to_owned();
    let run = GemRun::new(from_g).unwrap();
    assert_eq!(run.grade(), "G");
}

/// An empty grade list is the one thing the domain must refuse up front: v2 indexed `grades[gidx]`
/// before its window even opened.
#[test]
fn an_empty_grade_list_is_refused_up_front() {
    let mut settings = settings();
    settings.grades.clear();

    let err = GemRun::new(settings).unwrap_err();
    assert_eq!(
        err.message,
        "No gem grades are configured — the composer has nothing to work through."
    );
}

/// One combine turn counts the cycle and its grade BEFORE combining — the tally says where the work
/// went even if the run dies mid-cycle.
#[test]
fn a_combine_turn_counts_the_cycle_and_its_grade() {
    let mut run = GemRun::new(settings()).unwrap();

    run.combine_turn().unwrap();

    assert_eq!(run.cycle(), 1);
    assert_eq!(run.tally().count_for("N"), 1);
}

/// The normal settle turn moves back to Register and does the deregister/register click pair.
#[test]
fn a_normal_settle_turn_returns_to_register_and_clicks_twice() {
    let mut run = GemRun::new(settings()).unwrap();
    run.combine_turn().unwrap();

    let turn = run.finish_turn(Some(false)).unwrap();

    assert_eq!(
        turn.steps,
        vec![
            GemStep::Move { dx: 3, dy: 4 }, // Combine → Register
            GemStep::Wait(0.2),
            GemStep::ClickLeft,
            GemStep::Wait(0.3),
            GemStep::ClickLeft,
            GemStep::Wait(0.5),
        ]
    );
    assert!(turn.stop.is_none());
}

/// The empty streak guards against a single transient false empty: `empty_streak` consecutive empty
/// reads are needed before acting, and one non-empty read resets it.
#[test]
fn the_empty_streak_must_be_met_before_advancing() {
    let mut settings = settings();
    settings.empty_mode = EmptyMode::AdvanceGrade;
    settings.empty_streak = 2;
    let mut run = GemRun::new(settings).unwrap();
    run.combine_turn().unwrap();

    let first = run.finish_turn(Some(true)).unwrap();
    assert!(first.stop.is_none());
    // Still combining the same grade.
    assert_eq!(run.grade(), "N");

    run.combine_turn().unwrap();
    let second = run.finish_turn(Some(true)).unwrap();
    assert_eq!(run.grade(), "G");
    // The advance flow ends with the next grade → Register and a click.
    assert!(second.steps.contains(&GemStep::Wait(0.5)));
}

#[test]
fn a_non_empty_read_resets_the_empty_streak() {
    let mut settings = settings();
    settings.empty_mode = EmptyMode::AdvanceGrade;
    settings.empty_streak = 2;
    let mut run = GemRun::new(settings).unwrap();
    run.combine_turn().unwrap();
    run.finish_turn(Some(true)).unwrap();
    run.combine_turn().unwrap();
    run.finish_turn(Some(false)).unwrap();
    run.combine_turn().unwrap();

    // One empty read after the reset is not enough.
    let turn = run.finish_turn(Some(true)).unwrap();
    assert!(turn.stop.is_none());
    assert_eq!(run.grade(), "N");
}

/// "stop" mode never auto-advances, and v2 ALSO gates the check on a non-null empty signature even
/// when the saved crop exists (`EmptyMode != "stop" && EmptySignature != null`). Both are ported and
/// pinned: the caller is told not to read at all.
#[test]
fn the_auto_check_is_off_in_stop_mode_and_without_a_signature() {
    let run = GemRun::new(settings()).unwrap();
    assert!(!run.empty_check_enabled());

    let mut settings = settings();
    settings.empty_mode = EmptyMode::AdvanceGrade;
    let run = GemRun::new(settings.clone()).unwrap();
    assert!(run.empty_check_enabled());

    settings.empty_signature = None;
    let run = GemRun::new(settings).unwrap();
    assert!(!run.empty_check_enabled());

    // With the check off, an "empty" read changes nothing.
    let mut settings = self::settings();
    settings.empty_mode = EmptyMode::AdvanceGrade;
    settings.empty_signature = None;
    let mut run = GemRun::new(settings).unwrap();
    run.combine_turn().unwrap();
    let turn = run.finish_turn(Some(true)).unwrap();
    assert_eq!(run.grade(), "N");
    assert_eq!(turn.steps.len(), 6);
}

/// `advance_grade_clear` right-clicks the three resource slots before moving on, starting from
/// Register, then Resource3 → the next grade and a click.
#[test]
fn advance_grade_clear_clears_the_resources_before_selecting_the_next_grade() {
    let mut settings = settings();
    settings.empty_mode = EmptyMode::AdvanceGradeClear;
    settings.empty_streak = 1;
    let mut run = GemRun::new(settings).unwrap();
    run.combine_turn().unwrap();

    let turn = run.finish_turn(Some(true)).unwrap();

    // Combine → Register first, then the three right-clicks.
    assert_eq!(turn.steps[0], GemStep::Move { dx: 3, dy: 4 });
    assert_eq!(turn.steps[2], GemStep::Move { dx: 9, dy: 10 }); // Register → Resource1
    assert_eq!(turn.steps[4], GemStep::ClickRight);
    // Then Resource3 → G, and G → Register.
    assert!(turn.steps.contains(&GemStep::Move { dx: 17, dy: 18 }));
    assert!(turn.steps.contains(&GemStep::Move { dx: 11, dy: 21 }));
    assert_eq!(run.grade(), "G");
}

/// Plain `advance_grade` re-selects the next grade ABSOLUTELY (place the grade button and click)
/// because the cursor is at Register, not at Resource3.
#[test]
fn advance_grade_re_selects_the_next_grade_absolutely() {
    let mut settings = settings();
    settings.empty_mode = EmptyMode::AdvanceGrade;
    settings.empty_streak = 1;
    let mut run = GemRun::new(settings).unwrap();
    run.combine_turn().unwrap();

    let turn = run.finish_turn(Some(true)).unwrap();

    assert!(turn.steps.contains(&GemStep::Place(point(150, 200)))); // the G button
    assert!(turn.steps.contains(&GemStep::Move { dx: 11, dy: 21 })); // G → Register
    assert_eq!(run.grade(), "G");
}

/// A run is N → G → DG ONCE, not a loop: the last grade stops with v2's message instead of wrapping.
#[test]
fn the_last_grade_stops_the_composer_instead_of_wrapping() {
    let mut settings = settings();
    settings.empty_mode = EmptyMode::AdvanceGrade;
    settings.empty_streak = 1;
    settings.start_grade = "DG".to_owned();
    let mut run = GemRun::new(settings).unwrap();
    run.combine_turn().unwrap();

    let turn = run.finish_turn(Some(true)).unwrap();

    let stop = turn.stop.expect("the last grade must stop the run");
    assert_eq!(
        stop.message,
        "All grades done (last was DG) — composer stopped."
    );
    assert_eq!(run.grade(), "DG");
}

/// F9 advances BY HAND and wraps, and it logs the grade as "left by hand" rather than finished —
/// the grade did not run out, the player moved on. Nothing is cleared in this path.
#[test]
fn the_f9_hotkey_advances_by_hand_and_wraps() {
    let run_settings = {
        let mut settings = settings();
        settings.start_grade = "DG".to_owned();
        settings
    };
    let mut run = GemRun::new(run_settings).unwrap();

    let turn = run.advance_grade_by_hand().unwrap();

    assert_eq!(run.grade(), "N");
    assert_eq!(turn.steps[0], GemStep::Place(point(100, 200)));
}

/// The mid-run failure messages are v2's, and are reader-reported because the decision half cannot
/// see a window or a board.
#[test]
fn the_reader_reported_failures_carry_v2s_messages() {
    assert_eq!(
        seal_domain::tools::gem::window_missing().message,
        "Game window not found (or minimized) — open and restore the game first."
    );
    assert_eq!(
        seal_domain::tools::gem::grade_button_placement_failed("DG", "3 steps, ended at (1,2)")
            .message,
        "Couldn't move the cursor onto the DG button — 3 steps, ended at (1,2). Stopped instead of \
         clicking blind."
    );
    assert_eq!(
        seal_domain::tools::gem::route_placement_failed(
            "Move N → Register",
            "Register",
            "started at 0"
        )
        .message,
        "Move N → Register: couldn't move the cursor to Register — started at 0."
    );
}

#[test]
fn the_colour_composition_is_the_analyzers_six_features() {
    // A 2x2 solid red box: brightness 1, full saturation, every pixel coloured.
    let red = [255u8, 0, 0, 255].repeat(4);

    let composition = analyze_composition(&red, 2, 2, 40);

    assert!((composition.mean_v - 1.0).abs() < 1e-9);
    assert!((composition.mean_sat - 1.0).abs() < 1e-9);
    assert!((composition.colored_fraction - 1.0).abs() < 1e-9);
    assert!((composition.mean_r - 1.0).abs() < 1e-9);
    assert_eq!(composition.mean_g, 0.0);
    assert_eq!(composition.mean_b, 0.0);
}

#[test]
fn a_neutral_box_is_not_coloured() {
    let gray = [128u8, 128, 128, 255].repeat(4);

    let composition = analyze_composition(&gray, 2, 2, 40);

    assert!((composition.mean_sat - 0.0).abs() < 1e-9);
    assert_eq!(composition.colored_fraction, 0.0);
    assert!((composition.mean_r - composition.mean_g).abs() < 1e-9);
    assert_eq!(composition.mean_r, composition.mean_g);
    assert_eq!(composition.mean_g, composition.mean_b);
}

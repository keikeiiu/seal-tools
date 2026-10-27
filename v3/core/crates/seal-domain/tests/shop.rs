//! The shop's decision half: the bag-grid and shop-row derivations, the Start readiness refusals,
//! and the ordered transaction plan for one buy or sell pass.
//!
//! `BagGridTests.cs` and `ShopGeometryTests.cs` are the v2 oracles for the derivations and are
//! translated here; the readiness refusals and the transaction order come from `ShopTool.cs`, which
//! v2 could not test without a serial port and a live game.
//!
//! Selling is the only irreversible thing in this suite — a mis-aimed click sells the wrong stack and
//! it is gone — so two rules are enforced HERE rather than left to the loop: slots are sold HIGHEST
//! INDEX FIRST (correct whether or not the bag compacts after a sale), and `sell_cap` is a hard
//! ceiling, not advice.

use std::collections::BTreeMap;

use seal_domain::config::BuyPreset;
use seal_domain::decision::StopKind;
use seal_domain::geometry::Point;
use seal_domain::tools::shop::{
    BagGrid, ClickButton, ShopGeometry, ShopPlan, ShopSettings, ShopStep, scroll_settle,
};

fn point(x: i32, y: i32) -> Point {
    Point { x, y }
}

fn preset(scroll: i32, row: i32, count: i32) -> BuyPreset {
    BuyPreset { scroll, row, count }
}

fn settings() -> ShopSettings {
    ShopSettings {
        bag_grid: Some(vec![100, 200, 408, 408]),
        bag_slot: Some(vec![110, 210, 51, 51]),
        shop_rows: 10,
        shop_region: Some(vec![100, 300, 400, 470]),
        scroll_point: Some(vec![700, 300]),
        max_button: Some(vec![800, 500]),
        presets: BTreeMap::from([("spring".to_owned(), preset(2, 1, 5))]),
        sell_slots: Vec::new(),
        sell_cap: 16,
    }
}

// ── the bag grid (`BagGridTests`, translated) ────────────────────────────────────────────────

fn grid() -> Option<Vec<i32>> {
    Some(vec![100, 200, 408, 408])
}

#[test]
fn bag_centres_are_row_major_with_the_first_at_the_top_left() {
    let centres = BagGrid::centres(grid().as_deref());

    assert_eq!(BagGrid::SLOT_COUNT, 64);
    assert_eq!(centres.len(), 64);

    // Slot 0 is the top-left cell's centre: half a pitch in from the grid's origin.
    assert_eq!(centres[0], point(126, 226));
    // Slot 63 is the bottom-right one, half a pitch in from the far corner.
    assert_eq!(centres[63], point(482, 582));

    // Reading order, not column order: slot 1 is to the right of slot 0, slot 8 is below it.
    assert!(centres[1].x > centres[0].x);
    assert_eq!(centres[0].y, centres[1].y);
    assert_eq!(centres[0].x, centres[8].x);
    assert!(centres[8].y > centres[0].y);
}

#[test]
fn bag_adjacent_centres_are_one_pitch_apart_everywhere() {
    let centres = BagGrid::centres(grid().as_deref());
    let px = BagGrid::pitch_x(grid().as_deref().unwrap());
    let py = BagGrid::pitch_y(grid().as_deref().unwrap());

    // The whole design assumes uniformity, so the derived points must stay on the pitch across all
    // eight columns and rows — not just near the origin.
    for row in 0..BagGrid::ROWS {
        for col in 1..BagGrid::COLS {
            let a = centres[row * BagGrid::COLS + col - 1];
            let b = centres[row * BagGrid::COLS + col];
            assert!((f64::from(b.x - a.x) - px).abs() <= 1.0);
        }
    }
    for col in 0..BagGrid::COLS {
        for row in 1..BagGrid::ROWS {
            let a = centres[(row - 1) * BagGrid::COLS + col];
            let b = centres[row * BagGrid::COLS + col];
            assert!((f64::from(b.y - a.y) - py).abs() <= 1.0);
        }
    }
}

#[test]
fn bag_pitch_is_the_grid_divided_by_eight() {
    let grid = grid();
    assert_eq!(BagGrid::pitch_x(grid.as_deref().unwrap()), 51.0);
    assert_eq!(BagGrid::pitch_y(grid.as_deref().unwrap()), 51.0);
}

/// The single slot box exists only to answer "is the grid actually uniform?". It is not a second
/// source of truth, so the two agreeing must be silent and disagreeing must be loud.
#[test]
fn a_bag_slot_that_matches_the_pitch_is_accepted() {
    assert_eq!(
        BagGrid::disagreement(grid().as_deref(), Some(&[110, 210, 51, 51])),
        None
    );
}

/// The case that shipped broken: a slot is naturally SMALLER than the pitch, because the pitch is
/// centre-to-centre and counts the gap while the slot box measures the slot itself. An exact-match
/// tolerance read a perfectly good 47 px slot as wrong and refused to save.
#[test]
fn a_bag_slot_smaller_than_the_pitch_by_a_gap_is_accepted() {
    let grid = grid();
    assert_eq!(
        BagGrid::disagreement(grid.as_deref(), Some(&[110, 210, 47, 47])),
        None
    );
    assert_eq!(
        BagGrid::implied_gap(grid.as_deref().unwrap(), Some(&[110, 210, 47, 47])),
        4.0
    );
}

#[test]
fn a_grossly_wrong_bag_slot_is_reported() {
    // Two slots dragged as one, or the icon only — the kind of error the check is actually for.
    let problem = BagGrid::disagreement(grid().as_deref(), Some(&[110, 210, 95, 20])).unwrap();

    assert!(problem.contains("95x20"), "{problem}");
    assert!(problem.contains("per slot"), "{problem}");
}

#[test]
fn invalid_bag_rectangles_are_rejected() {
    let invalid: [Option<&[i32]>; 5] = [
        None,
        Some(&[]),
        Some(&[1, 2, 3]),
        Some(&[1, 2, 0, 50]),  // zero width
        Some(&[1, 2, 50, -5]), // negative height
    ];

    for rect in invalid {
        assert!(!BagGrid::is_valid_rect(rect), "should be invalid: {rect:?}");
        assert!(BagGrid::centres(rect).is_empty());
    }
}

/// A half-set calibration is the normal state while dragging, so it must produce a message rather
/// than an exception or an empty overlay.
#[test]
fn bag_disagreement_copes_with_an_incomplete_calibration() {
    assert!(BagGrid::disagreement(Some(&[]), Some(&[1, 2, 30, 30])).is_some());
    assert!(BagGrid::disagreement(grid().as_deref(), Some(&[])).is_some());
}

// ── the shop rows (`ShopGeometryTests`, translated) ─────────────────────────────────────────

fn region() -> Option<Vec<i32>> {
    Some(vec![100, 300, 400, 470])
}

#[test]
fn shop_pitch_is_the_region_divided_by_the_row_count() {
    assert_eq!(ShopGeometry::row_pitch(region().as_deref(), 10), 47.0);
    assert_eq!(ShopGeometry::DEFAULT_ROWS, 10);
}

#[test]
fn shop_row_zero_is_half_a_pitch_down_from_the_top() {
    let centre = ShopGeometry::row_centre(region().as_deref(), 10, 0).unwrap();

    assert_eq!(centre.x, 100 + 400 / 2); // horizontally centred in the region
    assert_eq!(centre.y, 324); // half of 47 is 23.5, rounded half-to-even (C# Math.Round)
}

#[test]
fn the_last_shop_row_stays_on_the_pitch() {
    // The row that matters most: nine pitches down, still on the region rather than past it.
    let centre = ShopGeometry::row_centre(region().as_deref(), 10, 9).unwrap();

    assert_eq!(centre.y, 300 + 446);
    assert!(centre.y < 300 + 470);
}

#[test]
fn every_shop_row_is_one_pitch_apart() {
    let centres = ShopGeometry::row_centres(region().as_deref(), 10);

    assert_eq!(centres.len(), 10);
    for i in 1..centres.len() {
        let gap = centres[i].y - centres[i - 1].y;
        assert!((46..=48).contains(&gap), "gap {gap}");
    }
}

#[test]
fn a_different_row_count_changes_the_pitch() {
    // The row count is a property of the game, not of the setup — if a game update changes it, every
    // row moves, which is exactly why it is config rather than a constant.
    assert_eq!(
        ShopGeometry::row_pitch(region().as_deref(), 10),
        470.0 / 10.0
    );
    assert_eq!(ShopGeometry::row_pitch(region().as_deref(), 5), 470.0 / 5.0);
}

#[test]
fn an_undrawn_shop_region_is_refused_not_guessed() {
    assert!(ShopGeometry::problem(None, 10).is_some());
    assert!(ShopGeometry::problem(Some(&[]), 10).is_some());
    assert!(ShopGeometry::problem(region().as_deref(), 0).is_some());
    assert_eq!(ShopGeometry::problem(region().as_deref(), 10), None);

    assert_eq!(ShopGeometry::row_centre(None, 10, 0), None);
    assert!(ShopGeometry::row_centres(None, 10).is_empty());
}

#[test]
fn a_negative_shop_row_is_refused() {
    assert_eq!(ShopGeometry::row_centre(region().as_deref(), 10, -1), None);
}

// ── readiness: v2's Start refusals ──────────────────────────────────────────────────────────

fn error_message(result: Result<ShopPlan, seal_domain::decision::StopReason>) -> String {
    match result {
        Err(stop) => {
            assert_eq!(stop.kind, StopKind::Error);
            stop.message
        }
        Ok(_) => panic!("expected the run to be refused"),
    }
}

#[test]
fn selling_without_a_bag_grid_is_refused() {
    let mut settings = settings();
    settings.bag_grid = None;

    assert_eq!(
        error_message(ShopPlan::sell(&settings)),
        "The bag grid isn't calibrated — save the Bag grid on the Calibrate tab."
    );
}

#[test]
fn selling_without_a_max_button_is_refused() {
    let mut settings = settings();
    settings.max_button = None;

    assert_eq!(
        error_message(ShopPlan::sell(&settings)),
        "The MAX button isn't calibrated — save the MAX button on the Calibrate tab."
    );
}

/// The focus point's refusal is longer than the others on purpose (v2's comment): it explains WHY a
/// click is ignored, because "nothing happens" looks exactly like a wrong coordinate.
#[test]
fn selling_without_a_focus_point_explains_why_it_matters() {
    let mut settings = settings();
    settings.scroll_point = None;

    assert_eq!(
        error_message(ShopPlan::sell(&settings)),
        "The focus point isn't calibrated — save the Focus / scroll point on the Calibrate tab. The game ignores a click while it is \
         unfocused, and it is unfocused because you just clicked this window to start the run."
    );
}

#[test]
fn selling_with_no_slots_selected_is_refused() {
    assert_eq!(
        error_message(ShopPlan::sell(&settings())),
        "No slots are selected to sell."
    );
}

#[test]
fn selling_more_slots_than_the_cap_allows_is_refused_with_both_numbers() {
    let mut settings = settings();
    settings.sell_slots = (0..18).collect();
    settings.sell_cap = 16;

    assert_eq!(
        error_message(ShopPlan::sell(&settings)),
        "18 slots selected but the per-run cap is 16. Raise the cap on purpose, or narrow the \
         selection."
    );
}

#[test]
fn buying_without_a_preset_is_refused() {
    let mut settings = settings();
    settings.presets.clear();

    assert_eq!(
        error_message(ShopPlan::buy(&settings, None, 1)),
        "No buy preset selected."
    );
}

#[test]
fn buying_with_a_preset_that_is_gone_is_refused_by_name() {
    assert_eq!(
        error_message(ShopPlan::buy(&settings(), Some("nightly"), 1)),
        "Buy preset 'nightly' no longer exists."
    );
}

#[test]
fn buying_without_a_shop_region_says_which_calibration_is_missing() {
    let mut settings = settings();
    settings.shop_region = None;

    assert_eq!(
        error_message(ShopPlan::buy(&settings, Some("spring"), 1)),
        "Shop rows aren't calibrated — the shop list region isn't drawn."
    );
}

#[test]
fn buying_with_a_zero_row_count_says_so() {
    let mut settings = settings();
    settings.shop_rows = 0;

    assert_eq!(
        error_message(ShopPlan::buy(&settings, Some("spring"), 1)),
        "Shop rows aren't calibrated — the configured row count is 0."
    );
}

// ── the sell plan ────────────────────────────────────────────────────────────────────────────

/// Highest index first, distinct, only real slots, and never more than the cap. This is the loop
/// that destroys things, so the order rules live in one place.
#[test]
fn the_sell_slots_are_highest_first_distinct_and_capped() {
    let mut settings = settings();
    settings.sell_slots = vec![0, 5, 2, 5, 99, -1];
    settings.sell_cap = 5;

    assert_eq!(
        seal_domain::tools::shop::sell_slots_to_sell(&settings),
        vec![5, 2, 0]
    );

    settings.sell_cap = 2;
    assert_eq!(
        seal_domain::tools::shop::sell_slots_to_sell(&settings),
        vec![5, 2]
    );

    // `Enumerable.Take` with a non-positive count is empty, not an exception.
    settings.sell_cap = 0;
    assert!(seal_domain::tools::shop::sell_slots_to_sell(&settings).is_empty());
}

/// The plan is the ordered effect list v2 performs, waits included: focus, then per slot the right
/// click and the MAX/Enter/Enter transaction.
#[test]
fn one_sold_slot_is_the_full_ordered_transaction() {
    let mut settings = settings();
    settings.sell_slots = vec![0];
    settings.sell_cap = 16;

    let plan = ShopPlan::sell(&settings).unwrap();

    assert_eq!(
        plan.steps,
        vec![
            // Focus the game with a left click on the inert focus point.
            ShopStep::Place(point(700, 300)),
            ShopStep::Wait(0.35),
            ShopStep::Click(ClickButton::Left),
            ShopStep::Wait(0.45),
            // The sale itself.
            ShopStep::Place(point(126, 226)),
            ShopStep::Wait(0.35),
            ShopStep::Click(ClickButton::Right),
            ShopStep::Wait(0.45),
            // MAX, Enter, Enter.
            ShopStep::Place(point(800, 500)),
            ShopStep::Wait(0.35),
            ShopStep::Click(ClickButton::Left),
            ShopStep::Wait(0.45),
            ShopStep::Enter,
            ShopStep::Wait(0.45),
            ShopStep::Enter,
            ShopStep::Wait(0.45),
        ]
    );
    assert_eq!(plan.done_message, "Sold 1 slot(s).");
}

// ── the buy plan ────────────────────────────────────────────────────────────────────────────

/// A buy pass focuses, scrolls the preset's notches ONCE, then returns to the same derived row for
/// every purchase. The count comes from the RUN, not the preset: a different number should not mean
/// editing the preset.
#[test]
fn a_buy_pass_scrolls_once_and_repeats_the_row() {
    let plan = ShopPlan::buy(&settings(), Some("spring"), 2).unwrap();

    let steps_per_purchase = 12;
    let scroll_steps = 2;
    let focus_steps = 4;
    assert_eq!(
        plan.steps.len(),
        focus_steps + scroll_steps + 2 * steps_per_purchase
    );

    // Focus, then the preset's 2 notches down, then the settle wait.
    assert_eq!(plan.steps[0], ShopStep::Place(point(700, 300)));
    assert_eq!(plan.steps[2], ShopStep::Click(ClickButton::Left));
    assert_eq!(
        plan.steps[4],
        ShopStep::Scroll {
            notches: 2,
            down: true
        }
    );
    assert_eq!(plan.steps[5], ShopStep::Wait(scroll_settle(2)));

    // Row 1 of 10 in a 470 px region: 300 + round(1.5 * 47) = 370.
    assert_eq!(plan.steps[6], ShopStep::Place(point(300, 370)));
    assert_eq!(plan.steps[8], ShopStep::Click(ClickButton::Right));
    // The second purchase aims at the SAME row; nothing scrolls again.
    assert_eq!(plan.steps[18], ShopStep::Place(point(300, 370)));

    assert_eq!(plan.done_message, "Bought 2x 'spring'.");
}

#[test]
fn the_buy_count_comes_from_the_run_and_is_at_least_one() {
    let plan = ShopPlan::buy(&settings(), Some("spring"), 0).unwrap();

    assert_eq!(plan.done_message, "Bought 1x 'spring'.");
}

#[test]
fn a_preset_with_no_scroll_skips_the_scroll_steps() {
    let mut settings = settings();
    settings.presets.insert("plain".to_owned(), preset(0, 0, 1));

    let plan = ShopPlan::buy(&settings, Some("plain"), 1).unwrap();

    assert_eq!(plan.steps[4], ShopStep::Place(point(300, 324)));
}

/// Deliberately unclamped, as v2's `ShopGeometry.RowCentre` says in as many words: "asking for a row
/// past the bottom means the preset is wrong, and that is the caller's to report, not to round off".
#[test]
fn a_row_past_the_bottom_is_planned_where_it_lands() {
    let mut settings = settings();
    settings.presets.insert("far".to_owned(), preset(0, 99, 1));

    let plan = ShopPlan::buy(&settings, Some("far"), 1).unwrap();

    assert_eq!(plan.steps[4], ShopStep::Place(point(300, 4976)));
}

// ── mid-run failures (the loop reports them; the messages live here) ───────────────────────

#[test]
fn the_mid_run_failure_messages_are_v2s() {
    assert_eq!(
        seal_domain::tools::shop::focus_point_missing().message,
        "The focus point isn't calibrated — the game has to be focused before it will take a click \
         or a wheel notch. Save the Focus / scroll point on the Calibrate tab."
    );
    assert_eq!(
        seal_domain::tools::shop::window_missing().message,
        "Game window not found (or minimized) — open and restore the game first."
    );
    assert_eq!(
        seal_domain::tools::shop::placement_failed(point(30, 40), None).message,
        "couldn't place the cursor at (30,40)"
    );
    assert_eq!(
        seal_domain::tools::shop::placement_failed(point(30, 40), Some("gave up after 9 steps"))
            .message,
        "gave up after 9 steps"
    );
    assert_eq!(
        seal_domain::tools::shop::row_unknown().message,
        "Couldn't work out the shop row — save the Shop list region on the Calibrate tab."
    );
}

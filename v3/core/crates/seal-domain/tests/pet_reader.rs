//! The pet feeder's reading half, translated from `FeederEtaTests.cs` and `FeederLayoutTests.cs`.
//!
//! These are the two pure pieces the resident feeder's visit decisions rest on: what the boarding
//! window SAYS about time (the game's own answer to "when should this row be looked at again") and
//! where a row's food slots and count crops ARE, derived from one dragged strip rather than fourteen
//! hand-drawn boxes.
//!
//! The strings in the tests are REAL — read off a live window on 2026-09-21 and out of `pet.log` —
//! which is why they are quoted rather than invented.

use seal_domain::tools::pet::{FeederEta, FeederLayout, Reading};

fn lines(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn parse(values: &[&str]) -> Option<Reading> {
    FeederEta::parse(&lines(values))
}

// ── FeederEta: the boarding window's time line ─────────────────────────────────────────────

/// The line that prompted all of this: row 3's pet, 14 minutes from being mailed.
#[test]
fn the_completion_form_is_read() {
    let reading = parse(&["代養完成預計所需時間：約14分"]).unwrap();

    assert!(reading.is_completion);
    assert_eq!(reading.minutes, 14);
}

/// The level form parses — and must NOT report completion, or the row would be revisited every
/// twenty minutes for the whole life of the pet.
#[test]
fn the_level_form_is_read_but_is_not_completion() {
    let reading = parse(&["到2為止預計所需時間：約 20分"]).unwrap();

    assert!(!reading.is_completion);
    assert_eq!(reading.minutes, 20);
    assert_eq!(FeederEta::completion_minutes(Some(&reading)), None);
}

/// The 餵養值/growth the level form counts to — the GROWTH (+0 … +9), NOT the stage.
#[test]
fn the_level_form_names_the_growth_it_counts_to() {
    let cases = [
        ("到2為止預計所需時間：約 20分", Some(2)),
        ("到6為止预計所需時間：約19分", Some(6)),
        ("到9為止预計所需時間：約94分", Some(9)),
        ("到3为止预计所需时间：约 12分", Some(3)), // the OCR's Simplified 止, mixed scripts
        ("代養完成預計所需時間：約14分", None),    // the completion form names no growth
        ("該欄位約27日9時51分16秒後到期。", None), // the shelf life is not one either
        ("到6為止", Some(6)),
        ("到12為止預計所需時間：約 20分", None), // above the game's own maximum (+9)
    ];

    for (line, expected) in cases {
        assert_eq!(FeederEta::growth_target(Some(line)), expected, "{line}");
    }
}

/// The feeding rate the window prints beside every row, read per row because rows may hold pets at
/// different stages. The strings are the live OCR's OWN output: the dominant form has its item count
/// eaten (`每1分最取适`), and the one that survives arrives as `固`. The ending is not matched at all;
/// only the two numbers are.
#[test]
fn the_rate_line_is_read_per_row() {
    let cases = [
        ("每1分 攝取3個", Some(3)), // the clean form, from the 2026-09-21 measurement
        ("每 1 分 攝取 5 個", Some(5)),
        ("每2分 攝取6個", Some(3)), // "3 per minute", stated as 6 in 2
        ("每1分攝取4个", Some(4)),  // Simplified 个, no spaces
        ("每1分瑜取3固", Some(3)),  // LIVE: 個 arrives as 固
        ("每1分输取3固", Some(3)),
        ("每1分最取适", None), // LIVE and dominant: the item count is GONE
        ("每分最取尚", None),  // and the minute base too — nothing to read
        ("每1分最取适 到7為止预計所需時間：約25分", None), // joined lines: the 7 is not an item count
        ("該欄位約27日9時51分16 秒後到期。", None),
        ("代養完成預計所需時間：約14分", None),
        ("到6為止预計所需時間：約19分", None),
        ("每1分 攝取300個", None), // implausible: rejected, not clamped
        ("每0分 攝取3個", None),
    ];

    for (line, expected) in cases {
        assert_eq!(FeederEta::parse_rate(&lines(&[line])), expected, "{line}");
    }
}

/// THE TRAP: the food's shelf life carries a 約 and a number near a 分, and reading it as a countdown
/// would schedule a row twenty-seven days out.
#[test]
fn the_food_expiry_is_not_a_time_to() {
    assert_eq!(parse(&["該欄位約27日9時51分16 秒後到期。"]), None);
}

/// The rate line has no 約 and no countdown.
#[test]
fn the_rate_line_is_not_a_time_to() {
    assert_eq!(parse(&["每1分 攝取3個。"]), None);
}

/// The real window, all four lines together — the expiry first, and it must not win merely for
/// arriving first.
#[test]
fn the_whole_row_reads_as_completion() {
    let reading = parse(&[
        "該欄位約27日9時51分20 秒後到期。",
        "每1分 攝取3個。",
        "代養完成預計所需時間：約14分",
    ])
    .unwrap();

    assert!(reading.is_completion);
    assert_eq!(reading.minutes, 14);
    assert_eq!(FeederEta::completion_minutes(Some(&reading)), Some(14));
}

/// Nothing to read is an ordinary answer, not an error: the caller keeps the food figure.
#[test]
fn nothing_read_is_none() {
    assert_eq!(parse(&[]), None);
    assert_eq!(parse(&[""]), None);
    assert_eq!(
        parse(&["每1分 攝取3個。", "该栏位约27日9时51分16 秒后到期。"]),
        None
    );
    // v2's null-lines case is the empty slice in Rust, which is the `parse(&[])` above.
}

/// OCR mangles Chinese, so the completion test survives a damaged character or two — a missed
/// completion costs a cycle, never a wrong answer.
#[test]
fn a_mangled_completion_line_still_reads() {
    let reading = parse(&["代養完戌預計所需時間：約14分"]).unwrap();

    assert!(reading.is_completion);
    assert_eq!(reading.minutes, 14);
}

// ── FeederLayout: slots, count regions and the time-line band ─────────────────────────────

/// Row 2's real strip: five slots across, dragged as one strip.
fn strip() -> Vec<i32> {
    vec![349, 454, 317, 58]
}

#[test]
fn a_strip_divides_into_this_rows_stack_count() {
    assert_eq!(
        FeederLayout::slot_boxes(Some(&strip()), 2).unwrap().len(),
        2
    );
    assert_eq!(
        FeederLayout::slot_boxes(Some(&strip()), 5).unwrap().len(),
        5
    );
}

/// No gap and no overlap: a sliver left unclaimed at the right end is precisely where the count is
/// read from, so the segments must add up to the strip.
#[test]
fn the_slots_cover_the_strip_exactly() {
    let slots = FeederLayout::slot_boxes(Some(&strip()), 5).unwrap();

    assert_eq!(slots[0][0], 349);
    assert_eq!(slots[4][0] + slots[4][2], 666);
    for slot in &slots {
        assert_eq!(slot[1], 454);
        assert_eq!(slot[3], 58);
    }
}

#[test]
fn the_divisions_are_even_and_in_order() {
    let slots = FeederLayout::slot_boxes(Some(&strip()), 5).unwrap();

    for i in 1..slots.len() {
        assert_eq!(slots[i - 1][0] + slots[i - 1][2], slots[i][0]);
        assert!(slots[i][2] > 0);
    }

    let widths: Vec<i32> = slots.iter().map(|slot| slot[2]).collect();
    let (min, max) = (
        widths.iter().min().copied().unwrap(),
        widths.iter().max().copied().unwrap(),
    );
    assert!(max - min <= 1, "uneven slots: {widths:?}");
}

/// A half-calibrated row must yield NO slots, not a row of boxes at (0,0) that the tool would then
/// click and read.
#[test]
fn a_usable_strip_is_required() {
    assert_eq!(FeederLayout::slot_boxes(None, 5), None);
    assert_eq!(FeederLayout::slot_boxes(Some(&[]), 5), None);
    assert_eq!(FeederLayout::slot_boxes(Some(&[1, 2, 3]), 5), None);
    assert_eq!(FeederLayout::slot_boxes(Some(&[0, 0, 0, 0]), 5), None);
}

#[test]
fn a_usable_stack_count_is_required() {
    assert_eq!(FeederLayout::slot_boxes(Some(&strip()), 0), None);
    assert_eq!(FeederLayout::slot_boxes(Some(&strip()), -1), None);
}

#[test]
fn the_read_region_runs_from_the_fraction_to_the_slots_right_edge() {
    let slot = vec![344, 260, 66, 55];

    let region = FeederLayout::read_region(Some(&slot), FeederLayout::READ_LEFT_FRACTION).unwrap();

    assert_eq!(region[0], 344 + 26); // 40% of 66 is 26.4, rounded half-to-even
    assert_eq!(region[0] + region[2], 410); // the slot's own right edge
    assert_eq!(region[1], 260); // the slot's top
    assert_eq!(region[3], 55); // and FULL HEIGHT
}

/// The most expensive measurement in this feature, and unexplained: a crop holding a perfectly
/// legible number found ZERO detected boxes at digit height, and read at 1.00 at the slot's full
/// height. Whatever the mechanism, the height comes from the slot and never from a drawn box.
#[test]
fn the_read_region_is_full_height_because_a_short_one_reads_nothing() {
    let slot = vec![344, 260, 66, 55];

    assert_eq!(
        FeederLayout::read_region(Some(&slot), 0.40).unwrap()[3],
        slot[3]
    );
}

#[test]
fn the_fraction_is_a_setting_so_it_has_to_move_the_region() {
    let slot = vec![344, 260, 66, 55];

    let narrow = FeederLayout::read_region(Some(&slot), 0.55).unwrap();
    let wide = FeederLayout::read_region(Some(&slot), 0.25).unwrap();

    assert!(
        narrow[0] > wide[0],
        "a larger fraction must start further right"
    );
    assert_eq!(narrow[0] + narrow[2], slot[0] + slot[2]);
    assert_eq!(wide[0] + wide[2], slot[0] + slot[2]);
}

#[test]
fn a_bad_slot_is_not_a_read_region() {
    assert_eq!(FeederLayout::read_region(None, 0.40), None);
    assert_eq!(FeederLayout::read_region(Some(&[1, 2, 3]), 0.40), None);
    assert_eq!(FeederLayout::read_region(Some(&[0, 0, 0, 0]), 0.40), None);
}

/// The time-line band is derived as RATIOS of the slot height, so a machine whose UI scales gets a
/// proportionally placed box with nothing to calibrate.
#[test]
fn the_time_line_region_is_derived_from_the_strip_as_ratios() {
    let strip = vec![344, 645, 326, 60];

    let region = FeederLayout::eta_region(Some(&strip)).unwrap();

    assert_eq!(region[0], 344 - 6);
    assert_eq!(region[1], 645 + 60 + 12);
    assert_eq!(region[2], 282);
    assert_eq!(region[3], 72);
}

/// Widening the band is what stops a 7-pixel difference between rows being fatal, and the old band
/// has to stay INSIDE the new one — a row that read before must not stop reading.
#[test]
fn the_time_line_region_contains_the_band_it_used_to_aim_at() {
    let strip = vec![344, 645, 326, 60];
    let region = FeederLayout::eta_region(Some(&strip)).unwrap();
    let (top, bottom) = (region[1], region[1] + region[3]);

    assert!(
        top <= 645 + 60 + 32,
        "the band must still start at or above the old +32"
    );
    assert!(
        bottom >= 645 + 60 + 56,
        "and must still reach the old band's end of +56"
    );
}

#[test]
fn the_time_line_region_scales_with_the_slot_height() {
    let tall = FeederLayout::eta_region(Some(&[344, 645, 652, 120])).unwrap();

    assert_eq!(tall[1], 645 + 120 + 24);
    assert_eq!(tall[3], 144);
}

#[test]
fn an_unusable_strip_gives_no_time_line_region() {
    assert_eq!(FeederLayout::eta_region(None), None);
    assert_eq!(FeederLayout::eta_region(Some(&[1, 2])), None);
}

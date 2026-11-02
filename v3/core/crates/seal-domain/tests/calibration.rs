//! `CalibrationCheckTests.cs` translated — the seven v2 cases, plus the rules v2 relies on without a
//! test (marked AUTHORED in the test names).
//!
//! The comparison is pure, so it is tested here rather than through the Setup tab: the launcher is
//! `net8.0-windows` and stays out of the test project. The three fields are the ones that silently
//! invalidate every stored coordinate at once.

use seal_domain::calibration::differences;
use seal_domain::config::CalibrationInfo;
use seal_domain::geometry::{DisplayInfo, Rect, Size};

/// `(int)Math.Round` for the fixtures below; the values are the reference machine's measurement.
// The fixtures round small measured pixel values; the saturating cast cannot truncate a real one.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the fixtures round small measured pixel values, which the saturating cast cannot truncate"
)]
fn rounded(value: f64) -> i32 {
    value.round() as i32
}

/// v2's `Display()` helper: the reference machine's measurement (1.5 scale, 2865×1789 client,
/// 3840×2160 screen), with a frame and a logical client in the same shape `Dpi.Measure` produces.
fn display(scale: f64, screen_w: i32, screen_h: i32, client_w: i32, client_h: i32) -> DisplayInfo {
    DisplayInfo {
        scale,
        monitor_dpi: u32::try_from(rounded(96.0 * scale)).unwrap_or(0),
        screen: Size::new(screen_w, screen_h),
        frame: Rect::new(100, 100, client_w + 16, client_h + 39),
        physical_client: Rect::new(108, 131, client_w, client_h),
        logical_client: Rect::new(
            72,
            87,
            rounded(f64::from(client_w) / scale),
            rounded(f64::from(client_h) / scale),
        ),
    }
}

fn reference() -> DisplayInfo {
    display(1.5, 3840, 2160, 2865, 1789)
}

/// v2's `Stored()` helper. `None` for any field means the stored calibration does not carry it.
fn stored(
    scale: Option<f64>,
    client: Option<[i32; 2]>,
    screen: Option<[i32; 2]>,
) -> CalibrationInfo {
    CalibrationInfo {
        dpi_scale: scale,
        monitor_dpi: scale.map(|scale| u32::try_from(rounded(96.0 * scale)).unwrap_or(0)),
        screen: screen.map(|[w, h]| vec![w, h]),
        client_size: client.map(|[w, h]| vec![w, h]),
        measured_at: None,
    }
}

fn stored_reference() -> CalibrationInfo {
    stored(Some(1.5), Some([2865, 1789]), Some([3840, 2160]))
}

/// `AMatchingMachineReportsNothing`.
#[test]
fn a_matching_machine_reports_nothing() {
    assert_eq!(
        differences(&stored_reference(), &reference()),
        [] as [std::string::String; 0]
    );
}

/// `AScaleThatOnlyDiffersInTheLastDigitsIsNotAMismatch` — the stored scale is rounded to four places
/// on save, so a float that came back a hair off is the same scale.
#[test]
fn a_scale_that_only_differs_in_the_last_digits_is_not_a_mismatch() {
    let live = display(1.50003, 3840, 2160, 2865, 1789);

    assert_eq!(
        differences(
            &stored(Some(1.5), Some([2865, 1789]), Some([3840, 2160])),
            &live
        ),
        [] as [std::string::String; 0]
    );
}

/// `ADifferentScaleIsNamed` — and the message is the reader's description, not a number dump.
#[test]
fn a_different_scale_is_named() {
    let live = display(1.25, 3840, 2160, 2865, 1789);
    let diffs = differences(&stored_reference(), &live);

    assert_eq!(diffs, ["scale 1.5 → 1.25"]);
}

/// `ADifferentClientSizeIsNamed`.
#[test]
fn a_different_client_size_is_named() {
    let live = display(1.5, 3840, 2160, 1920, 1080);
    let diffs = differences(&stored_reference(), &live);

    assert_eq!(diffs, ["client 2865×1789 → 1920×1080"]);
}

/// `ADifferentScreenIsNamed`.
#[test]
fn a_different_screen_is_named() {
    let live = display(1.5, 2560, 1440, 2865, 1789);
    let diffs = differences(&stored_reference(), &live);

    assert_eq!(diffs, ["screen 3840×2160 → 2560×1440"]);
}

/// `EveryMovedFieldIsReportedTogether` — all three, in v2's order (scale, client, screen).
#[test]
fn every_moved_field_is_reported_together() {
    let live = display(1.25, 2560, 1440, 1920, 1080);

    assert_eq!(
        differences(&stored_reference(), &live),
        [
            "scale 1.5 → 1.25",
            "client 2865×1789 → 1920×1080",
            "screen 3840×2160 → 2560×1440",
        ]
    );
}

/// `WhatTheStoredCalibrationDoesNotCarryIsNotAMismatch` — an install that has never been calibrated
/// has nothing to be wrong about.
#[test]
fn what_the_stored_calibration_does_not_carry_is_not_a_mismatch() {
    let never_calibrated = stored(None, None, None);

    assert_eq!(
        differences(&never_calibrated, &reference()),
        [] as [std::string::String; 0]
    );
}

// ── the rules v2 relies on without a test (AUTHORED) ─────────────────────────────────────

/// AUTHORED: `CalibrationCheck.cs:32,38` test `is { Count: 2 }`, so a stored list of any other length
/// carries no claim and is ignored rather than reported or panicking.
#[test]
fn a_stored_pair_that_is_not_two_long_is_ignored() {
    for (client, screen) in [
        (Some(vec![2865]), Some(vec![3840])),
        (Some(vec![2865, 1789, 1]), Some(vec![3840, 2160, 2])),
        (Some(Vec::new()), Some(Vec::new())),
    ] {
        let mut stored = stored_reference();
        stored.client_size = client;
        stored.screen = screen;

        // The scale still matches, and the malformed pairs must produce nothing.
        assert_eq!(
            differences(&stored, &reference()),
            [] as [std::string::String; 0]
        );
    }
}

/// AUTHORED: the comparison only reads the STORED field against the live one, so a live measurement
/// that has nothing stored to compare with is not a mismatch — the Setup surface reports "nothing
/// stored yet" separately.
#[test]
fn an_empty_stored_calibration_is_silent_against_any_live_measurement() {
    let stored = stored(None, None, None);

    assert_eq!(
        differences(&stored, &display(1.0, 1920, 1080, 800, 600)),
        [] as [std::string::String; 0]
    );
}

/// AUTHORED: `Math.Abs(scale - live) > 0.001` is a strict comparison, so exactly the tolerance is not a
/// mismatch while slightly more is.
#[test]
fn the_scale_tolerance_boundary_is_strict() {
    let at_boundary = display(1.501, 3840, 2160, 2865, 1789);
    assert!(
        differences(&stored_reference(), &at_boundary).is_empty(),
        "exactly 0.001 is not a mismatch (v2 compares with >)"
    );

    let past_boundary = display(1.5011, 3840, 2160, 2865, 1789);
    assert_eq!(
        differences(&stored_reference(), &past_boundary),
        ["scale 1.5 → 1.501"]
    );
}

/// AUTHORED: C#'s `{0.###}` shape, which the player reads — at most three decimals with trailing zeros
/// dropped, never a `Debug` float.
#[test]
fn the_scale_is_described_the_way_v2_formats_it() {
    let live = display(1.0, 3840, 2160, 2865, 1789);
    let stored = stored(Some(1.25), Some([2865, 1789]), Some([3840, 2160]));

    assert_eq!(differences(&stored, &live), ["scale 1.25 → 1"]);
}

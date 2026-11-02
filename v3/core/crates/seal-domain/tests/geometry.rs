//! Geometry from `PROTOCOL.md` §3 and the coordinate arithmetic from `v2/SealTools.Core`'s
//! `Dpi.Measure` / `WindowFinder.ComputeCursorTarget`.
//!
//! The JSON in these tests is the shape the spec prints, so a rename of a field here is a protocol
//! change rather than a Rust style change.

use seal_domain::geometry::{CursorTarget, DisplayInfo, Point, Rect, Size};
use serde_json::json;

/// The reference machine's measurement: a 2865×1789 physical client that a DPI-unaware process sees
/// as 1910×1193, at 150%.
fn reference_display() -> DisplayInfo {
    DisplayInfo::from_rects(
        Rect::new(150, 75, 3183, 1966),
        Rect::new(150, 75, 2865, 1789),
        Rect::new(100, 50, 1910, 1193),
        Size::new(3840, 2160),
    )
}

#[test]
fn rect_uses_the_protocol_field_names() {
    let rect = Rect::new(12, 47, 2865, 1789);

    let json = serde_json::to_value(rect).unwrap();
    assert_eq!(json, json!({"x": 12, "y": 47, "w": 2865, "h": 1789}));

    let back: Rect = serde_json::from_value(json).unwrap();
    assert_eq!(back, rect);
}

#[test]
fn rect_derives_its_edges_centre_and_contains() {
    let rect = Rect::new(10, 20, 100, 50);

    assert_eq!(rect.right(), 110);
    assert_eq!(rect.bottom(), 70);
    assert_eq!(rect.center(), Point::new(60, 45));
    assert!(rect.contains(Point::new(10, 20)));
    assert!(rect.contains(Point::new(109, 69)));
    // The right/bottom edge is exclusive: the origin is the first pixel, the edge is one past it.
    assert!(!rect.contains(Point::new(110, 70)));
    assert!(!rect.is_empty());
    assert!(Rect::new(0, 0, 0, 50).is_empty());
}

#[test]
fn display_info_serialises_exactly_as_the_spec_prints_it() {
    let display = reference_display();

    let json = serde_json::to_value(display).unwrap();
    assert_eq!(
        json,
        json!({
            "scale": 1.5,
            "monitor_dpi": 144,
            "screen": {"w": 3840, "h": 2160},
            "frame": {"x": 150, "y": 75, "w": 3183, "h": 1966},
            "physical_client": {"x": 150, "y": 75, "w": 2865, "h": 1789},
            "logical_client": {"x": 100, "y": 50, "w": 1910, "h": 1193}
        })
    );

    let back: DisplayInfo = serde_json::from_value(json).unwrap();
    assert_eq!(back, display);
}

/// `Dpi.Measure` derives the scale from the two CLIENT WIDTHS: the heights truncate
/// (1193 · 1.5 = 1789.5 → 1789), so a height ratio would come back as 1.4996 and drift every
/// calibrated coordinate by a pixel or two.
#[test]
fn scale_is_measured_from_the_client_widths_not_the_heights() {
    let display = reference_display();

    assert_eq!(display.scale, 1.5);
    assert_eq!(display.monitor_dpi, 144);
    assert_eq!(display.screen, Size::new(3840, 2160));
}

/// A zero-width logical client cannot yield a ratio; `Dpi.Measure` answers 1.0 rather than an
/// infinity, and the port keeps that so a minimised or not-yet-created window measures instead of
/// poisoning every later division.
#[test]
fn a_zero_width_logical_client_measures_as_scale_one() {
    let display = DisplayInfo::from_rects(
        Rect::new(0, 0, 800, 600),
        Rect::new(0, 0, 800, 600),
        Rect::new(0, 0, 0, 0),
        Size::new(1920, 1080),
    );

    assert_eq!(display.scale, 1.0);
    assert_eq!(display.monitor_dpi, 96);
}

/// `WindowFinder.ComputeCursorTarget` — the two spaces a client-relative physical offset maps to.
/// 1.5 exactly here, so the arithmetic is visible: 30 / 1.5 = 20, 45 / 1.5 = 30.
#[test]
fn a_client_relative_offset_maps_into_both_cursor_spaces() {
    let display = reference_display();

    let target = display.cursor_target(30, 45);

    assert_eq!(
        target,
        CursorTarget {
            logical: Point::new(120, 80),
            physical: Point::new(180, 120),
        }
    );
}

/// The one place the port must NOT "fix" the v2 arithmetic: C# `Math.Round` is round-half-to-EVEN,
/// and `SetCursorPos` received those values on every run of v2. Rust's `f64::round` is half-away-from-
/// zero, so 0.5 → 1 would move a cursor v2 never moved.
#[test]
fn the_logical_rounding_matches_c_sharps_round_half_to_even() {
    let display = DisplayInfo::from_rects(
        Rect::new(0, 0, 1920, 1080),
        Rect::new(0, 0, 1920, 1080),
        Rect::new(0, 0, 960, 540),
        Size::new(1920, 1080),
    );

    // scale 2.0: 1/2 = 0.5 → 0 (even), 3/2 = 1.5 → 2 (even), 5/2 = 2.5 → 2 (even).
    assert_eq!(display.cursor_target(1, 3).logical, Point::new(0, 2));
    assert_eq!(display.cursor_target(5, 1).logical, Point::new(2, 0));
    // The physical side is unscaled: no rounding to disagree about.
    assert_eq!(display.cursor_target(5, 1).physical, Point::new(5, 1));
}

/// The protocol is the contract, so a field a later release adds must not fail a decode here — the
/// same tolerance `ConfigLoader` gets from `IgnoreUnmatchedProperties`.
#[test]
fn unknown_fields_are_tolerated_when_decoding() {
    let display: DisplayInfo = serde_json::from_value(json!({
        "scale": 1.0,
        "monitor_dpi": 96,
        "screen": {"w": 1920, "h": 1080},
        "frame": {"x": 0, "y": 0, "w": 800, "h": 600},
        "physical_client": {"x": 0, "y": 0, "w": 800, "h": 600},
        "logical_client": {"x": 0, "y": 0, "w": 800, "h": 600},
        "a_field_from_a_later_release": true
    }))
    .unwrap();

    assert_eq!(display.scale, 1.0);
}

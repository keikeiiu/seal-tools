//! Geometry, as `PROTOCOL.md` §3 prints it and as v2's `Dpi.Measure` derives it.
//!
//! Nothing here knows the screen exists: the core measures a window and hands these values over,
//! and the shell maps a drag back into client-relative offsets using only this data
//! (`PLAN-V3-CORE.md` §5). Stored calibration keeps its v2 meaning — physical client-relative
//! pixels — so `local.yaml` from v2 loads unchanged.

use serde::{Deserialize, Serialize};

/// An integer pixel rectangle: `{"x", "y", "w", "h"}`.
///
/// Physical screen pixels unless the field that holds it says otherwise. The protocol has no
/// separate "physical" and "logical" rect type — which space a rect is in is its producer's
/// contract, and [`DisplayInfo`]'s field names are where that contract is written down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    #[must_use]
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    /// One past the last covered column. Exclusive, like every pixel-rect convention the tools
    /// already use (`BagGrid`, the OCR sub-bands).
    #[must_use]
    pub const fn right(&self) -> i32 {
        self.x + self.w
    }

    /// One past the last covered row.
    #[must_use]
    pub const fn bottom(&self) -> i32 {
        self.y + self.h
    }

    /// The point a click aims at. Integer division truncates; v2's `ResultGemCenter` and the bag-cell
    /// derivations do the same, and a half-pixel disagreement between two of them is exactly the
    /// drift this type exists to stop.
    #[must_use]
    pub const fn center(&self) -> Point {
        Point::new(self.x + self.w / 2, self.y + self.h / 2)
    }

    /// Half-open containment: the origin is inside, `right()`/`bottom()` are not.
    #[must_use]
    pub const fn contains(&self, point: Point) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }

    /// Nothing to read or click. A drag that produced zero width is not a calibrated box.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }
}

/// A width and height with no origin, which is all `screen` carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub w: i32,
    pub h: i32,
}

impl Size {
    #[must_use]
    pub const fn new(w: i32, h: i32) -> Self {
        Self { w, h }
    }
}

/// A screen-space pixel. Shared by both coordinate spaces, so the type does not name one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// What the core measured, and the only input the shell is allowed to map a frame with
/// (`PROTOCOL.md` §3, `PLAN-V3-CORE.md` §5).
///
/// The core process is DPI-UNAWARE on purpose, so Win32 reports the *logical* client while the
/// screen blit is 1:1 *physical*. That asymmetry is load-bearing: it is why
/// [`Self::logical_client`] and [`Self::physical_client`] are both here, and why the scale is
/// measured rather than asked for.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DisplayInfo {
    /// Physical px per LOGICAL px, e.g. 1.5. Measured per call, never cached
    /// (`WindowFinder.cs`: "Re-query per call — never cache across a mixed-DPI multi-monitor move").
    pub scale: f64,
    /// Effective monitor DPI, `96 * scale`. Shown as a cross-check: `GetDpiForWindow` reports the
    /// WINDOW's awareness (96 for a DPI-unaware game) rather than the monitor's scale, so it cannot
    /// be the source of truth.
    pub monitor_dpi: u32,
    /// The primary screen, physical pixels.
    pub screen: Size,
    /// The window frame, physical. Includes the title bar and borders, so it must never be used as
    /// a coordinate origin — diagnostic only.
    pub frame: Rect,
    /// The client area, physical. The canonical origin of every calibrated coordinate.
    pub physical_client: Rect,
    /// The client area as the DPI-unaware core process sees it. What `SetCursorPos` counts in.
    pub logical_client: Rect,
}

impl DisplayInfo {
    /// Derives the display from the rects a measurement returns.
    ///
    /// The scale is the ratio of the CLIENT WIDTHS, exactly as `v2/SealTools.Core/Dpi.cs` measures
    /// it. Widths and not heights because the height ratio truncates: 1193 × 1.5 = 1789.5, which v2
    /// reports as 1789, so a height ratio comes back 1.4996 and every derived coordinate drifts.
    ///
    /// A zero-width logical client cannot yield a ratio; v2 answers 1.0 there rather than infinity,
    /// which keeps a minimised window measurable instead of poisoning every later division.
    #[must_use]
    pub fn from_rects(
        frame: Rect,
        physical_client: Rect,
        logical_client: Rect,
        screen: Size,
    ) -> Self {
        let scale = if logical_client.w > 0 {
            f64::from(physical_client.w) / f64::from(logical_client.w)
        } else {
            1.0
        };

        Self {
            scale,
            // A measured client width is positive, so 96 * scale is a normal DPI (96-384); a nonsense
            // negative ratio answers 0 rather than wrapping to a huge u32. Checked rather than cast so
            // the diagnostic field cannot silently describe a different monitor.
            monitor_dpi: u32::try_from(round_ties_even(96.0 * scale)).unwrap_or(0),
            screen,
            frame,
            physical_client,
            logical_client,
        }
    }

    /// Where a client-relative PHYSICAL offset lands on screen in both coordinate spaces.
    ///
    /// A pure port of `WindowFinder.ComputeCursorTarget`:
    /// - `logical  = logical_client origin  + round(offset / scale)` — what `SetCursorPos` wants;
    /// - `physical = physical_client origin + offset`               — what `SetPhysicalCursorPos` wants.
    ///
    /// The rounding is round-half-to-EVEN, because that is what C# `Math.Round` did. Rust's
    /// `f64::round` rounds half away from zero, so it would move a cursor to a pixel v2 never
    /// clicked; the difference is one pixel on a 2× monitor, which is the size of the errors this
    /// contract exists to prevent.
    #[must_use]
    pub fn cursor_target(&self, physical_x: i32, physical_y: i32) -> CursorTarget {
        CursorTarget {
            logical: Point::new(
                self.logical_client.x + round_ties_even(f64::from(physical_x) / self.scale),
                self.logical_client.y + round_ties_even(f64::from(physical_y) / self.scale),
            ),
            physical: Point::new(
                self.physical_client.x + physical_x,
                self.physical_client.y + physical_y,
            ),
        }
    }
}

/// The same client-relative offset expressed in the two spaces the cursor APIs take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorTarget {
    pub logical: Point,
    pub physical: Point,
}

/// `(int)Math.Round(x)`: nearest integer, ties to even, saturating at the i32 bounds.
// Rust's float-to-int `as` conversion SATURATES at the i32 bounds, which is exactly the documented
// contract here (C#'s unchecked cast is the one that is unspecified); an explicit clamp plus a cast
// would spell the same conversion less directly.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the saturating f64-to-i32 cast is the documented (int)Math.Round behaviour"
)]
pub(crate) fn round_ties_even(value: f64) -> i32 {
    // A non-finite value cannot arise from real rects; treating it as 0 keeps the function total
    // rather than panicking on an unrepresentable input.
    if !value.is_finite() {
        return 0;
    }

    value.round_ties_even() as i32
}

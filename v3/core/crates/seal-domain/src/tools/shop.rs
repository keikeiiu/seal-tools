//! The shop tools' decision half: the two derived geometries, the Start refusals, and the ordered
//! transaction plan for one Buy or Sell pass.
//!
//! Ported from `v2/SealTools.Core/BagGrid.cs`, `ShopGeometry.cs` and `SealTools.Shop/ShopTool.cs`.
//! The tool reads nothing — no OCR, no window measurement beyond the cursor placement the caller
//! performs — so its whole decision content is geometry, readiness and step order.
//!
//! # Why the geometries are DERIVED rather than marked
//!
//! **The bag** is a uniform 8×8, so 64 calibrated points would be 64 chances to drag one wrong.
//! Instead the calibrator takes two rectangles: the whole grid (whose pitch is what every cell is
//! derived from) and one slot (the independent check that the grid really is uniform — evidence, not
//! a second source of truth, which is why a disagreement is reported rather than averaged away).
//!
//! **The shop list** rows are derived the same way: drag the visible region, configure how many rows
//! the game shows in it, and every row centre follows. Two hand-clicked rows would give the same
//! information and could disagree with each other; one region cannot.
//!
//! # Why the plan is explicit
//!
//! Selling is the only irreversible thing in this suite: a mis-aimed click sells the wrong stack and
//! it is gone. So the transaction's ORDER, its measured waits, and its two guard messages live here
//! where a test can hold them, and the loop only turns [`ShopStep`]s into effects.

use crate::config::BuyPreset;
use crate::decision::StopReason;
use crate::geometry::{Point, round_ties_even};
use std::collections::BTreeMap;

/// The wait before a dialog step. Generous on purpose: a transaction that outruns the game's
/// animation sells the wrong thing, and the cost of waiting is a slower correct run.
pub const DIALOG_WAIT: f64 = 0.45;

/// The wait after placing the cursor and before clicking.
pub const CLICK_WAIT: f64 = 0.35;

/// Must match `WHEEL_NOTCH_GAP_MS` in `seal_mouse.ino`.
pub const WHEEL_GAP_SECONDS: f64 = 0.025;

/// How long the firmware needs to walk a scroll out: one gap per notch plus slack. A short wait is
/// worse than a long one here — the HID moves that follow queue behind the scroll on the board, so
/// placing the cursor too early reads a cursor that has not moved yet and keeps correcting against a
/// stale position.
pub fn scroll_settle(notches: i32) -> f64 {
    f64::from(notches) * WHEEL_GAP_SECONDS + 0.4
}

fn is_point(point: &Option<Vec<i32>>) -> bool {
    match point {
        Some(values) => values.len() == 2,
        None => false,
    }
}

fn as_point(point: &Option<Vec<i32>>) -> Option<Point> {
    match point {
        Some(values) if values.len() == 2 => Some(Point {
            x: values[0],
            y: values[1],
        }),
        _ => None,
    }
}

// ── the bag grid ─────────────────────────────────────────────────────────────────────────────

/// The bag grid, derived from two dragged rectangles. A marker type, ported from v2's static class.
pub struct BagGrid;

impl BagGrid {
    pub const COLS: usize = 8;
    pub const ROWS: usize = 8;
    pub const SLOT_COUNT: usize = Self::COLS * Self::ROWS;

    /// A calibrated rectangle is `[x, y, w, h]` in client-relative physical pixels.
    pub fn is_valid_rect(rect: Option<&[i32]>) -> bool {
        match rect {
            Some(rect) => rect.len() == 4 && rect[2] > 0 && rect[3] > 0,
            None => false,
        }
    }

    /// Horizontal pitch the whole-grid rectangle implies — a slot plus its gap. The rectangle must be
    /// valid ([`Self::is_valid_rect`]).
    pub fn pitch_x(grid: &[i32]) -> f64 {
        f64::from(grid[2]) / Self::COLS as f64
    }

    /// Vertical pitch the whole-grid rectangle implies.
    pub fn pitch_y(grid: &[i32]) -> f64 {
        f64::from(grid[3]) / Self::ROWS as f64
    }

    /// The 64 slot CENTRES as client-relative physical offsets, row-major — index 0 is the top-left
    /// slot, index 63 the bottom-right. Centres, not corners, because that is where a click lands.
    pub fn centres(grid: Option<&[i32]>) -> Vec<Point> {
        let Some(grid) = grid.filter(|grid| Self::is_valid_rect(Some(grid))) else {
            return Vec::new();
        };

        let pitch_x = Self::pitch_x(grid);
        let pitch_y = Self::pitch_y(grid);
        let mut centres = Vec::with_capacity(Self::SLOT_COUNT);
        for row in 0..Self::ROWS {
            for col in 0..Self::COLS {
                centres.push(Point {
                    x: grid[0] + round_ties_even((col as f64 + 0.5) * pitch_x),
                    y: grid[1] + round_ties_even((row as f64 + 0.5) * pitch_y),
                });
            }
        }
        centres
    }

    /// Where the single calibrated slot is too far from the pitch the whole-grid rectangle implies,
    /// as a human-readable reason — or `None` when it is plausible. Returned rather than thrown so
    /// the calibrator can show it while the player is still dragging.
    ///
    /// A slot is naturally SMALLER than the pitch — the pitch is centre-to-centre and includes the
    /// gap, while the slot box measures the slot itself — so an exact match is the wrong expectation.
    /// The tolerance allows a gap of up to a quarter of a cell and still catches a gross mis-drag,
    /// like a slot box drawn around two slots or the grid box around half the bag.
    pub fn disagreement(grid: Option<&[i32]>, slot: Option<&[i32]>) -> Option<String> {
        let Some(grid) = grid.filter(|grid| Self::is_valid_rect(Some(grid))) else {
            return Some("the grid area isn't set".to_owned());
        };
        let Some(slot) = slot.filter(|slot| Self::is_valid_rect(Some(slot))) else {
            return Some("the slot box isn't set".to_owned());
        };

        let pitch_x = Self::pitch_x(grid);
        let pitch_y = Self::pitch_y(grid);
        let tolerance_x = (pitch_x * 0.25).max(5.0);
        let tolerance_y = (pitch_y * 0.25).max(5.0);
        let dx = (f64::from(slot[2]) - pitch_x).abs();
        let dy = (f64::from(slot[3]) - pitch_y).abs();
        if dx <= tolerance_x && dy <= tolerance_y {
            return None;
        }

        Some(format!(
            "the slot box is {}x{} but the grid works out to {}x{} per slot. A slot is normally a \
             little smaller than that — the difference is the gap between slots — so this is too far \
             apart to be a gap. One of the two drags is wrong; re-drag the one that looks off.",
            slot[2],
            slot[3],
            round_ties_even(pitch_x),
            round_ties_even(pitch_y),
        ))
    }

    /// The gap between slots implied by the two rectangles, for display. Not used in the derivation —
    /// the slot box is evidence, not a second source of truth.
    pub fn implied_gap(grid: &[i32], slot: Option<&[i32]>) -> f64 {
        match slot {
            Some(slot) if Self::is_valid_rect(Some(grid)) && Self::is_valid_rect(Some(slot)) => {
                Self::pitch_x(grid) - f64::from(slot[2])
            }
            _ => 0.0,
        }
    }
}

// ── the shop list rows ──────────────────────────────────────────────────────────────────────

/// Where to click for a shop list row: `(scroll, then row)`, derived from the visible list region.
pub struct ShopGeometry;

impl ShopGeometry {
    /// The row count for this game's shop list. A config value rather than a constant so a game
    /// update that changes it does not need a code change — but deliberately NOT in the UI: it is a
    /// property of the game, not of anyone's setup, and a wrong value shifts every derived row.
    pub const DEFAULT_ROWS: i32 = 10;

    /// The most notches one scroll command may carry — the firmware's `WHEEL_MAX_NOTCHES`, mirrored
    /// here so the tool and the calibrator share ONE number. A guard against a malformed value, not
    /// the reach: a preset's scroll is applied from wherever the list already is.
    pub const MAX_SCROLL_NOTCHES: i32 = 400;

    pub fn is_valid_region(region: Option<&[i32]>) -> bool {
        BagGrid::is_valid_rect(region)
    }

    /// Centre-to-centre row spacing implied by the region and the row count.
    pub fn row_pitch(region: Option<&[i32]>, rows: i32) -> f64 {
        match region {
            Some(region) if Self::is_valid_region(Some(region)) && rows > 0 => {
                f64::from(region[3]) / f64::from(rows)
            }
            _ => 0.0,
        }
    }

    /// Centre of the row at `index` in the visible list, or `None` when the region isn't drawn. Row 0
    /// is the top one. Deliberately NOT clamped: asking for a row past the bottom means the preset is
    /// wrong, and that is the caller's to report, not to round off.
    pub fn row_centre(region: Option<&[i32]>, rows: i32, index: i32) -> Option<Point> {
        let region = region.filter(|region| Self::is_valid_region(Some(region)))?;
        if rows < 1 || index < 0 {
            return None;
        }
        let pitch = Self::row_pitch(Some(region), rows);
        if pitch <= 0.0 {
            return None;
        }
        Some(Point {
            x: region[0] + region[2] / 2,
            y: region[1] + round_ties_even((f64::from(index) + 0.5) * pitch),
        })
    }

    /// Every row centre, for the calibrator's overlay.
    pub fn row_centres(region: Option<&[i32]>, rows: i32) -> Vec<Point> {
        let mut centres = Vec::with_capacity(rows.max(0) as usize);
        for index in 0..rows.max(0) {
            if let Some(centre) = Self::row_centre(region, rows, index) {
                centres.push(centre);
            }
        }
        centres
    }

    /// Why the row geometry can't be used, or `None` when it can.
    pub fn problem(region: Option<&[i32]>, rows: i32) -> Option<&'static str> {
        if !Self::is_valid_region(region) {
            return Some("the shop list region isn't drawn");
        }
        if rows < 1 {
            return Some("the configured row count is 0");
        }
        None
    }
}

// ── the settings the pass reads ────────────────────────────────────────────────────────────

/// The buy/sell block the decision half needs, resolved from the merged config.
///
/// Field sources: `buy_sell` in the merged config — `bag_grid`/`bag_slot`/`shop_rows`/`shop_region`/
/// `scroll_point`/`max_button`/`presets`/`sell_slots`/`sell_cap` (machine half in `local.yaml`; the
/// presets are personal and live there too).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShopSettings {
    pub bag_grid: Option<Vec<i32>>,
    pub bag_slot: Option<Vec<i32>>,
    pub shop_rows: i32,
    pub shop_region: Option<Vec<i32>>,
    /// Calibrated point that is left-clicked to focus the game. It must be somewhere INERT: this is
    /// a real click, so a point over a list row would select or buy that row.
    pub scroll_point: Option<Vec<i32>>,
    pub max_button: Option<Vec<i32>>,
    pub presets: BTreeMap<String, BuyPreset>,
    pub sell_slots: Vec<i32>,
    pub sell_cap: i32,
}

/// Which mouse button a transaction step clicks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickButton {
    Left,
    Right,
}

/// One effect in a pass, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum ShopStep {
    /// Move the cursor to this client-relative physical offset (the caller runs the closed loop).
    Place(Point),
    /// Click wherever the cursor was just placed.
    Click(ClickButton),
    /// The board's Enter command — the count dialog's confirm and the purchase confirmation.
    Enter,
    /// Wheel notches; `down` true is the board's `Z`, false its `Q`.
    Scroll { notches: i32, down: bool },
    /// Sleep the given seconds — the measured waits between transaction steps.
    Wait(f64),
}

/// One pass, ready to execute: the ordered steps and the line the player sees when it finishes.
///
/// Buy and Sell are counted jobs, not loops — running them forever would be a way to spend a fortune
/// or empty a bag by leaving the room.
#[derive(Debug, Clone, PartialEq)]
pub struct ShopPlan {
    pub steps: Vec<ShopStep>,
    pub done_message: String,
}

impl ShopPlan {
    /// One BUY pass: focus, scroll the preset's notches once, then click the same derived row and run
    /// the MAX/Enter/Enter transaction `count` times.
    ///
    /// `count` comes from the RUN, not the preset: a preset's own count is its usual amount, and
    /// asking for a different number should not mean editing it. It is clamped to at least one.
    pub fn buy(
        settings: &ShopSettings,
        preset_name: Option<&str>,
        count: i32,
    ) -> Result<Self, StopReason> {
        let preset = ready_buy(settings, preset_name)?;
        let count = count.max(1);

        let mut steps = focus_steps(settings)?;
        if preset.scroll > 0 {
            steps.push(ShopStep::Scroll {
                notches: preset.scroll,
                down: true,
            });
            steps.push(ShopStep::Wait(scroll_settle(preset.scroll)));
        }

        let row = ShopGeometry::row_centre(
            settings.shop_region.as_deref(),
            settings.shop_rows,
            preset.row,
        )
        .ok_or_else(row_unknown)?;
        let Some(max) = as_point(&settings.max_button) else {
            return Err(max_button_missing());
        };

        for _ in 0..count {
            steps.extend(transaction(row, max));
        }

        Ok(Self {
            steps,
            done_message: format!("Bought {count}x '{}'.", preset_name.unwrap_or_default()),
        })
    }

    /// One SELL pass: focus, then per selected slot — highest index first — right-click the bag cell
    /// and run the same MAX/Enter/Enter transaction.
    pub fn sell(settings: &ShopSettings) -> Result<Self, StopReason> {
        ready_sell(settings)?;

        let slots = sell_slots_to_sell(settings);
        let centres = BagGrid::centres(settings.bag_grid.as_deref());
        let Some(max) = as_point(&settings.max_button) else {
            return Err(max_button_missing());
        };

        let mut steps = focus_steps(settings)?;
        for slot in &slots {
            steps.extend(transaction(centres[*slot as usize], max));
        }

        Ok(Self {
            steps,
            done_message: format!("Sold {} slot(s).", slots.len()),
        })
    }
}

/// The slots one sell pass will sell: the configured selection filtered to real bag cells,
/// de-duplicated, HIGHEST INDEX FIRST, and capped.
///
/// Highest first is correct whether or not the bag compacts after a sale — ascending would silently
/// skip items if it does, because removing slot 0 shifts slot 1 into it. The cap is re-applied here
/// even though readiness checked it, because this is the loop that destroys things.
pub fn sell_slots_to_sell(settings: &ShopSettings) -> Vec<i32> {
    let centres = BagGrid::centres(settings.bag_grid.as_deref());

    let mut slots: Vec<i32> = Vec::new();
    for slot in &settings.sell_slots {
        let valid = *slot >= 0 && (*slot as usize) < centres.len();
        if valid && !slots.contains(slot) {
            slots.push(*slot);
        }
    }
    slots.sort_unstable_by(|left, right| right.cmp(left));

    let cap = settings.sell_cap.max(0) as usize;
    slots.truncate(cap);
    slots
}

/// v2's `Ready`, sell branch. Returns what cannot be done as a player-facing reason.
fn ready_sell(settings: &ShopSettings) -> Result<(), StopReason> {
    if !BagGrid::is_valid_rect(settings.bag_grid.as_deref()) {
        return Err(StopReason::error(
            "The bag grid isn't calibrated — save the Bag grid on the Calibrate tab.",
        ));
    }
    if !is_point(&settings.max_button) {
        return Err(max_button_missing());
    }
    if !is_point(&settings.scroll_point) {
        // A DIFFERENT sentence from the mid-run focus failure: at Start the explanation is why the
        // first click will be ignored, and it ends by telling the player where the mark is.
        return Err(StopReason::error(
            "The focus point isn't calibrated — save the Focus / scroll point on the Calibrate tab. The game ignores a click while \
             it is unfocused, and it is unfocused because you just clicked this window to start the \
             run.",
        ));
    }
    if settings.sell_slots.is_empty() {
        return Err(StopReason::error("No slots are selected to sell."));
    }
    if settings.sell_slots.len() as i32 > settings.sell_cap {
        return Err(StopReason::error(format!(
            "{} slots selected but the per-run cap is {}. Raise the cap on purpose, or narrow the \
             selection.",
            settings.sell_slots.len(),
            settings.sell_cap
        )));
    }
    Ok(())
}

/// v2's `Ready`, buy branch.
fn ready_buy<'a>(
    settings: &'a ShopSettings,
    preset_name: Option<&str>,
) -> Result<&'a BuyPreset, StopReason> {
    let Some(name) = preset_name else {
        return Err(StopReason::error("No buy preset selected."));
    };
    let Some(preset) = settings.presets.get(name) else {
        return Err(StopReason::error(format!(
            "Buy preset '{name}' no longer exists."
        )));
    };
    if let Some(problem) =
        ShopGeometry::problem(settings.shop_region.as_deref(), settings.shop_rows)
    {
        return Err(StopReason::error(format!(
            "Shop rows aren't calibrated — {problem}."
        )));
    }
    if !is_point(&settings.scroll_point) {
        return Err(StopReason::error(
            "The scroll point isn't calibrated — save the Focus / scroll point on the Calibrate tab.",
        ));
    }
    if !is_point(&settings.max_button) {
        return Err(max_button_missing());
    }
    Ok(preset)
}

/// The focus click both passes start with.
///
/// BOTH flows need it: starting a tool means clicking the launcher, and that takes focus away — after
/// which the game ignores a wheel notch AND a right-click. So it must be a real click, which is why
/// the point has to be inert.
fn focus_steps(settings: &ShopSettings) -> Result<Vec<ShopStep>, StopReason> {
    let Some(point) = as_point(&settings.scroll_point) else {
        return Err(focus_point_missing());
    };
    Ok(vec![
        ShopStep::Place(point),
        ShopStep::Wait(CLICK_WAIT),
        ShopStep::Click(ClickButton::Left),
        ShopStep::Wait(DIALOG_WAIT),
    ])
}

/// The half of a transaction that is identical for buying and selling.
fn transaction(target: Point, max: Point) -> Vec<ShopStep> {
    vec![
        ShopStep::Place(target),
        ShopStep::Wait(CLICK_WAIT),
        // RIGHT click: a shop row takes a right-click to open the count dialog — a left click does
        // nothing, which is what made the first live buy silently no-op.
        ShopStep::Click(ClickButton::Right),
        ShopStep::Wait(DIALOG_WAIT),
        // MAX, Enter for the count dialog, Enter again for the confirmation.
        ShopStep::Place(max),
        ShopStep::Wait(CLICK_WAIT),
        ShopStep::Click(ClickButton::Left),
        ShopStep::Wait(DIALOG_WAIT),
        ShopStep::Enter,
        ShopStep::Wait(DIALOG_WAIT),
        ShopStep::Enter,
        ShopStep::Wait(DIALOG_WAIT),
    ]
}

// ── the messages for failures the LOOP reports (the decision half cannot see the board) ─────

/// The focus point was missing when the pass tried to use it.
pub fn focus_point_missing() -> StopReason {
    StopReason::error(
        "The focus point isn't calibrated — the game has to be focused before it will take a click \
         or a wheel notch. Save the Focus / scroll point on the Calibrate tab.",
    )
}

/// The MAX button was missing when the pass tried to use it.
fn max_button_missing() -> StopReason {
    StopReason::error("The MAX button isn't calibrated — save the MAX button on the Calibrate tab.")
}

/// The shop row could not be derived from the region.
pub fn row_unknown() -> StopReason {
    StopReason::error(
        "Couldn't work out the shop row — save the Shop list region on the Calibrate tab.",
    )
}

/// The game window was not found (or is minimized) when a placement was attempted.
pub fn window_missing() -> StopReason {
    StopReason::error("Game window not found (or minimized) — open and restore the game first.")
}

/// The closed-loop placement failed. `detail` is the placement's own error when it has one;
/// otherwise the message names the point, as v2 does.
pub fn placement_failed(target: Point, detail: Option<&str>) -> StopReason {
    StopReason::error(match detail {
        Some(detail) => detail.to_owned(),
        None => format!("couldn't place the cursor at ({},{})", target.x, target.y),
    })
}

//! The tuner's decision half: what one OCR read means, what ends the run, and what the two guard
//! paths do.
//!
//! Ported from `v2/SealTools.Tuner/SealTuner.cs`'s run loop and `v2/SealTools.Core/GradeCycles.cs`.
//! v2 could not test the loop without a serial port, a game window and a calibration; the loop's
//! decision content — the stop conditions, the guards' messages, the countdown — is what is here.
//!
//! # The read contract
//!
//! [`TunerRead`] is one scan's result:
//!
//! - `grade` — the grade the OCR read off the grade line (`OcrEngine.Scan().Grade`). `None` when it
//!   recognised nothing there.
//! - `remaining` — the remaining spring count from the remaining-time row.
//! - `attributes` — the matched attribute lines from [`crate::ocr::AttrMatcher`], in read order. The
//!   matcher is run by the reader side; this half only decides.
//!
//! A scan where all three are empty is a READ FAILURE, not a result to compare.
//!
//! # Stop kinds
//!
//! The protocol has four finished reasons (`PROTOCOL.md` §5). The mapping chosen here, recorded
//! because v2 has no such vocabulary: reaching the target / matching the filter → `Done`; the game
//! running out of springs or never changing → `Done` (the run ended as the game made it end, not as a
//! tool failure); a total read failure or a spring-placement failure → `Error` (v2 put these on the
//! card's red bar); the mouse guard tripping → `Guard`.

use crate::config::{BoxConfig, FilterConfig, OcrGeometry, RegionConfig};
use crate::decision::StopReason;
use crate::geometry::{Point, Rect};
use crate::ocr::{AttrMatcher, MatchedAttr};

/// The Start countdown v2 shows before the first click ("5... 4... ..."), in seconds.
pub const START_COUNTDOWN: i32 = 5;

/// Where one scan came from, as the decision half needs it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TunerRead {
    pub grade: Option<String>,
    pub remaining: Option<i32>,
    pub attributes: Vec<MatchedAttr>,
}

impl TunerRead {
    /// True when the OCR recognised nothing at all. v2 treats this as a failure and stops: a total
    /// failure produces exactly the empty signature the run starts with, so it would otherwise count
    /// as the first repeat and trip "same result x3" after only two failed reads.
    pub fn is_total_failure(&self) -> bool {
        self.grade.is_none() && self.remaining.is_none() && self.attributes.is_empty()
    }

    /// The repeat signature v2 compares: grade, remaining, and the attribute lines joined as
    /// `name=value|name=value`. The order is the read order, which is why a changed line breaks the
    /// chain even when the set is the same.
    pub fn signature(&self) -> TunerSignature {
        TunerSignature {
            grade: self.grade.clone(),
            remaining: self.remaining,
            attributes: self
                .attributes
                .iter()
                .map(|attribute| {
                    format!(
                        "{}={}",
                        attribute.name,
                        attribute
                            .value
                            .map(|value| value.to_string())
                            .unwrap_or_default()
                    )
                })
                .collect(),
        }
    }
}

/// The three fields that decide whether a read is a repeat of the last one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunerSignature {
    pub grade: Option<String>,
    pub remaining: Option<i32>,
    pub attributes: String,
}

/// Which cursor placement the setup uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpringMode {
    /// "manual": the player positions the mouse on the 發條 button; the tool only sends C+E.
    Manual,
    /// "hid": the Arduino places the cursor closed-loop and, with a guard, keeps it there.
    Hid,
}

impl SpringMode {
    /// v2's comparison is `spring_mode == "hid"`, so anything else is manual behaviour.
    pub fn parse(text: &str) -> Self {
        if text == "hid" {
            Self::Hid
        } else {
            Self::Manual
        }
    }
}

/// What the run does when the mouse drifts off the spring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseGuard {
    Off,
    Stop,
    Recenter,
}

impl MouseGuard {
    /// v2's if/else: `"off"` never looks, `"stop"` halts, and ANYTHING ELSE takes the recenter
    /// branch — because that is what the final `else` does.
    pub fn parse(text: &str) -> Self {
        match text {
            "off" => Self::Off,
            "stop" => Self::Stop,
            _ => Self::Recenter,
        }
    }
}

/// The tuner settings the decision half reads, resolved from the merged config before the run starts.
///
/// Field sources in the merged config (`tuner` block, portable half in `local.yaml`'s `settings:`):
/// `grade_order`, `target_grade`, `max_retries`, `spring_mode`, `mouse_guard`, `guard_px`,
/// `recenter_max`, `filter`.
#[derive(Debug, Clone, PartialEq)]
pub struct TunerSettings {
    pub grade_order: Vec<String>,
    pub target_grade: String,
    pub max_retries: i32,
    pub spring_mode: SpringMode,
    pub mouse_guard: MouseGuard,
    pub guard_px: i32,
    pub recenter_max: i32,
    pub filter: FilterConfig,
}

/// What to do after one read.
#[derive(Debug, Clone, PartialEq)]
pub enum TunerDecision {
    /// Click + Enter again and read once more.
    Retry,
    Stop(StopReason),
}

/// The decision, plus the filter line the card shows.
#[derive(Debug, Clone, PartialEq)]
pub struct TunerOutcome {
    pub decision: TunerDecision,
    /// `filter.disabled` when the filter is off, else the filter's reason sentence
    /// (`AttrMatcher::CheckFilter`). v2 puts exactly this on the card's filter line.
    pub filter_status: String,
}

impl TunerOutcome {
    pub fn filter_status(&self) -> &str {
        &self.filter_status
    }
}

/// One tuning run's state: the attempt counter and the repeat chain.
#[derive(Debug, Clone, PartialEq)]
pub struct TunerRun {
    settings: TunerSettings,
    attempt: i32,
    consecutive_repeats: i32,
    previous: Option<TunerSignature>,
}

impl TunerRun {
    pub fn new(settings: TunerSettings) -> Self {
        Self {
            settings,
            attempt: 0,
            consecutive_repeats: 0,
            previous: None,
        }
    }

    pub fn settings(&self) -> &TunerSettings {
        &self.settings
    }

    /// The attempts started so far.
    pub fn attempt(&self) -> i32 {
        self.attempt
    }

    /// Repeats of the previous result; `2` means three identical results in a row.
    pub fn consecutive_repeats(&self) -> i32 {
        self.consecutive_repeats
    }

    /// Counts a new attempt before it is sent — v2 increments the attempt before clicking.
    pub fn begin_attempt(&mut self) -> i32 {
        self.attempt += 1;
        self.attempt
    }

    /// Turns one scan into the next action, and updates the repeat chain.
    ///
    /// The stop conditions are checked in v2's order, which is load-bearing: reaching the target (or
    /// matching the filter) outranks running out of springs; out of springs outranks the repeat
    /// counter; the retry ceiling is last.
    pub fn decide(&mut self, read: &TunerRead) -> TunerOutcome {
        let filter_result = AttrMatcher::check_filter(&read.attributes, &self.settings.filter);
        let filter_status = if self.settings.filter.enabled {
            filter_result.reason.clone()
        } else {
            "disabled".to_owned()
        };

        if read.is_total_failure() {
            return TunerOutcome {
                decision: TunerDecision::Stop(StopReason::error(
                    "Read failed — nothing recognised in the OCR region.",
                )),
                filter_status,
            };
        }

        let signature = read.signature();
        if self.previous.as_ref() == Some(&signature) {
            self.consecutive_repeats += 1;
        } else {
            self.consecutive_repeats = 0;
        }
        self.previous = Some(signature);

        let target = &self.settings.target_grade;
        let require_grade = self.settings.filter.require_grade.as_deref();
        // v2: a require_grade that is present, not blank, and not the literal "false" replaces the
        // target; everything else leaves target_grade in charge.
        let effective_grade = match require_grade {
            Some(required)
                if !required.trim().is_empty() && !required.eq_ignore_ascii_case("false") =>
            {
                required
            }
            _ => target.as_str(),
        };
        let filter_ok = if self.settings.filter.enabled {
            filter_result.passed
        } else {
            true
        };
        let grade_ok = read
            .grade
            .as_deref()
            .map(|grade| self.grade_index(grade) >= self.grade_index(effective_grade))
            .unwrap_or(false);

        let decision = if grade_ok && filter_ok {
            let grade = read.grade.as_deref().unwrap_or("");
            let message = if self.settings.filter.enabled {
                format!(">>> {grade} REACHED + FILTER MATCH <<<")
            } else {
                format!(">>> {grade} REACHED (no filter) <<<")
            };
            TunerDecision::Stop(StopReason::done(message))
        } else if self.settings.filter.enabled && require_grade.is_none() && filter_result.passed {
            let grade = read.grade.as_deref().unwrap_or("");
            TunerDecision::Stop(StopReason::done(format!(
                ">>> FILTER MATCHED at grade {grade} (no grade requirement) <<<"
            )))
        } else if read.remaining.is_some_and(|remaining| remaining <= 0) {
            TunerDecision::Stop(StopReason::done(">>> OUT OF SPRINGS <<<"))
        } else if self.consecutive_repeats >= 2 {
            TunerDecision::Stop(StopReason::done(
                ">>> STUCK / OUT OF SPRINGS (same result x3) <<<",
            ))
        } else if self.attempt >= self.settings.max_retries {
            TunerDecision::Stop(StopReason::done(format!(
                "[!] Max {}",
                self.settings.max_retries
            )))
        } else {
            TunerDecision::Retry
        };

        TunerOutcome {
            decision,
            filter_status,
        }
    }

    /// `List.IndexOf(grade)`: the position in `grade_order`, or -1.
    ///
    /// The -1 is observable and is kept: a read grade that is in neither the order nor the target makes
    /// both indexes -1, and `-1 >= -1` counts as a reached target. That is v2's behaviour and it is
    /// pinned by a test rather than "fixed".
    fn grade_index(&self, grade: &str) -> i32 {
        self.settings
            .grade_order
            .iter()
            .position(|known| known == grade)
            .map(|index| index as i32)
            .unwrap_or(-1)
    }
}

// ── the mouse guard (spring_mode: hid only) ───────────────────────────────────────────────────

/// What the cursor guard says before the next click.
#[derive(Debug, Clone, PartialEq)]
pub enum MouseGuardStep {
    /// Carry on. `drift_free` is true only when a measurement was taken and showed no drift, which is
    /// when v2 resets its runaway-recenter counter.
    Proceed {
        drift_free: bool,
    },
    /// Put the cursor back on the spring, then report the outcome to [`after_recenter`].
    Recenter,
    Stop(StopReason),
}

/// `CheckMouseGuard`, as a pure step.
///
/// `target` is the spring point in the LOGICAL cursor space (`DisplayInfo::cursor_target(...).logical`
/// — the caller computes it because that needs the measured display), or `None` when the game window
/// could not be found. `cursor` is the current logical cursor position, or `None` when unreadable.
///
/// Both `None`s FAIL OPEN, as v2 documents rather than fixes: a window that has gone away turns the
/// guard off instead of tripping it. The recorded reason is that it needs the window to move
/// mid-run.
pub fn mouse_guard_step(
    settings: &TunerSettings,
    target: Option<Point>,
    cursor: Option<Point>,
    _recenter_count: i32,
) -> MouseGuardStep {
    if settings.mouse_guard == MouseGuard::Off {
        return MouseGuardStep::Proceed { drift_free: false };
    }

    let (Some(target), Some(cursor)) = (target, cursor) else {
        return MouseGuardStep::Proceed { drift_free: false };
    };

    // v2 measures the larger axis in LOGICAL pixels and compares with `>`, so a cursor exactly
    // guard_px away has not drifted.
    let drift = (cursor.x - target.x).abs().max((cursor.y - target.y).abs());
    if drift <= settings.guard_px {
        return MouseGuardStep::Proceed { drift_free: true };
    }

    match settings.mouse_guard {
        MouseGuard::Stop => MouseGuardStep::Stop(StopReason::guard("mouse moved — stopped")),
        // Off was handled above; anything else is the recenter branch, as in v2.
        _ => MouseGuardStep::Recenter,
    }
}

/// What the guard decides after a recentre attempt. `recenter_count` is the count AFTER this attempt
/// (v2 increments before comparing).
pub fn after_recenter(
    settings: &TunerSettings,
    placed: Result<(), &str>,
    recenter_count: i32,
) -> MouseGuardStep {
    if let Err(detail) = placed {
        return MouseGuardStep::Stop(StopReason::error(format!(
            "cursor wouldn't re-centre — {detail}"
        )));
    }
    if recenter_count > settings.recenter_max {
        return MouseGuardStep::Stop(StopReason::guard("cursor keeps drifting — stopped"));
    }
    MouseGuardStep::Proceed { drift_free: false }
}

// ── placing the cursor on the spring before the first attempt (spring_mode: hid) ─────────────

/// What has to happen before the first click.
#[derive(Debug, Clone, PartialEq)]
pub enum SpringStep {
    /// `manual` mode: the player positions the mouse.
    NotNeeded,
    /// Place the cursor at this client-relative PHYSICAL point, then report the outcome to
    /// [`spring_failed`] or [`spring_window_missing`].
    Place {
        point: Point,
    },
    Stop(StopReason),
}

/// `PlaceOnSpring`, as a pure step.
pub fn spring_step(settings: &TunerSettings, spring_point: Option<Point>) -> SpringStep {
    if settings.spring_mode != SpringMode::Hid {
        return SpringStep::NotNeeded;
    }
    match spring_point {
        None => SpringStep::Stop(StopReason::error(
            "spring_mode is hid but tuner.spring_point isn't calibrated — save the Spring point on \
             the Calibrate tab",
        )),
        Some(point) => SpringStep::Place { point },
    }
}

/// The game window was not found before the placement.
pub fn spring_window_missing() -> StopReason {
    StopReason::error("game window not found — couldn't place the cursor on the spring")
}

/// The closed-loop placement failed; `detail` is the placement's own report.
pub fn spring_failed(detail: &str) -> StopReason {
    StopReason::error(format!(
        "couldn't place the cursor on the spring — {detail}"
    ))
}

// The per-grade tally is shared with the gem composer; re-exported here so callers that know it
// from the tuner can keep importing it from this module.
pub use crate::tools::grade_cycles::{GradeCycleRow, GradeCycles, describe};

/// v2 `BuildOcrGeometry`: the three dragged boxes become the tuner's OCR region and its bands.
///
/// The region is the bounding box of the three boxes, so the window can move/resize and the boxes
/// only have to surround the text; every sub-band is stored REGION-RELATIVE, which is the space
/// `seal_ocr::modes` reads. `row_height` is the measured line pitch when the caller has one
/// (`Check OCR`), else v2's fallback `attr.height / 3` — exact only when the attr box hugs its
/// three lines. The function is pure; the core owns validation, the shell owns drawing.
pub fn geometry_from_boxes(
    grade: Rect,
    attr: Rect,
    remaining: Rect,
    measured_row_height: Option<i32>,
) -> OcrGeometry {
    let left = grade.x.min(attr.x).min(remaining.x);
    let top = grade.y.min(attr.y).min(remaining.y);
    let right = grade.right().max(attr.right()).max(remaining.right());
    let bottom = grade.bottom().max(attr.bottom()).max(remaining.bottom());
    let row_height = measured_row_height
        .filter(|height| *height > 0)
        .unwrap_or_else(|| ((f64::from(attr.h)) / 3.0).round().max(1.0) as i32);
    OcrGeometry {
        region: Some(RegionConfig {
            left,
            top,
            width: right - left,
            height: bottom - top,
        }),
        grade_area: Some(BoxConfig {
            x1: grade.x - left,
            y1: grade.y - top,
            x2: grade.right() - left,
            y2: grade.bottom() - top,
        }),
        grade_y: vec![grade.y - top, grade.bottom() - top],
        attr_y: vec![attr.y - top, attr.bottom() - top],
        remaining_y: vec![remaining.y - top, remaining.bottom() - top],
        attr_x: vec![attr.x - left, attr.right() - left],
        remaining_x: vec![remaining.x - left, remaining.right() - left],
        row_height,
    }
}

#[cfg(test)]
mod geometry_tests {
    use super::*;
    use crate::geometry::Rect;

    #[test]
    fn the_region_is_the_bounding_box_and_the_bands_are_region_relative() {
        let geometry = geometry_from_boxes(
            Rect::new(120, 100, 40, 20),
            Rect::new(100, 130, 200, 60),
            Rect::new(110, 200, 150, 20),
            None,
        );
        assert_eq!(
            geometry.region,
            Some(RegionConfig {
                left: 100,
                top: 100,
                width: 200,
                height: 120,
            })
        );
        assert_eq!(
            geometry.grade_area,
            Some(BoxConfig {
                x1: 20,
                y1: 0,
                x2: 60,
                y2: 20
            })
        );
        assert_eq!(geometry.grade_y, vec![0, 20]);
        assert_eq!(geometry.attr_y, vec![30, 90]);
        assert_eq!(geometry.remaining_y, vec![100, 120]);
        assert_eq!(geometry.attr_x, vec![0, 200]);
        assert_eq!(geometry.remaining_x, vec![10, 160]);
        assert_eq!(geometry.row_height, 20, "attr height / 3");
    }

    #[test]
    fn a_measured_pitch_overrides_the_attr_height_guess() {
        let geometry = geometry_from_boxes(
            Rect::new(0, 0, 20, 20),
            Rect::new(0, 30, 100, 90),
            Rect::new(0, 130, 80, 20),
            Some(19),
        );
        assert_eq!(geometry.row_height, 19);
    }
}

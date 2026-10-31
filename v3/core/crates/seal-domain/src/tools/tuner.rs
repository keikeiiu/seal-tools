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

use crate::config::{
    BlueMaskConfig, BoxConfig, FilterConfig, GradeColorsConfig, GradeDecisionConfig, OcrGeometry,
    PurpleMaskConfig, RedMaskConfig, RegionConfig, WhiteMaskConfig, YellowMaskConfig,
};
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

// ── the grade COLOUR fallback (G25) ────────────────────────────────────────────────────────────

/// The five colour buckets v2 counted in the grade box. Only the letter in the box is coloured, so
/// the dominant colour above its absolute floor IS the grade; a white "N" has no coloured blob and
/// falls through to the white branch. Ported from `OcrEngine.DetectGradeColorScores`/`DecideGrade`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GradeColorScores {
    pub n: i32,
    pub g: i32,
    pub dg: i32,
    pub xg: i32,
    pub sg: i32,
}

/// v2's shipped `tuner.grade_colors` defaults, used when a field is absent (a hand-edited file must
/// degrade to no colour score, never a divide/range panic).
mod grade_defaults {
    pub const YELLOW_RG_GAP: i32 = 20;
    pub const YELLOW_GB_GAP: i32 = 20;
    pub const YELLOW_R_MIN: i32 = 120;
    pub const YELLOW_G_MIN: i32 = 120;
    pub const BLUE_BG_GAP: i32 = 25;
    pub const BLUE_BR_GAP: i32 = 25;
    pub const BLUE_B_MIN: i32 = 80;
    pub const RED_RG_GAP: i32 = 40;
    pub const RED_RB_GAP: i32 = 40;
    pub const RED_R_MIN: i32 = 150;
    pub const PURPLE_RB_GAP: i32 = 30;
    pub const PURPLE_BG_GAP: i32 = 30;
    pub const PURPLE_R_MIN: i32 = 80;
    pub const PURPLE_B_MIN: i32 = 80;
    pub const WHITE_MIN: i32 = 180;
    pub const TOTAL_MIN: i32 = 10;
    pub const DG_YELLOW: i32 = 80;
    pub const G_BLUE: i32 = 60;
    pub const XG_RED: i32 = 60;
    pub const SG_PURPLE: i32 = 60;
    pub const N_WHITE: i32 = 100;
    pub const N_YELLOW_MAX: i32 = 60;
    pub const N_BLUE_MAX: i32 = 60;
}

/// Count the five colour buckets over `rgba` (the grade box crop only), same masks and default
/// thresholds as v2. Pixels are BGR-agnostic here because the input is RGBA; v2's OpenCV Mat was
/// BGR and its `Item0/1/2` were B,G,R — the caller passes RGB order, and the masks are symmetric
/// in the only place that matters (they compare channels by name).
pub fn grade_color_scores(
    rgba: &[u8],
    width: usize,
    height: usize,
    colors: &GradeColorsConfig,
) -> GradeColorScores {
    use grade_defaults as d;
    let yellow = colors.yellow.unwrap_or(YellowMaskConfig {
        rg_gap: d::YELLOW_RG_GAP,
        gb_gap: d::YELLOW_GB_GAP,
        r_min: d::YELLOW_R_MIN,
        g_min: d::YELLOW_G_MIN,
    });
    let blue = colors.blue.unwrap_or(BlueMaskConfig {
        bg_gap: d::BLUE_BG_GAP,
        br_gap: d::BLUE_BR_GAP,
        b_min: d::BLUE_B_MIN,
    });
    let red = colors.red.unwrap_or(RedMaskConfig {
        rg_gap: d::RED_RG_GAP,
        rb_gap: d::RED_RB_GAP,
        r_min: d::RED_R_MIN,
    });
    let purple = colors.purple.unwrap_or(PurpleMaskConfig {
        rb_gap: d::PURPLE_RB_GAP,
        bg_gap: d::PURPLE_BG_GAP,
        r_min: d::PURPLE_R_MIN,
        b_min: d::PURPLE_B_MIN,
    });
    let white = colors.white.unwrap_or(WhiteMaskConfig {
        r_min: d::WHITE_MIN,
        g_min: d::WHITE_MIN,
        b_min: d::WHITE_MIN,
    });
    let mut scores = GradeColorScores::default();
    let pixels = width.saturating_mul(height).min(rgba.len() / 4);
    for pixel in rgba[..pixels * 4].as_chunks::<4>().0 {
        let (r, g, b) = (
            i32::from(pixel[0]),
            i32::from(pixel[1]),
            i32::from(pixel[2]),
        );
        let m_yellow =
            r > g + yellow.rg_gap && g > b + yellow.gb_gap && r > yellow.r_min && g > yellow.g_min;
        let m_blue = b > g + blue.bg_gap && b > r + blue.br_gap && b > blue.b_min;
        let m_red = r > g + red.rg_gap && r > b + red.rb_gap && r > red.r_min;
        let m_purple =
            r > b + purple.rb_gap && b > g + purple.bg_gap && r > purple.r_min && b > purple.b_min;
        let m_white = r > white.r_min && g > white.g_min && b > white.b_min;
        let any_colour = m_yellow || m_blue || m_red || m_purple;
        if m_white && !any_colour {
            scores.n += 1;
        }
        if m_yellow {
            scores.dg += 1;
        }
        if m_blue {
            scores.g += 1;
        }
        if m_red {
            scores.xg += 1;
        }
        if m_purple {
            scores.sg += 1;
        }
    }
    scores
}

/// v2's `DecideGrade`: the dominant colour above its absolute floor wins; white falls through to
/// "N" only when no colour blob wins. `None` when the box is too empty to decide (v2's `total_min`).
pub fn decide_grade_colour(
    scores: &GradeColorScores,
    colors: &GradeColorsConfig,
) -> Option<String> {
    use grade_defaults as d;
    let decision = colors.decision.unwrap_or(GradeDecisionConfig {
        total_min: d::TOTAL_MIN,
        dg_yellow: d::DG_YELLOW,
        g_blue: d::G_BLUE,
        xg_red: d::XG_RED,
        sg_purple: d::SG_PURPLE,
        n_white: d::N_WHITE,
        n_yellow_max: d::N_YELLOW_MAX,
        n_blue_max: d::N_BLUE_MAX,
    });
    let total = scores.n + scores.g + scores.dg + scores.xg + scores.sg;
    if total < decision.total_min {
        return None;
    }
    let blue_win = scores.g > decision.g_blue
        && scores.g >= scores.dg
        && scores.g >= scores.xg
        && scores.g >= scores.sg;
    let yellow_win = scores.dg > decision.dg_yellow
        && scores.dg >= scores.g
        && scores.dg >= scores.xg
        && scores.dg >= scores.sg;
    let red_win = scores.xg > decision.xg_red
        && scores.xg >= scores.g
        && scores.xg >= scores.dg
        && scores.xg >= scores.sg;
    let purple_win = scores.sg > decision.sg_purple
        && scores.sg >= scores.g
        && scores.sg >= scores.dg
        && scores.sg >= scores.xg;
    if blue_win {
        return Some("G".to_owned());
    }
    if yellow_win {
        return Some("DG".to_owned());
    }
    if red_win {
        return Some("XG".to_owned());
    }
    if purple_win {
        return Some("SG".to_owned());
    }
    if scores.n > decision.n_white
        && scores.dg < decision.n_yellow_max
        && scores.g < decision.n_blue_max
    {
        return Some("N".to_owned());
    }
    None
}

#[cfg(test)]
mod grade_colour_tests {
    use super::*;

    fn colors() -> GradeColorsConfig {
        GradeColorsConfig::default()
    }

    #[test]
    fn a_blue_box_reads_g_and_a_white_box_reads_n() {
        // A 100-pixel solid blue crop: B=220, R=G=10. The blue mask alone must win.
        let blue = [10u8, 10u8, 220u8, 255u8].repeat(100);
        let scores = grade_color_scores(&blue, 10, 10, &colors());
        assert_eq!(scores.g, 100);
        assert_eq!(
            decide_grade_colour(&scores, &colors()).as_deref(),
            Some("G")
        );

        let white = [230u8, 230u8, 230u8, 255u8].repeat(1000);
        let scores = grade_color_scores(&white, 20, 50, &colors());
        assert_eq!(scores.n, 1000);
        assert_eq!(
            decide_grade_colour(&scores, &colors()).as_deref(),
            Some("N")
        );
    }

    #[test]
    fn a_nearly_black_or_tiny_box_has_no_verdict() {
        let dark = [5u8, 5u8, 5u8, 255u8].repeat(100);
        let scores = grade_color_scores(&dark, 10, 10, &colors());
        assert_eq!(scores, GradeColorScores::default());
        assert_eq!(decide_grade_colour(&scores, &colors()), None);
    }
}

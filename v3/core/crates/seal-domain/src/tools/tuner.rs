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

use crate::config::FilterConfig;
use crate::decision::StopReason;
use crate::geometry::Point;
use crate::ocr::{AttrMatcher, MatchedAttr};
use std::collections::BTreeMap;

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
            "spring_mode is hid but tuner.spring_point isn't calibrated — click the 發條 button in \
             Calibrate Tuner",
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

// ── how many cycles were spent on each grade (GradeCycles) ───────────────────────────────────

/// One grade's line on the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradeCycleRow {
    pub name: String,
    pub status: String,
}

/// How many compose cycles a run has spent on each grade.
///
/// The composer's single Cycle counter says how much work a run has done and not where it went, and
/// the interesting number is per grade: a run that took 40 cycles on N and 3 on DG is telling you
/// something a total of 43 cannot. The player asked for it in as many words — *"N how many cycle, G
/// how many cycle, DG how many cycle"*.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GradeCycles {
    counts: BTreeMap<String, i32>,
}

impl GradeCycles {
    pub fn new() -> Self {
        Self::default()
    }

    /// One more cycle spent on `grade`.
    pub fn add(&mut self, grade: &str) {
        *self.counts.entry(grade.to_owned()).or_default() += 1;
    }

    /// Cycles spent on `grade` so far — 0 for one not reached yet.
    pub fn count_for(&self, grade: &str) -> i32 {
        self.counts.get(grade).copied().unwrap_or(0)
    }

    /// A new run: every count back to zero.
    pub fn reset(&mut self) {
        self.counts.clear();
    }

    /// One row per grade, in the order the composer runs them. Every grade appears including the ones
    /// not reached: "DG: 0 cycles" is information, and a list that grows as the run proceeds moves
    /// under the reader's eye.
    pub fn rows(&self, grades: &[String]) -> Vec<GradeCycleRow> {
        grades
            .iter()
            .map(|grade| GradeCycleRow {
                name: grade.clone(),
                status: describe(self.count_for(grade)),
            })
            .collect()
    }

    /// The whole tally on one line, for the log — `N 12 · G 8 · DG 3 · 23 total`. The total is the sum
    /// of the grades rather than a separate counter, so it cannot disagree with them.
    pub fn summary(&self, grades: &[String]) -> String {
        let mut parts: Vec<String> = grades
            .iter()
            .map(|grade| format!("{grade} {}", self.count_for(grade)))
            .collect();
        parts.push(format!("{} total", self.total(grades)));
        parts.join(" · ")
    }

    /// Cycles this tally has counted, across the listed grades.
    pub fn total(&self, grades: &[String]) -> i32 {
        grades.iter().map(|grade| self.count_for(grade)).sum()
    }
}

/// `1 cycle` / `4 cycles` — one wording, so the card and the log cannot disagree, and "1 cycles"
/// never reaches a reader.
pub fn describe(cycles: i32) -> String {
    if cycles == 1 {
        "1 cycle".to_owned()
    } else {
        format!("{cycles} cycles")
    }
}

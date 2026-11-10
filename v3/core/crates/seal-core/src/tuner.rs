//! The tuner: the first READ-LOOP tool (capture -> OCR -> decide -> effects).
//!
//! The other two live tools need no input from the screen. The tuner is the first that does, and that
//! makes the shape of every later reader tool:
//!
//! - **The read is a seam.** [`TunerSource`] is one scan plus a cursor read; the production source is
//!   `win::capture` + `seal-ocr`'s pipeline, and the tests script it. The DECISION half is
//!   `seal-domain`'s [`TunerRun`] and is not repeated here: this module only turns one
//!   `TunerOutcome` into the next few `Decision`s and holds the loop's state.
//! - **The command stream is v2's exactly** (`SealTuner.cs:104-160`): countdown, then one
//!   attempt = (HID spring placement once) `C` click, wait `click_enter_delay`, `E` Enter, wait
//!   `ocr_delay`, one read, decide. `Retry` loops; a `Stop` reason ends the run.
//! - **The mouse guard runs before each click and only in `hid` mode.** Both `None`s fail OPEN
//!   (window or cursor unreadable), which is v2's acknowledged behaviour and is why the guard is a
//!   domain function with that comment rather than a rule invented here.
//! - **Every read is logged, and a partial read is never decided on.** The production source writes
//!   one `logs/ocr_log.jsonl` record per completed capture+OCR pass (v2's per-`ScanOnce` record),
//!   saves that frame under `logs/captures/` when `tuner.save_captures` is on or on the last retry
//!   (v2's `forceCapture`), and records a read that is still partial after `tuner.ocr_retries` in
//!   `logs/ocr_errors.jsonl`. The decision half stops on that partial read instead of counting it as
//!   a repeat (issue #16's false "same result x3").
//!
//! One recorded simplification: a FAILED recentre placement surfaces as the effects' placement line,
//! not v2's composed "cursor wouldn't re-centre — `<detail>`". The stop still happens and still names
//! the cursor; only the wording differs, because the runner performs effects and the tool cannot see
//! their failure. Recorded in `OPEN.md`.

use std::collections::VecDeque;
use std::time::Instant;

use base64::Engine as _;
use seal_domain::config::FilterConfig;
use seal_domain::decision::{Decision, StopKind, StopReason};
use seal_domain::geometry::{DisplayInfo, Point, Rect};
use seal_domain::tools::tuner::{
    MouseGuardStep, START_COUNTDOWN, SpringMode, SpringStep, TunerDecision, TunerRead, TunerRun,
    TunerSettings, after_recenter, mouse_guard_step, spring_step,
};
use seal_tools::runner::{Tool, ToolStatus};
use seal_tools::tool::{ToolDetail, ToolId};
use serde_json::{Value, json};

/// v2's loop tick (`SealTuner.cs` `SleepCheck(0.05)`), used while nothing is due.
pub const TICK_SECONDS: f64 = 0.05;

/// The tuner's resolved start data: the decision settings, the spring point, and the OCR region.
#[derive(Debug, Clone, PartialEq)]
pub struct TunerPlan {
    pub settings: TunerSettings,
    pub spring: Option<Point>,
    pub region: Rect,
}

fn number(value: &Value, path: &str) -> Option<i32> {
    value
        .pointer(path)
        .and_then(Value::as_i64)
        .and_then(|number| i32::try_from(number).ok())
}

fn text(value: &Value, path: &str) -> Option<String> {
    value
        .pointer(path)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn point(value: &Value, path: &str) -> Option<Point> {
    let array = value.pointer(path)?.as_array()?;
    if array.len() != 2 {
        return None;
    }
    Some(Point::new(
        i32::try_from(array[0].as_i64()?).ok()?,
        i32::try_from(array[1].as_i64()?).ok()?,
    ))
}

/// The tuner's keep/reroll filter, read from the REAL merged config's `tuner.filter` — the path the
/// config registry owns (`seal-config/src/registry.rs`) and the Tuner tab edits. A filter that is
/// absent is the disabled default (the merged config always carries the shipped block, so absent
/// means a caller passed a partial fixture), but a present filter that cannot be deserialised is
/// REFUSED instead of silently becoming "filter off" (TOOL-F1).
///
/// `tuner_session`'s read side resolves the same block for its Test read; this function is
/// `pub(crate)` so that copy can adopt it without another filter-shape change.
///
/// # Errors
///
/// Returns the player-facing line naming the block when `tuner.filter` cannot be read as a
/// [`FilterConfig`].
pub(crate) fn filter_config(config: &Value) -> Result<FilterConfig, String> {
    let Some(block) = config.pointer("/tuner/filter") else {
        return Ok(FilterConfig::default());
    };
    serde_json::from_value(block.clone())
        .map_err(|error| format!("tuner.filter isn't usable — {error}. Fix it in the Tuner tab."))
}

/// Resolve the tuner plan out of the merged config, or the line that says which part is missing.
///
/// # Errors
///
/// Returns the line naming the first missing part of the tuner's calibration, or the line refusing a
/// malformed `tuner.filter`.
pub fn plan(config: &Value) -> Result<TunerPlan, String> {
    let region = Rect::new(
        number(config, "/tuner/ocr/region/left").ok_or_else(|| {
            "tuner.ocr.region isn't calibrated — draw it in Calibrate.".to_owned()
        })?,
        number(config, "/tuner/ocr/region/top").ok_or_else(|| {
            "tuner.ocr.region isn't calibrated — draw it in Calibrate.".to_owned()
        })?,
        number(config, "/tuner/ocr/region/width").ok_or_else(|| {
            "tuner.ocr.region isn't calibrated — draw it in Calibrate.".to_owned()
        })?,
        number(config, "/tuner/ocr/region/height").ok_or_else(|| {
            "tuner.ocr.region isn't calibrated — draw it in Calibrate.".to_owned()
        })?,
    );
    if region.w <= 0 || region.h <= 0 {
        return Err("tuner.ocr.region has no area — draw it again in Calibrate.".to_owned());
    }

    let grade_order: Vec<String> = config
        .pointer("/tuner/grade_order")
        .and_then(Value::as_array)
        .map(|grades| {
            grades
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let filter = filter_config(config)?;

    let settings = TunerSettings {
        grade_order,
        target_grade: text(config, "/tuner/target_grade").unwrap_or_default(),
        max_retries: number(config, "/tuner/max_retries").unwrap_or(999_999),
        spring_mode: SpringMode::parse(&text(config, "/tuner/spring_mode").unwrap_or_default()),
        mouse_guard: seal_domain::tools::tuner::MouseGuard::parse(
            &text(config, "/tuner/mouse_guard").unwrap_or_default(),
        ),
        guard_px: number(config, "/tuner/guard_px").unwrap_or(8),
        recenter_max: number(config, "/tuner/recenter_max").unwrap_or(20),
        filter,
    };
    let spring = point(config, "/tuner/spring_point");

    // Spring mode `hid` with no point would stop on the first attempt anyway; refusing at start puts
    // the fix before the first click and reuses the domain's own line.
    if settings.spring_mode == SpringMode::Hid
        && let SpringStep::Stop(reason) = spring_step(&settings, spring)
    {
        return Err(reason.message);
    }

    Ok(TunerPlan {
        settings,
        spring,
        region,
    })
}

/// One scan plus a cursor read: the loop's input seam.
pub trait TunerSource {
    /// One OCR read of the calibrated region, or the line that says why it could not be read.
    ///
    /// # Errors
    ///
    /// Returns the line explaining why the calibrated region could not be read.
    fn read(&mut self) -> Result<TunerRead, String>;
    /// The cursor in the LOGICAL screen space, or `None` when unreadable (the guard fails open).
    fn cursor(&mut self) -> Option<Point> {
        None
    }

    /// How many OCR captures this source has saved under `logs/captures/` for the run's summary.
    /// The scripted test source saves none.
    fn captures_saved(&self) -> usize {
        0
    }
}

/// Where the run is in v2's command stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Countdown,
    PlaceSpring,
    Guard,
    Click,
    WaitAfterClick,
    Enter,
    WaitAfterOcr,
    Read,
}

/// Where the run's monotonic time comes from; tests step it.
pub trait Now {
    fn seconds(&self) -> f64;
}

/// The production clock.
pub struct RealNow(Instant);

impl RealNow {
    #[must_use]
    pub fn start() -> Self {
        Self(Instant::now())
    }
}

impl Now for RealNow {
    fn seconds(&self) -> f64 {
        self.0.elapsed().as_secs_f64()
    }
}

/// The tuner as the runner sees it.
pub struct TunerTool<S: TunerSource> {
    run: TunerRun,
    plan: TunerPlan,
    source: S,
    display: DisplayInfo,
    now: Box<dyn Now + Send>,
    step: Step,
    queue: VecDeque<Decision>,
    started: f64,
    placed_spring: bool,
    recenter_count: i32,
    pending_click_delay: f64,
    pending_ocr_delay: f64,
    /// The filter line from the last read (`filter.disabled` when off). v2 shows it as the card's
    /// `Filter:` fact, so it is the part of the run the player reads between attempts.
    filter_line: String,
    /// The last OCR read's facts (grade/remaining/attributes), kept for the card's `detail` lines.
    /// `TunerRun::decide` consumes a read, so this is the only place the card can still see it.
    last_read: Option<TunerRead>,
}

impl<S: TunerSource> TunerTool<S> {
    pub fn new(plan: TunerPlan, source: S, display: DisplayInfo, now: Box<dyn Now + Send>) -> Self {
        let started = now.seconds();
        let timings = (0.4, 0.8); // v2 defaults; overwritten by `with_timings`
        Self {
            run: TunerRun::new(plan.settings.clone()),
            plan,
            source,
            display,
            now,
            step: Step::Countdown,
            queue: VecDeque::new(),
            started,
            placed_spring: false,
            recenter_count: 0,
            pending_click_delay: timings.0,
            pending_ocr_delay: timings.1,
            filter_line: String::new(),
            last_read: None,
        }
    }

    /// The two delays between C, E and the read (`tuner.timing.click_enter_delay`/`ocr_delay`).
    #[must_use]
    pub fn with_timings(mut self, click_enter_delay: f64, ocr_delay: f64) -> Self {
        self.pending_click_delay = click_enter_delay;
        self.pending_ocr_delay = ocr_delay;
        self
    }

    /// The card's `detail` lines in v2's `FormatStatus` order: Grade, Remaining, Attempt, the
    /// label-less `name=value` attribute lines, then Filter (v2's wording; the shell adds nothing).
    fn card_detail(&self) -> Vec<ToolDetail> {
        let mut detail = Vec::new();
        if let Some(read) = &self.last_read {
            if let Some(grade) = &read.grade {
                detail.push(ToolDetail::labeled("Grade", grade.clone()));
            }
            if let Some(remaining) = read.remaining {
                detail.push(ToolDetail::labeled("Remaining", remaining.to_string()));
            }
        }
        if self.run.attempt() > 0 {
            // v2 numbers the first attempt 1; `TunerRun` counts attempts STARTED, and the countdown
            // now begins the first one, so this is v2's number rather than a display clamp
            // (TOOL-F6 removed the `.max(1)` that used to hide the gap).
            detail.push(ToolDetail::labeled(
                "Attempt",
                self.run.attempt().to_string(),
            ));
        }
        if let Some(read) = &self.last_read {
            for attribute in &read.attributes {
                detail.push(ToolDetail::plain(match attribute.value {
                    Some(value) => format!("{}={}", attribute.name, value),
                    None => attribute.name.clone(),
                }));
            }
        }
        if !self.filter_line.is_empty() {
            detail.push(ToolDetail::labeled("Filter", self.filter_line.clone()));
        }
        detail
    }

    fn logical_spring(&self) -> Option<Point> {
        self.plan
            .spring
            .map(|spring| self.display.cursor_target(spring.x, spring.y).logical)
    }

    /// One attempt's beginning: hid mode places the cursor once, then every attempt clicks.
    fn begin_attempt(&mut self) {
        if self.plan.settings.spring_mode == SpringMode::Hid && !self.placed_spring {
            self.step = Step::PlaceSpring;
        } else {
            self.step = Step::Guard;
        }
    }
}

impl<S: TunerSource + Send> Tool for TunerTool<S> {
    fn id(&self) -> ToolId {
        ToolId::Tuner
    }

    fn status(&self) -> ToolStatus {
        if self.step == Step::Countdown {
            let remaining =
                (f64::from(START_COUNTDOWN) - (self.now.seconds() - self.started)).max(0.0);
            return ToolStatus {
                message: format!("Starting in {:.0}...", remaining.ceil()),
                countdown_s: Some(remaining),
                rows: Vec::new(),
                detail: Vec::new(),
            };
        }
        // The facts v2 printed below its card: Grade, Remaining, Attempt, one line per matched
        // attribute, then Filter. `message` keeps only the last EVENT (v2's warning line), so a fact
        // is never said twice (PLAN-V3-STATUS-DETAIL §5 ⚑2); no warning path exists yet, so it is
        // empty between reads.
        ToolStatus {
            message: String::new(),
            countdown_s: None,
            rows: Vec::new(),
            detail: self.card_detail(),
        }
    }

    /// `tool.finished.summary` (G11): the last read's grade, the attempt count, the OCR captures this
    /// run saved (the forced frames left for post-mortem study) and which protocol stop reason ended
    /// it (v2's "stopped by", in the wire's own word).
    fn summary(&self, reason: &StopReason) -> Option<Value> {
        let stopped_by = match reason.kind {
            StopKind::Done => "done",
            StopKind::Cancelled => "cancelled",
            StopKind::Error => "error",
            StopKind::Guard => "guard",
        };
        Some(json!({
            "grade": self.last_read.as_ref().and_then(|read| read.grade.clone()),
            "attempts": self.run.attempt(),
            "captures_saved": self.source.captures_saved(),
            "stopped_by": stopped_by,
        }))
    }

    fn decide(&mut self) -> Decision {
        if let Some(decision) = self.queue.pop_front() {
            return decision;
        }

        loop {
            match self.step {
                Step::Countdown => {
                    let elapsed = self.now.seconds() - self.started;
                    if elapsed < f64::from(START_COUNTDOWN) {
                        return Decision::Wait(1.0);
                    }
                    // v2 counts the attempt BEFORE the click (`SealTuner.cs:121` `attempt++`, then
                    // `:253`), so the countdown starting the first attempt is what makes
                    // `max_retries` an exact spring ceiling instead of one over (TOOL-F6). The retry
                    // path below begins each following attempt the same way.
                    self.run.begin_attempt();
                    self.begin_attempt();
                }
                Step::PlaceSpring => {
                    self.placed_spring = true;
                    self.step = Step::Guard;
                    match self.plan.spring {
                        Some(spring) => return Decision::Place(spring),
                        None => {
                            self.step = Step::Click;
                        }
                    }
                }
                Step::Guard => {
                    let cursor = self.source.cursor();
                    match mouse_guard_step(
                        &self.plan.settings,
                        self.logical_spring(),
                        cursor,
                        self.recenter_count,
                    ) {
                        MouseGuardStep::Proceed { drift_free } => {
                            if drift_free {
                                self.recenter_count = 0;
                            }
                            self.step = Step::Click;
                        }
                        MouseGuardStep::Recenter => {
                            self.recenter_count += 1;
                            let Some(spring) = self.plan.spring else {
                                self.step = Step::Click;
                                continue;
                            };
                            if let MouseGuardStep::Stop(reason) =
                                after_recenter(&self.plan.settings, Ok(()), self.recenter_count)
                            {
                                self.step = Step::Read;
                                return Decision::Stop(reason);
                            }
                            self.step = Step::Click;
                            return Decision::Place(spring);
                        }
                        MouseGuardStep::Stop(reason) => {
                            self.step = Step::Read;
                            return Decision::Stop(reason);
                        }
                    }
                }
                Step::Click => {
                    self.step = Step::WaitAfterClick;
                    return Decision::ClickHere;
                }
                Step::WaitAfterClick => {
                    self.step = Step::Enter;
                    return Decision::Wait(self.pending_click_delay);
                }
                Step::Enter => {
                    self.step = Step::WaitAfterOcr;
                    return Decision::Enter;
                }
                Step::WaitAfterOcr => {
                    self.step = Step::Read;
                    return Decision::Wait(self.pending_ocr_delay);
                }
                Step::Read => {
                    let read = match self.source.read() {
                        Ok(read) => read,
                        Err(line) => {
                            self.step = Step::Read;
                            return Decision::Stop(seal_domain::decision::StopReason::error(line));
                        }
                    };
                    // The decision half consumes the read; keep the facts for the card's detail.
                    self.last_read = Some(read.clone());
                    let outcome = self.run.decide(&read);
                    self.filter_line.clone_from(&outcome.filter_status);
                    match outcome.decision {
                        TunerDecision::Stop(reason) => {
                            self.step = Step::Read;
                            return Decision::Stop(reason);
                        }
                        TunerDecision::Retry => {
                            self.run.begin_attempt();
                            self.begin_attempt();
                        }
                    }
                }
            }
        }
    }
}

/// The two delays between the click, the Enter and the read (`tuner.timing.*`), with v2's defaults.
#[must_use]
pub fn timings(config: &Value) -> (f64, f64) {
    let get = |path: &str| config.pointer(path).and_then(Value::as_f64);
    (
        get("/tuner/timing/click_enter_delay").unwrap_or(0.4),
        get("/tuner/timing/ocr_delay").unwrap_or(0.8),
    )
}

/// The production source: capture the calibrated region, OCR it, and read the cursor for the guard.
///
/// The opening phase ([`Self::open`]) does everything that can fail before the first click: find the
/// window, measure the display the guard compares in, and load the OCR models. The read itself is
/// `win::capture` (foreground-gated) + `seal-ocr`'s pipeline + `seal-domain`'s matcher, all of which
/// are already tested on their own; this is the wiring.
pub struct OcrTunerSource {
    title: String,
    region: Rect,
    geometry: seal_ocr::modes::OcrGeometry,
    attributes: seal_domain::config::AttributesConfig,
    pipeline: seal_ocr::pipeline::OcrPipeline,
    /// `tuner.ocr_retries` (v2 default 3): a read is re-captured until it is CONFIRMED (grade +
    /// every attribute row + remaining) or the budget runs out. The last attempt leaves a capture
    /// for post-mortem study (v2's `forceCapture`).
    retries: u32,
    grade_colors: seal_domain::config::GradeColorsConfig,
    /// `<root>/logs` writer for the read loop's diagnosis. The opening phase has only the root (the
    /// supervisor's shared logger is not handed to a tool source), so the source opens its own
    /// `Logs` over the same files; every write is one append, which is all the file contract needs.
    logs: crate::logs::Logs,
    /// `tuner.save_captures`: save every completed read's frame, not only the forced last one.
    save_captures: bool,
    /// Captures saved by this run, reported in `tool.finished.summary`.
    captures_saved: usize,
}

impl OcrTunerSource {
    /// Open the read side, or name what is missing before the run starts.
    ///
    /// # Errors
    ///
    /// Returns the startup refusal naming a missing model or calibration.
    pub fn open(
        config: &Value,
        attributes: seal_domain::config::AttributesConfig,
        region: Rect,
        root: &std::path::Path,
    ) -> Result<(Self, DisplayInfo), String> {
        let title = crate::win::configured_title(config).map_err(|error| error.line())?;
        let hwnd = seal_win::window::find_window(&title)
            .ok_or_else(|| format!("game window not found — is {title:?} running?"))?;
        let display = seal_win::window::measure(hwnd).map_err(|error| error.to_string())?;
        let geometry = crate::dispatch::ocr_geometry(config);
        let ocr_version = crate::dispatch::ocr_model_version(config)
            .map_err(|error| format!("the OCR model set is invalid: {error}"))?;
        let pipeline = seal_ocr::pipeline::OcrPipeline::load_for_root_version(
            root,
            &attributes,
            geometry.row_height,
            ocr_version,
        )
        .map_err(|error| format!("the OCR models are not available: {}", error.message))?;
        let retries = config
            .pointer("/tuner/ocr_retries")
            .and_then(Value::as_i64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(3)
            .clamp(1, 100);
        let grade_colors = config
            .pointer("/tuner/grade_colors")
            .cloned()
            .and_then(|block| serde_json::from_value(block).ok())
            .unwrap_or_default();
        let logs = crate::logs::Logs::new(root);
        let save_captures = config
            .pointer("/tuner/save_captures")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok((
            Self {
                title,
                region,
                geometry,
                attributes,
                pipeline,
                retries,
                grade_colors,
                logs,
                save_captures,
                captures_saved: 0,
            },
            display,
        ))
    }
}

/// One completed capture+OCR pass, plus the `ocr_log.jsonl` record already written for it (kept so
/// the last partial attempt can also become an `ocr_errors.jsonl` record).
struct CompletedRead {
    read: TunerRead,
    record: Value,
}

/// What one read's retry policy ends with (v2's `OcrEngine.Scan`).
enum ReadPolicy {
    /// The read is complete; the decision half may act on it.
    Confirmed(TunerRead),
    /// The retry budget is gone and the read is still partial — issue #16's bad input. The record is
    /// the last attempt's `ocr_log` line, for `ocr_errors.jsonl`.
    Unconfirmed { read: TunerRead, record: Value },
    /// The last attempt failed outright; its line is the stop.
    Failed(String),
}

/// The retry policy of one read: re-capture until the read is confirmed, and keep the last partial
/// attempt for the error log. `read_once(attempt, force_capture)` is one capture+OCR pass and reports
/// the 1-based attempt number plus whether this is the last one (v2's `forceCapture`). Split from
/// [`OcrTunerSource`] so the policy is testable without Win32 or the OCR models.
fn read_policy<F>(retries: u32, mut read_once: F) -> ReadPolicy
where
    F: FnMut(u32, bool) -> Result<CompletedRead, String>,
{
    for attempt in 1..=retries {
        let last = attempt == retries;
        match read_once(attempt, last) {
            Ok(completed) => {
                if completed.read.is_confirmed() {
                    return ReadPolicy::Confirmed(completed.read);
                }
                if last {
                    return ReadPolicy::Unconfirmed {
                        read: completed.read,
                        record: completed.record,
                    };
                }
            }
            Err(error) => {
                if last {
                    return ReadPolicy::Failed(error);
                }
            }
        }
    }
    ReadPolicy::Failed("the tuner OCR read produced no attempt".to_owned())
}

/// The `ocr_log.jsonl` record for one completed tuner read: the `ocr.scan` shape
/// (`timestamp`/`grade`/`remaining`/`attributes`/`lines`/`capture_path`) plus the facts a tuner
/// diagnosis needs — where the read came from (`source`), which re-capture produced it (`attempt`) and
/// whether it was complete (`confirmed`).
fn completed_read_record(
    read: &TunerRead,
    lines: &[String],
    attributes: Option<&[Vec<String>]>,
    attempt: u32,
    timestamp: &str,
    capture_path: Option<&str>,
) -> Value {
    json!({
        "timestamp": timestamp,
        "source": "tuner",
        "attempt": attempt,
        "confirmed": read.is_confirmed(),
        "grade": read.grade.clone(),
        "remaining": read.remaining,
        "attributes": attributes,
        "lines": lines,
        "capture_path": capture_path.map_or(Value::Null, |path| Value::String(path.to_owned())),
    })
}

impl OcrTunerSource {
    /// Saves this frame's own PNG bytes (the bytes the OCR saw — never a second capture) and returns
    /// its root-relative path. v2's `ScanOnce`/`forceCapture`.
    fn save_capture(&mut self, frame: &Value, timestamp: &str) -> Option<String> {
        let data = frame.get("data").and_then(Value::as_str)?;
        let png = base64::engine::general_purpose::STANDARD
            .decode(data)
            .ok()?;
        let path = self
            .logs
            .capture(&format!("capture_{timestamp}.png"), &png)?;
        self.captures_saved += 1;
        Some(path)
    }

    /// One capture + OCR pass, logged as one `ocr_log.jsonl` record (v2 wrote one per `ScanOnce`).
    /// `attempt` is the 1-based re-capture number and `force_capture` is the last attempt (v2's
    /// `forceCapture`): it saves the frame even when `tuner.save_captures` is off, so a persistently
    /// partial read still leaves evidence for the next investigation.
    #[allow(
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        reason = "frame pixel coordinates and counts are far inside the range these conversions can represent"
    )]
    fn read_once(&mut self, attempt: u32, force_capture: bool) -> Result<CompletedRead, String> {
        let (frame, _display) =
            crate::win::capture(&self.title, Some(self.region)).map_err(|error| error.line())?;
        let (width, height, rgba) = crate::dispatch::frame_rgba(&frame)?;
        let scan = self
            .pipeline
            .scan_rgba(width, height, &rgba)
            .map_err(|error| error.message)?;
        let geometry = &self.geometry;
        let grade_image = geometry.grade_area.and_then(|area| {
            let crop_width = area.x2.saturating_sub(area.x1).max(1) as u32;
            let crop_height = area.y2.saturating_sub(area.y1).max(1) as u32;
            let rect = Rect::new(area.x1, area.y1, crop_width as i32, crop_height as i32);
            seal_win::pure::crop_rgba(width as u32, height as u32, &rgba, rect)
                .ok()
                .map(|(w, h, pixels)| (w as usize, h as usize, pixels))
        });
        let grade_mode = seal_ocr::modes::apply_with_image(
            seal_ocr::modes::Mode::Grade,
            &scan,
            &self.attributes,
            geometry,
            grade_image
                .as_ref()
                .map(|(w, h, pixels)| seal_ocr::modes::GradeImage {
                    rgba: pixels,
                    width: *w,
                    height: *h,
                    colors: &self.grade_colors,
                }),
        );
        let attributes_mode = seal_ocr::modes::apply(
            seal_ocr::modes::Mode::Attributes,
            &scan,
            &self.attributes,
            geometry,
        );
        let matcher = seal_domain::ocr::AttrMatcher::new(&self.attributes);
        let attributes =
            matcher.match_attributes(attributes_mode.attributes.as_deref().unwrap_or_default());
        let read = TunerRead {
            grade: grade_mode.grade,
            remaining: grade_mode
                .remaining
                .and_then(|count| i32::try_from(count).ok()),
            attributes,
        };
        let timestamp = crate::logs::scan_stamp();
        let capture_path = if self.save_captures || force_capture {
            self.save_capture(&frame, &timestamp)
        } else {
            None
        };
        let lines: Vec<String> = scan.lines.iter().map(|line| line.text.clone()).collect();
        let record = completed_read_record(
            &read,
            &lines,
            attributes_mode.attributes.as_deref(),
            attempt,
            &timestamp,
            capture_path.as_deref(),
        );
        self.logs.ocr_line(&record);
        Ok(CompletedRead { read, record })
    }
}

impl TunerSource for OcrTunerSource {
    fn read(&mut self) -> Result<TunerRead, String> {
        let retries = self.retries;
        match read_policy(retries, |attempt, force_capture| {
            self.read_once(attempt, force_capture)
        }) {
            ReadPolicy::Confirmed(read) => Ok(read),
            ReadPolicy::Unconfirmed { read, record } => {
                // v2's `LogOcrError`: the partial read is diagnosis evidence, and the decision half
                // refuses it (the fix for issue #16's false STUCK).
                self.logs.ocr_error(&record);
                Ok(read)
            }
            ReadPolicy::Failed(error) => Err(error),
        }
    }

    fn cursor(&mut self) -> Option<Point> {
        seal_win::window::cursor_position()
    }

    fn captures_saved(&self) -> usize {
        self.captures_saved
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use seal_domain::decision::StopKind;
    use seal_domain::geometry::Size;

    use super::*;

    /// A clock in milliseconds, shared with the test so it can advance time.
    struct StepNow(Arc<AtomicU64>);

    impl StepNow {
        fn new() -> (Self, Arc<AtomicU64>) {
            let clock = Arc::new(AtomicU64::new(0));
            (Self(clock.clone()), clock)
        }
    }

    impl Now for StepNow {
        #[allow(
            clippy::cast_precision_loss,
            reason = "both sides are frame pixel counts, far below 2^53, so this f64 conversion is exact"
        )]
        fn seconds(&self) -> f64 {
            self.0.load(Ordering::SeqCst) as f64 / 1000.0
        }
    }

    /// Advance the shared clock by one second.
    fn tick(clock: &Arc<AtomicU64>) {
        clock.fetch_add(1000, Ordering::SeqCst);
    }

    /// A scripted source: reads are taken in order, the cursor is fixed.
    struct FakeSource {
        reads: Vec<Result<TunerRead, String>>,
        cursor: Option<Point>,
    }

    impl TunerSource for FakeSource {
        fn read(&mut self) -> Result<TunerRead, String> {
            if self.reads.is_empty() {
                return Err("no more reads".to_owned());
            }
            self.reads.remove(0)
        }

        fn cursor(&mut self) -> Option<Point> {
            self.cursor
        }
    }

    fn display() -> DisplayInfo {
        DisplayInfo::from_rects(
            Rect::new(0, 0, 900, 700),
            Rect::new(0, 0, 900, 700),
            Rect::new(0, 0, 900, 700),
            Size::new(1920, 1080),
        )
    }

    fn config(spring_mode: &str, spring: Option<[i32; 2]>) -> Value {
        serde_json::json!({
            "tuner": {
                "grade_order": ["N", "G", "DG", "XG", "SG"],
                "target_grade": "DG",
                "max_retries": 999_999,
                "spring_mode": spring_mode,
                "mouse_guard": "off",
                "guard_px": 8,
                "recenter_max": 20,
                "ocr": {"region": {"left": 100, "top": 200, "width": 300, "height": 320}},
                "spring_point": spring,
                "timing": {"click_enter_delay": 0.4, "ocr_delay": 0.8},
                // The REAL merged shape: the config registry owns `tuner.filter` and the Tuner tab
                // edits it inside the tuner block. A top-level `filter` in a fixture is exactly what
                // hid TOOL-F1, so the fixture no longer has one.
                "filter": {"enabled": false},
            },
        })
    }

    fn attr(name: &str, value: i32) -> seal_domain::ocr::MatchedAttr {
        seal_domain::ocr::MatchedAttr {
            name: name.to_owned(),
            category: String::new(),
            value: Some(value),
            extra: None,
            raw: Vec::new(),
        }
    }

    /// A COMPLETE read: grade, spring count and the three named attribute rows v2's `IsConfirmed`
    /// requires. The decision half refuses PARTIAL reads, so every run fixture is confirmed.
    fn confirmed_read(grade: &str, remaining: i32) -> TunerRead {
        TunerRead {
            grade: Some(grade.to_owned()),
            remaining: Some(remaining),
            attributes: vec![attr("STR", 1), attr("DEX", 1), attr("INT", 1)],
        }
    }

    /// A PARTIAL read: one attribute row was lost — issue #16's bad input.
    fn partial_read() -> TunerRead {
        TunerRead {
            grade: Some("G".to_owned()),
            remaining: Some(9),
            attributes: vec![attr("STR", 1), attr("DEX", 1)],
        }
    }

    fn read_target() -> TunerRead {
        confirmed_read("DG", 10)
    }

    fn read_below_target() -> TunerRead {
        confirmed_read("G", 10)
    }

    fn plan_of(config: &Value) -> TunerPlan {
        plan(config).expect("resolves")
    }

    fn tool(
        config: &Value,
        reads: Vec<Result<TunerRead, String>>,
        cursor: Option<Point>,
    ) -> (TunerTool<FakeSource>, Arc<AtomicU64>) {
        let plan = plan_of(config);
        let (now, clock) = StepNow::new();
        let source = FakeSource { reads, cursor };
        let tool = TunerTool::new(plan, source, display(), Box::new(now));
        (tool, clock)
    }

    #[test]
    fn the_plan_resolves_the_settings_region_and_spring() {
        let plan = plan_of(&config("hid", Some([500, 600])));

        assert_eq!(plan.settings.target_grade, "DG");
        assert_eq!(plan.settings.grade_order.len(), 5);
        assert_eq!(plan.settings.spring_mode, SpringMode::Hid);
        assert_eq!(plan.region, Rect::new(100, 200, 300, 320));
        assert_eq!(plan.spring, Some(Point::new(500, 600)));
        assert!(
            !plan.settings.filter.enabled,
            "the fixture's tuner.filter is disabled"
        );
    }

    #[test]
    fn the_plan_reads_the_filter_from_inside_the_tuner_block() {
        // TOOL-F1: the live merged config puts the filter under `tuner`; a plan that reads a
        // top-level `filter` always gets the disabled default and ignores require_grade/rules.
        let mut config = config("manual", None);
        config["tuner"]["filter"] = serde_json::json!({
            "enabled": true,
            "match_mode": "any",
            "rules": [{"name": "STR"}],
        });

        let plan = plan_of(&config);

        assert!(
            plan.settings.filter.enabled,
            "tuner.filter is the real path"
        );
        assert_eq!(plan.settings.filter.rules.len(), 1);
    }

    #[test]
    fn a_malformed_filter_is_refused_instead_of_defaulting_to_disabled() {
        let mut config = config("manual", None);
        config["tuner"]["filter"] = serde_json::json!("not a filter block");

        let line = plan(&config).expect_err("a malformed filter must not load as filter-off");

        assert!(line.contains("tuner.filter"), "{line}");
        assert!(line.contains("isn't usable"), "{line}");
    }

    #[test]
    fn the_shipped_defaults_put_the_filter_inside_the_tuner_block() {
        // The bug TOOL-F1 describes was visible in the committed config and hidden by the fixtures:
        // `defaults.yaml` turns the filter ON with a DG requirement INSIDE `tuner:`. Pin the real
        // file so a later edit that moves it again fails here.
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../config/defaults.yaml");
        let text = std::fs::read_to_string(&path).expect("the repo's defaults.yaml is readable");
        let defaults = seal_config::yaml::parse(&text).expect("defaults.yaml parses");
        assert!(
            defaults.get("filter").is_none(),
            "the filter is not a top-level key"
        );
        let filter = filter_config(&defaults).expect("the shipped filter block resolves");
        assert!(filter.enabled, "the shipped filter is ON");
        assert_eq!(filter.require_grade.as_deref(), Some("DG"));
        assert_eq!(filter.rules.len(), 1, "the shipped rule table resolves");
    }

    #[test]
    fn an_uncalibrated_region_is_refused_at_start_with_the_calibrate_line() {
        let mut config = config("manual", None);
        config["tuner"]["ocr"] = serde_json::json!({});

        let line = plan(&config).expect_err("no region");

        assert!(line.contains("tuner.ocr.region"), "{line}");
        assert!(line.contains("Calibrate"), "{line}");
    }

    #[test]
    fn hid_without_a_spring_point_is_refused_with_the_domains_line() {
        let line = plan(&config("hid", None)).expect_err("hid needs a point");

        assert!(line.contains("spring_mode is hid"), "{line}");
        assert!(line.contains("spring_point"), "{line}");
    }

    #[test]
    fn the_status_shows_the_countdown_then_the_filter_line() {
        let (mut tool, clock) = tool(&config("manual", None), vec![Ok(read_target())], None);

        let status = tool.status();
        assert_eq!(status.message, "Starting in 5...");
        assert_eq!(status.countdown_s, Some(5.0));

        for _ in 0..START_COUNTDOWN {
            tool.decide();
            clock.fetch_add(1000, Ordering::SeqCst);
        }
        // The fifth Wait was decided while the clock still read 4 s; the next decision crosses the
        // countdown boundary and starts the first attempt.
        tool.decide();
        assert!(
            tool.status().countdown_s.is_none(),
            "the countdown is over once the first attempt starts"
        );

        // Through the attempt to the read; the filter is disabled in this fixture, and the domain
        tool.decide();
        tool.decide();
        tool.decide();
        tool.decide();
        tool.decide();
        let status = tool.status();
        assert_eq!(
            status.message, "",
            "Grade/Attempt/Filter are detail now, not the message (no fact twice)"
        );
        assert!(
            status.detail.contains(&ToolDetail::labeled("Grade", "DG")),
            "{:?}",
            status.detail
        );
        assert!(
            status
                .detail
                .contains(&ToolDetail::labeled("Remaining", "10")),
            "{:?}",
            status.detail
        );
        assert!(
            status.detail.contains(&ToolDetail::labeled("Attempt", "1")),
            "{:?}",
            status.detail
        );
        assert!(
            status
                .detail
                .contains(&ToolDetail::labeled("Filter", "disabled")),
            "{:?}",
            status.detail
        );

        // G11: the run's last facts survive the stop reason and are built before the tool is dropped.
        let summary = tool
            .summary(&StopReason::done("Reached DG"))
            .expect("the tuner publishes a summary");
        assert_eq!(summary["grade"], "DG");
        assert_eq!(summary["attempts"], 1);
        assert_eq!(summary["captures_saved"], 0);
        assert_eq!(summary["stopped_by"], "done");
    }

    #[test]
    fn one_attempt_is_countdown_place_click_enter_wait_read_then_stop() {
        let (mut tool, clock) = tool(
            &config("hid", Some([500, 600])),
            vec![Ok(read_target())],
            None,
        );

        // Countdown: one Wait per second, five of them.
        for _ in 0..START_COUNTDOWN {
            assert_eq!(tool.decide(), Decision::Wait(1.0));
            tick(&clock);
        }
        assert_eq!(tool.decide(), Decision::Place(Point::new(500, 600)));
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.4));
        assert_eq!(tool.decide(), Decision::Enter);
        assert_eq!(tool.decide(), Decision::Wait(0.8));
        let stop = tool.decide();
        match stop {
            Decision::Stop(reason) => {
                assert_eq!(reason.kind, StopKind::Done);
                assert!(reason.message.contains("DG"), "{}", reason.message);
            }
            other => panic!("expected the done stop, got {other:?}"),
        }
    }

    #[test]
    fn manual_mode_clicks_where_the_player_left_the_cursor() {
        let (mut tool, clock) = tool(&config("manual", None), vec![Ok(read_target())], None);

        for _ in 0..START_COUNTDOWN {
            assert_eq!(tool.decide(), Decision::Wait(1.0));
            tick(&clock);
        }
        assert_eq!(
            tool.decide(),
            Decision::ClickHere,
            "no placement in manual mode"
        );
    }

    /// The `ocr_log.jsonl` record keeps the `ocr.scan` shape and adds the tuner's own facts: which
    /// re-capture produced the read and whether it was complete.
    #[test]
    fn a_completed_read_records_the_scan_shape_plus_the_tuner_facts() {
        let read = confirmed_read("DG", 9);
        let record = completed_read_record(
            &read,
            &["GRADE: DG".to_owned(), "STR +1".to_owned()],
            Some(&[vec!["STR".to_owned(), "+1".to_owned()]]),
            2,
            "20261104_010203_456",
            Some("logs/captures/capture_20261104_010203_456.png"),
        );

        // The ocr.scan record's own fields, under the same names.
        assert_eq!(record["timestamp"], "20261104_010203_456");
        assert_eq!(record["grade"], "DG");
        assert_eq!(record["remaining"], 9);
        assert_eq!(record["lines"][0], "GRADE: DG");
        assert_eq!(record["attributes"][0][0], "STR");
        assert_eq!(
            record["capture_path"],
            "logs/captures/capture_20261104_010203_456.png"
        );
        // The tuner diagnosis facts.
        assert_eq!(record["source"], "tuner");
        assert_eq!(record["attempt"], 2);
        assert_eq!(record["confirmed"], true);

        // A partial read says so, and a missing capture is null rather than absent.
        let partial =
            completed_read_record(&partial_read(), &[], None, 3, "20261104_010203_457", None);
        assert_eq!(partial["confirmed"], false);
        assert_eq!(partial["attempt"], 3);
        assert!(partial["capture_path"].is_null());
    }

    /// A partial last attempt is force-captured (the flag v2's `forceCapture` uses) and its record is
    /// kept so `read()` can write `ocr_errors.jsonl`.
    #[test]
    fn the_last_partial_attempt_is_force_captured_and_kept_for_the_error_log() {
        let mut seen = Vec::new();
        let policy = read_policy(3, |attempt, force_capture| {
            seen.push((attempt, force_capture));
            Ok(CompletedRead {
                read: partial_read(),
                record: json!({"attempt": attempt}),
            })
        });

        assert_eq!(
            seen,
            vec![(1, false), (2, false), (3, true)],
            "only the final retry forces the capture"
        );
        let ReadPolicy::Unconfirmed { read, record } = policy else {
            panic!("a partial read is not confirmed");
        };
        assert!(!read.is_confirmed());
        assert_eq!(record["attempt"], 3, "the LAST attempt is the one kept");
    }

    #[test]
    fn a_confirmed_read_ends_the_retries() {
        let mut seen = Vec::new();
        let policy = read_policy(3, |attempt, force_capture| {
            seen.push((attempt, force_capture));
            Ok(CompletedRead {
                read: if attempt == 2 {
                    read_target()
                } else {
                    partial_read()
                },
                record: json!({"attempt": attempt}),
            })
        });

        assert_eq!(seen, vec![(1, false), (2, false)]);
        let ReadPolicy::Confirmed(read) = policy else {
            panic!("the second attempt is complete");
        };
        assert!(read.is_confirmed());
    }

    #[test]
    fn a_final_capture_failure_is_the_stop_not_an_earlier_partial_read() {
        let policy = read_policy(2, |attempt, _force_capture| {
            if attempt == 1 {
                Ok(CompletedRead {
                    read: partial_read(),
                    record: json!({}),
                })
            } else {
                Err("the game window is not in front".to_owned())
            }
        });

        let ReadPolicy::Failed(error) = policy else {
            panic!("the last attempt's error is the stop");
        };
        assert_eq!(error, "the game window is not in front");
    }

    /// Issue #16 end-to-end: the run used to click again on each partial read and stop as
    /// "same result x3" on the third. Now the FIRST partial read stops, before a second spring.
    #[test]
    fn a_partial_read_stops_with_the_confirming_error_before_another_click() {
        let reads = vec![Ok(partial_read()), Ok(partial_read()), Ok(partial_read())];
        let (mut tool, clock) = tool(&config("manual", None), reads, None);

        for _ in 0..START_COUNTDOWN {
            assert_eq!(tool.decide(), Decision::Wait(1.0));
            tick(&clock);
        }
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.4));
        assert_eq!(tool.decide(), Decision::Enter);
        assert_eq!(tool.decide(), Decision::Wait(0.8));
        match tool.decide() {
            Decision::Stop(reason) => {
                assert_eq!(reason.kind, StopKind::Error);
                assert_eq!(
                    reason.message,
                    seal_domain::tools::tuner::UNCONFIRMED_READ_MESSAGE
                );
                assert!(
                    !reason.message.contains("STUCK"),
                    "a partial read must not end as a false STUCK: {}",
                    reason.message
                );
            }
            other => panic!("expected the confirming stop, got {other:?}"),
        }
    }

    #[test]
    fn a_below_target_read_retries_the_whole_attempt() {
        let (mut tool, clock) = tool(
            &config("manual", None),
            vec![Ok(read_below_target()), Ok(read_target())],
            None,
        );

        for _ in 0..START_COUNTDOWN {
            assert_eq!(tool.decide(), Decision::Wait(1.0));
            tick(&clock);
        }
        // First attempt leads to a retry, not a stop.
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.4));
        assert_eq!(tool.decide(), Decision::Enter);
        assert_eq!(tool.decide(), Decision::Wait(0.8));
        // The retry starts a SECOND click sequence.
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.4));
        assert_eq!(tool.decide(), Decision::Enter);
        assert_eq!(tool.decide(), Decision::Wait(0.8));
        assert!(matches!(tool.decide(), Decision::Stop(_)));
    }

    #[test]
    fn a_total_read_failure_stops_with_the_v2_line() {
        let (mut tool, clock) = tool(
            &config("manual", None),
            vec![Ok(TunerRead::default())],
            None,
        );

        for _ in 0..START_COUNTDOWN {
            assert_eq!(tool.decide(), Decision::Wait(1.0));
            tick(&clock);
        }
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.4));
        assert_eq!(tool.decide(), Decision::Enter);
        assert_eq!(tool.decide(), Decision::Wait(0.8));
        match tool.decide() {
            Decision::Stop(reason) => {
                assert_eq!(reason.kind, StopKind::Error);
                assert!(reason.message.contains("Read failed"), "{}", reason.message);
            }
            other => panic!("expected the read failure, got {other:?}"),
        }
    }

    #[test]
    fn a_source_error_stops_with_its_line() {
        let (mut tool, clock) = tool(
            &config("manual", None),
            vec![Err("the game window is not in front".to_owned())],
            None,
        );

        for _ in 0..START_COUNTDOWN {
            assert_eq!(tool.decide(), Decision::Wait(1.0));
            tick(&clock);
        }
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.4));
        assert_eq!(tool.decide(), Decision::Enter);
        assert_eq!(tool.decide(), Decision::Wait(0.8));
        match tool.decide() {
            Decision::Stop(reason) => {
                assert_eq!(reason.kind, StopKind::Error);
                assert_eq!(reason.message, "the game window is not in front");
            }
            other => panic!("expected the source line, got {other:?}"),
        }
    }

    #[test]
    fn the_guard_stops_when_the_cursor_left_the_spring() {
        let mut config = config("hid", Some([500, 600]));
        config["tuner"]["mouse_guard"] = serde_json::json!("stop");
        // The cursor is far from the spring's logical point (500,600 at 1:1).
        let (mut tool, clock) = tool(&config, vec![Ok(read_target())], Some(Point::new(10, 10)));

        for _ in 0..START_COUNTDOWN {
            assert_eq!(tool.decide(), Decision::Wait(1.0));
            tick(&clock);
        }
        assert_eq!(tool.decide(), Decision::Place(Point::new(500, 600)));
        match tool.decide() {
            Decision::Stop(reason) => {
                assert_eq!(reason.kind, StopKind::Guard);
                assert!(reason.message.contains("mouse moved"), "{}", reason.message);
            }
            other => panic!("expected the guard stop, got {other:?}"),
        }
    }

    #[test]
    fn a_filtered_run_stops_with_filter_matched_even_below_the_target_grade() {
        // TOOL-F1's live symptom: a filter-only setup ("stop when the attributes match, at any grade")
        // ran on to the target because the run never saw the filter. Here the read is below
        // target_grade but passes the rule, so the domain stops with FILTER MATCHED.
        let mut config = config("manual", None);
        config["tuner"]["filter"] = serde_json::json!({
            "enabled": true,
            "match_mode": "any",
            "rules": [{"name": "STR", "count": 1}],
        });
        let (mut tool, clock) = tool(&config, vec![Ok(confirmed_read("G", 10))], None);

        for _ in 0..START_COUNTDOWN {
            assert_eq!(tool.decide(), Decision::Wait(1.0));
            tick(&clock);
        }
        // One whole attempt leads to the read, and the read ends the run.
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.4));
        assert_eq!(tool.decide(), Decision::Enter);
        assert_eq!(tool.decide(), Decision::Wait(0.8));
        match tool.decide() {
            Decision::Stop(reason) => {
                assert_eq!(reason.kind, StopKind::Done);
                assert!(
                    reason.message.contains("FILTER MATCHED at grade G"),
                    "{}",
                    reason.message
                );
            }
            other => panic!("expected the filter match, got {other:?}"),
        }
    }

    #[test]
    fn max_retries_two_gives_exactly_two_clicks() {
        // TOOL-F6: v2 counts the attempt BEFORE the click. With the countdown beginning the run,
        // `max_retries: 2` is exactly two springs; before the fix the counter started at 0, so the
        // same config spent a third spring.
        let mut config = config("manual", None);
        config["tuner"]["max_retries"] = serde_json::json!(2);
        let reads = vec![Ok(confirmed_read("G", 10)), Ok(confirmed_read("N", 10))];
        let (mut tool, clock) = tool(&config, reads, None);

        for _ in 0..START_COUNTDOWN {
            assert_eq!(tool.decide(), Decision::Wait(1.0));
            tick(&clock);
        }
        let mut clicks = 0;
        let mut stop = None;
        for _ in 0..20 {
            match tool.decide() {
                Decision::ClickHere => clicks += 1,
                Decision::Stop(reason) => {
                    stop = Some(reason);
                    break;
                }
                _ => {}
            }
        }

        assert_eq!(clicks, 2, "max_retries 2 means two attempts, not three");
        let stop = stop.expect("the run stops on the ceiling");
        assert_eq!(stop.kind, StopKind::Done);
        assert!(stop.message.contains("Max 2"), "{}", stop.message);
    }
}

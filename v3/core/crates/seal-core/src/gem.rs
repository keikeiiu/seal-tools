//! The gem composer: the READ-LOOP adapter around `seal-domain`'s [`GemRun`].
//!
//! The decision half is entirely the domain's: this module resolves the plan from the merged config,
//! holds the loop position (which turn is next), turns each [`GemTurn`] into [`Decision`]s in order,
//! and reads the one thing the composer observes — whether the composed-result box is empty.
//!
//! # The raw crop is the primary empty test
//!
//! `config/calib_gem_result.png` (saved by Calibrate Gem; `CALIBRATION.md` §5) is the empty result
//! box. The live capture of `gem.result_gem_area` is compared against it with v2's rule
//! (`GemComposer.cs:19,26`): a pixel differs when ANY channel differs by more than 30, and the
//! comparison ignores a 6-px inset because the box's drawn frame moves by a pixel when the window
//! moves — with the frame included an EMPTY box measured 7.7% differing, without it 0.0%, while a
//! gem scored 55% either way. The fraction is the colour-AND-shape-blind signal, so any gem reads the
//! same.
//!
//! When the crop is absent, the weaker colour composition (v2 `GemColorAnalyzer`) is the fallback;
//! with neither reference the reader answers [`EmptyRead::NoReference`], which the domain treats as
//! NOT empty — a missing reference must never advance a grade.
//!
//! # A missing calibration is a start refusal
//!
//! [`plan`] probes the domain with the run's opening turns, so an uncalibrated grade button, route
//! point or tuned move is the domain's own player line at Start rather than a failure mid-run. When
//! the run WILL read the result box (the domain's `empty_check_enabled`) a missing
//! `gem.result_gem_area` is refused with a named line; the read must never be guessed.
//!
//! # What this build does not do (reported)
//!
//! - The optional empty-check debug crops (`gem.save_empty_captures`) wait for the core's logging
//!   writer; the flag is resolved into the plan but nothing is written.
//! - The F9 "advance grade by hand" hotkey has no protocol path yet; [`GemTool::advance_grade_by_hand`]
//!   is the seam a future hotkey poll calls, and the tests exercise it directly.

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use seal_domain::config::{ColorComposition, MovementsConfig};
use seal_domain::decision::{Decision, StopReason};
use seal_domain::geometry::Rect;
use seal_domain::tools::gem::{
    EmptyMode, EmptyRead, GemRun, GemSettings, GemStep, GemTurn, MoveMode, result_box_is_empty,
};
use seal_tools::runner::{Tool, ToolStatus};
use seal_tools::tool::{ToolId, ToolRow};
use serde_json::Value;

/// v2's per-channel difference above which a pixel counts as "not the empty box"
/// (`GemComposer.cs:19`).
const EMPTY_DIFF_TOLERANCE: i32 = 30;

/// v2's border inset for the crop comparison (`GemComposer.cs:26`): the drawn frame moves with the
/// window, and its rows alone made an empty box read as 7.7% differing.
const EMPTY_COMPARE_INSET: usize = 6;

/// The refusal when the run needs the result-box read but the area is not calibrated.
const RESULT_AREA_LINE: &str =
    "gem.result_gem_area isn't calibrated — draw the result box in Calibrate Gem and save it.";

/// The refusal when the area exists but is unusable.
const RESULT_AREA_AREA_LINE: &str =
    "gem.result_gem_area has no area — draw it again in Calibrate Gem.";

/// The reference crop's path convention (`CALIBRATION.md` §5): beside the other calibration
/// artifacts, under the application root.
const REFERENCE_CROP: &str = "calib_gem_result.png";

/// The gem composer's resolved start data: the domain settings plus the reader's calibration.
#[derive(Debug, Clone, PartialEq)]
pub struct GemPlan {
    pub settings: GemSettings,
    /// `gem.result_gem_area` `[x, y, w, h]`; `None` when unset (allowed only when no read will run).
    pub result_area: Option<Rect>,
    /// `gem.colored_gap_min` (0-255), used by the colour-composition fallback.
    pub colored_gap_min: i32,
    /// `gem.save_empty_captures`; resolved but not yet written (see the module docs).
    pub save_empty_captures: bool,
}

fn number(value: &Value, path: &str) -> Option<i32> {
    value
        .pointer(path)
        .and_then(Value::as_i64)
        .and_then(|number| i32::try_from(number).ok())
}

fn float(value: &Value, path: &str) -> Option<f64> {
    value.pointer(path).and_then(Value::as_f64)
}

fn boolean(value: &Value, path: &str) -> Option<bool> {
    value.pointer(path).and_then(Value::as_bool)
}

fn text(value: &Value, path: &str) -> Option<String> {
    value
        .pointer(path)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn string_list(value: &Value, path: &str) -> Vec<String> {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// `gem.grade_positions`: point name → `[x, y]`.
fn grade_positions(value: &Value) -> BTreeMap<String, Vec<i32>> {
    let Some(object) = value
        .pointer("/gem/grade_positions")
        .and_then(Value::as_object)
    else {
        return BTreeMap::new();
    };
    object
        .iter()
        .map(|(name, points)| {
            let points = points
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_i64)
                        .filter_map(|number| i32::try_from(number).ok())
                        .collect()
                })
                .unwrap_or_default();
            (name.clone(), points)
        })
        .collect()
}

/// `gem.resource_gems`: the three resource-slot points.
fn resource_gems(value: &Value) -> Vec<Vec<i32>> {
    value
        .pointer("/gem/resource_gems")
        .and_then(Value::as_array)
        .map(|points| {
            points
                .iter()
                .map(|point| {
                    point
                        .as_array()
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(Value::as_i64)
                                .filter_map(|number| i32::try_from(number).ok())
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Resolve the domain settings out of the merged `gem` block.
fn settings(config: &Value) -> Result<GemSettings, String> {
    let grades = string_list(config, "/gem/grades");
    let start_grade = text(config, "/gem/start_grade")
        .unwrap_or_else(|| grades.first().cloned().unwrap_or_default());
    let movements: MovementsConfig = config
        .pointer("/gem/movements")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| format!("gem.movements isn't usable — {error}."))?
        .unwrap_or_default();
    let empty_signature = match config.pointer("/gem/empty_signature") {
        None | Some(Value::Null) => None,
        Some(value) => Some(serde_json::from_value(value.clone()).map_err(|_| {
            "gem.empty_signature isn't usable — sample the empty result box again in Calibrate Gem."
                .to_owned()
        })?),
    };
    Ok(GemSettings {
        grades,
        start_grade,
        grade_positions: grade_positions(config),
        movements,
        move_mode: MoveMode::parse(
            &text(config, "/gem/move_mode").unwrap_or_else(|| "arduino".to_owned()),
        ),
        resource_gems: resource_gems(config),
        // The merged config always carries the defaults; these fallbacks are the v2 `AppConfig`
        // property defaults for a config that predates the blocks.
        empty_mode: EmptyMode::parse(
            &text(config, "/gem/empty_mode").unwrap_or_else(|| "stop".to_owned()),
        ),
        empty_distance: float(config, "/gem/empty_distance").unwrap_or(0.18),
        empty_signature,
        empty_streak: number(config, "/gem/empty_streak").unwrap_or(2),
    })
}

/// `gem.result_gem_area` `[x, y, w, h]`.
fn result_area(config: &Value) -> Result<Option<Rect>, String> {
    let Some(value) = config.pointer("/gem/result_gem_area") else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(values) = value.as_array() else {
        return Err("gem.result_gem_area isn't a usable [x, y, w, h] area — draw it again in Calibrate Gem.".to_owned());
    };
    if values.len() != 4 {
        return Err("gem.result_gem_area isn't a usable [x, y, w, h] area — draw it again in Calibrate Gem.".to_owned());
    }
    let mut numbers = [0i32; 4];
    for (slot, value) in numbers.iter_mut().zip(values) {
        *slot = value
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .ok_or_else(|| {
                "gem.result_gem_area isn't a usable [x, y, w, h] area — draw it again in Calibrate Gem."
                    .to_owned()
            })?;
    }
    let rect = Rect::new(numbers[0], numbers[1], numbers[2], numbers[3]);
    if rect.w <= 0 || rect.h <= 0 {
        return Err(RESULT_AREA_AREA_LINE.to_owned());
    }
    Ok(Some(rect))
}

/// Resolve and VALIDATE the plan: a missing calibrated point/move surfaces here as the domain's own
/// player line (the probe asks the domain for its opening turns without performing them).
pub fn plan(config: &Value) -> Result<GemPlan, String> {
    let settings = settings(config)?;
    // The probe is the domain's own start sequence: if a grade button, route point or tuned move is
    // missing, the refusal is the line the domain already writes for it.
    let mut probe = GemRun::new(settings.clone()).map_err(|reason| reason.message)?;
    probe.select_grade_turn().map_err(|reason| reason.message)?;
    probe.combine_turn().map_err(|reason| reason.message)?;
    probe.finish_turn(None).map_err(|reason| reason.message)?;

    let result_area = result_area(config)?;
    let reads = settings.empty_mode != EmptyMode::Stop && settings.empty_signature.is_some();
    if reads && result_area.is_none() {
        return Err(RESULT_AREA_LINE.to_owned());
    }
    Ok(GemPlan {
        settings,
        result_area,
        colored_gap_min: number(config, "/gem/colored_gap_min").unwrap_or(40),
        save_empty_captures: boolean(config, "/gem/save_empty_captures").unwrap_or(false),
    })
}

/// One read: the box's colour composition (the fallback) or the fraction of pixels differing from
/// the saved empty crop (the primary signal).
pub trait GemSource {
    fn empty_read(&mut self) -> Result<EmptyRead, String>;
}

/// The empty-result crop: `config/calib_gem_result.png`, decoded to RGBA8 at open.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReferenceCrop {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

/// The production reader: capture the calibrated area and compare/analyse it.
pub struct GemResultSource {
    title: String,
    area: Option<Rect>,
    reference: Option<ReferenceCrop>,
    signature: Option<ColorComposition>,
    colored_gap_min: i32,
}

impl GemResultSource {
    /// Open the read side. The reference crop is loaded ONCE at open (v2's `EmptyReference()` loaded
    /// it once too); a crop that exists but cannot be decoded is a start refusal naming the file
    /// rather than a silent fallback, because the primary signal is the one the measurements trust.
    pub fn open(config: &Value, plan: &GemPlan, root: &Path) -> Result<Self, String> {
        let title = crate::win::configured_title(config).map_err(|error| error.line())?;
        let path = root.join("config").join(REFERENCE_CROP);
        let reference = if path.is_file() {
            let bytes = std::fs::read(&path)
                .map_err(|error| format!("config/{REFERENCE_CROP} could not be read — {error}"))?;
            let (width, height, rgba) = seal_win::pure::decode_png(&bytes).map_err(|_| {
                format!("config/{REFERENCE_CROP} couldn't be decoded — save the empty result box again in Calibrate Gem.")
            })?;
            Some(ReferenceCrop {
                width: width as usize,
                height: height as usize,
                rgba,
            })
        } else {
            None
        };
        Ok(Self {
            title,
            area: plan.result_area,
            reference,
            signature: plan.settings.empty_signature,
            colored_gap_min: plan.colored_gap_min,
        })
    }
}

impl GemSource for GemResultSource {
    fn empty_read(&mut self) -> Result<EmptyRead, String> {
        let Some(area) = self.area else {
            return Err(RESULT_AREA_LINE.to_owned());
        };
        let (frame, _display) =
            crate::win::capture(&self.title, Some(area)).map_err(|error| error.line())?;
        let (width, height, rgba) = crate::dispatch::frame_rgba(&frame)?;
        Ok(empty_read_from_frame(
            self.reference.as_ref(),
            self.signature.as_ref(),
            self.colored_gap_min,
            width,
            height,
            &rgba,
        ))
    }
}

/// The pure part of the read: compare with the crop when sizes agree, else fall back to the colour
/// composition, else "no reference" (which the domain treats as NOT empty).
fn empty_read_from_frame(
    reference: Option<&ReferenceCrop>,
    signature: Option<&ColorComposition>,
    colored_gap_min: i32,
    width: usize,
    height: usize,
    rgba: &[u8],
) -> EmptyRead {
    if let Some(reference) = reference
        && reference.width == width
        && reference.height == height
        && let Some(fraction) =
            diff_fraction(rgba, reference.width, reference.height, &reference.rgba)
    {
        return EmptyRead::DiffFraction(fraction);
    }
    match signature {
        // The domain's arithmetic compares the composition with the signature, so this reference
        // only decides WHETHER the fallback exists; the pixels carry the six features.
        Some(_) => {
            EmptyRead::Composition(analyze_composition(rgba, width, height, colored_gap_min))
        }
        None => EmptyRead::NoReference,
    }
}

/// v2 `GemColorAnalyzer.DiffFraction`: the fraction of pixels differing by more than `tolerance` on
/// any channel, ignoring a square `inset` at the border. Channel order is irrelevant here (the
/// maximum absolute difference is per channel), so RGBA and BGR pairs compare the same way.
fn diff_fraction(live: &[u8], width: usize, height: usize, reference: &[u8]) -> Option<f64> {
    if live.len() < width * height * 4 || reference.len() < width * height * 4 {
        return None;
    }
    let mut inset = EMPTY_COMPARE_INSET;
    if inset * 2 >= width.min(height) {
        inset = 0;
    }
    let mut differing = 0usize;
    let mut compared = 0usize;
    for y in inset..height - inset {
        for x in inset..width - inset {
            let live_pixel = (y * width + x) * 4;
            let reference_pixel = live_pixel;
            let mut difference = 0i32;
            for channel in 0..3 {
                difference = difference.max(
                    (i32::from(live[live_pixel + channel])
                        - i32::from(reference[reference_pixel + channel]))
                    .abs(),
                );
            }
            compared += 1;
            if difference > EMPTY_DIFF_TOLERANCE {
                differing += 1;
            }
        }
    }
    (compared > 0).then(|| differing as f64 / compared as f64)
}

/// v2 `GemColorAnalyzer.Analyze`: the six-feature colour fingerprint of a box. `colored_gap_min` is
/// the channel max-min gap above which a pixel counts as distinctly coloured.
fn analyze_composition(
    rgba: &[u8],
    width: usize,
    height: usize,
    colored_gap_min: i32,
) -> ColorComposition {
    let pixels = width.saturating_mul(height);
    if pixels == 0 || rgba.len() < pixels * 4 {
        return ColorComposition::default();
    }
    let mut sum_v = 0.0;
    let mut sum_sat = 0.0;
    let mut sum_r = 0.0;
    let mut sum_g = 0.0;
    let mut sum_b = 0.0;
    let mut colored = 0usize;
    for pixel in rgba[..pixels * 4].as_chunks::<4>().0 {
        let (r, g, b) = (
            i32::from(pixel[0]),
            i32::from(pixel[1]),
            i32::from(pixel[2]),
        );
        let value = r.max(g).max(b);
        let minimum = r.min(g).min(b);
        let saturation = if value == 0 {
            0
        } else {
            (value - minimum) * 255 / value
        };
        sum_v += f64::from(value) / 255.0;
        sum_sat += f64::from(saturation) / 255.0;
        sum_r += f64::from(r) / 255.0;
        sum_g += f64::from(g) / 255.0;
        sum_b += f64::from(b) / 255.0;
        if value - minimum >= colored_gap_min {
            colored += 1;
        }
    }
    let count = pixels as f64;
    ColorComposition {
        mean_v: sum_v / count,
        mean_sat: sum_sat / count,
        colored_fraction: colored as f64 / count,
        mean_r: sum_r / count,
        mean_g: sum_g / count,
        mean_b: sum_b / count,
    }
}

/// Where the loop is in v2's turn sequence.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    /// Choose the grade and move it to Register (the run's opening, and after a manual advance).
    Select,
    /// Count the cycle and combine.
    Combine,
    /// Read the result box for the combine just performed.
    Read,
    /// Move back to Register and either continue or advance the grade.
    Finish(Option<bool>),
    /// The last grade finished; the queued steps drain and then the stop is returned.
    Finished,
}

/// The composer as the runner sees it.
pub struct GemTool<S: GemSource> {
    run: GemRun,
    plan: GemPlan,
    source: S,
    queue: VecDeque<Decision>,
    phase: Phase,
    pending_stop: Option<StopReason>,
}

impl<S: GemSource> GemTool<S> {
    pub fn new(plan: GemPlan, source: S) -> Result<Self, String> {
        let run = GemRun::new(plan.settings.clone()).map_err(|reason| reason.message)?;
        Ok(Self {
            run,
            plan,
            source,
            queue: VecDeque::new(),
            phase: Phase::Select,
            pending_stop: None,
        })
    }

    /// F9: advance the grade BY HAND. The hotkey path does not exist yet (the core polls no
    /// hotkeys), so this is the seam a future poll calls; it uses the domain's own turn, which logs
    /// the move as "left by hand" rather than "finished".
    pub fn advance_grade_by_hand(&mut self) -> Result<(), String> {
        if self.phase == Phase::Finished {
            return Err("the composer has already finished".to_owned());
        }
        let turn = self
            .run
            .advance_grade_by_hand()
            .map_err(|reason| reason.message)?;
        self.push_turn(turn);
        self.phase = Phase::Combine;
        self.pending_stop = None;
        Ok(())
    }

    fn push_turn(&mut self, turn: GemTurn) {
        for step in turn.steps {
            self.queue.push_back(map_step(step));
        }
        if let Some(stop) = turn.stop {
            self.pending_stop = Some(stop);
            self.phase = Phase::Finished;
        }
    }

    fn message(&self) -> String {
        format!("Grade {} — cycle {}", self.run.grade(), self.run.cycle())
    }
}

fn map_step(step: GemStep) -> Decision {
    match step {
        GemStep::Place(point) => Decision::Place(point),
        GemStep::Move { dx, dy } => Decision::MoveBy { dx, dy },
        GemStep::ClickLeft => Decision::ClickHere,
        GemStep::ClickRight => Decision::RightClickHere,
        GemStep::Wait(seconds) => Decision::Wait(seconds),
    }
}

impl<S: GemSource + Send> Tool for GemTool<S> {
    fn id(&self) -> ToolId {
        ToolId::Gem
    }

    fn status(&self) -> ToolStatus {
        let rows = self
            .run
            .tally()
            .rows(&self.plan.settings.grades)
            .into_iter()
            .map(|row| ToolRow::new(row.name, row.status))
            .collect();
        ToolStatus {
            message: self.message(),
            countdown_s: None,
            rows,
        }
    }

    fn decide(&mut self) -> Decision {
        loop {
            if let Some(decision) = self.queue.pop_front() {
                return decision;
            }
            if let Some(stop) = self.pending_stop.take() {
                return Decision::Stop(stop);
            }
            match self.phase {
                Phase::Finished => {
                    return Decision::Stop(StopReason::done("The gem composer has finished."));
                }
                Phase::Select => match self.run.select_grade_turn() {
                    Ok(turn) => {
                        self.push_turn(turn);
                        if self.phase != Phase::Finished {
                            self.phase = Phase::Combine;
                        }
                    }
                    Err(reason) => return Decision::Stop(reason),
                },
                Phase::Combine => match self.run.combine_turn() {
                    Ok(turn) => {
                        self.push_turn(turn);
                        if self.phase != Phase::Finished {
                            self.phase = if self.run.empty_check_enabled() {
                                Phase::Read
                            } else {
                                Phase::Finish(None)
                            };
                        }
                    }
                    Err(reason) => return Decision::Stop(reason),
                },
                Phase::Read => {
                    let read = match self.source.empty_read() {
                        Ok(read) => read,
                        Err(line) => return Decision::Stop(StopReason::error(line)),
                    };
                    let empty = result_box_is_empty(
                        &read,
                        self.plan.settings.empty_signature.as_ref(),
                        self.plan.settings.empty_distance,
                    );
                    self.phase = Phase::Finish(Some(empty));
                }
                Phase::Finish(empty) => match self.run.finish_turn(empty) {
                    Ok(turn) => {
                        self.push_turn(turn);
                        if self.phase != Phase::Finished {
                            self.phase = Phase::Combine;
                        }
                    }
                    Err(reason) => return Decision::Stop(reason),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seal_domain::geometry::Point;

    fn signature() -> Value {
        serde_json::json!({
            "mean_v": 0.3, "mean_sat": 0.1, "colored_fraction": 0.05,
            "mean_r": 0.3, "mean_g": 0.3, "mean_b": 0.3
        })
    }

    fn arduino_config(empty_mode: &str, empty_streak: i32) -> Value {
        serde_json::json!({
            "window": {"title": "TW_LIVE"},
            "gem": {
                "grades": ["N", "G"],
                "start_grade": "N",
                "empty_mode": empty_mode,
                "empty_streak": empty_streak,
                "move_mode": "arduino",
                "colored_gap_min": 40,
                "empty_distance": 0.18,
                "empty_signature": signature(),
                "result_gem_area": [610, 800, 220, 60],
                "grade_positions": {
                    "N": [10, 10], "G": [20, 20],
                    "Register": [40, 40], "Combine": [50, 50]
                },
                "resource_gems": [[60, 60], [70, 70], [80, 80]]
            }
        })
    }

    fn tuned_config() -> Value {
        serde_json::json!({
            "window": {"title": "TW_LIVE"},
            "gem": {
                "grades": ["N"],
                "start_grade": "N",
                "empty_mode": "stop",
                "move_mode": "tuned",
                "grade_positions": {"N": [10, 10]},
                "movements": {
                    "radio_to_register": {"N": [-10, 20]},
                    "register_combine": [30, 40],
                    "combine_register": [-30, -40]
                }
            }
        })
    }

    fn plan_of(config: &Value) -> GemPlan {
        plan(config).expect("resolves")
    }

    /// A scripted read seam: one `EmptyRead` per call, then a failure.
    struct FakeSource {
        reads: Vec<Result<EmptyRead, String>>,
        used: usize,
    }

    impl FakeSource {
        fn new(reads: Vec<Result<EmptyRead, String>>) -> Self {
            Self { reads, used: 0 }
        }
    }

    impl GemSource for FakeSource {
        fn empty_read(&mut self) -> Result<EmptyRead, String> {
            let index = self.used;
            self.used += 1;
            self.reads
                .get(index)
                .cloned()
                .unwrap_or_else(|| Err("no more reads".to_owned()))
        }
    }

    /// Walks the tool until it stops, with a hard cap so a loop bug fails the test instead of hanging.
    fn run_until_stop(tool: &mut GemTool<FakeSource>) -> Vec<Decision> {
        let mut decisions = Vec::new();
        for _ in 0..400 {
            let decision = tool.decide();
            let terminal = decision.is_terminal();
            decisions.push(decision);
            if terminal {
                return decisions;
            }
        }
        panic!("the tool did not stop: {decisions:?}");
    }

    fn tool_with(config: &Value, reads: Vec<Result<EmptyRead, String>>) -> GemTool<FakeSource> {
        let source = FakeSource::new(reads);
        GemTool::new(plan_of(config), source).expect("a plan that validates")
    }

    #[test]
    fn the_plan_resolves_the_domain_settings_and_the_reader_calibration() {
        let plan = plan_of(&arduino_config("advance_grade", 2));

        assert_eq!(plan.settings.grades, vec!["N", "G"]);
        assert_eq!(plan.settings.start_grade, "N");
        assert_eq!(plan.settings.move_mode, MoveMode::Arduino);
        assert_eq!(plan.settings.empty_mode, EmptyMode::AdvanceGrade);
        assert_eq!(plan.settings.empty_distance, 0.18);
        assert_eq!(plan.result_area, Some(Rect::new(610, 800, 220, 60)));
        assert_eq!(plan.colored_gap_min, 40);
        assert!(!plan.save_empty_captures);
    }

    #[test]
    fn a_run_that_reads_without_a_result_area_is_refused_with_its_line() {
        let mut config = arduino_config("advance_grade", 2);
        config["gem"]
            .as_object_mut()
            .unwrap()
            .remove("result_gem_area");

        let line = plan(&config).expect_err("the read has nowhere to capture");

        assert!(line.contains("gem.result_gem_area"), "{line}");
        assert!(line.contains("Calibrate Gem"), "{line}");
        assert_eq!(line, RESULT_AREA_LINE);
    }

    #[test]
    fn a_run_that_never_reads_may_omit_the_result_area() {
        let mut config = arduino_config("stop", 2);
        config["gem"]
            .as_object_mut()
            .unwrap()
            .remove("result_gem_area");

        let plan = plan(&config).expect("no read, no area needed");

        assert_eq!(plan.result_area, None);
    }

    #[test]
    fn an_uncalibrated_grade_position_is_refused_with_the_domains_line() {
        let mut config = arduino_config("stop", 2);
        config["gem"]["grade_positions"] =
            serde_json::json!({"Register": [40, 40], "Combine": [50, 50]});

        let line = plan(&config).expect_err("no grade button");

        assert!(
            line.contains("Grade position 'N' isn't calibrated yet"),
            "{line}"
        );
        assert!(line.contains("Calibrate Gem"), "{line}");
    }

    #[test]
    fn an_unsaved_tuned_move_is_refused_with_the_domains_line() {
        let mut config = tuned_config();
        config["gem"]["movements"] = serde_json::json!({});

        let line = plan(&config).expect_err("no tuned counts");

        assert!(line.contains("Save Composer Moves"), "{line}");
    }

    #[test]
    fn no_grades_is_the_domains_own_line() {
        let mut config = arduino_config("stop", 2);
        config["gem"]["grades"] = serde_json::json!([]);

        assert_eq!(
            plan(&config).expect_err("nothing to work through"),
            "No gem grades are configured — the composer has nothing to work through."
        );
    }

    #[test]
    fn arduino_mode_selects_places_and_combines_on_the_configured_points() {
        let config = arduino_config("stop", 2);
        let mut tool = tool_with(&config, Vec::new());

        // The opening turn places the grade button, clicks, then places Register and clicks.
        assert_eq!(tool.decide(), Decision::Place(Point::new(10, 10)));
        assert_eq!(tool.decide(), Decision::Wait(0.3));
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.5));
        assert_eq!(tool.decide(), Decision::Place(Point::new(40, 40)));
        assert_eq!(tool.decide(), Decision::Wait(0.3));
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.5));
        // The combine turn places Combine and clicks.
        assert_eq!(tool.decide(), Decision::Place(Point::new(50, 50)));
        assert_eq!(tool.decide(), Decision::Wait(0.2));
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.8));
    }

    #[test]
    fn tuned_mode_sends_the_measured_raw_moves_not_points() {
        let config = tuned_config();
        let mut tool = tool_with(&config, Vec::new());

        // The grade button is still placed (v2 selects the grade absolutely) ...
        assert_eq!(tool.decide(), Decision::Place(Point::new(10, 10)));
        assert_eq!(tool.decide(), Decision::Wait(0.3));
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.5));
        // ... but every ROUTE is the measured HID move, never a placement.
        assert_eq!(tool.decide(), Decision::MoveBy { dx: -10, dy: 20 });
        assert_eq!(tool.decide(), Decision::Wait(0.3));
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.5));
        assert_eq!(tool.decide(), Decision::MoveBy { dx: 30, dy: 40 });
        assert_eq!(tool.decide(), Decision::Wait(0.2));
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.8));
    }

    #[test]
    fn an_empty_read_advances_the_grade_and_finishes_after_the_last_one() {
        let config = arduino_config("advance_grade", 1);
        let mut tool = tool_with(
            &config,
            vec![
                Ok(EmptyRead::DiffFraction(0.0)),
                Ok(EmptyRead::DiffFraction(0.0)),
            ],
        );

        let decisions = run_until_stop(&mut tool);

        // Advance from N to G places the G button ...
        assert!(
            decisions.contains(&Decision::Place(Point::new(20, 20))),
            "the next grade is selected: {decisions:?}"
        );
        // ... and the second empty result on the LAST grade ends the run with the domain's line.
        match decisions.last() {
            Some(Decision::Stop(reason)) => {
                assert!(
                    reason.message.contains("All grades done (last was G)"),
                    "{}",
                    reason.message
                );
            }
            other => panic!("expected the done stop, got {other:?}"),
        }
    }

    #[test]
    fn a_composition_read_without_the_crop_can_still_be_empty() {
        // No saved crop: the reader answers with the colour composition, and the domain compares it
        // with the sampled empty signature. A matching composition is empty, so the grade advances.
        let config = arduino_config("advance_grade", 1);
        let signature_like = ColorComposition {
            mean_v: 0.3,
            mean_sat: 0.1,
            colored_fraction: 0.05,
            mean_r: 0.3,
            mean_g: 0.3,
            mean_b: 0.3,
        };
        let mut tool = tool_with(
            &config,
            vec![
                Ok(EmptyRead::Composition(signature_like)),
                Ok(EmptyRead::Composition(signature_like)),
            ],
        );

        let decisions = run_until_stop(&mut tool);

        assert!(
            decisions.contains(&Decision::Place(Point::new(20, 20))),
            "the fallback read advanced the grade: {decisions:?}"
        );
        match decisions.last() {
            Some(Decision::Stop(reason)) => {
                assert!(
                    reason.message.contains("All grades done"),
                    "{}",
                    reason.message
                );
            }
            other => panic!("expected the done stop, got {other:?}"),
        }
    }

    #[test]
    fn a_non_empty_read_takes_the_normal_deregister_register_pair_and_counts_the_cycle() {
        let config = arduino_config("advance_grade", 2);
        let mut tool = tool_with(
            &config,
            vec![
                Ok(EmptyRead::DiffFraction(0.9)),
                Ok(EmptyRead::DiffFraction(0.9)),
            ],
        );

        let decisions = run_until_stop(&mut tool);

        // Three cycles all on N; the third read errors ("no more reads") and stops the run.
        match decisions.last() {
            Some(Decision::Stop(reason)) => assert_eq!(reason.message, "no more reads"),
            other => panic!("expected the read failure, got {other:?}"),
        }
        assert_eq!(tool.run.cycle(), 3);
        let rows = tool.status().rows;
        assert_eq!(rows[0].name, "N");
        assert_eq!(rows[0].status, "3 cycles");
        assert_eq!(rows[1].status, "0 cycles");
    }

    #[test]
    fn a_read_failure_stops_with_its_line() {
        let config = arduino_config("advance_grade", 2);
        let mut tool = tool_with(
            &config,
            vec![Err(
                "capture refused because another window is in front".to_owned()
            )],
        );

        let decisions = run_until_stop(&mut tool);

        match decisions.last() {
            Some(Decision::Stop(reason)) => {
                assert_eq!(
                    reason.message,
                    "capture refused because another window is in front"
                );
            }
            other => panic!("expected the source line, got {other:?}"),
        }
    }

    #[test]
    fn advance_grade_by_hand_selects_the_next_grade_without_counting_a_cycle() {
        let config = arduino_config("stop", 2);
        let mut tool = tool_with(&config, Vec::new());
        for _ in 0..8 {
            tool.decide(); // the opening turn
        }

        tool.advance_grade_by_hand()
            .expect("the domain allows the move");

        assert_eq!(tool.decide(), Decision::Place(Point::new(20, 20)));
        assert_eq!(tool.run.cycle(), 0, "a manual advance is not a cycle");
        assert!(
            tool.status().message.contains('G'),
            "{}",
            tool.status().message
        );
    }

    #[test]
    fn the_status_rows_are_one_row_per_grade_in_run_order() {
        let config = arduino_config("stop", 2);
        let tool = tool_with(&config, Vec::new());

        let status = tool.status();

        assert_eq!(status.rows.len(), 2);
        assert_eq!(status.rows[0].name, "N");
        assert_eq!(status.rows[1].name, "G");
        assert!(status.message.contains("Grade N"), "{}", status.message);
    }

    #[test]
    fn no_reference_never_reads_as_empty() {
        let reference = None;
        let read = empty_read_from_frame(reference, None, 40, 4, 4, &[0u8; 64]);

        assert_eq!(read, EmptyRead::NoReference);
        assert!(!result_box_is_empty(&read, None, 0.18));
    }

    #[test]
    fn the_crop_comparison_ignores_the_six_pixel_border() {
        // A 16x16 crop with the same all-0 interior; the LIVE frame differs only in the outer rows.
        let reference = vec![0u8; 16 * 16 * 4];
        let mut live = vec![0u8; 16 * 16 * 4];
        for y in [0usize, 1, 2, 3, 4, 5, 10, 11, 12, 13, 14, 15] {
            for x in 0..16 {
                let offset = (y * 16 + x) * 4;
                live[offset] = 255;
            }
        }

        let fraction = diff_fraction(&live, 16, 16, &reference).expect("same size");

        assert_eq!(
            fraction, 0.0,
            "every difference is inside the excluded border"
        );
    }

    #[test]
    fn an_interior_difference_is_the_fraction_of_compared_pixels() {
        let reference = vec![0u8; 16 * 16 * 4];
        let mut live = vec![0u8; 16 * 16 * 4];
        // One interior pixel differs by more than the 30/channel tolerance.
        live[(8 * 16 + 8) * 4] = 200;
        let compared = (16 - 2 * EMPTY_COMPARE_INSET).pow(2) as f64;

        let fraction = diff_fraction(&live, 16, 16, &reference).expect("same size");

        assert!((fraction - 1.0 / compared).abs() < 1e-12, "{fraction}");
    }

    #[test]
    fn mismatched_crop_sizes_fall_back_to_the_colour_composition() {
        let reference = ReferenceCrop {
            width: 8,
            height: 8,
            rgba: vec![0u8; 8 * 8 * 4],
        };
        let signature = ColorComposition::default();
        let frame = vec![255u8; 4 * 4 * 4];

        let read = empty_read_from_frame(Some(&reference), Some(&signature), 40, 4, 4, &frame);

        assert!(matches!(read, EmptyRead::Composition(_)), "{read:?}");
    }

    #[test]
    fn the_colour_composition_is_the_analyzers_six_features() {
        // A 2x2 solid red box: brightness 1, full saturation, every pixel coloured.
        let red = [255u8, 0, 0, 255].repeat(4);

        let composition = analyze_composition(&red, 2, 2, 40);

        assert!((composition.mean_v - 1.0).abs() < 1e-9);
        assert!((composition.mean_sat - 1.0).abs() < 1e-9);
        assert!((composition.colored_fraction - 1.0).abs() < 1e-9);
        assert!((composition.mean_r - 1.0).abs() < 1e-9);
        assert_eq!(composition.mean_g, 0.0);
        assert_eq!(composition.mean_b, 0.0);
    }

    #[test]
    fn a_neutral_box_is_not_coloured() {
        let gray = [128u8, 128, 128, 255].repeat(4);

        let composition = analyze_composition(&gray, 2, 2, 40);

        assert!((composition.mean_sat - 0.0).abs() < 1e-9);
        assert_eq!(composition.colored_fraction, 0.0);
        assert!((composition.mean_r - composition.mean_g).abs() < 1e-9);
        assert_eq!(composition.mean_r, composition.mean_g);
        assert_eq!(composition.mean_g, composition.mean_b);
    }
}

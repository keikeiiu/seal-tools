//! The assembled OCR pipeline: det → post-process → crops → rec → lines.
//!
//! Everything text-related stays in `seal-domain`: this module only calls `TextCleaner`. The image
//! plumbing is pure (no Win32): callers hand in RGBA8 and get line results; [`crate::model`] owns
//! the sessions. Line bucketing is v2's `BuildLines` (`v2/SealTools.Core/OcrEngine.cs:220-240`,
//! called with `Math.Max(8, _cfg.Tuner.Ocr.RowHeight)`): `rowKey = y / rowHeight`, items in a row
//! concatenated left-to-right — NOT the 10-px `detgeom::sort_boxes`, which is for already-detected
//! boxes.
//!
//! Constants are the C# PP-OCRv4 defaults from `v3/docs/records/OCR-RESEARCH.md` §3: det limit side
//! 736 (`limit_type=min`), dims rounded up to a multiple of 32, mean/std 0.485/0.456/0.406,
//! `thresh = 0.3`, a 2×2 dilate, `box_thresh = 0.6`, `unclip_ratio = 1.5`, crop height 48.
//!
//! Known approximation (reported): the crop uses the unclipped MIN-RECT quad rather than Clipper's
//! offset polygon as the perspective source; the offset is still computed and used as the box
//! geometry/score gate. The real-capture tests are what settle whether that difference matters.
//!
//! # Model directory resolution
//!
//! [`resolve_models_dir`] is the ONE resolution path (`CUTOVER.md` packaging finding 6):
//! `SEAL_OCR_MODELS` when set (wins or fails; it never falls through), then `<config root>/models`,
//! then `<the seal-core.exe directory>/models`. A candidate counts only when all three
//! [`MODEL_FILES`] are present; the failure line lists every attempt and becomes the player-readable
//! `ocr_unavailable` detail.

use std::path::{Path, PathBuf};

use seal_domain::config::AttributesConfig;
use seal_domain::ocr::TextCleaner;

use crate::det::DetModel;
use crate::detgeom::{DetBox, Pt, Quad, box_score, contour_area, min_area_rect, unclip_quad};
use crate::detmask::{ScoreMap, dilate, find_contours, threshold};
use crate::imageops::{Rgb, det_input, perspective_crop, rec_input, resize_linear};
use crate::model::{Decoded, OcrError, RecModel};

/// The detector's input side, as the C# library configures it.
const DET_LIMIT_SIDE: f64 = 736.0;
const DET_PAD_MULTIPLE: usize = 32;
const DET_THRESH: f32 = 0.3;
const BOX_THRESH: f64 = 0.6;
const UNCLIP_RATIO: f64 = 1.5;
const REC_HEIGHT: usize = 48;

/// Why a pipeline could not be built. `reason` is the protocol `ocr_unavailable` detail value
/// (`PROTOCOL.md` §6); the message is for the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineError {
    pub reason: &'static str,
    pub message: String,
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.reason, self.message)
    }
}

impl std::error::Error for PipelineError {}

/// One recognised text item before row bucketing.
#[derive(Debug, Clone, PartialEq)]
pub struct RecognizedLine {
    pub text: String,
    pub confidence: f32,
    /// The detected box in the ORIGINAL image's coordinates.
    pub box_quad: Quad,
}

/// One reconstructed OCR line (v2 `BuildLines`' `(y, minX, maxX, text, conf)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub confidence: f32,
    pub box_quad: Quad,
}

/// The scan result: raw items plus the bucketed lines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scan {
    pub items: Vec<RecognizedLine>,
    pub lines: Vec<Line>,
}

/// The pipeline owns the two sessions and the cleaner; constructed once and cached by the core.
pub struct OcrPipeline {
    det: DetModel,
    rec: RecModel,
    cleaner: TextCleaner,
    row_height: u32,
}

/// The three model files the pipeline loads. A candidate directory is usable only when ALL THREE are
/// present: a directory with two of three is a broken install, not a fallback.
pub const MODEL_FILES: [&str; 3] = [
    "ch_PP-OCRv4_det_infer.onnx",
    "ch_PP-OCRv4_rec_infer.onnx",
    "ch_ppocr_mobile_v2.0_cls_infer.onnx",
];

fn missing_models(dir: &Path) -> Vec<&'static str> {
    MODEL_FILES
        .iter()
        .copied()
        .filter(|name| !dir.join(name).is_file())
        .collect()
}

/// The one models-directory resolver (`CUTOVER.md` packaging finding 6), in this fixed order:
///
/// 1. `SEAL_OCR_MODELS` when set — it WINS or fails; an explicit env var is never silently ignored
///    in favour of a fallback (a dev pointing it somewhere wrong must learn);
/// 2. `<config root>/models`;
/// 3. `<the running seal-core.exe directory>/models`.
///
/// A candidate counts only when all three [`MODEL_FILES`] exist in it. When no candidate works, the
/// error names every directory attempted (and why) because the line is the player-readable
/// `ocr_unavailable` detail. `env`, `root` and `exe_dir` are injected so the resolver is a pure
/// function over paths and can be unit-tested without a machine.
pub fn resolve_models_dir(
    env: Option<&Path>,
    root: Option<&Path>,
    exe_dir: Option<&Path>,
) -> Result<PathBuf, PipelineError> {
    if let Some(dir) = env {
        if !dir.is_dir() {
            return Err(PipelineError {
                reason: "models_dir_missing",
                message: format!("SEAL_OCR_MODELS is not a directory: {}", dir.display()),
            });
        }
        let missing = missing_models(dir);
        if !missing.is_empty() {
            return Err(PipelineError {
                reason: "models_dir_missing",
                message: format!(
                    "SEAL_OCR_MODELS is set to {} but it is missing: {}; a set SEAL_OCR_MODELS is \
                     never ignored",
                    dir.display(),
                    missing.join(", ")
                ),
            });
        }
        return Ok(dir.to_path_buf());
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(root) = root {
        candidates.push(root.join("models"));
    }
    if let Some(exe_dir) = exe_dir {
        let candidate = exe_dir.join("models");
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    let mut attempted: Vec<String> = Vec::new();
    for directory in &candidates {
        if !directory.is_dir() {
            attempted.push(format!("{} (no such directory)", directory.display()));
            continue;
        }
        let missing = missing_models(directory);
        if missing.is_empty() {
            return Ok(directory.clone());
        }
        attempted.push(format!(
            "{} (missing {})",
            directory.display(),
            missing.join(", ")
        ));
    }
    let tried = if attempted.is_empty() {
        "none (no config root or exe directory was available)".to_owned()
    } else {
        attempted.join("; ")
    };
    Err(PipelineError {
        reason: "models_env_unset",
        message: format!(
            "SEAL_OCR_MODELS is not set and no models directory was found. Tried: {tried}. Place the \
             three .onnx files in <root>/models or set SEAL_OCR_MODELS."
        ),
    })
}

impl OcrPipeline {
    /// Loads det+rec from `models_dir` and builds the cleaner from the dictionary's `text_fixes`.
    pub fn load(
        models_dir: &Path,
        attributes: &AttributesConfig,
        row_height: u32,
    ) -> Result<Self, PipelineError> {
        let det = DetModel::load(&models_dir.join("ch_PP-OCRv4_det_infer.onnx"), 1)
            .map_err(|error| load_error("det", error))?;
        let rec = RecModel::load(&models_dir.join("ch_PP-OCRv4_rec_infer.onnx"), 1)
            .map_err(|error| load_error("rec", error))?;
        let cleaner = TextCleaner::new(&attributes.text_fixes).map_err(|error| PipelineError {
            reason: "text_fixes_invalid",
            message: error.message(),
        })?;
        Ok(Self {
            det,
            rec,
            cleaner,
            row_height: row_height.max(8),
        })
    }

    /// The one entry point the core uses: resolve the models directory for this install (env, then
    /// `<root>/models`, then beside the exe) and load the pipeline. [`Self::load`] remains the
    /// low-level "open this directory" step used by tests and by [`Self::load_for_root`].
    pub fn load_for_root(
        root: &Path,
        attributes: &AttributesConfig,
        row_height: u32,
    ) -> Result<Self, PipelineError> {
        let env = std::env::var_os("SEAL_OCR_MODELS").map(PathBuf::from);
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf));
        let dir = resolve_models_dir(env.as_deref(), Some(root), exe_dir.as_deref())?;
        Self::load(&dir, attributes, row_height)
    }

    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// One full pass over an RGBA8 image. Alpha is ignored (the captures are opaque); an empty image
    /// is an empty scan, not an error.
    pub fn scan_rgba(
        &mut self,
        width: usize,
        height: usize,
        rgba: &[u8],
    ) -> Result<Scan, PipelineError> {
        if width == 0 || height == 0 || rgba.is_empty() {
            return Ok(Scan::default());
        }
        if rgba.len() < width * height * 4 {
            return Err(PipelineError {
                reason: "bad_image",
                message: format!(
                    "RGBA buffer is {} bytes, need {}",
                    rgba.len(),
                    width * height * 4
                ),
            });
        }
        let image = rgba_to_rgb(width, height, rgba);
        let items = self.detect_and_read(&image)?;
        let lines = build_lines(&items, self.row_height);
        Ok(Scan { items, lines })
    }

    fn detect_and_read(&mut self, image: &Rgb) -> Result<Vec<RecognizedLine>, PipelineError> {
        let (resized, scale_x, scale_y) = det_resize(image);
        let (shape, probs) = self.run_det(&resized)?;
        let (map_h, map_w) = match shape.as_slice() {
            [1, 1, h, w] => (*h as usize, *w as usize),
            _ => {
                return Err(PipelineError {
                    reason: "det_output_shape",
                    message: format!("det output is {shape:?}, expected [1,1,H,W]"),
                });
            }
        };
        if map_w == 0 || map_h == 0 || probs.len() < map_w * map_h {
            return Ok(Vec::new());
        }
        let score_map = ScoreMap::new(map_w, map_h, probs[..map_w * map_h].to_vec());
        let mask = dilate(&threshold(&score_map, DET_THRESH));

        let mut items = Vec::new();
        for contour in find_contours(&mask) {
            if contour.points.len() < 3 {
                continue;
            }
            let quad = min_area_rect(&contour.points);
            if contour_area(&quad_points(&quad)) < 1.0 {
                continue;
            }
            if box_score(&score_map, &quad) < BOX_THRESH {
                continue;
            }
            let Some(_offset) = unclip_quad(&quad, UNCLIP_RATIO) else {
                continue;
            };
            let mapped = Quad(quad.0.map(|point| Pt {
                x: (point.x * scale_x).clamp(0.0, image.width as f64 - 1.0),
                y: (point.y * scale_y).clamp(0.0, image.height as f64 - 1.0),
            }));
            let Some(item) = self.read_quad(image, &mapped)? else {
                continue;
            };
            items.push(item);
        }
        Ok(items)
    }

    fn run_det(&mut self, image: &Rgb) -> Result<(Vec<i64>, Vec<f32>), PipelineError> {
        let data = det_input(image);
        let shape = [1i64, 3, image.height as i64, image.width as i64];
        self.det
            .infer(&data, shape)
            .map_err(|error| runtime_error("det", error))
    }

    fn read_quad(
        &mut self,
        image: &Rgb,
        quad: &Quad,
    ) -> Result<Option<RecognizedLine>, PipelineError> {
        let width = quad_width(quad);
        let height = quad_height(quad);
        if width < 2.0 || height < 2.0 {
            return Ok(None);
        }
        let out_w = ((REC_HEIGHT as f64 * width / height).ceil() as usize).clamp(4, 4096);
        let crop = perspective_crop(image, quad, out_w, REC_HEIGHT);
        let input = rec_input(&crop);
        let shape = [1i64, 3, REC_HEIGHT as i64, input.width as i64];
        let output = self
            .rec
            .infer(&input.data, &shape)
            .map_err(|error| runtime_error("rec", error))?;
        let decoded: Decoded = self.rec.decode(&output);
        let text = self.cleaner.clean(&decoded.text);
        if text.is_empty() {
            return Ok(None);
        }
        Ok(Some(RecognizedLine {
            text,
            confidence: decoded.confidence,
            box_quad: *quad,
        }))
    }
}

fn load_error(which: &str, error: OcrError) -> PipelineError {
    PipelineError {
        reason: "model_load_failed",
        message: format!("{which} model: {error}"),
    }
}

fn runtime_error(which: &str, error: OcrError) -> PipelineError {
    PipelineError {
        reason: "runtime_failed",
        message: format!("{which}: {error}"),
    }
}

/// RGBA8 → packed RGB, alpha dropped (the client area is opaque; v2 also read BGR/colour only).
fn rgba_to_rgb(width: usize, height: usize, rgba: &[u8]) -> Rgb {
    let mut pixels = Vec::with_capacity(width * height * 3);
    let (pixels4, _) = rgba.as_chunks::<4>();
    for pixel in pixels4 {
        pixels.extend_from_slice(&pixel[..3]);
    }
    Rgb::new(width, height, pixels)
}

/// Resize the detector input so the SHORT side reaches 736, round dims up to a multiple of 32, and
/// return the x/y scale factors from detector pixels back to original pixels.
fn det_resize(image: &Rgb) -> (Rgb, f64, f64) {
    let short = image.width.min(image.height).max(1) as f64;
    let ratio = if short < DET_LIMIT_SIDE {
        DET_LIMIT_SIDE / short
    } else {
        1.0
    };
    let round_up = |value: usize| value.div_ceil(DET_PAD_MULTIPLE) * DET_PAD_MULTIPLE;
    let width = round_up((image.width as f64 * ratio).round() as usize).max(DET_PAD_MULTIPLE);
    let height = round_up((image.height as f64 * ratio).round() as usize).max(DET_PAD_MULTIPLE);
    let resized = resize_linear(image, width, height);
    let scale_x = image.width as f64 / width as f64;
    let scale_y = image.height as f64 / height as f64;
    (resized, scale_x, scale_y)
}

fn quad_points(quad: &Quad) -> Vec<(f64, f64)> {
    quad.0.iter().map(|point| (point.x, point.y)).collect()
}

fn quad_width(quad: &Quad) -> f64 {
    (distance(quad.0[0], quad.0[1]) + distance(quad.0[2], quad.0[3])) / 2.0
}

fn quad_height(quad: &Quad) -> f64 {
    (distance(quad.0[1], quad.0[2]) + distance(quad.0[3], quad.0[0])) / 2.0
}

fn distance(a: Pt, b: Pt) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// v2 `BuildLines` (`OcrEngine.cs:220-240`): bucket by `y / rowHeight`, order rows by key, order items
/// in a row by left edge, concatenate their text, average confidence. Items whose text became empty
/// after cleaning do not start a line.
pub fn build_lines(items: &[RecognizedLine], row_height: u32) -> Vec<Line> {
    use std::collections::BTreeMap;
    let row_height = row_height.max(1);
    let mut rows: BTreeMap<i64, Vec<&RecognizedLine>> = BTreeMap::new();
    for item in items {
        let y = item
            .box_quad
            .0
            .iter()
            .map(|point| point.y)
            .fold(f64::INFINITY, f64::min);
        rows.entry((y / row_height as f64).floor() as i64)
            .or_default()
            .push(item);
    }
    let mut lines = Vec::new();
    for (_, mut row) in rows {
        row.sort_by(|a, b| {
            left_x(a)
                .partial_cmp(&left_x(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let text = row
            .iter()
            .map(|item| item.text.as_str())
            .collect::<String>()
            .trim()
            .to_owned();
        if text.is_empty() {
            continue;
        }
        let confidence = row.iter().map(|item| item.confidence).sum::<f32>() / row.len() as f32;
        lines.push(Line {
            text,
            confidence,
            box_quad: row[0].box_quad,
        });
    }
    lines
}

fn left_x(item: &RecognizedLine) -> f64 {
    item.box_quad
        .0
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min)
}

/// `DetBox` convenience for the report/tests: the unclipped box filter path.
pub fn accepted_box(map: &ScoreMap, quad: &Quad) -> Option<DetBox> {
    crate::detgeom::accept_box(map, quad, BOX_THRESH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detgeom::Pt;

    fn quad(x: f64, y: f64) -> Quad {
        Quad([
            Pt { x, y },
            Pt { x: x + 10.0, y },
            Pt {
                x: x + 10.0,
                y: y + 10.0,
            },
            Pt { x, y: y + 10.0 },
        ])
    }

    fn item(text: &str, confidence: f32, x: f64, y: f64) -> RecognizedLine {
        RecognizedLine {
            text: text.to_owned(),
            confidence,
            box_quad: quad(x, y),
        }
    }

    #[test]
    fn build_lines_buckets_by_row_height_and_orders_left_to_right() {
        let items = vec![
            item("b", 0.9, 50.0, 2.0),
            item("a", 0.5, 10.0, 2.0),
            item("c", 0.7, 5.0, 25.0),
        ];

        let lines = build_lines(&items, 10);

        assert_eq!(lines.len(), 2, "two row buckets: y=2 and y=25");
        assert_eq!(lines[0].text, "ab");
        assert_eq!(lines[1].text, "c");
        assert!((lines[0].confidence - 0.7).abs() < 1e-6);
    }

    #[test]
    fn a_taller_row_height_merges_what_a_shorter_one_splits() {
        let items = vec![item("b", 0.9, 50.0, 2.0), item("a", 0.5, 10.0, 25.0)];

        assert_eq!(build_lines(&items, 10).len(), 2);
        let merged = build_lines(&items, 40);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "ab");
    }

    #[test]
    fn empty_text_starts_no_line() {
        let items = vec![item("", 0.9, 0.0, 0.0), item("x", 0.9, 0.0, 0.0)];
        let lines = build_lines(&items, 10);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "x");
    }

    #[test]
    fn rgba_to_rgb_drops_alpha() {
        let rgba = [1u8, 2, 3, 255, 4, 5, 6, 128];
        let rgb = rgba_to_rgb(2, 1, &rgba);
        assert_eq!((rgb.width, rgb.height), (2, 1));
        assert_eq!(rgb.pixels, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn det_resize_rounds_to_multiples_of_32_and_keeps_the_scale() {
        let image = crate::imageops::Rgb::new(100, 50, vec![0; 100 * 50 * 3]);
        let (resized, scale_x, scale_y) = det_resize(&image);

        assert_eq!(resized.width % 32, 0, "width is padded to a 32 multiple");
        assert_eq!(resized.height % 32, 0, "height is padded to a 32 multiple");
        assert!(resized.height >= 736, "the short side reaches 736");
        assert!((scale_x - 100.0 / resized.width as f64).abs() < 1e-9);
        assert!((scale_y - 50.0 / resized.height as f64).abs() < 1e-9);
    }

    // The model-directory resolution tests live in `tests/models_resolution.rs`, as pure functions
    // over injected paths (no process environment, no machine).
}

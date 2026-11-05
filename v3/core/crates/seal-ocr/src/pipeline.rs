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
//! Known deviation from `OCR-RESEARCH` §3: the reference crops each box from Clipper's offset polygon,
//! and so does this pipeline — the crop quad is the min-area rectangle of the UNCLIPPED polygon, not
//! the raw (shrunk) DB kernel (OCR-F1). The real-capture tests are what settle whether the remaining
//! integer-pixel difference matters.
//!
//! # Model directory resolution
//!
//! [`resolve_models_dir`] is the ONE resolution path (`CUTOVER.md` packaging finding 6):
//! `SEAL_OCR_MODELS` when set (wins or fails; it never falls through), then `<config root>/models`,
//! then `<the seal-core.exe directory>/models`. A candidate counts only when both
//! [`REQUIRED_MODELS`] are present (the classifier file is optional, OCR-F10); the failure line lists
//! every attempt and becomes the player-readable
//! `ocr_unavailable` detail.

use std::path::{Path, PathBuf};

use seal_domain::config::AttributesConfig;
use seal_domain::ocr::TextCleaner;

use crate::det::DetModel;
use crate::detgeom::{DetBox, Pt, Quad, contour_crop_quad, quad_height, quad_width, sort_quads};
use crate::detmask::{ScoreMap, dilate, find_contours, threshold};
use crate::imageops::{
    Rgb, det_input, perspective_crop, rec_input, resize_linear, upscale_rgba_cubic,
};
use crate::model::{Decoded, OcrError, RecModel};

/// The detector's input side, as the C# library configures it.
const DET_LIMIT_SIDE: f64 = 736.0;
/// `ResizeImageWithinBounds`: the bounds the reference keeps around the limit-side step.
const DET_MIN_SIDE: f64 = 30.0;
const DET_MAX_SIDE: f64 = 2000.0;
/// `ApplyVerticalPadding`: `h <= 30` or `w / h > 8` triggers padding to `2 * max(w / 8, 30)` rows.
const VERTICAL_PAD_MAX_HEIGHT: usize = 30;
const VERTICAL_PAD_MIN_HEIGHT: usize = 30;
const VERTICAL_PAD_MAX_ASPECT: f64 = 8.0;
const DET_PAD_MULTIPLE: usize = 32;
const DET_THRESH: f32 = 0.3;
const BOX_THRESH: f64 = 0.6;
const UNCLIP_RATIO: f64 = 1.5;
const REC_HEIGHT: usize = 48;
/// The reference `max_candidates` (PP-OCRv4 default set): at most this many contours are scored per
/// map. The 2×2 dilation leaves one contour per text blob; anything beyond a thousand candidates is a
/// noisy score map, and scoring all of them was the O(map × contours) blow-up of OCR-F2.
const MAX_CANDIDATES: usize = 1000;

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

/// The model files the pipeline loads. A candidate directory is usable only when BOTH are present: a
/// directory with one is a broken install, not a fallback.
pub const REQUIRED_MODELS: [&str; 2] = ["ch_PP-OCRv4_det_infer.onnx", "ch_PP-OCRv4_rec_infer.onnx"];

/// The orientation classifier file: OPTIONAL. Nothing loads it — this pipeline has no classifier
/// stage and game UI text is never upside down — so a release that ships it beside the other two is
/// fine, but a directory without it is a working install, not `ocr_unavailable` (OCR-F10).
pub const OPTIONAL_MODELS: [&str; 1] = ["ch_ppocr_mobile_v2.0_cls_infer.onnx"];

fn missing_models(dir: &Path) -> Vec<&'static str> {
    REQUIRED_MODELS
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
/// A candidate counts only when both [`REQUIRED_MODELS`] exist in it. When no candidate works, the
/// error names every directory attempted (and why) because the line is the player-readable
/// `ocr_unavailable` detail. `env`, `root` and `exe_dir` are injected so the resolver is a pure
/// function over paths and can be unit-tested without a machine.
///
/// # Errors
///
/// Returns a [`PipelineError`] when the env var is set but unusable (it is never silently ignored)
/// or when no candidate directory holds both [`REQUIRED_MODELS`]; the message names every directory
/// tried and why.
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
             two .onnx files (det + rec) in <root>/models or set SEAL_OCR_MODELS."
        ),
    })
}

impl OcrPipeline {
    /// Loads det+rec from `models_dir` and builds the cleaner from the dictionary's `text_fixes`.
    ///
    /// # Errors
    ///
    /// Returns a [`PipelineError`] when a model fails to load or the dictionary's `text_fixes` are
    /// refused.
    pub fn load(
        models_dir: &Path,
        attributes: &AttributesConfig,
        row_height: u32,
    ) -> Result<Self, PipelineError> {
        let det = DetModel::load(&models_dir.join("ch_PP-OCRv4_det_infer.onnx"), 1)
            .map_err(|error| load_error("det", &error))?;
        let rec = RecModel::load(&models_dir.join("ch_PP-OCRv4_rec_infer.onnx"), 1)
            .map_err(|error| load_error("rec", &error))?;
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
    ///
    /// # Errors
    ///
    /// Returns the resolver's error when no models directory is found, otherwise [`Self::load`]'s.
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

    #[must_use]
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Whether both pipelines use the same loaded det and rec sessions. The core builds three (the
    /// dispatcher's, the tuner's and the pet's), and they must share one ~25 MB model set rather than
    /// each loading a copy (OCR-F9).
    #[must_use]
    pub fn shares_models_with(&self, other: &Self) -> bool {
        self.det.shares_with(&other.det) && self.rec.shares_with(&other.rec)
    }

    /// One full pass over an RGBA8 image. Alpha is ignored (the captures are opaque); an empty image
    /// is an empty scan, not an error.
    ///
    /// # Errors
    ///
    /// Returns a [`PipelineError`] when the RGBA buffer is shorter than `width * height * 4`, or when
    /// a detection or recognition inference fails.
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
            return Err(bad_image(width, height, rgba.len()));
        }
        let image = rgba_to_rgb(width, height, rgba);
        self.scan_rgb(&image)
    }

    /// One full pass over an RGBA8 crop that is first upscaled by `factor` with CUBIC interpolation.
    ///
    /// v2's count ladder did exactly that (`InterpolationFlags.Cubic` at 3×/9×/24×); nearest
    /// neighbour fed the recogniser blocky glyphs it was never trained on (OCR-F12). A factor of 1 is
    /// plain [`Self::scan_rgba`].
    ///
    /// # Errors
    ///
    /// Returns a [`PipelineError`] when the RGBA buffer is shorter than `width * height * 4`, or when
    /// a detection or recognition inference fails.
    pub fn scan_rgba_upscaled(
        &mut self,
        width: usize,
        height: usize,
        rgba: &[u8],
        factor: u32,
    ) -> Result<Scan, PipelineError> {
        let factor = factor.max(1);
        if factor == 1 || width == 0 || height == 0 || rgba.is_empty() {
            return self.scan_rgba(width, height, rgba);
        }
        let (scaled_width, scaled_height, scaled) = upscale_rgba_cubic(width, height, rgba, factor);
        if scaled.is_empty() {
            return Err(bad_image(width, height, rgba.len()));
        }
        let image = rgba_to_rgb(scaled_width, scaled_height, &scaled);
        self.scan_rgb(&image)
    }

    fn scan_rgb(&mut self, image: &Rgb) -> Result<Scan, PipelineError> {
        if image.width == 0 || image.height == 0 {
            return Ok(Scan::default());
        }
        let items = self.detect_and_read(image)?;
        let lines = build_lines(&items, self.row_height);
        Ok(Scan { items, lines })
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "the clamp bounds are the capture's own pixel dimensions, exactly representable in f64"
    )]
    fn detect_and_read(&mut self, image: &Rgb) -> Result<Vec<RecognizedLine>, PipelineError> {
        let det_input = det_preprocess(image);
        let (shape, probs) = self.run_det(&det_input.image)?;
        let Some((map_h, map_w)) = det_map_shape(&shape) else {
            return Err(PipelineError {
                reason: "det_output_shape",
                message: format!("det output is {shape:?}, expected [1,1,H,W]"),
            });
        };
        if map_w == 0 || map_h == 0 || probs.len() < map_w * map_h {
            return Ok(Vec::new());
        }
        let score_map = ScoreMap::new(map_w, map_h, probs[..map_w * map_h].to_vec());
        let mask = dilate(&threshold(&score_map, DET_THRESH));

        let mut items = Vec::new();
        for contour in find_contours(&mask).into_iter().take(MAX_CANDIDATES) {
            // The crop comes from the UNCLIPPED box, not the shrunk DB kernel: the min rect alone
            // cuts ascenders/descenders (OCR-F1). `contour_crop_quad` also owns the min-size,
            // box-score and unclip filters, in the reference order.
            let Some(quad) =
                contour_crop_quad(&score_map, &contour.points, BOX_THRESH, UNCLIP_RATIO)
            else {
                continue;
            };
            let mapped = Quad(quad.0.map(|point| Pt {
                x: (point.x * det_input.scale_x).clamp(0.0, image.width as f64 - 1.0),
                y: (point.y * det_input.scale_y).clamp(0.0, image.height as f64 - 1.0),
            }));
            let Some(item) = self.read_quad(image, &mapped)? else {
                continue;
            };
            items.push(item);
        }
        // `SortedBoxes` over the recognized items: `find_contours` emits raster order, which is not
        // left-to-right within a row, and the grade/attribute readers concatenate `items` directly
        // (OCR-F5). Sorting here also makes `scan.items` deterministic for every caller.
        let mut ordered: Vec<(Quad, RecognizedLine)> = items
            .into_iter()
            .map(|item| (item.box_quad, item))
            .collect();
        sort_quads(&mut ordered);
        Ok(ordered.into_iter().map(|(_, item)| item).collect())
    }

    #[allow(
        clippy::cast_possible_wrap,
        reason = "the detector input dimensions are the capture's pixel size, far below i64::MAX"
    )]
    fn run_det(&mut self, image: &Rgb) -> Result<(Vec<i64>, Vec<f32>), PipelineError> {
        let data = det_input(image);
        let shape = [1i64, 3, image.height as i64, image.width as i64];
        self.det
            .infer(&data, shape)
            .map_err(|error| runtime_error("det", &error))
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
        reason = "REC_HEIGHT is 48 and the tensor width is the crop's positive aspect ratio times it, clamped to 4..=4096; the capture dimensions fit i64"
    )]
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
        self.read_crop(&crop, *quad)
    }

    /// Recognise one already-cropped line: resize to the recogniser height, run rec, decode, clean.
    /// [`Self::read_quad`] adds the perspective crop in front of this.
    #[allow(
        clippy::cast_possible_wrap,
        reason = "REC_HEIGHT is 48 and the tensor width is the crop's width, far below i64::MAX"
    )]
    fn read_crop(
        &mut self,
        crop: &Rgb,
        quad: Quad,
    ) -> Result<Option<RecognizedLine>, PipelineError> {
        if crop.width < 2 || crop.height < 2 {
            return Ok(None);
        }
        let input = rec_input(crop);
        let shape = [1i64, 3, REC_HEIGHT as i64, input.width as i64];
        let output = self
            .rec
            .infer(&input.data, &shape)
            .map_err(|error| runtime_error("rec", &error))?;
        let decoded: Decoded = self.rec.decode(&output);
        let text = self.cleaner.clean(&decoded.text);
        if text.is_empty() {
            return Ok(None);
        }
        Ok(Some(RecognizedLine {
            text,
            confidence: decoded.confidence,
            box_quad: quad,
        }))
    }

    /// One FIXED-LAYOUT line read: no detection, no contours, no crops — the calibrated region IS the
    /// line (`PaddleOCR`'s `det=False` path). Call it for count, remaining, grade and eta boxes whose
    /// geometry is already known; keep [`Self::scan_rgba`] for free-form regions (tooltips, attribute
    /// blocks), which is where OCR-F14's det cost still belongs.
    ///
    /// The returned scan has at most one item, whose quad is the whole region, and the row-bucketed
    /// line the readers already expect.
    ///
    /// # Errors
    ///
    /// Returns a [`PipelineError`] when the RGBA buffer is shorter than `width * height * 4`, or when
    /// the recognition inference fails.
    #[allow(
        clippy::cast_precision_loss,
        reason = "the region quad is the capture's own pixel grid, whose dimensions are exactly representable in f64"
    )]
    pub fn read_line_rgba(
        &mut self,
        width: usize,
        height: usize,
        rgba: &[u8],
    ) -> Result<Scan, PipelineError> {
        if width < 2 || height < 2 || rgba.is_empty() {
            return Ok(Scan::default());
        }
        if rgba.len() < width * height * 4 {
            return Err(bad_image(width, height, rgba.len()));
        }
        let image = rgba_to_rgb(width, height, rgba);
        let quad = Quad([
            Pt { x: 0.0, y: 0.0 },
            Pt {
                x: width as f64,
                y: 0.0,
            },
            Pt {
                x: width as f64,
                y: height as f64,
            },
            Pt {
                x: 0.0,
                y: height as f64,
            },
        ]);
        let items: Vec<RecognizedLine> = self.read_crop(&image, quad)?.into_iter().collect();
        let lines = build_lines(&items, self.row_height);
        Ok(Scan { items, lines })
    }
}

fn load_error(which: &str, error: &OcrError) -> PipelineError {
    PipelineError {
        reason: "model_load_failed",
        message: format!("{which} model: {error}"),
    }
}

fn runtime_error(which: &str, error: &OcrError) -> PipelineError {
    PipelineError {
        reason: "runtime_failed",
        message: format!("{which}: {error}"),
    }
}

fn bad_image(width: usize, height: usize, len: usize) -> PipelineError {
    PipelineError {
        reason: "bad_image",
        message: format!("RGBA buffer is {len} bytes, need {}", width * height * 4),
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

/// The detector's input frame plus the det→original scale per axis.
#[derive(Debug, Clone, PartialEq)]
pub struct DetInput {
    pub image: Rgb,
    pub scale_x: f64,
    pub scale_y: f64,
}

/// The detector preprocessing, in the reference's order (OCR-F3):
///
/// 1. [`apply_vertical_padding`] — a short or very wide crop gets black rows before any resize;
/// 2. `ResizeImageWithinBounds` — the min side is raised to 30 px and the max side reduced to 2,000;
/// 3. the PP-OCRv4 limit `limit_type = min`, `limit_side_len = 736`: the SHORT side reaches 736;
/// 4. the 2,000 max side again, because step 3 can blow a wide strip out past it.
///
/// Dimensions are rounded up to a multiple of 32. The padding is top-anchored, so the original
/// image's pixel coordinates are unchanged in the padded frame and the scale stays per-axis.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the resize works on the capture's own pixel dimensions and a positive scale ratio, so the rounded dimensions are positive and then rounded up to the 32 multiple"
)]
pub fn det_preprocess(image: &Rgb) -> DetInput {
    if image.width == 0 || image.height == 0 {
        return DetInput {
            image: Rgb::new(0, 0, Vec::new()),
            scale_x: 1.0,
            scale_y: 1.0,
        };
    }
    let padded = apply_vertical_padding(image);
    let width = padded.width as f64;
    let height = padded.height as f64;
    let ratio = det_ratio(width, height);
    let round_up = |value: f64| {
        ((value.round() as usize).div_ceil(DET_PAD_MULTIPLE) * DET_PAD_MULTIPLE)
            .max(DET_PAD_MULTIPLE)
    };
    let new_width = round_up(width * ratio);
    let new_height = round_up(height * ratio);
    let resized = resize_linear(&padded, new_width, new_height);
    DetInput {
        image: resized,
        scale_x: width / new_width as f64,
        scale_y: height / new_height as f64,
    }
}

/// The one uniform scale from the padded image to the detector input, as the reference applies its
/// bounds in order (OCR-F3). Separated so the arithmetic is unit-tested without pixels.
fn det_ratio(width: f64, height: f64) -> f64 {
    let short = width.min(height);
    let long = width.max(height);
    // `ResizeImageWithinBounds`: min side 30, max side 2,000.
    let mut ratio = if short < DET_MIN_SIDE {
        DET_MIN_SIDE / short
    } else {
        1.0
    };
    if long * ratio > DET_MAX_SIDE {
        ratio = DET_MAX_SIDE / long;
    }
    // `limit_type = min`, `limit_side_len = 736`: the short side must reach 736.
    if short * ratio < DET_LIMIT_SIDE {
        ratio = DET_LIMIT_SIDE / short;
    }
    // The 736 upscale can push a wide strip back over the max side; the bound is the hard one.
    if long * ratio > DET_MAX_SIDE {
        ratio = DET_MAX_SIDE / long;
    }
    ratio
}

/// `DetPreprocess.ApplyVerticalPadding`: pad to `2 * max(w / 8, 30)` rows with BLACK when `h <= 30`
/// or `w / h > 8`. The content keeps the top of the padded image, so its own coordinates do not move;
/// only the resize scale changes (OCR-F3). A crop that needs no padding is returned unchanged.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "the aspect test divides the capture's own pixel dimensions, which are exactly representable in f64"
)]
pub fn apply_vertical_padding(image: &Rgb) -> Rgb {
    let (width, height) = (image.width, image.height);
    if width == 0 || height == 0 {
        return image.clone();
    }
    let too_wide = width as f64 / height as f64 > VERTICAL_PAD_MAX_ASPECT;
    if height > VERTICAL_PAD_MAX_HEIGHT && !too_wide {
        return image.clone();
    }
    let target = 2 * (width / 8).max(VERTICAL_PAD_MIN_HEIGHT);
    if target <= height {
        return image.clone();
    }
    let mut pixels = Vec::with_capacity(width * target * 3);
    pixels.extend_from_slice(&image.pixels);
    pixels.resize(width * target * 3, 0);
    Rgb::new(width, target, pixels)
}

/// The detector map's `(height, width)` from the model's `[1,1,H,W]` output shape; `None` for any
/// other shape or a dimension that is not a non-negative length.
fn det_map_shape(shape: &[i64]) -> Option<(usize, usize)> {
    match shape {
        [1, 1, height, width] => Some((
            usize::try_from(*height).ok()?,
            usize::try_from(*width).ok()?,
        )),
        _ => None,
    }
}

/// v2 `BuildLines` (`OcrEngine.cs:220-240`): bucket by `y / rowHeight`, order rows by key, order items
/// in a row by left edge, concatenate their text, average confidence. Items whose text became empty
/// after cleaning do not start a line.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "row keys are capture pixel rows (far below 2^52, so they convert to f64 and back exactly) and `row.len()` is the handful of items in one row"
)]
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
        rows.entry((y / f64::from(row_height)).floor() as i64)
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
#[must_use]
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
    #[allow(
        clippy::cast_precision_loss,
        reason = "the fixture's image dimensions are small integers, exactly representable in f64"
    )]
    fn det_preprocess_rounds_to_multiples_of_32_and_keeps_the_scale() {
        let image = crate::imageops::Rgb::new(100, 50, vec![0; 100 * 50 * 3]);
        let pre = det_preprocess(&image);

        assert_eq!(pre.image.width % 32, 0, "width is padded to a 32 multiple");
        assert_eq!(
            pre.image.height % 32,
            0,
            "height is padded to a 32 multiple"
        );
        assert!(pre.image.height >= 736, "the short side reaches 736");
        assert!((pre.scale_x - 100.0 / pre.image.width as f64).abs() < 1e-9);
        assert!((pre.scale_y - 50.0 / pre.image.height as f64).abs() < 1e-9);
    }

    #[test]
    fn a_tiny_region_stays_under_the_two_thousand_pixel_max_side() {
        // A 15x40 feeder count region used to become 736x1984 (the 736 short-side limit alone). The
        // reference's bounds keep the long side at or below 2,000 (OCR-F3).
        let image = crate::imageops::Rgb::new(15, 40, vec![0; 15 * 40 * 3]);

        let pre = det_preprocess(&image);

        assert!(pre.image.width <= 2000, "width {}", pre.image.width);
        assert!(pre.image.height <= 2000, "height {}", pre.image.height);
        assert_eq!(pre.image.width % 32, 0);
    }

    #[test]
    fn a_short_wide_strip_is_black_padded_before_the_resize() {
        // A 400x30 eta strip is padded to 400x100 with black rows BEFORE the resize: the 736 short
        // side then only needs a 5x scale, not 24.5x (OCR-F3).
        let strip = crate::imageops::Rgb::new(400, 30, vec![255; 400 * 30 * 3]);

        let padded = apply_vertical_padding(&strip);
        assert_eq!((padded.width, padded.height), (400, 100));
        assert_eq!(padded.pixel(0, 0), (255, 255, 255), "content top-anchored");
        assert_eq!(padded.pixel(399, 29), (255, 255, 255));
        assert_eq!(padded.pixel(0, 99), (0, 0, 0), "black padding below");

        let pre = det_preprocess(&strip);
        assert!(
            pre.image.height <= 600,
            "the padded 4:1 strip is used, not 400x30 blown up to height 736: {}",
            pre.image.height
        );
        assert!(
            pre.image.width <= 2032,
            "the 2,000 max side (rounded to 32) bounds the width: {}",
            pre.image.width
        );
    }

    #[test]
    fn a_normal_crop_is_not_vertically_padded() {
        let image = crate::imageops::Rgb::new(100, 50, vec![7; 100 * 50 * 3]);
        assert_eq!(apply_vertical_padding(&image), image);
    }

    #[test]
    fn a_wide_but_tall_enough_crop_is_padded_on_its_aspect_alone() {
        // h > 30 alone does not decide: w/h > 8 triggers the padding rule too.
        let image = crate::imageops::Rgb::new(400, 40, vec![1; 400 * 40 * 3]);
        let padded = apply_vertical_padding(&image);
        assert_eq!(padded.height, 100, "2 * (400 / 8)");
    }

    #[test]
    fn cubic_resize_interpolates_where_nearest_would_repeat_blocks() {
        // A 2x1 black/white strip upscaled 4x: nearest gives exactly two values (the repeated
        // blocks); cubic produces the intermediate greys the recogniser was trained on (OCR-F12).
        let source = crate::imageops::Rgb::new(2, 1, vec![0, 0, 0, 255, 255, 255]);

        let cubic = crate::imageops::resize_cubic(&source, 8, 4);

        let distinct: std::collections::BTreeSet<u8> =
            cubic.pixels.iter().step_by(3).copied().collect();
        assert!(
            distinct.len() > 2,
            "cubic interpolation produces intermediate values: {distinct:?}"
        );
        assert!(
            distinct.iter().any(|value| (1..255).contains(value)),
            "at least one interior grey: {distinct:?}"
        );
        assert_eq!(
            crate::imageops::resize_cubic(&source, 2, 1),
            source,
            "a same-size resize is a copy"
        );
    }

    #[test]
    fn cubic_rgba_upscale_is_smooth_and_opaque() {
        let rgba = [0u8, 0, 0, 255, 255, 255, 255, 255];

        let (width, height, scaled) = crate::imageops::upscale_rgba_cubic(2, 1, &rgba, 4);

        assert_eq!((width, height), (8, 4));
        assert_eq!(scaled.len(), width * height * 4);
        assert!(
            scaled
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 255),
            "the ladder output is opaque RGBA"
        );
        let distinct: std::collections::BTreeSet<u8> = scaled
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[0])
            .collect();
        assert!(distinct.len() > 2, "{distinct:?}");
    }

    // The model-directory resolution tests live in `tests/models_resolution.rs`, as pure functions
    // over injected paths (no process environment, no machine).
}

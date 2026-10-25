//! Image plumbing for the detector and recogniser inputs: perspective crop (cubic, replicate border)
//! and the recogniser's 48-px tensor normalisation.
//!
//! Deliberately dependency-free: a tiny RGB buffer, a bilinear resize, the cubic warp, and the
//! normalisation. No drawing/`OcrDrawer` code — that insight is debug-only per the research.

use crate::detgeom::{Pt, Quad};

/// A packed RGB8 image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgb {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Rgb {
    pub fn new(width: usize, height: usize, pixels: Vec<u8>) -> Self {
        assert_eq!(pixels.len(), width * height * 3, "RGB buffer size");
        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let index = (y * self.width + x) * 3;
        (
            self.pixels[index],
            self.pixels[index + 1],
            self.pixels[index + 2],
        )
    }

    fn set_pixel(&mut self, x: usize, y: usize, value: (u8, u8, u8)) {
        let index = (y * self.width + x) * 3;
        self.pixels[index] = value.0;
        self.pixels[index + 1] = value.1;
        self.pixels[index + 2] = value.2;
    }

    /// Replicate-border fetch by integer pixel index.
    fn pixel_replicate(&self, x: i32, y: i32) -> (u8, u8, u8) {
        let x = x.clamp(0, self.width as i32 - 1) as usize;
        let y = y.clamp(0, self.height as i32 - 1) as usize;
        self.pixel(x, y)
    }
}

/// Catmull-Rom-style cubic kernel with OpenCV's `a = -0.75`.
fn cubic_kernel(x: f64) -> f64 {
    const A: f64 = -0.75;
    let x = x.abs();
    if x <= 1.0 {
        (A + 2.0) * x * x * x - (A + 3.0) * x * x + 1.0
    } else if x < 2.0 {
        A * x * x * x - 5.0 * A * x * x + 8.0 * A * x - 4.0 * A
    } else {
        0.0
    }
}

/// `Cv2.WarpPerspective(..., InterpolationFlags.Cubic, BorderTypes.Replicate)`: map the output pixel
/// grid onto the quad. The quad's corners are pixel-CORNER coordinates (so an identity quad that
/// covers `[0,width]x[0,height]` is a copy).
pub fn perspective_crop(source: &Rgb, quad: &Quad, out_w: usize, out_h: usize) -> Rgb {
    let homography = homography_from_quad(quad, out_w, out_h);
    let mut output = Rgb::new(out_w, out_h, vec![0u8; out_w * out_h * 3]);
    for y in 0..out_h {
        for x in 0..out_w {
            let (input_x, input_y) = apply_homography(&homography, x as f64 + 0.5, y as f64 + 0.5);
            let sample_x = input_x - 0.5;
            let sample_y = input_y - 0.5;
            // Out-of-range samples are replicated, matching BorderTypes.Replicate.
            let clamped_x = sample_x.clamp(0.0, source.width as f64 - 1.0);
            let clamped_y = sample_y.clamp(0.0, source.height as f64 - 1.0);
            let base_x = clamped_x.floor() as i32;
            let base_y = clamped_y.floor() as i32;
            let tx = clamped_x - base_x as f64;
            let ty = clamped_y - base_y as f64;
            let mut sums = [0.0f64; 3];
            for row in -1..=2 {
                let wy = cubic_kernel(row as f64 - ty);
                for col in -1..=2 {
                    let wx = cubic_kernel(col as f64 - tx);
                    let weight = wx * wy;
                    if weight == 0.0 {
                        continue;
                    }
                    let (r, g, b) = source.pixel_replicate(base_x + col, base_y + row);
                    sums[0] += weight * f64::from(r);
                    sums[1] += weight * f64::from(g);
                    sums[2] += weight * f64::from(b);
                }
            }
            let value = (
                sums[0].round().clamp(0.0, 255.0) as u8,
                sums[1].round().clamp(0.0, 255.0) as u8,
                sums[2].round().clamp(0.0, 255.0) as u8,
            );
            output.set_pixel(x, y, value);
        }
    }
    output
}

type Homography = [[f64; 3]; 3];

fn homography_from_quad(quad: &Quad, out_w: usize, out_h: usize) -> Homography {
    let destinations = [
        (quad.0[0].x, quad.0[0].y),
        (quad.0[1].x, quad.0[1].y),
        (quad.0[2].x, quad.0[2].y),
        (quad.0[3].x, quad.0[3].y),
    ];
    let sources = [
        (0.0, 0.0),
        (out_w as f64, 0.0),
        (out_w as f64, out_h as f64),
        (0.0, out_h as f64),
    ];
    solve_homography(&sources, &destinations)
}

/// Solves the 8 unknowns of the output→input homography (h33 = 1).
fn solve_homography(from: &[(f64, f64); 4], to: &[(f64, f64); 4]) -> Homography {
    let mut matrix = [[0.0f64; 9]; 8];
    for index in 0..4 {
        let (x, y) = from[index];
        let (u, v) = to[index];
        matrix[index * 2] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        matrix[index * 2 + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    // Gaussian elimination with partial pivoting.
    for column in 0..8 {
        let pivot = (column..8)
            .max_by(|a, b| {
                matrix[*a][column]
                    .abs()
                    .partial_cmp(&matrix[*b][column].abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(column);
        matrix.swap(column, pivot);
        let divisor = matrix[column][column];
        if divisor.abs() < 1e-12 {
            return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        }
        for value in matrix[column].iter_mut().skip(column) {
            *value /= divisor;
        }
        let pivot_row = matrix[column];
        for (row_index, row) in matrix.iter_mut().enumerate() {
            if row_index == column {
                continue;
            }
            let factor = row[column];
            if factor == 0.0 {
                continue;
            }
            for (slot, pivot_value) in row.iter_mut().zip(pivot_row.iter()).skip(column) {
                *slot -= factor * pivot_value;
            }
        }
    }
    let mut solution = [0.0f64; 8];
    for index in 0..8 {
        solution[index] = matrix[index][8];
    }
    [
        [solution[0], solution[1], solution[2]],
        [solution[3], solution[4], solution[5]],
        [solution[6], solution[7], 1.0],
    ]
}

fn apply_homography(h: &Homography, x: f64, y: f64) -> (f64, f64) {
    let denominator = h[2][0] * x + h[2][1] * y + h[2][2];
    let u = (h[0][0] * x + h[0][1] * y + h[0][2]) / denominator;
    let v = (h[1][0] * x + h[1][1] * y + h[1][2]) / denominator;
    (u, v)
}

/// Bilinear resize (OpenCV `INTER_LINEAR`): destination pixel centres map back with
/// `src = (dst + 0.5) * scale - 0.5`.
pub fn resize_linear(source: &Rgb, new_width: usize, new_height: usize) -> Rgb {
    if new_width == 0 || new_height == 0 {
        return Rgb::new(0, 0, Vec::new());
    }
    let scale_x = source.width as f64 / new_width as f64;
    let scale_y = source.height as f64 / new_height as f64;
    let mut output = Rgb::new(
        new_width,
        new_height,
        vec![128u8; new_width * new_height * 3],
    );
    for y in 0..new_height {
        let source_y = (y as f64 + 0.5) * scale_y - 0.5;
        let y0 = source_y.floor() as i32;
        let fy = source_y - y0 as f64;
        for x in 0..new_width {
            let source_x = (x as f64 + 0.5) * scale_x - 0.5;
            let x0 = source_x.floor() as i32;
            let fx = source_x - x0 as f64;
            let mut channels = [0.0f64; 3];
            for (dx, weight_x) in [(0, 1.0 - fx), (1, fx)] {
                for (dy, weight_y) in [(0, 1.0 - fy), (1, fy)] {
                    let (r, g, b) = source.pixel_replicate(x0 + dx, y0 + dy);
                    let weight = weight_x * weight_y;
                    channels[0] += weight * f64::from(r);
                    channels[1] += weight * f64::from(g);
                    channels[2] += weight * f64::from(b);
                }
            }
            output.set_pixel(
                x,
                y,
                (
                    channels[0].round().clamp(0.0, 255.0) as u8,
                    channels[1].round().clamp(0.0, 255.0) as u8,
                    channels[2].round().clamp(0.0, 255.0) as u8,
                ),
            );
        }
    }
    output
}

/// One recogniser input tensor: CHW f32 in `[-1, 1]`, height 48, width from the C# batch rule.
#[derive(Debug, Clone, PartialEq)]
pub struct RecInput {
    pub channels: usize,
    pub height: usize,
    pub width: usize,
    pub data: Vec<f32>,
}

/// The recogniser's crop→tensor path (`RecPreprocess.ResizeNormImg` + `PreprocessBatchCore`):
///
/// - the model's input height is a 48-px CONTRACT (the model declares it dynamic, so preprocess owns
///   it);
/// - the tensor width is `round(48 * max(320/48, crop w/h))` — the 320-wide cap is a floor on the
///   ratio, not a hard width cap, so a wide crop widens the tensor;
/// - the crop is resized (bilinear) to `min(ceil(48*w/h), width) x 48`, then right-padded with 0.0;
/// - values are `x * 2/255 - 1`, i.e. mean/std 0.5/0.5, RGB→CHW.
pub fn rec_input(crop: &Rgb) -> RecInput {
    const HEIGHT: usize = 48;
    const CONFIG_RATIO: f64 = 320.0 / 48.0;
    let ratio = crop.width as f64 / crop.height as f64;
    let max_ratio = CONFIG_RATIO.max(ratio);
    let width = (HEIGHT as f64 * max_ratio).round() as usize;
    let estimated = (HEIGHT as f64 * ratio).ceil() as usize;
    let resized_width = estimated.min(width).max(1);
    let resized = resize_linear(crop, resized_width, HEIGHT);
    let mut data = vec![0.0f32; 3 * HEIGHT * width];
    for channel in 0..3 {
        for y in 0..HEIGHT {
            for x in 0..resized_width {
                let (r, g, b) = resized.pixel(x, y);
                let value = f64::from(match channel {
                    0 => r,
                    1 => g,
                    _ => b,
                }) * 2.0
                    / 255.0
                    - 1.0;
                data[channel * HEIGHT * width + y * width + x] = value as f32;
            }
        }
    }
    RecInput {
        channels: 3,
        height: HEIGHT,
        width,
        data,
    }
}

/// The detector's input normalisation: CHW RGB with `mean = [0.485,0.456,0.406]`,
/// `std = [0.229,0.224,0.225]` (the C# library's PP-OCRv4 defaults, *not* the Python package's
/// 0.5/0.5).
pub fn det_input(image: &Rgb) -> Vec<f32> {
    const MEAN: [f64; 3] = [0.485, 0.456, 0.406];
    const STD: [f64; 3] = [0.229, 0.224, 0.225];
    let plane = image.width * image.height;
    let mut data = vec![0.0f32; plane * 3];
    for y in 0..image.height {
        for x in 0..image.width {
            let (r, g, b) = image.pixel(x, y);
            let values = [r, g, b];
            for channel in 0..3 {
                data[channel * plane + y * image.width + x] =
                    ((f64::from(values[channel]) / 255.0 - MEAN[channel]) / STD[channel]) as f32;
            }
        }
    }
    data
}

/// Convenience for tests: a point from the detgeom type.
pub fn point(x: f64, y: f64) -> Pt {
    Pt { x, y }
}

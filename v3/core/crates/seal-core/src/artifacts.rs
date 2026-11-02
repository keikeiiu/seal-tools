//! The artifact overlay writer: draw calibration marks onto a captured frame.
//!
//! v2 saved `config/calib_tuner.png`, `calib_gem.png` and `calib_pet.png` by drawing over the
//! calibration screenshot (`CALIBRATION.md` §5). The capture itself is already a PNG in the core's
//! frame cache; this module owns the only image arithmetic the calibration sessions need — outlined
//! bands/boxes and filled point dots — with no dependency beyond the `png` crate `seal-win` already
//! uses. Coordinates are client-relative physical pixels, the same space as every calibration mark;
//! the image is the frame's own pixels, so no DPI conversion happens here.

use seal_domain::geometry::{Point, Rect};
use seal_win::WinError;

/// The small fixed overlay palette. v2 used green/blue/orange for the tuner's three bands, yellow
/// for gem points and magenta for the result box; these names give later session saves one shared
/// vocabulary instead of scattered RGB triples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Green,
    Blue,
    Orange,
    Yellow,
    Magenta,
    White,
}

impl Color {
    #[must_use]
    pub const fn rgb(self) -> [u8; 3] {
        match self {
            Self::Green => [0, 200, 0],
            Self::Blue => [0, 120, 255],
            Self::Orange => [255, 140, 0],
            Self::Yellow => [255, 220, 0],
            Self::Magenta => [255, 0, 200],
            Self::White => [255, 255, 255],
        }
    }

    const fn rgba(self) -> [u8; 4] {
        let [r, g, b] = self.rgb();
        [r, g, b, 255]
    }
}

/// One mark drawn on the frame. `Rect` is outlined (never filled): the bands and result boxes mark
/// an area without hiding its pixels. `Dot` is filled, for a point mark (gem grades/resources, the
/// tuner spring point, the pet hover point).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Rect {
        rect: Rect,
        color: Color,
    },
    Dot {
        point: Point,
        color: Color,
        radius: u32,
    },
}

/// Draw `shapes` over the PNG `png` and return PNG bytes at the same dimensions.
///
/// Every shape is clipped to the image instead of panicking: a stale or hand-edited mark may point
/// partly outside the captured frame, and a save must refuse or draw what is visible rather than
/// kill the core. A non-PNG input is the decoder's own error (`WinError::EmptyCapture`), the same
/// shape the existing crop path reports.
///
/// # Errors
///
/// Returns the decoder's `WinError::EmptyCapture` for a non-PNG input, and the encoder's error when the annotated PNG cannot be written.
pub fn annotate_png(png: &[u8], shapes: &[Shape]) -> Result<Vec<u8>, WinError> {
    let (width, height, mut rgba) = seal_win::pure::decode_png(png)?;
    for shape in shapes {
        match *shape {
            Shape::Rect { rect, color } => draw_rect(&mut rgba, width, height, rect, color),
            Shape::Dot {
                point,
                color,
                radius,
            } => draw_dot(&mut rgba, width, height, point, color, radius),
        }
    }
    // `encode_png` takes BGRA, the order a Win32 capture hands back; `decode_png` gives RGBA.
    let (pixels, _) = rgba.as_chunks::<4>();
    let mut bgra = Vec::with_capacity(rgba.len());
    for pixel in pixels {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    seal_win::pure::encode_png(width, height, &bgra)
}

#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "frame pixel coordinates and counts are far inside the range these conversions can represent"
)]
fn draw_rect(rgba: &mut [u8], width: u32, height: u32, rect: Rect, color: Color) {
    let left = rect.x.max(0);
    let top = rect.y.max(0);
    let right = rect.right().min(width as i32);
    let bottom = rect.bottom().min(height as i32);
    if left >= right || top >= bottom {
        return;
    }
    let fill = color.rgba();
    let mut put = |x: i32, y: i32| {
        let index = ((y as usize) * width as usize + x as usize) * 4;
        rgba[index..index + 4].copy_from_slice(&fill);
    };
    for x in left..right {
        put(x, top);
        put(x, bottom - 1);
    }
    for y in top..bottom {
        put(left, y);
        put(right - 1, y);
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "frame pixel coordinates and counts are far inside the range these conversions can represent"
)]
fn draw_dot(rgba: &mut [u8], width: u32, height: u32, point: Point, color: Color, radius: u32) {
    let radius = i64::from(radius.min(i32::MAX as u32));
    let fill = color.rgba();
    let min_x = (i64::from(point.x) - radius).max(0);
    let max_x = (i64::from(point.x) + radius).min(i64::from(width) - 1);
    let min_y = (i64::from(point.y) - radius).max(0);
    let max_y = (i64::from(point.y) + radius).min(i64::from(height) - 1);
    if min_x > max_x || min_y > max_y {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x - i64::from(point.x);
            let dy = y - i64::from(point.y);
            if dx * dx + dy * dy <= radius * radius {
                let index = (y as usize * width as usize + x as usize) * 4;
                rgba[index..index + 4].copy_from_slice(&fill);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BACKGROUND: [u8; 3] = [40, 40, 40];

    fn frame(width: u32, height: u32) -> Vec<u8> {
        let bgra =
            [BACKGROUND[2], BACKGROUND[1], BACKGROUND[0], 255].repeat((width * height) as usize);
        seal_win::pure::encode_png(width, height, &bgra).expect("a synthetic frame")
    }

    fn pixels(png: &[u8]) -> (u32, u32, Vec<u8>) {
        seal_win::pure::decode_png(png).expect("annotated PNG decodes")
    }

    fn at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * width + x) * 4) as usize;
        [
            pixels[index],
            pixels[index + 1],
            pixels[index + 2],
            pixels[index + 3],
        ]
    }

    #[test]
    fn a_rect_is_an_outline_and_the_frame_keeps_its_size() {
        let source = frame(20, 10);
        let annotated = annotate_png(
            &source,
            &[Shape::Rect {
                rect: Rect::new(2, 2, 6, 4),
                color: Color::Green,
            }],
        )
        .expect("draws");
        let (width, height, rgba) = pixels(&annotated);
        assert_eq!((width, height), (20, 10));

        let green = [
            Color::Green.rgb()[0],
            Color::Green.rgb()[1],
            Color::Green.rgb()[2],
            255,
        ];
        assert_eq!(at(&rgba, width, 2, 2), green, "top-left border");
        assert_eq!(at(&rgba, width, 7, 5), green, "bottom-right border");
        assert_eq!(
            at(&rgba, width, 4, 3),
            [BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], 255],
            "the interior is not filled"
        );
        assert_eq!(
            at(&rgba, width, 9, 4),
            [BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], 255],
            "outside is untouched"
        );
    }

    #[test]
    fn a_dot_fills_its_centre_and_radius() {
        let source = frame(10, 10);
        let annotated = annotate_png(
            &source,
            &[Shape::Dot {
                point: Point::new(5, 5),
                color: Color::Yellow,
                radius: 2,
            }],
        )
        .expect("draws");
        let (width, _, rgba) = pixels(&annotated);
        let yellow = [
            Color::Yellow.rgb()[0],
            Color::Yellow.rgb()[1],
            Color::Yellow.rgb()[2],
            255,
        ];
        assert_eq!(at(&rgba, width, 5, 5), yellow, "centre");
        assert_eq!(at(&rgba, width, 5, 3), yellow, "top of the radius");
        assert_eq!(
            at(&rgba, width, 0, 0),
            [BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], 255],
            "outside the radius is untouched"
        );
    }

    #[test]
    fn a_dot_fills_its_centre_and_clips_at_the_frame_edges() {
        let source = frame(10, 10);
        let annotated = annotate_png(
            &source,
            &[Shape::Dot {
                point: Point::new(-1, -1),
                color: Color::Yellow,
                radius: 3,
            }],
        )
        .expect("clips without panicking");
        let (width, _, rgba) = pixels(&annotated);
        let yellow = [
            Color::Yellow.rgb()[0],
            Color::Yellow.rgb()[1],
            Color::Yellow.rgb()[2],
            255,
        ];
        assert_eq!(at(&rgba, width, 0, 0), yellow, "visible clipped pixel");
        assert_eq!(
            at(&rgba, width, 4, 4),
            [BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], 255],
            "outside the clipped dot is untouched"
        );
    }

    #[test]
    fn an_out_of_bounds_rect_is_clipped_instead_of_panicking() {
        let source = frame(8, 8);
        let annotated = annotate_png(
            &source,
            &[Shape::Rect {
                rect: Rect::new(-4, -4, 10, 10),
                color: Color::Magenta,
            }],
        )
        .expect("clips");
        let (width, _, rgba) = pixels(&annotated);
        assert_eq!(
            at(&rgba, width, 0, 0),
            [
                Color::Magenta.rgb()[0],
                Color::Magenta.rgb()[1],
                Color::Magenta.rgb()[2],
                255
            ]
        );
    }

    #[test]
    fn a_non_png_is_the_decoders_error() {
        let error = annotate_png(b"not a PNG", &[]).expect_err("refused");
        assert_eq!(error, seal_win::WinError::EmptyCapture);
    }
}

//! The detector post-processing, pure and model-free: every test here runs with no `SEAL_OCR_MODELS`
//! and no ONNX session.
//!
//! The fixtures are hand-built score maps and point sets, so the expected values are arithmetic, not
//! a recorded model output. Constants are the ones in `OCR-RESEARCH.md` §3 (the C# library's
//! PP-OCRv4 defaults): threshold 0.3, dilation 2×2, `box_thresh` 0.6, unclip 1.5, the 10 px row
//! threshold, 48-px input height, 320-wide rec cap.

use seal_ocr::detgeom::{
    DetBox, Quad, arc_length, box_score, contour_area, filled_mean, min_area_rect, offset_round,
    sort_boxes, unclip_distance,
};
use seal_ocr::detmask::{Mask, ScoreMap, dilate, find_contours, threshold};
use seal_ocr::imageops::{Rgb, perspective_crop, rec_input};

fn score_map(width: usize, height: usize, scores: Vec<f32>) -> ScoreMap {
    ScoreMap::new(width, height, scores)
}

// ── 1. threshold + dilate ────────────────────────────────────────────────────────────────────

#[test]
fn threshold_is_strictly_greater_than_the_constant() {
    let map = score_map(3, 1, vec![0.3, 0.300_001, 0.29]);

    let mask = threshold(&map, 0.3);

    // OpenCV's THRESH_BINARY is `src > thresh` (255) / 0 otherwise; 0.3 exactly is NOT selected.
    assert!(!mask.get(0, 0));
    assert!(mask.get(1, 0));
    assert!(!mask.get(2, 0));
}

#[test]
fn dilate_uses_a_2x2_kernel_with_opencvs_default_anchor() {
    // A single set pixel at (1,1) in a 3x3 map. OpenCV's 2x2 kernel anchor is (1,1), so the output is
    // set where the 2x2 window ending at the output pixel contains it: (1,1),(2,1),(1,2),(2,2).
    let mut mask = Mask::new(3, 3);
    mask.set(1, 1, true);

    let grown = dilate(&mask);

    let set: Vec<(usize, usize)> = (0..3)
        .flat_map(|y| (0..3).map(move |x| (x, y)))
        .filter(|(x, y)| grown.get(*x, *y))
        .collect();
    assert_eq!(set, vec![(1, 1), (2, 1), (1, 2), (2, 2)]);
}

#[test]
fn dilate_keeps_a_border_touching_pixel_inside_the_map() {
    let mut mask = Mask::new(3, 3);
    mask.set(0, 0, true);

    let grown = dilate(&mask);

    // The window at (0,0) is clipped, not wrapped: only (0,0),(1,0),(0,1),(1,1) can be set.
    let set: Vec<(usize, usize)> = (0..3)
        .flat_map(|y| (0..3).map(move |x| (x, y)))
        .filter(|(x, y)| grown.get(*x, *y))
        .collect();
    assert_eq!(set, vec![(0, 0), (1, 0), (0, 1), (1, 1)]);
}

#[test]
fn an_empty_map_has_no_contours() {
    let mask = threshold(&score_map(4, 4, vec![0.0; 16]), 0.3);

    assert_eq!(find_contours(&mask), [] as [seal_ocr::detmask::Contour; 0]);
}

// ── 2. connected components + contours ────────────────────────────────────────────────────────

fn blob_mask(points: &[(usize, usize)], width: usize, height: usize) -> Mask {
    let mut mask = Mask::new(width, height);
    for (x, y) in points {
        mask.set(*x, *y, true);
    }
    mask
}

#[test]
fn two_separated_blobs_come_out_in_raster_order_and_stay_separate() {
    // Blob A at (0,0) and blob B at (3,0): raster order must find A first.
    let mut mask = Mask::new(6, 3);
    for (x, y) in [(0usize, 0usize), (1, 0), (0, 1), (1, 1)] {
        mask.set(x, y, true);
    }
    for (x, y) in [(3usize, 0usize), (4, 0), (3, 1), (4, 1)] {
        mask.set(x, y, true);
    }

    let contours = find_contours(&mask);

    assert_eq!(contours.len(), 2, "one contour per blob: {contours:?}");
    assert_eq!(contours[0].points.first(), Some(&(0, 0)));
    assert_eq!(contours[1].points.first(), Some(&(3, 0)));
    // Each 2x2 blob's boundary is its four pixels; the traced perimeter is 4 px.
    assert_eq!(contours[0].points.len(), 4);
    assert_eq!(contours[1].points.len(), 4);
}

#[test]
fn a_one_pixel_blob_is_its_own_contour() {
    let mask = blob_mask(&[(2, 1)], 5, 4);

    let contours = find_contours(&mask);

    assert_eq!(contours.len(), 1);
    assert_eq!(contours[0].points, vec![(2, 1)]);
}

#[test]
fn a_blob_touching_the_edge_is_traced_without_wrapping() {
    // A full first column: the boundary runs down and back up, never wrapping across x = 0/width.
    let mut mask = Mask::new(4, 4);
    for y in 0..4 {
        mask.set(0, y, true);
    }

    let contours = find_contours(&mask);

    assert_eq!(contours.len(), 1);
    let points = &contours[0].points;
    assert!(points.iter().all(|(x, _)| *x < 4), "no wrap: {points:?}");
    assert!(points.iter().all(|(_, y)| *y < 4), "no wrap: {points:?}");
}

#[test]
fn a_hole_is_a_second_contour_with_the_opposite_orientation() {
    // 5x5 ring: outer border set, centre 3x3 clear. RETR_LIST yields the outer contour then its hole.
    let mut mask = Mask::new(5, 5);
    for y in 0..5 {
        for x in 0..5 {
            let border = x == 0 || y == 0 || x == 4 || y == 4;
            mask.set(x, y, border);
        }
    }

    let contours = find_contours(&mask);

    assert_eq!(contours.len(), 2, "outer + hole: {contours:?}");
    let outer: Vec<(f64, f64)> = contours[0]
        .points
        .iter()
        .map(|(x, y)| (f64::from(*x), f64::from(*y)))
        .collect();
    let hole: Vec<(f64, f64)> = contours[1]
        .points
        .iter()
        .map(|(x, y)| (f64::from(*x), f64::from(*y)))
        .collect();
    assert!(contour_area(&outer) > contour_area(&hole));
    // OpenCV traces outer borders clockwise and holes counter-clockwise (image coordinates, y down).
    assert!(signed_area(&outer) > 0.0, "outer is clockwise: {outer:?}");
    assert!(
        signed_area(&hole) < 0.0,
        "hole is counter-clockwise: {hole:?}"
    );
}

fn signed_area(points: &[(f64, f64)]) -> f64 {
    let mut sum = 0.0;
    for index in 0..points.len() {
        let (x1, y1) = points[index];
        let (x2, y2) = points[(index + 1) % points.len()];
        sum += x1 * y2 - x2 * y1;
    }
    -sum / 2.0
}

// ── 3. min-area rectangle ───────────────────────────────────────────────────────────────────

fn quad_points(quad: &Quad) -> Vec<(f64, f64)> {
    quad.0.iter().map(|point| (point.x, point.y)).collect()
}

fn quad_area(quad: &Quad) -> f64 {
    contour_area(&quad_points(quad))
}

#[test]
fn an_axis_aligned_rectangle_reduces_to_itself() {
    let points = [(0, 0), (10, 0), (10, 4), (0, 4)];

    let quad = min_area_rect(&points);

    assert!((quad_area(&quad) - 40.0).abs() < 1e-9, "{quad:?}");
    for expected in [(0.0, 0.0), (10.0, 0.0), (10.0, 4.0), (0.0, 4.0)] {
        assert!(
            quad.0
                .iter()
                .any(|point| (point.x - expected.0).abs() < 1e-6
                    && (point.y - expected.1).abs() < 1e-6),
            "the rectangle's own corners are returned: {quad:?}"
        );
    }
}

#[test]
#[allow(
    clippy::cast_possible_truncation,
    reason = "the fixture's rotated corners round to small integers, well inside i32"
)]
fn a_thirty_degree_rotated_rectangle_recovers_its_area() {
    // 10 x 4 rectangle rotated by 30 degrees.
    let (w, h) = (10.0f64, 4.0f64);
    let angle = 30.0f64.to_radians();
    let (sin, cos) = angle.sin_cos();
    let points: Vec<(i32, i32)> = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]
        .iter()
        .map(|(x, y)| {
            (
                (x * cos - y * sin).round() as i32,
                (x * sin + y * cos).round() as i32,
            )
        })
        .collect();

    let quad = min_area_rect(&points);

    // The corners are rounded to integers, so the hull of a rotated 40 px² rectangle can shrink by a
    // couple of pixels² — the check is that the calipers find the rotated rectangle, not the bbox.
    assert!(
        (quad_area(&quad) - 40.0).abs() < 3.0,
        "area within rounding: {quad:?}"
    );
}

#[test]
fn a_square_at_45_degrees_finds_area_200_not_the_axis_aligned_box() {
    // Diamond with vertices (10,10),(20,0),(10,-10),(0,0): side sqrt(200), area 200. The axis-aligned
    // bounding box (400) is not the minimum.
    let points = [(0, 0), (10, 10), (20, 0), (10, -10)];

    let quad = min_area_rect(&points);

    assert!((quad_area(&quad) - 200.0).abs() < 1.0, "{quad:?}");
}

#[test]
fn a_degenerate_collinear_contour_has_zero_area() {
    let points = [(0, 0), (1, 1), (2, 2), (3, 3)];

    let quad = min_area_rect(&points);

    assert!(quad_area(&quad) < 1e-9, "{quad:?}");
}

#[test]
fn box_points_are_clockwise_starting_at_the_top_left_most_corner() {
    let points = [(0, 0), (10, 0), (10, 4), (0, 4)];

    let quad = min_area_rect(&points);

    // Our documented order: start at the top-most, then left-most corner, walk clockwise.
    assert_eq!(quad.0[0].x, 0.0);
    assert_eq!(quad.0[0].y, 0.0);
    let first = (quad.0[0].x, quad.0[0].y);
    let second = (quad.0[1].x, quad.0[1].y);
    assert_eq!(first, (0.0, 0.0));
    assert_eq!(second, (10.0, 0.0), "clockwise from the start: {quad:?}");
}

// ── 4. box score, filter, unclip ───────────────────────────────────────────────────────────

#[test]
fn the_box_score_is_the_mean_over_the_filled_polygon() {
    let map = score_map(
        4,
        4,
        vec![
            0.0, 0.0, 0.0, 0.0, // y0
            0.0, 0.5, 0.6, 0.0, // y1
            0.0, 0.7, 0.8, 0.0, // y2
            0.0, 0.0, 0.0, 0.0, // y3
        ],
    );
    let polygon = [
        seal_ocr::detgeom::Pt { x: 1.0, y: 1.0 },
        seal_ocr::detgeom::Pt { x: 3.0, y: 1.0 },
        seal_ocr::detgeom::Pt { x: 3.0, y: 3.0 },
        seal_ocr::detgeom::Pt { x: 1.0, y: 3.0 },
    ];

    let mean = filled_mean(&map, &polygon);

    assert!((mean - 0.65).abs() < 1e-6, "{mean}");
}

#[test]
fn box_thresh_rejects_a_low_scoring_box() {
    let map = score_map(4, 4, vec![0.1; 16]);
    let quad = Quad([
        seal_ocr::detgeom::Pt { x: 1.0, y: 1.0 },
        seal_ocr::detgeom::Pt { x: 3.0, y: 1.0 },
        seal_ocr::detgeom::Pt { x: 3.0, y: 3.0 },
        seal_ocr::detgeom::Pt { x: 1.0, y: 3.0 },
    ]);
    assert!(box_score(&map, &quad) < 0.6, "0.1 mean is below box_thresh");
}

#[test]
fn unclip_distance_is_area_times_ratio_over_perimeter() {
    let square = Quad([
        seal_ocr::detgeom::Pt { x: 0.0, y: 0.0 },
        seal_ocr::detgeom::Pt { x: 10.0, y: 0.0 },
        seal_ocr::detgeom::Pt { x: 10.0, y: 10.0 },
        seal_ocr::detgeom::Pt { x: 0.0, y: 10.0 },
    ]);
    let points = quad_points(&square);
    let area = contour_area(&points);
    let perimeter = arc_length(&points, true);

    assert!((area - 100.0).abs() < 1e-9);
    assert!((perimeter - 40.0).abs() < 1e-9);
    let distance = unclip_distance(area, perimeter, 1.5);
    assert!((distance - 3.75).abs() < 1e-9);
}

#[test]
fn the_round_join_offset_moves_each_edge_out_by_the_distance() {
    let square = Quad([
        seal_ocr::detgeom::Pt { x: 0.0, y: 0.0 },
        seal_ocr::detgeom::Pt { x: 10.0, y: 0.0 },
        seal_ocr::detgeom::Pt { x: 10.0, y: 10.0 },
        seal_ocr::detgeom::Pt { x: 0.0, y: 10.0 },
    ]);

    let offset = offset_round(&square, 3.75);

    let xs: Vec<f64> = offset.iter().map(|point| point.x).collect();
    let ys: Vec<f64> = offset.iter().map(|point| point.y).collect();
    let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min_y = ys.iter().copied().fold(f64::INFINITY, f64::min);
    let max_y = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    // Round join (Minkowski sum with a disk): edges move out by the distance, corners are rounded.
    assert!((min_x + 3.75).abs() < 1e-6, "{offset:?}");
    assert!((max_x - 13.75).abs() < 1e-6, "{offset:?}");
    assert!((min_y + 3.75).abs() < 1e-6, "{offset:?}");
    assert!((max_y - 13.75).abs() < 1e-6, "{offset:?}");
    // Area = original + perimeter*d + pi*d^2.
    let offset_points: Vec<(f64, f64)> = offset.iter().map(|point| (point.x, point.y)).collect();
    let area = contour_area(&offset_points);
    let expected = 100.0 + 40.0 * 3.75 + std::f64::consts::PI * 3.75 * 3.75;
    assert!(
        (area - expected).abs() / expected < 0.02,
        "area {area} vs {expected}"
    );
}

// ── 5. warp + row sort ────────────────────────────────────────────────────────────────────

fn ramp_image(width: usize, height: usize) -> Rgb {
    let mut pixels = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            pixels.push(u8::try_from(x * 3).expect("the fixture ramp fits a byte"));
            pixels.push(u8::try_from(y * 5).expect("the fixture ramp fits a byte"));
            pixels.push(128u8);
        }
    }
    Rgb::new(width, height, pixels)
}

#[test]
fn an_identity_quad_recovers_the_image() {
    let image = ramp_image(8, 6);
    let quad = Quad([
        seal_ocr::detgeom::Pt { x: 0.0, y: 0.0 },
        seal_ocr::detgeom::Pt { x: 8.0, y: 0.0 },
        seal_ocr::detgeom::Pt { x: 8.0, y: 6.0 },
        seal_ocr::detgeom::Pt { x: 0.0, y: 6.0 },
    ]);

    let crop = perspective_crop(&image, &quad, 8, 6);

    assert_eq!(crop.width, 8);
    assert_eq!(crop.height, 6);
    assert_eq!(crop.pixels, image.pixels, "an identity quad is a copy");
}

#[test]
fn the_warp_recovers_a_known_affine_shift() {
    let image = ramp_image(8, 6);
    // Shift the source view left by 2 px: output x=0..5 samples source x+2.
    let quad = Quad([
        seal_ocr::detgeom::Pt { x: 2.0, y: 0.0 },
        seal_ocr::detgeom::Pt { x: 8.0, y: 0.0 },
        seal_ocr::detgeom::Pt { x: 8.0, y: 6.0 },
        seal_ocr::detgeom::Pt { x: 2.0, y: 6.0 },
    ]);

    let crop = perspective_crop(&image, &quad, 6, 6);

    // The source is a linear ramp in x (3x) and y (5y), which cubic interpolation reproduces exactly.
    for y in 0..6 {
        for x in 0..6 {
            let got = crop.pixel(x, y);
            let expected = ((x + 2) * 3, y * 5, 128u8);
            assert!(
                usize::from(got.0).abs_diff(expected.0) <= 1
                    && usize::from(got.1).abs_diff(expected.1) <= 1,
                "({x},{y}) got {got:?} expected {expected:?}"
            );
            assert_eq!(got.2, 128);
        }
    }
}

#[test]
fn the_warp_replicates_the_border_instead_of_reading_out_of_bounds() {
    let image = ramp_image(8, 6);
    // Sample starting 2 px left of the image: the first two output columns must replicate column 0.
    let quad = Quad([
        seal_ocr::detgeom::Pt { x: -2.0, y: 0.0 },
        seal_ocr::detgeom::Pt { x: 6.0, y: 0.0 },
        seal_ocr::detgeom::Pt { x: 6.0, y: 6.0 },
        seal_ocr::detgeom::Pt { x: -2.0, y: 6.0 },
    ]);

    let crop = perspective_crop(&image, &quad, 8, 6);

    assert_eq!(crop.pixel(0, 0), image.pixel(0, 0));
    assert_eq!(crop.pixel(1, 0), image.pixel(0, 0), "replicate border");
    assert_eq!(crop.pixel(4, 0), image.pixel(2, 0));
}

#[test]
fn the_row_sort_uses_the_ten_pixel_threshold() {
    // First points: A(10,100), B(50,100), C(0,105), D(0,115), E(5,115).
    let quad_at = |x: f64, y: f64| {
        Quad([
            seal_ocr::detgeom::Pt { x, y },
            seal_ocr::detgeom::Pt { x: x + 5.0, y },
            seal_ocr::detgeom::Pt {
                x: x + 5.0,
                y: y + 5.0,
            },
            seal_ocr::detgeom::Pt { x, y: y + 5.0 },
        ])
    };
    let mut boxes = vec![
        DetBox {
            quad: quad_at(10.0, 100.0),
            score: 0.9,
        },
        DetBox {
            quad: quad_at(50.0, 100.0),
            score: 0.9,
        },
        DetBox {
            quad: quad_at(0.0, 105.0),
            score: 0.9,
        },
        DetBox {
            quad: quad_at(0.0, 115.0),
            score: 0.9,
        },
        DetBox {
            quad: quad_at(5.0, 115.0),
            score: 0.9,
        },
    ];

    sort_boxes(&mut boxes);

    let xs: Vec<f64> = boxes.iter().map(|box_| box_.quad.0[0].x).collect();
    // Row 0 (tops 100..109, y gap < 10): C(0), A(10), B(50). Row 1 (D and E, gap 10 from C): D(0), E(5).
    assert_eq!(xs, vec![0.0, 10.0, 50.0, 0.0, 5.0]);
}

// ── 6. crop → tensor input: 48-px rec normalisation ─────────────────────────────────────

#[test]
fn rec_input_is_rgb_chw_height_48_and_normalised_to_minus_one_one() {
    let white = Rgb::new(1, 1, vec![255, 255, 255]);
    let input = rec_input(&white);

    assert_eq!(input.channels, 3);
    assert_eq!(input.height, 48);
    assert_eq!(
        input.width, 320,
        "the 320-wide cap comes from the config ratio"
    );
    // x*2/255 - 1 == (x/255 - 0.5)/0.5: white is +1, black -1, mid-grey ~0.0039.
    assert!((input.data[0] - 1.0).abs() < 1e-6);
    // The resized 48-px-wide image occupies the first 48 columns; the rest is zero padding.
    // Channel 0, row 0, first padded column (the 48-px resized crop occupies x < 48).
    assert_eq!(input.data[48], 0.0, "right padding is zero");
}

#[test]
fn rec_input_of_black_is_minus_one() {
    let black = Rgb::new(1, 1, vec![0, 0, 0]);

    let input = rec_input(&black);

    assert!((input.data[0] + 1.0).abs() < 1e-6);
}

#[test]
fn rec_input_of_a_wide_crop_keeps_its_aspect_ratio_above_the_cap() {
    // ratio 10 > 320/48: the tensor widens to round(48*10) = 480 rather than squeezing into 320.
    let wide = Rgb::new(100, 10, vec![128; 100 * 10 * 3]);

    let input = rec_input(&wide);

    assert_eq!(input.width, 480);
    assert_eq!(input.height, 48);
}

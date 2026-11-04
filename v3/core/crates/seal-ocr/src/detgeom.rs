//! The detector geometry: contour area/length, convex hull, `MinAreaRect`, `FillPoly`+`Mean` box
//! scoring, the round-join unclip offset, and the row sort.
//!
//! No dependency is added: the min-area rectangle is a monotonic-chain hull + rotating calipers, and
//! the round-join unclip is the Minkowski sum of the convex box with a disk (the Clipper
//! `InflatePaths(..., JoinType.Round)` equivalent for the convex candidates DB produces). Both are
//! small enough to own; the alternatives considered are in the task report.

/// A point in image coordinates (x right, y down), as f64 for geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

/// Four ordered corners, clockwise in image coordinates starting at the top-left corner (the order
/// `FindContours` + row sort expect: the first point's Y is the sort key).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad(pub [Pt; 4]);

/// Shoelace area (absolute).
#[must_use]
pub fn contour_area(points: &[(f64, f64)]) -> f64 {
    signed_area(points).abs()
}

fn signed_area(points: &[(f64, f64)]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for index in 0..points.len() {
        let (x1, y1) = points[index];
        let (x2, y2) = points[(index + 1) % points.len()];
        sum += x1 * y2 - x2 * y1;
    }
    sum / 2.0
}

/// `ArcLength`: pairwise distance, closing the loop when `closed`.
#[must_use]
pub fn arc_length(points: &[(f64, f64)], closed: bool) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let mut length = 0.0;
    for index in 0..points.len() - 1 {
        length += distance(points[index], points[index + 1]);
    }
    if closed {
        length += distance(points[points.len() - 1], points[0]);
    }
    length
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn distance_pt(a: Pt, b: Pt) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// The unclip distance the C# computes: `area * unclip_ratio / perimeter`.
#[must_use]
pub fn unclip_distance(area: f64, perimeter: f64, unclip_ratio: f64) -> f64 {
    if perimeter <= 0.0 {
        return 0.0;
    }
    area * unclip_ratio / perimeter
}

/// `OpenCV` `ContourArea` sign helper for a traced contour (positive = counter-clockwise in image
/// coordinates, which is what the contour tracer normalises outer boundaries to).
#[must_use]
pub fn polygon_is_counter_clockwise(points: &[(f64, f64)]) -> bool {
    signed_area(points) > 0.0
}

/// Monotonic-chain convex hull over float points, collinear points removed. Returns points in
/// counter-clockwise order (image coordinates). Non-finite points are dropped rather than sorted into
/// a meaningless order, so a corrupt score map cannot panic the calipers.
#[allow(
    clippy::float_cmp,
    reason = "duplicate corners are exact duplicates: they come either from integer pixel indices or from the same offset sample pushed twice, and exact equality is what must collapse them"
)]
pub fn convex_hull_pts(points: &[Pt]) -> Vec<Pt> {
    let mut sorted: Vec<Pt> = points
        .iter()
        .copied()
        .filter(|point| point.x.is_finite() && point.y.is_finite())
        .collect();
    sorted.sort_unstable_by(compare_points);
    sorted.dedup_by(|a, b| a.x == b.x && a.y == b.y);
    if sorted.len() <= 2 {
        return sorted;
    }
    let mut lower: Vec<Pt> = Vec::new();
    for point in &sorted {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], *point) <= 0.0
        {
            lower.pop();
        }
        lower.push(*point);
    }
    let mut upper: Vec<Pt> = Vec::new();
    for point in sorted.iter().rev() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], *point) <= 0.0
        {
            upper.pop();
        }
        upper.push(*point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn compare_points(a: &Pt, b: &Pt) -> std::cmp::Ordering {
    a.x.partial_cmp(&b.x)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
}

fn cross(origin: Pt, a: Pt, b: Pt) -> f64 {
    (a.x - origin.x) * (b.y - origin.y) - (a.y - origin.y) * (b.x - origin.x)
}

/// The smallest-area rectangle around `points` (hull + rotating calipers), returned as four corners
/// clockwise from the top-left corner. A degenerate (collinear) input gives a zero-area rectangle
/// lying on the line, which the box filter then refuses.
///
/// This is the float form of `MinAreaRect` on purpose: the crop is the min-area rectangle of the
/// UNCLIPPED offset polygon (OCR-F1), which is f64.
#[must_use]
pub fn min_area_rect_pts(points: &[Pt]) -> Quad {
    let hull = convex_hull_pts(points);
    if hull.len() < 2 {
        let corner = hull.first().copied().unwrap_or(Pt { x: 0.0, y: 0.0 });
        return Quad([corner; 4]);
    }

    let mut best: Option<(f64, [Pt; 4])> = None;
    for index in 0..hull.len() {
        let a = hull[index];
        let b = hull[(index + 1) % hull.len()];
        let (edge_x, edge_y) = (b.x - a.x, b.y - a.y);
        let length = (edge_x * edge_x + edge_y * edge_y).sqrt();
        if length <= f64::EPSILON {
            continue;
        }
        let (ux, uy) = (edge_x / length, edge_y / length);
        let (vx, vy) = (-uy, ux);
        let mut min_u = f64::INFINITY;
        let mut max_u = f64::NEG_INFINITY;
        let mut min_v = f64::INFINITY;
        let mut max_v = f64::NEG_INFINITY;
        for point in &hull {
            let u = point.x * ux + point.y * uy;
            let v = point.x * vx + point.y * vy;
            min_u = min_u.min(u);
            max_u = max_u.max(u);
            min_v = min_v.min(v);
            max_v = max_v.max(v);
        }
        let area = (max_u - min_u) * (max_v - min_v);
        let to_world = |u: f64, v: f64| Pt {
            x: u * ux + v * vx,
            y: u * uy + v * vy,
        };
        let corners = [
            to_world(min_u, min_v),
            to_world(max_u, min_v),
            to_world(max_u, max_v),
            to_world(min_u, max_v),
        ];
        if best.as_ref().is_none_or(|(best_area, _)| area < *best_area) {
            best = Some((area, corners));
        }
    }

    let (_, corners) = best.unwrap_or((0.0, [hull[0]; 4]));
    Quad(order_corners(corners))
}

/// [`min_area_rect_pts`] for a traced contour's pixel indices.
#[must_use]
pub fn min_area_rect(points: &[(i32, i32)]) -> Quad {
    let points: Vec<Pt> = points
        .iter()
        .map(|(x, y)| Pt {
            x: f64::from(*x),
            y: f64::from(*y),
        })
        .collect();
    min_area_rect_pts(&points)
}

/// Clockwise in image coordinates from the TOP-LEFT corner: `PaddleOCR`'s `get_mini_boxes` order, not
/// "start at the top-most corner". The two differ the moment a box is tilted a degree or two
/// counter-clockwise (its right end higher): the top-most corner is then the top-RIGHT one, and
/// starting there swaps the quad's width and height, which hands the recogniser a glyph rotated 90°
/// (OCR-F4). Sorting by x first and pairing the two left corners (smaller y = top-left) and the two
/// right ones (smaller y = top-right) makes the horizontal edges `0→1` and `3→2` for every tilt.
fn order_corners(mut corners: [Pt; 4]) -> [Pt; 4] {
    corners.sort_by(compare_points);
    let (bl, tl) = if corners[0].y <= corners[1].y {
        (corners[1], corners[0])
    } else {
        (corners[0], corners[1])
    };
    let (br, tr) = if corners[2].y <= corners[3].y {
        (corners[3], corners[2])
    } else {
        (corners[2], corners[3])
    };
    [tl, tr, br, bl]
}

/// The mean score over the pixels whose centres fall inside `polygon` (the `FillPoly` + `Mean`
/// approximation; `OpenCV`'s fill rule for integer polygons is boundary-inclusive and is called out in
/// the report as an ambiguity).
#[must_use]
pub fn filled_mean(map: &crate::detmask::ScoreMap, polygon: &[Pt]) -> f64 {
    filled_mean_scanned(map, polygon).0
}

/// [`filled_mean`] with the number of candidate pixels the scan visited. The scan is bounded by the
/// polygon's clamped integer bounding box (reference `box_score_fast`), so it is O(box) rather than
/// O(score map) per contour (OCR-F2); the count is what the regression test asserts and what a
/// latency investigation can attribute work to.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the polygon scan runs over the score map's own pixel grid: its dimensions are exactly representable in f64 and the floor/ceil of a bounded, finite coordinate is a non-negative index below the map size"
)]
pub fn filled_mean_scanned(map: &crate::detmask::ScoreMap, polygon: &[Pt]) -> (f64, usize) {
    let coords: Vec<(f64, f64)> = polygon.iter().map(|point| (point.x, point.y)).collect();
    if coords.len() < 3
        || map.width == 0
        || map.height == 0
        || coords.iter().any(|(x, y)| !x.is_finite() || !y.is_finite())
    {
        return (0.0, 0);
    }
    let min_x = coords
        .iter()
        .map(|point| point.0)
        .fold(f64::INFINITY, f64::min);
    let max_x = coords
        .iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = coords
        .iter()
        .map(|point| point.1)
        .fold(f64::INFINITY, f64::min);
    let max_y = coords
        .iter()
        .map(|point| point.1)
        .fold(f64::NEG_INFINITY, f64::max);
    if max_x < 0.0
        || max_y < 0.0
        || min_x > (map.width - 1) as f64
        || min_y > (map.height - 1) as f64
    {
        return (0.0, 0);
    }
    let x0 = min_x.floor().clamp(0.0, (map.width - 1) as f64) as usize;
    let x1 = max_x.ceil().clamp(0.0, (map.width - 1) as f64) as usize;
    let y0 = min_y.floor().clamp(0.0, (map.height - 1) as f64) as usize;
    let y1 = max_y.ceil().clamp(0.0, (map.height - 1) as f64) as usize;
    let mut sum = 0.0;
    let mut count = 0usize;
    let mut scanned = 0usize;
    for y in y0..=y1 {
        for x in x0..=x1 {
            scanned += 1;
            if point_in_polygon(&coords, x as f64 + 0.5, y as f64 + 0.5) {
                sum += f64::from(map.get(x, y));
                count += 1;
            }
        }
    }
    if count == 0 {
        (0.0, scanned)
    } else {
        (sum / count as f64, scanned)
    }
}

fn point_in_polygon(polygon: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut inside = false;
    let mut j = polygon.len().wrapping_sub(1);
    for i in 0..polygon.len() {
        let (xi, yi) = polygon[i];
        let (xj, yj) = polygon[j];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// `FillPoly` + `Mean`: the box's score in the detector map.
#[must_use]
pub fn box_score(map: &crate::detmask::ScoreMap, quad: &Quad) -> f64 {
    filled_mean(map, &quad.0)
}

/// Minkowski sum of a convex quad with a disk: the Clipper round-join unclip. Returns an ordered
/// polygon (counter-clockwise in image coordinates) sampled with 16 segments per rounded corner.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "the angle sweep is sampled with a small integer segment index, exactly representable in f64"
)]
pub fn offset_round(quad: &Quad, distance: f64) -> Vec<Pt> {
    const ARC_SEGMENTS: usize = 16;

    let mut polygon: Vec<Pt> = quad.0.to_vec();
    if distance <= 0.0 {
        return polygon;
    }
    // Work counter-clockwise in image coordinates (negative shoelace for y-down? we normalise by
    // area sign: outward offsets must increase the area).
    let coords: Vec<(f64, f64)> = polygon.iter().map(|point| (point.x, point.y)).collect();
    if signed_area(&coords) < 0.0 {
        polygon.reverse();
    }
    let count = polygon.len();
    let mut offsets: Vec<Pt> = Vec::new();
    // Outward normal of edge i->i+1 is (dy, -dx) for a counter-clockwise polygon in a y-up frame;
    // with image coordinates the sign works out to (dy, -dx) as well for the normalised winding we
    // compute by checking the area sign above.
    let normals: Vec<Pt> = (0..count)
        .map(|index| {
            let a = polygon[index];
            let b = polygon[(index + 1) % count];
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length = (dx * dx + dy * dy).sqrt().max(f64::EPSILON);
            Pt {
                x: dy / length,
                y: -dx / length,
            }
        })
        .collect();
    for index in 0..count {
        let previous_normal = normals[(index + count - 1) % count];
        let next_normal = normals[index];
        let corner = polygon[index];
        let start_angle = previous_normal.y.atan2(previous_normal.x);
        let end_angle = next_normal.y.atan2(next_normal.x);
        let mut sweep = end_angle - start_angle;
        while sweep > std::f64::consts::PI {
            sweep -= std::f64::consts::TAU;
        }
        while sweep < -std::f64::consts::PI {
            sweep += std::f64::consts::TAU;
        }
        for step in 0..=ARC_SEGMENTS {
            let angle = start_angle + sweep * (step as f64 / ARC_SEGMENTS as f64);
            offsets.push(Pt {
                x: corner.x + distance * angle.cos(),
                y: corner.y + distance * angle.sin(),
            });
        }
    }
    offsets
}

/// A detected box: its quad and the detector-map score.
#[derive(Debug, Clone, PartialEq)]
pub struct DetBox {
    pub quad: Quad,
    pub score: f64,
}

/// `MinAreaRect` + box score + the `box_thresh` filter: `None` for a degenerate quad or a low score.
#[must_use]
pub fn accept_box(map: &crate::detmask::ScoreMap, quad: &Quad, box_thresh: f64) -> Option<DetBox> {
    let coords: Vec<(f64, f64)> = quad.0.iter().map(|point| (point.x, point.y)).collect();
    if contour_area(&coords) < 1e-6 {
        return None;
    }
    let score = box_score(map, quad);
    (score >= box_thresh).then_some(DetBox { quad: *quad, score })
}

/// `ContourArea` + `ArcLength` + unclip: `None` when the quad is degenerate or the offset polygon
/// self-intersects (the C# Clipper result for such a box is refused by the caller).
#[must_use]
pub fn unclip_quad(quad: &Quad, unclip_ratio: f64) -> Option<Vec<Pt>> {
    let coords: Vec<(f64, f64)> = quad.0.iter().map(|point| (point.x, point.y)).collect();
    let area = contour_area(&coords);
    if area < 1e-6 {
        return None;
    }
    let perimeter = arc_length(&coords, true);
    let distance = unclip_distance(area, perimeter, unclip_ratio);
    let offset = offset_round(quad, distance);
    is_simple_polygon(&offset).then_some(offset)
}

/// `DetPostprocess._minSize`: a min-area rectangle whose short side is below this is a speck, not a
/// glyph.
pub const MIN_BOX_SIDE: f64 = 3.0;
/// The post-unclip floor, `_minSize + 2`: DB predicts a SHRUNK text kernel, so the unclipped box is
/// the one the recogniser may see and a still-tiny result is refused.
pub const MIN_UNCLIPPED_SIDE: f64 = MIN_BOX_SIDE + 2.0;

/// The DB post-processing that turns one traced contour into the crop quad (`PaddleOCR`
/// `boxes_from_bitmap`, OCR-F1):
///
/// 1. `MinAreaRect` the contour and refuse a short side below [`MIN_BOX_SIDE`];
/// 2. score it with [`box_score`] and refuse below `box_thresh`;
/// 3. unclip the box by `unclip_ratio` (round join) — this is what restores the glyph extent DB's
///    shrink lost, so the crop is NOT the raw min rect;
/// 4. take the `MinAreaRect` of the offset polygon, and refuse it below [`MIN_UNCLIPPED_SIDE`].
///
/// `None` means "drop this contour" and is not an error.
#[must_use]
pub fn contour_crop_quad(
    map: &crate::detmask::ScoreMap,
    contour: &[(i32, i32)],
    box_thresh: f64,
    unclip_ratio: f64,
) -> Option<Quad> {
    if contour.len() < 3 {
        return None;
    }
    let shrunk = min_area_rect(contour);
    if quad_short_side(&shrunk) < MIN_BOX_SIDE {
        return None;
    }
    let coords: Vec<(f64, f64)> = shrunk.0.iter().map(|point| (point.x, point.y)).collect();
    if contour_area(&coords) < 1.0 || box_score(map, &shrunk) < box_thresh {
        return None;
    }
    let offset = unclip_quad(&shrunk, unclip_ratio)?;
    let expanded = min_area_rect_pts(&offset);
    (quad_short_side(&expanded) >= MIN_UNCLIPPED_SIDE).then_some(expanded)
}

/// True when no two non-adjacent edges of the closed polygon intersect.
#[must_use]
pub fn is_simple_polygon(points: &[Pt]) -> bool {
    let count = points.len();
    if count < 3 {
        return false;
    }
    for first in 0..count {
        let first_next = (first + 1) % count;
        for second in (first + 1)..count {
            let second_next = (second + 1) % count;
            if first == second || first_next == second || second_next == first {
                continue;
            }
            if segments_intersect(
                points[first],
                points[first_next],
                points[second],
                points[second_next],
            ) {
                return false;
            }
        }
    }
    true
}

fn segments_intersect(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let orientation = |p: Pt, q: Pt, r: Pt| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
    let o1 = orientation(a, b, c);
    let o2 = orientation(a, b, d);
    let o3 = orientation(c, d, a);
    let o4 = orientation(c, d, b);
    (o1 * o2 < 0.0) && (o3 * o4 < 0.0)
}

/// The C# `SortedBoxes`: stable Y sort, line ids advancing when the Y gap is >= 10 px, then stable
/// sort by (line, first-point X).
pub fn sort_boxes(boxes: &mut [DetBox]) {
    let mut entries: Vec<(Quad, DetBox)> =
        boxes.iter().map(|box_| (box_.quad, box_.clone())).collect();
    sort_quads(&mut entries);
    for (slot, (_, box_)) in boxes.iter_mut().zip(entries) {
        *slot = box_;
    }
}

/// [`sort_boxes`] over `(quad, value)` pairs: the reference order is a property of the quads, so the
/// recogniser's items can be sorted by the same rule without copying them into [`DetBox`]es (OCR-F5).
/// Stable throughout: equal keys keep their input order.
pub fn sort_quads<T>(entries: &mut Vec<(Quad, T)>) {
    entries.sort_by(|(a, _), (b, _)| {
        a.0[0]
            .y
            .partial_cmp(&b.0[0].y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut line_ids = vec![0usize; entries.len()];
    for index in 1..entries.len() {
        let gap = entries[index].0.0[0].y - entries[index - 1].0.0[0].y;
        line_ids[index] = line_ids[index - 1] + usize::from(gap >= 10.0);
    }
    let mut indexed: Vec<(usize, (Quad, T))> =
        line_ids.into_iter().zip(entries.drain(..)).collect();
    indexed.sort_by(|(line_a, (a, _)), (line_b, (b, _))| {
        line_a.cmp(line_b).then(
            a.0[0]
                .x
                .partial_cmp(&b.0[0].x)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    entries.extend(indexed.into_iter().map(|(_, entry)| entry));
}

/// Distance helper used by tests/reporting.
#[must_use]
pub fn quad_perimeter(quad: &Quad) -> f64 {
    arc_length(
        &quad
            .0
            .iter()
            .map(|point| (point.x, point.y))
            .collect::<Vec<_>>(),
        true,
    )
}

/// The quad's width: the mean of its two horizontal edge lengths. The corner ORDER matters — see
/// [`order_corners`] (top-left first makes `0→1` and `3→2` the horizontal edges).
#[must_use]
pub fn quad_width(quad: &Quad) -> f64 {
    f64::midpoint(
        distance_pt(quad.0[0], quad.0[1]),
        distance_pt(quad.0[3], quad.0[2]),
    )
}

/// The quad's height: the mean of its two vertical edge lengths.
#[must_use]
pub fn quad_height(quad: &Quad) -> f64 {
    f64::midpoint(
        distance_pt(quad.0[1], quad.0[2]),
        distance_pt(quad.0[0], quad.0[3]),
    )
}

/// The shorter side of the quad: what `MinAreaRect`'s two-element size would be `min()` of, and the
/// value the `min_size` filters compare.
#[must_use]
pub fn quad_short_side(quad: &Quad) -> f64 {
    quad_width(quad).min(quad_height(quad))
}

/// Corner-to-corner distance (diagnostics only).
#[must_use]
pub fn quad_diagonal(quad: &Quad) -> f64 {
    distance_pt(quad.0[0], quad.0[2])
}

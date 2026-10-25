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

/// Four ordered corners, clockwise in image coordinates starting at the top-most/left-most corner
/// (the order `FindContours` + row sort expect: the first point's Y is the sort key).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad(pub [Pt; 4]);

/// Shoelace area (absolute).
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
pub fn unclip_distance(area: f64, perimeter: f64, unclip_ratio: f64) -> f64 {
    if perimeter <= 0.0 {
        return 0.0;
    }
    area * unclip_ratio / perimeter
}

/// OpenCV `ContourArea` sign helper for a traced contour (positive = counter-clockwise in image
/// coordinates, which is what the contour tracer normalises outer boundaries to).
pub fn polygon_is_counter_clockwise(points: &[(f64, f64)]) -> bool {
    signed_area(points) > 0.0
}

/// Monotonic-chain convex hull, collinear points removed. Returns points in counter-clockwise order.
pub fn convex_hull(points: &[(i32, i32)]) -> Vec<(f64, f64)> {
    let mut sorted: Vec<(i32, i32)> = points.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.len() <= 2 {
        return sorted.into_iter().map(cast).collect();
    }
    let mut lower: Vec<(i32, i32)> = Vec::new();
    for point in &sorted {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], *point) <= 0
        {
            lower.pop();
        }
        lower.push(*point);
    }
    let mut upper: Vec<(i32, i32)> = Vec::new();
    for point in sorted.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], *point) <= 0
        {
            upper.pop();
        }
        upper.push(*point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower.into_iter().map(cast).collect()
}

fn cast(point: (i32, i32)) -> (f64, f64) {
    (point.0 as f64, point.1 as f64)
}

fn cross(origin: (i32, i32), a: (i32, i32), b: (i32, i32)) -> i64 {
    (a.0 as i64 - origin.0 as i64) * (b.1 as i64 - origin.1 as i64)
        - (a.1 as i64 - origin.1 as i64) * (b.0 as i64 - origin.0 as i64)
}

/// The smallest-area rectangle around `points` (hull + rotating calipers), returned as four corners
/// clockwise from the top-most/left-most corner. A degenerate (collinear) input gives a zero-area
/// rectangle lying on the line, which the box filter then refuses.
pub fn min_area_rect(points: &[(i32, i32)]) -> Quad {
    let hull = convex_hull(points);
    if hull.len() < 2 {
        let point = hull.first().copied().unwrap_or((0.0, 0.0));
        let corner = Pt {
            x: point.0,
            y: point.1,
        };
        return Quad([corner; 4]);
    }

    let mut best: Option<(f64, [Pt; 4])> = None;
    for index in 0..hull.len() {
        let (ax, ay) = hull[index];
        let (bx, by) = hull[(index + 1) % hull.len()];
        let (edge_x, edge_y) = (bx - ax, by - ay);
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
        for (px, py) in &hull {
            let u = px * ux + py * uy;
            let v = px * vx + py * vy;
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

    let (_, corners) = best.unwrap_or_else(|| {
        let point = Pt {
            x: hull[0].0,
            y: hull[0].1,
        };
        (0.0, [point; 4])
    });
    Quad(order_corners(corners))
}

/// Clockwise from the top-most (then left-most) corner in image coordinates.
fn order_corners(corners: [Pt; 4]) -> [Pt; 4] {
    let centre = Pt {
        x: corners.iter().map(|point| point.x).sum::<f64>() / 4.0,
        y: corners.iter().map(|point| point.y).sum::<f64>() / 4.0,
    };
    // Sort by angle around the centre: that is a simple (non-self-intersecting) cycle for a convex
    // rectangle, which is all MinAreaRect ever returns.
    let mut ordered: Vec<Pt> = corners.to_vec();
    ordered.sort_by(|a, b| {
        let angle_a = (a.y - centre.y).atan2(a.x - centre.x);
        let angle_b = (b.y - centre.y).atan2(b.x - centre.x);
        angle_a
            .partial_cmp(&angle_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    // Rotate to start at the top-most (then left-most) corner; keep the angle order (clockwise in
    // image coordinates because y grows downward).
    if let Some(start) = ordered
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.y.partial_cmp(&b.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        })
        .map(|(index, _)| index)
    {
        ordered.rotate_left(start);
    }
    [ordered[0], ordered[1], ordered[2], ordered[3]]
}

/// The mean score over the pixels whose centres fall inside `polygon` (the `FillPoly` + `Mean`
/// approximation; OpenCV's fill rule for integer polygons is boundary-inclusive and is called out in
/// the report as an ambiguity).
pub fn filled_mean(map: &crate::detmask::ScoreMap, polygon: &[Pt]) -> f64 {
    let coords: Vec<(f64, f64)> = polygon.iter().map(|point| (point.x, point.y)).collect();
    let mut sum = 0.0;
    let mut count = 0usize;
    for y in 0..map.height {
        for x in 0..map.width {
            if point_in_polygon(&coords, x as f64 + 0.5, y as f64 + 0.5) {
                sum += f64::from(map.get(x, y));
                count += 1;
            }
        }
    }
    if count == 0 { 0.0 } else { sum / count as f64 }
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
pub fn box_score(map: &crate::detmask::ScoreMap, quad: &Quad) -> f64 {
    filled_mean(map, &quad.0)
}

/// Minkowski sum of a convex quad with a disk: the Clipper round-join unclip. Returns an ordered
/// polygon (counter-clockwise in image coordinates) sampled with 16 segments per rounded corner.
pub fn offset_round(quad: &Quad, distance: f64) -> Vec<Pt> {
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
    const ARC_SEGMENTS: usize = 16;
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

/// `MinAreaRect` + box score + the box_thresh filter: `None` for a degenerate quad or a low score.
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

/// True when no two non-adjacent edges of the closed polygon intersect.
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
    boxes.sort_by(|a, b| {
        a.quad.0[0]
            .y
            .partial_cmp(&b.quad.0[0].y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut line_ids = vec![0usize; boxes.len()];
    for index in 1..boxes.len() {
        let gap = boxes[index].quad.0[0].y - boxes[index - 1].quad.0[0].y;
        line_ids[index] = line_ids[index - 1] + usize::from(gap >= 10.0);
    }
    let mut indexed: Vec<(usize, DetBox)> =
        line_ids.into_iter().zip(boxes.iter().cloned()).collect();
    indexed.sort_by(|(line_a, box_a), (line_b, box_b)| {
        line_a.cmp(line_b).then(
            box_a.quad.0[0]
                .x
                .partial_cmp(&box_b.quad.0[0].x)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    for (slot, (_, box_)) in boxes.iter_mut().zip(indexed) {
        *slot = box_;
    }
}

/// Distance helper used by tests/reporting.
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

/// Corner-to-corner distance (diagnostics only).
pub fn quad_diagonal(quad: &Quad) -> f64 {
    distance_pt(quad.0[0], quad.0[2])
}

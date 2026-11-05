//! The detector's binary map: threshold, 2×2 dilate, and the `FindContours` equivalent.
//!
//! Constants and call shapes are `DetPreprocess.cs`/`DetPostprocess.cs` as recorded in
//! `OCR-RESEARCH.md` §3. Two `OpenCV` details are documented rather than guessed:
//!
//! - `THRESH_BINARY` is **strictly greater** than the threshold (`src > thresh`), not `>=`.
//! - The 2×2 dilation uses `OpenCV`'s default anchor for a 2×2 kernel, `(1,1)`, so the window ends at
//!   the output pixel. A 0-anchored kernel would shift every box by one pixel.
//!
//! # Contour order
//!
//! Boundary tracing is a crack walk: every directed grid edge between a component pixel and a
//! non-component pixel is followed with the component on the right, so one cycle is one closed
//! boundary. A component's cycles are emitted **largest area first** (the outer boundary before its
//! holes), components themselves in raster order of their first pixel — the `RETR_LIST` reading that
//! matters for the row sort. `OpenCV`'s exact diagonal 8-connectivity tie-breaking on self-touching
//! contours is not reproduced; that is the documented ambiguity.
//!
//! The walk keeps its directed edges in a sorted `Vec` (by start vertex) and each component's pixels
//! in a `Vec`, with the component label image as the inside test. A per-start `HashMap` used to
//! overwrite one of the two edges a pinch vertex can own, cutting the boundary into open chains, and
//! a per-pixel `BTreeSet` membership test made the walk O(boundary · log pixels) (OCR-F16).
//!
//! Orientation follows `OpenCV`'s `contourArea` sign convention: outer boundaries are
//! counter-clockwise (positive area under `OpenCV`'s convention; the opposite shoelace sign in image
//! coordinates) and holes the other way, which is what `detgeom::polygon_is_counter_clockwise`
//! exposes.

/// A detector output: one f32 probability per pixel, row-major `[height, width]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreMap {
    pub width: usize,
    pub height: usize,
    pub scores: Vec<f32>,
}

impl ScoreMap {
    /// # Panics
    ///
    /// Panics when `scores.len()` is not `width * height`.
    #[must_use]
    pub fn new(width: usize, height: usize, scores: Vec<f32>) -> Self {
        assert_eq!(scores.len(), width * height, "score map size");
        Self {
            width,
            height,
            scores,
        }
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        self.scores[y * self.width + x]
    }
}

/// A binary map (1 = foreground).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    pub width: usize,
    pub height: usize,
    bits: Vec<bool>,
}

impl Mask {
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            bits: vec![false; width * height],
        }
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.bits[y * self.width + x]
    }

    pub fn set(&mut self, x: usize, y: usize, value: bool) {
        self.bits[y * self.width + x] = value;
    }
}

/// `Cv2.Threshold(..., thresh, 255, Binary)`: strictly greater becomes foreground.
#[must_use]
pub fn threshold(map: &ScoreMap, thresh: f32) -> Mask {
    Mask {
        width: map.width,
        height: map.height,
        bits: map.scores.iter().map(|score| *score > thresh).collect(),
    }
}

/// `Cv2.Dilate` with a 2×2 rectangle kernel and `OpenCV`'s default anchor `(1,1)`: the window covering
/// input `(x-1..x, y-1..y)` ends at the output pixel.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the mask's pixel coordinates come from the detector map, whose dimensions fit i32, and every i32-to-usize conversion is guarded by a non-negative check"
)]
pub fn dilate(mask: &Mask) -> Mask {
    let mut grown = Mask::new(mask.width, mask.height);
    for y in 0..mask.height {
        for x in 0..mask.width {
            let mut set = mask.get(x, y);
            for (dx, dy) in [(-1i32, 0i32), (-1, -1), (0, -1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0
                    && ny >= 0
                    && (nx as usize) < mask.width
                    && (ny as usize) < mask.height
                    && mask.get(nx as usize, ny as usize)
                {
                    set = true;
                }
            }
            grown.set(x, y, set);
        }
    }
    grown
}

/// One traced boundary: pixel coordinates, (x, y), in tracing order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contour {
    pub points: Vec<(i32, i32)>,
}

/// One foreground component: its label in the label image, and its pixels in discovery order.
type Cluster = (usize, Vec<(i32, i32)>);

/// One directed boundary edge between two grid vertices, with a component pixel on its RIGHT.
type Edge = ((i32, i32), (i32, i32));

#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the mask's pixel coordinates come from the detector map, whose dimensions fit i32, and every i32-to-usize conversion is preceded by a non-negative check"
)]
pub fn find_contours(mask: &Mask) -> Vec<Contour> {
    // 1. 4-connected foreground components, raster order of their first pixel, on a label image that
    //    the boundary walk then uses as its inside test (no per-pixel set membership, OCR-F16).
    let mut labels = vec![usize::MAX; mask.width * mask.height];
    let mut components: Vec<Cluster> = Vec::new();
    for y in 0..mask.height {
        for x in 0..mask.width {
            let index = y * mask.width + x;
            if !mask.bits[index] || labels[index] != usize::MAX {
                continue;
            }
            let label = components.len();
            let mut pixels = Vec::new();
            let mut stack = vec![(x as i32, y as i32)];
            while let Some((px, py)) = stack.pop() {
                if px < 0 || py < 0 || px as usize >= mask.width || py as usize >= mask.height {
                    continue;
                }
                let pixel_index = py as usize * mask.width + px as usize;
                if !mask.bits[pixel_index] || labels[pixel_index] != usize::MAX {
                    continue;
                }
                labels[pixel_index] = label;
                pixels.push((px, py));
                stack.push((px - 1, py));
                stack.push((px + 1, py));
                stack.push((px, py - 1));
                stack.push((px, py + 1));
            }
            components.push((label, pixels));
        }
    }

    // 2. Each component's boundary cycles via crack following; largest first (outer then holes).
    let mut contours = Vec::new();
    for (label, pixels) in &components {
        let mut cycles = boundary_cycles(mask, &labels, *label, pixels);
        cycles.sort_by(|a, b| {
            let area_a = polygon_area(a);
            let area_b = polygon_area(b);
            area_b
                .abs()
                .partial_cmp(&area_a.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (index, cycle) in cycles.into_iter().enumerate() {
            if cycle.len() < 2 && pixels.len() > 1 {
                continue;
            }
            let mut points = cycle;
            let sum = shoelace(&points);
            if index == 0 {
                // Outer boundary: counter-clockwise under OpenCV's convention, i.e. negative
                // shoelace in image coordinates.
                if sum > 0.0 {
                    points.reverse();
                }
            } else if sum < 0.0 {
                points.reverse(); // holes the other way
            }
            // The first point is the scan-first (top-most, then left-most) boundary pixel.
            if let Some(offset) = points
                .iter()
                .enumerate()
                .min_by_key(|(_, (x, y))| (*y, *x))
                .map(|(index, _)| index)
            {
                points.rotate_left(offset);
            }
            contours.push(Contour { points });
        }
    }
    contours
}

fn shoelace(points: &[(i32, i32)]) -> f64 {
    let mut sum = 0.0;
    for index in 0..points.len() {
        let (x1, y1) = points[index];
        let (x2, y2) = points[(index + 1) % points.len()];
        sum += f64::from(x1) * f64::from(y2) - f64::from(x2) * f64::from(y1);
    }
    sum / 2.0
}

fn polygon_area(points: &[(i32, i32)]) -> f64 {
    shoelace(points)
}

/// All closed boundary cycles of one labeled component, walking directed grid edges with the
/// component on the right. The edges live in a sorted `Vec` (start vertex, then end vertex), so a
/// pinch vertex's two outgoing edges are both kept: a per-start `HashMap` overwrote one of them and
/// cut the boundary into open chains. Each edge is visited exactly once, and the inside test is the
/// label image, which makes the walk O(boundary · log boundary) rather than O(boundary · pixels)
/// (OCR-F16).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "pixels come from the detector map, whose dimensions fit i32; every i32-to-usize conversion here is behind a non-negative bounds check"
)]
fn boundary_cycles(
    mask: &Mask,
    labels: &[usize],
    label: usize,
    pixels: &[(i32, i32)],
) -> Vec<Vec<(i32, i32)>> {
    let inside = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let (x, y) = (x as usize, y as usize);
        x < mask.width && y < mask.height && labels[y * mask.width + x] == label
    };

    // Directed edges (vertex -> vertex) with a component pixel on the right.
    let mut edges: Vec<Edge> = Vec::new();
    for &(x, y) in pixels {
        if !inside(x, y - 1) {
            edges.push(((x, y), (x + 1, y))); // top edge, east; the component is south (right)
        }
        if !inside(x + 1, y) {
            edges.push(((x + 1, y), (x + 1, y + 1))); // right edge, south
        }
        if !inside(x, y + 1) {
            edges.push(((x + 1, y + 1), (x, y + 1))); // bottom edge, west
        }
        if !inside(x - 1, y) {
            edges.push(((x, y + 1), (x, y))); // left edge, north
        }
    }
    edges.sort_unstable();

    let mut visited = vec![false; edges.len()];
    let mut cycles = Vec::new();
    let mut cursor = 0usize;
    while cursor < edges.len() {
        let start_edge = cursor;
        let mut edge_index = start_edge;
        let mut points: Vec<(i32, i32)> = Vec::new();
        loop {
            visited[edge_index] = true;
            let vertex = edges[edge_index].0;
            let next = edges[edge_index].1;
            let step = (next.0 - vertex.0, next.1 - vertex.1);
            // The component pixel on the right of the directed edge.
            let pixel = match step {
                (1, 0) => (vertex.0, vertex.1),          // east: south cell
                (0, 1) => (vertex.0 - 1, vertex.1),      // south: west cell
                (-1, 0) => (vertex.0 - 1, vertex.1 - 1), // west: north cell
                (0, -1) => (vertex.0, vertex.1 - 1),     // north: east cell
                _ => (vertex.0, vertex.1),
            };
            if points.last() != Some(&pixel) {
                points.push(pixel);
            }
            let Some(found) = next_edge_by_turn(&edges, next, step) else {
                break;
            };
            if found == start_edge || visited[found] {
                break;
            }
            edge_index = found;
        }
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        if !points.is_empty() {
            cycles.push(points);
        }
        while cursor < edges.len() && visited[cursor] {
            cursor += 1;
        }
    }
    cycles
}

/// The outgoing edge that keeps the walk on the same face: the next one CLOCKWISE from the reverse of
/// the incoming direction (the standard planar face traversal). With one outgoing edge it is that
/// edge; at a pinch vertex, where two leave, this is what keeps the two loops apart instead of
/// hopping between them (OCR-F16).
fn next_edge_by_turn(edges: &[Edge], vertex: (i32, i32), incoming: (i32, i32)) -> Option<usize> {
    let reverse_rank = direction_rank(-incoming.0, -incoming.1)?;
    let mut best: Option<(usize, usize)> = None;
    let mut index = edges.partition_point(|edge| edge.0 < vertex);
    while index < edges.len() && edges[index].0 == vertex {
        let step = (edges[index].1.0 - vertex.0, edges[index].1.1 - vertex.1);
        if let Some(rank) = direction_rank(step.0, step.1) {
            // Clockwise distance from the reverse of the incoming direction; a full turn is last.
            let offset = match (rank + 4 - reverse_rank) % 4 {
                0 => 4,
                other => other,
            };
            if best.is_none_or(|(current, _)| offset < current) {
                best = Some((offset, index));
            }
        }
        index += 1;
    }
    best.map(|(_, index)| index)
}

/// One of the four grid directions, ranked clockwise on screen (y down): north, east, south, west.
fn direction_rank(dx: i32, dy: i32) -> Option<usize> {
    match (dx, dy) {
        (0, -1) => Some(0),
        (1, 0) => Some(1),
        (0, 1) => Some(2),
        (-1, 0) => Some(3),
        _ => None,
    }
}

//! The `ocr.scan` modes: `lines`, `count`, `attributes`, `grade` (and `remaining` as a follow-up).
//!
//! Text rules stay in `seal-domain`: attribute sign fixing is [`seal_domain::ocr::fix_sign`] and line
//! text was already cleaned by the pipeline's `TextCleaner`. The v2 call sites this ports:
//! `OcrEngine.ScanOnce` (`v2/SealTools.Core/OcrEngine.cs:273-345`): grade line from the configured
//! region-relative `grade_area` box when present, otherwise the full read Y-filtered; attributes
//! from the X+Y-filtered items grouped by row and classified label/value, `FixSign`, first 3 rows
//! starting at Y >= 10.
//!
//! The grade COLOUR fallback (`DetectGradeColorScores`/`DecideGrade`) and the `remaining` count read
//! are ported (G25): a `grade` scan returns BOTH fields, with the colour fallback used only when the
//! text read found no grade letter. `count` keeps the feeder's highest-confidence digit line; it is
//! a different reader with a different question, not the tuner's remaining band.

use seal_domain::config::AttributesConfig;
use seal_domain::ocr::fix_sign;

use crate::pipeline::{Line, RecognizedLine, Scan};

/// The `mode` argument of `ocr.scan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Lines,
    Count,
    Attributes,
    Grade,
}

impl Mode {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "lines" => Some(Self::Lines),
            "count" => Some(Self::Count),
            "attributes" => Some(Self::Attributes),
            "grade" => Some(Self::Grade),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lines => "lines",
            Self::Count => "count",
            Self::Attributes => "attributes",
            Self::Grade => "grade",
        }
    }
}

/// The v2 `OcrGeometry` bands, read from the config where present. Absent bands mean no filtering,
/// which is v2's behaviour for an empty list (`FilterItems` returns everything when `hasX` is false
/// and the Y range is the whole image); the row height falls back to 10.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrGeometry {
    pub attr_x: Option<(i32, i32)>,
    pub attr_y: Option<(i32, i32)>,
    pub grade_y: Option<(i32, i32)>,
    /// The grade-letter box, REGION-RELATIVE (the same space as every band): v2's
    /// `DetectGradeColorScores(mat, ga)` cropped the captured REGION image with it, and
    /// `ConfigValidator` checks its x2/y2 against `region.width/height`. `None` means absent or
    /// outside the region; the reader then falls back to [`Self::grade_y`].
    pub grade_area: Option<GradeArea>,
    pub remaining_x: Option<(i32, i32)>,
    pub remaining_y: Option<(i32, i32)>,
    pub row_height: u32,
}

/// A rectangle inside the captured OCR region: `x1..x2`, `y1..y2` (half-open, v2's `BoxConfig`).
/// Only constructed for valid, in-region boxes by the core's [`OcrGeometry`] producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GradeArea {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl GradeArea {
    /// Whether a recognized item's box touches this area. Boxes are in the recognized region's
    /// pixels (the pipeline runs on the region capture), so the comparison is direct.
    fn selects(&self, item: &RecognizedLine) -> bool {
        // v2 cropped `Rect(x1, y1, x2 - x1, y2 - y1)`: left/top inclusive, right/bottom exclusive.
        item_max_x(item) > f64::from(self.x1)
            && item_x(item) < f64::from(self.x2)
            && item_max_y(item) > f64::from(self.y1)
            && item_y(item) < f64::from(self.y2)
    }
}

impl Default for OcrGeometry {
    fn default() -> Self {
        Self {
            attr_x: None,
            attr_y: None,
            grade_y: None,
            grade_area: None,
            remaining_x: None,
            remaining_y: None,
            row_height: 10,
        }
    }
}

/// v2 `MeasureLineSpacing`: the measured attribute line pitch, in pixels.
///
/// Collapse the boxes' top Ys closer than 3 px into one line (one glyph can be split into several
/// boxes at nearly the same Y), then take the MEDIAN gap between consecutive lines. `None` when
/// fewer than two lines were seen, which is v2's own answer and makes the caller keep the configured
/// `row_height` rather than invent one.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "item tops are capture pixel coordinates, far inside i32's range; the cast only rounds the measured Y"
)]
pub fn measure_line_spacing(items: &[RecognizedLine]) -> Option<u32> {
    let mut collapsed: Vec<i32> = Vec::new();
    let mut tops: Vec<i32> = items
        .iter()
        .map(|item| item_y(item).round() as i32)
        .collect();
    tops.sort_unstable();
    for y in tops {
        if collapsed.last().is_none_or(|previous| y - previous > 3) {
            collapsed.push(y);
        }
    }
    if collapsed.len() < 2 {
        return None;
    }
    let mut gaps: Vec<i32> = collapsed.windows(2).map(|pair| pair[1] - pair[0]).collect();
    gaps.sort_unstable();
    u32::try_from(gaps[gaps.len() / 2]).ok()
}

/// The protocol's one result shape (`PROTOCOL.md` §4): absent fields are `None`/null.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModeResult {
    pub lines: Vec<String>,
    pub attributes: Option<Vec<Vec<String>>>,
    pub grade: Option<String>,
    pub remaining: Option<u32>,
}

/// The grade box's pixels, for the COLOUR fallback (G25). `rgba` is the cropped
/// `tuner.ocr.grade_area` image; `None` when the caller has no pixels (OCR fixtures, unit tests),
/// which keeps the text grade as the only source.
#[derive(Debug, Clone, Copy)]
pub struct GradeImage<'a> {
    pub rgba: &'a [u8],
    pub width: usize,
    pub height: usize,
    pub colors: &'a seal_domain::config::GradeColorsConfig,
}

/// Applies one mode to a scan. `attributes` is the active dictionary (its `negative_attr_markers`
/// feed the domain's sign fix).
#[must_use]
pub fn apply(
    mode: Mode,
    scan: &Scan,
    attributes: &AttributesConfig,
    geometry: &OcrGeometry,
) -> ModeResult {
    apply_with_image(mode, scan, attributes, geometry, None)
}

/// [`apply`] plus the grade image, so `mode:"grade"` can use v2's COLOUR fallback when the text
/// read found no grade letter. `remaining` is read in the same pass (v2's `ScanOnce` returned both).
#[must_use]
pub fn apply_with_image(
    mode: Mode,
    scan: &Scan,
    attributes: &AttributesConfig,
    geometry: &OcrGeometry,
    image: Option<GradeImage<'_>>,
) -> ModeResult {
    let lines: Vec<String> = scan.lines.iter().map(|line| line.text.clone()).collect();
    match mode {
        Mode::Lines => ModeResult {
            lines,
            ..ModeResult::default()
        },
        Mode::Count => ModeResult {
            lines,
            remaining: feeder_count(&scan.lines),
            ..ModeResult::default()
        },
        Mode::Attributes => ModeResult {
            lines,
            attributes: Some(read_attributes(&scan.items, attributes, geometry)),
            remaining: None,
            ..ModeResult::default()
        },
        Mode::Grade => {
            let grade = read_grade(&scan.items, geometry).or_else(|| {
                let image = image?;
                let scores = seal_domain::tools::tuner::grade_color_scores(
                    image.rgba,
                    image.width,
                    image.height,
                    image.colors,
                );
                seal_domain::tools::tuner::decide_grade_colour(&scores, image.colors)
            });
            ModeResult {
                lines,
                grade,
                remaining: read_remaining(&scan.items, geometry),
                ..ModeResult::default()
            }
        }
    }
}

/// `FeederCount` rule: the best (highest-confidence) line that contains digits, digits taken in
/// order. The v2 tool reader's exact preference order is a follow-up; this is the documented first
/// pass and is pinned by unit tests.
pub fn feeder_count(lines: &[Line]) -> Option<u32> {
    let mut candidates: Vec<&Line> = lines.iter().filter(|line| has_digit(&line.text)).collect();
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let digits: String = candidates
        .first()?
        .text
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// v2's remaining-spring count (`ScanOnce` step 5): the remaining X/Y band, confidence > 0.4, the
/// first digit run 1..=99999. The X band is optional; when absent the whole width is used.
pub fn read_remaining(items: &[RecognizedLine], geometry: &OcrGeometry) -> Option<u32> {
    for item in items {
        let y = item_y(item);
        if !in_band(y, geometry.remaining_y) {
            continue;
        }
        if let Some((left, right)) = geometry.remaining_x
            && (item_max_x(item) < f64::from(left) || item_x(item) > f64::from(right))
        {
            continue;
        }
        if item.confidence <= 0.4 {
            continue;
        }
        let digits: String = item
            .text
            .trim()
            .chars()
            .skip_while(|character| !character.is_ascii_digit())
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(number) = digits.parse::<u32>()
            && (1..=99_999).contains(&number)
        {
            return Some(number);
        }
    }
    None
}

fn has_digit(text: &str) -> bool {
    text.chars().any(|character| character.is_ascii_digit())
}

/// `DetectGradeFromLine` (`OcrEngine.cs:290-...`): concatenate the grade band's lines uppercase, cut
/// after the last `:`/`：` (or a leading `GRADE`/`等级` label), then take the longest-first match of
/// `SG, XG, DG, G, N`.
pub fn read_grade(items: &[RecognizedLine], geometry: &OcrGeometry) -> Option<String> {
    // v2 used grade_area for its colour fallback; the colour decision is a marked follow-up in
    // v3, so its region-relative box selects the grade line here instead. When the box is absent
    // or invalid the caller leaves it None and today's grade_y band is preserved; if a present box
    // selects nothing (a hand-edited/legacy box), fall back to the band rather than losing the read.
    let mut band: Vec<&RecognizedLine> = match geometry.grade_area {
        Some(area) => items.iter().filter(|item| area.selects(item)).collect(),
        None => items
            .iter()
            .filter(|item| in_band(item_y(item), geometry.grade_y))
            .collect(),
    };
    if band.is_empty() {
        band = items
            .iter()
            .filter(|item| in_band(item_y(item), geometry.grade_y))
            .collect();
    }
    if band.is_empty() {
        band = items.iter().collect();
    }
    // OCR-F5: v2 built the grade line with `BuildLines`, which orders a row left to right. The
    // pipeline now sorts `items` too, but sorting the band here keeps `read_grade` correct for any
    // caller (and for a `Scan` assembled by hand), and makes the read deterministic.
    band.sort_by(|a, b| {
        item_x(a)
            .partial_cmp(&item_x(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    // v2's grade-candidate filters (`OcrEngine.cs:280-288`): confidence > 0.1, the "300" artifact,
    // duplicates collapsed. v2's `length > 1` gate ran on whole built lines; here items are words, so
    // a one-character item is normal (a tight `grade_area` holds just the letter) and the gate's
    // equivalent is the empty-line check below.
    let mut seen: Vec<&str> = Vec::new();
    let mut line = String::new();
    for item in band {
        if item.confidence <= 0.1 || item.text == "300" || seen.contains(&item.text.as_str()) {
            continue;
        }
        seen.push(&item.text);
        line.push_str(&item.text);
    }
    let text = line.to_uppercase();
    if text.is_empty() {
        return None;
    }
    let mut rest = text.as_str();
    if let Some(position) = rest.rfind([':', '：']) {
        rest = &rest[position + 1..];
    } else {
        for label in ["GRADE", "等级"] {
            if let Some(stripped) = rest.strip_prefix(label) {
                rest = stripped;
                break;
            }
        }
    }
    ["SG", "XG", "DG", "G", "N"]
        .into_iter()
        .find(|grade| rest.contains(grade))
        .map(str::to_owned)
}

/// The launcher's attribute read (`OcrEngine.cs:299-331`): X+Y-band filtered, row-bucketed, each
/// token classified label/value, then the domain's `fix_sign`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "row keys are capture pixel-row indices derived from the item Ys: their magnitude is far below f64's exact-integer range and the floor of a non-negative coordinate fits i64"
)]
pub fn read_attributes(
    items: &[RecognizedLine],
    attributes: &AttributesConfig,
    geometry: &OcrGeometry,
) -> Vec<Vec<String>> {
    use std::collections::BTreeMap;
    let row_height = geometry.row_height.max(1);
    let mut rows: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for item in items {
        let y = item_y(item);
        let x = item_x(item);
        if !in_band(y, geometry.attr_y) {
            continue;
        }
        if let Some((left, right)) = geometry.attr_x
            && (item_max_x(item) < f64::from(left) || x > f64::from(right))
        {
            continue;
        }
        if item.confidence < 0.3 || item.text.chars().count() < 2 {
            continue;
        }
        let row = rows
            .entry((y / f64::from(row_height)).floor() as i64)
            .or_default();
        if !row.contains(&item.text) {
            row.push(item.text.clone());
        }
    }

    let mut out = Vec::new();
    for (row_key, row) in rows {
        let y = row_key as f64 * f64::from(row_height);
        if y < 10.0 {
            continue;
        }
        let mut labels = Vec::new();
        let mut values = Vec::new();
        for text in row {
            let has_cjk = !text.is_ascii();
            let has_sign = text.contains('+') || text.contains('-');
            let stripped: String = text
                .replace([',', '.'], "")
                .trim_start_matches(['+', '-'])
                .to_owned();
            let is_number = !stripped.is_empty()
                && stripped.chars().all(|character| character.is_ascii_digit());
            if has_cjk || !(has_sign || (is_number && text.chars().count() <= 3)) {
                labels.push(text);
            } else {
                values.push(text);
            }
        }
        let fixed = fix_sign(&labels, &values, &attributes.negative_attr_markers);
        if !fixed.is_empty() {
            out.push(fixed);
        }
        if out.len() == 3 {
            break;
        }
    }
    out
}

fn in_band(value: f64, band: Option<(i32, i32)>) -> bool {
    match band {
        Some((low, high)) => value >= f64::from(low) && value <= f64::from(high),
        None => true,
    }
}

fn item_y(item: &RecognizedLine) -> f64 {
    item.box_quad
        .0
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min)
}

fn item_x(item: &RecognizedLine) -> f64 {
    item.box_quad
        .0
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min)
}

fn item_max_x(item: &RecognizedLine) -> f64 {
    item.box_quad
        .0
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max)
}

fn item_max_y(item: &RecognizedLine) -> f64 {
    item.box_quad
        .0
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detgeom::{Pt, Quad};

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
            raw_text: text.to_owned(),
            confidence,
            box_quad: quad(x, y),
        }
    }

    fn line(text: &str, confidence: f32) -> Line {
        Line {
            text: text.to_owned(),
            confidence,
            box_quad: quad(0.0, 0.0),
        }
    }

    #[test]
    fn modes_round_trip_and_reject_unknown_names() {
        for mode in [Mode::Lines, Mode::Count, Mode::Attributes, Mode::Grade] {
            assert_eq!(Mode::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(Mode::parse("tooltip"), None);
        assert_eq!(Mode::parse(""), None);
    }

    #[test]
    fn the_count_is_the_best_confident_line_that_holds_digits() {
        let lines = vec![line("stack 12", 0.5), line("3", 0.9)];
        assert_eq!(feeder_count(&lines), Some(3));
        assert_eq!(feeder_count(&[line("no digits", 0.9)]), None);
        assert_eq!(feeder_count(&[]), None);
    }

    #[test]
    fn the_grade_line_is_concatenated_left_to_right() {
        // The detector emits items in raster order, and a row can start with the letter: " DG" at
        // x=50 must not be concatenated BEFORE "GRADE" at x=0, or the line reads "DGGRADE:" and the
        // post-separator text is empty (OCR-F5).
        let items = vec![
            item(" DG", 0.9, 50.0, 0.0),
            item(":", 0.9, 40.0, 0.0),
            item("GRADE", 0.9, 0.0, 0.0),
        ];
        assert_eq!(
            read_grade(&items, &OcrGeometry::default()),
            Some("DG".to_owned())
        );
    }

    #[test]
    fn the_grade_line_applies_v2s_confidence_and_artifact_filters() {
        // v2 dropped grade candidates with conf <= 0.1 or text "300" (OcrEngine.cs:280-288). Here the
        // only grade token sits below the confidence floor, so the read must fall through to the
        // colour fallback rather than read "DG" out of the noisy band.
        let items = vec![
            item("GRADE:", 0.95, 0.0, 0.0),
            item("300", 0.95, 45.0, 0.0),
            item("DG", 0.05, 55.0, 0.0),
        ];
        assert_eq!(read_grade(&items, &OcrGeometry::default()), None);

        let confident = vec![item("GRADE:", 0.95, 0.0, 0.0), item("DG", 0.95, 55.0, 0.0)];
        assert_eq!(
            read_grade(&confident, &OcrGeometry::default()),
            Some("DG".to_owned())
        );
    }

    #[test]
    fn the_grade_is_read_after_the_label_separator() {
        let items = vec![
            item("GRADE", 0.9, 0.0, 0.0),
            item(":", 0.9, 40.0, 0.0),
            item(" DG", 0.9, 50.0, 0.0),
        ];
        assert_eq!(
            read_grade(&items, &OcrGeometry::default()),
            Some("DG".to_owned())
        );

        let without_label = vec![item("N", 0.9, 0.0, 0.0)];
        assert_eq!(
            read_grade(&without_label, &OcrGeometry::default()),
            Some("N".to_owned())
        );
        assert_eq!(read_grade(&[], &OcrGeometry::default()), None);
    }

    #[test]
    fn the_grade_band_filters_when_it_has_values_and_falls_back_when_empty() {
        let items = vec![item("DG", 0.9, 0.0, 2.0), item("XG", 0.9, 0.0, 200.0)];
        let banded = OcrGeometry {
            grade_y: Some((0, 20)),
            ..OcrGeometry::default()
        };
        assert_eq!(read_grade(&items, &banded), Some("DG".to_owned()));

        let empty_band = OcrGeometry {
            grade_y: Some((900, 1000)),
            ..OcrGeometry::default()
        };
        assert!(
            read_grade(&items, &empty_band).is_some(),
            "an empty band falls back to all items"
        );
    }

    #[test]
    fn the_grade_area_is_a_half_open_region_relative_box() {
        let area = GradeArea {
            x1: 100,
            y1: 10,
            x2: 200,
            y2: 40,
        };
        assert!(area.selects(&item("inside", 0.9, 120.0, 20.0)));
        assert!(area.selects(&item("left edge", 0.9, 100.0, 10.0)));
        assert!(!area.selects(&item("right edge", 0.9, 200.0, 20.0)));
        assert!(!area.selects(&item("below", 0.9, 120.0, 40.0)));
        assert!(!area.selects(&item("above", 0.9, 120.0, 0.0)));
        assert!(!area.selects(&item("left", 0.9, 80.0, 20.0)));
    }

    #[test]
    fn the_grade_area_bounds_the_grade_read_and_absent_falls_back_to_the_band() {
        let items = vec![
            item("GRADE: DG", 0.9, 120.0, 2.0),
            item("XG", 0.9, 500.0, 2.0),
        ];
        let area = OcrGeometry {
            grade_area: Some(GradeArea {
                x1: 100,
                y1: 0,
                x2: 200,
                y2: 40,
            }),
            grade_y: Some((0, 40)),
            ..OcrGeometry::default()
        };
        assert_eq!(
            read_grade(&items, &area),
            Some("DG".to_owned()),
            "the box selects the grade line and excludes the XG outside it"
        );

        let band_only = OcrGeometry {
            grade_y: Some((0, 40)),
            ..OcrGeometry::default()
        };
        assert_eq!(
            read_grade(&items, &band_only),
            Some("XG".to_owned()),
            "without a box, the pre-existing grade_y band sees both lines"
        );
    }

    #[test]
    fn the_grade_mode_selects_the_line_inside_the_area() {
        let scan = Scan {
            items: vec![
                item("GRADE: G", 0.9, 120.0, 2.0),
                item("DG", 0.9, 500.0, 2.0),
            ],
            lines: Vec::new(),
        };
        let geometry = OcrGeometry {
            grade_area: Some(GradeArea {
                x1: 100,
                y1: 0,
                x2: 200,
                y2: 40,
            }),
            ..OcrGeometry::default()
        };
        let result = apply(Mode::Grade, &scan, &AttributesConfig::default(), &geometry);
        assert_eq!(result.grade, Some("G".to_owned()));
    }

    #[test]
    fn attributes_group_by_row_and_keep_labels_and_values_together() {
        let items = vec![
            item("力量", 0.9, 0.0, 20.0),
            item("12", 0.9, 100.0, 20.0),
            item("敏捷", 0.9, 0.0, 60.0),
            item("7", 0.9, 100.0, 60.0),
        ];
        let rows = read_attributes(
            &items,
            &AttributesConfig::default(),
            &OcrGeometry::default(),
        );
        assert_eq!(rows.len(), 2, "two rows above the y>=10 cut: {rows:?}");
        assert_eq!(rows[0].len(), 2, "a label and its value: {rows:?}");
        assert!(rows[0].iter().any(|text| text == "力量"));
        assert!(rows[1].iter().any(|text| text == "敏捷"));
    }

    #[test]
    fn apply_returns_one_shape_and_only_the_modes_field() {
        let scan = Scan {
            items: vec![item("GRADE: DG", 0.9, 0.0, 2.0), item("3", 0.9, 0.0, 20.0)],
            lines: vec![line("GRADE: DG", 0.9), line("3", 0.9)],
        };
        let attributes = AttributesConfig::default();
        let geometry = OcrGeometry::default();

        let lines = apply(Mode::Lines, &scan, &attributes, &geometry);
        assert_eq!(lines.lines.len(), 2);
        assert!(lines.attributes.is_none() && lines.grade.is_none() && lines.remaining.is_none());

        let count = apply(Mode::Count, &scan, &attributes, &geometry);
        assert_eq!(count.remaining, Some(3));

        let grade = apply(Mode::Grade, &scan, &attributes, &geometry);
        assert_eq!(grade.grade, Some("DG".to_owned()));

        let attrs = apply(Mode::Attributes, &scan, &attributes, &geometry);
        assert!(attrs.attributes.is_some());
    }

    #[test]
    fn line_spacing_collapses_nearby_tops_and_takes_the_median_gap() {
        // Two glyph boxes at almost the same Y are ONE line; the gaps are 20, 20, 25 → median 20.
        let items = vec![
            item("a", 0.9, 0.0, 0.0),
            item("b", 0.9, 10.0, 2.0),
            item("c", 0.9, 0.0, 20.0),
            item("d", 0.9, 0.0, 40.0),
            item("e", 0.9, 0.0, 65.0),
        ];
        assert_eq!(measure_line_spacing(&items), Some(20));
    }

    #[test]
    fn line_spacing_is_none_with_fewer_than_two_collapsed_lines() {
        assert_eq!(measure_line_spacing(&[]), None);
        assert_eq!(
            measure_line_spacing(&[item("a", 0.9, 0.0, 0.0), item("b", 0.9, 5.0, 1.0)]),
            None,
            "two boxes at the same Y are one line"
        );
    }

    #[test]
    fn remaining_uses_its_band_x_filter_and_confidence_floor() {
        let mut geometry = OcrGeometry {
            remaining_y: Some((50, 70)),
            remaining_x: Some((0, 200)),
            ..OcrGeometry::default()
        };
        let items = vec![
            item("99", 0.9, 0.0, 10.0),   // wrong Y band
            item("42", 0.9, 400.0, 60.0), // outside the X band
            item("7", 0.3, 0.0, 60.0),    // too low a confidence
            item("12", 0.9, 10.0, 60.0),  // accepted
        ];
        assert_eq!(read_remaining(&items, &geometry), Some(12));
        geometry.remaining_y = None;
        geometry.remaining_x = None;
        let no_digits = vec![item("springs", 0.9, 10.0, 60.0)];
        assert_eq!(read_remaining(&no_digits, &geometry), None);
    }

    #[test]
    fn the_grade_falls_back_to_the_grade_box_colour_when_the_text_is_missing() {
        let scan = Scan {
            lines: Vec::new(),
            items: Vec::new(),
        };
        let attrs = AttributesConfig::default();
        let geometry = OcrGeometry {
            grade_area: Some(GradeArea {
                x1: 0,
                y1: 0,
                x2: 10,
                y2: 10,
            }),
            ..OcrGeometry::default()
        };
        let blue = [10u8, 10u8, 220u8, 255u8].repeat(100);
        let colors = seal_domain::config::GradeColorsConfig::default();
        let result = apply_with_image(
            Mode::Grade,
            &scan,
            &attrs,
            &geometry,
            Some(GradeImage {
                rgba: &blue,
                width: 10,
                height: 10,
                colors: &colors,
            }),
        );
        assert_eq!(result.grade.as_deref(), Some("G"));

        // Without the pixels the text read is the only source, and it found nothing.
        let without = apply_with_image(Mode::Grade, &scan, &attrs, &geometry, None);
        assert_eq!(without.grade, None);
    }
}

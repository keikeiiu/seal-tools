//! The `ocr.scan` modes: `lines`, `count`, `attributes`, `grade` (and `remaining` as a follow-up).
//!
//! Text rules stay in `seal-domain`: attribute sign fixing is [`seal_domain::ocr::fix_sign`] and line
//! text was already cleaned by the pipeline's `TextCleaner`. The v2 call sites this ports:
//! `OcrEngine.ScanOnce` (`v2/SealTools.Core/OcrEngine.cs:273-345`): grade line from the full read
//! (Y-filtered), attributes from the X+Y-filtered items grouped by row and classified label/value,
//! `FixSign`, first 3 rows starting at Y >= 10.
//!
//! **Follow-ups, marked here rather than silently omitted:** the grade COLOUR fallback
//! (`DetectGradeColorScores`/`DecideGrade`) and the `remaining` count read are not ported; the result
//! carries `remaining: null` and colour is not consulted.

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
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "lines" => Some(Self::Lines),
            "count" => Some(Self::Count),
            "attributes" => Some(Self::Attributes),
            "grade" => Some(Self::Grade),
            _ => None,
        }
    }

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
    pub row_height: u32,
}

impl Default for OcrGeometry {
    fn default() -> Self {
        Self {
            attr_x: None,
            attr_y: None,
            grade_y: None,
            row_height: 10,
        }
    }
}

/// The protocol's one result shape (`PROTOCOL.md` §4): absent fields are `None`/null.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModeResult {
    pub lines: Vec<String>,
    pub attributes: Option<Vec<Vec<String>>>,
    pub grade: Option<String>,
    pub remaining: Option<u32>,
}

/// Applies one mode to a scan. `attributes` is the active dictionary (its `negative_attr_markers`
/// feed the domain's sign fix).
pub fn apply(
    mode: Mode,
    scan: &Scan,
    attributes: &AttributesConfig,
    geometry: &OcrGeometry,
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
        Mode::Grade => ModeResult {
            lines,
            grade: read_grade(&scan.items, geometry),
            ..ModeResult::default()
        },
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

fn has_digit(text: &str) -> bool {
    text.chars().any(|character| character.is_ascii_digit())
}

/// `DetectGradeFromLine` (`OcrEngine.cs:290-...`): concatenate the grade band's lines uppercase, cut
/// after the last `:`/`：` (or a leading `GRADE`/`等级` label), then take the longest-first match of
/// `SG, XG, DG, G, N`.
pub fn read_grade(items: &[RecognizedLine], geometry: &OcrGeometry) -> Option<String> {
    let mut band: Vec<&RecognizedLine> = items
        .iter()
        .filter(|item| in_band(item_y(item), geometry.grade_y))
        .collect();
    if band.is_empty() {
        band = items.iter().collect();
    }
    let text: String = band
        .iter()
        .map(|item| item.text.as_str())
        .collect::<String>()
        .to_uppercase();
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
}

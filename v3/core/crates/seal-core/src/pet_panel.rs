//! Pet increment 4: the Test-read-panel diagnostic (`pet.test_read_panel`).
//!
//! The command hovers the chosen bag cell (board closed-loop placement, never a click), waits the
//! requested delay, captures the configured tooltip box and OCRs it. The pure half here owns the
//! argument validation, the cell-centre/tooltip geometry and the panel projection, so a fake
//! frame/OCR test can prove the numbers without a live game.

use seal_domain::config::TooltipConfig;
use seal_domain::geometry::{Point, Rect};
use seal_domain::ocr::PetPanel;
use seal_domain::tools::shop::BagGrid;
use serde_json::{Value, json};

use crate::pet_capture::{IconError, page_grid};

/// The panel-read arguments: 1-based page/cell like every Pet-tab command, and a bounded settle
/// delay after the hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelArguments {
    pub page: usize,
    pub cell: usize,
    pub delay_ms: u64,
}

pub const DEFAULT_DELAY_MS: u64 = 700;
pub const MAX_DELAY_MS: u64 = 5_000;

///
/// # Errors
///
/// Returns the player-facing line and the field name when `page` or `cell` is malformed.
#[allow(
    clippy::cast_possible_truncation,
    reason = "row/page/cell indices are small values validated before use"
)]
pub fn arguments(args: &Value) -> Result<PanelArguments, (String, &'static str)> {
    let page = args
        .get("page")
        .and_then(Value::as_u64)
        .filter(|page| *page >= 1)
        .ok_or_else(|| {
            (
                "pet.test_read_panel's \"page\" must be a 1-based integer".to_owned(),
                "page",
            )
        })? as usize;
    let cell = args
        .get("cell")
        .and_then(Value::as_u64)
        .filter(|cell| (1..=BagGrid::SLOT_COUNT as u64).contains(cell))
        .ok_or_else(|| {
            (
                format!(
                    "pet.test_read_panel's \"cell\" must be between 1 and {}",
                    BagGrid::SLOT_COUNT
                ),
                "cell",
            )
        })? as usize;
    let delay_ms = match args.get("delay_ms") {
        None => DEFAULT_DELAY_MS,
        Some(value) => value
            .as_u64()
            .filter(|delay| *delay <= MAX_DELAY_MS)
            .ok_or_else(|| {
                (
                    format!(
                        "pet.test_read_panel's \"delay_ms\" must be between 0 and {MAX_DELAY_MS}"
                    ),
                    "delay_ms",
                )
            })?,
    };
    Ok(PanelArguments {
        page,
        cell,
        delay_ms,
    })
}

/// The hover point (cell centre) and the tooltip region (centre + configured offset/size).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelGeometry {
    pub centre: Point,
    pub region: Rect,
}

///
/// # Errors
///
/// Returns the player-facing line and the field name when the panel's marks are not calibrated.
pub fn geometry(
    config: &Value,
    arguments: &PanelArguments,
) -> Result<PanelGeometry, (String, &'static str)> {
    let grid = page_grid(config, arguments.page).map_err(|error| match error {
        IconError::BadRequest { message, field } => (message, field),
        IconError::Internal(message) => (message, "internal"),
    })?;
    let cell = BagGrid::cell_box(Some(&grid), arguments.cell - 1).ok_or_else(|| {
        (
            "The boarding bag grid works out to an empty cell — re-drag it on the Calibrate tab."
                .to_owned(),
            "bag_grid",
        )
    })?;
    let tooltip: TooltipConfig = config
        .get("tooltip")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    if !tooltip.is_set() {
        return Err((
            "The tooltip/panel region isn't calibrated — save the tooltip offset and size (the \
             hover panel box) on the Calibrate tab."
                .to_owned(),
            "tooltip",
        ));
    }
    let centre = Point::new(cell.x + cell.w / 2, cell.y + cell.h / 2);
    Ok(PanelGeometry {
        centre,
        region: Rect::new(
            centre.x + tooltip.offset_x,
            centre.y + tooltip.offset_y,
            tooltip.width,
            tooltip.height,
        ),
    })
}

/// Project the OCR lines into the protocol's panel object. `PetPanel::parse` is deliberately
/// all-or-nothing on the EXP anchor; a panel it cannot fully parse still returns every raw line,
/// the number tokens it saw and an explicit `null` panel — never invented numbers (the diagnostic's
/// point). `line` is the parser's own description, so the shell renders one authoritative sentence
/// instead of re-deriving stage/growth/EXP.
#[must_use]
pub fn panel_result(lines: &[String]) -> Value {
    let panel = PetPanel::parse(lines).map(|panel| {
        json!({
            "growth": panel.growth,
            "exp": panel.exp,
            "line": panel.describe(),
        })
    });
    json!({
        "lines": lines,
        "panel": panel,
        "numbers": number_tokens(lines),
    })
}

/// Every number-shaped token in the OCR text, in reading order (v2's `NumbersWithContext`).
///
/// This is the diagnostic's "what did the recogniser actually see?" evidence: the parser may reject
/// the panel, but the digits it produced are still worth showing, because a wrong region usually
/// reads somebody else's numbers rather than nothing.
fn number_tokens(lines: &[String]) -> Vec<String> {
    let text = lines.join(" ");
    let characters: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < characters.len() {
        if !characters[at].is_ascii_digit() {
            at += 1;
            continue;
        }
        let start = at;
        while at < characters.len() && characters[at].is_ascii_digit() {
            at += 1;
        }
        if at + 1 < characters.len()
            && (characters[at] == '.' || characters[at] == ',')
            && characters[at + 1].is_ascii_digit()
        {
            at += 1;
            while at < characters.len() && characters[at].is_ascii_digit() {
                at += 1;
            }
        }
        tokens.push(characters[start..at].iter().collect());
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Value {
        json!({
            "pet": {
                "bag_grid": [0, 0, 160, 160],
                "page_tabs": [[10, 10]],
            },
            "tooltip": {
                "offset_x": 3,
                "offset_y": -2,
                "width": 120,
                "height": 80,
            }
        })
    }

    #[test]
    fn arguments_default_and_bound_the_delay() {
        assert_eq!(
            arguments(&json!({"page": 1, "cell": 34})).unwrap(),
            PanelArguments {
                page: 1,
                cell: 34,
                delay_ms: 700
            }
        );
        assert_eq!(
            arguments(&json!({"page": 2, "cell": 1, "delay_ms": 0}))
                .unwrap()
                .delay_ms,
            0
        );
        assert_eq!(
            arguments(&json!({"page": 2, "cell": 1, "delay_ms": 5000}))
                .unwrap()
                .delay_ms,
            5000
        );
        assert_eq!(
            arguments(&json!({"page": 1, "cell": 1, "delay_ms": 5001}))
                .unwrap_err()
                .1,
            "delay_ms"
        );
        assert_eq!(
            arguments(&json!({"page": 0, "cell": 1})).unwrap_err().1,
            "page"
        );
        assert_eq!(
            arguments(&json!({"page": 1, "cell": 65})).unwrap_err().1,
            "cell"
        );
    }

    #[test]
    fn geometry_is_the_cell_centre_plus_the_tooltip_box() {
        // 160px grid => 20px pitch; zero-based cell 33 (1-based 34) is row 4, col 1. Its centre is
        // (30, 90), so the panel region is (33, 88, 120, 80).
        let answer = geometry(
            &config(),
            &PanelArguments {
                page: 1,
                cell: 34,
                delay_ms: 700,
            },
        )
        .expect("geometry");
        assert_eq!(answer.centre, Point::new(30, 90));
        assert_eq!(answer.region, Rect::new(33, 88, 120, 80));
    }

    #[test]
    fn missing_tooltip_and_page_are_named_prerequisites() {
        let mut no_tooltip = config();
        no_tooltip["tooltip"] = json!({"offset_x": 0, "offset_y": 0, "width": 0, "height": 0});
        assert_eq!(
            geometry(
                &no_tooltip,
                &PanelArguments {
                    page: 1,
                    cell: 1,
                    delay_ms: 700
                }
            )
            .unwrap_err()
            .1,
            "tooltip"
        );
        assert_eq!(
            geometry(
                &config(),
                &PanelArguments {
                    page: 2,
                    cell: 1,
                    delay_ms: 700
                }
            )
            .unwrap_err()
            .1,
            "page"
        );
    }

    #[test]
    fn a_full_panel_parses_to_exact_numbers_and_a_line() {
        let answer = panel_result(&["(6階) 真蔚藍鳳凰 +7 [52.18%]".to_owned()]);
        assert_eq!(answer["panel"]["growth"], 7);
        assert_eq!(answer["panel"]["exp"], 52.18);
        assert_eq!(answer["panel"]["line"], "stage 6, +7, 52.18%");
        assert_eq!(answer["lines"][0], "(6階) 真蔚藍鳳凰 +7 [52.18%]");
        assert_eq!(answer["numbers"], json!(["6", "7", "52.18"]));
    }

    #[test]
    fn a_partial_panel_keeps_raw_lines_numbers_and_a_null_panel() {
        let answer = panel_result(&["(6階) 真蔚藍鳳凰 +7".to_owned()]);
        assert_eq!(answer["lines"][0], "(6階) 真蔚藍鳳凰 +7");
        assert!(answer["panel"].is_null());
        assert_eq!(answer["numbers"], json!(["6", "7"]));
    }

    #[test]
    fn number_tokens_keep_decimals_and_thousands_separators_as_read() {
        let answer = panel_result(&[
            "price 1,234".to_owned(),
            "exp 52.18%".to_owned(),
            "no digits".to_owned(),
        ]);
        assert_eq!(answer["numbers"], json!(["1,234", "52.18"]));
    }
}

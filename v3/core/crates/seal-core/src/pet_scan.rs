//! Pet increment 3: the read-only bag scans and the explicit food-cell apply.
//!
//! The scans are pure over an already-captured `PetImage` (the Win32 capture is the dispatcher's
//! one OS call) and reuse the run's matcher — `pet::score_all_scores`, `differing_fraction_images`,
//! `MATCH_LIMIT` — rather than a second comparison implementation. Evidence (the page PNG and the
//! scan report) is written by the handler under `<root>/logs/reads/`; it is best-effort and never
//! makes a scan fail.

use std::collections::BTreeMap;

use seal_domain::tools::pet::MATCH_LIMIT;
use serde_json::{Value, json};

use crate::pet::{PetImage, decode_icon, score_all_scores};
use crate::pet_capture::{IconError, page_grid};

/// A page/cell box argument, 1-based in the protocol (the Pet tab's own numbering).
pub fn page_argument(args: &Value) -> Result<usize, (String, &'static str)> {
    let Some(page) = args.get("page").and_then(Value::as_u64) else {
        return Err((
            "pet scan commands need a 1-based integer \"page\"".to_owned(),
            "page",
        ));
    };
    if page == 0 {
        return Err((
            "page scan commands need a 1-based integer \"page\"".to_owned(),
            "page",
        ));
    }
    Ok(page as usize)
}

/// Validate an apply selection. Returns ZERO-BASED `(page, cell)` pairs for `pet.food_slots`
/// (the config's own ordering), after checking the 1-based protocol args against the marks.
pub fn apply_cells(
    config: &Value,
    args: &Value,
) -> Result<Vec<(usize, usize)>, (String, &'static str)> {
    let Some(cells) = args.get("cells").and_then(Value::as_array) else {
        return Err((
            "pet.apply_food_scan needs \"cells\": an array of [page, cell] pairs".to_owned(),
            "cells",
        ));
    };
    if cells.is_empty() {
        // The scan's own rule: nothing found means nothing written. An explicit empty apply is
        // refused rather than silently clearing the cells a run is using.
        return Err((
            "pet.apply_food_scan's \"cells\" is empty; there is nothing to replace the food cells with"
                .to_owned(),
            "cells",
        ));
    }
    let tabs: Vec<usize> = config
        .pointer("/pet/page_tabs")
        .and_then(Value::as_array)
        .map(|tabs| {
            tabs.iter()
                .map(|tab| tab.as_array().map_or(0, Vec::len))
                .collect()
        })
        .unwrap_or_default();
    let mut output = Vec::with_capacity(cells.len());
    for (index, cell) in cells.iter().enumerate() {
        let Some(pair) = cell.as_array() else {
            return Err((
                format!("pet.apply_food_scan's \"cells\"[{index}] is not a [page, cell] pair"),
                "cells",
            ));
        };
        let (Some(page), Some(cell)) = (
            pair.first().and_then(Value::as_u64),
            pair.get(1).and_then(Value::as_u64),
        ) else {
            return Err((
                format!("pet.apply_food_scan's \"cells\"[{index}] needs two integers"),
                "cells",
            ));
        };
        if pair.len() != 2 {
            return Err((
                format!("pet.apply_food_scan's \"cells\"[{index}] needs exactly [page, cell]"),
                "cells",
            ));
        }
        if page == 0 || page as usize > tabs.len() || tabs[(page - 1) as usize] != 2 {
            return Err((
                format!(
                    "pet.apply_food_scan's \"cells\"[{index}] page {page} has no 2-point tab mark"
                ),
                "cells",
            ));
        }
        if !(1..=64).contains(&cell) {
            return Err((
                format!("pet.apply_food_scan's \"cells\"[{index}] cell {cell} is outside 1..=64"),
                "cells",
            ));
        }
        output.push(((page - 1) as usize, (cell - 1) as usize));
    }
    Ok(output)
}

pub fn frame_image(frame: &Value) -> Result<PetImage, String> {
    let (width, height, rgba) = crate::dispatch::frame_rgba(frame)?;
    Ok(PetImage {
        width,
        height,
        rgba,
    })
}

fn icon_error(error: IconError) -> (String, &'static str) {
    match error {
        IconError::BadRequest { message, field } => (message, field),
        IconError::Internal(message) => (message, "internal"),
    }
}

/// The two `logs/reads/` names for one page scan. The report carries the page too: the stamp is
/// second-resolution, so sweeping ITEM1–3 quickly used to let page 2's report overwrite page 1's
/// while page 1's returned evidence path pointed at the newer file.
pub fn evidence_names(page: usize, stamp: &str) -> (String, String) {
    (
        format!("scan_page{page}_{stamp}.png"),
        format!("scan_page{page}_{stamp}.txt"),
    )
}

/// The configured food icon, decoded. Kept separate from the scan so the handler can refuse a
/// missing/corrupt icon BEFORE the foreground capture and evidence write.
pub fn food_icon(config: &Value) -> Result<PetImage, (String, &'static str)> {
    let food_icon_png = config
        .pointer("/pet/food_icon_png")
        .and_then(Value::as_str)
        .filter(|png| !png.trim().is_empty())
        .ok_or_else(|| {
            (
                "No food icon has been captured yet — capture one food cell's icon first."
                    .to_owned(),
                "food_icon_png",
            )
        })?;
    decode_icon(food_icon_png, "The food icon").map_err(|message| {
        (
            format!("The stored food icon could not be decoded — capture it again: {message}"),
            "food_icon_png",
        )
    })
}

/// Food scan with an already-decoded icon (the handler validates it before capturing).
pub fn food_scan_with_icon(
    config: &Value,
    page: usize,
    bag: &PetImage,
    icon: &PetImage,
) -> Result<(Value, Vec<String>), (String, &'static str)> {
    let grid = page_grid(config, page).map_err(icon_error)?;
    let scores = score_all_scores(bag, &grid, icon);
    let mut hits: Vec<(usize, f64)> = scores
        .iter()
        .copied()
        .filter(|(_, score)| *score <= MATCH_LIMIT)
        .collect();
    // CELL order, not score order: the feeder walks `pet.food_slots` in this order, and near-equal
    // food instances would otherwise come back in an arbitrary order.
    hits.sort_by_key(|(cell, _)| *cell);
    let cells: Vec<usize> = hits.iter().map(|(cell, _)| cell + 1).collect();
    let runner_up = scores.get(1).map(|(cell, score)| (*cell + 1, *score));

    let mut report = Vec::new();
    if cells.is_empty() {
        report.push(format!("page {page}: nothing matched"));
    } else {
        report.push(format!(
            "page {page}: {} cell(s) — {}",
            cells.len(),
            cells
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
        for (cell, score) in &hits {
            report.push(format!(
                "  cell {}: {:.3} (limit {:.3})",
                cell + 1,
                score,
                MATCH_LIMIT
            ));
        }
        if let Some((cell, score)) = runner_up {
            report.push(format!("  runner-up: cell {cell} at {score:.3}"));
        }
    }

    Ok((
        json!({
            "page": page,
            "cells": cells,
            "report": report,
        }),
        report,
    ))
}

/// Food scan convenience: decode the configured icon then run the body above.
pub fn food_scan(
    config: &Value,
    page: usize,
    bag: &PetImage,
) -> Result<(Value, Vec<String>), (String, &'static str)> {
    let icon = food_icon(config)?;
    food_scan_with_icon(config, page, bag, &icon)
}

/// One queued pet's icon, already decoded; `None` is a crop that would not decode.
pub fn queue_entries(config: &Value) -> Vec<(String, Option<PetImage>)> {
    config
        .pointer("/pet/queue")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let label = entry
                        .get("label")
                        .and_then(Value::as_str)
                        .filter(|label| !label.trim().is_empty())
                        .map(str::to_owned)
                        .unwrap_or_else(|| "(no name)".to_owned());
                    let icon = entry
                        .get("png")
                        .and_then(Value::as_str)
                        .filter(|png| !png.trim().is_empty())
                        .and_then(|png| {
                            decode_icon(png, &format!("Queue pet {}'s icon", index + 1)).ok()
                        });
                    (label, icon)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Queue scan: every configured queue icon against every cell of the page. `per_entry` keeps the
/// configured order; `claimed` names cells matched by more than one icon; `deduced` is the v2
/// player-facing "label ×n" line.
pub fn queue_scan(
    config: &Value,
    page: usize,
    bag: &PetImage,
) -> Result<(Value, Vec<String>), (String, &'static str)> {
    let grid = page_grid(config, page).map_err(icon_error)?;
    let entries = queue_entries(config);
    let mut per_entry = Vec::with_capacity(entries.len());
    let mut claimed: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut deduced = Vec::new();
    let mut report = Vec::new();
    if entries.is_empty() {
        report.push(format!("page {page}: no queued pet icons configured"));
    }

    for (index, (label, icon)) in entries.iter().enumerate() {
        let Some(icon) = icon else {
            per_entry.push(json!({"index": index, "label": label, "hits": []}));
            report.push(format!("  page {page}  {label} — icon wouldn't decode"));
            continue;
        };
        let scores = score_all_scores(bag, &grid, icon);
        let runner_up = scores.get(1).map(|(_, score)| *score);
        let hits: Vec<(usize, f64)> = scores
            .iter()
            .copied()
            .filter(|(_, score)| *score <= MATCH_LIMIT)
            .collect();
        if !hits.is_empty() {
            deduced.push(format!("{label} ×{}", hits.len()));
        }
        for (cell, _) in &hits {
            claimed.entry(*cell).or_default().push(label.clone());
        }
        let hits_json: Vec<Value> = hits
            .iter()
            .map(|(cell, score)| {
                json!({
                    "cell": cell + 1,
                    "score": score,
                    "runner_up": runner_up,
                })
            })
            .collect();
        per_entry.push(json!({"index": index, "label": label, "hits": hits_json}));

        if let Some((best_cell, best_score)) = scores.first() {
            report.push(format!(
                "  page {page}  {label} — best {best_score:.3} at cell {}{}{}",
                best_cell + 1,
                if hits.len() > 1 {
                    format!(", {} cells under the limit", hits.len())
                } else {
                    ", nothing else close".to_owned()
                },
                runner_up.map_or_else(String::new, |score| format!("; runner-up {score:.3}")),
            ));
        }
    }
    report.insert(
        0,
        format!(
            "page {page}:  {}",
            if deduced.is_empty() {
                "no queued pet here".to_owned()
            } else {
                deduced.join(",   ")
            }
        ),
    );
    for (cell, labels) in &claimed {
        if labels.len() > 1 {
            let mut labels = labels.clone();
            labels.sort();
            labels.dedup();
            report.push(format!(
                "          — cell {} claimed by more than one queued pet ({})",
                cell + 1,
                labels.join(", ")
            ));
        }
    }

    let claimed_json: Vec<Value> = claimed
        .iter()
        .filter(|(_, labels)| labels.len() > 1)
        .map(|(cell, labels)| {
            let mut labels = labels.clone();
            labels.sort();
            labels.dedup();
            json!({"cell": cell + 1, "labels": labels})
        })
        .collect();

    Ok((
        json!({
            "page": page,
            "per_entry": per_entry,
            "claimed": claimed_json,
            "deduced": deduced,
        }),
        report,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    const BG: [u8; 4] = [40, 40, 40, 255];
    const RED: [u8; 4] = [200, 40, 40, 255];
    const GREEN: [u8; 4] = [40, 200, 40, 255];

    fn png_base64(color: [u8; 4]) -> String {
        let bgra: Vec<u8> = color
            .repeat(20 * 20)
            .chunks(4)
            .flat_map(|p| [p[2], p[1], p[0], p[3]])
            .collect();
        let png = seal_win::pure::encode_png(20, 20, &bgra).expect("icon png");
        base64::engine::general_purpose::STANDARD.encode(png)
    }

    fn bag(cell_colors: &[(usize, [u8; 4])]) -> PetImage {
        let mut rgba = BG.repeat(160 * 160);
        for (cell, color) in cell_colors {
            let left = (cell % 8) * 20;
            let top = (cell / 8) * 20;
            for y in top..top + 20 {
                for x in left..left + 20 {
                    let at = (y * 160 + x) * 4;
                    rgba[at..at + 4].copy_from_slice(color);
                }
            }
        }
        PetImage {
            width: 160,
            height: 160,
            rgba,
        }
    }

    fn config(tabs: usize, food_icon: bool, queue: &[(&str, [u8; 4])]) -> Value {
        let page_tabs: Vec<Value> = (0..tabs).map(|_| json!([10, 10])).collect();
        let queue: Vec<Value> = queue
            .iter()
            .map(|(label, color)| json!({"label": label, "png": png_base64(*color)}))
            .collect();
        let mut pet = json!({
            "bag_grid": [0, 0, 160, 160],
            "page_tabs": page_tabs,
            "queue": queue,
        });
        if food_icon {
            pet["food_icon_png"] = Value::String(png_base64(RED));
        }
        json!({"pet": pet})
    }

    #[test]
    fn food_scan_returns_cell_order_and_thresholded_hits() {
        let config = config(2, true, &[]);
        let bag = bag(&[(35, RED), (12, RED)]);
        let (result, report) = food_scan(&config, 1, &bag).expect("food scan");
        assert_eq!(result["page"], 1);
        assert_eq!(result["cells"], json!([13, 36]), "1-based, CELL order");
        assert!(report[0].contains("2 cell(s)"));
        assert!(report[0].contains("13, 36"));
        assert!(report.iter().any(|line| line.contains("limit 0.500")));
    }

    #[test]
    fn food_scan_empty_page_has_no_cells_and_says_nothing_matched() {
        let config = config(1, true, &[]);
        let (result, report) = food_scan(&config, 1, &bag(&[])).expect("food scan");
        assert_eq!(result["cells"], json!([]));
        assert_eq!(report, vec!["page 1: nothing matched"]);
    }

    #[test]
    fn food_scan_missing_icon_or_page_is_a_named_bad_request() {
        let no_icon = config(1, false, &[]);
        assert_eq!(
            food_scan(&no_icon, 1, &bag(&[])).unwrap_err().1,
            "food_icon_png"
        );
        let config = config(1, true, &[]);
        assert_eq!(food_scan(&config, 2, &bag(&[])).unwrap_err().1, "page");
    }

    #[test]
    fn queue_scan_reports_per_entry_hits_and_claimed_conflicts() {
        let config = config(1, false, &[("red", RED), ("same", RED), ("green", GREEN)]);
        let bag = bag(&[(12, RED), (35, GREEN)]);
        let (result, _report) = queue_scan(&config, 1, &bag).expect("queue scan");
        assert_eq!(result["page"], 1);
        assert_eq!(result["per_entry"].as_array().unwrap().len(), 3);
        assert_eq!(result["per_entry"][0]["label"], "red");
        assert_eq!(result["per_entry"][0]["hits"][0]["cell"], 13);
        assert_eq!(result["per_entry"][0]["hits"][0]["score"], 0.0);
        assert_eq!(result["per_entry"][2]["label"], "green");
        assert_eq!(result["per_entry"][2]["hits"][0]["cell"], 36);
        assert_eq!(result["claimed"].as_array().unwrap().len(), 1);
        assert_eq!(result["claimed"][0]["cell"], 13);
        assert!(
            result["deduced"]
                .as_array()
                .unwrap()
                .iter()
                .any(|line| line == "red ×1")
        );
    }

    #[test]
    fn queue_scan_no_icons_is_empty_not_an_error() {
        let config = config(1, false, &[]);
        let (result, report) = queue_scan(&config, 1, &bag(&[])).expect("queue scan");
        assert_eq!(result["per_entry"], json!([]));
        assert!(
            report
                .iter()
                .any(|line| line.contains("no queued pet icons"))
        );
    }

    #[test]
    fn apply_cells_is_one_based_and_writes_zero_based_pairs() {
        let config = config(2, false, &[]);
        assert_eq!(
            apply_cells(&config, &json!({"cells": [[1, 1], [2, 64]]})),
            Ok(vec![(0, 0), (1, 63)])
        );
        assert_eq!(
            apply_cells(&config, &json!({"cells": []})).unwrap_err().1,
            "cells"
        );
        assert_eq!(
            apply_cells(&config, &json!({"cells": [[3, 1]]}))
                .unwrap_err()
                .1,
            "cells"
        );
        assert_eq!(
            apply_cells(&config, &json!({"cells": [[1, 65]]}))
                .unwrap_err()
                .1,
            "cells"
        );
        assert_eq!(
            apply_cells(&config, &json!({"cells": [[1]]}))
                .unwrap_err()
                .1,
            "cells"
        );
    }

    #[test]
    fn evidence_names_are_page_specific_even_for_the_same_stamp() {
        let (png1, txt1) = evidence_names(1, "20261028_120000");
        let (png2, txt2) = evidence_names(2, "20261028_120000");
        assert_eq!(png1, "scan_page1_20261028_120000.png");
        assert_eq!(txt1, "scan_page1_20261028_120000.txt");
        assert_ne!(png1, png2);
        assert_ne!(txt1, txt2, "same-second page reports must not overwrite");
    }
}

// ── G14: the feedable sweep's one-page result and the queue rebuild ────────────────────────────

/// `pet.scan_feeding_page {page, harvest?}` arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedingArguments {
    pub page: usize,
    pub harvest: bool,
}

pub fn feeding_arguments(args: &Value) -> Result<FeedingArguments, (String, &'static str)> {
    let page = page_argument(args)?;
    let harvest = match args.get("harvest") {
        None => false,
        Some(value) => value.as_bool().ok_or_else(|| {
            (
                "pet.scan_feeding_page's \"harvest\" must be a boolean".to_owned(),
                "harvest",
            )
        })?,
    };
    Ok(FeedingArguments { page, harvest })
}

/// One swept cell's protocol object. `status` is v2's three-way classification: a panel that did
/// not read is `unknown` — NEVER `finished`, because a failed read must not hide a pet the run still
/// has to feed.
pub fn feeding_cell(
    cell: usize,
    label: &str,
    match_score: Option<f64>,
    panel: Option<&seal_domain::ocr::PetPanel>,
    crop_png_base64: Option<String>,
) -> Value {
    let (status, growth, exp, line) = match panel {
        Some(panel) if panel.is_finished() => (
            "finished",
            Some(panel.growth),
            Some(panel.exp),
            Some(panel.describe()),
        ),
        Some(panel) => (
            "feedable",
            Some(panel.growth),
            Some(panel.exp),
            Some(panel.describe()),
        ),
        None => ("unknown", None, None, None),
    };
    let mut value = json!({
        "cell": cell,
        "label": label,
        "status": status,
        "growth": growth,
        "exp": exp,
        "line": line,
        "match": match_score,
    });
    if let Some(crop) = crop_png_base64 {
        value["crop_png_base64"] = Value::String(crop);
    }
    value
}

pub fn feeding_summary(cells: &[Value]) -> String {
    let count = |status: &str| {
        cells
            .iter()
            .filter(|cell| cell.get("status").and_then(Value::as_str) == Some(status))
            .count()
    };
    format!(
        "{} feedable, {} finished, {} unread",
        count("feedable"),
        count("finished"),
        count("unknown")
    )
}

/// `pet.scan_apply_queue {queue:[{label,png,rect?,species?}]}`: validate the replacement queue. An
/// EMPTY queue is refused — the same "never clear on nothing" rule the scan side follows, because
/// replacing a hand-built queue with nothing is one edit away and is the worst outcome here.
pub fn queue_apply_arguments(args: &Value) -> Result<Vec<Value>, (String, &'static str)> {
    let Some(entries) = args.get("queue").and_then(Value::as_array) else {
        return Err((
            "pet.scan_apply_queue needs \"queue\": an array of icons, or the queue is left alone"
                .to_owned(),
            "queue",
        ));
    };
    if entries.is_empty() {
        return Err((
            "pet.scan_apply_queue's \"queue\" is empty; the existing queue was left exactly as it was"
                .to_owned(),
            "queue",
        ));
    }
    let mut out = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let Some(object) = entry.as_object() else {
            return Err((
                format!("queued icon {} must be an object", index + 1),
                "queue",
            ));
        };
        let Some(png) = object
            .get("png")
            .and_then(Value::as_str)
            .filter(|png| !png.trim().is_empty())
        else {
            return Err((
                format!("queued icon {} has no png crop", index + 1),
                "queue",
            ));
        };
        // Decode now: a corrupt crop in a queue replacement must be a cheap refusal, not a run that
        // starts and cannot match anything.
        if decode_icon(png, &format!("Queued icon {}'s png", index + 1)).is_err() {
            return Err((
                format!("queued icon {}'s png is not a decodable image", index + 1),
                "queue",
            ));
        }
        let label = object
            .get("label")
            .and_then(Value::as_str)
            .filter(|label| !label.trim().is_empty())
            .map_or_else(|| format!("page cell {}", index + 1), str::to_owned);
        let mut item = json!({"label": label, "png": png});
        if let Some(rect) = object.get("rect").and_then(Value::as_array) {
            if rect.len() != 4 || rect.iter().any(|value| value.as_i64().is_none()) {
                return Err((
                    format!("queued icon {}'s rect must be [x, y, w, h]", index + 1),
                    "queue",
                ));
            }
            item["rect"] = Value::Array(rect.clone());
        }
        if let Some(species) = object
            .get("species")
            .and_then(Value::as_str)
            .filter(|species| !species.trim().is_empty())
        {
            item["species"] = Value::String(species.to_owned());
        }
        out.push(item);
    }
    Ok(out)
}

#[cfg(test)]
mod g14_tests {
    use super::*;
    use seal_domain::ocr::PetPanel;

    fn panel(finished: bool) -> PetPanel {
        PetPanel {
            stage: Some(6),
            growth: if finished { 9 } else { 7 },
            exp: if finished { 100.0 } else { 50.0 },
        }
    }

    #[test]
    fn a_failed_read_is_unknown_never_finished() {
        let unknown = feeding_cell(7, "pet", Some(0.02), None, None);
        assert_eq!(unknown["status"], "unknown");
        assert!(unknown["growth"].is_null());
        let finished = feeding_cell(8, "pet", Some(0.02), Some(&panel(true)), None);
        assert_eq!(finished["status"], "finished");
        let feedable = feeding_cell(9, "pet", Some(0.02), Some(&panel(false)), None);
        assert_eq!(feedable["status"], "feedable");
    }

    #[test]
    fn an_empty_queue_replacement_is_refused() {
        let error = queue_apply_arguments(&json!({"queue": []})).expect_err("empty is refused");
        assert_eq!(error.1, "queue");
        let error = queue_apply_arguments(&json!({"queue": [{"label": "x"}]}))
            .expect_err("a crop is required");
        assert_eq!(error.1, "queue");
    }
}

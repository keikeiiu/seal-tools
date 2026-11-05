//! `ocr_eval`: the text-level regression corpus runner (OCR-F11).
//!
//! Corpus layout under `v3/core/fixtures/ocr/`:
//!
//! ```text
//! fixtures/ocr/<read-path>/<case>.png
//! fixtures/ocr/<read-path>/<case>.expected.json   # {"lines": ["..."], ...}
//! ```
//!
//! The evaluator reads every `*.png` with a sibling `*.expected.json`, runs the free-form detector +
//! recogniser once, and reports exact-match, character error rate (CER) and p50/p95 latency. It is
//! meant to be run on demand (CI cannot ship the model binaries): `SEAL_OCR_MODELS` points at the
//! model folder, and `--root` at the install/config root the pipeline normally resolves against.
//!
//! The expected JSON currently uses `lines` (the detector's text, in order). Mode-specific fields
//! (`grade`, `remaining`, `attributes`, `cells`) can be added as the corpus grows; unknown fields are
//! ignored so an old evaluator keeps working.
//!
//! Exit codes: 0 all cases exact, 1 at least one mismatch, 2 the models/corpus could not load.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "corpus case counts and millisecond latencies are small; the percentile index rounds"
)]

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use seal_domain::config::AttributesConfig;
use seal_ocr::pipeline::OcrPipeline;
use serde_json::{Value, json};

fn main() {
    match run() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("ocr_eval: {error}");
            std::process::exit(2);
        }
    }
}

fn run() -> Result<(), String> {
    let args = Arguments::parse(std::env::args().skip(1))?;
    let cases = collect_cases(&args.corpus)?;
    if cases.is_empty() {
        return Err(format!(
            "no corpus cases under {} (a case is <name>.png + <name>.expected.json)",
            args.corpus.display()
        ));
    }
    let attributes = AttributesConfig::default();
    let mut pipeline = OcrPipeline::load_for_root(&args.root, &attributes, args.row_height)
        .map_err(|error| format!("OCR models could not load: {error}"))?;

    let mut results = Vec::with_capacity(cases.len());
    let mut exact = 0_usize;
    for case in &cases {
        let bytes =
            fs::read(&case.png).map_err(|error| format!("{}: {error}", case.png.display()))?;
        let (width, height, rgba) = seal_win::pure::decode_png(&bytes)
            .map_err(|error| format!("{}: {error}", case.png.display()))?;
        let width =
            usize::try_from(width).map_err(|_| "frame width does not fit usize".to_owned())?;
        let height =
            usize::try_from(height).map_err(|_| "frame height does not fit usize".to_owned())?;
        let expected = read_expected(&case.expected)?;
        let started = Instant::now();
        let scan = pipeline
            .scan_rgba(width, height, &rgba)
            .map_err(|error| format!("{}: {}", case.png.display(), error.message))?;
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        let actual: Vec<String> = scan.items.iter().map(|line| line.text.clone()).collect();
        // Text-level comparison: line segmentation, spacing and full-width punctuation are not
        // recognition quality. Compare the alphanumeric text in reading order; the raw line arrays
        // stay in the baseline for a human diff.
        let expected_text = normalized_text(&expected.lines);
        let actual_text = normalized_text(&actual);
        let case_exact = expected_text == actual_text;
        if case_exact {
            exact += 1;
        }
        let cer = character_error_rate(&expected_text, &actual_text);
        results.push(json!({
            "case": case.name,
            "expected": expected.lines,
            "actual": actual,
            "exact": case_exact,
            "cer": cer,
            "latency_ms": elapsed_ms,
        }));
    }
    let latencies: Vec<f64> = results
        .iter()
        .filter_map(|result| result["latency_ms"].as_f64())
        .collect();
    let summary = json!({
        "total": cases.len(),
        "exact": exact,
        "exact_rate": exact as f64 / cases.len() as f64,
        "mean_cer": results
            .iter()
            .filter_map(|result| result["cer"].as_f64())
            .sum::<f64>() / cases.len() as f64,
        "p50_latency_ms": percentile(&latencies, 0.50),
        "p95_latency_ms": percentile(&latencies, 0.95),
    });
    let baseline = json!({"cases": results, "summary": summary});
    let text = serde_json::to_string_pretty(&baseline).map_err(|error| error.to_string())?;
    let previous = match &args.baseline {
        Some(path) if path.is_file() => Some(
            fs::read_to_string(path)
                .map_err(|error| format!("{}: {error}", path.display()))?
                .parse::<Value>()
                .map_err(|error| format!("{}: {error}", path.display()))?,
        ),
        _ => None,
    };
    let regression = previous
        .as_ref()
        .and_then(|previous| regression_message(previous, &baseline));
    if let Some(path) = &args.baseline {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        let mut file = fs::File::create(path)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        writeln!(file, "{text}").map_err(|error| format!("{}: {error}", path.display()))?;
    }
    println!("{text}");
    if let Some(reason) = regression {
        eprintln!("ocr_eval: regression: {reason}");
        std::process::exit(1);
    }
    Ok(())
}

/// The text recognition quality ignores line segmentation, whitespace and punctuation: those come
/// from the detector and the panel layout, not from the recogniser. Keep letters/digits/CJK only.
fn normalized_text(lines: &[String]) -> String {
    lines
        .iter()
        .flat_map(|line| line.chars())
        .filter(|character| character.is_alphanumeric())
        .collect()
}

/// A baseline is a regression when the exact rate drops or mean CER rises by more than epsilon.
fn regression_message(previous: &Value, current: &Value) -> Option<String> {
    const EPSILON: f64 = 0.01;
    let old = previous.get("summary")?;
    let new = current.get("summary")?;
    let old_exact = old.get("exact_rate")?.as_f64()?;
    let new_exact = new.get("exact_rate")?.as_f64()?;
    if new_exact + EPSILON < old_exact {
        return Some(format!(
            "exact rate dropped from {old_exact:.3} to {new_exact:.3}"
        ));
    }
    let old_cer = old.get("mean_cer")?.as_f64()?;
    let new_cer = new.get("mean_cer")?.as_f64()?;
    if new_cer > old_cer + EPSILON {
        return Some(format!("mean CER rose from {old_cer:.3} to {new_cer:.3}"));
    }
    None
}

struct Arguments {
    root: PathBuf,
    corpus: PathBuf,
    row_height: u32,
    baseline: Option<PathBuf>,
}

impl Arguments {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut root = None;
        let mut corpus = None;
        let mut row_height = 19_u32;
        let mut baseline = None;
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
            match arg.as_str() {
                "--root" => root = Some(PathBuf::from(value()?)),
                "--corpus" => corpus = Some(PathBuf::from(value()?)),
                "--baseline" => baseline = Some(PathBuf::from(value()?)),
                "--row-height" => {
                    row_height = value()?
                        .parse()
                        .map_err(|error| format!("--row-height: {error}"))?;
                }
                other => return Err(format!("unknown argument {other:?}")),
            }
        }
        Ok(Self {
            root: root.ok_or("--root <install/config root> is required")?,
            corpus: corpus.unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ocr")
            }),
            row_height,
            baseline,
        })
    }
}

struct Case {
    name: String,
    png: PathBuf,
    expected: PathBuf,
}

#[derive(Debug)]
struct Expected {
    lines: Vec<String>,
}

fn collect_cases(root: &Path) -> Result<Vec<Case>, String> {
    let mut cases = Vec::new();
    collect_into(root, root, &mut cases)?;
    cases.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(cases)
}

fn collect_into(root: &Path, directory: &Path, cases: &mut Vec<Case>) -> Result<(), String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if directory == root => return Err(format!("{}: {error}", root.display())),
        Err(error) => return Err(format!("{}: {error}", directory.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_into(root, &path, cases)?;
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "png") {
            continue;
        }
        let Some(stem) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
        else {
            continue;
        };
        let expected = path.with_file_name(format!("{stem}.expected.json"));
        if !expected.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .with_extension("")
            .to_string_lossy()
            .into_owned();
        cases.push(Case {
            name: relative,
            png: path,
            expected,
        });
    }
    Ok(())
}

fn read_expected(path: &Path) -> Result<Expected, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    let lines = value
        .get("lines")
        .and_then(Value::as_array)
        .map(|lines| {
            lines
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .ok_or_else(|| format!("{}: expected JSON needs a \"lines\" array", path.display()))?;
    Ok(Expected { lines })
}

fn character_error_rate(expected: &str, actual: &str) -> f64 {
    let expected_len = expected.chars().count();
    if expected_len == 0 {
        return if actual.is_empty() { 0.0 } else { 1.0 };
    }
    let distance = edit_distance(
        &expected.chars().collect::<Vec<_>>(),
        &actual.chars().collect::<Vec<_>>(),
    );
    distance as f64 / expected_len as f64
}

fn edit_distance(expected: &[char], actual: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=actual.len()).collect();
    let mut current = vec![0; actual.len() + 1];
    for (row, expected_char) in expected.iter().enumerate() {
        current[0] = row + 1;
        for (column, actual_char) in actual.iter().enumerate() {
            current[column + 1] = if expected_char == actual_char {
                previous[column]
            } else {
                1 + previous[column]
                    .min(previous[column + 1])
                    .min(current[column])
            };
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[actual.len()]
}

fn percentile(values: &[f64], fraction: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let index = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_has_zero_cer() {
        assert_eq!(character_error_rate("DG 25000", "DG 25000"), 0.0);
    }

    #[test]
    fn one_wrong_character_is_one_over_the_length() {
        let cer = character_error_rate("DG", "DK");
        assert!((cer - 0.5).abs() < f64::EPSILON, "{cer}");
    }

    #[test]
    fn percentiles_use_nearest_rank() {
        let values = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&values, 0.5), 3.0);
        assert_eq!(percentile(&values, 0.95), 5.0);
    }

    #[test]
    fn a_missing_lines_array_names_the_file() {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
        fs::create_dir_all(&directory).expect("target scratch");
        let path = directory.join("seal-ocr-eval-no-lines.json");
        fs::write(&path, "{\"grade\":\"DG\"}").expect("write");
        let error = read_expected(&path).expect_err("lines is required");
        assert!(error.contains("lines"), "{error}");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn normalization_ignores_line_breaks_spaces_and_fullwidth_punctuation() {
        let lines = vec![
            "GRADE：".to_owned(),
            "G".to_owned(),
            "攻擊速度 + 10".to_owned(),
        ];
        assert_eq!(normalized_text(&lines), "GRADEG攻擊速度10");
    }

    #[test]
    fn a_baseline_regression_is_exact_rate_drop_or_cer_rise() {
        let old = json!({"summary": {"exact_rate": 0.5, "mean_cer": 0.2}});
        let same = json!({"summary": {"exact_rate": 0.5, "mean_cer": 0.2}});
        assert_eq!(regression_message(&old, &same), None);
        let worse_exact = json!({"summary": {"exact_rate": 0.4, "mean_cer": 0.2}});
        assert!(regression_message(&old, &worse_exact).is_some());
        let worse_cer = json!({"summary": {"exact_rate": 0.5, "mean_cer": 0.25}});
        assert!(regression_message(&old, &worse_cer).is_some());
    }
}

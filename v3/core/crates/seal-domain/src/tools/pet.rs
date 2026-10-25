//! The resident pet feeder's pure half — PART 1: the boarding window's READERS and the derived
//! layout. The visit decisions and their messages land as part 2 in this same module.
//!
//! # What this module owns, and what it deliberately does not
//!
//! The pet feeder is the resident tool: it can run for days, it shares the game with the other
//! tools, and it is the one that keeps working while nobody is watching. That makes it the most
//! failure-prone tool in the suite, and it is why the split below is written down rather than
//! discovered:
//!
//! - **HERE (part 1):** what the boarding window SAYS — the per-row time line and its completion form,
//!   the per-row feeding rate the game prints, and where a row's food slots / count crops / time-line
//!   band ARE (derived from one dragged strip as ratios of the slot height).
//! - **HERE (part 2, next):** pure VISIT decisions over the task-4 [`crate::ocr::PetPanel`] rows and
//!   the run's session state — which row to look at, reload, feed or leave, and the messages the card
//!   and the logs show.
//! - **NOT HERE — the `seal-tools` ARBITER's:** when the feeder may TAKE the game, how it behaves
//!   while another tool holds it ("armed while another tool holds the game"), how the card keeps a
//!   standing line while it defers, and how it yields/re-acquires. Those are inter-tool scheduling
//!   rules owned by `seal-tools`; the pet half only supplies the FACTS the arbiter needs ("wants the
//!   game now", "waiting until T", what it is showing on the card). Duplicating a deferral rule here
//!   would give two owners of one behaviour.
//!
//! Ported from `v2/SealTools.Core/FeederEta.cs`, `FeederLayout.cs` and their test oracles
//! (`FeederEtaTests.cs`, `FeederLayoutTests.cs`). The strings in those tests are REAL — read off a
//! live window on 2026-09-21 and out of `pet.log` — and are kept verbatim.
//!
//! No new regex dependency: the three patterns in `FeederEta` are small and explicit, matching .NET's
//! first-match/greedy-with-backtracking behaviour for the shapes they meet.

use crate::geometry::round_ties_even;
use crate::ocr::PetPanel;
use crate::tools::shop::BagGrid;

/// What a boarding row's time line says: how many minutes, and whether it is the COMPLETION form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    /// The number before 分.
    pub minutes: i32,
    /// True for the completion form (`代養完成預計所需時間`), false for the level form
    /// (`到N為止預計所需時間`).
    pub is_completion: bool,
    /// The line as read (trimmed), because the card shows it and only the player can tell whether a
    /// mangled read still meant the thing it looks like.
    pub line: String,
}

/// What the boarding window says about TIME — the game's own answer to "when should this row be
/// looked at again".
///
/// Each row carries two lines of text under its food slots, and the second changes form depending on
/// where the pet is in its run. Read off a live window (2026-09-21):
///
/// ```text
/// 代養完成預計所需時間：約14分        boarding COMPLETES in about 14 min   ← a pet about to finish
/// 到2為止預計所需時間：約 20分        reaches level 2 in about 20 min      ← a pet mid-run
/// 該欄位約27日9時51分16 秒後到期。     the food in this slot expires in 27d   ← shelf life, not a time-to
/// 每1分 攝取3個。                     consumes 3 per minute
/// ```
///
/// **The completion form is the one that matters**: a pet that finishes is mailed, the row goes
/// empty, and without this line the row waits out its whole cycle with nothing in the tray.
/// **Only the completion form drives a schedule** — the level form resets every level and reads 20-39
/// minutes on a healthy pet, so treating it as a revisit trigger would visit constantly.
pub struct FeederEta;

impl FeederEta {
    /// The GROWTH (餵養值, +0 … +9) a line says the pet is heading for, or `None` when it names none.
    ///
    /// THREE NUMBERS LIVE NEAR EACH OTHER, and the first version confused two of them — the player
    /// caught it: *"there is no stage 8 stage 9."* The hover panel's `(6階)` is the **stage**; the
    /// panel's `+7` and this line's `到9為止` are the **growth**, which ends at +9/100 % — the
    /// finished pet. DISPLAY ONLY: the level form drives nothing.
    pub fn growth_target(line: Option<&str>) -> Option<i32> {
        let line = line?;
        let characters: Vec<char> = line.chars().collect();
        let growth = match_growth_to(&characters)?;
        (0..=PetPanel::MAX_GROWTH)
            .contains(&growth)
            .then_some(growth)
    }

    /// `每1分 攝取3個` — the game's own statement of the FEEDING RATE, per row.
    ///
    /// MATCHED LOOSELY, because the live OCR mangles this line far worse than a dictionary word: the
    /// forms actually in `pet.log` are `每1分最取适` (37 times — the ITEM COUNT IS GONE),
    /// `每分最取尚`, `每1分瑜取3固` and `每1分输取3固`. So the ending is NOT matched at all — `個`
    /// arrives as `固`, `适`, `尚` — and only the two NUMBERS are required. When the item count did not
    /// survive there is nothing to read and this returns `None`, which is the honest answer (the
    /// configured rate stands; it is the fallback).
    ///
    /// Implausible values are rejected rather than clamped: a misread rate would silently rescale
    /// every time this row computes.
    pub fn parse_rate(lines: &[String]) -> Option<i32> {
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let characters: Vec<char> = line.chars().collect();
            let Some((minutes, items)) = match_rate_line(&characters) else {
                continue;
            };
            if minutes <= 0 {
                continue;
            }
            // Whole items per minute, 1..30: the config's own scale, and wide enough that no honest
            // line is refused while a two-digit misread still is.
            if items <= 0 || items > 30 * minutes {
                continue;
            }
            let rate = round_ties_even(f64::from(items) / f64::from(minutes));
            if (1..=30).contains(&rate) {
                return Some(rate);
            }
        }
        None
    }

    /// Reads the time line out of a row's text, or `None` when there is nothing to read.
    ///
    /// `None` is an ordinary answer — an older window, a mangled read, a pet whose line says
    /// something else — and the caller falls back to the food figure.
    pub fn parse(lines: &[String]) -> Option<Reading> {
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            // The food's shelf life carries a 約 and a 分-shaped number too; 到期 is what tells them
            // apart, and the anchor alone would not.
            if line.contains('到') && line.contains('期') {
                continue;
            }

            let characters: Vec<char> = line.chars().collect();
            let Some(minutes) = match_about_minutes(&characters) else {
                continue;
            };

            // COMPLETION FIRST: that line also carries 所需時間, so testing for the level form first
            // would classify a finished pet as one that is merely levelling.
            if line.contains('完') {
                return Some(Reading {
                    minutes,
                    is_completion: true,
                    line: line.trim().to_owned(),
                });
            }
            if line.contains("所需時間") {
                return Some(Reading {
                    minutes,
                    is_completion: false,
                    line: line.trim().to_owned(),
                });
            }
        }
        None
    }

    /// How long until the row next needs looking at BECAUSE OF THE PET — `None` unless the game said
    /// the boarding completes, in which case the pet is about to be mailed and the row emptied.
    pub fn completion_minutes(reading: Option<&Reading>) -> Option<i32> {
        reading
            .filter(|reading| reading.is_completion)
            .map(|reading| reading.minutes)
    }
}

fn skip_whitespace(characters: &[char], from: usize) -> usize {
    let mut at = from;
    while at < characters.len() && characters[at].is_whitespace() {
        at += 1;
    }
    at
}

fn digit_run(characters: &[char], from: usize, limit: usize) -> (usize, String) {
    let mut at = from;
    let mut digits = String::new();
    while at < characters.len() && digits.len() < limit && characters[at].is_ascii_digit() {
        digits.push(characters[at]);
        at += 1;
    }
    (digits.len(), digits)
}

/// `約\s*(\d{1,4})\s*分`.
fn match_about_minutes(characters: &[char]) -> Option<i32> {
    for start in 0..characters.len() {
        if characters[start] != '約' {
            continue;
        }
        let digits_start = skip_whitespace(characters, start + 1);
        let (count, _) = digit_run(characters, digits_start, 4);
        // Greedy with backtracking: four digits first, then fewer — the regex engine's behaviour.
        for taken in (1..=count).rev() {
            let unit = skip_whitespace(characters, digits_start + taken);
            if unit < characters.len() && characters[unit] == '分' {
                let digits: String = characters[digits_start..digits_start + taken]
                    .iter()
                    .collect();
                if let Ok(minutes) = digits.parse() {
                    return Some(minutes);
                }
            }
        }
    }
    None
}

/// `到\s*(\d{1,2})\s*[為为]止`.
fn match_growth_to(characters: &[char]) -> Option<i32> {
    for start in 0..characters.len() {
        if characters[start] != '到' {
            continue;
        }
        let digits_start = skip_whitespace(characters, start + 1);
        let (count, _) = digit_run(characters, digits_start, 2);
        for taken in (1..=count).rev() {
            let unit = skip_whitespace(characters, digits_start + taken);
            if unit + 1 < characters.len()
                && matches!(characters[unit], '為' | '为')
                && characters[unit + 1] == '止'
            {
                let digits: String = characters[digits_start..digits_start + taken]
                    .iter()
                    .collect();
                if let Ok(growth) = digits.parse() {
                    return Some(growth);
                }
            }
        }
    }
    None
}

/// `每\s*(\d{1,3})\s*分[^\d到約為止]{0,6}(\d{1,4})`.
fn match_rate_line(characters: &[char]) -> Option<(i32, i32)> {
    // What the gap class excludes. Note 為 (not 为) and 約 (not 约) — the committed pattern's own
    // characters, kept exactly because a gap that admitted them could join the rate line to a time
    // line the way `pet.log` shows it arriving.
    let excluded =
        |character: char| character.is_numeric() || matches!(character, '到' | '約' | '為' | '止');

    for start in 0..characters.len() {
        if characters[start] != '每' {
            continue;
        }
        let minutes_start = skip_whitespace(characters, start + 1);
        let (count, _) = digit_run(characters, minutes_start, 3);
        for taken in (1..=count).rev() {
            let unit = skip_whitespace(characters, minutes_start + taken);
            if unit >= characters.len() || characters[unit] != '分' {
                continue;
            }

            // Greedy gap of up to six characters none of which is a digit or the time line's own
            // characters; the item count then follows.
            let mut at = unit + 1;
            let mut gap = 0;
            while at < characters.len() && gap < 6 && !excluded(characters[at]) {
                at += 1;
                gap += 1;
            }
            let (item_count, items) = digit_run(characters, at, 4);
            if item_count == 0 {
                continue;
            }

            let minutes: String = characters[minutes_start..minutes_start + taken]
                .iter()
                .collect();
            let (Ok(minutes), Ok(items)) = (minutes.parse(), items.parse()) else {
                continue;
            };
            return Some((minutes, items));
        }
    }
    None
}

// ── where a row's slots and crops are (FeederLayout) ────────────────────────────────────────

/// Where a boarding row's food slots are, and where the count/time crops sit inside them — four
/// drags (one strip per row), and nothing else.
///
/// The calibration it replaces was one hand-drawn box per food slot: fourteen boxes, all the same
/// shape repeated. A middle revision asked for six drags on the reasoning that a MEASURED offset
/// beats a proportional one; the reasoning was sound and the drag was not — the band of left edges
/// that reads correctly is about THREE PIXELS wide, and the player, asked twice to draw a box into
/// it, missed it twice. What the crop alone cannot fix is the READER's knife-edge, which is why the
/// tool reads up to three upscales before giving up; the first rung is this crop at the usual scale.
pub struct FeederLayout;

impl FeederLayout {
    /// Where the count is read from inside a slot: this far across it, running to its own right edge
    /// at its FULL HEIGHT.
    ///
    /// MEASURED, and the only approach of four that has held: two crops read at 0.369-0.431 of the
    /// slot ("138" and "300"); a scan of every left edge a pixel apart put the good band there and
    /// nowhere else. FULL HEIGHT is not a preference either: a crop cut to the digits' height finds
    /// NOTHING while holding a perfectly legible number. A SETTING rather than a constant, because
    /// the band is narrow — a machine that reads nothing has to move it without a rebuild.
    pub const READ_LEFT_FRACTION: f64 = 0.40;

    /// The slot boxes of one row, from a strip dragged across its food slots.
    ///
    /// Divided evenly, which is an approximation: the player's own calibration had slots 62, 64, 66
    /// and 62 apart in one row. That is harmless for the FOOD DRAG and it is the thinnest part of the
    /// count read — worth knowing rather than discovering.
    ///
    /// Integer arithmetic throughout, on the SLOT EDGES rather than on the width divided and then
    /// re-multiplied: rounding the width first leaves a sliver of unclaimed space at the right end,
    /// which is exactly where the count is read from.
    ///
    /// `None` when the strip or the stack count is unusable, so a half-calibrated row yields no slots
    /// rather than a row of boxes at (0,0).
    pub fn slot_boxes(strip: Option<&[i32]>, stacks: i32) -> Option<Vec<Vec<i32>>> {
        let strip = strip.filter(|strip| BagGrid::is_valid_rect(Some(strip)))?;
        if stacks < 1 {
            return None;
        }

        let mut boxes = Vec::with_capacity(stacks as usize);
        for index in 0..stacks {
            let width = i64::from(strip[2]);
            let stacks64 = i64::from(stacks);
            let left = strip[0] + (width * i64::from(index) / stacks64) as i32;
            let right = strip[0] + (width * (i64::from(index) + 1) / stacks64) as i32;
            boxes.push(vec![left, strip[1], (right - left).max(1), strip[3]]);
        }
        Some(boxes)
    }

    /// Where a row's TIME line is read from — the text under its food slots, derived from the strip
    /// the same way the count crop is.
    ///
    /// **DERIVED AS RATIOS OF THE SLOT HEIGHT**, not as fixed pixels: the strip is calibrated per
    /// machine, so a PC whose slots are taller gets a proportionally lower and taller region without
    /// anything to calibrate.
    ///
    /// **AND DELIBERATELY GENEROUS VERTICALLY.** Two measured rows agreed on `+34`, so this used to
    /// aim a 24-tall box at it: correct on both, fail-silent on a third. The free row carries an extra
    /// line above its timer that pushes the timer to about `+57` — inside the old box by 7 pixels,
    /// which is nothing, and outside it by 7 in the other direction, which is TOTAL, because a crop
    /// that misses returns nothing rather than something wrong. Widening is free here: the parser
    /// PATTERN-MATCHES and skips the expiry line, a bare count cannot match `約N分`, and the new band
    /// is a strict superset of the old — **no row that read before can stop reading**.
    pub fn eta_region(strip: Option<&[i32]>) -> Option<Vec<i32>> {
        let strip = strip.filter(|strip| BagGrid::is_valid_rect(Some(strip)))?;
        let height = f64::from(strip[3]);
        Some(vec![
            strip[0] - round_ties_even(height * 0.10),
            strip[1] + strip[3] + round_ties_even(height * 0.20),
            round_ties_even(height * 4.7),
            round_ties_even(height * 1.20),
        ])
    }

    /// The region a count is read from for one slot.
    pub fn read_region(slot: Option<&[i32]>, left_fraction: f64) -> Option<Vec<i32>> {
        let slot = slot.filter(|slot| BagGrid::is_valid_rect(Some(slot)))?;
        let left = slot[0] + round_ties_even(f64::from(slot[2]) * left_fraction);
        let right = slot[0] + slot[2];
        if right - left < 4 {
            return None;
        }
        Some(vec![left, slot[1], right - left, slot[3]])
    }
}

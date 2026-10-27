//! The pet feeder's pure VISIT decisions — PART 2: the schedule from a row's read, the veto, the
//! row outcomes and their messages, the card, and the Start refusals.
//!
//! What is NOT here, and why: the resident/deferral rules — when the feeder may TAKE the game, how it
//! behaves while another tool holds it, how it waits and re-acquires — belong to the `seal-tools`
//! arbiter. This module supplies the FACTS that arbiter needs ([`PetCard::facts`]: whether a row wants
//! the game NOW, and when the next action is due) as plain data, and never decides the deferral.
//!
//! Times are MINUTES FROM NOW (`f64`), not wall clocks: the decision half has no clock, so anything
//! the player reads as `HH:mm` is formatted by the caller from [`FoldResult::next_minutes`] and the
//! card's `next_minutes` / `next_label` pair.

use super::FeederLayout;
use crate::decision::StopReason;
use crate::ocr::PetPanel;

// ── constants (PetTool.cs / PetConfig.cs), the ones a decision depends on ───────────────────

/// The most items a stack can hold — a MAX load. A number above this is a misread, not a count.
pub const STACK_SIZE: i32 = 300;
/// How many reloads may fail in a row before the ROW is dropped (the other rows carry on).
pub const MAX_FAILURES: i32 = 3;
/// A row retried after a failure.
pub const RETRY_MINUTES: f64 = 5.0;
/// A row waiting for the BAG to hold a feedable pet — longer because it is not a fault to recover
/// from: nothing changes until the player puts one in, or a loader returns one.
pub const NO_PET_RETRY_MINUTES: f64 = 30.0;
/// How much fresh feeding time makes a timer-driven reload not worth doing.
pub const VETO_MARGIN_MINUTES: f64 = 30.0;
/// Above this difference the slot is no longer showing the BOARDING WINDOW at all, so the answer must
/// be "unknown" rather than "occupied". Measured: empty slot 0.000, pet 0.65-0.785, not-the-window
/// 0.91+; 0.85 sits in the empty stretch.
pub const PET_SLOT_NOT_THE_WINDOW_ABOVE: f64 = 0.85;
/// How unlike the pet a bag cell may look and still be clicked.
pub const MATCH_LIMIT: f64 = 0.5;

/// How many times to re-click the pet before giving up (the failure is intermittent; the cursor is
/// verified on target).
pub const PLACE_ATTEMPTS: i32 = 3;

// ── reading a food stack's count (FeederCount.cs) ──────────────────────────────────────────

/// What the boarding window drew on a food slot: the digits of the best line above the score gate.
///
/// READ FROM A CROP, NOT FROM THE SLOT: the detector finds no text in a whole-slot image (the food
/// icon fills the frame) but reads the same digits at FULL HEIGHT — and a HALF-height crop finds
/// nothing again, so "crop tighter" is not the rule either.
///
/// The number is only accepted when TWO crops a few pixels apart agree, because the window is about
/// three pixels wide: a slightly-off crop does not fail blankly, it answers WRONGLY ("300" → "0" at
/// 0.56, "84" → "4" at 0.89). A wrong count schedules a pet to starve or overfeeds it.
pub struct FeederCount;

impl FeederCount {
    pub const MAX_STACK: i32 = STACK_SIZE;
    /// A genuine count scores 0.90-1.00; everything the food icon produces scores at most 0.63. 0.75
    /// sits inside that gap, and also rejects a genuine read taken at the very edge of the window.
    pub const MIN_SCORE: f64 = 0.75;
    /// Below this fraction of warm (red-over-blue) pixels a slot holds no food: measured 0.0 % empty,
    /// 30.4-37.0 % with food, so 2 % is an order of magnitude inside the gap in both directions.
    pub const EMPTY_SLOT_BELOW: f64 = 0.02;

    /// The number out of one read: the digits of the BEST-scoring line above `min_score`, or `None`.
    ///
    /// Best-scoring rather than concatenating everything, because a real read comes back with junk
    /// beside it — the same crop returned "300" at 1.00 next to a junk "0" at 0.56 — and the junk can
    /// outscore a genuine reading at a bad crop. Taking the single best line is what makes the gate
    /// mean anything.
    pub fn parse(lines: &[(String, f64)], min_score: f64) -> Option<i32> {
        let best = lines
            .iter()
            .filter(|(_, score)| *score >= min_score)
            .max_by(|a, b| a.1.total_cmp(&b.1))?;

        let digits: String = best.0.chars().filter(char::is_ascii_digit).collect();
        if digits.is_empty() || digits.len() > 3 {
            return None;
        }
        let count: i32 = digits.parse().ok()?;
        (0..=Self::MAX_STACK).contains(&count).then_some(count)
    }
}

// ── the feeding table (PetFeeding.cs) ──────────────────────────────────────────────────────

/// How much a pet still needs, and how long that is — from the scraped table (`docs/pet-data.csv`).
///
/// THE TABLE IS THE ONLY SOURCE, and that is the player's decision rather than a shortcut: *"the base
/// feeding value is predetermined, nowhere can you find it in the game. We believe the data."*
///
/// It records, all measured: `cost(+n) = wyz × (1 + n/10)` for n = 0…9, and the whole run to
/// `+9/100%` is `wyz × 14.5` (the +9 bar fills too — 14.5, not 9). The table also carries the total
/// value AND total minutes per pet, which collapses to one line: **whatever fraction of the total
/// value is left is the same fraction of the total minutes.**
///
/// `(species, stage)` identifies a line, NOT the name: the game is Traditional and the table is
/// Simplified, and the recorded rule is never to string-match one against the other.
pub struct PetFeeding;

/// One pet LINE at one stage.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedingLine {
    pub species: String,
    pub pet_id: i32,
    pub name: String,
    pub stage: i32,
    pub wyz: f64,
    pub total_value: f64,
    pub food_items: f64,
    pub minutes: f64,
}

impl PetFeeding {
    /// The `.G` pets' value: stage 7, unboardable, and dropping them on load is what makes
    /// `(species, stage)` a key (seven of the sixty-nine pairs are ambiguous with them in).
    pub const UNBOARDABLE_WYZ: f64 = 33000.0;

    /// Parses the shipped CSV text. A row that cannot parse is skipped, not fatal: a schedule that
    /// cannot be computed falls back to the configured cycle, which is what the tool did before any
    /// of this existed.
    pub fn parse(text: &str) -> Vec<FeedingLine> {
        let normalised = text.replace("\r\n", "\n");
        let rows: Vec<&str> = normalised.split('\n').collect();
        let mut lines = Vec::new();

        for row in rows.iter().skip(1) {
            let fields: Vec<&str> = row.split(',').collect();
            if fields.len() < 8 {
                continue;
            }
            let Some(stage) = fields[3].trim().parse::<i32>().ok() else {
                continue;
            };
            let (Some(wyz), Some(total), Some(items), Some(minutes)) = (
                parse_number(fields[4]),
                parse_number(fields[5]),
                parse_number(fields[6]),
                parse_number(fields[7]),
            ) else {
                continue;
            };
            if wyz == Self::UNBOARDABLE_WYZ {
                continue;
            }
            lines.push(FeedingLine {
                species: fields[0].trim().to_owned(),
                pet_id: fields[1].trim().parse().unwrap_or(0),
                name: fields[2].trim().to_owned(),
                stage,
                wyz,
                total_value: total,
                food_items: items,
                minutes,
            });
        }
        lines
    }

    /// The line for a species at a stage — `None` when the table has no such pet. A species spans
    /// five stages or seven, so a stage outside its range is a species that was named wrongly rather
    /// than a pet the table is missing.
    pub fn find<'a>(
        table: &'a [FeedingLine],
        species: Option<&str>,
        stage: i32,
    ) -> Option<&'a FeedingLine> {
        let species = species?;
        table
            .iter()
            .find(|line| line.species == species && line.stage == stage)
    }

    /// What is left to feed, in the table's own units: the levels above the pet's own, plus the part
    /// of its current bar still to fill.
    ///
    /// At `+9` 100 % this is zero — the goal state. At `+0` 0 % it is `wyz × 14.5`, the table's own
    /// total.
    pub fn remaining_value(wyz: f64, growth: i32, exp_percent: f64) -> f64 {
        let exp = clamp_exp(exp_percent);
        if growth >= PetPanel::MAX_GROWTH {
            return wyz * (1.0 + f64::from(PetPanel::MAX_GROWTH) / 10.0) * (1.0 - exp / 100.0);
        }

        let mut remaining = wyz * (1.0 + f64::from(growth) / 10.0) * (1.0 - exp / 100.0);
        for level in growth + 1..=PetPanel::MAX_GROWTH {
            remaining += wyz * (1.0 + f64::from(level) / 10.0);
        }
        remaining
    }

    /// How long the pet still needs, in minutes — `None` when the line cannot answer, which the
    /// caller treats as "no estimate".
    pub fn minutes_to_finish(
        line: Option<&FeedingLine>,
        growth: i32,
        exp_percent: f64,
    ) -> Option<f64> {
        let line = line?;
        if line.total_value <= 0.0 || line.minutes <= 0.0 {
            return None;
        }
        let remaining = Self::remaining_value(line.wyz, growth, exp_percent);
        Some(remaining / line.total_value * line.minutes)
    }
}

fn parse_number(field: &str) -> Option<f64> {
    field.trim().parse().ok()
}

/// An EXP% outside 0-100 is a misread; CLAMPED rather than rejected, because the direction that
/// matters is the safe one — a `100 %` that reads as `110` must still mean "done".
fn clamp_exp(exp_percent: f64) -> f64 {
    exp_percent.clamp(0.0, 100.0)
}

// ── the schedule from one row's read (ScheduleFor / RowReading) ─────────────────────────────

/// Where a row's next reload came from. Only [`ScheduleSource::CountRead`] is FOOD EVIDENCE for the
/// veto, and even that one needs the slot read as occupied on the same pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleSource {
    /// A feeder count came back and there is food in the tray.
    CountRead,
    /// The game's own completion line parsed — not food evidence on its own.
    CompletionRead,
    /// Counts came back as zero: reload now.
    CountEmpty,
    /// The row is not boarding: reload now.
    NotBoarding,
    /// Nothing readable: the configured cycle is assumed.
    CycleFallback,
    /// RELOAD EVERY ROW ON START — every row is due; no reading can cancel that.
    StartOverride,
}

impl ScheduleSource {
    /// The words a person reads in `pet.log`, not the enum name.
    pub fn describe(self) -> &'static str {
        match self {
            Self::CountRead => "a feeder count was read, with food in it",
            Self::CompletionRead => "only the game's completion line was read",
            Self::CountEmpty => "the counts read zero",
            Self::NotBoarding => "the row is not boarding",
            Self::CycleFallback => "nothing was readable, so the configured cycle is assumed",
            Self::StartOverride => "RELOAD EVERY ROW ON START",
        }
    }
}

/// When a row next needs a reload, from what was read about it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowReading {
    /// Minutes from now.
    pub next_minutes: f64,
    pub source: ScheduleSource,
    /// Whether THIS pass read the slot as occupied — the veto's first gate. False when the slot could
    /// not be read at all: unknown must keep meaning act.
    pub slot_occupied: bool,
}

/// Per-row rate: the game's own line read for THIS row when there was one, else the configured
/// fallback — clamped to at least 1 so a broken config cannot divide by zero or spin the loop.
pub fn rate_for(measured: Option<i32>, configured: i32) -> i32 {
    measured.unwrap_or_else(|| configured.max(1))
}

/// Items one reload puts into a row, derived from that row's stack count so the two cannot disagree.
pub fn load_items(stacks: i32) -> i32 {
    stacks.max(1) * STACK_SIZE
}

/// Minutes that row's load lasts, at the given rate.
pub fn load_minutes(stacks: i32, items_per_minute: i32) -> i32 {
    if items_per_minute > 0 {
        load_items(stacks) / items_per_minute
    } else {
        0
    }
}

/// Minutes between reloads for a row: the load, plus the wait past empty — with the tool's own guard
/// (`max(5, …)`, and 60 when the rate is broken so a misconfiguration cannot spin the loop).
pub fn cycle_minutes(stacks: i32, items_per_minute: i32, wait_after_empty_minutes: i32) -> f64 {
    let full = load_minutes(stacks, items_per_minute);
    if full <= 0 {
        return 60.0;
    }
    f64::from(full + wait_after_empty_minutes).max(5.0)
}

/// The configured half of a row's schedule — the same for every row of a setup.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetScheduleSettings {
    pub items_per_minute: i32,
    pub wait_after_empty_minutes: i32,
    pub reload_on_start: bool,
}

/// What one row's READ said, as the schedule needs it. `rate` is [`rate_for`]'s answer for THIS row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScheduleRead {
    pub boarding_running: bool,
    pub items_left: Option<i32>,
    pub completion_minutes: Option<f64>,
    pub rate: i32,
    pub stacks: i32,
    pub slot_occupied: bool,
}

/// `ScheduleFor`: when this row next needs a reload, from what was read about it.
///
/// `items_left` is the summed feeder count when it could be read. ZERO IS RELOAD NOW, with no margin
/// — the wait exists to arrive AFTER a tray that still has food has drained; a tray already reading
/// zero IS past empty. (`"if we know it is empty, why wait another 5 minutes?"` — the player, and the
/// answer was that it was sleep for its own sake, costing the pet food.)
///
/// `None` means either "not boarding" (reload now — it needs a pet and a fill) or "couldn't read"
/// (cycle fallback, which assumes a full load). The two are different and the source says which.
pub fn schedule_for(read: &ScheduleRead, settings: &PetScheduleSettings) -> RowReading {
    let slot_occupied = read.slot_occupied;
    if settings.reload_on_start {
        return RowReading {
            next_minutes: 0.0,
            source: ScheduleSource::StartOverride,
            slot_occupied,
        };
    }
    if !read.boarding_running {
        return RowReading {
            next_minutes: 0.0,
            source: ScheduleSource::NotBoarding,
            slot_occupied,
        };
    }

    // THE GAME'S OWN COMPLETION TIME, when it gives one: a pet that finishes is MAILED, so the row
    // will be empty then and wants a new one.
    let finish = read.completion_minutes;

    let Some(items) = read.items_left else {
        return match finish {
            Some(finish) => RowReading {
                next_minutes: finish + f64::from(settings.wait_after_empty_minutes),
                source: ScheduleSource::CompletionRead,
                slot_occupied,
            },
            None => RowReading {
                next_minutes: cycle_minutes(
                    read.stacks,
                    settings.items_per_minute,
                    settings.wait_after_empty_minutes,
                ),
                source: ScheduleSource::CycleFallback,
                slot_occupied,
            },
        };
    };

    if items <= 0 {
        return RowReading {
            next_minutes: 0.0,
            source: ScheduleSource::CountEmpty,
            slot_occupied,
        };
    }

    // WHICHEVER RUNS OUT FIRST — the food in the tray, or the pet itself. The source stays CountRead
    // even when the finish wins the min: a count was read and it was above zero, which is the food
    // evidence the veto needs.
    let food = (f64::from(items) / f64::from(read.rate.max(1))).max(0.0);
    let base = match finish {
        Some(finish) => food.min(finish),
        None => food,
    };
    RowReading {
        next_minutes: base + f64::from(settings.wait_after_empty_minutes),
        source: ScheduleSource::CountRead,
        slot_occupied,
    }
}

/// `ConsiderVeto`: whether a fresh reading lets this row's reload be SKIPPED.
///
/// POSITIVE-ONLY, and that is the whole safety: a skip needs FOOD SEEN on this visit — the slot
/// freshly read as occupied, a count that came back as a number, and that number above zero. Every
/// other state ACTS: an unreadable count, an unreadable slot, a completion line with no count (no
/// evidence it is trustworthy on a dry tray), and the cycle fallback. A misread count that says "full"
/// would skip a needed reload, and an unfed pet cannot be undone.
///
/// `strikes` is only logged — a veto leaves the count untouched, because the row proved it can be fed.
pub fn consider_veto(fresh: &RowReading, strikes: i32) -> (bool, String) {
    let standing = if strikes > 0 {
        format!(" ({strikes} strike(s) standing)")
    } else {
        String::new()
    };

    if fresh.source == ScheduleSource::StartOverride {
        return (false, "RELOAD EVERY ROW ON START is on — acting".to_owned());
    }
    if !fresh.slot_occupied {
        return (
            false,
            format!(
                "the slot was not read as occupied ({}) — acting{standing}",
                fresh.source.describe()
            ),
        );
    }
    if fresh.source != ScheduleSource::CountRead {
        return (
            false,
            format!(
                "no count came back ({}) — acting, because unknown must never cancel a reload{standing}",
                fresh.source.describe()
            ),
        );
    }
    if fresh.next_minutes <= VETO_MARGIN_MINUTES {
        return (
            false,
            format!(
                "the fresh read leaves {:.0} min of food (within the {:.0} min margin) — acting{standing}",
                fresh.next_minutes, VETO_MARGIN_MINUTES
            ),
        );
    }
    (
        true,
        format!(
            "the timer had it due, but the fresh read found food still in the feeder — {:.0} min to go \
             (> {:.0} min margin). Nothing ended, moved, loaded or started; its next comes from this \
             reading{standing}",
            fresh.next_minutes, VETO_MARGIN_MINUTES
        ),
    )
}

// ── one row's outcome from a visit (RowOutcome / FoldBack) ─────────────────────────────────

/// One row's result from a visit.
#[derive(Debug, Clone, PartialEq)]
pub enum RowOutcome {
    /// The reload ran: a pet went in and food was loaded.
    Reloaded {
        /// How long the boarded pet still needs (from the hover panel), or `None`.
        pet_minutes: Option<f64>,
        /// The game's own finish time read AFTER the reload — the reading taken before describes the
        /// pet that was just taken out.
        landed_line_minutes: Option<f64>,
    },
    /// No RELOAD was attempted: the boarding window would not come up. A WAIT, not a failure — the
    /// open itself did click, but the row's own steps never ran, so nothing is counted against it.
    Deferred,
    /// The bag holds no pet that can be fed — every queued one has finished, or none is there. Also a
    /// WAIT: the player may simply not have another pet yet.
    NothingToBoard,
    /// A real failure, with the row's reason.
    Failed { error: String },
}

/// What folding one row's outcome back does to the schedule and the card.
#[derive(Debug, Clone, PartialEq)]
pub struct FoldResult {
    pub failures: i32,
    /// `None` when the row has been GIVEN UP ON (dropped from the schedule).
    pub next_minutes: Option<f64>,
    pub message: String,
    /// For the log only, when the row is dropped.
    pub dropped_log: Option<String>,
}

/// Folds one visit's row result into the schedule and the strike counts — the row rules the tool has
/// always had, in ONE place because there are two callers (the main loop and the run's opening look's
/// hand-off). A run whose first reload followed different rules from its second would be a run nobody
/// can reason about.
///
/// `next_label` is the caller's formatted `HH:mm` for the "reloaded, next …" line — the decision half
/// has no clock.
pub fn fold_back(
    row: &str,
    outcome: &RowOutcome,
    failures: i32,
    loaded_minutes: i32,
    wait_after_empty_minutes: i32,
    next_label: &str,
) -> FoldResult {
    match outcome {
        RowOutcome::Reloaded {
            pet_minutes,
            landed_line_minutes,
        } => {
            // WHICHEVER RUNS OUT FIRST: what the pet still needs, or what was just loaded.
            let from_pet = match (pet_minutes, landed_line_minutes) {
                (Some(a), Some(b)) => Some(a.min(*b)),
                (a, b) => a.or(*b),
            };
            let loaded = f64::from(loaded_minutes);
            let wait = match from_pet {
                Some(needs) => needs.min(loaded).max(0.0),
                None => loaded,
            };
            let next = wait + f64::from(wait_after_empty_minutes);
            let suffix = match from_pet {
                Some(minutes) if minutes < loaded => {
                    format!(" (the pet finishes first, in {minutes:.0} min)")
                }
                _ => String::new(),
            };
            FoldResult {
                failures: 0,
                next_minutes: Some(next),
                message: format!("{row} reloaded. Next {next_label}.{suffix}"),
                dropped_log: None,
            }
        }
        RowOutcome::Deferred => FoldResult {
            failures: 0,
            next_minutes: Some(RETRY_MINUTES),
            message: format!("{row} is waiting for the boarding window to open."),
            dropped_log: Some(format!(
                "  {row}: the boarding window would not open — no reload was attempted, so nothing is \
                 counted against the row; trying again in {RETRY_MINUTES:.0} min."
            )),
        },
        RowOutcome::NothingToBoard => FoldResult {
            failures: 0,
            next_minutes: Some(NO_PET_RETRY_MINUTES),
            message: format!("{row} is waiting for a pet that can be fed."),
            dropped_log: Some(format!(
                "  {row}: nothing in the bag can be boarded — the row keeps its place and is looked \
                 at again in {NO_PET_RETRY_MINUTES:.0} min. NOT a failure."
            )),
        },
        RowOutcome::Failed { error } => {
            let strikes = failures + 1;
            let message = format!("{row} failed {strikes}/{MAX_FAILURES}: {error}");
            if strikes >= MAX_FAILURES {
                // A row that keeps failing is DROPPED rather than taking the run with it: one bad
                // calibration should not stop the other three being fed. The reason comes with the
                // death, not only with the earlier strikes.
                FoldResult {
                    failures: strikes,
                    next_minutes: None,
                    message,
                    dropped_log: Some(format!(
                        "  {row} given up on after {MAX_FAILURES} failures — {error} — the other rows \
                         carry on"
                    )),
                }
            } else {
                FoldResult {
                    failures: strikes,
                    next_minutes: Some(RETRY_MINUTES),
                    message,
                    dropped_log: None,
                }
            }
        }
    }
}

// ── the card (PublishSchedule), and the FACTS the arbiter needs ──────────────────────────

/// One row as the pet half knows it when it publishes the card.
#[derive(Debug, Clone, PartialEq)]
pub struct PetRowView {
    pub name: String,
    /// Whether the row still has a next reload (false = given up on).
    pub has_next: bool,
    pub boarding_running: bool,
    /// Minutes from now, when there is a next.
    pub next_minutes: Option<f64>,
    /// What the row is carrying: species and/or the panel's reading.
    pub pet_detail: Option<String>,
    /// The GROWTH the game's own line says the pet is heading for (`→ +9`) — display only.
    pub growth_target: Option<i32>,
    /// The game's own rate line for this row, if it stated one.
    pub measured_rate: Option<i32>,
}

/// One line on the card.
#[derive(Debug, Clone, PartialEq)]
pub struct PetCardRow {
    pub name: String,
    pub status: String,
    pub detail: Option<String>,
    pub next_minutes: Option<f64>,
}

/// The ran's card: a line per row, and the summary the player reads at a glance.
///
/// THE STANDING LINE exists because a reload is hours apart: a card that only reported the last thing
/// that happened said nothing for most of a run. The MOMENT is carried rather than a countdown, so the
/// shell can recompute the wait every UI tick instead of showing a number that was true when written.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PetCard {
    pub rows: Vec<PetCardRow>,
    /// Rows with a boarding running, in schedule order.
    pub boarding: Vec<String>,
    pub next_row: Option<String>,
    pub next_minutes: Option<f64>,
    pub queued: usize,
}

impl PetCard {
    /// The summary line: `boarding row 1, row 2  ·  next row 3 at 19:40  ·  1 more queued`, or
    /// `no rows scheduled`.
    ///
    /// `next_label` is the caller's formatted `HH:mm` for the soonest row; ignored when there is none.
    pub fn schedule_line(&self, next_label: &str) -> String {
        let Some(next_row) = &self.next_row else {
            return "no rows scheduled".to_owned();
        };
        let boarding = if self.boarding.is_empty() {
            "nothing boarding".to_owned()
        } else {
            format!("boarding {}", self.boarding.join(", "))
        };
        format!(
            "{boarding}  ·  next {next_row} at {next_label}{}",
            if self.queued > 0 {
                format!("  ·  {} more queued", self.queued)
            } else {
                String::new()
            }
        )
    }

    /// The FACTS the `seal-tools` arbiter needs from this half — plain data, no rule:
    /// whether a row wants the game NOW, and when the next action is due if not.
    pub fn facts(&self) -> PetFacts {
        let wants_game_now = self.next_minutes.is_some_and(|minutes| minutes <= 0.0);
        PetFacts {
            wants_game_now,
            next_action_in_minutes: self.next_minutes,
            card: self.clone(),
        }
    }
}

/// What the pet half tells the arbiter. Deliberately facts, not permissions: WHEN the feeder may take
/// the game is the arbiter's rule, and it lives in `seal-tools` so there is one owner of it.
#[derive(Debug, Clone, PartialEq)]
pub struct PetFacts {
    pub wants_game_now: bool,
    pub next_action_in_minutes: Option<f64>,
    pub card: PetCard,
}

/// `PublishSchedule`: one line per row, then the summary.
///
/// ONE LINE PER ROW, always — the player asked to see all four rows' condition rather than only the
/// soonest: *"if I have 4 pets pending to feed … list out all 4 conditions"*. The detail is the last
/// feeder reading, shown as the reading it is: the tray drains between visits and nothing here is a
/// live count.
pub fn build_card(views: &[PetRowView]) -> PetCard {
    let mut rows = Vec::with_capacity(views.len());
    let mut boarding = Vec::new();
    let mut scheduled: Vec<(&str, f64)> = Vec::new();

    for view in views {
        let status = if !view.has_next {
            "given up"
        } else if view.boarding_running {
            "boarding"
        } else {
            "waiting for a reload"
        };

        // THE PET, and — when the game's own line names one — the growth it says the pet is heading
        // for, on the SAME line (`→ +9`, in the panel's own vocabulary; NOT a stage).
        let mut detail = view.pet_detail.clone().filter(|text| !text.is_empty());
        if let Some(growth) = view.growth_target {
            detail = Some(match detail {
                Some(text) => format!("{text}  ·  → +{growth}"),
                None => format!("→ +{growth}"),
            });
        }
        // The row's own feeding rate, when the window stated one — the visible half of the per-row
        // rate reading ("it is important as they may have different stage pet in the future").
        if let Some(rate) = view.measured_rate {
            detail = Some(match detail {
                Some(text) => format!("{text}  ·  {rate}/min (game line)"),
                None => format!("{rate}/min (game line)"),
            });
        }

        rows.push(PetCardRow {
            name: view.name.clone(),
            status: status.to_owned(),
            detail,
            next_minutes: view.next_minutes,
        });

        if view.boarding_running {
            boarding.push(view.name.clone());
        }
        if let Some(minutes) = view.next_minutes {
            scheduled.push((view.name.as_str(), minutes));
        }
    }

    let mut card = PetCard {
        rows,
        boarding,
        next_row: None,
        next_minutes: None,
        queued: 0,
    };

    if scheduled.is_empty() {
        return card;
    }
    let soonest = scheduled
        .iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("scheduled is non-empty");
    card.next_row = Some(soonest.0.to_owned());
    card.next_minutes = Some(soonest.1);
    card.queued = scheduled.len().saturating_sub(1);
    card
}

// ── the Start refusals (Ready) ─────────────────────────────────────────────────────────────

/// One breeding row as the setup check sees it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PetRowSetup {
    pub enabled: bool,
    pub toggle_label: Option<Vec<i32>>,
    pub feeder_strip: Option<Vec<i32>>,
    pub stacks: i32,
}

/// The whole pet setup a Start must validate. Field sources: the merged `pet` block plus the shared
/// buy/sell `max_button`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PetSetup {
    pub menu_button: Option<Vec<i32>>,
    pub feed_icon: Option<Vec<i32>>,
    pub close_button: Option<Vec<i32>>,
    pub bag_grid: Option<Vec<i32>>,
    pub bag_slot: Option<Vec<i32>>,
    pub page_tabs: Vec<Vec<i32>>,
    pub slots: Vec<PetRowSetup>,
    pub food_load_mode: String,
    pub return_slot: Option<Vec<i32>>,
    pub queue_len: usize,
    pub food_slots: Vec<Vec<i32>>,
    pub max_button: Option<Vec<i32>>,
    /// The shared buy/sell MAX, used when no pet-specific one is set — the same count dialog.
    pub buy_sell_max_button: Option<Vec<i32>>,
}

fn is_point(point: &Option<Vec<i32>>) -> bool {
    point.as_ref().is_some_and(|values| values.len() == 2)
}

fn valid_rect(rect: &Option<Vec<i32>>) -> bool {
    rect.as_ref()
        .is_some_and(|values| values.len() == 4 && values[2] > 0 && values[3] > 0)
}

/// `Ready`: why a run can't start, or `Ok`. Checked before anything moves, so a half-calibrated setup
/// says what is missing instead of clicking into empty screen.
pub fn ready(setup: &PetSetup) -> Result<(), StopReason> {
    let refusal = |message: &str| Err(StopReason::error(message));

    if !is_point(&setup.menu_button) {
        return refusal(
            "The pet menu button isn't calibrated — save the 目錄 (menu) point on the Calibrate tab.",
        );
    }
    if !is_point(&setup.feed_icon) {
        return refusal(
            "The pet feed icon isn't calibrated — save the Pet feed icon point on the Calibrate tab.",
        );
    }
    if !is_point(&setup.close_button) {
        return refusal(
            "The boarding window's X isn't calibrated — save the Boarding X point on the Calibrate tab.",
        );
    }
    if !valid_rect(&setup.bag_grid) || !valid_rect(&setup.bag_slot) {
        return refusal(
            "The boarding bag's grid isn't calibrated — save the Boarding bag grid and Boarding bag slot on the Calibrate tab. (Its own, not the shop's.)",
        );
    }
    if setup.page_tabs.is_empty() {
        return refusal(
            "The bag page tabs aren't calibrated — save the Bag page tab points on the Calibrate tab.",
        );
    }
    if setup.slots.is_empty() {
        return refusal(
            "No breeding row is set up — rows come from your migrated v2 config and adding a row is not built yet. Each row needs its start/end button and its pet slot marked on the Calibrate tab.",
        );
    }
    if !setup.slots.iter().any(|row| row.enabled) {
        return refusal("Every row is unticked — nothing to feed. Tick the row on the Pet tab.");
    }

    // EVERY row the tool DRIVES, not just the first. Only the ticked ones: a row that is switched off
    // is not driven, so demanding it be fully calibrated is what used to make a half-set-up row block
    // a run that never touches it.
    for (index, row) in setup.slots.iter().enumerate() {
        if !row.enabled {
            continue;
        }
        if !valid_rect(&row.toggle_label) {
            return Err(StopReason::error(format!(
                "Row {}'s start/end button isn't marked — save the Row start/end button for row {} on the Calibrate tab. The tool has nothing to click for that row. Untick it on the Pet tab to leave it alone.",
                index + 1,
                index + 1
            )));
        }
    }

    // A DRAG has to name the box it drops into, and the food strip is what says where the boxes are.
    // Right-click does not need it — it lets the game choose, which is the entire difference between
    // the two modes.
    if is_drag(&setup.food_load_mode) {
        for (index, row) in setup.slots.iter().enumerate() {
            if !row.enabled {
                continue;
            }
            if FeederLayout::slot_boxes(row.feeder_strip.as_deref(), row.stacks).is_none() {
                return Err(StopReason::error(format!(
                    "Row {}'s food strip isn't drawn — save the Row food strip for row {} on the Calibrate tab. The food load is set to drag, which has to name the box it drops into, and the strip is what says where the boxes are.",
                    index + 1,
                    index + 1
                )));
            }
        }
    }

    // Every marked cell carries a PAGE, so a tab that was never calibrated is a mark pointing at a
    // page the tool cannot reach. Checked here rather than discovered mid-flow: the reload closes the
    // window on any failure, so a bad page surfaced as "it opened and then shut again".
    let mut pages: Vec<(&str, i32)> = Vec::new();
    if let Some(return_slot) = &setup.return_slot
        && return_slot.len() == 2
    {
        pages.push(("The return slot", return_slot[0]));
    }
    for cell in &setup.food_slots {
        if cell.len() == 2 {
            pages.push(("A food cell", cell[0]));
        }
    }

    for (what, page) in pages {
        if page < 0 || page as usize >= setup.page_tabs.len() {
            return Err(StopReason::error(format!(
                "{what} is marked on bag page {}, but only {} page tab(s) are calibrated — save the Bag page tab point on the Calibrate tab, or change it on the Pet tab.",
                page + 1,
                setup.page_tabs.len()
            )));
        }
        if setup.page_tabs[page as usize].len() != 2 {
            return Err(StopReason::error(format!(
                "{what} is marked on bag page {}, and that page's tab isn't calibrated — save the Bag page tab point ITEM{} on the Calibrate tab.",
                page + 1,
                page + 1
            )));
        }
    }

    // The fixed cell is only required while there is no queue to find the pet by.
    if !(setup
        .return_slot
        .as_ref()
        .is_some_and(|slot| slot.len() == 2))
        && setup.queue_len == 0
    {
        return refusal(
            "Neither a return slot nor any queued pet icon is set — the tool has nothing to find the pet by. Set the return slot on the Pet tab, or add a pet icon to pet.queue in config/local.yaml (adding queue entries is not built yet).",
        );
    }
    if setup.food_slots.is_empty() {
        return refusal(
            "No food cells are marked — mark the food cells on the Pet tab. Nothing says where the pet food lives.",
        );
    }
    if max_button(setup).is_none() {
        return refusal("No MAX is calibrated — save the MAX button on the Calibrate tab.");
    }
    Ok(())
}
fn is_drag(food_load_mode: &str) -> bool {
    food_load_mode.eq_ignore_ascii_case("drag")
}

fn max_button(setup: &PetSetup) -> Option<&Vec<i32>> {
    setup
        .max_button
        .as_ref()
        .or(setup.buy_sell_max_button.as_ref())
}

// ── the messages the loop shows (player-visible, ported verbatim) ───────────────────────────

/// The visit's opening line: one row names itself, a batch says how many.
pub fn visit_opening_message(batch: &[String]) -> String {
    if batch.len() > 1 {
        format!("Opening the breeder for {} rows…", batch.len())
    } else {
        format!(
            "{}: opening the boarding window…",
            batch.first().map(String::as_str).unwrap_or("")
        )
    }
}

/// The step a reload is on — each names itself on the card as it runs, because without that a failure
/// reads as "it opened the window and then closed it again".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadStep {
    Reloading,
    EndingBoarding,
    PlacingPet,
    LoadingFood,
    StartingBoarding,
}

impl ReloadStep {
    pub fn text(self) -> &'static str {
        match self {
            Self::Reloading => "reloading…",
            Self::EndingBoarding => "ending boarding…",
            Self::PlacingPet => "placing the pet…",
            Self::LoadingFood => "loading the food…",
            Self::StartingBoarding => "starting boarding…",
        }
    }
}

pub fn reload_step_message(row: &str, step: ReloadStep) -> String {
    format!("{row}: {}", step.text())
}

/// The end toggle press did NOT bring the pet back, so it did the opposite of what was asked. The one
/// failure that used to log nothing — and the most common in `pet.log`.
pub fn end_toggle_failed(row: &str) -> String {
    format!(
        "{row}: the pet did NOT come back out of the loader after pressing the end button — so that \
         press did the opposite of what was asked and a boarding may now be running with nothing in \
         it. Stopping rather than clicking on: check the row in the game, and re-capture its \
         empty-slot reference in config/local.yaml (the crops come from your v2 config; re-capturing them is not built yet) if the slot is empty."
    )
}

/// The right-click placement never took: the slot still looks empty after the attempts.
pub fn pet_did_not_go_in() -> String {
    format!(
        "The pet did not go in after {PLACE_ATTEMPTS} right-clicks — the slot still looks empty. Nothing was loaded. Check the return slot on the Pet tab."
    )
}
/// Every candidate was found and REFUSED (+9/100 %), which is a different fix from "nothing found".
pub fn all_refused_message(refused: &[String]) -> String {
    format!(
        "Every pet that could be boarded reads +9/100%. Refused {}. Capture the icon of a pet that can still be fed and add it to pet.queue in config/local.yaml (adding queue entries is not built yet).",
        refused.join("; ")
    )
}
/// Nothing usable in the bag: the return slot is empty and no queued icon matched.
pub fn nothing_matched_message() -> String {
    "The return slot holds no pet, and no queued icon matched one in the bag. Capture the icon of a pet that can still be fed and add it to pet.queue in config/local.yaml (adding queue entries is not built yet)."
        .to_owned()
}
/// No way to find a pet at all — no queued icon and no return slot marked.
pub fn no_way_to_find_pet_message() -> String {
    "Neither a queued pet icon nor a return slot is set — there is nothing to right-click. Set the return slot on the Pet tab, or add a pet icon to pet.queue in config/local.yaml (adding queue entries is not built yet)."
        .to_owned()
}
/// The bag held no feedable pet, with the sweep's reason.
pub fn no_pet_in_bag_message(why: &str) -> String {
    format!(
        "No queued pet is in the bag ({why}). Right-clicked nothing. If the pet finished it was mailed, so this is the queue being empty of live pets — add the next one's icon to pet.queue in config/local.yaml (adding queue entries is not built yet)."
    )
}
pub fn bag_grid_missing_message() -> String {
    "The bag grid isn't calibrated — save the Boarding bag grid on the Calibrate tab; there is \
     nothing to right-click."
        .to_owned()
}
/// Every row was dropped after its strikes: the run has nothing left to feed.
pub fn every_row_failed_message() -> String {
    "Every row has failed repeatedly — stopped.".to_owned()
}

pub fn stopped_before_reading_message() -> String {
    "Stopped before it could read the rows.".to_owned()
}

pub fn checking_rows_message() -> String {
    "Checking what is already running…".to_owned()
}

/// The game window left the front MID-READ: five minutes and a fresh look, not the configured cycle —
/// the game is there and a row may be dry NOW.
pub fn window_left_front_message() -> String {
    format!(
        "The game window left the front while reading the rows — trying again in {RETRY_MINUTES:.0} min."
    )
}

/// The game window was not in front BEFORE the visit: no click is sent, no strike counted, the rows
/// keep their place. (Whether the arbiter may then take the game IS the arbiter's rule; this is only
/// the line the card shows while the caller defers.)
pub fn window_not_in_front_message(row: &str) -> String {
    format!(
        "The game window isn't in front — {row} keeps its place; nothing was clicked, and it is tried \
         again in {RETRY_MINUTES:.0} min."
    )
}

/// The note left on the card when a stop lands INSIDE a visit: the reload was cut and this process
/// cannot put the game back. The same words for the loop's visits and for the look's hand-off.
pub fn stopped_during(rows: &[String]) -> String {
    if rows.len() == 1 {
        format!(
            "Stopped during {} — it may not have been fed. Check it in game.",
            rows[0]
        )
    } else {
        format!(
            "Stopped during {} rows ({}) — some may not have been fed. Check them in game.",
            rows.len(),
            rows.join(", ")
        )
    }
}

// ── facts for the arbiter, from the schedule only (no rules) ──────────────────────────────

/// The next action the pet half knows about, in minutes from now, from a set of per-row next times.
/// `None` means no row is scheduled (all given up). The arbiter uses this to decide WHEN it may take
/// the game; the feeder itself never waits on the game window beyond the loop's own deferral.
pub fn next_action_minutes(next: &[(String, f64)]) -> Option<(String, f64)> {
    next.iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(row, minutes)| (row.clone(), *minutes))
}

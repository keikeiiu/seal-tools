//! The pet feeder — the LAST live tool: the resident feeder's plan, card surface, read seam and loop.
//!
//! The decision content is `seal-domain`'s ([`PetSetup::ready`], [`schedule_for`], [`consider_veto`],
//! [`fold_back`], [`build_card`], [`visit_opening_message`], [`reload_step_message`],
//! [`food_load::complaint`] …); nothing here re-implements one. The I/O is [`PetSource`], scripted in
//! every test: no window, no board, no OCR models. The production [`PetReadSource`] is
//! `win::capture` + `seal-ocr` + `seal_win` over the regions the domain specifies
//! ([`FeederLayout`]'s eta/slot/read regions, the tooltip box from config).
//!
//! The loop is v2's shape: opening LOOK (read every row; that reading is the schedule) → due rows
//! reloaded in the same window → close → wait for the soonest next (a long `Decision::Wait`, which
//! the runner cancels DURING) → repeat. A timer-due row is re-read and the domain's positive-only
//! veto can cancel its reload. One row's reload is end-toggle → verify empty → place pet (icon sweep
//! or the return slot, hover panel checked for a finished pet) → load food per stack (right-click +
//! MAX + Enter, or the firmware-2 drag with an explicit HoldLeft/ReleaseLeft pair) → start boarding →
//! verify → fold the outcome back. Failures fold into strikes and a row is dropped after
//! `MAX_FAILURES`; every dropped row ends the run with the domain's line.
//!
//! # The game hand-over (wired)
//!
//! The feeder is resident (`ToolId::is_resident`): the arbiter arms it without the game. This loop
//! asks for the game through [`seal_tools::game::GamePort`] BEFORE its first decision that can click
//! (the opening look included — it must not steal focus behind a foreground run), and gives it back
//! after the window closes and before the long wait, so a foreground start that was deferred behind
//! the feeder actually starts (`GameRelease::StartPending`). Every exit path releases: a terminal
//! decision releases, and the tool's `Drop` guard releases a claim interrupted mid-visit. A blocked
//! request produces NO decisions — the worker waits inside `request` while the supervisor stashes the
//! reply. Foreground tools do not ask: `start` claims the game for them.
//!
//! # Deviations from v2, recorded
//!
//! - The feeder-count read is ONE OCR pass per slot plus the warm-colour verdict; v2's 3×/9×/24×
//!   upscale ladder is a recorded follow-up (the `Option<i32>` seam does not change).
//! - The feeding table is `<root>/docs/pet-data.csv`; the release zip ships that file, so the
//!   computed pet-minutes are available in an installed payload. If the file is absent (an older
//!   or manually extracted install), the schedule still comes from the time line / counts /
//!   configured cycle, exactly v2's fallback; a missing table is not a code I/O failure.
//! - There is no close-button "is the boarding window still open" probe; the visit always opens and
//!   closes with the configured marks.
//! - `ToolStatus` has no schedule field: the message is the last action and the schedule line lives in
//!   the rows' `next_action_in_seconds` (the shell already counts down). v2's separate
//!   `state.Schedule` line has no protocol slot.
//! - A candidate pet is hover-checked before the right-click; the drag uses closed-loop `Place` for
//!   the drop point (v2's `PlaceOn`) with an explicit hold/release pair; the effects' Drop guard is
//!   the backstop for a failure or cancel between them.
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use seal_domain::config::AttributesConfig;
use seal_domain::decision::{Decision, StopReason};
use seal_domain::geometry::{Point, Rect};
use seal_domain::ocr::PetPanel;
use seal_domain::tools::describe;
use seal_domain::tools::pet::{
    FeederCount, FeederEta, FeederLayout, FeedingLine, MATCH_LIMIT, PET_SLOT_NOT_THE_WINDOW_ABOVE,
    PLACE_ATTEMPTS, PetCard, PetFeeding, PetRowSetup, PetRowView, PetScheduleSettings, PetSetup,
    ReloadStep, RowOutcome, ScheduleRead, build_card, consider_veto, every_row_failed_message,
    fold_back, food_load, pet_did_not_go_in, rate_for, reload_step_message, schedule_for,
    visit_opening_message,
};
use seal_domain::tools::shop::BagGrid;
use seal_tools::game::GamePort;
use seal_tools::runner::{Tool, ToolStatus};
use seal_tools::tool::{ToolId, ToolRow};
use serde_json::Value;

/// v2 `IconMatch.DefaultInset` (`v2/SealTools.Core/IconMatch.cs:24`): the border inset of the
/// stored-crop-vs-live-region comparison. Six pixels is what the measured 7.7 % empty-vs-itself error
/// bought.
const ICON_INSET: usize = 6;

/// v2 `IconMatch.DefaultTolerance` (`IconMatch.cs:28`): a per-channel difference at or below this is
/// the same pixel — the game re-renders its icons, so an identical item is not bit-identical.
const ICON_TOLERANCE: i32 = 24;

/// v2 `FeederCount.WarmFraction`'s `redOverBlue = 40` (`v2/SealTools.Core/FeederCount.cs`).
const WARM_RED_OVER_BLUE: i32 = 40;

/// v2 `FeederCount.WarmFraction`'s `inset = 4`: the cell's border is in the same place in every slot
/// and says nothing about the contents.
const WARM_INSET: usize = 4;

/// A decoded reference crop (base64 PNG from `local.yaml`): RGBA8, kept as bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceCrop {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// One breeding row as the run plans it: the domain's setup fields plus the reader's own data (the
/// boarding slot box, the empty-slot reference, the configured session state).
#[derive(Debug, Clone, PartialEq)]
pub struct PetRowPlan {
    /// `Row 1` … (v2 `PetTool.NameOf`, 1-based, matching the window).
    pub name: String,
    pub enabled: bool,
    pub toggle_label: Option<Vec<i32>>,
    pub boarding_pet_slot: Option<Rect>,
    pub feeder_strip: Option<Vec<i32>>,
    pub stacks: i32,
    /// The row's slot as it looks EMPTY. `None` when the player has not captured one — the slot read
    /// then answers "unknown", never "empty".
    pub empty_reference: Option<ReferenceCrop>,
    /// The configured/ticked session state (v2's `PetSlotConfig.BoardingRunning`).
    pub boarding_running: bool,
}

/// One queued pet, as configured. The icon is decoded in increment 2, where the icon matcher lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetQueuePlan {
    pub label: String,
    pub species: Option<String>,
    pub png: Option<String>,
}

/// The whole resolved start: the domain setup the decision half consumes, and the reader-side data
/// the source needs.
#[derive(Debug, Clone, PartialEq)]
pub struct PetPlan {
    pub setup: PetSetup,
    pub rows: Vec<PetRowPlan>,
    pub queue: Vec<PetQueuePlan>,
    /// Decoded queue icons (increment 2's bag sweep); `None` when that entry has no usable icon.
    pub queue_icons: Vec<Option<PetImage>>,
    /// Decoded food icon for the bag-cell re-marking; `None` when the player has not captured one.
    pub food_icon: Option<PetImage>,
    /// The feeding table (`<root>/docs/pet-data.csv`), empty when the file is absent (the schedule then
    /// falls back to the configured cycle, v2's rule).
    pub feeding: Vec<FeedingLine>,
    pub food_slots: Vec<Vec<i32>>,
    pub food_slots_used: i32,
    pub food_icon_png: Option<String>,
    pub close_button_png: Option<String>,
    pub boarding_window_may_be_open: bool,
    pub items_per_minute: i32,
    pub wait_after_empty_minutes: i32,
    pub action_wait_ms: i32,
    pub reload_on_start: bool,
    pub pet_slot_occupied_above: f64,
    pub feeder_count_left_fraction: f64,
    pub feeder_count_min_score: f64,
    pub tooltip: seal_domain::config::TooltipConfig,
    pub root: PathBuf,
}

impl PetPlan {
    /// The rows the tool drives (`ActiveRows`): a row with its tick off is invisible to every
    /// consumer, which is the domain's documented rule.
    pub fn active_rows(&self) -> impl Iterator<Item = &PetRowPlan> {
        self.rows.iter().filter(|row| row.enabled)
    }
}

fn number(value: &Value, path: &str) -> Option<i32> {
    value
        .pointer(path)
        .and_then(Value::as_i64)
        .and_then(|number| i32::try_from(number).ok())
}

fn float(value: &Value, path: &str) -> Option<f64> {
    value.pointer(path).and_then(Value::as_f64)
}

fn boolean(value: &Value, path: &str) -> Option<bool> {
    value.pointer(path).and_then(Value::as_bool)
}

fn text(value: &Value, path: &str) -> Option<String> {
    value
        .pointer(path)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn int_array(value: &Value, path: &str) -> Option<Vec<i32>> {
    let array = value.pointer(path)?.as_array()?;
    Some(
        array
            .iter()
            .filter_map(Value::as_i64)
            .filter_map(|number| i32::try_from(number).ok())
            .collect(),
    )
}

fn int_matrix(value: &Value, path: &str) -> Vec<Vec<i32>> {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.as_array()
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(Value::as_i64)
                                .filter_map(|number| i32::try_from(number).ok())
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}

fn point(value: &Value, path: &str) -> Option<Vec<i32>> {
    int_array(value, path).filter(|values| values.len() == 2)
}

fn rect_of(values: &[i32]) -> Option<Rect> {
    (values.len() == 4 && values[2] > 0 && values[3] > 0)
        .then(|| Rect::new(values[0], values[1], values[2], values[3]))
}

/// `pet.slots` entry → the domain's row setup.
fn row_setup(value: &Value, index: usize) -> PetRowSetup {
    let at = |field: &str| format!("/pet/slots/{index}/{field}");
    PetRowSetup {
        // Missing means ON (v2 `PetSlotConfig.Enabled`), so a file written before the field existed
        // keeps driving its rows.
        enabled: boolean(value, &at("enabled")).unwrap_or(true),
        toggle_label: int_array(value, &at("toggle_label")),
        feeder_strip: int_array(value, &at("feeder_strip")),
        stacks: number(value, &at("stacks")).unwrap_or(2),
    }
}

/// Build the domain's `PetSetup` from the merged config. Field sources are the machine `pet` block
/// plus the shared buy/sell `max_button` (`PetSetup`'s own doc).
fn setup(config: &Value) -> PetSetup {
    let slots = config
        .pointer("/pet/slots")
        .and_then(Value::as_array)
        .map(|slots| {
            slots
                .iter()
                .enumerate()
                .map(|(index, _)| row_setup(config, index))
                .collect()
        })
        .unwrap_or_default();
    PetSetup {
        menu_button: point(config, "/pet/menu_button"),
        feed_icon: point(config, "/pet/feed_icon"),
        close_button: point(config, "/pet/close_button"),
        bag_grid: int_array(config, "/pet/bag_grid"),
        bag_slot: int_array(config, "/pet/bag_slot"),
        page_tabs: int_matrix(config, "/pet/page_tabs"),
        slots,
        food_load_mode: text(config, "/pet/food_load_mode")
            .unwrap_or_else(|| food_load::DEFAULT.to_owned()),
        return_slot: int_array(config, "/pet/return_slot"),
        queue_len: config
            .pointer("/pet/queue")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        food_slots: int_matrix(config, "/pet/food_slots"),
        max_button: point(config, "/pet/max_button"),
        buy_sell_max_button: point(config, "/buy_sell/max_button"),
    }
}

/// Decode one base64 PNG reference crop. A data-URI prefix is tolerated because a calibration written
/// from a browser-based tool carries one; anything else is the named refusal.
fn decode_crop(png: &str, what: &str) -> Result<ReferenceCrop, String> {
    let payload = png.rsplit(',').next().unwrap_or(png).trim();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| format!("{what} couldn't be decoded — re-capture it and set the new crop in config/local.yaml (the crops come from your v2 config; re-capturing them is not built yet)."))?;
    let (width, height, rgba) = seal_win::pure::decode_png(&bytes)
        .map_err(|_| format!("{what} couldn't be decoded — re-capture it and set the new crop in config/local.yaml (the crops come from your v2 config; re-capturing them is not built yet)."))?;
    Ok(ReferenceCrop {
        width: width as usize,
        height: height as usize,
        rgba,
    })
}

/// Decode one reference icon (queue entry / food icon) to RGBA8. Same refusal shape as the row
/// crops: a corrupt icon is a loud failure naming the item, never a silently skipped pet.
fn decode_icon(png: &str, what: &str) -> Result<PetImage, String> {
    let payload = png.rsplit(',').next().unwrap_or(png).trim();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| format!("{what} couldn't be decoded — re-capture it and set the new crop in config/local.yaml (the crops come from your v2 config; re-capturing them is not built yet)."))?;
    let (width, height, rgba) = seal_win::pure::decode_png(&bytes)
        .map_err(|_| format!("{what} couldn't be decoded — re-capture it and set the new crop in config/local.yaml (the crops come from your v2 config; re-capturing them is not built yet)."))?;
    Ok(PetImage {
        width: width as usize,
        height: height as usize,
        rgba,
    })
}

fn queue_icons(config: &Value) -> Result<Vec<Option<PetImage>>, String> {
    config
        .pointer("/pet/queue")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .enumerate()
                .map(
                    |(index, entry)| match entry.get("png").and_then(Value::as_str) {
                        Some(png) if !png.trim().is_empty() => Ok(Some(decode_icon(
                            png,
                            &format!("Queue pet {}'s icon", index + 1),
                        )?)),
                        _ => Ok(None),
                    },
                )
                .collect()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

/// The feeding table, loaded once per plan. A missing/unreadable table costs the COMPUTED schedule
/// only (v2 `PetFeeding.Load`'s rule): every pet still boards and is fed on the configured cycle.
fn feeding_table(root: &Path) -> Vec<FeedingLine> {
    let path = root.join("docs").join("pet-data.csv");
    std::fs::read_to_string(path)
        .map(|text| PetFeeding::parse(&text))
        .unwrap_or_default()
}
fn row_plan(config: &Value, index: usize, setup: &PetRowSetup) -> Result<PetRowPlan, String> {
    let at = |field: &str| format!("/pet/slots/{index}/{field}");
    let empty_reference = match text(config, &at("pet_slot_empty_png")) {
        Some(png) if !png.trim().is_empty() => Some(decode_crop(
            &png,
            &format!("Row {}'s empty-slot reference", index + 1),
        )?),
        _ => None,
    };
    Ok(PetRowPlan {
        name: format!("Row {}", index + 1),
        enabled: setup.enabled,
        toggle_label: setup.toggle_label.clone(),
        boarding_pet_slot: int_array(config, &at("boarding_pet_slot"))
            .as_deref()
            .and_then(rect_of),
        feeder_strip: setup.feeder_strip.clone(),
        stacks: setup.stacks,
        empty_reference,
        boarding_running: boolean(config, &at("boarding_running")).unwrap_or(false),
    })
}

fn queue(config: &Value) -> Vec<PetQueuePlan> {
    config
        .pointer("/pet/queue")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .map(|entry| PetQueuePlan {
                    label: entry
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    species: entry
                        .get("species")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    png: entry.get("png").and_then(Value::as_str).map(str::to_owned),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Resolve and VALIDATE the pet plan from the merged config.
///
/// The required-marks check is the domain's [`ready`](seal_domain::tools::pet::ready): every refusal
/// is the player line v2 already had. Only two things are added here, both reader-side: a crop that
/// is present but cannot be decoded is a start refusal naming the row, and `pet.food_load_mode` is
/// resolved for the board check ([`check_board`], which the opening phase calls with the firmware
/// level it read).
pub fn plan(config: &Value, root: &Path) -> Result<PetPlan, String> {
    let setup = setup(config);
    seal_domain::tools::pet::ready(&setup).map_err(|reason| reason.message)?;

    let rows = setup
        .slots
        .iter()
        .enumerate()
        .map(|(index, row)| row_plan(config, index, row))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PetPlan {
        setup,
        rows,
        queue: queue(config),
        queue_icons: queue_icons(config)?,
        food_icon: match text(config, "/pet/food_icon_png") {
            Some(png) if !png.trim().is_empty() => Some(decode_icon(&png, "The food icon")?),
            _ => None,
        },
        feeding: feeding_table(root),
        food_slots: int_matrix(config, "/pet/food_slots"),
        food_slots_used: number(config, "/pet/food_slots_used").unwrap_or(0),
        food_icon_png: text(config, "/pet/food_icon_png"),
        close_button_png: text(config, "/pet/close_button_png"),
        boarding_window_may_be_open: boolean(config, "/pet/boarding_window_may_be_open")
            .unwrap_or(false),
        items_per_minute: number(config, "/pet/items_per_minute").unwrap_or(3),
        wait_after_empty_minutes: number(config, "/pet/wait_after_empty_minutes").unwrap_or(5),
        action_wait_ms: number(config, "/pet/action_wait_ms").unwrap_or(1200),
        reload_on_start: boolean(config, "/pet/reload_on_start").unwrap_or(false),
        pet_slot_occupied_above: float(config, "/pet/pet_slot_occupied_above").unwrap_or(0.05),
        feeder_count_left_fraction: float(config, "/pet/feeder_count_left_fraction")
            .unwrap_or(FeederLayout::READ_LEFT_FRACTION),
        feeder_count_min_score: float(config, "/pet/feeder_count_min_score")
            .unwrap_or(FeederCount::MIN_SCORE),
        tooltip: config
            .pointer("/tooltip")
            .cloned()
            .map(|value| serde_json::from_value(value).unwrap_or_default())
            .unwrap_or_default(),
        root: root.to_path_buf(),
    })
}

/// The board/food-load compatibility check the opening phase runs. A drag needs firmware 2; a board
/// that never answered counts as a DEFINITE no (the sketch that predates version reporting also
/// predates the drag commands), and the sentence is the domain's [`food_load::complaint`].
pub fn check_board(plan: &PetPlan, firmware_level: Option<i32>) -> Result<(), String> {
    match food_load::complaint(Some(plan.setup.food_load_mode.as_str()), firmware_level) {
        Some(line) => Err(line),
        None => Ok(()),
    }
}

// ── the card surface (the domain's, mapped to what the runner publishes) ─────────────────────

/// The opening card: one line per configured row, so the player sees all four conditions rather than
/// only the soonest (`build_card`'s own rule) before the run has read anything.
pub fn initial_card(plan: &PetPlan) -> PetCard {
    let views = plan
        .rows
        .iter()
        .map(|row| PetRowView {
            name: row.name.clone(),
            has_next: true,
            boarding_running: row.boarding_running,
            next_minutes: None,
            pet_detail: None,
            growth_target: None,
            measured_rate: None,
        })
        .collect::<Vec<_>>();
    build_card(&views)
}

/// `PetCard` → the runner's standing rows (`ToolStatus.rows`). The shell already renders
/// `ToolRow { name, status, detail, next_action_in_seconds }`.
pub fn tool_rows(card: &PetCard) -> Vec<ToolRow> {
    card.rows
        .iter()
        .map(|row| {
            let mut tool_row = ToolRow::new(row.name.clone(), row.status.clone());
            tool_row.detail = row.detail.clone();
            tool_row.next_action_in_seconds = row.next_minutes.map(|minutes| minutes * 60.0);
            tool_row
        })
        .collect()
}

/// The card as a `ToolStatus`: the schedule line as the message (the caller supplies the formatted
/// `HH:mm` because the decision half has no clock) plus the standing rows.
pub fn status(card: &PetCard, next_label: &str) -> seal_tools::runner::ToolStatus {
    seal_tools::runner::ToolStatus {
        message: card.schedule_line(next_label),
        countdown_s: None,
        rows: tool_rows(card),
    }
}

/// The visit's opening line, for the loop and the tests: one row names itself, a batch says how many.
pub fn opening_message(batch: &[String]) -> String {
    visit_opening_message(batch)
}

/// The tally wording shared with the tuner/gem card (`GradeCycles::describe`'s sibling): kept here so
/// a card line and a log line cannot drift.
pub fn describe_cycles(cycles: i32) -> String {
    describe(cycles)
}

// ── the read seam ──────────────────────────────────────────────────────────────────────────

/// A decoded screen region, RGBA8. The loop never needs the pixels except through the comparisons
/// below, which are pure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl PetImage {
    pub fn pixel(&self, x: usize, y: usize) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y * self.width + x) * 4;
        self.rgba
            .get(index..index + 4)
            .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
    }
}

/// What one row's feeder slots said. `counts` has one entry per slot box (`None` = unreadable); the
/// domain's rule is that a total is only meaningful when at least one slot read, and zero is a real
/// reading ("reload now") rather than a failure.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FeederCounts {
    pub counts: Vec<Option<i32>>,
    /// True when at least one count came from DIGITS (v2's proof that the pixels are the boarding
    /// window speaking, not a colour verdict).
    pub digits_read: bool,
}

impl FeederCounts {
    /// The domain's sum: `None` when no slot read, otherwise the sum of the readable slots.
    pub fn total(&self) -> Option<i32> {
        let readable: Vec<i32> = self.counts.iter().flatten().copied().collect();
        (!readable.is_empty()).then(|| readable.iter().sum())
    }
}

/// What a row's boarding slot shows: the raw differing fraction (logged by the loop) and the
/// three-way answer (empty / occupied / cannot tell — the domain's own `PetSlotNotTheWindowAbove`
/// split).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SlotState {
    pub difference: Option<f64>,
    pub empty: Option<bool>,
}

/// Everything the loop reads from the game, as one injectable seam. The production implementation is
/// [`PetReadSource`]; the tests script it.
pub trait PetSource {
    /// One OCR pass over a row's time band: the raw lines (the domain's [`FeederEta::parse`] and
    /// [`FeederEta::parse_rate`] consume them). Empty when the row has no strip.
    fn row_lines(&mut self, row: &PetRowPlan) -> Result<Vec<String>, String>;
    /// The counts drawn on a row's food slots.
    fn feeder_counts(&mut self, row: &PetRowPlan) -> Result<FeederCounts, String>;
    /// The hover panel's OCR lines for a point (client-relative physical). Empty when the tooltip is
    /// not calibrated — the caller fails open on an unparsed panel.
    fn hover_panel(&mut self, at: Point) -> Result<Vec<String>, String>;
    /// The row's boarding slot versus its saved empty crop.
    fn slot_state(&mut self, row: &PetRowPlan) -> Result<SlotState, String>;
    /// The whole client, for increment 2's bag-page icon sweep.
    fn client_capture(&mut self) -> Result<PetImage, String>;
    /// One client-relative region, for increment 2's bag-cell checks.
    fn region_capture(&mut self, region: Rect) -> Result<PetImage, String>;
    /// The cursor in the process's own logical space (`None` when unreadable).
    fn cursor(&mut self) -> Option<Point>;
    /// Whether the game is the foreground window — the gate every screen read consults (a capture of
    /// whatever is in front produces confident readings of the wrong thing).
    fn game_in_front(&mut self) -> bool;
}

/// The production reader: capture through `win::capture` (foreground-gated), OCR through
/// `seal-ocr`'s pipeline, comparisons against the decoded reference crops.
pub struct PetReadSource {
    title: String,
    tooltip: seal_domain::config::TooltipConfig,
    feeder_count_left_fraction: f64,
    feeder_count_min_score: f64,
    pet_slot_occupied_above: f64,
    pipeline: seal_ocr::pipeline::OcrPipeline,
}

impl PetReadSource {
    /// Open the read side, or name what is missing before the run starts: a level-1 board with drag
    /// configured (the domain's complaint), a missing `window.title`, or unavailable OCR models.
    pub fn open(
        config: &Value,
        plan: &PetPlan,
        attributes: &AttributesConfig,
        firmware_level: Option<i32>,
        root: &std::path::Path,
    ) -> Result<Self, String> {
        check_board(plan, firmware_level)?;
        let title = crate::win::configured_title(config).map_err(|error| error.line())?;
        let geometry = crate::dispatch::ocr_geometry(config);
        let pipeline =
            seal_ocr::pipeline::OcrPipeline::load_for_root(root, attributes, geometry.row_height)
                .map_err(|error| format!("the OCR models are not available: {}", error.message))?;
        Ok(Self {
            title,
            tooltip: plan.tooltip,
            feeder_count_left_fraction: plan.feeder_count_left_fraction,
            feeder_count_min_score: plan.feeder_count_min_score,
            pet_slot_occupied_above: plan.pet_slot_occupied_above,
            pipeline,
        })
    }

    fn capture(&mut self, region: Option<Rect>) -> Result<PetImage, String> {
        let (frame, _display) =
            crate::win::capture(&self.title, region).map_err(|error| error.line())?;
        let (width, height, rgba) = crate::dispatch::frame_rgba(&frame)?;
        Ok(PetImage {
            width,
            height,
            rgba,
        })
    }

    /// One capture plus one OCR pass, for the reads that need lines. The count read uses the same
    /// shape so the warm-colour verdict sees exactly the pixels the OCR saw.
    fn capture_lines(&mut self, region: Rect) -> Result<(PetImage, Vec<(String, f64)>), String> {
        let image = self.capture(Some(region))?;
        let scan = self
            .pipeline
            .scan_rgba(image.width, image.height, &image.rgba)
            .map_err(|error| error.message)?;
        let lines = scan
            .items
            .iter()
            .map(|item| (item.text.clone(), f64::from(item.confidence)))
            .collect();
        Ok((image, lines))
    }
}

impl PetSource for PetReadSource {
    fn row_lines(&mut self, row: &PetRowPlan) -> Result<Vec<String>, String> {
        let Some(region) = FeederLayout::eta_region(row.feeder_strip.as_deref())
            .as_deref()
            .and_then(rect_of)
        else {
            return Ok(Vec::new());
        };
        let (_image, lines) = self.capture_lines(region)?;
        Ok(lines.into_iter().map(|(text, _)| text).collect())
    }

    fn feeder_counts(&mut self, row: &PetRowPlan) -> Result<FeederCounts, String> {
        let Some(slots) = FeederLayout::slot_boxes(row.feeder_strip.as_deref(), row.stacks) else {
            return Ok(FeederCounts::default());
        };
        let mut counts = Vec::with_capacity(slots.len());
        let mut digits_read = false;
        for slot in slots {
            let Some(region) =
                FeederLayout::read_region(Some(&slot), self.feeder_count_left_fraction)
                    .as_deref()
                    .and_then(rect_of)
            else {
                counts.push(None);
                continue;
            };
            let (image, lines) = self.capture_lines(region)?;
            match FeederCount::parse(&lines, self.feeder_count_min_score) {
                Some(count) => {
                    digits_read = true;
                    counts.push(Some(count));
                }
                None => {
                    // v2's pixel verdict for the ambiguity the OCR cannot settle: an empty slot has
                    // no digits, exactly like a failed crop. Colour settles it.
                    let empty = warm_fraction(&image) < FeederCount::EMPTY_SLOT_BELOW;
                    counts.push(empty.then_some(0));
                }
            }
        }
        Ok(FeederCounts {
            counts,
            digits_read,
        })
    }

    fn hover_panel(&mut self, at: Point) -> Result<Vec<String>, String> {
        if !self.tooltip.is_set() {
            return Ok(Vec::new());
        }
        let region = Rect::new(
            at.x + self.tooltip.offset_x,
            at.y + self.tooltip.offset_y,
            self.tooltip.width,
            self.tooltip.height,
        );
        let (_image, lines) = self.capture_lines(region)?;
        Ok(lines.into_iter().map(|(text, _)| text).collect())
    }

    fn slot_state(&mut self, row: &PetRowPlan) -> Result<SlotState, String> {
        let (Some(slot), Some(reference)) = (row.boarding_pet_slot, row.empty_reference.as_ref())
        else {
            return Ok(SlotState::default());
        };
        let live = self.capture(Some(slot))?;
        let difference = differing_fraction(reference, &live);
        Ok(SlotState {
            difference,
            empty: empty_from_difference(difference, self.pet_slot_occupied_above),
        })
    }

    fn client_capture(&mut self) -> Result<PetImage, String> {
        self.capture(None)
    }

    fn region_capture(&mut self, region: Rect) -> Result<PetImage, String> {
        self.capture(Some(region))
    }

    fn cursor(&mut self) -> Option<Point> {
        seal_win::window::cursor_position()
    }

    fn game_in_front(&mut self) -> bool {
        seal_win::window::find_window(&self.title).is_some_and(seal_win::window::is_foreground)
    }
}

/// v2 `IconMatch.DifferingFraction`: the fraction of pixels whose worst channel differs by more than
/// [`ICON_TOLERANCE`], after insetting both images by [`ICON_INSET`]. `None` when the sizes cannot be
/// compared (the two crops are taken in the same calibrated region, so a mismatch means a stale
/// reference — "unknown" is safer than a resized comparison).
fn differing_fraction(reference: &ReferenceCrop, live: &PetImage) -> Option<f64> {
    if reference.width != live.width
        || reference.height != live.height
        || reference.rgba.len() < reference.width * reference.height * 4
        || live.rgba.len() < live.width * live.height * 4
    {
        return None;
    }
    let mut inset = ICON_INSET;
    if inset * 2 >= reference.width.min(reference.height) {
        inset = 0;
    }
    let mut differing = 0usize;
    let mut compared = 0usize;
    for y in inset..reference.height - inset {
        for x in inset..reference.width - inset {
            let index = (y * reference.width + x) * 4;
            let mut difference = 0i32;
            for channel in 0..3 {
                difference = difference.max(
                    (i32::from(reference.rgba[index + channel])
                        - i32::from(live.rgba[index + channel]))
                    .abs(),
                );
            }
            compared += 1;
            if difference > ICON_TOLERANCE {
                differing += 1;
            }
        }
    }
    (compared > 0).then(|| differing as f64 / compared as f64)
}

/// v2 `FeederCount.WarmFraction`: the fraction of the (inset) crop that looks strongly red-over-blue
/// — the one question the OCR cannot answer, because an empty slot and a failed crop both have no
/// digits.
fn warm_fraction(image: &PetImage) -> f64 {
    let inset = WARM_INSET;
    if inset * 2 >= image.width.min(image.height) {
        return 0.0;
    }
    let mut warm = 0usize;
    let mut counted = 0usize;
    for y in inset..image.height - inset {
        for x in inset..image.width - inset {
            let index = (y * image.width + x) * 4;
            let blue = i32::from(image.rgba[index + 2]);
            let red = i32::from(image.rgba[index]);
            counted += 1;
            if red - blue > WARM_RED_OVER_BLUE {
                warm += 1;
            }
        }
    }
    if counted == 0 {
        0.0
    } else {
        warm as f64 / counted as f64
    }
}

/// The domain's three-way slot answer from the raw difference: empty at/below the configured
/// occupied threshold, occupied below the "not the window" ceiling, unknown above it (a covered
/// screen or a window that never opened — the measured 0.85 gap, `PET_SLOT_NOT_THE_WINDOW_ABOVE`).
fn empty_from_difference(difference: Option<f64>, occupied_above: f64) -> Option<bool> {
    let difference = difference?;
    if difference <= occupied_above {
        Some(true)
    } else if difference < PET_SLOT_NOT_THE_WINDOW_ABOVE {
        Some(false)
    } else {
        None
    }
}

// ── icon matching: v2 `IconMatch` as pure comparison code ───────────────────────────────────
//
// Same reasoning as the gem composer's crop comparison: this is measurement arithmetic fed by the
// capture seam, not a decision, so it lives here as small pure functions named after v2's and is
// unit-tested without a game. The reference icons are decoded once in `plan`; the bag image comes
// from `PetSource::client_capture`, i.e. the same RGBA the captures produce.

/// v2 `IconMatch.SearchRadius`: the ±3 px offset search that accounts for a sprite not sitting
/// pixel-exactly where the grid says.
const SEARCH_RADIUS: i32 = 3;

/// v2 `PetTool.BagCellChangedAbove`: a bag cell is "emptied"/"moved" above this differing fraction.
const BAG_CELL_CHANGED_ABOVE: f64 = 0.10;

/// v2 `PetTool.FoodDragAttempts`.
const FOOD_DRAG_ATTEMPTS: i32 = 3;

/// The UI waits v2 uses (`PetTool.cs:33-100`). They are decisions' waits, not clock reads.
const WINDOW_WAIT: f64 = 0.9;
const CLICK_WAIT: f64 = 0.35;
const PAGE_WAIT: f64 = 0.9;
const DRAG_GRAB_WAIT: f64 = 0.35;
const START_WAIT: f64 = 2.0;
const RETRY_MINUTES: f64 = 5.0;

fn inset_bounds(width: usize, height: usize, inset: usize) -> Option<(usize, usize, usize, usize)> {
    if inset * 2 >= width || inset * 2 >= height {
        return None;
    }
    Some((inset, inset, width - inset, height - inset))
}

/// v2 `Cv2.Resize(..., InterpolationFlags.Area)`: box-average downscale, nearest upscale (icons are
/// compared against a cell crop that is normally larger, so the downscale is the path that matters).
fn area_resize(image: &PetImage, width: usize, height: usize) -> PetImage {
    if width == 0 || height == 0 {
        return PetImage {
            width,
            height,
            rgba: Vec::new(),
        };
    }
    let mut rgba = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let src_x = ((x as f64 + 0.5) * image.width as f64 / width as f64)
                .floor()
                .clamp(0.0, (image.width.saturating_sub(1)) as f64)
                as usize;
            let src_y = ((y as f64 + 0.5) * image.height as f64 / height as f64)
                .floor()
                .clamp(0.0, (image.height.saturating_sub(1)) as f64)
                as usize;
            let source = (src_y * image.width + src_x) * 4;
            let target = (y * width + x) * 4;
            rgba[target..target + 4].copy_from_slice(&image.rgba[source..source + 4]);
        }
    }
    PetImage {
        width,
        height,
        rgba,
    }
}

/// v2 `IconMatch.DifferingFraction`: the fraction of pixels whose worst channel differs by more than
/// `tolerance`, after insetting both, with `b` resized to `a`'s size first (v2 resizes the CELL crop
/// down to the reference).
fn differing_fraction_images(
    a: &PetImage,
    b: &PetImage,
    inset: usize,
    tolerance: i32,
) -> Option<f64> {
    if a.width == 0 || a.height == 0 || b.width == 0 || b.height == 0 {
        return None;
    }
    let (left, top, right, bottom) = inset_bounds(a.width, a.height, inset)?;
    let other = if a.width == b.width && a.height == b.height {
        b.clone()
    } else {
        area_resize(b, a.width, a.height)
    };
    let mut differing = 0usize;
    let mut compared = 0usize;
    for y in top..bottom {
        for x in left..right {
            let index = (y * a.width + x) * 4;
            let mut difference = 0i32;
            for channel in 0..3 {
                difference = difference.max(
                    (i32::from(a.rgba[index + channel]) - i32::from(other.rgba[index + channel]))
                        .abs(),
                );
            }
            compared += 1;
            if difference > tolerance {
                differing += 1;
            }
        }
    }
    (compared > 0).then(|| differing as f64 / compared as f64)
}

fn pitch(grid: &[i32]) -> Option<(i32, i32)> {
    let x = BagGrid::pitch_x(grid).round() as i32;
    let y = BagGrid::pitch_y(grid).round() as i32;
    (x > 0 && y > 0).then_some((x, y))
}

/// v2 `IconMatch.BestOffsetScore`: the lowest differing fraction over every offset within
/// `SEARCH_RADIUS` of `centre`, comparing `icon` with the bag region at the cell pitch.
fn best_offset_score(
    bag: &PetImage,
    centre: Point,
    pitch_x: i32,
    pitch_y: i32,
    icon: &PetImage,
) -> Option<f64> {
    let mut best = f64::MAX;
    for dy in -SEARCH_RADIUS..=SEARCH_RADIUS {
        for dx in -SEARCH_RADIUS..=SEARCH_RADIUS {
            let left = centre.x + dx - pitch_x / 2;
            let top = centre.y + dy - pitch_y / 2;
            if left < 0 || top < 0 {
                continue;
            }
            let (left, top) = (left as usize, top as usize);
            let (width, height) = (pitch_x as usize, pitch_y as usize);
            if left + width > bag.width || top + height > bag.height {
                continue;
            }
            let mut crop = PetImage {
                width,
                height,
                rgba: Vec::with_capacity(width * height * 4),
            };
            for y in top..top + height {
                let start = (y * bag.width + left) * 4;
                crop.rgba
                    .extend_from_slice(&bag.rgba[start..start + width * 4]);
            }
            if let Some(score) = differing_fraction_images(icon, &crop, ICON_INSET, ICON_TOLERANCE)
            {
                best = best.min(score);
                if best == 0.0 {
                    return Some(0.0);
                }
            }
        }
    }
    (best != f64::MAX).then_some(best)
}

/// v2 `IconMatch.ScoreAll`: every cell's score, best first, over a calibrated bag grid.
fn score_all(bag: &PetImage, grid: &[i32], icon: &PetImage) -> Vec<(usize, f64)> {
    let Some((pitch_x, pitch_y)) = pitch(grid) else {
        return Vec::new();
    };
    let mut scores: Vec<(usize, f64)> = BagGrid::centres(Some(grid))
        .into_iter()
        .enumerate()
        .filter_map(|(index, centre)| {
            best_offset_score(bag, centre, pitch_x, pitch_y, icon).map(|score| (index, score))
        })
        .filter(|(_, score)| *score <= MATCH_LIMIT)
        .collect();
    scores.sort_by(|a, b| a.1.total_cmp(&b.1));
    scores
}

/// v2 `IconMatch.ScoreCell`: one bag cell's score against an icon, with the sweep's own inset,
/// tolerance and ±radius search. The crop must be a cell plus the radius on every side (v2's
/// null-when-too-tight rule), so a caller comparing a smaller window fails loudly instead of
/// silently scoring a different comparison than the sweep.
pub fn score_cell(crop: &PetImage, icon: &PetImage, pitch_x: i32, pitch_y: i32) -> Option<f64> {
    if pitch_x <= 0 || pitch_y <= 0 {
        return None;
    }
    if (crop.width as i32) < pitch_x + 2 * SEARCH_RADIUS
        || (crop.height as i32) < pitch_y + 2 * SEARCH_RADIUS
    {
        return None;
    }
    let centre = Point::new((crop.width / 2) as i32, (crop.height / 2) as i32);
    best_offset_score(crop, centre, pitch_x, pitch_y, icon)
}

/// v2 `IconMatch.CellsHolding`: every cell holding `icon`, in CELL order (the order the food walk
/// relies on; score order is arbitrary for near-identical instances).
pub fn cells_holding(bag: &PetImage, grid: &[i32], icon: &PetImage, limit: f64) -> Vec<usize> {
    let Some((pitch_x, pitch_y)) = pitch(grid) else {
        return Vec::new();
    };
    BagGrid::centres(Some(grid))
        .into_iter()
        .enumerate()
        .filter_map(|(index, centre)| {
            best_offset_score(bag, centre, pitch_x, pitch_y, icon)
                .filter(|score| *score <= limit)
                .map(|_| index)
        })
        .collect()
}

// ── the loop ───────────────────────────────────────────────────────────────────────────────

/// One row's live schedule state.
#[derive(Debug, Clone)]
struct RowState {
    /// `Some` while the row is scheduled; `None` once it has been given up on (v2's fold drops it).
    next_minutes: Option<f64>,
    failures: i32,
    boarding: bool,
    detail: Option<String>,
    growth_target: Option<i32>,
    measured_rate: Option<i32>,
    species: Option<String>,
    panel: Option<PetPanel>,
}

impl RowState {
    fn new(boarding: bool) -> Self {
        Self {
            next_minutes: Some(f64::INFINITY),
            failures: 0,
            boarding,
            detail: None,
            growth_target: None,
            measured_rate: None,
            species: None,
            panel: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Click 目錄 and the feed icon; the window opens.
    Open,
    /// Read every row through the open window; this reading is also the schedule.
    Look,
    /// The batch of due rows, in row order.
    Visit,
    /// Wait for the soonest next (a cancel is noticed inside the wait by the runner).
    Wait,
    /// The run is over.
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Begin,
    EndToggle,
    VerifyEmpty,
    PickPage,
    CapturePage,
    ScanPage,
    ChooseCandidate,
    HoverRead,
    ClickCandidate,
    VerifyPlaced,
    FoodPick,
    FoodPlace,
    FoodDragCheck,
    FoodDone,
    StartToggle,
    VerifyStarted,
    Fold,
}

#[derive(Debug, Clone)]
struct Candidate {
    point: Point,
    label: String,
    species: Option<String>,
}

#[derive(Debug)]
struct Visit {
    batch: Vec<usize>,
    pos: usize,
    stage: Stage,
    page: usize,
    candidates: Vec<Candidate>,
    candidate: Option<Candidate>,
    attempts: i32,
    stack: i32,
    food_used: i32,
    food_cell: Option<Point>,
    slot_centre: Option<Point>,
    before: Option<PetImage>,
    outcome: Option<RowOutcome>,
}

impl Visit {
    fn new(batch: Vec<usize>) -> Self {
        Self {
            batch,
            pos: 0,
            stage: Stage::Begin,
            page: 0,
            candidates: Vec::new(),
            candidate: None,
            attempts: 0,
            stack: 0,
            food_used: 0,
            food_cell: None,
            slot_centre: None,
            before: None,
            outcome: None,
        }
    }
}

/// The resident pet feeder's loop: the plan/read-seam adapter around the domain's schedule, reload
/// and fold rules.
pub struct PetTool<S: PetSource> {
    plan: PetPlan,
    source: S,
    /// The game hand-over capability; see [`PetTool::ensure_game`] / [`PetTool::release_game`].
    game: Arc<dyn GamePort>,
    held: bool,
    state: Vec<RowState>,
    phase: Phase,
    queue: VecDeque<Decision>,
    pending_stop: Option<StopReason>,
    visit: Option<Visit>,
    message: String,
    countdown_s: Option<f64>,
}

impl<S: PetSource> PetTool<S> {
    pub fn new(plan: PetPlan, source: S, game: Arc<dyn GamePort>) -> Self {
        let state: Vec<RowState> = plan
            .rows
            .iter()
            .map(|row| RowState::new(row.boarding_running))
            .collect();
        let message = visit_opening_message(
            &plan
                .active_rows()
                .map(|row| row.name.clone())
                .collect::<Vec<_>>(),
        );
        Self {
            plan,
            source,
            game,
            held: false,
            state,
            phase: Phase::Open,
            queue: VecDeque::new(),
            pending_stop: None,
            visit: None,
            message,
            countdown_s: None,
        }
    }

    /// Take the game before the FIRST decision that can click (the opening look included): if a
    /// foreground tool holds it, the worker blocks here and produces no decisions — the feeder must
    /// not drive input behind another run.
    fn ensure_game(&mut self) -> Result<(), StopReason> {
        if self.held {
            return Ok(());
        }
        self.game
            .request(ToolId::Pet)
            .map(|()| self.held = true)
            .map_err(StopReason::error)
    }

    /// Give the game back (idempotent).
    fn release_game(&mut self) {
        if self.held {
            self.game.release(ToolId::Pet);
            self.held = false;
        }
    }
    pub fn card(&self) -> PetCard {
        let views: Vec<PetRowView> = self
            .plan
            .rows
            .iter()
            .zip(self.state.iter())
            .filter(|(row, _)| row.enabled)
            .map(|(row, state)| PetRowView {
                name: row.name.clone(),
                has_next: state.next_minutes.is_some(),
                boarding_running: state.boarding,
                next_minutes: state.next_minutes,
                pet_detail: state.detail.clone(),
                growth_target: state.growth_target,
                measured_rate: state.measured_rate,
            })
            .collect();
        build_card(&views)
    }

    fn active_rows(&self) -> Vec<usize> {
        self.plan
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.enabled)
            .map(|(index, _)| index)
            .collect()
    }

    fn wait(&mut self, seconds: f64) {
        self.queue.push_back(Decision::Wait(seconds));
    }

    fn click(&mut self, point: Point) {
        self.queue.push_back(Decision::Click(point));
    }

    fn action_wait(&self) -> f64 {
        (self.plan.action_wait_ms.max(200) as f64) / 1000.0
    }

    fn schedule_settings(&self) -> PetScheduleSettings {
        PetScheduleSettings {
            items_per_minute: self.plan.items_per_minute.max(1),
            wait_after_empty_minutes: self.plan.wait_after_empty_minutes,
            reload_on_start: self.plan.reload_on_start,
        }
    }

    /// The opening look: read every active row through the window that was just opened. The reading
    /// and the schedule are the same pass (v2 `ReadRows`), and a due row is reloaded in this window
    /// (v2's look hand-off), so a cold start does not close and re-open.
    fn look(&mut self) {
        let mut retry = false;
        let mut due: Vec<usize> = Vec::new();
        let mut unread = false;
        for index in self.active_rows() {
            let row = self.plan.rows[index].clone();
            match self.source.slot_state(&row) {
                Ok(slot) => match slot.empty {
                    Some(true) => {
                        self.state[index].boarding = false;
                        self.state[index].detail = None;
                        self.state[index].species = None;
                        self.state[index].next_minutes = Some(0.0);
                        due.push(index);
                    }
                    Some(false) => {
                        self.state[index].boarding = true;
                        let was_due = self.state[index]
                            .next_minutes
                            .is_some_and(|minutes| minutes <= 0.0);
                        let reading = self.read_boarding_row(index);
                        if reading.next_minutes <= 0.0 {
                            self.state[index].next_minutes = Some(0.0);
                            due.push(index);
                        } else if was_due {
                            // The timer said this row was due; this fresh read may cancel the
                            // reload. Positive-only: only a fresh count with food in the slot vetoes.
                            let (skip, note) = consider_veto(&reading, self.state[index].failures);
                            if skip {
                                self.message = note;
                            } else {
                                self.state[index].next_minutes = Some(0.0);
                                due.push(index);
                            }
                        }
                    }
                    None => {
                        // Unknown keeps the previous belief (v2's rule) and schedules from the
                        // configured cycle rather than acting on a guess.
                        unread = true;
                        let fallback = schedule_for(
                            &ScheduleRead {
                                boarding_running: true,
                                items_left: None,
                                completion_minutes: None,
                                rate: self.plan.items_per_minute.max(1),
                                stacks: row.stacks,
                                slot_occupied: false,
                            },
                            &self.schedule_settings(),
                        );
                        self.state[index].next_minutes = Some(fallback.next_minutes);
                    }
                },
                Err(line) => {
                    retry = true;
                    self.message = line;
                    break;
                }
            }
        }
        if retry {
            for index in self.active_rows() {
                if self.state[index].next_minutes.is_some() {
                    self.state[index].next_minutes = Some(RETRY_MINUTES);
                }
            }
            self.close_window();
            self.phase = Phase::Wait;
            return;
        }
        if unread {
            self.message =
                "One or more slots couldn't be read — carrying on with the configured cycle."
                    .to_owned();
        }
        if due.is_empty() {
            self.close_window();
            self.phase = Phase::Wait;
            self.message = self
                .card()
                .schedule_line(&self.next_label(self.soonest_minutes()));
        } else {
            self.message = visit_opening_message(
                &due.iter()
                    .map(|index| self.plan.rows[*index].name.clone())
                    .collect::<Vec<_>>(),
            );
            self.phase = Phase::Visit;
            self.visit = Some(Visit::new(due));
        }
    }

    /// Read a boarding row: the game's own time line and rate, then the feeder counts; the three feed
    /// `schedule_for`. A read failure falls back to the configured cycle (never a guess that a row is
    /// dry).
    fn read_boarding_row(&mut self, index: usize) -> seal_domain::tools::pet::RowReading {
        let row = self.plan.rows[index].clone();
        let (eta, rate) = match self.source.row_lines(&row) {
            Ok(lines) => (FeederEta::parse(&lines), FeederEta::parse_rate(&lines)),
            Err(_) => (None, None),
        };
        let items = match self.source.feeder_counts(&row) {
            Ok(counts) => counts.total(),
            Err(_) => None,
        };
        let completion = FeederEta::completion_minutes(eta.as_ref()).map(f64::from);
        let read = ScheduleRead {
            boarding_running: true,
            items_left: items,
            completion_minutes: completion,
            rate: rate_for(rate, self.plan.items_per_minute),
            stacks: row.stacks,
            slot_occupied: true,
        };
        let reading = schedule_for(&read, &self.schedule_settings());
        let state = &mut self.state[index];
        state.next_minutes = Some(reading.next_minutes);
        state.measured_rate = rate;
        state.growth_target = eta
            .as_ref()
            .and_then(|reading| FeederEta::growth_target(Some(&reading.line)));
        state.detail = eta.as_ref().map(|reading| reading.line.clone());
        reading
    }

    fn soonest_minutes(&self) -> f64 {
        self.state
            .iter()
            .filter(|state| state.next_minutes.is_some())
            .filter_map(|state| state.next_minutes)
            .fold(f64::INFINITY, f64::min)
    }

    fn next_label(&self, minutes: f64) -> String {
        if minutes.is_finite() {
            format!("{minutes:.0} min from now")
        } else {
            "later".to_owned()
        }
    }

    fn close_window(&mut self) {
        if let Some(close) = self.plan.setup.close_button.as_deref().and_then(point_of) {
            self.click(close);
            self.wait(WINDOW_WAIT);
        }
    }

    fn fold(&mut self, index: usize, outcome: RowOutcome) {
        let row = &self.plan.rows[index];
        let loaded = seal_domain::tools::pet::load_minutes(
            row.stacks,
            self.state[index]
                .measured_rate
                .unwrap_or_else(|| self.plan.items_per_minute.max(1)),
        );
        let label = format!("+{} min", loaded + self.plan.wait_after_empty_minutes);
        let folded = fold_back(
            &row.name,
            &outcome,
            self.state[index].failures,
            loaded,
            self.plan.wait_after_empty_minutes,
            &label,
        );
        let state = &mut self.state[index];
        state.failures = folded.failures;
        state.next_minutes = folded.next_minutes;
        match &outcome {
            RowOutcome::Reloaded { .. } => state.boarding = true,
            RowOutcome::Failed { .. } => {}
            RowOutcome::Deferred | RowOutcome::NothingToBoard => state.boarding = false,
        }
        self.message = folded.message;
    }

    fn visit_step(&mut self) {
        let Some(mut visit) = self.visit.take() else {
            self.phase = Phase::Wait;
            return;
        };
        if visit.pos >= visit.batch.len() {
            self.close_window();
            self.phase = Phase::Wait;
            self.message = self
                .card()
                .schedule_line(&self.next_label(self.soonest_minutes()));
            self.visit = None;
            return;
        }
        let index = visit.batch[visit.pos];
        let row_name = self.plan.rows[index].name.clone();
        let row = self.plan.rows[index].clone();
        match visit.stage {
            Stage::Begin => {
                self.message = reload_step_message(&row_name, ReloadStep::Reloading);
                visit.stage = Stage::EndToggle;
            }
            Stage::EndToggle => {
                if self.state[index].boarding {
                    self.message = reload_step_message(&row_name, ReloadStep::EndingBoarding);
                    if let Some(toggle) = row.toggle_label.as_deref().and_then(toggle_point) {
                        self.click(toggle);
                        self.wait(self.action_wait());
                    }
                    visit.stage = Stage::VerifyEmpty;
                } else {
                    visit.stage = Stage::VerifyEmpty;
                }
            }
            Stage::VerifyEmpty => match self.source.slot_state(&row) {
                Ok(slot) if slot.empty == Some(false) => {
                    visit.outcome = Some(RowOutcome::Failed {
                        error: seal_domain::tools::pet::end_toggle_failed(&row_name),
                    });
                    visit.stage = Stage::Fold;
                }
                _ => visit.stage = Stage::PickPage,
            },
            Stage::PickPage => {
                visit.page = 0;
                visit.candidates.clear();
                visit.attempts = 0;
                visit.stack = 0;
                visit.food_used = self.plan.food_slots_used;
                visit.stage = Stage::CapturePage;
            }
            Stage::CapturePage => {
                let tabs = self.plan.setup.page_tabs.clone();
                if visit.page < tabs.len() {
                    if let Some(tab) = point_of(tabs[visit.page].as_slice()) {
                        self.click(tab);
                        self.wait(PAGE_WAIT);
                    }
                    visit.stage = Stage::ScanPage;
                } else {
                    visit.stage = Stage::ChooseCandidate;
                }
            }
            Stage::ScanPage => {
                let icons = self.plan.queue_icons.clone();
                if let Ok(bag) = self.source.client_capture()
                    && let Some(grid) = self.plan.setup.bag_grid.clone()
                {
                    let centres = BagGrid::centres(Some(&grid));
                    let mut found: Vec<Candidate> = Vec::new();
                    for (entry, icon) in icons.iter().enumerate() {
                        let Some(icon) = icon else { continue };
                        for (cell, _score) in score_all(&bag, &grid, icon) {
                            if let Some(centre) = centres.get(cell) {
                                found.push(Candidate {
                                    point: *centre,
                                    label: self
                                        .plan
                                        .queue
                                        .get(entry)
                                        .map(|entry| entry.label.clone())
                                        .unwrap_or_default(),
                                    species: self
                                        .plan
                                        .queue
                                        .get(entry)
                                        .and_then(|entry| entry.species.clone()),
                                });
                            }
                        }
                    }
                    visit.candidates.extend(found);
                }
                visit.page += 1;
                visit.stage = Stage::CapturePage;
            }
            Stage::ChooseCandidate => {
                if visit.candidates.is_empty() && self.plan.setup.return_slot.is_some() {
                    // The return slot holds the pet this row just finished feeding; it has no queue
                    // entry, so no icon is needed and no species line is known.
                    let slot = self.plan.setup.return_slot.clone().unwrap_or_default();
                    let cell = slot.get(1).copied().unwrap_or(0);
                    if let Some(centre) =
                        self.plan.setup.bag_grid.as_deref().and_then(|grid| {
                            BagGrid::centres(Some(grid)).get(cell as usize).copied()
                        })
                    {
                        visit.candidates.push(Candidate {
                            point: centre,
                            label: String::new(),
                            species: None,
                        });
                    }
                }
                if visit.candidates.is_empty() {
                    let reason = if self.plan.queue.is_empty() {
                        seal_domain::tools::pet::no_way_to_find_pet_message()
                    } else {
                        seal_domain::tools::pet::nothing_matched_message()
                    };
                    self.message = reason;
                    visit.outcome = Some(RowOutcome::NothingToBoard);
                    visit.stage = Stage::Fold;
                } else {
                    visit.candidate = Some(visit.candidates.remove(0));
                    if self.plan.tooltip.is_set() {
                        if let Some(candidate) = &visit.candidate {
                            self.queue.push_back(Decision::Place(candidate.point));
                            self.wait(self.plan.tooltip.hover_delay_ms.max(100) as f64 / 1000.0);
                        }
                        visit.stage = Stage::HoverRead;
                    } else {
                        visit.stage = Stage::ClickCandidate;
                    }
                }
            }
            Stage::HoverRead => {
                let candidate = visit.candidate.clone();
                let mut finished = false;
                if let Some(candidate) = &candidate
                    && let Ok(lines) = self.source.hover_panel(candidate.point)
                    && let Some(panel) = PetPanel::parse(&lines)
                {
                    finished = panel.is_finished();
                    self.state[index].panel = Some(panel);
                    self.state[index].species = candidate.species.clone();
                }
                if finished {
                    self.message = seal_domain::tools::pet::all_refused_message(&[candidate
                        .map(|candidate| candidate.label)
                        .unwrap_or_default()]);
                    visit.stage = Stage::ChooseCandidate;
                } else {
                    visit.stage = Stage::ClickCandidate;
                }
            }
            Stage::ClickCandidate => {
                self.message = reload_step_message(&row_name, ReloadStep::PlacingPet);
                if let Some(candidate) = visit.candidate.clone() {
                    self.queue.push_back(Decision::Place(candidate.point));
                    self.queue.push_back(Decision::RightClickHere);
                    self.wait(self.action_wait());
                }
                visit.attempts += 1;
                visit.stage = Stage::VerifyPlaced;
            }
            Stage::VerifyPlaced => match self.source.slot_state(&row) {
                Ok(slot) if slot.empty == Some(false) => {
                    visit.stage = Stage::FoodPick;
                }
                _ => {
                    if visit.attempts < PLACE_ATTEMPTS {
                        visit.stage = Stage::ClickCandidate;
                    } else {
                        visit.outcome = Some(RowOutcome::Failed {
                            error: pet_did_not_go_in(),
                        });
                        visit.stage = Stage::Fold;
                    }
                }
            },
            Stage::FoodPick => {
                if visit.stack < row.stacks.max(1) {
                    self.message = reload_step_message(&row_name, ReloadStep::LoadingFood);
                }
                if visit.stack >= row.stacks.max(1) {
                    visit.stage = Stage::StartToggle;
                } else {
                    match self.plan.food_slots.get(visit.food_used as usize) {
                        Some(cell) => {
                            let page = cell.first().copied().unwrap_or(0);
                            let cell_index = cell.get(1).copied().unwrap_or(0);
                            let tabs = self.plan.setup.page_tabs.clone();
                            if let Some(tab) = tabs.get(page as usize).and_then(|tab| point_of(tab))
                            {
                                self.click(tab);
                                self.wait(PAGE_WAIT);
                            }
                            visit.food_cell =
                                self.plan.setup.bag_grid.as_deref().and_then(|grid| {
                                    BagGrid::centres(Some(grid))
                                        .get(cell_index as usize)
                                        .copied()
                                });
                            visit.stage = Stage::FoodPlace;
                        }
                        None => {
                            visit.outcome = Some(RowOutcome::Failed {
                                error: format!(
                                    "No food cells left — {} of {} used. Mark the cells holding food again on the Pet tab.",
                                    visit.food_used,
                                    self.plan.food_slots.len()
                                ),
                            });
                            visit.stage = Stage::Fold;
                        }
                    }
                }
            }
            Stage::FoodPlace => {
                let Some(cell) = visit.food_cell else {
                    visit.outcome = Some(RowOutcome::Failed {
                        error: "The bag grid isn't calibrated — save the Boarding bag grid on the Calibrate tab; there is nothing to right-click."
                            .to_owned(),
                    });
                    visit.stage = Stage::Fold;
                    self.visit = Some(visit);
                    return;
                };
                visit.slot_centre = self.plan.rows[index]
                    .feeder_strip
                    .as_deref()
                    .and_then(|strip| FeederLayout::slot_boxes(Some(strip), row.stacks))
                    .and_then(|slots| slots.get(visit.stack as usize).cloned())
                    .and_then(|slot| {
                        (slot.len() >= 4)
                            .then(|| Point::new(slot[0] + slot[2] / 2, slot[1] + slot[3] / 2))
                    });
                let drag = food_load::is_drag(Some(&self.plan.setup.food_load_mode));
                if drag {
                    let slot = visit.slot_centre;
                    let Some(slot) = slot else {
                        visit.outcome = Some(RowOutcome::Failed {
                            error: "The food load is set to drag but this row's food strip doesn't divide into its stack count — save the Row food strip on the Calibrate tab, or set the load mode back to \"right_click\" on the Pet tab."
                                .to_owned(),
                        });
                        visit.stage = Stage::Fold;
                        self.visit = Some(visit);
                        return;
                    };
                    if let Some(grid) = self.plan.setup.bag_grid.as_deref()
                        && let Some((pitch_x, pitch_y)) = pitch(grid)
                    {
                        let rect =
                            Rect::new(cell.x - pitch_x / 2, cell.y - pitch_y / 2, pitch_x, pitch_y);
                        visit.before = self.source.region_capture(rect).ok();
                    }
                    self.queue.push_back(Decision::Place(cell));
                    self.wait(CLICK_WAIT);
                    self.queue.push_back(Decision::HoldLeft);
                    self.wait(DRAG_GRAB_WAIT);
                    self.queue.push_back(Decision::Place(slot));
                    self.wait(CLICK_WAIT);
                    self.queue.push_back(Decision::ReleaseLeft);
                    self.wait(self.action_wait());
                    visit.stage = Stage::FoodDragCheck;
                } else {
                    self.queue.push_back(Decision::Place(cell));
                    self.queue.push_back(Decision::RightClickHere);
                    self.wait(self.action_wait());
                    let max = self
                        .plan
                        .setup
                        .buy_sell_max_button
                        .clone()
                        .or_else(|| self.plan.setup.max_button.clone());
                    if let Some(max) = max.as_deref().and_then(point_of) {
                        self.click(max);
                        self.wait(self.action_wait());
                    }
                    self.queue.push_back(Decision::Enter);
                    self.wait(self.action_wait());
                    visit.stage = Stage::FoodDone;
                }
            }
            Stage::FoodDragCheck => {
                let mut moved = true;
                if let Some(before) = visit.before.take()
                    && let Some(cell) = visit.food_cell
                    && let Some(grid) = self.plan.setup.bag_grid.clone()
                    && let Some((pitch_x, pitch_y)) = pitch(&grid)
                {
                    let rect =
                        Rect::new(cell.x - pitch_x / 2, cell.y - pitch_y / 2, pitch_x, pitch_y);
                    if let Ok(after) = self.source.region_capture(rect) {
                        moved =
                            differing_fraction_images(&before, &after, ICON_INSET, ICON_TOLERANCE)
                                .is_none_or(|changed| changed > BAG_CELL_CHANGED_ABOVE);
                    }
                }
                if moved {
                    visit.stage = Stage::FoodDone;
                } else if visit.attempts < FOOD_DRAG_ATTEMPTS {
                    visit.attempts += 1;
                    visit.stage = Stage::FoodPlace;
                } else {
                    visit.outcome = Some(RowOutcome::Failed {
                        error: format!(
                            "{}: stack {} would not leave the bag after {FOOD_DRAG_ATTEMPTS} drag(s) — \
                             the cell still holds it, so nothing was loaded for that stack.",
                            row_name,
                            visit.stack + 1
                        ),
                    });
                    visit.stage = Stage::Fold;
                }
            }
            Stage::FoodDone => {
                visit.food_used += 1;
                visit.stack += 1;
                visit.stage = Stage::FoodPick;
            }
            Stage::StartToggle => {
                self.message = reload_step_message(&row_name, ReloadStep::StartingBoarding);
                if let Some(toggle) = row.toggle_label.as_deref().and_then(toggle_point) {
                    self.click(toggle);
                    self.wait(START_WAIT);
                }
                visit.stage = Stage::VerifyStarted;
            }
            Stage::VerifyStarted => {
                let started =
                    matches!(self.source.slot_state(&row), Ok(slot) if slot.empty == Some(false));
                if !started {
                    visit.outcome = Some(RowOutcome::Failed {
                        error: pet_did_not_go_in(),
                    });
                    visit.stage = Stage::Fold;
                } else {
                    let landed = match self.source.row_lines(&row) {
                        Ok(lines) => {
                            FeederEta::completion_minutes(FeederEta::parse(&lines).as_ref())
                                .map(f64::from)
                        }
                        Err(_) => None,
                    };
                    let panel = self.state[index].panel;
                    let species = self.state[index].species.clone();
                    let pet_minutes = panel.and_then(|panel| {
                        PetFeeding::find(
                            &self.plan.feeding,
                            species.as_deref(),
                            panel.stage.unwrap_or(-1),
                        )
                        .and_then(|line| {
                            PetFeeding::minutes_to_finish(Some(line), panel.growth, panel.exp)
                        })
                    });
                    visit.outcome = Some(RowOutcome::Reloaded {
                        pet_minutes,
                        landed_line_minutes: landed,
                    });
                    visit.stage = Stage::Fold;
                }
            }
            Stage::Fold => {
                let outcome = visit.outcome.take().unwrap_or(RowOutcome::Deferred);
                self.fold(index, outcome);
                visit.pos += 1;
                visit.stage = Stage::Begin;
                visit.attempts = 0;
                visit.stack = 0;
                visit.food_used = self.plan.food_slots_used;
                visit.candidate = None;
                visit.candidates.clear();
                visit.food_cell = None;
                visit.slot_centre = None;
                visit.before = None;
                if visit.pos >= visit.batch.len() {
                    self.close_window();
                    self.phase = Phase::Wait;
                    self.message = self
                        .card()
                        .schedule_line(&self.next_label(self.soonest_minutes()));
                    self.visit = None;
                    return;
                }
            }
        }
        self.visit = Some(visit);
    }
    fn decide_once(&mut self) -> Decision {
        loop {
            if let Some(decision) = self.queue.pop_front() {
                return decision;
            }
            if let Some(stop) = self.pending_stop.take() {
                return Decision::Stop(stop);
            }
            match self.phase {
                Phase::Open => {
                    self.countdown_s = None;
                    if let Err(reason) = self.ensure_game() {
                        return Decision::Stop(reason);
                    }
                    if let Some(menu) = self.plan.setup.menu_button.as_deref().and_then(point_of) {
                        self.click(menu);
                        self.wait(WINDOW_WAIT);
                    }
                    if let Some(feed) = self.plan.setup.feed_icon.as_deref().and_then(point_of) {
                        self.click(feed);
                        self.wait(WINDOW_WAIT);
                    }
                    self.message = seal_domain::tools::pet::checking_rows_message();
                    self.phase = Phase::Look;
                }
                Phase::Look => self.look(),
                Phase::Visit => self.visit_step(),
                Phase::Wait => {
                    // The window is closed (the close decisions drained first): give the game back
                    // before the long wait, so a deferred foreground start can run.
                    self.release_game();
                    let scheduled: Vec<usize> = self
                        .active_rows()
                        .into_iter()
                        .filter(|index| self.state[*index].next_minutes.is_some())
                        .collect();
                    if scheduled.is_empty() {
                        self.phase = Phase::Finished;
                        self.pending_stop = Some(StopReason::done(every_row_failed_message()));
                        continue;
                    }
                    let soonest = scheduled
                        .into_iter()
                        .min_by(|a, b| {
                            self.state[*a]
                                .next_minutes
                                .unwrap_or(f64::INFINITY)
                                .total_cmp(&self.state[*b].next_minutes.unwrap_or(f64::INFINITY))
                        })
                        .expect("scheduled is non-empty");
                    let minutes = self.state[soonest].next_minutes.unwrap_or(0.0);
                    if minutes <= 0.0 {
                        self.phase = Phase::Open;
                        self.message =
                            visit_opening_message(&[self.plan.rows[soonest].name.clone()]);
                    } else {
                        let seconds = minutes * 60.0;
                        let label = self.next_label(minutes);
                        self.countdown_s = Some(seconds);
                        self.message = self.card().schedule_line(&label);
                        self.phase = Phase::Open;
                        return Decision::Wait(seconds);
                    }
                }
                Phase::Finished => {
                    return Decision::Stop(
                        self.pending_stop
                            .take()
                            .unwrap_or_else(|| StopReason::done("The pet feeder is stopping.")),
                    );
                }
            }
        }
    }
}

impl<S: PetSource + Send> Tool for PetTool<S> {
    fn id(&self) -> ToolId {
        ToolId::Pet
    }

    fn status(&self) -> ToolStatus {
        let card = self.card();
        ToolStatus {
            message: self.message.clone(),
            countdown_s: self.countdown_s,
            rows: tool_rows(&card),
        }
    }

    fn decide(&mut self) -> Decision {
        let decision = self.decide_once();
        if decision.is_terminal() {
            // A terminal decision ends the run; no claim may outlive it.
            self.release_game();
        }
        decision
    }
}

impl<S: PetSource> Drop for PetTool<S> {
    fn drop(&mut self) {
        // A stop can land anywhere, including before the explicit release in `Phase::Wait`; the
        // claim must never outlive the tool (the effects' Drop guard is the same principle).
        self.release_game();
    }
}
/// One compact point conversion for a config `[x, y]`.
fn point_of(values: &[i32]) -> Option<Point> {
    (values.len() == 2).then(|| Point::new(values[0], values[1]))
}
/// The click point of a toggle: a point when drawn as one, else a box's centre (v2's
/// `toggle_label` is a box; a hand-drawn point is accepted too).
fn toggle_point(values: &[i32]) -> Option<Point> {
    match values.len() {
        2 => Some(Point::new(values[0], values[1])),
        4 => Some(Point::new(
            values[0] + values[2] / 2,
            values[1] + values[3] / 2,
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seal_domain::ocr::PetPanel;
    use seal_domain::tools::pet::{
        FeederEta, PetScheduleSettings, ScheduleRead, ScheduleSource, schedule_for,
    };

    /// A 2×2 red reference crop (RGBA), used for the comparison tests.
    fn red_crop(width: usize, height: usize) -> ReferenceCrop {
        ReferenceCrop {
            width,
            height,
            rgba: [255u8, 0, 0, 255].repeat(width * height),
        }
    }

    fn png_of(width: u32, height: u32, bgra: &[u8]) -> String {
        let png = seal_win::pure::encode_png(width, height, bgra).expect("encodes");
        base64::engine::general_purpose::STANDARD.encode(png)
    }

    /// A valid row: the marks `ready` demands, the reader's crop, and machine state.
    fn slot_json(index: usize) -> Value {
        serde_json::json!({
            "enabled": true,
            "toggle_label": [5, 5 + index as i64, 40, 20],
            "boarding_pet_slot": [50, 60 + 40 * index as i64, 30, 40],
            "feeder_strip": [100, 200 + 40 * index as i64, 300, 50],
            "stacks": 2,
            "pet_slot_empty_png": png_of(2, 2, &[0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255]),
            "boarding_running": false,
        })
    }

    fn config_with_rows(rows: usize) -> Value {
        let slots: Vec<Value> = (0..rows).map(slot_json).collect();
        serde_json::json!({
            "window": {"title": "TW_LIVE"},
            "buy_sell": {"max_button": [900, 500]},
            "pet": {
                "menu_button": [10, 10],
                "feed_icon": [20, 20],
                "close_button": [30, 30],
                "page_tabs": [[100, 100]],
                "bag_grid": [0, 0, 400, 400],
                "bag_slot": [0, 0, 40, 40],
                "food_slots": [[0, 0]],
                "return_slot": [0, 1],
                "food_load_mode": "right_click",
                "items_per_minute": 3,
                "pet_slot_occupied_above": 0.05,
                "slots": slots,
                "queue": [{"label": "pet a", "species": "S", "png": null}]
            },
            "tooltip": {"offset_x": 20, "offset_y": 10, "width": 200, "height": 120, "hover_delay_ms": 700}
        })
    }

    fn plan_of(rows: usize) -> PetPlan {
        plan(&config_with_rows(rows), Path::new("C:/root")).expect("a valid fixture")
    }

    fn row(index: usize) -> PetRowPlan {
        PetRowPlan {
            name: format!("Row {}", index + 1),
            enabled: true,
            toggle_label: Some(vec![5, 5 + index as i32, 40, 20]),
            boarding_pet_slot: Some(Rect::new(50, 60 + 40 * index as i32, 30, 40)),
            feeder_strip: Some(vec![100, 200, 300, 50]),
            stacks: 2,
            empty_reference: Some(red_crop(2, 2)),
            boarding_running: false,
        }
    }

    // ── plan / refusals ──────────────────────────────────────────────────────────────────────

    #[test]
    fn the_plan_resolves_the_setup_the_rows_and_the_reader_data() {
        let plan = plan_of(2);

        assert_eq!(plan.setup.slots.len(), 2);
        assert_eq!(plan.rows.len(), 2);
        assert_eq!(plan.rows[0].name, "Row 1");
        assert_eq!(plan.rows[1].name, "Row 2");
        assert_eq!(plan.rows[0].stacks, 2);
        assert_eq!(
            plan.rows[0].boarding_pet_slot,
            Some(Rect::new(50, 60, 30, 40))
        );
        let reference = plan.rows[0].empty_reference.as_ref().expect("decoded");
        assert_eq!(reference.width, 2);
        assert_eq!(reference.height, 2);
        assert_eq!(plan.items_per_minute, 3);
        assert_eq!(plan.wait_after_empty_minutes, 5);
        assert_eq!(plan.action_wait_ms, 1200);
        assert!(!plan.reload_on_start);
        assert_eq!(
            plan.feeder_count_left_fraction,
            FeederLayout::READ_LEFT_FRACTION
        );
        assert_eq!(plan.feeder_count_min_score, FeederCount::MIN_SCORE);
        assert_eq!(plan.tooltip.width, 200);
        assert_eq!(plan.root, PathBuf::from("C:/root"));
    }

    #[test]
    fn an_uncalibrated_setup_is_refused_with_the_domains_own_line() {
        let line = plan(&serde_json::json!({"pet": {}}), Path::new("C:/root"))
            .expect_err("an empty pet block cannot start");

        assert!(line.contains("pet menu button isn't calibrated"), "{line}");
        assert!(line.contains("Calibrate tab"), "{line}");
    }

    #[test]
    fn every_row_unticked_is_refused_with_the_domains_line() {
        let mut config = config_with_rows(2);
        config["pet"]["slots"][0]["enabled"] = serde_json::json!(false);
        config["pet"]["slots"][1]["enabled"] = serde_json::json!(false);

        let line = plan(&config, Path::new("C:/root")).expect_err("nothing to feed");

        assert_eq!(
            line,
            "Every row is unticked — nothing to feed. Tick the row on the Pet tab."
        );
    }

    #[test]
    fn drag_configured_without_a_food_strip_is_refused_with_the_domains_line() {
        let mut config = config_with_rows(1);
        config["pet"]["food_load_mode"] = serde_json::json!("drag");
        config["pet"]["slots"][0]["feeder_strip"] = Value::Null;

        let line = plan(&config, Path::new("C:/root")).expect_err("drag needs the strip");

        assert!(line.contains("food strip isn't drawn"), "{line}");
        assert!(line.contains("Calibrate tab"), "{line}");
    }

    #[test]
    fn a_food_cell_on_an_unmarked_page_is_refused_with_the_domains_line() {
        let mut config = config_with_rows(1);
        config["pet"]["food_slots"] = serde_json::json!([[1, 0]]);

        let line = plan(&config, Path::new("C:/root")).expect_err("only one page is marked");

        assert!(line.contains("marked on bag page 2"), "{line}");
        assert!(line.contains("only 1 page tab(s)"), "{line}");
    }

    #[test]
    fn no_way_to_find_a_pet_is_refused() {
        let mut config = config_with_rows(1);
        config["pet"]["return_slot"] = Value::Null;
        config["pet"]["queue"] = serde_json::json!([]);

        let line = plan(&config, Path::new("C:/root")).expect_err("no return slot, no queue");

        assert!(
            line.contains("Neither a return slot nor any queued pet icon"),
            "{line}"
        );
    }

    #[test]
    fn no_max_button_is_refused() {
        let mut config = config_with_rows(1);
        config["pet"]["max_button"] = Value::Null;
        config["buy_sell"]
            .as_object_mut()
            .unwrap()
            .remove("max_button");

        let line = plan(&config, Path::new("C:/root")).expect_err("no MAX");

        assert!(line.contains("No MAX is calibrated"), "{line}");
    }

    #[test]
    fn no_food_cells_are_refused() {
        let mut config = config_with_rows(1);
        config["pet"]["food_slots"] = serde_json::json!([]);

        let line = plan(&config, Path::new("C:/root")).expect_err("nothing says where food is");

        assert!(line.contains("No food cells are marked"), "{line}");
    }

    #[test]
    fn a_missing_empty_reference_is_tolerated_and_reads_unknown() {
        let mut config = config_with_rows(1);
        config["pet"]["slots"][0]["pet_slot_empty_png"] = Value::Null;

        let plan = plan(&config, Path::new("C:/root")).expect("a missing reference is not fatal");

        assert!(plan.rows[0].empty_reference.is_none());
        // No reference: the production slot read cannot make one up; the domain treats unknown as
        // "carry on" rather than as a failure.
        assert_eq!(empty_from_difference(None, 0.05), None);
    }

    #[test]
    fn a_corrupt_empty_reference_is_a_named_start_refusal() {
        let mut config = config_with_rows(1);
        config["pet"]["slots"][0]["pet_slot_empty_png"] = serde_json::json!("bm90IGEgcG5n");

        let line = plan(&config, Path::new("C:/root")).expect_err("a stale crop is a loud failure");

        assert!(
            line.contains("Row 1's empty-slot reference couldn't be decoded"),
            "{line}"
        );
        assert!(
            line.contains("re-capture it and set the new crop"),
            "{line}"
        );
    }

    #[test]
    fn a_level_one_board_refuses_drag_food_load_with_the_domains_sentence() {
        let mut config = config_with_rows(1);
        config["pet"]["food_load_mode"] = serde_json::json!("drag");
        let plan = plan(&config, Path::new("C:/root")).expect("the plan itself is valid");

        let line = check_board(&plan, Some(1)).expect_err("firmware 1 has no drag");

        assert!(line.contains("firmware 1"), "{line}");
        assert!(line.contains("reflash the board"), "{line}");
        // A board that never answered is a definite no, not "maybe".
        let line = check_board(&plan, None).expect_err("unreported firmware predates drag");
        assert!(line.contains("did not report a firmware version"), "{line}");
        assert_eq!(check_board(&plan, Some(2)), Ok(()));
    }

    #[test]
    fn right_click_food_load_is_compatible_with_every_board() {
        let plan = plan_of(1);

        assert_eq!(check_board(&plan, None), Ok(()));
        assert_eq!(check_board(&plan, Some(1)), Ok(()));
        assert_eq!(check_board(&plan, Some(2)), Ok(()));
    }

    // ── card surface ────────────────────────────────────────────────────────────────────────

    #[test]
    fn a_two_row_setup_publishes_two_rows_and_its_opening_line() {
        let plan = plan_of(2);

        let card = initial_card(&plan);
        let rows = tool_rows(&card);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "Row 1");
        assert_eq!(rows[0].status, "waiting for a reload");
        assert_eq!(rows[1].status, "waiting for a reload");
        assert_eq!(rows[0].detail, None);
        assert_eq!(rows[0].next_action_in_seconds, None);
        assert!(
            opening_message(&[plan.rows[0].name.clone()])
                .contains("Row 1: opening the boarding window")
        );
    }

    #[test]
    fn a_four_row_setup_publishes_every_row_and_the_batch_opening_line() {
        let mut config = config_with_rows(4);
        config["pet"]["slots"][1]["boarding_running"] = serde_json::json!(true);
        let plan = plan(&config, Path::new("C:/root")).expect("four rows");

        let card = initial_card(&plan);

        assert_eq!(card.rows.len(), 4);
        assert_eq!(card.rows[1].name, "Row 2");
        assert_eq!(card.rows[1].status, "boarding");
        assert_eq!(
            opening_message(&["Row 1".to_owned(), "Row 2".to_owned(), "Row 3".to_owned()]),
            "Opening the breeder for 3 rows…"
        );
    }

    #[test]
    fn the_status_carries_the_schedule_line_and_the_rows() {
        let plan = plan_of(2);
        let views = vec![
            PetRowView {
                name: "Row 1".to_owned(),
                has_next: true,
                boarding_running: true,
                next_minutes: Some(5.0),
                pet_detail: Some("a pet".to_owned()),
                growth_target: None,
                measured_rate: Some(3),
            },
            PetRowView {
                name: "Row 2".to_owned(),
                has_next: true,
                boarding_running: false,
                next_minutes: Some(120.0),
                pet_detail: None,
                growth_target: None,
                measured_rate: None,
            },
        ];
        let card = build_card(&views);

        let status = status(&card, "19:40");

        assert!(
            status.message.contains("boarding Row 1"),
            "{}",
            status.message
        );
        assert!(
            status.message.contains("next Row 1 at 19:40"),
            "{}",
            status.message
        );
        assert_eq!(status.rows.len(), 2);
        assert_eq!(status.rows[0].status, "boarding");
        assert_eq!(status.rows[0].next_action_in_seconds, Some(300.0));
        assert_eq!(
            status.rows[0].detail.as_deref(),
            Some("a pet  ·  3/min (game line)")
        );
        let _ = plan;
    }

    // ── the read seam, scripted ─────────────────────────────────────────────────────────────

    /// A scripted source: every method answers what the test set, no window or OCR involved.
    struct FakeSource {
        lines: Vec<String>,
        counts: FeederCounts,
        hover: Vec<String>,
        slot: SlotState,
        image: PetImage,
        cursor: Option<Point>,
        front: bool,
    }

    impl PetSource for FakeSource {
        fn row_lines(&mut self, _row: &PetRowPlan) -> Result<Vec<String>, String> {
            Ok(self.lines.clone())
        }
        fn feeder_counts(&mut self, _row: &PetRowPlan) -> Result<FeederCounts, String> {
            Ok(self.counts.clone())
        }
        fn hover_panel(&mut self, _at: Point) -> Result<Vec<String>, String> {
            Ok(self.hover.clone())
        }
        fn slot_state(&mut self, _row: &PetRowPlan) -> Result<SlotState, String> {
            Ok(self.slot)
        }
        fn client_capture(&mut self) -> Result<PetImage, String> {
            Ok(self.image.clone())
        }
        fn region_capture(&mut self, _region: Rect) -> Result<PetImage, String> {
            Ok(self.image.clone())
        }
        fn cursor(&mut self) -> Option<Point> {
            self.cursor
        }
        fn game_in_front(&mut self) -> bool {
            self.front
        }
    }

    fn fake() -> FakeSource {
        FakeSource {
            lines: vec![
                "代養完成預計所需時間：約14分".to_owned(),
                "每1分 攝取3個".to_owned(),
            ],
            counts: FeederCounts {
                counts: vec![Some(120), Some(300)],
                digits_read: true,
            },
            hover: vec!["(6階) +7 [52.18%]".to_owned()],
            slot: SlotState {
                difference: Some(0.0),
                empty: Some(true),
            },
            image: PetImage {
                width: 2,
                height: 2,
                rgba: vec![0u8; 16],
            },
            cursor: Some(Point::new(10, 20)),
            front: true,
        }
    }

    #[test]
    fn the_scripted_source_feeds_the_domains_row_readers() {
        let mut source = fake();
        let row = row(0);

        let lines = source.row_lines(&row).expect("scripted");
        assert_eq!(
            FeederEta::parse(&lines),
            Some(seal_domain::tools::pet::Reading {
                minutes: 14,
                is_completion: true,
                line: "代養完成預計所需時間：約14分".to_owned(),
            })
        );
        assert_eq!(FeederEta::parse_rate(&lines), Some(3));
        assert_eq!(
            source.feeder_counts(&row).expect("scripted").total(),
            Some(420)
        );
        assert_eq!(
            PetPanel::parse(&source.hover_panel(Point::new(0, 0)).expect("scripted"))
                .map(|panel| (panel.growth, panel.exp)),
            Some((7, 52.18))
        );
        assert_eq!(
            source.slot_state(&row).expect("scripted"),
            SlotState {
                difference: Some(0.0),
                empty: Some(true)
            }
        );
        assert_eq!(source.cursor(), Some(Point::new(10, 20)));
        assert!(source.game_in_front());
        assert_eq!(source.client_capture().expect("scripted").width, 2);
        assert_eq!(
            source
                .region_capture(Rect::new(0, 0, 2, 2))
                .expect("scripted")
                .height,
            2
        );
    }

    #[test]
    fn a_feeder_total_feeds_the_domains_schedule() {
        let counts = fake().counts;
        let read = ScheduleRead {
            boarding_running: true,
            items_left: counts.total(),
            completion_minutes: None,
            rate: 3,
            stacks: 2,
            slot_occupied: true,
        };

        let reading = schedule_for(
            &read,
            &PetScheduleSettings {
                items_per_minute: 3,
                wait_after_empty_minutes: 5,
                reload_on_start: false,
            },
        );

        assert_eq!(reading.source, ScheduleSource::CountRead);
        // 420 items at 3/min = 140 min of food, plus the 5-minute wait past empty.
        assert!((reading.next_minutes - 145.0).abs() < 1e-9, "{reading:?}");
        assert!(reading.slot_occupied);
    }

    // ── the production reader's pure comparisons ────────────────────────────────────────────

    #[test]
    fn the_slot_comparison_ignores_its_six_pixel_border() {
        let reference = red_crop(16, 16);
        let mut live = red_crop(16, 16);
        // Differ only in the excluded border rows/columns.
        for y in 0..16 {
            for x in 0..16 {
                if x < 6 || y < 6 || x >= 10 || y >= 10 {
                    let index = (y * 16 + x) * 4;
                    live.rgba[index] = 0; // red goes to zero in the border
                }
            }
        }

        let fraction = differing_fraction(
            &reference,
            &PetImage {
                width: 16,
                height: 16,
                rgba: live.rgba,
            },
        )
        .expect("same size");

        assert_eq!(fraction, 0.0);
    }

    #[test]
    fn an_interior_slot_difference_is_a_fraction_and_a_size_mismatch_is_unknown() {
        let reference = red_crop(16, 16);
        let mut live = red_crop(16, 16);
        live.rgba[(8 * 16 + 8) * 4] = 0; // one interior pixel, > tolerance
        let compared = (16 - 2 * ICON_INSET).pow(2) as f64;

        let fraction = differing_fraction(
            &reference,
            &PetImage {
                width: 16,
                height: 16,
                rgba: live.rgba,
            },
        )
        .expect("same size");
        assert!((fraction - 1.0 / compared).abs() < 1e-12, "{fraction}");

        let smaller = PetImage {
            width: 8,
            height: 8,
            rgba: vec![0; 8 * 8 * 4],
        };
        assert_eq!(differing_fraction(&reference, &smaller), None);
    }

    #[test]
    fn warm_pixels_count_as_food_colour_and_neutral_pixels_do_not() {
        let food = PetImage {
            width: 16,
            height: 16,
            rgba: [200u8, 40, 20, 255].repeat(16 * 16),
        };
        let neutral = PetImage {
            width: 16,
            height: 16,
            rgba: [128u8, 128, 128, 255].repeat(16 * 16),
        };

        assert!((warm_fraction(&food) - 1.0).abs() < 1e-12);
        assert_eq!(warm_fraction(&neutral), 0.0);
    }

    #[test]
    fn the_slot_three_way_split_sits_where_v2_measured_it() {
        assert_eq!(empty_from_difference(Some(0.0), 0.05), Some(true));
        assert_eq!(empty_from_difference(Some(0.05), 0.05), Some(true));
        assert_eq!(empty_from_difference(Some(0.06), 0.05), Some(false));
        assert_eq!(empty_from_difference(Some(0.84), 0.05), Some(false));
        // 0.85 and above is NOT the boarding window: unknown, never "occupied".
        assert_eq!(empty_from_difference(Some(0.85), 0.05), None);
        assert_eq!(empty_from_difference(Some(1.0), 0.05), None);
        assert_eq!(empty_from_difference(None, 0.05), None);
    }

    #[test]
    fn an_open_of_an_unusable_config_refuses_before_any_window_work() {
        // No window title: the opening phase names it before the OCR models are even loaded.
        let config = config_with_rows(1);
        let plan = plan(&config, Path::new("C:/root")).expect("valid fixture");
        let mut config_without_title = config;
        config_without_title["window"] = serde_json::json!({});

        let line = match PetReadSource::open(
            &config_without_title,
            &plan,
            &AttributesConfig::default(),
            Some(2),
            Path::new("C:/root"),
        ) {
            Ok(_) => panic!("a missing title must refuse the opening phase"),
            Err(line) => line,
        };

        assert_eq!(line, "window.title is not configured");
    }
}
#[cfg(test)]
mod loop_tests {
    use super::*;
    use std::cell::Cell;

    use seal_tools::cancel::CancelToken;
    use seal_tools::clock::Clock;
    use seal_tools::runner::{EffectError, Effects, STOPPED_MESSAGE, run};
    use seal_tools::tool::{StateCell, ToolSnapshot};

    fn png(width: u32, height: u32, bgra: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD
            .encode(seal_win::pure::encode_png(width, height, bgra).expect("encodes"))
    }

    fn empty_row(index: usize) -> Value {
        serde_json::json!({
            "enabled": true,
            "toggle_label": [5, 5 + index as i64, 40, 20],
            "boarding_pet_slot": [50, 60 + 40 * index as i64, 30, 40],
            "feeder_strip": [100, 200 + 40 * index as i64, 300, 50],
            "stacks": 1,
            "pet_slot_empty_png": png(2, 2, &[0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255]),
            "boarding_running": false,
        })
    }

    /// A valid one-row config; `drag` picks the food-load mode and `return_slot`/`queue` choose how a
    /// pet is found.
    fn config(rows: usize, drag: bool, return_slot: bool, queue: bool) -> Value {
        let slots: Vec<Value> = (0..rows).map(empty_row).collect();
        let food_slots = serde_json::json!([[0, 0], [0, 1], [0, 2]]);
        let queue = if queue {
            serde_json::json!([{"label": "pet a", "species": "S", "png": png(4, 4, &[0, 0, 255, 255].repeat(16))}])
        } else {
            serde_json::json!([])
        };
        serde_json::json!({
            "window": {"title": "TW_LIVE"},
            "buy_sell": {"max_button": [900, 500]},
            "pet": {
                "menu_button": [10, 10],
                "feed_icon": [20, 20],
                "close_button": [30, 30],
                "page_tabs": [[100, 100]],
                "bag_grid": [0, 0, 400, 400],
                "bag_slot": [0, 0, 40, 40],
                "food_slots": food_slots,
                "food_load_mode": if drag { "drag" } else { "right_click" },
                "max_button": [900, 500],
                "items_per_minute": 3,
                "pet_slot_occupied_above": 0.05,
                "slots": slots,
                "queue": queue,
                "return_slot": if return_slot { serde_json::json!([0, 1]) } else { Value::Null }
            }
        })
    }

    fn plan_of(config: &Value) -> PetPlan {
        plan(config, Path::new("C:/root")).expect("a valid fixture")
    }

    fn populated() -> SlotState {
        SlotState {
            difference: Some(0.65),
            empty: Some(false),
        }
    }

    fn empty() -> SlotState {
        SlotState {
            difference: Some(0.0),
            empty: Some(true),
        }
    }

    /// A scripted read seam: slot states pop in order (last repeats), lines/counts/hover are fixed.
    struct FakeSource {
        slots: VecDeque<Result<SlotState, String>>,
        last: Result<SlotState, String>,
        lines: Vec<String>,
        counts: FeederCounts,
        hover: Vec<String>,
        bag: PetImage,
        captures: VecDeque<PetImage>,
    }

    impl FakeSource {
        fn new(slots: Vec<SlotState>) -> Self {
            let mut queue = VecDeque::new();
            for slot in slots {
                queue.push_back(Ok(slot));
            }
            Self {
                last: queue.back().cloned().unwrap_or(Ok(empty())),
                slots: queue,
                lines: vec![
                    "代養完成預計所需時間：約14分".to_owned(),
                    "每1分 攝取3個".to_owned(),
                ],
                counts: FeederCounts {
                    counts: vec![Some(120), Some(300)],
                    digits_read: true,
                },
                hover: vec!["(6階) +7 [52.18%]".to_owned()],
                bag: PetImage {
                    width: 40,
                    height: 40,
                    rgba: vec![0u8; 40 * 40 * 4],
                },
                captures: VecDeque::new(),
            }
        }
    }

    impl PetSource for FakeSource {
        fn row_lines(&mut self, _row: &PetRowPlan) -> Result<Vec<String>, String> {
            Ok(self.lines.clone())
        }
        fn feeder_counts(&mut self, _row: &PetRowPlan) -> Result<FeederCounts, String> {
            Ok(self.counts.clone())
        }
        fn hover_panel(&mut self, _at: Point) -> Result<Vec<String>, String> {
            Ok(self.hover.clone())
        }
        fn slot_state(&mut self, _row: &PetRowPlan) -> Result<SlotState, String> {
            self.slots.pop_front().unwrap_or_else(|| self.last.clone())
        }
        fn client_capture(&mut self) -> Result<PetImage, String> {
            Ok(self.bag.clone())
        }
        fn region_capture(&mut self, _region: Rect) -> Result<PetImage, String> {
            Ok(self.captures.pop_front().unwrap_or_else(|| PetImage {
                width: 2,
                height: 2,
                rgba: vec![0u8; 16],
            }))
        }
        fn cursor(&mut self) -> Option<Point> {
            Some(Point::new(0, 0))
        }
        fn game_in_front(&mut self) -> bool {
            true
        }
    }

    /// Run until the tool schedules its next visit (a long wait), recording each decide's status.
    fn run_to_wait(tool: &mut PetTool<FakeSource>) -> (Vec<Decision>, Vec<String>) {
        let mut decisions = Vec::new();
        let mut messages = Vec::new();
        for _ in 0..2_000 {
            let status = tool.status();
            if status.message != messages.last().cloned().unwrap_or_default() {
                messages.push(status.message.clone());
            }
            let decision = tool.decide();
            let after = tool.status();
            if after.message != messages.last().cloned().unwrap_or_default() {
                messages.push(after.message);
            }
            let terminal = decision.is_terminal();
            let long_wait = matches!(decision, Decision::Wait(seconds) if seconds > 60.0);
            decisions.push(decision);
            if terminal || long_wait {
                return (decisions, messages);
            }
        }
        panic!("the tool never reached a scheduled wait: {decisions:?}");
    }

    fn a_toggle_click() -> Decision {
        Decision::Click(Point::new(25, 15))
    }

    #[test]
    fn a_row_that_boards_and_feeds_reloads_then_schedules_its_next() {
        let config = config(1, false, true, false);
        let plan = plan_of(&config);
        let mut source = FakeSource::new(vec![empty(), empty(), populated(), populated()]);
        source.lines = vec![
            "代養完成預計所需時間：約14分".to_owned(),
            "每1分 攝取3個".to_owned(),
        ];
        let mut tool = PetTool::new(plan, source, Arc::new(seal_tools::game::AllowAll));

        let (decisions, messages) = run_to_wait(&mut tool);

        assert!(
            messages.iter().any(|message| message.contains("reloading")),
            "step messages reach the card: {messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("placing the pet")),
            "{messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("loading the food")),
            "{messages:?}"
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("starting boarding")),
            "{messages:?}"
        );
        // The pet came out of the return slot and two stacks were loaded: MAX + one Enter per stack.
        assert!(
            decisions.contains(&a_toggle_click()),
            "the start toggle is pressed"
        );
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == Decision::Enter)
                .count(),
            1,
            "one Enter per stack (the dialog has no confirmation): {decisions:?}"
        );
        match decisions.last() {
            Some(Decision::Wait(seconds)) => assert!(*seconds > 60.0, "{seconds}"),
            other => panic!("expected the scheduled wait, got {other:?}"),
        }
        let card = tool.card();
        assert_eq!(card.rows[0].status, "boarding");
        assert!(
            card.rows[0]
                .next_minutes
                .is_some_and(|minutes| minutes > 10.0),
            "{card:?}"
        );
    }

    #[test]
    fn nothing_matched_in_the_bag_is_a_wait_not_a_failure() {
        // A queue icon exists but the bag image holds none of it: the scan finds nothing, and the row
        // is parked for the longer no-pet retry rather than counting a strike.
        let config = config(1, false, false, true);
        let plan = plan_of(&config);
        let mut source = FakeSource::new(vec![empty(), empty()]);
        source.bag = PetImage {
            width: 40,
            height: 40,
            rgba: vec![0u8; 40 * 40 * 4],
        };
        let mut tool = PetTool::new(plan, source, Arc::new(seal_tools::game::AllowAll));

        let (decisions, _messages) = run_to_wait(&mut tool);

        assert!(
            !decisions.contains(&Decision::RightClickHere),
            "nothing was right-clicked: {decisions:?}"
        );
        let card = tool.card();
        assert!(
            card.rows[0]
                .next_minutes
                .is_some_and(|minutes| minutes >= 29.0),
            "{card:?}"
        );
    }

    #[test]
    fn the_food_drag_holds_and_releases_exactly_once_in_order() {
        let config = config(1, true, true, false);
        let plan = plan_of(&config);
        let mut source = FakeSource::new(vec![empty(), empty(), populated(), populated()]);
        // Before/after the drop differ: the stack left the cell.
        source.captures.push_back(PetImage {
            width: 4,
            height: 4,
            rgba: vec![0u8; 64],
        });
        source.captures.push_back(PetImage {
            width: 4,
            height: 4,
            rgba: vec![255u8; 64],
        });
        let mut tool = PetTool::new(plan, source, Arc::new(seal_tools::game::AllowAll));

        let (decisions, messages) = run_to_wait(&mut tool);

        let hold = decisions
            .iter()
            .position(|decision| *decision == Decision::HoldLeft)
            .expect("a drag presses the button");
        let release = decisions
            .iter()
            .position(|decision| *decision == Decision::ReleaseLeft)
            .expect("a drag releases the button");
        assert!(hold < release, "press before release: {decisions:?}");
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == Decision::HoldLeft)
                .count(),
            1
        );
        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == Decision::ReleaseLeft)
                .count(),
            1
        );
        assert!(
            messages
                .iter()
                .any(|message| message.contains("loading the food")),
            "{messages:?}"
        );
    }

    #[test]
    fn four_boarding_rows_publish_four_rows_with_their_next_times() {
        let config = config(4, false, true, false);
        let plan = plan_of(&config);
        let mut source = FakeSource::new(vec![populated(), populated(), populated(), populated()]);
        source.lines = vec![
            "到2為止預計所需時間：約 20分".to_owned(),
            "每1分 攝取3個".to_owned(),
        ];
        let mut tool = PetTool::new(plan, source, Arc::new(seal_tools::game::AllowAll));

        let (_decisions, _messages) = run_to_wait(&mut tool);

        let status = tool.status();
        assert_eq!(status.rows.len(), 4);
        for row in &status.rows {
            assert_eq!(row.status, "boarding", "{row:?}");
            assert!(
                row.next_action_in_seconds
                    .is_some_and(|seconds| seconds > 60.0),
                "{row:?}"
            );
        }
        assert!(
            status.message.contains("boarding Row 1"),
            "{}",
            status.message
        );
    }

    #[derive(Default)]
    struct RecordingEffects {
        held: bool,
    }

    impl Effects for RecordingEffects {
        fn click(&mut self, _at: Point) -> Result<(), EffectError> {
            Ok(())
        }
        fn key(&mut self, _key: seal_domain::decision::Key) -> Result<(), EffectError> {
            Ok(())
        }
        fn enter(&mut self) -> Result<(), EffectError> {
            Ok(())
        }
        fn place(&mut self, _at: Point) -> Result<(), EffectError> {
            Ok(())
        }
        fn click_here(&mut self) -> Result<(), EffectError> {
            Ok(())
        }
        fn right_click_here(&mut self) -> Result<(), EffectError> {
            Ok(())
        }
        fn move_by(&mut self, _dx: i32, _dy: i32) -> Result<(), EffectError> {
            Ok(())
        }
        fn hold_left(&mut self) -> Result<(), EffectError> {
            self.held = true;
            Ok(())
        }
        fn release_left(&mut self) -> Result<(), EffectError> {
            self.held = false;
            Ok(())
        }
    }

    /// A clock that reports every sleep and cancels the run once enough time has passed — the long
    /// scheduled wait is what the test wants to interrupt.
    struct CancelDuringLongWait {
        token: CancelToken,
        slept: Cell<f64>,
    }

    impl Clock for CancelDuringLongWait {
        fn sleep_seconds(&self, seconds: f64) {
            self.slept.set(self.slept.get() + seconds);
            if self.slept.get() > 60.0 {
                self.token.cancel();
            }
        }
    }

    #[test]
    fn a_stop_during_the_long_wait_ends_cancelled_and_leaves_no_button_held() {
        let config = config(1, true, true, false);
        let plan = plan_of(&config);
        let mut source = FakeSource::new(vec![empty(), empty(), populated(), populated()]);
        source.captures.push_back(PetImage {
            width: 4,
            height: 4,
            rgba: vec![0u8; 64],
        });
        source.captures.push_back(PetImage {
            width: 4,
            height: 4,
            rgba: vec![255u8; 64],
        });
        let mut tool = PetTool::new(plan, source, Arc::new(seal_tools::game::AllowAll));
        let token = CancelToken::new();
        let handle = seal_tools::arbiter::RunHandle {
            tool: ToolId::Pet,
            token: token.clone(),
            state: StateCell::new(ToolSnapshot::new(ToolId::Pet)),
        };
        let clock = CancelDuringLongWait {
            token: token.clone(),
            slept: Cell::new(0.0),
        };
        let mut effects = RecordingEffects::default();

        let reason = run(&mut tool, &handle, &clock, &mut effects);

        assert_eq!(reason.kind, seal_domain::decision::StopKind::Cancelled);
        assert_eq!(reason.message, STOPPED_MESSAGE);
        assert!(
            !effects.held,
            "the drag released before the wait, and nothing is left held"
        );
    }

    // ── the icon-match port ─────────────────────────────────────────────────────────────────

    #[test]
    fn the_icon_difference_ignores_the_border_and_resizes_the_cell() {
        let icon = PetImage {
            width: 16,
            height: 16,
            rgba: [200u8, 40, 20, 255].repeat(16 * 16),
        };
        let same = icon.clone();
        assert_eq!(
            differing_fraction_images(&icon, &same, ICON_INSET, ICON_TOLERANCE),
            Some(0.0)
        );

        let mut border_only = icon.clone();
        for y in 0..16 {
            for x in 0..16 {
                if x < ICON_INSET || y < ICON_INSET || x >= 16 - ICON_INSET || y >= 16 - ICON_INSET
                {
                    let index = (y * 16 + x) * 4;
                    border_only.rgba[index] = 0;
                }
            }
        }
        assert_eq!(
            differing_fraction_images(&icon, &border_only, ICON_INSET, ICON_TOLERANCE),
            Some(0.0)
        );

        // A cell crop larger than the icon is resized down to it, not refused.
        let larger = area_resize(&icon, 32, 32);
        assert!(
            differing_fraction_images(&icon, &larger, ICON_INSET, ICON_TOLERANCE)
                .is_some_and(|fraction| fraction < 0.05),
            "an area-resized copy reads as the same picture"
        );
    }

    #[test]
    fn score_cell_finds_an_exact_icon_at_the_centre_offset() {
        let icon = PetImage {
            width: 16,
            height: 16,
            rgba: [200u8, 40, 20, 255].repeat(16 * 16),
        };
        // pitch 6 with the ±3 search, so a 12x12 crop can host the whole search. The icon is placed
        // exactly where dx=dy=0 reads it.
        let mut crop = PetImage {
            width: 22,
            height: 22,
            rgba: vec![0u8; 22 * 22 * 4],
        };
        for y in 3..19 {
            for x in 3..19 {
                let source = ((y - 3) * 16 + (x - 3)) * 4;
                let target = (y * 22 + x) * 4;
                crop.rgba[target..target + 4].copy_from_slice(&icon.rgba[source..source + 4]);
            }
        }

        assert_eq!(score_cell(&crop, &icon, 6, 6), Some(0.0));
        // A crop too tight to host the search is refused, not scored at fewer offsets.
        let tight = PetImage {
            width: 12,
            height: 12,
            rgba: vec![0u8; 12 * 12 * 4],
        };
        assert_eq!(score_cell(&tight, &icon, 12, 12), None);
    }
    /// A scripted `/game` capability: records request/release calls and can hold the first request
    /// until a gate opens (the blocked-worker test).
    #[derive(Default)]
    struct RecordingPort {
        requests: std::sync::Mutex<Vec<ToolId>>,
        releases: std::sync::Mutex<Vec<ToolId>>,
        gate: std::sync::Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    }

    impl RecordingPort {
        fn requests(&self) -> Vec<ToolId> {
            self.requests.lock().expect("not poisoned").clone()
        }

        fn releases(&self) -> Vec<ToolId> {
            self.releases.lock().expect("not poisoned").clone()
        }
    }

    impl GamePort for RecordingPort {
        fn request(&self, tool: ToolId) -> Result<(), String> {
            self.requests.lock().expect("not poisoned").push(tool);
            if let Some(gate) = self.gate.lock().expect("not poisoned").take() {
                let _ = gate.recv();
            }
            Ok(())
        }

        fn release(&self, tool: ToolId) {
            self.releases.lock().expect("not poisoned").push(tool);
        }

        fn should_stand_down(&self) -> bool {
            false
        }
    }

    fn pet_with_port(
        config: &Value,
        slots: Vec<SlotState>,
        port: Arc<RecordingPort>,
    ) -> PetTool<FakeSource> {
        PetTool::new(plan_of(config), FakeSource::new(slots), port)
    }

    #[test]
    fn the_pet_requests_the_game_before_its_first_click_and_releases_before_the_wait() {
        let config = config(1, false, true, false);
        let port = Arc::new(RecordingPort::default());
        let mut tool = pet_with_port(
            &config,
            vec![empty(), empty(), populated(), populated()],
            Arc::clone(&port),
        );

        // The first decision can click (the 目錄 button), so the claim must already be held.
        assert_eq!(tool.decide(), Decision::Click(Point::new(10, 10)));
        assert_eq!(port.requests(), vec![ToolId::Pet]);

        let _ = run_to_wait(&mut tool);

        assert_eq!(
            port.releases(),
            vec![ToolId::Pet],
            "the game is given back before the long wait"
        );
    }

    #[test]
    fn dropping_a_pet_tool_mid_visit_releases_the_game() {
        let config = config(1, false, true, false);
        let port = Arc::new(RecordingPort::default());
        let mut tool = pet_with_port(
            &config,
            vec![empty(), empty(), populated(), populated()],
            Arc::clone(&port),
        );
        let _ = tool.decide();
        assert_eq!(port.requests(), vec![ToolId::Pet]);

        drop(tool);

        assert_eq!(
            port.releases(),
            vec![ToolId::Pet],
            "the Drop guard releases a claim that a stop interrupted"
        );
    }

    #[test]
    fn a_blocked_request_produces_no_decision_until_the_game_is_granted() {
        let (release, gate) = std::sync::mpsc::channel();
        let port = Arc::new(RecordingPort {
            gate: std::sync::Mutex::new(Some(gate)),
            ..RecordingPort::default()
        });
        let config = config(1, false, true, false);
        let mut tool = pet_with_port(
            &config,
            vec![empty(), empty(), populated(), populated()],
            Arc::clone(&port),
        );
        let (decision_sender, decisions) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let decision = tool.decide();
            decision_sender
                .send(decision)
                .expect("the test is listening");
        });

        assert!(
            decisions
                .recv_timeout(std::time::Duration::from_millis(150))
                .is_err(),
            "no decision may be produced while the request is blocked"
        );

        release.send(()).expect("the game is granted");
        assert_eq!(
            decisions
                .recv_timeout(std::time::Duration::from_secs(2))
                .expect("a decision after the grant"),
            Decision::Click(Point::new(10, 10))
        );
        worker.join().expect("the worker ends");
        assert_eq!(port.requests(), vec![ToolId::Pet]);
    }
}

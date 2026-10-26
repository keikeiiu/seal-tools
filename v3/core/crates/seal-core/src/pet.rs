//! The pet feeder — INCREMENT 1: the plan, the card surface, and the READ SEAM.
//!
//! The pet feeder is the resident tool and v2's most failure-prone loop (`v2/SealTools.Pet/PetTool.cs`
//! is 3,520 lines). This module is deliberately split the same way the other readers are:
//!
//! - **The decisions are `seal-domain`'s** ([`PetSetup::ready`], [`schedule_for`], [`fold_back`],
//!   [`build_card`], [`visit_opening_message`], [`reload_step_message`], [`food_load::complaint`] …).
//!   Nothing here re-implements one.
//! - **The I/O is a seam.** [`PetSource`] is the two dozen screen reads the loop will need; the
//!   production [`PetReadSource`] is `win::capture` + `seal-ocr` + `seal_win` over the regions the
//!   domain already specifies ([`FeederLayout::eta_region`]/[`slot_boxes`]/[`read_region`], the
//!   tooltip box from config). The tests script it: no window, no board, no OCR models.
//!
//! # What this increment is NOT
//!
//! The loop (schedule, reload sequence, four-row state machine, food-load drag) and the registry
//! flip are increment 2. `pet` therefore still answers with the "not available in this build yet"
//! line: no registry change here.
//!
//! # The reader is never allowed to guess
//!
//! A row's empty-slot crop is decoded once at plan time. A crop that is PRESENT but cannot be
//! decoded/based64-decoded is a named start refusal (a stale calibration is a loud failure, not a
//! silent "unknown" forever); a crop that is MISSING leaves [`PetRowPlan::empty_reference`] `None`,
//! and the domain's slot read then answers "unknown" — which every caller treats as "carry on", the
//! v2 rule ("a missing check must not stop a run that would otherwise work"). The required marks are
//! the domain's own `ready` lines, so the player gets the calibration sentence rather than a new one
//! invented here.
//!
//! # Deviation from v2, recorded
//!
//! v2's feeder-count read walks a 3×/9×/24× upscale ladder (its own comment calls the reader
//! knife-edge on those digits). `OcrPipeline::scan_rgba` has no upscale parameter, so this increment's
//! production read is ONE pass per slot at the pipeline's own detector scale, plus the v2 empty-slot
//! colour verdict when no number comes back. The ladder is a small follow-up (the contract is in
//! `FeederCount`/`v2 PetTool.CountReadUpscales`) and does not change the seam: the loop still receives
//! `Option<i32>` per slot.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use seal_domain::config::AttributesConfig;
use seal_domain::geometry::{Point, Rect};
use seal_domain::tools::describe;
use seal_domain::tools::pet::{FeederCount, PET_SLOT_NOT_THE_WINDOW_ABOVE};
use seal_domain::tools::pet::{
    FeederLayout, PetCard, PetRowSetup, PetRowView, PetSetup, build_card, food_load,
    visit_opening_message,
};
use seal_tools::tool::ToolRow;
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
        .map_err(|_| format!("{what} couldn't be decoded — re-capture it on the Pet tab."))?;
    let (width, height, rgba) = seal_win::pure::decode_png(&bytes)
        .map_err(|_| format!("{what} couldn't be decoded — re-capture it on the Pet tab."))?;
    Ok(ReferenceCrop {
        width: width as usize,
        height: height as usize,
        rgba,
    })
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
    ) -> Result<Self, String> {
        check_board(plan, firmware_level)?;
        let title = crate::win::configured_title(config).map_err(|error| error.line())?;
        let geometry = crate::dispatch::ocr_geometry(config);
        let pipeline =
            seal_ocr::pipeline::OcrPipeline::load_from_env(attributes, geometry.row_height)
                .map_err(|error| {
                    format!(
                        "the OCR models are not available ({}) — set SEAL_OCR_MODELS",
                        error.reason
                    )
                })?;
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

        assert!(line.contains("目錄 isn't calibrated"), "{line}");
        assert!(line.contains("Calibrate Pet"), "{line}");
    }

    #[test]
    fn every_row_unticked_is_refused_with_the_domains_line() {
        let mut config = config_with_rows(2);
        config["pet"]["slots"][0]["enabled"] = serde_json::json!(false);
        config["pet"]["slots"][1]["enabled"] = serde_json::json!(false);

        let line = plan(&config, Path::new("C:/root")).expect_err("nothing to feed");

        assert_eq!(
            line,
            "Every row is unticked — nothing to feed. Tick a row on the Pet tab."
        );
    }

    #[test]
    fn drag_configured_without_a_food_strip_is_refused_with_the_domains_line() {
        let mut config = config_with_rows(1);
        config["pet"]["food_load_mode"] = serde_json::json!("drag");
        config["pet"]["slots"][0]["feeder_strip"] = Value::Null;

        let line = plan(&config, Path::new("C:/root")).expect_err("drag needs the strip");

        assert!(line.contains("food strip isn't drawn"), "{line}");
        assert!(line.contains("Calibrate Pet"), "{line}");
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
        assert!(line.contains("re-capture it on the Pet tab"), "{line}");
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
        ) {
            Ok(_) => panic!("a missing title must refuse the opening phase"),
            Err(line) => line,
        };

        assert_eq!(line, "window.title is not configured");
    }
}

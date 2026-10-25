//! The `local.yaml` overlay: v2's `ConfigLoader.Local*` mirror classes, ported field for field.
//!
//! This is the machine's own file, so most of its fields are nullable by design: **a field the
//! overlay does not mention means "not stated"**, and the merge must leave the shipped value
//! standing (`ConfigLoader.ApplySettings`: "Every guard is 'is it PRESENT', never 'is it non-empty'").
//! That is why nearly every field here is an `Option` — a non-nullable field would make silence
//! indistinguishable from zero, which is how a partial write blanks a setting.
//!
//! The nested whole objects (`window`, `reference_window`, `hotkeys`, `models`, `timing`,
//! `grade_colors`, `filter`) keep the shape of the type they mirror instead of a nullable copy,
//! because v2 copies and applies those whole.
//!
//! A name in a quest or spammer map always has a body after the decode (see [`super::de`]); values
//! whose body was null load as empty/default bodies, and the key is KEPT — dropping it would rename
//! the player's flow.

use super::de::opt_map_with_default_bodies;
use super::quest::{QuestPreset, QuestSequence};
use super::shared::{
    BuyPreset, CalibrationInfo, GameVariant, HotkeysConfig, ModelsConfig, MovementsConfig,
    OcrGeometry, ReferenceWindowConfig, SpammerCombo, SpammerStartup, TimingConfig, TooltipConfig,
    WindowConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The top level of `local.yaml`: the blocks this app owns (`ConfigLoader.OwnedLocalBlocks`).
///
/// A top-level block that is NOT in this type survives a save verbatim in v2 (`ForeignBlocks`) — that
/// is the writer's job, in `seal-config`. On LOAD an unknown block is simply ignored, which serde
/// does by default.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalOverrides {
    #[serde(default)]
    pub calibration: Option<CalibrationInfo>,
    #[serde(default)]
    pub tuner: Option<LocalTuner>,
    #[serde(default)]
    pub gem: Option<LocalGem>,
    #[serde(default)]
    pub arduino: Option<LocalArduino>,
    #[serde(default)]
    pub ui: Option<LocalUi>,
    #[serde(default)]
    pub spammer: Option<LocalSpammer>,
    #[serde(default)]
    pub buy_sell: Option<LocalBuySell>,
    #[serde(default)]
    pub pet: Option<LocalPet>,
    #[serde(default)]
    pub tooltip: Option<TooltipConfig>,
    #[serde(default)]
    pub quest: Option<LocalQuest>,
    /// The PORTABLE settings, in the machine's own file. Mirrors the shipped template's sections and
    /// fields exactly — that is what the fold compares.
    #[serde(default)]
    pub settings: Option<LocalSettings>,
}

/// The machine-specific half of the tuner block: the calibrated OCR geometry and the 發條 button.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalTuner {
    #[serde(default)]
    pub ocr: Option<OcrGeometry>,
    /// `[x, y]` of the 發條 button, client-relative physical pixels. Only read in `hid` spring mode.
    #[serde(default)]
    pub spring_point: Option<Vec<i32>>,
}

/// The gem composer's calibrated geometry and the empty-result-box reference.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalGem {
    #[serde(default)]
    pub grade_positions: Option<BTreeMap<String, Vec<i32>>>,
    #[serde(default)]
    pub movements: Option<MovementsConfig>,
    #[serde(default)]
    pub result_gem_area: Option<Vec<i32>>,
    #[serde(default)]
    pub resource_gems: Option<Vec<Vec<i32>>>,
    /// Sampled empty-result-gem colour reference. The FALLBACK for the saved empty crop.
    #[serde(default)]
    pub empty_signature: Option<super::shared::ColorComposition>,
    #[serde(default)]
    pub empty_distance: Option<f64>,
}

/// Optional override for the board's COM port. Empty = find the board by its IDs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalArduino {
    #[serde(default)]
    pub port: Option<String>,
}

/// Windows-specific launcher state: where the window sits, how big it is when expanded, and whether
/// it floats above other windows. Machine-specific, so it lives here rather than in the portable
/// defaults — a second PC wants its own placement.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalUi {
    #[serde(default)]
    pub left: Option<f64>,
    #[serde(default)]
    pub top: Option<f64>,
    #[serde(default)]
    pub width: Option<f64>,
    /// Height to restore when the config region is expanded. The collapsed and mini heights are
    /// computed by the window, not stored.
    #[serde(default)]
    pub expanded_height: Option<f64>,
    #[serde(default)]
    pub pinned: bool,
}

/// The player's own spammer rotations. Personal, not machine-specific, but stored here because
/// `defaults.yaml` is the template that ships — a preset written there would be published with the
/// next release.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalSpammer {
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default, deserialize_with = "opt_map_with_default_bodies")]
    pub presets: Option<BTreeMap<String, BTreeMap<String, f64>>>,
    /// Preset name → the keys that outrank the rest, in order.
    #[serde(default, deserialize_with = "opt_map_with_default_bodies")]
    pub priority: Option<BTreeMap<String, Vec<String>>>,
    /// Preset name → that preset's combos, in the order they are tried.
    #[serde(default, deserialize_with = "opt_map_with_default_bodies")]
    pub combos: Option<BTreeMap<String, Vec<SpammerCombo>>>,
    /// Preset name → that preset's opening: lead-in and step.
    #[serde(default, deserialize_with = "opt_map_with_default_bodies")]
    pub startup: Option<BTreeMap<String, SpammerStartup>>,
    /// LEGACY, read on load only — the opening step as a bare map, before the lead-in existed.
    /// Folded into [`Self::startup`] during the merge and never written again.
    #[serde(default)]
    pub startup_step: Option<BTreeMap<String, f64>>,
}

/// The buy/sell tool's geometry and presets: machine-specific like the gem positions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalBuySell {
    /// Whole bag grid, client-relative physical `[x, y, w, h]`.
    #[serde(default)]
    pub bag_grid: Option<Vec<i32>>,
    /// One bag slot in the same space, kept as the uniformity check.
    #[serde(default)]
    pub bag_slot: Option<Vec<i32>>,
    /// Rows of the shop list visible at once.
    #[serde(default)]
    pub shop_rows: Option<i32>,
    /// The visible shop list region `[x, y, w, h]`.
    #[serde(default)]
    pub shop_region: Option<Vec<i32>>,
    /// Where the cursor parks so the wheel scrolls the list.
    #[serde(default)]
    pub scroll_point: Option<Vec<i32>>,
    /// MAX button centre in the count dialog.
    #[serde(default)]
    pub max_button: Option<Vec<i32>>,
    /// Named buy items. Personal, so they stay out of the shipped `defaults.yaml`.
    #[serde(default, deserialize_with = "opt_map_with_default_bodies")]
    pub presets: Option<BTreeMap<String, BuyPreset>>,
    /// Bag slots selected for selling (0 = top-left). Deliberately NOT persisted by the launcher —
    /// this field exists for compatibility with a file that has it.
    #[serde(default)]
    pub sell_slots: Option<Vec<i32>>,
    /// Hard per-run ceiling on slots sold. Selling is the one irreversible thing here.
    #[serde(default)]
    pub sell_cap: Option<i32>,
}

/// One breeding row, as it is written to `local.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalPetSlot {
    /// Missing means ON, so a file written before this existed keeps driving its rows.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// The 開始代養 / 結束代養 button as a box.
    #[serde(default)]
    pub toggle_label: Option<Vec<i32>>,
    /// The pet's slot in the boarding window.
    #[serde(default)]
    pub boarding_pet_slot: Option<Vec<i32>>,
    /// This row's food slots as ONE strip, divided by `stacks` at run time.
    #[serde(default)]
    pub feeder_strip: Option<Vec<i32>>,
    /// 2 on the free row, 5 on a paid one.
    #[serde(default)]
    pub stacks: Option<i32>,
    /// THIS ROW's pet slot as it looks EMPTY, cropped as a base64 PNG.
    #[serde(default)]
    pub pet_slot_empty_png: Option<String>,
    /// Whether boarding is running on THIS row right now.
    #[serde(default)]
    pub boarding_running: Option<bool>,
}

/// One queued pet, as it is written to `local.yaml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalPetQueueEntry {
    /// For the log and the UI only. NOTHING matches on it — the recogniser read 真蔚蓝尽凰 for
    /// 真蔚藍鳳凰 on a live read, and the crop is the identity.
    #[serde(default)]
    pub label: Option<String>,
    /// Where the icon was dragged on the calibration capture.
    #[serde(default)]
    pub rect: Option<Vec<i32>>,
    /// The icon itself, cropped at [`Self::rect`].
    #[serde(default)]
    pub png: Option<String>,
    /// Which pet LINE this is, for the feeding table.
    #[serde(default)]
    pub species: Option<String>,
}

/// The pet food auto-replacement geometry — machine-specific in full, including the bag grid, which
/// is NOT the same one the shop opens beside.
///
/// What is SHARED and what is PER ROW is the split that matters: the way in (目錄, the feed icon,
/// the X), the bag and its grid, the food pool and the queue are shared; the toggle, the boarding
/// slot and the stack count belong to a row and live in [`Self::slots`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalPet {
    #[serde(default)]
    pub menu_button: Option<Vec<i32>>,
    #[serde(default)]
    pub feed_icon: Option<Vec<i32>>,
    #[serde(default)]
    pub close_button: Option<Vec<i32>>,
    /// A crop of the boarding X as it looks with the window OPEN — session state the RUN learns,
    /// not something the calibrator draws.
    #[serde(default)]
    pub close_button_png: Option<String>,
    /// Whether the window may still be on screen. Gate for the already-open probe.
    #[serde(default)]
    pub boarding_window_may_be_open: Option<bool>,
    /// The bag's ITEM1 / ITEM2 / ITEM3 tab points, in page order.
    #[serde(default)]
    pub page_tabs: Option<Vec<Vec<i32>>>,
    #[serde(default)]
    pub bag_grid: Option<Vec<i32>>,
    #[serde(default)]
    pub bag_slot: Option<Vec<i32>>,
    /// The bag cells holding pet food, each as `[page, cell]`.
    #[serde(default)]
    pub food_slots: Option<Vec<Vec<i32>>>,
    /// How many of [`Self::food_slots`] have been used up.
    #[serde(default)]
    pub food_slots_used: Option<i32>,
    /// A crop of ONE food item, as base64 PNG — the reference the food cells are found with.
    #[serde(default)]
    pub food_icon_png: Option<String>,
    #[serde(default)]
    pub action_wait_ms: Option<i32>,
    #[serde(default)]
    pub wait_after_empty_minutes: Option<i32>,
    /// `right_click` | `drag` — how a food stack gets into the boarding window.
    #[serde(default)]
    pub food_load_mode: Option<String>,
    /// How far across each food slot the count is read from.
    #[serde(default)]
    pub feeder_count_left_fraction: Option<f64>,
    /// How confident the reader must be for a line to count as the count.
    #[serde(default)]
    pub feeder_count_min_score: Option<f64>,
    #[serde(default)]
    pub reload_on_start: Option<bool>,
    /// The bag cell a returned pet lands in, as `[page, cell]`.
    #[serde(default)]
    pub return_slot: Option<Vec<i32>>,
    /// Optional override for the count dialog's MAX (normally [`LocalBuySell::max_button`] is used:
    /// the boarding count dialog is the same dialog).
    #[serde(default)]
    pub max_button: Option<Vec<i32>>,
    #[serde(default)]
    pub slots: Option<Vec<LocalPetSlot>>,
    #[serde(default)]
    pub queue: Option<Vec<LocalPetQueueEntry>>,
}

/// The quest hand-in flows in `local.yaml`. The map VALUES are the config types themselves
/// ([`QuestPreset`] / [`QuestSequence`]), not mirrors — that is the same choice `LocalBuySell` makes
/// for [`BuyPreset`], and it leaves no hand-written field list to forget.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalQuest {
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default, deserialize_with = "opt_map_with_default_bodies")]
    pub presets: Option<BTreeMap<String, QuestPreset>>,
    /// Sequences, carried the same way: personal, so they live in local.yaml and not in the file a
    /// public build ships.
    #[serde(default)]
    pub active_sequence: Option<String>,
    #[serde(default, deserialize_with = "opt_map_with_default_bodies")]
    pub sequences: Option<BTreeMap<String, QuestSequence>>,
    /// The mouse-drift guard, in the units the process reads the cursor in.
    #[serde(default)]
    pub guard_px: Option<i32>,
}

/// The PORTABLE settings, in the machine's own file (`ConfigLoader.LocalSettings`).
///
/// Every section is nullable and so is every VALUE inside them, and that is load-bearing rather than
/// defensive: a block the fold writes is partial by design — it carries only the values the player
/// changed — so a field the block does not mention has to mean "not stated" and leave the shipped
/// value standing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalSettings {
    #[serde(default)]
    pub window: Option<WindowConfig>,
    #[serde(default)]
    pub reference_window: Option<ReferenceWindowConfig>,
    #[serde(default)]
    pub game: Option<LocalSettingsGame>,
    #[serde(default)]
    pub arduino: Option<LocalSettingsArduino>,
    #[serde(default)]
    pub hotkeys: Option<HotkeysConfig>,
    #[serde(default)]
    pub tuner: Option<LocalSettingsTuner>,
    #[serde(default)]
    pub gem: Option<LocalSettingsGem>,
    #[serde(default)]
    pub pet: Option<LocalSettingsPet>,
    /// The once-only fold's marker. A MODELLED field rather than a comment or a bare key, because
    /// every writer re-serialises from the object graph and would strip anything it does not know
    /// about — the marker is what makes "fold once" true, so it has to survive every save.
    #[serde(default)]
    pub legacy_folded: Option<bool>,
}

/// `game`, without `GameConfig`'s computed `active()` — that property is a reference INTO
/// `variants`, and serialising the graph emits the variant twice, the second time as a YAML alias.
/// The mirror names the variant and the table, and never the resolved object.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalSettingsGame {
    /// Nullable so that "not stated" and "deliberately empty" stay different: an empty variant is a
    /// real choice — it means the plain `window.title` is the target.
    #[serde(default)]
    pub variant: Option<String>,
    /// Merged per name rather than replaced, so a client a later release adds still arrives.
    #[serde(default, deserialize_with = "opt_map_with_default_bodies")]
    pub variants: Option<BTreeMap<String, GameVariant>>,
}

/// `arduino`, without the port: that one is this machine's and lives in [`LocalArduino`], which is
/// why the projection writes the portable half with no port.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalSettingsArduino {
    #[serde(default)]
    pub vid: Option<i32>,
    #[serde(default)]
    pub pid: Option<Vec<i32>>,
    #[serde(default)]
    pub baud: Option<i32>,
}

/// The portable tuner fields. The calibrated geometry (`ocr`, `spring_point`) is machine state and
/// stays in [`LocalTuner`], so the two blocks never name the same field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalSettingsTuner {
    #[serde(default)]
    pub grade_order: Option<Vec<String>>,
    #[serde(default)]
    pub target_grade: Option<String>,
    #[serde(default)]
    pub max_retries: Option<i32>,
    #[serde(default)]
    pub save_captures: Option<bool>,
    #[serde(default)]
    pub ocr_retries: Option<i32>,
    /// `manual` (you position the mouse on the 發條 button yourself) or `hid` (the Arduino places it
    /// closed-loop and the guard keeps it there).
    #[serde(default)]
    pub spring_mode: Option<String>,
    /// `off` | `stop` | `recenter`, `hid` spring mode only.
    #[serde(default)]
    pub mouse_guard: Option<String>,
    /// How far (px) the cursor may sit from the spring point before the guard acts.
    #[serde(default)]
    pub guard_px: Option<i32>,
    /// Runaway guard for `recenter`.
    #[serde(default)]
    pub recenter_max: Option<i32>,
    #[serde(default)]
    pub models: Option<ModelsConfig>,
    #[serde(default)]
    pub timing: Option<TimingConfig>,
    #[serde(default)]
    pub grade_colors: Option<super::shared::GradeColorsConfig>,
    #[serde(default)]
    pub filter: Option<super::shared::FilterConfig>,
}

/// The portable gem fields. The calibrated points, the move counts and the empty-slot signature are
/// machine state and stay in [`LocalGem`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalSettingsGem {
    #[serde(default)]
    pub grades: Option<Vec<String>>,
    #[serde(default)]
    pub start_grade: Option<String>,
    /// `stop` | `advance_grade` | `advance_grade_clear`.
    #[serde(default)]
    pub empty_mode: Option<String>,
    /// Consecutive empty reads before the `empty_mode` action fires.
    #[serde(default)]
    pub empty_streak: Option<i32>,
    /// Channel max-min gap above which a pixel counts as "coloured".
    #[serde(default)]
    pub colored_gap_min: Option<i32>,
    #[serde(default)]
    pub save_empty_captures: Option<bool>,
    /// `tuned` | `arduino` — which move set the composer uses between calibrated points.
    #[serde(default)]
    pub move_mode: Option<String>,
}

/// The two pet settings the feeder reads which are true on every machine.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalSettingsPet {
    /// The FALLBACK feeding rate, used only when the boarding window's own rate line could not be
    /// read.
    #[serde(default)]
    pub items_per_minute: Option<i32>,
    /// How unlike the empty reference a slot must look before it counts as occupied.
    #[serde(default)]
    pub pet_slot_occupied_above: Option<f64>,
}

//! Config value types shared between the local overlay and the merged config, ported from
//! `v2/SealTools.Core/Config/AppConfig.cs`.
//!
//! Null-tolerance policy (see [`crate::config::de`]): a field whose v2 property is a REFERENCE type
//! tolerates a key with no value — a bare `title:` is `""`, a bare `rules:` is `[]` — because
//! YamlDotNet hands those reads a null and v2 either treats it as nothing or crashes on it later.
//! A field whose v2 property is a VALUE type (`int`, `double`, `bool`) is left strict: a null there
//! fails v2's load too, and a silent zero would be a setting that means something different from
//! absent.
//!
//! The defaults are v2's property initialisers, ported so a file that omits a key gets the value the
//! C# object starts with rather than `0`. The shipped template (`config/defaults.yaml`) carries the
//! same values; the initialisers only decide what happens to a file that predates a key.

use crate::config::de::null_as_default;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_hover_delay_ms() -> i32 {
    1200
}

fn default_match_mode() -> String {
    "any".to_owned()
}

fn default_one() -> i32 {
    1
}

fn default_gap() -> f64 {
    0.8
}

fn default_attributes() -> String {
    "attributes.yaml".to_owned()
}

/// The game window to drive. `window.title` is a SUBSTRING match (`WindowFinder.FindByTitle` uses
/// `Contains`); when `game.variant` is set it overrides this at load.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowConfig {
    #[serde(default, deserialize_with = "null_as_default")]
    pub title: String,
}

/// The client size a calibration was measured against — informational, and compared by the Setup
/// tab's mismatch line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReferenceWindowConfig {
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub height: i32,
}

/// One game client: the window to find, and the dictionary that client's text needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameVariant {
    /// A **substring** of the client's window title.
    #[serde(default, deserialize_with = "null_as_default")]
    pub title: String,

    /// The dictionary file, in `config/`. A missing one fails loudly on load rather than matching
    /// nothing — an empty dictionary reads every attribute as no attribute, silently, which looks
    /// exactly like "the game changed".
    #[serde(default = "default_attributes", deserialize_with = "null_as_default")]
    pub attributes: String,
}

impl Default for GameVariant {
    fn default() -> Self {
        Self {
            title: String::new(),
            attributes: default_attributes(),
        }
    }
}

/// Which game client this install is pointed at (`PLAN-US-CLIENT.md`).
///
/// The TW and US clients are the same game with different TEXT, and the text is not decoration
/// here — it is what the suite MATCHES: the attribute dictionary, the per-level phrases, the
/// feeder's time line. So a variant names **the words to match against**, which is why this is one
/// choice rather than a list of acceptable titles.
///
/// **RESOLVED AT LOAD.** The loader sets `window.title` from the active variant, so the single value
/// every tool already reads stays the single value they read — no call site has to know a variant
/// exists. Unset or unknown changes NOTHING: a file written before variants existed carries only
/// `window.title`, and an unrecognised name must not blank the target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameConfig {
    /// Which variant is active. **Empty means "not configured"**, and then `window.title` is used as
    /// it is.
    #[serde(default, deserialize_with = "null_as_default")]
    pub variant: String,

    /// The known clients. Adding one is a block here (or in `defaults.yaml`) and a dictionary file
    /// in `config/`. Missing → the two the C# initialiser carries; the shipped template spells out
    /// the same two.
    #[serde(default = "shipped_variants")]
    pub variants: BTreeMap<String, GameVariant>,
}

/// The two clients v2 ships: `GameConfig`'s own initialiser, which is what a file that omits
/// `variants:` falls back to.
fn shipped_variants() -> BTreeMap<String, GameVariant> {
    let mut variants = BTreeMap::new();
    variants.insert(
        "tw".to_owned(),
        GameVariant {
            title: "TW_LIVE".to_owned(),
            attributes: "attributes.yaml".to_owned(),
        },
    );
    variants.insert(
        "us".to_owned(),
        GameVariant {
            title: "US_LIVE".to_owned(),
            attributes: "attributes.us.yaml".to_owned(),
        },
    );
    variants
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            variant: String::new(),
            variants: shipped_variants(),
        }
    }
}

impl GameConfig {
    /// The client the active name points at, or `None` when it is unset or names something unknown —
    /// in which case the configured `window.title` stands, which is the safe direction: an
    /// unrecognised name must not blank the target.
    pub fn active(&self) -> Option<&GameVariant> {
        self.variant_named(&self.variant)
    }

    /// Looks a client up by NAME, trimmed, ignoring case.
    ///
    /// The lookup ignores case even though the map is keyed case-sensitively, because v2's dictionary
    /// is `StringComparer.OrdinalIgnoreCase` and a hand-typed `TW` has to work. (A *loaded* v2 config
    /// loses that comparer — YamlDotNet builds a case-sensitive dictionary — but the v2 test that
    /// pins the rule builds the initialiser and expects `TW` to resolve, so case-insensitivity is the
    /// intended behaviour and the port keeps it.) Variant names are ASCII today (`tw`, `us`), which is
    /// why the ASCII fold is enough here.
    pub fn variant_named(&self, name: &str) -> Option<&GameVariant> {
        let wanted = name.trim();
        if wanted.is_empty() {
            return None;
        }

        self.variants.get(wanted).or_else(|| {
            self.variants
                .iter()
                .find(|(variant, _)| variant.eq_ignore_ascii_case(wanted))
                .map(|(_, variant)| variant)
        })
    }
}

/// The display environment a calibration was measured in — written by the calibrators, read by the
/// Setup tab. All sizes are PHYSICAL pixels. Storing the scale makes the coordinates portable:
/// another machine can tell it must convert or recalibrate (`COORDINATES.md`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CalibrationInfo {
    /// Physical px per logical px at calibration time (e.g. 1.5).
    #[serde(default)]
    pub dpi_scale: Option<f64>,
    /// Effective monitor DPI at calibration time (`96 * dpi_scale`, e.g. 144).
    #[serde(default)]
    pub monitor_dpi: Option<u32>,
    /// Primary screen size in physical pixels `[width, height]`.
    #[serde(default)]
    pub screen: Option<Vec<i32>>,
    /// Game client size in physical pixels `[width, height]`.
    #[serde(default)]
    pub client_size: Option<Vec<i32>>,
    /// When the calibration was saved (informational).
    #[serde(default)]
    pub measured_at: Option<String>,
}

/// The four global hotkeys, as virtual-key codes (`GetAsyncKeyState`). Read by `ToolBase.SleepCheck`
/// and by the launcher; validated when they are TYPED, by the authoring rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HotkeysConfig {
    #[serde(default)]
    pub start: i32,
    #[serde(default)]
    pub quit: i32,
    #[serde(default)]
    pub advance_grade: i32,
    #[serde(default)]
    pub pause: i32,
}

/// Where the game's hover panel sits relative to the cursor.
///
/// A capability, not a setting of any one tool: the panel is anchored to the pointer, so its position
/// relative to the pointer is the same everywhere in the game even though its SIZE varies by what is
/// under it. Calibrating that offset once serves every hover-read this suite ever does.
///
/// All four values are client-relative PHYSICAL pixels, like every other calibrated coordinate here,
/// which is why they belong in local.yaml. [`Self::is_set`] is the gate: an offset alone means
/// nothing without a size to read, and a size without an offset would read the wrong place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TooltipConfig {
    /// Panel's left edge relative to the cursor. Negative when the panel sits left of the pointer,
    /// which is what happens near a screen edge if the game flips it.
    #[serde(default)]
    pub offset_x: i32,
    /// Panel's top edge relative to the cursor.
    #[serde(default)]
    pub offset_y: i32,
    /// The captured box's size — deliberately for the LARGEST panel rather than one per item type,
    /// so the offset stays universal.
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub height: i32,
    /// How long the panel takes to appear after the cursor settles on an item. Too short and the
    /// capture finds an empty region, which reads exactly like "nothing here".
    #[serde(default = "default_hover_delay_ms")]
    pub hover_delay_ms: i32,
}

impl Default for TooltipConfig {
    fn default() -> Self {
        Self {
            offset_x: 0,
            offset_y: 0,
            width: 0,
            height: 0,
            hover_delay_ms: default_hover_delay_ms(),
        }
    }
}

impl TooltipConfig {
    /// True once a box has been dragged.
    pub fn is_set(&self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// Colour-composition fingerprint of a region, used to tell whether the gem composer's RESULT box is
/// empty. The EMPTY reference is sampled live during gem calibration and runtime frames are compared
/// against it; the weaker fallback when the saved empty crop is missing.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ColorComposition {
    /// Mean value (brightness), 0..1.
    #[serde(default)]
    pub mean_v: f64,
    /// Mean saturation, 0..1.
    #[serde(default)]
    pub mean_sat: f64,
    /// Fraction of distinctly-coloured pixels, 0..1.
    #[serde(default)]
    pub colored_fraction: f64,
    #[serde(default)]
    pub mean_r: f64,
    #[serde(default)]
    pub mean_g: f64,
    #[serde(default)]
    pub mean_b: f64,
}

/// The three OCR model paths.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ModelsConfig {
    #[serde(default, deserialize_with = "null_as_default")]
    pub detector: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub recognizer: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub classifier: String,
}

/// The tuner's two per-cycle waits.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct TimingConfig {
    #[serde(default)]
    pub click_enter_delay: f64,
    #[serde(default)]
    pub ocr_delay: f64,
}

/// A channel max-min gap mask: "this pixel is yellow/blue/red/purple/white".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct YellowMaskConfig {
    #[serde(default)]
    pub rg_gap: i32,
    #[serde(default)]
    pub gb_gap: i32,
    #[serde(default)]
    pub r_min: i32,
    #[serde(default)]
    pub g_min: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BlueMaskConfig {
    #[serde(default)]
    pub bg_gap: i32,
    #[serde(default)]
    pub br_gap: i32,
    #[serde(default)]
    pub b_min: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RedMaskConfig {
    #[serde(default)]
    pub rg_gap: i32,
    #[serde(default)]
    pub rb_gap: i32,
    #[serde(default)]
    pub r_min: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PurpleMaskConfig {
    #[serde(default)]
    pub rb_gap: i32,
    #[serde(default)]
    pub bg_gap: i32,
    #[serde(default)]
    pub r_min: i32,
    #[serde(default)]
    pub b_min: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WhiteMaskConfig {
    #[serde(default)]
    pub r_min: i32,
    #[serde(default)]
    pub g_min: i32,
    #[serde(default)]
    pub b_min: i32,
}

/// Thresholds for turning the masks' pixel counts into a grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GradeDecisionConfig {
    #[serde(default)]
    pub total_min: i32,
    #[serde(default)]
    pub dg_yellow: i32,
    #[serde(default)]
    pub g_blue: i32,
    #[serde(default)]
    pub xg_red: i32,
    #[serde(default)]
    pub sg_purple: i32,
    #[serde(default)]
    pub n_white: i32,
    #[serde(default)]
    pub n_yellow_max: i32,
    #[serde(default)]
    pub n_blue_max: i32,
}

/// The tuner's grade-colour classifier, and the decision table built on it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GradeColorsConfig {
    #[serde(default)]
    pub yellow: Option<YellowMaskConfig>,
    #[serde(default)]
    pub blue: Option<BlueMaskConfig>,
    #[serde(default)]
    pub red: Option<RedMaskConfig>,
    #[serde(default)]
    pub purple: Option<PurpleMaskConfig>,
    #[serde(default)]
    pub white: Option<WhiteMaskConfig>,
    #[serde(default)]
    pub decision: Option<GradeDecisionConfig>,
}

/// One attribute rule for the tuner's filter: `name` appears `count` times, with `min`/`max` bounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterRule {
    #[serde(default, deserialize_with = "null_as_default")]
    pub name: String,
    #[serde(default = "default_one")]
    pub count: i32,
    #[serde(default)]
    pub min: Option<i32>,
    #[serde(default)]
    pub max: Option<i32>,
}

impl Default for FilterRule {
    fn default() -> Self {
        Self {
            name: String::new(),
            count: default_one(),
            min: None,
            max: None,
        }
    }
}

/// The tuner's keep/reroll filter. `match_mode` is one of `any`, `all`, `per_attr`; the values are
/// kept as strings so an unknown one loads and is REPORTED rather than failing the whole config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_match_mode", deserialize_with = "null_as_default")]
    pub match_mode: String,
    /// A grade the result must reach regardless of the rules; `None` means "no grade requirement".
    #[serde(default)]
    pub require_grade: Option<String>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub rules: Vec<FilterRule>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub override_rules: Vec<FilterRule>,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            match_mode: default_match_mode(),
            require_grade: None,
            rules: Vec::new(),
            override_rules: Vec::new(),
        }
    }
}

/// A rectangle in client-relative physical pixels, as `left/top/width/height` — the shape the OCR
/// region has in `local.yaml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RegionConfig {
    #[serde(default)]
    pub left: i32,
    #[serde(default)]
    pub top: i32,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub height: i32,
}

/// A box in region-relative pixels, as two corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BoxConfig {
    #[serde(default)]
    pub x1: i32,
    #[serde(default)]
    pub y1: i32,
    #[serde(default)]
    pub x2: i32,
    #[serde(default)]
    pub y2: i32,
}

/// The tuner's OCR geometry: the captured region, the sub-boxes within it, and the horizontal/vertical
/// bands the rows live in. Machine-specific, so it lives in `local.yaml`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct OcrGeometry {
    /// The captured window, client-relative physical. `None` when the key is present with no value —
    /// which the validator reports (v2 crashed on it before a window appeared).
    #[serde(default)]
    pub region: Option<RegionConfig>,
    /// The grade text's box within [`Self::region`].
    #[serde(default)]
    pub grade_area: Option<BoxConfig>,
    /// `[top, bottom]` of the grade row inside the region.
    #[serde(default, deserialize_with = "null_as_default")]
    pub grade_y: Vec<i32>,
    /// `[top, bottom]` of the attribute rows.
    #[serde(default, deserialize_with = "null_as_default")]
    pub attr_y: Vec<i32>,
    /// `[top, bottom]` of the remaining-time row.
    #[serde(default, deserialize_with = "null_as_default")]
    pub remaining_y: Vec<i32>,
    /// Optional `[left, right]` band for attributes; omitted entirely by older files.
    #[serde(default, deserialize_with = "null_as_default")]
    pub attr_x: Vec<i32>,
    /// Optional `[left, right]` band for the remaining time.
    #[serde(default, deserialize_with = "null_as_default")]
    pub remaining_x: Vec<i32>,
    #[serde(default)]
    pub row_height: i32,
}

/// The gem composer's hand-tuned HID movement counts — the "tuned" move set (`MOVE-SETS.md`). Raw
/// pointer counts, never scaled by DPI, which is why they are machine state.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MovementsConfig {
    #[serde(default, deserialize_with = "null_as_default")]
    pub radio_to_register: BTreeMap<String, Vec<i32>>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub register_combine: Vec<i32>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub combine_register: Vec<i32>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub grade_next: Vec<i32>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub grade_prev: Vec<i32>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub register_slot1: Vec<i32>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub slot1_slot2: Vec<i32>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub slot2_slot3: Vec<i32>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub slot3_to_n: Vec<i32>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub slot3_to_g: Vec<i32>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub slot3_to_dg: Vec<i32>,
}

/// A group of keys that must be cast IN ORDER, with a pause between the steps — the shape a game's
/// combo effect needs.
///
/// THE PAIR IS THE UNIT: a combo is ready only when EVERY key in it is off cooldown, so the spammer
/// never starts one it cannot finish. A key named in a combo is cast only as part of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpammerCombo {
    /// The keys, in cast order. One key is not a combo and is dropped — half a combo is worse than
    /// none.
    #[serde(default, deserialize_with = "null_as_default")]
    pub keys: Vec<String>,

    /// Seconds to hold between one step and the next. NOTHING else is pressed during the hold.
    #[serde(default = "default_gap")]
    pub gap: f64,
}

impl Default for SpammerCombo {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            gap: default_gap(),
        }
    }
}

/// How a spammer run's OPENING behaves, per preset: a buff rotation wants a lead-in to position in, a
/// combat one wants to start on the click.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct SpammerStartup {
    /// Seconds to wait after Start before pressing anything.
    #[serde(default)]
    pub lead_in: f64,
    /// Seconds between the OPENING presses. Zero makes every key due at once.
    #[serde(default)]
    pub step: f64,
}

/// One named thing to buy. Position is (scroll, then row), because the list scrolls and a row index
/// alone would mean nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyPreset {
    /// Wheel notches down from the top of the list before clicking. 0 = no scrolling.
    #[serde(default)]
    pub scroll: i32,
    /// Row index within the visible list after scrolling. 0 = the top row.
    #[serde(default)]
    pub row: i32,
    /// How many to buy per run.
    #[serde(default = "default_one")]
    pub count: i32,
}

impl Default for BuyPreset {
    fn default() -> Self {
        Self {
            scroll: 0,
            row: 0,
            count: default_one(),
        }
    }
}

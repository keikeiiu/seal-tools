//! Boot-time validation and the variant dictionary: the pure rules finally RUN before any command.
//!
//! Two of `PORT-COVERAGE.md`'s open items live here: **G4** (`ConfigValidator`, the ported rule set in
//! `seal_domain::config::validate`) was landed but never called, and **OPEN §3**'s "the dispatcher
//! does not call `load_attributes_named` yet". v2's `ConfigLoader.Load` ended with
//! `ConfigValidator.Validate(AppConfig)` (`ConfigLoader.cs:150`) — after the merge and after the
//! variant resolved the title, so the rules saw the values the app would run with — and a failure was
//! a `ConfigException` thrown before OCR or the COM port existed (`ConfigValidator.cs:9-11`).
//!
//! # What a failure does here
//!
//! PROTOCOL.md §4/§5 has **no shape for "the config is invalid"**: the events table's `core.stopping`
//! takes `eof|shutdown|fatal`, but emitting any event before the handshake would make the shell parse
//! an event while it is waiting for its `hello` response, and inventing a `hello` error code is a
//! protocol decision. So the defined behaviour is the one §1 already gives a core that cannot start:
//! it exits non-zero with the reason on **stderr**, and the shell shows its captured stderr
//! (§1 "Core crash"; the existing config-read failure path already does this). The v2 text is kept
//! verbatim — `ConfigErrors::message()` is `ConfigValidator.cs:65-68`'s `ConfigException` message — so
//! the line the player sees is the line v2 would have shown in its crash dialog. A protocol proposal
//! (a `hello` error code or a pre-handshake `log` event) is reported to the Lead rather than invented.
//!
//! # Mapping the merged JSON onto v2's typed graph
//!
//! v2 validated a deserialised `AppConfig`, and the presence rules matter: a section **absent** from
//! the file gets its field initialisers (so an absent `window:` fails `title`, an absent `arduino:`
//! fails the VID range, an absent `tuner:` has no OCR geometry), while a section **present with a
//! null body** (`window:`) is skipped by v2's reflection walk. `seal-config`'s merged JSON keeps that
//! distinction (`Value::Null` vs absent), and this module rebuilds exactly the `ValidationInput` the
//! rules expect.

use seal_config::ConfigError;
use seal_domain::config::{
    ArduinoInput, AttributesConfig, BoxConfig, FilterConfig, GemInput, OcrGeometry, RegionConfig,
    TunerInput, ValidationInput, WindowConfig, validate,
};
use serde_json::{Map, Value};

/// The dictionary a config with no (or an unknown) variant uses — `FirmwareVersion`-style constant
/// from the v2 default (`GameVariant.attributes`'s default is the TW dictionary).
pub const DEFAULT_DICTIONARY: &str = "attributes.yaml";

/// Why the core cannot start, or cannot reload what is on disk.
#[derive(Debug)]
pub enum StartupError {
    /// A file could not be read, parsed or written (`seal-config`'s error, already a v2-style line).
    Config(ConfigError),
    /// The ported validator refused the merged config; `message()` rebuilds v2's `ConfigException`.
    Invalid {
        /// Lines from `ConfigValidator`'s own list.
        lines: Vec<String>,
        /// Lines this module adds where v2 would have died deserialising (e.g. `tuner: "x"`).
        malformed: Vec<String>,
    },
    /// The active variant's attribute dictionary could not be loaded or decoded. This is v2's
    /// `ConfigException` out of `LoadAttributes` — a core that cannot load it must not run.
    Dictionary { line: String },
}

impl StartupError {
    /// The sentence(s) the player sees. For [`Self::Invalid`] this is v2's `ConfigException` text
    /// verbatim (`ConfigValidator.cs:65-68`), with this module's malformed-block lines first.
    pub fn message(&self) -> String {
        match self {
            Self::Config(error) => error.to_string(),
            Self::Dictionary { line } => line.clone(),
            Self::Invalid { lines, malformed } => {
                let mut all: Vec<&str> = malformed.iter().map(String::as_str).collect();
                all.extend(lines.iter().map(String::as_str));
                format!(
                    "Config validation failed:\n  - {}\nCheck defaults.yaml and local.yaml.",
                    all.join("\n  - ")
                )
            }
        }
    }

    /// The validator's player-facing lines for `detail.errors` (`PROTOCOL.md` §6 `config_invalid`).
    #[must_use]
    pub fn invalid_errors(&self) -> Option<Vec<String>> {
        match self {
            Self::Invalid { lines, malformed } => {
                let mut all = malformed.clone();
                all.extend(lines.iter().cloned());
                Some(all)
            }
            Self::Config(_) | Self::Dictionary { .. } => None,
        }
    }
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl From<ConfigError> for StartupError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

/// The active client: its variant name (when one resolved) and the dictionary file it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Active {
    /// `None` when unset or unknown: v2's load changes NOTHING in that case (`resolve_variant`).
    pub variant: Option<String>,
    /// A file name in `config/`, e.g. `attributes.us.yaml`.
    pub dictionary: String,
}

/// Resolves `game.variant` against `game.variants` and returns the dictionary the active client uses.
///
/// Case-insensitive, like the loader's title resolution. Unset or unknown falls back to
/// [`DEFAULT_DICTIONARY`] with no variant name, matching `resolve_variant`'s "an unrecognised name
/// must not blank the target".
pub fn active(config: &Value) -> Active {
    let default = Active {
        variant: None,
        dictionary: DEFAULT_DICTIONARY.to_owned(),
    };
    let Some(game) = config.get("game").and_then(Value::as_object) else {
        return default;
    };
    let Some(requested) = game
        .get("variant")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return default;
    };
    let Some(variants) = game.get("variants").and_then(Value::as_object) else {
        return default;
    };
    let Some((name, variant)) = variants
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(requested))
    else {
        return default;
    };
    let dictionary = variant
        .get("attributes")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|dictionary| !dictionary.is_empty())
        .unwrap_or(DEFAULT_DICTIONARY);
    Active {
        variant: Some(name.clone()),
        dictionary: dictionary.to_owned(),
    }
}

/// Loads one attribute dictionary (`config/<name>`, seeded from `<name>.example` when present) and
/// decodes it. The one place boot, reload and the apply's pre-write check get the dictionary from, so
/// the three cannot drift.
///
/// # Errors
///
/// Returns the startup refusal when the dictionary file cannot be read, parsed or decoded.
pub fn load_dictionary(
    store: &seal_config::ConfigStore,
    dictionary: &str,
) -> Result<AttributesConfig, StartupError> {
    let raw =
        store
            .load_attributes_named(dictionary)
            .map_err(|error| StartupError::Dictionary {
                line: error.to_string(),
            })?;
    serde_json::from_value(raw).map_err(|error| StartupError::Dictionary {
        line: format!("the attribute dictionary {dictionary} could not be read: {error}"),
    })
}

/// Runs the ported `ConfigValidator` rules over the merged config.
///
/// # Errors
///
/// Returns the startup refusal listing every malformed config field.
pub fn validate_config(config: &Value) -> Result<(), StartupError> {
    let (input, malformed) = validation_input(config);
    let errors = validate(&input);
    if malformed.is_empty() && errors.is_empty() {
        return Ok(());
    }
    Err(StartupError::Invalid {
        lines: errors.lines().to_vec(),
        malformed,
    })
}

/// Rebuilds v2's typed graph from the merged JSON, keeping v2's absent-vs-null distinction.
#[must_use]
pub fn validation_input(config: &Value) -> (ValidationInput, Vec<String>) {
    let mut malformed = Vec::new();
    let input = ValidationInput {
        window: window_input(config),
        arduino: arduino_input(config),
        tuner: tuner_input(config, &mut malformed),
        gem: gem_input(config),
    };
    (input, malformed)
}

fn section<'a>(config: &'a Value, name: &str) -> Presence<'a> {
    match config.get(name) {
        None => Presence::Absent,
        Some(Value::Null) => Presence::Null,
        Some(value) => Presence::Value(value),
    }
}

enum Presence<'a> {
    Absent,
    Null,
    Value(&'a Value),
}

// ── the sections v2 validates ───────────────────────────────────────────────────────────────

fn window_input(config: &Value) -> Option<WindowConfig> {
    match section(config, "window") {
        // Absent: v2's field initialiser gives an empty title, which `[Required]` refuses.
        Presence::Absent => Some(WindowConfig::default()),
        Presence::Null => None,
        Presence::Value(value) => Some(WindowConfig {
            title: value
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        }),
    }
}

fn arduino_input(config: &Value) -> Option<ArduinoInput> {
    match section(config, "arduino") {
        // Absent: v2's int default is 0, outside `[Range(1, int.MaxValue)]`.
        Presence::Absent => Some(ArduinoInput::default()),
        Presence::Null => None,
        Presence::Value(value) => Some(ArduinoInput {
            vid: value.get("vid").and_then(Value::as_i64).unwrap_or(0),
        }),
    }
}

fn tuner_input(config: &Value, malformed: &mut Vec<String>) -> Option<TunerInput> {
    match section(config, "tuner") {
        Presence::Absent => Some(TunerInput::default()),
        Presence::Null => None,
        Presence::Value(Value::Object(tuner)) => Some(TunerInput {
            spring_mode: field_string(tuner, "spring_mode", "manual"),
            mouse_guard: field_string(tuner, "mouse_guard", "off"),
            filter: match nested(tuner, "filter") {
                Nested::Absent => Some(FilterConfig::default()),
                Nested::Null => None,
                Nested::Object(filter) => Some(FilterConfig {
                    match_mode: filter
                        .get("match_mode")
                        .and_then(Value::as_str)
                        .unwrap_or("any")
                        .to_owned(),
                    ..FilterConfig::default()
                }),
            },
            ocr: match nested(tuner, "ocr") {
                Nested::Absent => Some(OcrGeometry::default()),
                Nested::Null => None,
                Nested::Object(ocr) => Some(ocr_geometry(ocr)),
            },
        }),
        Presence::Value(_) => {
            malformed.push("tuner: the section must be a mapping, or absent".to_owned());
            Some(TunerInput::default())
        }
    }
}

fn gem_input(config: &Value) -> Option<GemInput> {
    match section(config, "gem") {
        Presence::Value(Value::Object(gem)) => Some(GemInput {
            move_mode: field_string(gem, "move_mode", "arduino"),
            empty_mode: field_string(gem, "empty_mode", "stop"),
            colored_gap_min: gem
                .get("colored_gap_min")
                .and_then(Value::as_i64)
                .unwrap_or(40),
        }),
        Presence::Absent | Presence::Value(_) => Some(GemInput::default()),
        Presence::Null => None,
    }
}

// ── small presence-aware field reads ─────────────────────────────────────────────────────────

/// A string field with v2's field-initialiser default: absent → the default, present-null → `None`
/// (which the `[AllowedValues]` annotations accept, as every `DataAnnotation` except `Required` does).
fn field_string(section: &Map<String, Value>, name: &str, default: &str) -> Option<String> {
    match section.get(name) {
        None => Some(default.to_owned()),
        Some(Value::Null) => None,
        Some(value) => Some(value.as_str().unwrap_or(default).to_owned()),
    }
}

/// A nested object: absent → v2's field initialiser (a default object); present-null → skipped.
enum Nested<'a> {
    Absent,
    Null,
    Object(&'a Map<String, Value>),
}

fn nested<'a>(section: &'a Map<String, Value>, name: &str) -> Nested<'a> {
    match section.get(name) {
        Some(Value::Null) => Nested::Null,
        Some(Value::Object(object)) => Nested::Object(object),
        None | Some(_) => Nested::Absent,
    }
}

fn ocr_geometry(ocr: &Map<String, Value>) -> OcrGeometry {
    OcrGeometry {
        region: match nested(ocr, "region") {
            Nested::Absent => Some(RegionConfig::default()),
            Nested::Null => None,
            Nested::Object(region) => Some(RegionConfig {
                left: int_field(region, "left", 0),
                top: int_field(region, "top", 0),
                width: int_field(region, "width", 0),
                height: int_field(region, "height", 0),
            }),
        },
        grade_area: match nested(ocr, "grade_area") {
            Nested::Absent => Some(BoxConfig::default()),
            Nested::Null => None,
            Nested::Object(area) => Some(BoxConfig {
                x1: int_field(area, "x1", 0),
                y1: int_field(area, "y1", 0),
                x2: int_field(area, "x2", 0),
                y2: int_field(area, "y2", 0),
            }),
        },
        grade_y: band(ocr, "grade_y"),
        attr_y: band(ocr, "attr_y"),
        remaining_y: band(ocr, "remaining_y"),
        attr_x: band(ocr, "attr_x"),
        remaining_x: band(ocr, "remaining_x"),
        row_height: int_field(ocr, "row_height", 0),
    }
}

fn int_field(section: &Map<String, Value>, name: &str, default: i64) -> i32 {
    section
        .get(name)
        .and_then(Value::as_i64)
        .unwrap_or(default)
        .try_into()
        .unwrap_or(i32::MAX)
}

fn band(section: &Map<String, Value>, name: &str) -> Vec<i32> {
    section
        .get(name)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_i64)
                .map(|value| value.try_into().unwrap_or(i32::MAX))
                .collect()
        })
        .unwrap_or_default()
}

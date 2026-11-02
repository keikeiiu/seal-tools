//! `ConfigValidator`: the load-time checks v2 ran before anything touched the game window.
//!
//! # THIS ORACLE IS AUTHORED, NOT TRANSLATED
//!
//! v2 has **no test for this file** (`ConfigLoaderTests.cs` contains no `ConfigException`/`Throws`
//! assertion), so every expectation below was written from `v2/SealTools.Core/Config/ConfigValidator.cs`
//! and `Config/AppConfig.cs`. Each rule and each test cites the v2 line it came from; if this module
//! and those files ever disagree, **the v2 files are right and this module is the bug**.
//!
//! # What v2 actually checked
//!
//! Two layers, in this order (`ConfigValidator.cs:16-68`):
//!
//! 1. **`DataAnnotations` over the whole object graph** (`ConfigValidator.cs:71-100`): v2 walked every
//!    property, recursed into class-typed values and skipped `null` ones, running
//!    `Validator.TryValidateObject(validateAllProperties: true)` on each object. Only ten attributes
//!    exist in the entire config (`AppConfig.cs`): `window.title` Required, `arduino.vid` Range,
//!    `tuner.spring_mode`/`tuner.mouse_guard`/`tuner.filter.match_mode`/`gem.move_mode`/
//!    `gem.empty_mode` `AllowedValues`, `tuner.ocr.region` width/height Range, and
//!    `gem.colored_gap_min` Range. Nothing else in the config carries a check — that is a fact about
//!    v2, not an omission here.
//! 2. **Relational checks** (`ConfigValidator.cs:27-63`): the tuner section, its OCR geometry, the
//!    grade box's ordering and containment, and the required/optional band lengths.
//!
//! The failures are collected (not first-error-wins) and thrown as ONE `ConfigException` whose text
//! is `"Config validation failed:\n  - "` + the lines joined by `"\n  - "` +
//! `"\nCheck defaults.yaml and local.yaml."` (`ConfigValidator.cs:65-68`). [`ConfigErrors::message`]
//! reproduces that exactly.
//!
//! # The shape of the input, and the null quirk
//!
//! v2's annotations live on the DESERIALISED `AppConfig` object graph, so the input here is the same
//! shape: every section is an `Option`, where `None` means **the section was present with no value**
//! (`tuner:` with a bare key) and therefore skipped by the recursion
//! (`ConfigValidator.cs:84`: `if (value == null) continue;`). A section that is **absent** from the
//! file is not `None` here — v2's field initialisers make it a default object, and the caller passes
//! that (`Some(Default::default())`). That distinction is load-bearing and pinned by tests: a bare
//! `window:` validates CLEAN in v2 (the recursion skips it), while an absent `window:` produces the
//! required-title error. This is v2 behaviour, quirks included; the module does not "fix" it.
//!
//! `AllowedValues` in `DataAnnotations` treats `null` as valid (the convention every `DataAnnotation`
//! except `Required` follows), so a `spring_mode:` with no value passes validation and the runtime
//! fallback is what runs — again pinned rather than fixed.
//!
//! # What this module is NOT
//!
//! It is not wired into `seal-core`. The loader's validation call is a separate integration: this is
//! the rule set, ready for the loader to call with the merged values.
//!
//! Positions and the reader side are untouched: this module only compares numbers and strings.

use super::shared::{FilterConfig, OcrGeometry, WindowConfig};

/// One section of the merged config as the deserialiser would have handed it to v2.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValidationInput {
    /// `None` when the `window:` section is present with a null body (skipped by v2's recursion).
    pub window: Option<WindowConfig>,
    pub arduino: Option<ArduinoInput>,
    pub tuner: Option<TunerInput>,
    pub gem: Option<GemInput>,
}

impl ValidationInput {
    /// A complete, valid configuration — the shipped `defaults.yaml` shape for every checked field.
    /// Tests start from this and break one thing at a time.
    #[must_use]
    pub fn clean() -> Self {
        Self {
            window: Some(WindowConfig {
                title: "Seal Online".to_owned(),
            }),
            arduino: Some(ArduinoInput { vid: 0x2341 }),
            tuner: Some(TunerInput::calibrated()),
            gem: Some(GemInput::default()),
        }
    }
}

impl TunerInput {
    /// A calibrated tuner: valid geometry and bands, which is what makes [`ValidationInput::clean`]
    /// pass. The default `OcrGeometry` is deliberately NOT this — an absent geometry is exactly the
    /// failure the relational checks exist to describe.
    #[must_use]
    pub fn calibrated() -> Self {
        use super::shared::{BoxConfig, RegionConfig};
        Self {
            spring_mode: Some("manual".to_owned()),
            mouse_guard: Some("off".to_owned()),
            filter: Some(FilterConfig::default()),
            ocr: Some(OcrGeometry {
                region: Some(RegionConfig {
                    left: 0,
                    top: 0,
                    width: 400,
                    height: 300,
                }),
                grade_area: Some(BoxConfig {
                    x1: 0,
                    y1: 0,
                    x2: 100,
                    y2: 40,
                }),
                grade_y: vec![0, 40],
                attr_y: vec![40, 80],
                remaining_y: vec![80, 100],
                attr_x: Vec::new(),
                remaining_x: Vec::new(),
                row_height: 20,
            }),
        }
    }
}

/// `arduino.vid` — `[Range(1, int.MaxValue)]` (`AppConfig.cs:822`). An absent key leaves v2's int
/// default `0`, which fails the range.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArduinoInput {
    pub vid: i64,
}

/// The tuner fields that carry a check.
#[derive(Debug, Clone, PartialEq)]
pub struct TunerInput {
    /// `tuner.spring_mode`: `manual|hid` (`AppConfig.cs:854`). `None` = present-null (passes).
    pub spring_mode: Option<String>,
    /// `tuner.mouse_guard`: `off|stop|recenter` (`AppConfig.cs:864`). `None` = present-null (passes).
    pub mouse_guard: Option<String>,
    /// `tuner.filter` — only `match_mode` carries a check (`AppConfig.cs:950`). `None` = null section.
    pub filter: Option<FilterConfig>,
    /// `tuner.ocr` — the geometry the relational checks read. `None` = null section.
    pub ocr: Option<OcrGeometry>,
}

impl Default for TunerInput {
    /// v2's `TunerConfig` field initialisers: manual spring, guard off, a default filter and an
    /// empty-but-object OCR geometry. The geometry is what makes a fresh install fail validation with
    /// the descriptive tuner OCR lines rather than a null dereference.
    fn default() -> Self {
        Self {
            spring_mode: Some("manual".to_owned()),
            mouse_guard: Some("off".to_owned()),
            filter: Some(FilterConfig::default()),
            ocr: Some(OcrGeometry::default()),
        }
    }
}

/// The gem fields that carry a check.
#[derive(Debug, Clone, PartialEq)]
pub struct GemInput {
    /// `gem.move_mode`: `tuned|arduino` (`AppConfig.cs:968`). `None` = present-null (passes).
    pub move_mode: Option<String>,
    /// `gem.empty_mode`: `stop|advance_grade|advance_grade_clear` (`AppConfig.cs:986`).
    pub empty_mode: Option<String>,
    /// `gem.colored_gap_min`, 0-255 (`AppConfig.cs:999`).
    pub colored_gap_min: i64,
}

impl Default for GemInput {
    /// v2's initialisers: `arduino`, `stop`, `40` (`AppConfig.cs:969,987,1000`).
    fn default() -> Self {
        Self {
            move_mode: Some("arduino".to_owned()),
            empty_mode: Some("stop".to_owned()),
            colored_gap_min: 40,
        }
    }
}

/// Every failure v2 would have collected, in v2's order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigErrors {
    lines: Vec<String>,
}

impl ConfigErrors {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// The individual rule lines, unprefixed — `window.title is required`, not the `"Window: "` path
    /// wrapper v2's reflection added (that wrapper IS included in the lines; this doc's example is the
    /// attribute's own message).
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// v2's `ConfigException` text (`ConfigValidator.cs:65-68`), or `None` when nothing failed.
    #[must_use]
    pub fn message(&self) -> Option<String> {
        if self.lines.is_empty() {
            return None;
        }
        Some(format!(
            "Config validation failed:\n  - {}\nCheck defaults.yaml and local.yaml.",
            self.lines.join("\n  - ")
        ))
    }

    fn push(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }
}

/// The v2 check set, in v2's order: the annotation walk first, the relational checks after it.
#[must_use]
pub fn validate(input: &ValidationInput) -> ConfigErrors {
    let mut errors = ConfigErrors::default();
    annotation_errors(input, &mut errors);
    relational_errors(input, &mut errors);
    errors
}

/// The ten `DataAnnotations`, with the property-path prefix v2's `ValidateObject` produced: the walk
/// appends the *property name* (`PascalCase`) to the path (`ConfigValidator.cs:76,118-119`), so a
/// nested failure reads `Tuner.Filter: tuner.filter.match_mode must be any|all|per_attr`.
///
/// Order: `ValidateObject` validates each object's own properties first, then recurses into its
/// class-typed properties in declaration order (`ConfigValidator.cs:73-99`). For `AppConfig` that is
/// Window → Arduino → Tuner → Gem, and inside the tuner Filter → Ocr, because `Filter` is declared
/// above `Ocr` (`AppConfig.cs:848-849`).
fn annotation_errors(input: &ValidationInput, errors: &mut ConfigErrors) {
    // WindowConfig.Title Required (`AppConfig.cs:793`). Required on a string fails for null AND "".
    if let Some(window) = &input.window
        && window.title.is_empty()
    {
        errors.push("Window: window.title is required");
    }

    // ArduinoConfig.Vid Range(1, int.MaxValue) (`AppConfig.cs:822`).
    if let Some(arduino) = &input.arduino
        && arduino.vid < 1
    {
        errors.push("Arduino: arduino.vid must be > 0");
    }

    if let Some(tuner) = &input.tuner {
        if let Some(mode) = &tuner.spring_mode
            && !matches!(mode.as_str(), "manual" | "hid")
        {
            errors.push("Tuner: tuner.spring_mode must be manual|hid");
        }
        if let Some(guard) = &tuner.mouse_guard
            && !matches!(guard.as_str(), "off" | "stop" | "recenter")
        {
            errors.push("Tuner: tuner.mouse_guard must be off|stop|recenter");
        }
        // Filter is declared above Ocr (`AppConfig.cs:848-849`), so its recursion comes first.
        if let Some(filter) = &tuner.filter
            && !matches!(filter.match_mode.as_str(), "any" | "all" | "per_attr")
        {
            errors.push("Tuner.Filter: tuner.filter.match_mode must be any|all|per_attr");
        }
        // RegionConfig.Width then .Height (`AppConfig.cs:897-900`), in property order.
        if let Some(region) = &tuner.ocr.as_ref().and_then(|ocr| ocr.region) {
            if region.width < 1 {
                errors.push("Tuner.Ocr.Region: tuner.ocr.region width/height must be > 0");
            }
            if region.height < 1 {
                errors.push("Tuner.Ocr.Region: tuner.ocr.region width/height must be > 0");
            }
        }
    }

    if let Some(gem) = &input.gem {
        if let Some(mode) = &gem.move_mode
            && !matches!(mode.as_str(), "tuned" | "arduino")
        {
            errors.push("Gem: gem.move_mode must be tuned|arduino");
        }
        if let Some(mode) = &gem.empty_mode
            && !matches!(
                mode.as_str(),
                "stop" | "advance_grade" | "advance_grade_clear"
            )
        {
            errors.push("Gem: gem.empty_mode must be stop|advance_grade|advance_grade_clear");
        }
        if !(0..=255).contains(&gem.colored_gap_min) {
            errors.push("Gem: gem.colored_gap_min must be 0-255");
        }
    }
}

/// The relational checks (`ConfigValidator.cs:27-63`), appended after the annotation walk.
///
/// Every message is v2's, including its `$"...region is {w}x{h}"` suffix and the band wording.
fn relational_errors(input: &ValidationInput, errors: &mut ConfigErrors) {
    // `ConfigValidator.cs:27-32`: a bare `tuner:` key makes the whole section null, and the
    // annotation walk skips null values — so this is the first place v2 would have crashed.
    let Some(tuner) = &input.tuner else {
        errors.push(
            "tuner: the section is empty — it needs grade_order, the ocr geometry and the timing \
             (edit the tuner block in config/local.yaml; the tuner's OCR geometry is hand-edited \
             in v3.0 because the three-box session is not built)",
        );
        return;
    };

    // `ConfigValidator.cs:35-39`.
    let Some(ocr) = &tuner.ocr else {
        errors.push("tuner.ocr: missing — the tuner needs its OCR geometry (the tuner's OCR geometry is hand-edited in config/local.yaml; the three-box session is not built in v3.0)");
        return;
    };

    // `ConfigValidator.cs:42-54`. Note the two independent `if/else` chains: an ordered-but-outside
    // box produces the containment error too, and a missing region does not skip the band checks.
    if ocr.grade_area.is_none() {
        errors.push("tuner.ocr.grade_area: missing — needs x1, y1, x2, y2");
    } else if let Some(area) = ocr.grade_area
        && (area.x2 <= area.x1 || area.y2 <= area.y1)
    {
        errors.push("tuner.ocr.grade_area: x2/y2 must be greater than x1/y1");
    }

    if ocr.region.is_none() {
        errors.push("tuner.ocr.region: missing — needs left, top, width, height");
    } else if let (Some(region), Some(area)) = (ocr.region, ocr.grade_area)
        && (area.x1 < 0 || area.y1 < 0 || area.x2 > region.width || area.y2 > region.height)
    {
        errors.push(format!(
            "tuner.ocr.grade_area must lie inside tuner.ocr.region (region is {}x{})",
            region.width, region.height
        ));
    }

    // `ConfigValidator.cs:57-61`: the three required bands, then the two optional ones. OcrEngine
    // indexes these as [0]/[1], so a short list is an IndexOutOfRange at scan time.
    require_band(errors, "tuner.ocr.grade_y", &ocr.grade_y);
    require_band(errors, "tuner.ocr.attr_y", &ocr.attr_y);
    require_band(errors, "tuner.ocr.remaining_y", &ocr.remaining_y);
    require_optional_band(errors, "tuner.ocr.attr_x", &ocr.attr_x);
    require_optional_band(errors, "tuner.ocr.remaining_x", &ocr.remaining_x);
}

/// `ConfigValidator.cs:103-109`: a required band is `[top, bottom]` (or `[left, right]`) and must be
/// ordered. A list LONGER than two is accepted — only the first two are compared — and that v2 quirk
/// is pinned rather than tightened.
fn require_band(errors: &mut ConfigErrors, name: &str, band: &[i32]) {
    if band.len() < 2 {
        errors.push(format!("{name}: needs 2 values; got {}", band.len()));
    } else if band[1] <= band[0] {
        errors.push(format!(
            "{name}: the second value must be greater than the first"
        ));
    }
}

/// `ConfigValidator.cs:112-116`: an optional band may be omitted entirely, but if present it must be
/// a complete pair. Longer-than-two is accepted, as above.
fn require_optional_band(errors: &mut ConfigErrors, name: &str, band: &[i32]) {
    if band.len() == 1 {
        errors.push(format!("{name}: leave it out or give 2 values; got 1"));
    }
}

//! The OCR "wordings" dictionary: `v2/config/attributes.yaml`, typed.
//!
//! Ported from `v2/SealTools.Core/Config/AttributesConfig.cs`. The file is hand-editable by contract
//! ("Adding or correcting an OCR variant is a config edit, not a code change"), so its decode is as
//! tolerant as the config model's: unknown keys are ignored, a missing section is empty, and a bare
//! key (`text_fixes:`, `whole:`) loads rather than failing.
//!
//! What each section is FOR, because the split is not obvious from the names:
//!
//! - [`AttributesConfig::attributes`] — each standard attribute's canonical name, its category, and
//!   its OCR-garbled `variants`. For a per-level attribute (`plv*`) the LAST variant is the stat name
//!   the matcher keys on; for every other one the variants are alternative spellings of the name.
//! - [`AttributesConfig::per_level_stats`] — the stat names the per-level parser recognises. It was
//!   hardcoded in v1's `_extract_perlevel`; moving it here made a new stat a config edit.
//! - [`AttributesConfig::negative_attr_markers`] — labels that always carry a negative value, applied
//!   when the sign is fixed up after a read (the OCR drops the minus).
//! - [`AttributesConfig::text_fixes`] — the cleanup table. ORDER MATTERS; see
//!   [`crate::ocr::TextCleaner`].

use serde::{Deserialize, Serialize};

/// `v2/config/attributes.yaml` — the OCR dictionary for one game client.
///
/// The US client ships `attributes: []` with nothing else, on purpose: an absent dictionary makes
/// every attribute read as "no attribute", silently, which looks exactly like the game having
/// changed — so the file exists (the loader refuses a missing one) and says "nothing here yet".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AttributesConfig {
    /// The standard attribute definitions, in the order the Attributes tab lists them.
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub attributes: Vec<AttributeDef>,

    /// Stat names recognised by the per-level attribute parser.
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub per_level_stats: Vec<String>,

    /// Attribute labels that always carry a negative value.
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub negative_attr_markers: Vec<String>,

    /// The OCR text-cleanup tables.
    #[serde(default)]
    pub text_fixes: TextFixesConfig,
}

/// One attribute: the name filter rules match on, its category, and the forms OCR returns.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AttributeDef {
    /// What filter rules match on, so this is also the name to write in a tuner filter rule.
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub name: String,

    /// Free-form grouping, shown in the Attributes tab and used by the per-level rule
    /// (`plv…` categories are the per-level ones).
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub category: String,

    /// The OCR-garbled forms. The first is normally the exact name; the last is the stat name for a
    /// `plv*` category.
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub variants: Vec<String>,
}

/// The fix tables, in the order [`crate::ocr::TextCleaner`] applies them.
///
/// `final` is a Rust keyword, hence the renamed field; the serde name is v2's, so the file decodes
/// unchanged.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TextFixesConfig {
    /// If the WHOLE trimmed string equals `from`, replace it and stop — no later table runs.
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub whole: Vec<TextFix>,

    /// Substring replacements. A pure-ASCII `from` is replaced on WORD BOUNDARIES, so a correct
    /// "GRADE" cannot become "GGRADE" (v1 matched with a `\b` regex; the port keeps that).
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub substring: Vec<TextFix>,

    /// RapidOCR defaults to simplified Chinese; these convert the characters that arrive that way.
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub simplified_traditional: Vec<TextFix>,

    /// Applied after the simplified→traditional pass, for corrections that would be wrong earlier.
    #[serde(
        rename = "final",
        default,
        deserialize_with = "crate::config::de::null_as_default"
    )]
    pub final_rules: Vec<TextFix>,

    /// `.NET` regular-expression fixes — the negative lookahead restores a dropped trailing stat
    /// character, where a plain substring replace would corrupt the correct form (the truncated form
    /// is a PREFIX of the correct one).
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub regex: Vec<TextFix>,
}

/// One `{from, to}` entry.
///
/// `to` may be omitted; v2 would hand a null to `string.Replace` and throw, and the decode repairs it
/// to an empty replacement instead (which deletes `from` from the text — a `to` is a replacement, not
/// a requirement).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextFix {
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub from: String,
    #[serde(default, deserialize_with = "crate::config::de::null_as_default")]
    pub to: String,
}

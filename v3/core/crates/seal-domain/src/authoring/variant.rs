//! Game-variant switching: `AppConfig.ApplyVariant` + `LauncherService.SwitchVariant`, as a pure
//! plan.
//!
//! THE ORDER IS THE SAFETY, and it is why this is two functions instead of one mutating call. v2
//! resolves the variant first, loads its dictionary second, and only then leaves the config switched,
//! rolling the title back if the load fails — "a config pointing at a variant whose dictionary is
//! missing is not a degraded launcher, it is one that THROWS in its constructor and never opens a
//! window at all".
//!
//! [`plan_variant_switch`] is the resolve step: it computes everything the switch would write and
//! changes NOTHING, so a failure cannot leave the config half-switched. The caller loads
//! [`VariantSwitch::dictionary`], and only then applies the plan.

use crate::config::GameConfig;
use std::fmt;

/// What a successful switch would write, computed before anything is committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantSwitch {
    /// The name as the player typed it, trimmed — v2 stores the typed name, not the table's spelling
    /// (`TW` stays `TW`), and the lookup ignores case on every later load for that reason.
    pub variant: String,
    /// The window title the variant resolves to.
    pub window_title: String,
    /// The dictionary file in `config/` to load before committing.
    pub dictionary: String,
}

/// Why the switch cannot happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantError {
    /// The name is not one of the configured clients.
    Unknown {
        /// The typed name, trimmed.
        requested: String,
        /// The clients this config knows, in map order.
        known: Vec<String>,
    },
}

impl VariantError {
    /// The line the Setup tab's status shows — v2's `ApplyVariant` wording.
    pub fn message(&self) -> String {
        match self {
            Self::Unknown { requested, known } => format!(
                "\"{requested}\" is not a known game variant. Known: {}.",
                known.join(", ")
            ),
        }
    }
}

impl fmt::Display for VariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for VariantError {}

/// Resolves a requested client to everything the switch would set, without setting any of it.
///
/// An unknown (or blank) name fails with the known clients listed, and the caller's config is
/// untouched by construction — there is no half-switched state to roll back.
///
/// A variant with a blank dictionary falls back to `attributes.yaml`, which is what v2's
/// `ConfigLoader.LoadAttributes(null)` does: a config saved before a client grew its own dictionary
/// must still start rather than pointing at no file at all.
pub fn plan_variant_switch(
    game: &GameConfig,
    requested: &str,
) -> Result<VariantSwitch, VariantError> {
    let wanted = requested.trim();

    let variant = game
        .variant_named(wanted)
        .ok_or_else(|| VariantError::Unknown {
            requested: wanted.to_owned(),
            known: game.variants.keys().cloned().collect(),
        })?;

    let dictionary = {
        let attributes = variant.attributes.trim();
        if attributes.is_empty() {
            "attributes.yaml".to_owned()
        } else {
            attributes.to_owned()
        }
    };

    Ok(VariantSwitch {
        variant: wanted.to_owned(),
        window_title: variant.title.clone(),
        dictionary,
    })
}

/// The load-time rule: the active variant RESOLVES the window title, and an unset or unknown variant
/// changes nothing.
///
/// Unset is the shape of every config written before variants existed; unknown is a typo. In both the
/// configured `window.title` stands — blanking it would send every tool hunting a window called "".
/// v2's guard is `{ Title.Length: > 0 }`, so a variant with an empty title leaves the title alone too.
pub fn resolve_window_title<'a>(game: &'a GameConfig, configured: &'a str) -> &'a str {
    match game.active() {
        Some(variant) if !variant.title.is_empty() => variant.title.as_str(),
        _ => configured,
    }
}

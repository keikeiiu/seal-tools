//! Repairs for hand-edited YAML, ported from what v2 actually does with the file.
//!
//! `v2/SealTools.Core/Config/ConfigLoader.cs` deserialises with
//! `IgnoreUnmatchedProperties` and lets YamlDotNet hand a null to any *reference* property whose
//! key is present with no value (`run:`, `steps:`, `- `). Its readers then either treat the null as
//! "nothing" or throw — the class of failure `QuestConfig.NormaliseNullBodies` exists to remove, and
//! the one `local.yaml` is hand-editable by contract. The helpers here repair the null at the decode
//! instead, so every reader afterwards has the invariant v2's load step establishes:
//!
//! - a named body (`run:`) is an empty body, not a null, and the key is KEPT (dropping it would
//!   rename the player's flow);
//! - a list whose items are bodies (a bare `- ` entry) has a blank body;
//! - a list/object field is its type's default rather than a null a reader has to guard.
//!
//! Value-type properties (`loops:`, an `int`) get no repair: YamlDotNet fails such a load too, and a
//! quiet zero there would be a setting that means something different from absent.

use serde::de::{Deserialize, Deserializer};
use std::collections::BTreeMap;

/// A field of a reference type whose key is present with no value: `title:` → `""`, `rules:` → `[]`.
pub(crate) fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// A named map whose VALUES are bodies: `presets: {run: }` keeps `run` and gives it an empty body.
pub(crate) fn map_with_default_bodies<'de, D, V>(
    deserializer: D,
) -> Result<BTreeMap<String, V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de> + Default,
{
    Ok(
        Option::<BTreeMap<String, Option<V>>>::deserialize(deserializer)?
            .unwrap_or_default()
            .into_iter()
            .map(|(name, body)| (name, body.unwrap_or_default()))
            .collect(),
    )
}

/// The same repair for a map that is itself optional — the local overlay's "not stated" and "stated
/// as empty" must stay different, so the outer null stays `None`.
pub(crate) fn opt_map_with_default_bodies<'de, D, V>(
    deserializer: D,
) -> Result<Option<BTreeMap<String, V>>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de> + Default,
{
    Ok(
        Option::<BTreeMap<String, Option<V>>>::deserialize(deserializer)?.map(|map| {
            map.into_iter()
                .map(|(name, body)| (name, body.unwrap_or_default()))
                .collect()
        }),
    )
}

/// A list whose ITEMS are bodies: a bare `- ` list item loads as an empty body.
pub(crate) fn vec_with_default_bodies<'de, D, V>(deserializer: D) -> Result<Vec<V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de> + Default,
{
    Ok(Option::<Vec<Option<V>>>::deserialize(deserializer)?
        .unwrap_or_default()
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect())
}

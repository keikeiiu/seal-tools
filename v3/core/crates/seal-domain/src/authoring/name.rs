//! The naming rules from the launcher's `PresetPicker` and `PresetCopy.cs`.
//!
//! One picker serves the Quest flow, Quest sequence and Spammer preset lists. v2's comment says why
//! it is one: the shape had been hand-rolled three times and "each copy had its own bugs", so the
//! rules that are the same everywhere live together. They are pure logic over names, which is why
//! they belong here rather than in a widget.

use std::fmt;

/// The suffix [`copy_name`] appends for the first copy.
const COPY_SUFFIX: &str = " (copy)";
/// The prefix [`copy_name`] appends for the second and later copies, before the number.
const NUMBERED_PREFIX: &str = " (copy ";

/// What a name belongs to. The noun is part of the RULE, not decoration: it is what the refusal says,
/// and three lists share the rule — "A flow needs a name." versus "A preset needs a name.".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetNoun {
    Flow,
    Sequence,
    Preset,
}

impl PresetNoun {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::Sequence => "sequence",
            Self::Preset => "preset",
        }
    }
}

/// Why a typed name cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    /// Empty after trimming.
    NeedsName { noun: PresetNoun },
    /// Already in play.
    Duplicate { noun: PresetNoun, name: String },
}

impl NameError {
    /// The line the picker's `InfoBar` shows.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::NeedsName { noun } => format!("A {} needs a name.", noun.as_str()),
            Self::Duplicate { noun, name } => {
                format!(
                    "There is already a {} called \"{name}\" — pick another name.",
                    noun.as_str()
                )
            }
        }
    }
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for NameError {}

/// Validates a name the player TYPED (create or rename), returning the trimmed name that will be
/// stored.
///
/// * `current` is the name being renamed, or `None` for a create. Renaming to itself is the picker's
///   no-op success, not a duplicate.
/// * `taken` is every name in play — the picker's dictionary keys.
///
/// The comparison is CASE-SENSITIVE, because v2's `Dictionary.Contains(name)` is: the dictionaries
/// are keyed case-sensitively, so "Mine" beside "mine" is accepted, and only the GENERATED copy name
/// below refuses a case-only difference (the picker sorts ignore-case, so a generated name is the one
/// place a player would see two indistinguishable rows).
///
/// # Errors
///
/// Returns [`NameError::NeedsName`] when the name is empty after trimming, and
/// [`NameError::Duplicate`] when another name in `taken` is already exactly this one.
pub fn validate_name<I, S>(
    noun: PresetNoun,
    requested: &str,
    current: Option<&str>,
    taken: I,
) -> Result<String, NameError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let name = requested.trim();
    if name.is_empty() {
        return Err(NameError::NeedsName { noun });
    }

    // Before the duplicate check: the name being renamed is IN `taken`, and renaming to it unchanged
    // must close the prompt rather than refuse itself.
    if current == Some(name) {
        return Ok(name.to_owned());
    }

    if taken.into_iter().any(|existing| existing.as_ref() == name) {
        return Err(NameError::Duplicate {
            noun,
            name: name.to_owned(),
        });
    }

    Ok(name.to_owned())
}

/// The name a Duplicate button gives its copy: `"<name> (copy)"`, then `"<name> (copy 2)"`, up to the
/// first free name — v2's `PresetCopy.CopyName`, ported with its tests.
///
/// Two rules in here are not obvious:
///
/// * **The source's own suffix is stripped FIRST, and the count resumes from it.** Without the strip
///   the second copy would be "mine (copy) (copy)"; without resuming, copying "mine (copy 2)" would
///   walk BACKWARDS into a free "mine (copy)" and produce a name that sorts above the preset it came
///   from.
/// * **`taken` is compared case-INSENSITIVELY**, unlike a typed name: the pickers sort ignore-case,
///   so "mine (copy)" beside "Mine (copy)" is two rows the player cannot tell apart, and a generated
///   name must never be what creates one.
pub fn copy_name<I, S>(source: &str, taken: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let taken: Vec<String> = taken
        .into_iter()
        .map(|existing| existing.as_ref().to_lowercase())
        .collect();

    let mut stem = source;
    let mut start: i64 = 0;

    if let Some(stripped) = stem.strip_suffix(COPY_SUFFIX) {
        stem = stripped;
        start = 1;
    } else if let Some(open) = stem.rfind(NUMBERED_PREFIX) {
        // " (copy <n>)": the DIGITS are what make a suffix ours, so "chapter (copy x)" keeps all of
        // it, and "(copy 0)" is not a suffix we wrote — the count starts at 1.
        let tail = &stem[open + NUMBERED_PREFIX.len()..];
        if let Some(number) = tail.strip_suffix(')')
            && !number.is_empty()
            && number.bytes().all(|byte| byte.is_ascii_digit())
            && let Ok(parsed) = number.parse::<i32>()
            && parsed >= 1
        {
            stem = &stem[..open];
            start = i64::from(parsed);
        }
    }

    // `long`, not `int`: the parsed index is an int32 and the count continues from it, so at
    // `Int32::MAX` an int counter would wrap and name the copy "(copy -2147483648)".
    let mut count = start + 1;
    loop {
        let candidate = if count == 1 {
            format!("{stem} (copy)")
        } else {
            format!("{stem} (copy {count})")
        };
        if !taken.contains(&candidate.to_lowercase()) {
            return candidate;
        }
        count += 1;
    }
}

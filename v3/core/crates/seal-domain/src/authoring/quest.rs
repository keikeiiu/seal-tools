//! Quest authoring: reference integrity, the Save gate, the step rule, and the picker's
//! rename / duplicate / delete behaviour.
//!
//! The failures these prevent are silent on screen. A sequence entry naming a deleted flow makes the
//! run skip a step it cannot plan, and nothing says so; a rename that leaves entries behind is only
//! noticed by a Save that then refuses. `QuestConfigTests.cs` is the oracle for the first, and the
//! launcher's Quest-tab handler for the refusals.

use super::name::{NameError, PresetNoun, copy_name, validate_name};
use crate::config::{QuestConfig, QuestStep, quest_action};
use std::fmt;

// ── reference integrity ────────────────────────────────────────────────────────────────────────

/// One sequence entry that would not run: a flow name that is not in the flow list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DanglingEntry {
    /// The sequence that holds the entry.
    pub sequence: String,
    /// 1-based position within [`Self::sequence`] — the number the player reads next to the rows.
    pub entry: usize,
    /// The flow name as written, trimmed (an untrimmed name would still run, so it is not reported).
    pub flow: String,
}

impl DanglingEntry {
    /// One line of the refusal.
    #[must_use]
    pub fn message(&self) -> String {
        if self.flow.is_empty() {
            format!(
                "\"{}\" entry {} has no flow chosen",
                self.sequence, self.entry
            )
        } else {
            format!(
                "\"{}\" entry {} names the flow \"{}\", which does not exist",
                self.sequence, self.entry, self.flow
            )
        }
    }
}

/// The Save refusal: every dangling entry, in a fixed order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSaveError {
    pub dangling: Vec<DanglingEntry>,
}

impl QuestSaveError {
    /// The message the Quest tab's Save barrier shows — the v2 wording, line for line.
    pub fn message(&self) -> String {
        let mut lines: Vec<String> = self.dangling.iter().map(DanglingEntry::message).collect();
        lines.push(
            "Pick an existing flow for each entry, or remove it, then Save again.".to_owned(),
        );
        lines.join("\n")
    }
}

impl fmt::Display for QuestSaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for QuestSaveError {}

/// Every entry, in every sequence, that names a flow the config does not have.
///
/// Ordered by sequence name IGNORE-CASE then by position, so the same defect reads the same way twice
/// — the message lists every hit, and a message whose lines move between saves is one the player
/// cannot compare.
///
/// The flow name is trimmed before the lookup, which is exactly what the runner does: a hand-edited
/// `preset: " name "` still runs, so refusing it here would block a Save for a sequence that works.
/// A name whose body is null is repaired at decode ([`crate::config`]), so "present but empty" cannot
/// reach here — the invariant v2 needed `NormaliseNullBodies()` to establish.
#[must_use]
pub fn dangling_sequence_entries(quest: &QuestConfig) -> Vec<DanglingEntry> {
    let mut sequences: Vec<(&str, &crate::config::QuestSequence)> = quest
        .sequences
        .iter()
        .map(|(name, sequence)| (name.as_str(), sequence))
        .collect();
    sequences.sort_by(|left, right| {
        left.0
            .to_lowercase()
            .cmp(&right.0.to_lowercase())
            .then_with(|| left.0.cmp(right.0))
    });

    let mut dangling = Vec::new();
    for (name, sequence) in sequences {
        for (index, entry) in sequence.entries.iter().enumerate() {
            let flow = entry.preset.trim();
            if !quest.presets.contains_key(flow) {
                dangling.push(DanglingEntry {
                    sequence: name.to_owned(),
                    entry: index + 1,
                    flow: flow.to_owned(),
                });
            }
        }
    }
    dangling
}

/// The Save gate: `Ok(())` when every sequence entry names a real flow, else the refusal.
///
/// Save writes the WHOLE quest block, so every sequence is checked, not the active one: a dangling
/// entry in a sequence the player is not looking at is still about to be saved.
///
/// # Errors
///
/// Returns [`QuestSaveError`] carrying every dangling entry (see
/// [`dangling_sequence_entries`]) so the tab can refuse the Save and list all of them at once.
pub fn check_saveable(quest: &QuestConfig) -> Result<(), QuestSaveError> {
    let dangling = dangling_sequence_entries(quest);
    if dangling.is_empty() {
        Ok(())
    } else {
        Err(QuestSaveError { dangling })
    }
}

// ── the step rule ──────────────────────────────────────────────────────────────────────────────

/// Why this step cannot be sent, or `None` when it can — v2's `QuestAction.Complaint`, ported.
///
/// The caller names the step's number ("step 7"), because that is what the player has to go and fix;
/// this function judges the step itself. A step that cannot be sent, in a flow replayed 500 times, is
/// indistinguishable from a quest that behaves differently today, so it is refused BEFORE the run.
///
/// The action list is checked exactly (the values are lowercase and the picker writes them); the
/// `key` comparison ignores case, as v2's does.
#[must_use]
pub fn quest_step_complaint(step: &QuestStep) -> Option<String> {
    let action = step.action.trim();

    if !quest_action::ALL.contains(&action) {
        return Some(format!(
            "\"{}\" is not a step this tool can send — pick one of {}.",
            step.action,
            quest_action::ALL.join(", ")
        ));
    }

    if !action.eq_ignore_ascii_case(quest_action::KEY) {
        return None;
    }

    let value = step.value.as_deref().unwrap_or("");
    if value.is_empty() {
        return Some("a key step with no character would send nothing at all.".to_owned());
    }

    let characters: Vec<char> = value.chars().collect();
    if characters.len() > 1 {
        return Some(format!(
            "a key step sends ONE character, and \"{value}\" is {} — only the board's own F1-F12 \
             command carries a number, and that is not this.",
            characters.len()
        ));
    }

    if !(' '..='~').contains(&characters[0]) {
        return Some(format!(
            "\"{value}\" is not a printable character the board can press."
        ));
    }

    None
}

// ── picker operations ─────────────────────────────────────────────────────────────────────────

/// Which picker action needed a name that is not there. The verb is part of the message: the picker
/// says "Pick a flow to rename", "to copy" or "to delete" — three different sentences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetAction {
    Rename,
    Copy,
    Delete,
}

impl PresetAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rename => "rename",
            Self::Copy => "copy",
            Self::Delete => "delete",
        }
    }
}

/// Why a rename / duplicate / delete cannot happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresetError {
    /// The UI's stale selection: the name the action would act on is not in play.
    Missing {
        noun: PresetNoun,
        action: PresetAction,
    },
    /// The Quest tab cannot run with no flow at all, so its last flow is not deletable.
    LastFlow,
    /// The new name is not usable — empty, or already in play.
    Name(NameError),
}

impl PresetError {
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Missing { noun, action } => {
                format!("Pick a {} to {}.", noun.as_str(), action.as_str())
            }
            Self::LastFlow => {
                "This is the only flow — create another before deleting it.".to_owned()
            }
            Self::Name(error) => error.message(),
        }
    }
}

impl fmt::Display for PresetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for PresetError {}

impl From<NameError> for PresetError {
    fn from(error: NameError) -> Self {
        Self::Name(error)
    }
}

/// Renames a flow, keeping every reference true, and returns how many sequence entries were
/// re-pointed.
///
/// The body MOVES — a rename does not leave a copy behind under the old name, because a rename that
/// quietly doubles the list is not a rename. The active flow's target is a NAME, so it follows too,
/// or Start would run a different flow; and every sequence entry that named the old name follows,
/// compared trimmed and case-sensitively exactly as the runner looks it up (v2's `AfterRename`).
///
/// # Errors
///
/// Returns [`PresetError::Missing`] when `from` is not a flow in play, and the naming refusal from
/// [`validate_name`] (wrapped in [`PresetError::Name`]) when `to` is empty or already taken.
pub fn rename_flow(quest: &mut QuestConfig, from: &str, to: &str) -> Result<usize, PresetError> {
    if !quest.presets.contains_key(from) {
        return Err(PresetError::Missing {
            noun: PresetNoun::Flow,
            action: PresetAction::Rename,
        });
    }

    let name = validate_name(
        PresetNoun::Flow,
        to,
        Some(from),
        quest.presets.keys().map(String::as_str),
    )?;
    if name == from {
        return Ok(0);
    }

    let Some(body) = quest.presets.remove(from) else {
        // The presence check above makes this unreachable; returning the same refusal keeps a future
        // edit to that check from panicking in the tab's handler.
        return Err(PresetError::Missing {
            noun: PresetNoun::Flow,
            action: PresetAction::Rename,
        });
    };
    quest.presets.insert(name.clone(), body);

    if quest.active == from {
        quest.active.clone_from(&name);
    }

    let mut repointed = 0;
    for sequence in quest.sequences.values_mut() {
        for entry in &mut sequence.entries {
            if entry.preset.trim() == from {
                entry.preset.clone_from(&name);
                repointed += 1;
            }
        }
    }

    Ok(repointed)
}

/// Renames a sequence. Nothing references a sequence by name except the active choice, which follows
/// — entries inside it name FLOWS, which this must not touch.
///
/// # Errors
///
/// Returns [`PresetError::Missing`] when `from` is not a sequence in play, and the naming refusal from
/// [`validate_name`] (wrapped in [`PresetError::Name`]) when `to` is empty or already taken.
pub fn rename_sequence(quest: &mut QuestConfig, from: &str, to: &str) -> Result<(), PresetError> {
    if !quest.sequences.contains_key(from) {
        return Err(PresetError::Missing {
            noun: PresetNoun::Sequence,
            action: PresetAction::Rename,
        });
    }

    let name = validate_name(
        PresetNoun::Sequence,
        to,
        Some(from),
        quest.sequences.keys().map(String::as_str),
    )?;
    if name == from {
        return Ok(());
    }

    let Some(body) = quest.sequences.remove(from) else {
        // The presence check above makes this unreachable; returning the same refusal keeps a future
        // edit to that check from panicking in the tab's handler.
        return Err(PresetError::Missing {
            noun: PresetNoun::Sequence,
            action: PresetAction::Rename,
        });
    };
    quest.sequences.insert(name.clone(), body);

    // The picker's active value is trimmed (`ActiveSequenceOrNull` trims too), so a padded
    // `active_sequence` that names the renamed sequence still follows it.
    if quest.active_sequence.trim() == from {
        quest.active_sequence = name;
    }

    Ok(())
}

/// Deletes a flow.
///
/// The LAST flow is refused: the Quest tab would have nothing to run, and v2's delete said exactly
/// that before asking "Delete the flow?". Deleting a flow a sequence still names is ALLOWED — the Save
/// gate is where the player is stopped and told, rather than the delete choosing a replacement the
/// player did not pick.
///
/// The picker leaves the list on another flow afterwards; which one is the map's first name. v2 took
/// its insertion-ordered dictionary's first key and a `BTreeMap` has no insertion order, so the choice
/// here is deterministic-but-sorted instead of insertion-ordered — the active flow is a UI selection,
/// not a stored calibration, and a stable order beats an arbitrary one.
///
/// # Errors
///
/// Returns [`PresetError::Missing`] when `name` is not a flow in play, and [`PresetError::LastFlow`]
/// when it is the only flow left.
pub fn remove_flow(quest: &mut QuestConfig, name: &str) -> Result<(), PresetError> {
    if !quest.presets.contains_key(name) {
        return Err(PresetError::Missing {
            noun: PresetNoun::Flow,
            action: PresetAction::Delete,
        });
    }

    if quest.presets.len() <= 1 {
        return Err(PresetError::LastFlow);
    }

    quest.presets.remove(name);
    quest.active = quest.presets.keys().next().cloned().unwrap_or_default();

    Ok(())
}

/// Duplicates a flow under the next free generated name, selects the copy, and returns its name.
///
/// The copy is independent of the source. In Rust that is `Clone` (every field is owned), which is
/// why there is no `CopyQuestPreset` here: the v2 function exists only because C# would have shared
/// the step list.
///
/// # Errors
///
/// Returns [`PresetError::Missing`] when `from` is not a flow in play — the only refusal, because the
/// generated name is always free.
pub fn duplicate_flow(quest: &mut QuestConfig, from: &str) -> Result<String, PresetError> {
    let source = quest
        .presets
        .get(from)
        .cloned()
        .ok_or(PresetError::Missing {
            noun: PresetNoun::Flow,
            action: PresetAction::Copy,
        })?;

    let name = copy_name(from, quest.presets.keys().map(String::as_str));
    quest.presets.insert(name.clone(), source);
    quest.active.clone_from(&name);

    Ok(name)
}

/// Duplicates a sequence under the next free generated name and selects it.
///
/// Its entries still name the SAME flows: duplicating a sequence is not a reason for every flow in it
/// to be duplicated too, and pointing a sequence at a copy is one edit in its entry editor.
///
/// # Errors
///
/// Returns [`PresetError::Missing`] when `from` is not a sequence in play — the only refusal, because
/// the generated name is always free.
pub fn duplicate_sequence(quest: &mut QuestConfig, from: &str) -> Result<String, PresetError> {
    let source = quest
        .sequences
        .get(from)
        .cloned()
        .ok_or(PresetError::Missing {
            noun: PresetNoun::Sequence,
            action: PresetAction::Copy,
        })?;

    let name = copy_name(from, quest.sequences.keys().map(String::as_str));
    quest.sequences.insert(name.clone(), source);
    quest.active_sequence.clone_from(&name);

    Ok(name)
}

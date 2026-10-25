//! The Quest tool's config types, ported from `AppConfig.cs`'s `QuestConfig` family.
//!
//! The values are shared with the local overlay (`ConfigLoader.LocalQuest` uses these same classes
//! as its map values, deliberately: "Handing the whole object across leaves no list to forget").
//!
//! Three properties of the v2 types are load-bearing and are kept here:
//!
//! - **A name always has a body.** A hand-edited `run:` with no value arrives as a null body, and
//!   `TryGetValue` then reports the name as PRESENT — that reached five readers in v2 as a
//!   `NullReferenceException`. The decode repairs it to an empty body ([`crate::config`]'s helpers),
//!   so the invariant holds by construction instead of by a `NormaliseNullBodies()` call.
//! - **A missing body is not a fallback.** `active` naming nothing yields no flow rather than
//!   another quest's flow: replaying the wrong dialogue is worse than replaying nothing.
//! - **`active_sequence` empty means "run the single flow"**, which is the shape of every config
//!   written before sequences existed; it must stay a real, expressible choice.

use crate::config::de::{map_with_default_bodies, null_as_default, vec_with_default_bodies};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The five values a quest step's `action:` accepts — the board's whole vocabulary for this tool
/// (`Core/QuestAction.cs`, ported verbatim).
///
/// Stored as a plain `String` rather than an enum because `local.yaml` is hand-editable: an unknown
/// action must load and then be REPORTED by the authoring rule (`QuestAction.Complaint`), not fail
/// the whole config load.
pub mod quest_action {
    /// Left click, at wherever the cursor is. The tool never moves it.
    pub const CLICK: &str = "click";
    /// Right click, at wherever the cursor is.
    pub const RIGHT_CLICK: &str = "right_click";
    /// Press one printable character — a digit or a letter. The firmware's `K` handler.
    pub const KEY: &str = "key";
    /// Enter — the board's own command, not `K` with a newline.
    pub const ENTER: &str = "enter";
    /// Send nothing and wait, for a dialogue beat longer than the others.
    pub const WAIT: &str = "wait";

    /// Every value the setting accepts, in the order the launcher's picker shows them.
    pub const ALL: [&str; 5] = [CLICK, RIGHT_CLICK, KEY, ENTER, WAIT];
}

fn default_quest_active() -> String {
    "default".to_owned()
}

fn default_guard_px() -> i32 {
    8
}

fn default_initial_wait_seconds() -> f64 {
    10.0
}

fn default_loops() -> i32 {
    1
}

fn default_step_action() -> String {
    quest_action::CLICK.to_owned()
}

fn default_step_delay_seconds() -> f64 {
    0.25
}

fn default_times() -> i32 {
    1
}

/// The Quest hand-in tool's configured flows.
///
/// The tool REPLAYS an input sequence and reads nothing: no capture, no OCR, no window. That is why
/// it has no calibration at all — the player puts the mouse where the clicks have to land, and the
/// tool clicks wherever it is. Its entire fragility budget goes on one thing instead: a wrong
/// sequence does the loop count in wrong actions, and the game accepts every one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestConfig {
    /// Which flow the tool runs. Missing → `default`; null → no flow chosen (v2's initializer is
    /// `"default"`, and a null there crashed its lookups, so the tolerant direction is "none").
    #[serde(default = "default_quest_active", deserialize_with = "null_as_default")]
    pub active: String,

    /// Named flows, one per quest. A name's body is never null after the decode.
    #[serde(default, deserialize_with = "map_with_default_bodies")]
    pub presets: BTreeMap<String, QuestPreset>,

    /// Which SEQUENCE to run instead of a single flow. **Empty means "run [`Self::active`] on its
    /// own"**, which is what every config written before sequences says — so this is additive and
    /// nothing has to be migrated.
    #[serde(default, deserialize_with = "null_as_default")]
    pub active_sequence: String,

    /// Named sequences: several flows in one run, for the quests that share an NPC.
    #[serde(default, deserialize_with = "map_with_default_bodies")]
    pub sequences: BTreeMap<String, QuestSequence>,

    /// How far the cursor may move, in the units the process reads it in, before the run STOPS
    /// (the player's rule: "if there is big swing on mouse movement, we stop it").
    ///
    /// Tool-level rather than per flow, because the mouse does not care which quest is being handed
    /// in. The tool reads the cursor ONCE at run start and compares against that, which is why it can
    /// fail CLOSED where the tuner's guard fails open: an unreadable cursor stops this run.
    #[serde(default = "default_guard_px")]
    pub guard_px: i32,
}

impl Default for QuestConfig {
    fn default() -> Self {
        Self {
            active: default_quest_active(),
            presets: BTreeMap::new(),
            active_sequence: String::new(),
            sequences: BTreeMap::new(),
            guard_px: default_guard_px(),
        }
    }
}

/// One quest's flow: how long to wait, how many times, and what to send each time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestPreset {
    /// Seconds to wait before the first input — the whole of "positioning". The tool never moves the
    /// cursor, so this is the player's window to put the mouse where the clicks have to land, exactly
    /// as the tuner's `manual` spring mode does it.
    #[serde(default = "default_initial_wait_seconds")]
    pub initial_wait_seconds: f64,

    /// How many times the step list is replayed.
    #[serde(default = "default_loops")]
    pub loops: i32,

    /// The steps, in order. A bare `steps:` is an empty flow, not a null list a reader must guard.
    #[serde(default, deserialize_with = "vec_with_default_bodies")]
    pub steps: Vec<QuestStep>,
}

impl Default for QuestPreset {
    fn default() -> Self {
        Self {
            initial_wait_seconds: default_initial_wait_seconds(),
            loops: default_loops(),
            steps: Vec::new(),
        }
    }
}

/// Several flows in one run — A three times, then B twice, and the whole list repeated by a master
/// loop. For the quests that share an NPC (`PLAN-QUEST.md` §11).
///
/// EVERY FLOW IN A SEQUENCE CLICKS THE SAME PLACE, and the initial wait belongs to the sequence,
/// ONCE: the tool never moves the cursor, so a flow inside a sequence cannot have a position of its
/// own. There is NO failure detection, deliberately — the tool reads nothing, so it cannot tell a
/// flow that worked from one that did not, and a rule that looked like detection could never fire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestSequence {
    /// Seconds to wait before anything runs — the ONE positioning window for the whole sequence.
    #[serde(default = "default_initial_wait_seconds")]
    pub initial_wait_seconds: f64,

    /// How many times the whole entry list is replayed.
    #[serde(default = "default_loops")]
    pub loops: i32,

    /// Which flow, and how many times in a row, before moving to the next.
    #[serde(default, deserialize_with = "vec_with_default_bodies")]
    pub entries: Vec<QuestSequenceEntry>,
}

impl Default for QuestSequence {
    fn default() -> Self {
        Self {
            initial_wait_seconds: default_initial_wait_seconds(),
            loops: default_loops(),
            entries: Vec::new(),
        }
    }
}

/// One line of a sequence: a flow's NAME (looked up in [`QuestConfig::presets`] at run time), and how
/// many times to replay it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestSequenceEntry {
    /// A name, trimmed before lookup by the runner. Null means no flow chosen — reported by the
    /// authoring rule as a blank name rather than thrown on.
    #[serde(default, deserialize_with = "null_as_default")]
    pub preset: String,

    #[serde(default = "default_times")]
    pub times: i32,
}

impl Default for QuestSequenceEntry {
    fn default() -> Self {
        Self {
            preset: String::new(),
            times: default_times(),
        }
    }
}

/// One input in a flow, and how long to wait after sending it.
///
/// The delay is PER STEP rather than one number for the flow, because a dialogue step that needs
/// 1.5 s sitting next to one that needs 0.2 s is exactly where a single global delay breaks. The
/// default is a placeholder, not a measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestStep {
    /// One of [`quest_action::ALL`]. Ignored by every action except [`quest_action::KEY`] and the
    /// "sends nothing" rule; validated by the authoring half, not by the decode.
    #[serde(default = "default_step_action", deserialize_with = "null_as_default")]
    pub action: String,

    /// The character a `key` step sends. Ignored by the others.
    #[serde(default)]
    pub value: Option<String>,

    #[serde(default = "default_step_delay_seconds")]
    pub delay_seconds: f64,
}

impl Default for QuestStep {
    fn default() -> Self {
        Self {
            action: default_step_action(),
            value: None,
            delay_seconds: default_step_delay_seconds(),
        }
    }
}

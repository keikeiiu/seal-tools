//! Tool identity and published state: the eight ids, the lifecycle phase, and the immutable snapshot
//! a reader can never see half-updated.
//!
//! The torn-read finding (`PROGRESS.md` entries 63 and 68): `ToolState`'s "reads are atomic" comment
//! was false for its `int?` and `DateTime?` members — the 750 ms UI tick could read a half-updated
//! `Remaining`/`NextActionAt` pair and draw a garbage countdown. The comment was wrong because the
//! type permitted it. Here a published value is an `Arc<ToolSnapshot>`: the writer replaces a whole
//! snapshot under a short lock and a reader clones the Arc, so a mixed pair is unrepresentable
//! rather than merely avoided. `PROTOCOL.md` §5's `state` event carries exactly these fields; the
//! core serialises this type into it.

use std::fmt;
use std::sync::{Arc, RwLock};

/// The eight tool ids `PROTOCOL.md` §9 defines, and nothing else — the same list
/// `LauncherService._toolRunners` accepted, which is why "may this start?" and "what runs for it?"
/// can never disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolId {
    Spammer,
    HoldSpace,
    Buy,
    Sell,
    Tuner,
    Gem,
    Quest,
    Pet,
}

impl ToolId {
    /// Every id, in the protocol's order.
    pub const ALL: [ToolId; 8] = [
        Self::Spammer,
        Self::HoldSpace,
        Self::Buy,
        Self::Sell,
        Self::Tuner,
        Self::Gem,
        Self::Quest,
        Self::Pet,
    ];

    /// The id as the protocol spells it (`PROTOCOL.md` §9: `holdspace`, `buy`, `sell`, …).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spammer => "spammer",
            Self::HoldSpace => "holdspace",
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Tuner => "tuner",
            Self::Gem => "gem",
            Self::Quest => "quest",
            Self::Pet => "pet",
        }
    }

    /// Exact, untrimmed lookup: an id is a wire value, not player input.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.as_str() == name)
    }

    /// The one tool that owns a schedule rather than an operation (`LauncherService.ResidentId`):
    /// the pet feeder waits for the game instead of taking it for a whole run.
    pub const fn is_resident(self) -> bool {
        matches!(self, Self::Pet)
    }
}

impl fmt::Display for ToolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where the lifecycle is. `PROGRESS.md` entry 57 is why [`Self::engaged`] exists rather than a
/// second "is it running?" question: the toggle asked *is it running?* three times and each fix
/// closed one window and revealed another; the question with no window is *is it engaged at all?*
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPhase {
    Idle,
    Starting,
    Running,
    Stopping,
}

impl ToolPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
        }
    }

    /// True for everything but [`Self::Idle`]: a press on an engaged tool stops or cancels it, never
    /// starts it again.
    pub const fn engaged(self) -> bool {
        !matches!(self, Self::Idle)
    }
}

impl fmt::Display for ToolPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One row of a tool's standing plan — the pet feeder's four breeding rows, the gem composer's
/// grades. v2's `ToolRowState`, with its `DateTime? NextAt` expressed as seconds from the publish
/// (`countdown_s`), because the crate has no clock/calendar dependency and the reader only needs to
/// count down.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolRow {
    pub name: String,
    /// The short state word the card shows — `boarding`, `waiting for a reload`, `given up`.
    pub status: String,
    /// What the row is doing, when there is something to say (the pet the row boarded, its rate …).
    pub detail: Option<String>,
    /// When this row next needs attention, in seconds from the publish; `None` when never again.
    pub next_action_in_seconds: Option<f64>,
}

impl ToolRow {
    pub fn new(name: impl Into<String>, status: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: status.into(),
            detail: None,
            next_action_in_seconds: None,
        }
    }

    pub fn detail(mut self, detail: Option<impl Into<String>>) -> Self {
        self.detail = detail.map(Into::into);
        self
    }

    pub fn next_action_in_seconds(mut self, seconds: Option<f64>) -> Self {
        self.next_action_in_seconds = seconds;
        self
    }
}

/// One labelled fact on a running card ([PROTOCOL.md] §5 `state.detail`).
///
/// Decided in PLAN-V3-STATUS-DETAIL §5: the core owns both the label and the rendered value (v2's
/// wording ported into the decision halves), a label-less entry is v2's `· name=value` attribute
/// line, and the shell only lays out `label: value` — "no widget keeps a rule". `[]` means "no
/// detail to show", exactly like `rows`.
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDetail {
    /// The core's label (`Grade`, `Remaining`, `Attempt`, `Filter`), or `None` for an attribute line.
    pub label: Option<String>,
    /// The core's rendered value; the shell adds no punctuation.
    pub value: String,
}

impl ToolDetail {
    pub fn labeled(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            value: value.into(),
        }
    }

    pub fn plain(value: impl Into<String>) -> Self {
        Self {
            label: None,
            value: value.into(),
        }
    }
}

/// Everything `PROTOCOL.md` §5's `state` event carries, as one immutable value. Built by replacing
/// whole fields (builder methods consume and return), never by mutating a published instance.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSnapshot {
    pub tool: ToolId,
    /// v2's card boolean: true while the tool is engaged (starting, running or stopping), false when
    /// it is gone. `phase` says which of the three, so a card can render `Starting…`/`● running`/
    /// `Stopping…` without a second field.
    pub running: bool,
    pub phase: ToolPhase,
    /// The last thing that happened, in the tool's own words ("row 2: looking", "DONE — 4 row(s)").
    pub message: String,
    /// v2's `NextActionAt` as seconds remaining at the publish: the card counts down from this.
    pub countdown_s: Option<f64>,
    pub rows: Vec<ToolRow>,
    /// The running card's labelled facts (`Grade: DG`, `Attempt: 7`, `· attack speed=+10`). Empty when
    /// the tool has no detail to show; additive in `v:1`, so an old core's missing field is `[]`.
    pub detail: Vec<ToolDetail>,
    /// A failure worth keeping on the card; `message` may describe what was happening at the time.
    pub error: Option<String>,
}

impl ToolSnapshot {
    /// The idle snapshot for one tool: what a stopped tool publishes.
    pub fn new(tool: ToolId) -> Self {
        Self {
            tool,
            running: false,
            phase: ToolPhase::Idle,
            message: String::new(),
            countdown_s: None,
            rows: Vec::new(),
            detail: Vec::new(),
            error: None,
        }
    }

    /// Engages the tool in `phase` and sets the card boolean from it.
    pub fn running(mut self, phase: ToolPhase) -> Self {
        self.phase = phase;
        self.running = phase.engaged();
        self
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn countdown_s(mut self, countdown_s: Option<f64>) -> Self {
        self.countdown_s = countdown_s;
        self
    }

    pub fn rows(mut self, rows: Vec<ToolRow>) -> Self {
        self.rows = rows;
        self
    }

    pub fn detail(mut self, detail: Vec<ToolDetail>) -> Self {
        self.detail = detail;
        self
    }

    pub fn error(mut self, error: Option<String>) -> Self {
        self.error = error;
        self
    }

    /// The final snapshot of a run that ended on its own or was stopped: idle, not running, and the
    /// reason the player reads.
    pub fn stopped(mut self, message: impl Into<String>) -> Self {
        self.running = false;
        self.phase = ToolPhase::Idle;
        self.message = message.into();
        self
    }
}

/// The published snapshot of one tool: one writer, many readers, and every read is a whole snapshot.
///
/// The writer replaces the current `Arc` under a write lock; a reader takes a READ lock, clones that
/// `Arc`, and releases the lock before it renders anything. The returned value is immutable, so no
/// field can be observed mid-update and a slow reader cannot block another reader.
#[derive(Debug, Clone, Default)]
pub struct StateCell {
    current: Arc<RwLock<Option<Arc<ToolSnapshot>>>>,
}

impl StateCell {
    /// A cell with no snapshot published yet — a tool that has never run. [`StateCell::read`] then
    /// returns `None`.
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn new(initial: ToolSnapshot) -> Self {
        let cell = Self::empty();
        cell.publish(initial);
        cell
    }

    /// Replaces the published snapshot. A panic in a publisher poisons the lock; recovering from the
    /// poison keeps the cell readable for every later reader, which is the whole point of publishing
    /// a value rather than exposing state.
    pub fn publish(&self, snapshot: ToolSnapshot) {
        let mut current = self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *current = Some(Arc::new(snapshot));
    }

    /// The current snapshot, or `None` before the first publish. The returned `Arc` is immutable:
    /// a later publish changes the cell, never a value somebody already holds.
    pub fn read(&self) -> Option<Arc<ToolSnapshot>> {
        self.current
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

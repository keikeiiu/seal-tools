//! The Quest hand-in tool's decision half: what a run will execute, and every refusal and stop that
//! happens before or during it.
//!
//! Ported from `v2/SealTools.Quest/QuestTool.cs`. The tool reads NOTHING — no capture, no OCR, no
//! window lookup — so it has no calibration: the player puts the mouse where the clicks have to land
//! during the positioning wait, the tool clicks wherever the cursor is, and the cursor is then
//! watched so a run STOPS if it moves.
//!
//! The trade that buys, in the plan's words: **a wrong flow does `loops` wrong actions and the game
//! accepts every one of them.** Nothing here can notice that; that is what Test one loop is for,
//! which is why the same plan build serves it (`loops_override = Some(1)`).
//!
//! The step RULES already live in [`crate::authoring::quest_step_complaint`] — this module is the
//! executor around it: resolve what to run, validate it before anything is clicked, and say how far
//! it got when a guard or a stop ends it.

use crate::authoring::quest_step_complaint;
use crate::config::{QuestConfig, QuestPreset, QuestSequence, QuestStep, quest_action};
use crate::decision::StopReason;
use crate::geometry::Point;

/// One thing a step sends to the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestCommand {
    Click,
    RightClick,
    Enter,
    /// The firmware's `K` command: exactly one printable character, validated before the run.
    Key(char),
    /// A wait step: sends nothing and passes its delay.
    NoInput,
}

/// One step, resolved from the config and validated.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestPlanStep {
    pub command: QuestCommand,
    /// Seconds to wait AFTER sending, from the step's own `delay_seconds`.
    pub wait_after_seconds: f64,
    /// The line the card shows for this step (`QuestAction.Describe`).
    pub label: String,
}

/// One pass of a flow's steps. For a single flow the label is empty; inside a sequence it is the
/// `"a 1/3 · "` prefix the card shows.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestPass {
    /// Prefix for the step labels — `""` for a single flow, `"{flow} {pass}/{times} · "` inside a
    /// sequence.
    pub label: String,
    pub flow: String,
    pub steps: Vec<QuestPlanStep>,
}

/// What the run will actually execute.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestPlan {
    /// The card's schedule line, word for word from v2.
    pub schedule: String,
    /// The ONE positioning window, in seconds. For a sequence it belongs to the sequence, once: a
    /// flow's own wait means "when I am run alone".
    pub positioning_seconds: f64,
    /// Master loops. A single flow's `loops` and a sequence's master loops are the same loop shape.
    pub loops: i32,
    /// The passes one master loop iterates, in order.
    pub passes: Vec<QuestPass>,
    pub kind: QuestPlanKind,
    pub guard_px: i32,
}

/// Which shape of run this is, for the completion message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestPlanKind {
    Flow { name: String },
    Sequence { name: String },
}

impl QuestPlan {
    /// The line a finished run reports (`tool.finished.message`), from v2's own log.
    #[must_use]
    pub fn done_message(&self) -> String {
        match &self.kind {
            QuestPlanKind::Flow { name } => {
                format!("DONE — {} loop(s) of \"{name}\"", self.loops)
            }
            QuestPlanKind::Sequence { name } => {
                format!("DONE — {} master loop(s) of \"{name}\"", self.loops)
            }
        }
    }

    /// The positioning countdown: one entry per second, e.g. `3` for a 3 s wait and also for 2.3 s
    /// (v2's `Math.Ceiling`). The card shows "move the mouse into place — {n}s" for each.
    // A configured countdown in seconds; Rust's saturating f64-to-i32 cast matches the C# original on
    // every value a config can hold, and a nonsense value saturates rather than wrapping.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a configured countdown in seconds; the saturating cast is v2's Math.Ceiling conversion"
    )]
    #[must_use]
    pub fn positioning_counts(&self) -> Vec<i32> {
        let mut left = self.positioning_seconds.ceil() as i32;
        let mut counts = Vec::new();
        while left > 0 {
            counts.push(left);
            left -= 1;
        }
        counts
    }
}

/// The positioning line the card shows for one countdown second.
#[must_use]
pub fn positioning_text(seconds_left: i32) -> String {
    format!("move the mouse into place — {seconds_left}s")
}

/// Resolves and validates what the run will execute.
///
/// Order of precedence, and it is v2's: the CARD's sequence choice, then the card's flow choice, then
/// the tab's active sequence, then the tab's single flow. A blank override counts as absent
/// (`IsNullOrWhiteSpace`), and an active sequence that is empty or unknown falls back to the single
/// flow — the shape of every config written before sequences existed.
///
/// Everything is validated BEFORE anything is clicked: a step that sends nothing is
/// indistinguishable from the game behaving differently today, and at a few hundred loops there is no
/// way to tell afterwards.
///
/// # Errors
///
/// Returns [`StopReason::error`] when the chosen sequence or flow does not exist, when a resolved
/// flow has no sendable step, or when no runnable flow/sequence is configured at all — never a
/// half-planned run.
pub fn plan(
    quest: &QuestConfig,
    sequence_override: Option<&str>,
    flow_override: Option<&str>,
    loops_override: Option<i32>,
) -> Result<QuestPlan, StopReason> {
    if let Some(sequence) = sequence_override.filter(|choice| !choice.trim().is_empty()) {
        let name = sequence.trim();
        let Some(body) = quest.sequences.get(name) else {
            return Err(StopReason::error(format!(
                "There is no sequence called \"{name}\" — pick another on the card, or reload the \
                 config."
            )));
        };
        return sequence_plan(quest, body, name, loops_override);
    }

    if let Some(flow) = flow_override.filter(|choice| !choice.trim().is_empty()) {
        let name = flow.trim();
        let Some(preset) = quest.presets.get(name) else {
            return Err(StopReason::error(format!(
                "There is no flow called \"{name}\" — pick another on the card, or reload the \
                 config."
            )));
        };
        let loops = loops_override.unwrap_or_else(|| preset.loops.max(1));
        return flow_plan(quest, preset, name, loops);
    }

    if let Some(sequence) = quest.active_sequence_or_null() {
        let name = quest.active_sequence.trim();
        return sequence_plan(quest, sequence, name, loops_override);
    }

    // The tab's single flow. A missing active preset is an EMPTY preset here, which is exactly what
    // v2's `ActivePreset` returns for that case — and it fails validation with the "no steps" line.
    let name = quest.active.clone();
    let preset = quest.presets.get(&name).cloned().unwrap_or_default();
    let loops = loops_override.unwrap_or_else(|| preset.loops.max(1));
    flow_plan(quest, &preset, &name, loops)
}

/// One flow, on its own.
fn flow_plan(
    quest: &QuestConfig,
    preset: &QuestPreset,
    flow: &str,
    loops: i32,
) -> Result<QuestPlan, StopReason> {
    validate_flow(preset, flow)?;

    Ok(QuestPlan {
        schedule: format!(
            "flow \"{flow}\" · {} step(s) × {loops} loop(s)",
            preset.steps.len()
        ),
        positioning_seconds: preset.initial_wait_seconds,
        loops,
        passes: vec![QuestPass {
            label: String::new(),
            flow: flow.to_owned(),
            steps: plan_steps(preset),
        }],
        kind: QuestPlanKind::Flow {
            name: flow.to_owned(),
        },
        guard_px: quest.guard_px,
    })
}

/// A sequence: A three times, then B twice, and the whole list repeated by a master loop.
///
/// EVERY entry, and every flow it names, is checked BEFORE anything is clicked — a sequence is longer
/// than a flow, so a mistake in it costs more.
fn sequence_plan(
    quest: &QuestConfig,
    sequence: &QuestSequence,
    name: &str,
    loops_override: Option<i32>,
) -> Result<QuestPlan, StopReason> {
    if sequence.entries.is_empty() {
        return Err(StopReason::error(format!(
            "The sequence \"{name}\" has no entries — add some on the Quest tab."
        )));
    }

    let mut planned: Vec<(String, QuestPreset, i32)> = Vec::new();
    for entry in &sequence.entries {
        let flow_name = entry.preset.trim();
        let Some(preset) = quest.presets.get(flow_name) else {
            return Err(StopReason::error(format!(
                "The sequence \"{name}\" names a flow \"{flow_name}\" that does not exist."
            )));
        };
        validate_flow(preset, flow_name)?;
        planned.push((flow_name.to_owned(), preset.clone(), entry.times.max(1)));
    }

    let loops = loops_override.unwrap_or(sequence.loops).max(1);
    let shape = planned
        .iter()
        .map(|(flow, _, times)| format!("{flow}×{times}"))
        .collect::<Vec<_>>()
        .join(" + ");

    let mut passes = Vec::new();
    for (flow, preset, times) in &planned {
        for pass in 1..=*times {
            passes.push(QuestPass {
                label: format!("{flow} {pass}/{times} · "),
                flow: flow.clone(),
                steps: plan_steps(preset),
            });
        }
    }

    Ok(QuestPlan {
        schedule: format!("sequence \"{name}\" · {loops} × ({shape})"),
        positioning_seconds: sequence.initial_wait_seconds,
        loops,
        passes,
        kind: QuestPlanKind::Sequence {
            name: name.to_owned(),
        },
        guard_px: quest.guard_px,
    })
}

/// Why this flow cannot run, or `Ok`. v2's `Validate`: empty step list, then every step's complaint.
fn validate_flow(preset: &QuestPreset, flow: &str) -> Result<(), StopReason> {
    if preset.steps.is_empty() {
        return Err(StopReason::error(format!(
            "The flow \"{flow}\" has no steps — add some on the Quest tab."
        )));
    }

    for (index, step) in preset.steps.iter().enumerate() {
        if let Some(problem) = quest_step_complaint(step) {
            return Err(StopReason::error(format!(
                "Flow \"{flow}\" step {}: {problem}",
                index + 1
            )));
        }
    }

    Ok(())
}

/// A validated flow's steps, resolved to board commands.
fn plan_steps(preset: &QuestPreset) -> Vec<QuestPlanStep> {
    preset.steps.iter().map(plan_step).collect()
}

/// One validated step → its board command, delay and label.
fn plan_step(step: &QuestStep) -> QuestPlanStep {
    let action = step.action.trim().to_lowercase();
    let command = match action.as_str() {
        quest_action::CLICK => QuestCommand::Click,
        quest_action::RIGHT_CLICK => QuestCommand::RightClick,
        quest_action::ENTER => QuestCommand::Enter,
        quest_action::KEY => QuestCommand::Key(
            step.value
                .as_deref()
                .unwrap_or("")
                .chars()
                .next()
                .unwrap_or(' '),
        ),
        _ => QuestCommand::NoInput,
    };

    QuestPlanStep {
        command,
        wait_after_seconds: step.delay_seconds,
        label: step_label(step),
    }
}

/// `QuestAction.Describe`: how a step reads in a log line or on the card.
///
/// `key {value} ({delay}s)`, `wait ({delay}s)`, or `{action with _ as space} ({delay}s)`.
#[must_use]
pub fn step_label(step: &QuestStep) -> String {
    let action = step.action.trim();
    let delay = format_delay(step.delay_seconds);
    if action.eq_ignore_ascii_case(quest_action::KEY) {
        format!("key {} ({delay}s)", step.value.as_deref().unwrap_or(""))
    } else if action.eq_ignore_ascii_case(quest_action::WAIT) {
        format!("wait ({delay}s)")
    } else {
        format!("{} ({delay}s)", action.replace('_', " "))
    }
}

/// v2's `"0.##"`: two decimals at most, trailing zeros trimmed.
fn format_delay(delay: f64) -> String {
    let mut formatted = format!("{delay:.2}");
    if formatted.contains('.') {
        while formatted.ends_with('0') {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            formatted.pop();
        }
    }
    formatted
}

// ── the guard, and the run's two ending lines ────────────────────────────────────────────────

/// The guard's reference, read ONCE per run and read STRICTLY: an unreadable cursor ends the run
/// rather than letting it click on at a place nothing is watching — the difference from the tuner's
/// guard, which can fail open because it may not be able to measure the window.
///
/// `cursor` is the logical cursor position the core reads (DPI-unaware, so both this and every later
/// read are in the same space).
///
/// # Errors
///
/// Returns a guard [`StopReason`] when the cursor could not be read; the run must not start, because
/// nothing would be watching for the mouse being moved.
pub fn guard_reference(cursor: Option<Point>) -> Result<Point, StopReason> {
    cursor.ok_or_else(|| {
        StopReason::error(
            "The cursor couldn't be read, so it can't be watched — stopping rather than clicking at \
             a place nothing is checking.",
        )
    })
}

/// Why the cursor says the run should stop, or `None` when it is where the player left it.
///
/// Checked BEFORE every send: the cost of a missed check is a click at the wrong place, the cost of
/// the check is one cursor read. The threshold is `>`, and both reads are in the same logical space.
#[must_use]
pub fn guard_problem(start: Point, cursor: Option<Point>, guard_px: i32) -> Option<StopReason> {
    let Some(now) = cursor else {
        return Some(StopReason::error(
            "The cursor stopped being readable, so it can't be watched any more.",
        ));
    };

    let moved = (now.x - start.x).abs().max((now.y - start.y).abs());
    (moved > guard_px).then(|| {
        StopReason::guard(format!(
            "The mouse moved — it is at ({},{}) and started at ({},{}).",
            now.x, now.y, start.x, start.y
        ))
    })
}

/// v2's single-flow stop line: the guard's reason, then where the run was.
#[must_use]
pub fn stopped_on_loop(reason: &str, loop_number: i32, loops: i32) -> String {
    format!("{reason} Stopped on loop {loop_number} of {loops}.")
}

/// v2's sequence stop line: the guard's reason, then the flow and the master loop.
#[must_use]
pub fn stopped_in_sequence(
    reason: &str,
    flow: &str,
    pass: i32,
    times: i32,
    master: i32,
    loops: i32,
) -> String {
    format!("{reason} Stopped during {flow} {pass}/{times} of master loop {master}/{loops}.")
}

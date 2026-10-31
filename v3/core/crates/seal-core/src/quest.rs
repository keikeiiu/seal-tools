//! The quest hand-in tool: a pure CONFIG-driven executor with a cursor guard.
//!
//! v2's `QuestTool` reads nothing from the screen — the player positions the mouse during the
//! countdown, the tool clicks wherever the cursor is, and it watches the cursor so a stray move STOPS
//! the run instead of clicking somewhere else. The decision half is `seal-domain`'s
//! (`QuestPlan`, `guard_reference`, `guard_problem`, the stop lines); this module turns the plan into
//! the runner's `Decision`s and holds the loop's position.
//!
//! The one seam is [`QuestSource`]: the cursor read. Everything else is the plan, which is validated
//! BEFORE the first click (a wrong flow does `loops` wrong actions and the game accepts every one).
//!
//! The command stream is v2's: guard → send → wait the step's own `delay_seconds`, per step; passes
//! repeat inside master loops; a guard trip composes v2's "Stopped on loop N of M." / "Stopped during
//! <flow> p/t of master loop m/M." lines. A failed send ends the run with the effects' own sentence.
//! A failed placement never applies: the quest sends only bare C/R/E/K, so it never moves the cursor.

use std::collections::VecDeque;

use seal_board::command::BoardCommand;
use seal_board::transport::Transport;
use seal_domain::config::QuestConfig;
use seal_domain::decision::{Decision, Key, StopKind, StopReason};
use seal_domain::geometry::Point;
use seal_domain::tools::quest::{
    QuestCommand, QuestPlan, QuestPlanKind, guard_problem, guard_reference, plan, positioning_text,
    stopped_in_sequence, stopped_on_loop,
};
use seal_tools::runner::{EffectError, Effects, Tool, ToolStatus};
use seal_tools::tool::ToolId;
use serde_json::Value;

/// The seconds of one positioning countdown tick (v2's `SleepCheck(1)` per printed count).
pub const POSITIONING_TICK_SECONDS: f64 = 1.0;

/// Where the run's cursor read comes from; the production one is `GetCursorPos`.
pub trait QuestSource {
    /// The cursor in the LOGICAL screen space, or `None` when it cannot be read.
    fn cursor(&mut self) -> Option<Point>;
}

/// The production source: the DPI-unaware process's cursor read.
#[cfg(windows)]
#[derive(Debug, Default)]
pub struct WinQuestSource;

#[cfg(windows)]
impl QuestSource for WinQuestSource {
    fn cursor(&mut self) -> Option<Point> {
        seal_win::window::cursor_position()
    }
}

/// Resolve the run's plan out of the merged config, or the line that refuses the start.
pub fn resolve(config: &Value) -> Result<QuestPlan, StopReason> {
    resolve_with(config, None)
}

/// [`resolve`] with an optional master-loop override. `Some(1)` is the Quest tab's **Test one
/// loop**: the same validated plan, run exactly once, so a wrong flow cannot do its whole loop
/// count before the player sees it.
pub fn resolve_with(config: &Value, loops_override: Option<i32>) -> Result<QuestPlan, StopReason> {
    let quest: QuestConfig = match config.get("quest").cloned() {
        Some(block) => serde_json::from_value(block).map_err(|error| {
            StopReason::error(format!(
                "quest is not usable — {error}. Fix it in the Quest tab."
            ))
        })?,
        None => QuestConfig::default(),
    };
    let mut resolved = plan(&quest, None, None, None)?;
    if let Some(loops) = loops_override {
        resolved.loops = loops.max(1);
    }
    Ok(resolved)
}

/// The quest's effects: bare sends only, over the board's command grammar.
pub struct QuestEffects<T: Transport> {
    transport: T,
}

impl<T: Transport> QuestEffects<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    fn send(&mut self, command: BoardCommand) -> Result<(), EffectError> {
        self.transport
            .write_line(&command.encode())
            .map_err(|error| EffectError::new(error.message()))
    }
}

impl<T: Transport> Effects for QuestEffects<T> {
    fn click(&mut self, _at: Point) -> Result<(), EffectError> {
        Err(EffectError::new(
            "the quest tool clicks where the cursor is and never places it",
        ))
    }

    fn key(&mut self, key: Key) -> Result<(), EffectError> {
        let command = match key {
            Key::Char(character) => BoardCommand::Key {
                character,
                fast: false,
            },
            Key::Function(number) => BoardCommand::FunctionKey {
                number,
                fast: false,
            },
        };
        self.send(command)
    }

    fn enter(&mut self) -> Result<(), EffectError> {
        self.send(BoardCommand::Enter)
    }

    fn click_here(&mut self) -> Result<(), EffectError> {
        self.send(BoardCommand::LeftClick)
    }

    fn right_click_here(&mut self) -> Result<(), EffectError> {
        self.send(BoardCommand::RightClick)
    }
}

/// The run's position in the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The first cursor read: it fails CLOSED, because clicking at an unwatched place is the one
    /// thing this tool must not do.
    GuardReference,
    Positioning(usize),
    Steps,
    Done,
}

/// The quest run as the runner sees it.
pub struct QuestTool<S: QuestSource> {
    plan: QuestPlan,
    source: S,
    start: Option<Point>,
    phase: Phase,
    queue: VecDeque<Decision>,
    message: String,
    loop_index: i32,
    pass_index: usize,
    step_index: usize,
}

impl<S: QuestSource> QuestTool<S> {
    pub fn new(plan: QuestPlan, source: S) -> Self {
        Self {
            plan,
            source,
            start: None,
            phase: Phase::GuardReference,
            queue: VecDeque::new(),
            message: String::new(),
            loop_index: 0,
            pass_index: 0,
            step_index: 0,
        }
    }

    /// `(flow, pass number, how many passes of that flow)`, for v2's sequence stop line.
    fn sequence_position(&self) -> (String, i32, i32) {
        let passes = &self.plan.passes;
        let current = passes[self.pass_index].flow.clone();
        let mut start = self.pass_index;
        while start > 0 && passes[start - 1].flow == current {
            start -= 1;
        }
        let mut end = start;
        while end < passes.len() && passes[end].flow == current {
            end += 1;
        }
        (
            current,
            (self.pass_index - start + 1) as i32,
            (end - start) as i32,
        )
    }

    /// The guard check before one send, as the line v2 would stop with.
    fn guard_trip(&mut self) -> Option<StopReason> {
        let start = self.start?;
        let problem = guard_problem(start, self.source.cursor(), self.plan.guard_px)?;
        let loops = self.plan.loops.max(1);
        let message = match &self.plan.kind {
            QuestPlanKind::Flow { .. } => {
                stopped_on_loop(&problem.message, self.loop_index + 1, loops)
            }
            QuestPlanKind::Sequence { .. } => {
                let (flow, pass, times) = self.sequence_position();
                stopped_in_sequence(
                    &problem.message,
                    &flow,
                    pass,
                    times,
                    self.loop_index + 1,
                    loops,
                )
            }
        };
        Some(StopReason::new(problem.kind, message))
    }
}

impl<S: QuestSource + Send> Tool for QuestTool<S> {
    fn id(&self) -> ToolId {
        ToolId::Quest
    }

    fn status(&self) -> ToolStatus {
        ToolStatus {
            message: self.message.clone(),
            countdown_s: None,
            rows: Vec::new(),
            detail: Vec::new(),
        }
    }

    fn decide(&mut self) -> Decision {
        if let Some(decision) = self.queue.pop_front() {
            return decision;
        }

        loop {
            match self.phase {
                Phase::GuardReference => match guard_reference(self.source.cursor()) {
                    Ok(start) => {
                        self.start = Some(start);
                        self.phase = Phase::Positioning(0);
                    }
                    Err(reason) => {
                        self.phase = Phase::Done;
                        return Decision::Stop(reason);
                    }
                },
                Phase::Positioning(index) => {
                    let counts = self.plan.positioning_counts();
                    if index < counts.len() {
                        self.message = positioning_text(counts[index]);
                        self.phase = Phase::Positioning(index + 1);
                        return Decision::Wait(POSITIONING_TICK_SECONDS);
                    }
                    self.phase = Phase::Steps;
                }
                Phase::Steps => {
                    let loops = self.plan.loops.max(1);
                    if self.loop_index >= loops {
                        self.phase = Phase::Done;
                        return Decision::Stop(StopReason::done(self.plan.done_message()));
                    }
                    if self.pass_index >= self.plan.passes.len() {
                        self.loop_index += 1;
                        self.pass_index = 0;
                        self.step_index = 0;
                        continue;
                    }
                    let pass_len = self.plan.passes[self.pass_index].steps.len();
                    if self.step_index >= pass_len {
                        self.pass_index += 1;
                        self.step_index = 0;
                        continue;
                    }
                    // Clone the step's data before the guard: the guard reads the source and may end
                    // the run, so no borrow of the plan may be alive across it.
                    let (pass_label, step) = {
                        let pass = &self.plan.passes[self.pass_index];
                        (pass.label.clone(), pass.steps[self.step_index].clone())
                    };
                    if let Some(reason) = self.guard_trip() {
                        self.phase = Phase::Done;
                        return Decision::Stop(reason);
                    }
                    self.message = format!("{pass_label}{}", step.label);
                    self.step_index += 1;

                    if step.command == QuestCommand::NoInput {
                        if step.wait_after_seconds > 0.0 {
                            return Decision::Wait(step.wait_after_seconds);
                        }
                        continue;
                    }

                    if step.wait_after_seconds > 0.0 {
                        self.queue
                            .push_back(Decision::Wait(step.wait_after_seconds));
                    }
                    return match step.command {
                        QuestCommand::Click => Decision::ClickHere,
                        QuestCommand::RightClick => Decision::RightClickHere,
                        QuestCommand::Enter => Decision::Enter,
                        QuestCommand::Key(character) => Decision::Key(Key::Char(character)),
                        QuestCommand::NoInput => Decision::Wait(step.wait_after_seconds),
                    };
                }
                Phase::Done => {
                    return Decision::Stop(StopReason::new(
                        StopKind::Done,
                        self.plan.done_message(),
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scripted cursor: each call takes the next scripted read, repeating the last one.
    struct ScriptedCursor {
        reads: Vec<Option<Point>>,
        index: usize,
    }

    impl ScriptedCursor {
        fn always(point: Option<Point>) -> Self {
            Self {
                reads: vec![point],
                index: 0,
            }
        }

        fn sequence(reads: Vec<Option<Point>>) -> Self {
            Self { reads, index: 0 }
        }
    }

    impl QuestSource for ScriptedCursor {
        fn cursor(&mut self) -> Option<Point> {
            let read = self
                .reads
                .get(self.index.min(self.reads.len().saturating_sub(1)))
                .copied()
                .flatten();
            self.index += 1;
            read
        }
    }

    fn config(steps: Value, loops: i32) -> Value {
        serde_json::json!({
            "quest": {
                "active": "run1",
                "guard_px": 8,
                "presets": {
                    "run1": {
                        "initial_wait_seconds": 1.0,
                        "loops": loops,
                        "steps": steps,
                    }
                }
            }
        })
    }

    fn simple_steps() -> Value {
        serde_json::json!([
            {"action": "click", "delay_seconds": 0.2},
            {"action": "key", "value": "1", "delay_seconds": 0.1},
            {"action": "enter"},
            {"action": "right_click"},
        ])
    }

    fn run(config: Value, source: ScriptedCursor) -> QuestTool<ScriptedCursor> {
        let plan = resolve(&config).expect("the plan resolves");
        QuestTool::new(plan, source)
    }

    #[test]
    fn the_plan_resolves_the_flow_schedule_and_labels() {
        let plan = resolve(&config(simple_steps(), 2)).expect("a plan");

        assert!(plan.schedule.contains("flow \"run1\""), "{}", plan.schedule);
        assert_eq!(plan.loops, 2);
        assert_eq!(plan.positioning_seconds, 1.0);
        assert_eq!(plan.positioning_counts(), vec![1]);
        assert_eq!(plan.passes[0].steps.len(), 4);
    }

    #[test]
    fn a_flow_with_no_steps_is_refused_with_the_v2_line() {
        let reason = resolve(&config(serde_json::json!([]), 1)).expect_err("no steps");

        assert!(
            reason.message.contains("has no steps"),
            "{}",
            reason.message
        );
    }

    #[test]
    fn positioning_runs_once_then_the_steps_send_in_order() {
        let mut tool = run(
            config(simple_steps(), 1),
            ScriptedCursor::always(Some(Point::new(5, 5))),
        );

        assert_eq!(tool.decide(), Decision::Wait(POSITIONING_TICK_SECONDS));
        assert_eq!(tool.status().message, positioning_text(1));

        // One master loop of the four steps: click, wait, key, wait, enter, wait, right click, wait,
        // done. The steps with no explicit delay carry v2's default 0.25 s, so they wait too.
        assert_eq!(tool.decide(), Decision::ClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.2));
        assert_eq!(tool.decide(), Decision::Key(Key::Char('1')));
        assert_eq!(tool.decide(), Decision::Wait(0.1));
        assert_eq!(tool.decide(), Decision::Enter);
        assert_eq!(tool.decide(), Decision::Wait(0.25));
        assert_eq!(tool.decide(), Decision::RightClickHere);
        assert_eq!(tool.decide(), Decision::Wait(0.25));
        match tool.decide() {
            Decision::Stop(reason) => {
                assert_eq!(reason.kind, StopKind::Done);
                assert!(reason.message.contains("1 loop(s)"), "{}", reason.message);
            }
            other => panic!("expected done, got {other:?}"),
        }
    }

    #[test]
    fn test_one_loop_overrides_the_configured_count() {
        let plan = resolve_with(&config(simple_steps(), 5), Some(1)).expect("resolves");
        assert_eq!(
            plan.loops, 1,
            "Test one loop runs the same plan exactly once"
        );
    }

    #[test]
    fn loops_repeat_the_pass_the_configured_number_of_times() {
        let mut tool = run(
            config(simple_steps(), 2),
            ScriptedCursor::always(Some(Point::new(5, 5))),
        );

        tool.decide(); // positioning
        let mut clicks = 0;
        let mut done = None;
        for _ in 0..40 {
            match tool.decide() {
                Decision::ClickHere => clicks += 1,
                Decision::Stop(reason) => {
                    done = Some(reason);
                    break;
                }
                _ => {}
            }
        }

        assert_eq!(clicks, 2, "one click per loop");
        assert!(
            done.expect("done").message.contains("2 loop(s)"),
            "the completion line counts the loops"
        );
    }

    #[test]
    fn an_unreadable_cursor_at_start_stops_before_anything_is_clicked() {
        let mut tool = run(config(simple_steps(), 1), ScriptedCursor::always(None));

        match tool.decide() {
            Decision::Stop(reason) => {
                assert_eq!(reason.kind, StopKind::Error);
                assert!(
                    reason.message.contains("cursor couldn't be read"),
                    "{}",
                    reason.message
                );
            }
            other => panic!("expected the guard reference failure, got {other:?}"),
        }
    }

    #[test]
    fn a_cursor_that_moves_mid_run_stops_with_the_loop_line() {
        let source = ScriptedCursor::sequence(vec![
            Some(Point::new(5, 5)),   // guard reference
            Some(Point::new(5, 5)),   // before the first step: still in place
            Some(Point::new(50, 50)), // before the third step: drifted
        ]);
        let mut tool = run(config(simple_steps(), 1), source);

        tool.decide(); // positioning
        assert_eq!(tool.decide(), Decision::ClickHere);
        tool.decide(); // the click's wait
        // The next step's guard reads the drifted cursor.
        match tool.decide() {
            Decision::Stop(reason) => {
                assert_eq!(reason.kind, StopKind::Guard);
                assert!(
                    reason.message.contains("Stopped on loop 1 of 1."),
                    "{}",
                    reason.message
                );
                assert!(
                    reason.message.contains("The mouse moved"),
                    "{}",
                    reason.message
                );
            }
            other => panic!("expected the guard stop, got {other:?}"),
        }
    }

    #[test]
    fn a_sequence_runs_each_flow_its_number_of_passes() {
        let config = serde_json::json!({
            "quest": {
                "active": "run1",
                "guard_px": 8,
                "presets": {
                    "run1": {"initial_wait_seconds": 1.0, "loops": 1, "steps": [{"action": "click"}]},
                    "run2": {"initial_wait_seconds": 1.0, "loops": 1, "steps": [{"action": "enter"}]}
                },
                "active_sequence": "seq1",
                "sequences": {
                    "seq1": {
                        "initial_wait_seconds": 1.0,
                        "loops": 1,
                        "entries": [
                            {"preset": "run1", "times": 2},
                            {"preset": "run2", "times": 1}
                        ]
                    }
                }
            }
        });
        let plan = resolve(&config).expect("the sequence resolves");
        assert_eq!(plan.passes.len(), 3, "two passes of run1 then one of run2");

        let mut tool = QuestTool::new(plan, ScriptedCursor::always(Some(Point::new(0, 0))));
        tool.decide(); // positioning
        let mut sends = Vec::new();
        for _ in 0..20 {
            match tool.decide() {
                Decision::ClickHere => sends.push("click"),
                Decision::Enter => sends.push("enter"),
                Decision::Stop(reason) => {
                    assert!(reason.message.contains("master loop"), "{}", reason.message);
                    break;
                }
                _ => {}
            }
        }
        assert_eq!(sends, vec!["click", "click", "enter"]);
    }
}

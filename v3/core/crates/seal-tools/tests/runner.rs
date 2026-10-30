//! The runner seam, driven end to end by a fake tool, fake effects and a fake clock.
//!
//! This is the test v2 could never write (`PROGRESS.md` entry 59): a whole tool run — decisions,
//! effects, a stop during a wait, a failed effect — with no board, no game and no real time. The
//! runner is pure except for the `Effects`/`Clock` it is handed.

use std::collections::VecDeque;
use std::sync::Mutex;

use seal_domain::decision::{Decision, Key, StopKind, StopReason};
use seal_domain::geometry::Point;
use seal_tools::arbiter::Arbiter;
use seal_tools::clock::Clock;
use seal_tools::runner::{
    EFFECT_FAILED_MESSAGE, EffectError, Effects, STOPPED_MESSAGE, Tool, ToolStatus, run,
};
use seal_tools::tool::{StateCell, ToolId, ToolPhase};

struct ScriptedTool {
    id: ToolId,
    script: VecDeque<Decision>,
}

impl ScriptedTool {
    fn new(script: Vec<Decision>) -> Self {
        Self {
            id: ToolId::Tuner,
            script: script.into(),
        }
    }
}

impl Tool for ScriptedTool {
    fn id(&self) -> ToolId {
        self.id
    }

    fn decide(&mut self) -> Decision {
        self.script
            .pop_front()
            .unwrap_or_else(|| Decision::stop(StopReason::done("script ended")))
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Click(Point),
    Key(Key),
}

#[derive(Default)]
struct RecordingEffects {
    calls: Mutex<Vec<Call>>,
    fail_click: Option<&'static str>,
}

impl RecordingEffects {
    fn calls(&self) -> Vec<Call> {
        self.calls.lock().expect("not poisoned").clone()
    }
}

impl Effects for RecordingEffects {
    fn click(&mut self, at: Point) -> Result<(), EffectError> {
        self.calls
            .lock()
            .expect("not poisoned")
            .push(Call::Click(at));
        match self.fail_click {
            Some(message) => Err(EffectError::new(message)),
            None => Ok(()),
        }
    }

    fn key(&mut self, key: Key) -> Result<(), EffectError> {
        self.calls
            .lock()
            .expect("not poisoned")
            .push(Call::Key(key));
        Ok(())
    }
}

/// A tool whose status changes with each decision, plus effects that read the PUBLISHED cell when they
/// run — so the test observes exactly what a shell would have received, in order.
struct StatusTool {
    step: usize,
}

impl Tool for StatusTool {
    fn id(&self) -> ToolId {
        ToolId::Tuner
    }

    fn decide(&mut self) -> Decision {
        self.step += 1;
        match self.step {
            1 => Decision::click(point(1, 2)),
            2 => Decision::key('a').unwrap(),
            _ => Decision::stop(StopReason::done("done")),
        }
    }

    fn status(&self) -> ToolStatus {
        match self.step {
            0 => ToolStatus::default(),
            1 => ToolStatus {
                message: "Attempt 1".to_owned(),
                countdown_s: Some(3.0),
                rows: Vec::new(),
                detail: Vec::new(),
            },
            _ => ToolStatus {
                message: "filter disabled".to_owned(),
                countdown_s: None,
                rows: Vec::new(),
                detail: Vec::new(),
            },
        }
    }
}

/// Records the message of the published snapshot at the moment each effect runs.
#[derive(Default)]
struct StatusObservingEffects {
    seen: Mutex<Vec<String>>,
    state: StateCell,
}

impl Effects for StatusObservingEffects {
    fn click(&mut self, _at: Point) -> Result<(), EffectError> {
        self.observe();
        Ok(())
    }

    fn key(&mut self, _key: Key) -> Result<(), EffectError> {
        self.observe();
        Ok(())
    }
}

impl StatusObservingEffects {
    fn observe(&mut self) {
        let message = self
            .state
            .read()
            .map(|snapshot| format!("{}|{:?}", snapshot.message, snapshot.countdown_s))
            .unwrap_or_else(|| "none".to_owned());
        self.seen.lock().expect("not poisoned").push(message);
    }
}

#[derive(Default)]
struct RecordingClock {
    slept: Mutex<Vec<f64>>,
}

impl RecordingClock {
    fn total(&self) -> f64 {
        self.slept.lock().expect("not poisoned").iter().sum()
    }

    fn ticks(&self) -> usize {
        self.slept.lock().expect("not poisoned").len()
    }
}

impl Clock for RecordingClock {
    fn sleep_seconds(&self, seconds: f64) {
        self.slept.lock().expect("not poisoned").push(seconds);
    }
}

fn point(x: i32, y: i32) -> Point {
    Point::new(x, y)
}

#[test]
fn a_tool_status_is_published_before_each_decision_runs() {
    let mut arbiter = Arbiter::new();
    let handle = match arbiter.start(ToolId::Tuner) {
        seal_tools::arbiter::Start::Started(handle) => handle,
        other => panic!("expected a start, got {other:?}"),
    };
    let mut tool = StatusTool { step: 0 };
    let mut effects = StatusObservingEffects {
        seen: Mutex::new(Vec::new()),
        state: handle.state.clone(),
    };
    let clock = RecordingClock::default();

    let reason = run(&mut tool, &handle, &clock, &mut effects);

    assert_eq!(reason.message, "done");
    assert_eq!(
        effects.seen.lock().expect("not poisoned").clone(),
        vec![
            "Attempt 1|Some(3.0)".to_owned(),
            "filter disabled|None".to_owned()
        ],
        "the shell sees each status as the whole published snapshot, before its decision runs"
    );
}

#[test]
fn the_loop_performs_the_decisions_in_order() {
    let mut arbiter = Arbiter::new();
    let handle = match arbiter.start(ToolId::Tuner) {
        seal_tools::arbiter::Start::Started(handle) => handle,
        other => panic!("expected a start, got {other:?}"),
    };
    let mut tool = ScriptedTool::new(vec![
        Decision::click(point(10, 20)),
        Decision::key('a').unwrap(),
        Decision::wait(0.1),
        Decision::stop(StopReason::done("DONE — 4 row(s)")),
    ]);
    let clock = RecordingClock::default();
    let mut effects = RecordingEffects::default();

    let reason = run(&mut tool, &handle, &clock, &mut effects);

    assert_eq!(reason.kind, StopKind::Done);
    assert_eq!(reason.message, "DONE — 4 row(s)");
    assert_eq!(
        effects.calls(),
        vec![Call::Click(point(10, 20)), Call::Key(Key::Char('a'))]
    );
    assert!((clock.total() - 0.1).abs() < 1e-9);
}

#[test]
fn the_loop_publishes_running_before_the_first_decision() {
    let mut arbiter = Arbiter::new();
    let handle = match arbiter.start(ToolId::Tuner) {
        seal_tools::arbiter::Start::Started(handle) => handle,
        other => panic!("expected a start, got {other:?}"),
    };
    let mut tool = ScriptedTool::new(vec![Decision::stop(StopReason::done("done"))]);

    let _ = run(
        &mut tool,
        &handle,
        &RecordingClock::default(),
        &mut RecordingEffects::default(),
    );

    let snapshot = handle.state.read().expect("published");
    assert_eq!(snapshot.tool, ToolId::Tuner);
    assert_eq!(snapshot.phase, ToolPhase::Running);
    assert!(snapshot.running);
}

#[test]
fn a_run_stopped_before_it_starts_does_no_work() {
    let mut arbiter = Arbiter::new();
    let handle = match arbiter.start(ToolId::Tuner) {
        seal_tools::arbiter::Start::Started(handle) => handle,
        other => panic!("expected a start, got {other:?}"),
    };
    let _ = arbiter.stop(Some(ToolId::Tuner));
    let mut tool = ScriptedTool::new(vec![
        Decision::click(point(1, 1)),
        Decision::click(point(2, 2)),
    ]);
    let clock = RecordingClock::default();
    let mut effects = RecordingEffects::default();

    let reason = run(&mut tool, &handle, &clock, &mut effects);

    assert_eq!(reason.kind, StopKind::Cancelled);
    assert_eq!(reason.message, STOPPED_MESSAGE);
    assert!(effects.calls().is_empty(), "the work was abandoned");
    assert_eq!(clock.ticks(), 0, "and no wait was paid");
}

#[test]
fn a_stop_during_a_wait_abandons_the_wait_rather_than_finishing_it() {
    // The pet feeder's ~30 s reload, in a test: the token is cancelled during the first tick, and
    // the loop leaves immediately instead of sleeping the whole wait out.
    struct CancellingClock {
        handle_token: seal_tools::cancel::CancelToken,
        slept: Mutex<Vec<f64>>,
    }
    impl Clock for CancellingClock {
        fn sleep_seconds(&self, seconds: f64) {
            self.slept.lock().expect("not poisoned").push(seconds);
            self.handle_token.cancel();
        }
    }

    let mut arbiter = Arbiter::new();
    let handle = match arbiter.start(ToolId::Pet) {
        seal_tools::arbiter::Start::Started(handle) => handle,
        other => panic!("expected a start, got {other:?}"),
    };
    let clock = CancellingClock {
        handle_token: handle.token.clone(),
        slept: Mutex::new(Vec::new()),
    };
    let mut tool = ScriptedTool::new(vec![Decision::wait(30.0), Decision::click(point(1, 1))]);
    let mut effects = RecordingEffects::default();

    let reason = run(&mut tool, &handle, &clock, &mut effects);

    assert_eq!(reason.kind, StopKind::Cancelled);
    assert_eq!(clock.slept.lock().expect("not poisoned").len(), 1);
    assert!(effects.calls().is_empty());
}

#[test]
fn an_effect_failure_ends_the_run_as_an_error_with_its_line() {
    let mut arbiter = Arbiter::new();
    let handle = match arbiter.start(ToolId::Tuner) {
        seal_tools::arbiter::Start::Started(handle) => handle,
        other => panic!("expected a start, got {other:?}"),
    };
    let mut tool = ScriptedTool::new(vec![
        Decision::click(point(1, 1)),
        Decision::click(point(2, 2)),
    ]);
    let mut effects = RecordingEffects {
        calls: Mutex::new(Vec::new()),
        fail_click: Some("the board is not open"),
    };

    let reason = run(&mut tool, &handle, &RecordingClock::default(), &mut effects);

    assert_eq!(reason.kind, StopKind::Error);
    assert_eq!(reason.message, "the board is not open");
    assert_eq!(
        effects.calls().len(),
        1,
        "the run does not carry on after a failed effect"
    );
}

#[test]
fn an_effect_failure_with_no_message_gets_the_defined_line() {
    let mut arbiter = Arbiter::new();
    let handle = match arbiter.start(ToolId::Tuner) {
        seal_tools::arbiter::Start::Started(handle) => handle,
        other => panic!("expected a start, got {other:?}"),
    };
    let mut tool = ScriptedTool::new(vec![Decision::click(point(1, 1))]);
    let mut effects = RecordingEffects {
        calls: Mutex::new(Vec::new()),
        fail_click: Some(""),
    };

    let reason = run(&mut tool, &handle, &RecordingClock::default(), &mut effects);

    assert_eq!(reason.kind, StopKind::Error);
    assert_eq!(reason.message, EFFECT_FAILED_MESSAGE);
}

#[test]
fn a_stop_decision_ends_the_run_with_its_own_reason() {
    let mut arbiter = Arbiter::new();
    let handle = match arbiter.start(ToolId::Tuner) {
        seal_tools::arbiter::Start::Started(handle) => handle,
        other => panic!("expected a start, got {other:?}"),
    };
    let mut tool = ScriptedTool::new(vec![Decision::stop(StopReason::guard(
        "the cursor moved — stopped",
    ))]);

    let reason = run(
        &mut tool,
        &handle,
        &RecordingClock::default(),
        &mut RecordingEffects::default(),
    );

    assert_eq!(reason.kind, StopKind::Guard);
    assert_eq!(reason.message, "the cursor moved — stopped");
}

#[test]
fn the_stop_is_checked_between_decisions_too() {
    // A tool that keeps returning decisions must still be stopped by a token cancelled between two
    // of them — the check is not only at the top of the run.
    struct CancellingEffects {
        token: seal_tools::cancel::CancelToken,
        clicks: usize,
    }
    impl Effects for CancellingEffects {
        fn click(&mut self, _at: Point) -> Result<(), EffectError> {
            self.clicks += 1;
            self.token.cancel();
            Ok(())
        }
        fn key(&mut self, _key: Key) -> Result<(), EffectError> {
            Ok(())
        }
    }

    let mut arbiter = Arbiter::new();
    let handle = match arbiter.start(ToolId::Spammer) {
        seal_tools::arbiter::Start::Started(handle) => handle,
        other => panic!("expected a start, got {other:?}"),
    };
    let script: Vec<Decision> = (0..5).map(|i| Decision::click(point(i, i))).collect();
    let mut tool = ScriptedTool::new(script);
    let mut effects = CancellingEffects {
        token: handle.token.clone(),
        clicks: 0,
    };

    let reason = run(&mut tool, &handle, &RecordingClock::default(), &mut effects);

    assert_eq!(reason.kind, StopKind::Cancelled);
    assert_eq!(effects.clicks, 1, "the next decision was never asked for");
}

#[test]
fn the_arbiter_publishes_the_runners_reason_as_the_final_snapshot() {
    // The seam between the two increments: the runner returns the reason, the arbiter publishes it
    // and clears the slot — one place owns the final word, so reason and card cannot disagree.
    let mut arbiter = Arbiter::new();
    let handle = match arbiter.start(ToolId::Quest) {
        seal_tools::arbiter::Start::Started(handle) => handle,
        other => panic!("expected a start, got {other:?}"),
    };
    let mut tool = ScriptedTool::new(vec![Decision::stop(StopReason::done("DONE — 20 loop(s)"))]);

    let reason = run(
        &mut tool,
        &handle,
        &RecordingClock::default(),
        &mut RecordingEffects::default(),
    );
    let finish = arbiter.finished(ToolId::Quest, reason);

    let snapshot = finish.final_snapshot.expect("a final snapshot");
    assert_eq!(snapshot.message, "DONE — 20 loop(s)");
    assert_eq!(snapshot.phase, ToolPhase::Idle);
    assert!(!arbiter.is_running(ToolId::Quest));
    assert_eq!(arbiter.game_owner(), None);
}

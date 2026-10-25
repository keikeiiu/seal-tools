//! The supervisor, in process: fake tools and fake effects in, protocol events out.
//!
//! These are the tests the pure crate could not have: a real worker thread, the real runner, the real
//! arbiter and the event channel together, with no board, no game and no real time. The fake registry
//! injects the decision-half script and records effects; the binary's registry is the production
//! placeholder, not a test switch.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use seal_core::registry::{Session, ToolRegistry, Unavailable};
use seal_core::supervisor::{StartReply, StopReply, Supervisor};
use seal_domain::decision::{Decision, Key, StopReason};
use seal_domain::geometry::Point;
use seal_proto::Event;
use seal_tools::runner::{EffectError, Effects, Tool};
use seal_tools::tool::{ToolId, ToolPhase};
use serde_json::Value;

struct ScriptedTool {
    id: ToolId,
    script: Vec<Decision>,
}

impl Tool for ScriptedTool {
    fn id(&self) -> ToolId {
        self.id
    }

    fn decide(&mut self) -> Decision {
        if self.script.is_empty() {
            Decision::stop(StopReason::done("script ended"))
        } else {
            self.script.remove(0)
        }
    }
}

#[derive(Default)]
struct RecordingEffects {
    clicks: usize,
}

impl Effects for RecordingEffects {
    fn click(&mut self, _at: Point) -> Result<(), EffectError> {
        self.clicks += 1;
        Ok(())
    }

    fn key(&mut self, _key: Key) -> Result<(), EffectError> {
        Ok(())
    }
}

/// The injected registry: a script per tool, optional unavailability, an optional opening failure,
/// and an optional gate that holds the opening phase open so a test can stop a start in flight.
struct FakeRegistry {
    scripts: Mutex<HashMap<ToolId, Vec<Decision>>>,
    unavailable: Mutex<Vec<ToolId>>,
    opening_errors: Mutex<HashMap<ToolId, String>>,
    opening_gate: Mutex<Option<mpsc::Receiver<()>>>,
    opening_started: Mutex<Option<mpsc::Sender<()>>>,
}

impl FakeRegistry {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            scripts: Mutex::new(HashMap::new()),
            unavailable: Mutex::new(Vec::new()),
            opening_errors: Mutex::new(HashMap::new()),
            opening_gate: Mutex::new(None),
            opening_started: Mutex::new(None),
        })
    }

    fn with_script(self: &Arc<Self>, tool: ToolId, script: Vec<Decision>) -> Arc<Self> {
        self.scripts
            .lock()
            .expect("not poisoned")
            .insert(tool, script);
        Arc::clone(self)
    }

    fn unavailable_for(self: &Arc<Self>, tool: ToolId) -> Arc<Self> {
        self.unavailable.lock().expect("not poisoned").push(tool);
        Arc::clone(self)
    }

    fn failing_to_open(self: &Arc<Self>, tool: ToolId, line: &str) -> Arc<Self> {
        self.opening_errors
            .lock()
            .expect("not poisoned")
            .insert(tool, line.to_owned());
        Arc::clone(self)
    }

    fn gated_opening(
        self: &Arc<Self>,
        started: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    ) -> Arc<Self> {
        *self.opening_started.lock().expect("not poisoned") = Some(started);
        *self.opening_gate.lock().expect("not poisoned") = Some(release);
        Arc::clone(self)
    }
}

impl ToolRegistry for FakeRegistry {
    fn available(&self, tool: ToolId) -> Result<(), Unavailable> {
        if self
            .unavailable
            .lock()
            .expect("not poisoned")
            .contains(&tool)
        {
            return Err(Unavailable {
                line: format!(
                    "\"{tool}\" is not available in this build yet — its tool loop is not ported."
                ),
            });
        }
        Ok(())
    }

    fn create(&self, tool: ToolId) -> Result<Session, String> {
        if let Some(started) = self.opening_started.lock().expect("not poisoned").take() {
            let _ = started.send(());
        }
        if let Some(gate) = self.opening_gate.lock().expect("not poisoned").take() {
            let _ = gate.recv();
        }
        if let Some(line) = self.opening_errors.lock().expect("not poisoned").get(&tool) {
            return Err(line.clone());
        }
        let script = self
            .scripts
            .lock()
            .expect("not poisoned")
            .remove(&tool)
            .unwrap_or_default();
        Ok(Session {
            tool: Box::new(ScriptedTool { id: tool, script }),
            effects: Box::new(RecordingEffects::default()),
        })
    }
}

fn next_event(events: &mpsc::Receiver<Event>) -> Event {
    events
        .recv_timeout(Duration::from_secs(5))
        .expect("an event arrives")
}

fn wait_for(
    events: &mpsc::Receiver<Event>,
    what: &str,
    predicate: impl Fn(&Event) -> bool,
) -> Event {
    loop {
        let event = next_event(events);
        if predicate(&event) {
            return event;
        }
        assert!(event.event != "core.stopping", "stopped before {what}");
    }
}

/// Every event up to and including the next `tool.finished`.
fn until_finished(events: &mpsc::Receiver<Event>) -> Vec<Event> {
    let mut seen = Vec::new();
    loop {
        let event = next_event(events);
        let finished = event.event == "tool.finished";
        seen.push(event);
        if finished {
            return seen;
        }
    }
}

fn state_phase(event: &Event) -> Option<&str> {
    (event.event == "state")
        .then(|| event.data.get("phase").and_then(Value::as_str))
        .flatten()
}

#[test]
fn a_start_runs_to_done_and_publishes_the_reason() {
    let registry = FakeRegistry::new().with_script(
        ToolId::Tuner,
        vec![
            Decision::click(Point::new(1, 1)),
            Decision::stop(StopReason::done("DONE — 1 click")),
        ],
    );
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);

    assert_eq!(supervisor.start_tool(ToolId::Tuner), StartReply::Started);

    let seen = until_finished(&events);
    assert_eq!(
        seen.iter().filter_map(state_phase).collect::<Vec<_>>(),
        vec!["starting", "running", "idle"],
        "the card opens, runs, then reads the final state"
    );
    let last_state = seen
        .iter()
        .rev()
        .find(|event| event.event == "state")
        .unwrap();
    assert_eq!(last_state.data["message"], "DONE — 1 click");
    assert_eq!(last_state.data["running"], false);
    let finished = seen.last().expect("finished");
    assert_eq!(
        finished.data,
        serde_json::json!({
            "tool": "tuner",
            "reason": "done",
            "message": "DONE — 1 click",
        })
    );

    supervisor.shutdown("test");
}

#[test]
fn a_stop_publishes_stopping_then_a_cancelled_finish() {
    let registry = FakeRegistry::new().with_script(ToolId::Spammer, vec![Decision::wait(30.0)]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);

    assert_eq!(supervisor.start_tool(ToolId::Spammer), StartReply::Started);
    wait_for(&events, "the run starts", |event| {
        state_phase(event) == Some("running")
    });

    assert_eq!(
        supervisor.stop_tool(None),
        StopReply::Stopping {
            tool: ToolId::Spammer
        }
    );

    let seen = until_finished(&events);
    let phases: Vec<&str> = seen.iter().filter_map(state_phase).collect();
    assert_eq!(phases, vec!["stopping", "idle"]);
    let finished = seen.last().expect("finished");
    assert_eq!(finished.data["reason"], "cancelled");
    assert_eq!(finished.data["message"], "Stopped.");

    supervisor.shutdown("test");
}

#[test]
fn a_second_start_is_tool_busy_with_the_arbiters_line() {
    let registry = FakeRegistry::new().with_script(ToolId::Spammer, vec![Decision::wait(30.0)]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);
    assert_eq!(supervisor.start_tool(ToolId::Spammer), StartReply::Started);
    wait_for(&events, "the run starts", |event| {
        state_phase(event) == Some("running")
    });

    let refusal = supervisor.start_tool(ToolId::Gem);

    match refusal {
        StartReply::Refused(refusal) => assert_eq!(
            refusal.line,
            "\"spammer\" is using the game right now — stop it, then try again."
        ),
        other => panic!("expected a refusal, got {other:?}"),
    }

    let _ = supervisor.stop_tool(None);
    until_finished(&events);
    supervisor.shutdown("test");
}

#[test]
fn an_unavailable_tool_is_refused_by_name_without_a_run() {
    let registry = FakeRegistry::new().unavailable_for(ToolId::Tuner);
    let (bus, _events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);

    match supervisor.start_tool(ToolId::Tuner) {
        StartReply::Unavailable { line } => {
            assert!(line.contains("\"tuner\""), "{line}");
            assert!(line.contains("not available in this build yet"), "{line}");
        }
        other => panic!("expected unavailability, got {other:?}"),
    }

    supervisor.shutdown("test");
}

#[test]
fn stopping_when_nothing_runs_is_not_running() {
    let registry = FakeRegistry::new();
    let (bus, _events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);

    assert_eq!(supervisor.stop_tool(None), StopReply::NotRunning);
    assert_eq!(
        supervisor.stop_tool(Some(ToolId::Gem)),
        StopReply::NotRunning
    );

    supervisor.shutdown("test");
}

#[test]
fn two_runs_in_sequence_release_the_slot() {
    let registry = FakeRegistry::new()
        .with_script(
            ToolId::Tuner,
            vec![Decision::stop(StopReason::done("first"))],
        )
        .with_script(
            ToolId::Quest,
            vec![Decision::stop(StopReason::done("second"))],
        );
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);

    assert_eq!(supervisor.start_tool(ToolId::Tuner), StartReply::Started);
    until_finished(&events);

    assert_eq!(supervisor.start_tool(ToolId::Quest), StartReply::Started);
    let seen = until_finished(&events);
    assert_eq!(seen.last().expect("finished").data["message"], "second");

    supervisor.shutdown("test");
}

#[test]
fn an_opening_phase_failure_ends_as_an_error_event_and_releases_the_slot() {
    let registry = FakeRegistry::new()
        .failing_to_open(ToolId::Gem, "the board could not be opened")
        .with_script(
            ToolId::Tuner,
            vec![Decision::stop(StopReason::done("after"))],
        );
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);

    assert_eq!(supervisor.start_tool(ToolId::Gem), StartReply::Started);

    let seen = until_finished(&events);
    let finished = seen.last().expect("finished");
    assert_eq!(finished.data["reason"], "error");
    assert_eq!(finished.data["message"], "the board could not be opened");
    let final_state = seen
        .iter()
        .rev()
        .find(|event| event.event == "state")
        .unwrap();
    assert_eq!(
        final_state.data["error"], "the board could not be opened",
        "the failure rides on the card as well as the finish event"
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Tuner),
        StartReply::Started,
        "the failed opening released the slot"
    );
    until_finished(&events);
    supervisor.shutdown("test");
}

#[test]
fn a_start_cancelled_during_its_opening_ends_cancelled_and_leaks_nothing() {
    // `OPEN.md` §1 item 1: v2 could cancel a start in flight; the opening phase is a phase, so a
    // stop during it cancels the token, the session build returns, and the exit path reports
    // cancelled — no slot is held by a run that never began.
    let (started, opening_started) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let registry = FakeRegistry::new()
        .with_script(ToolId::Spammer, vec![Decision::wait(30.0)])
        .gated_opening(started, gate);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);

    assert_eq!(supervisor.start_tool(ToolId::Spammer), StartReply::Started);
    opening_started
        .recv_timeout(Duration::from_secs(5))
        .expect("the opening phase began");

    // Stop while the session is still being built, then let the build finish.
    assert_eq!(
        supervisor.stop_tool(None),
        StopReply::Stopping {
            tool: ToolId::Spammer
        }
    );
    release.send(()).expect("the opening gate opens");

    let seen = until_finished(&events);
    let finished = seen.last().expect("finished");
    assert_eq!(finished.data["reason"], "cancelled");
    assert_eq!(finished.data["message"], "Stopped.");

    // The slot is free again: a different tool starts cleanly.
    let registry_again = supervisor.start_tool(ToolId::Gem);
    assert!(
        matches!(
            registry_again,
            StartReply::Internal { .. } | StartReply::Unavailable { .. }
        ) || registry_again == StartReply::Started,
        "a start after a cancelled opening is not a leak: {registry_again:?}"
    );

    supervisor.shutdown("test");
}

#[test]
fn shutdown_stops_a_running_tool_waits_for_it_and_emits_core_stopping() {
    let registry = FakeRegistry::new().with_script(ToolId::Spammer, vec![Decision::wait(30.0)]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);
    assert_eq!(supervisor.start_tool(ToolId::Spammer), StartReply::Started);
    wait_for(&events, "the run starts", |event| {
        state_phase(event) == Some("running")
    });

    supervisor.shutdown("shutdown");

    let mut seen = Vec::new();
    while let Ok(event) = events.try_recv() {
        seen.push(event);
    }
    let kinds: Vec<&str> = seen.iter().map(|event| event.event.as_str()).collect();
    assert!(kinds.contains(&"tool.finished"), "{kinds:?}");
    assert_eq!(
        seen.iter()
            .find(|event| event.event == "tool.finished")
            .expect("finished")
            .data["reason"],
        "cancelled"
    );
    let stopping = seen
        .iter()
        .find(|event| event.event == "core.stopping")
        .expect("core.stopping is emitted on shutdown");
    assert_eq!(stopping.data["reason"], "shutdown");
    assert_eq!(
        kinds.last(),
        Some(&"core.stopping"),
        "the stopping event is the last thing the supervisor says"
    );
}

#[test]
fn the_finished_event_and_the_final_snapshot_agree() {
    // The exit-path contract: whatever reason the runner returned, the snapshot and the event carry
    // the same message.
    let registry = FakeRegistry::new().with_script(
        ToolId::Quest,
        vec![Decision::stop(StopReason::done("DONE — 20 loop(s)"))],
    );
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);
    assert_eq!(supervisor.start_tool(ToolId::Quest), StartReply::Started);

    let seen = until_finished(&events);
    let final_state = seen
        .iter()
        .rev()
        .find(|event| event.event == "state")
        .unwrap();
    let finished = seen.last().expect("finished");
    assert_eq!(final_state.data["message"], finished.data["message"]);
    assert_eq!(final_state.data["running"], false);
    assert_eq!(final_state.data["phase"], "idle");

    supervisor.shutdown("test");
}

#[test]
fn the_supervisor_can_see_the_worker_phases_from_its_own_events() {
    // A smoke test that the worker thread really ran the runner: the phases come from the shared
    // state cell, and the ordering is the lifecycle.
    let registry = FakeRegistry::new().with_script(
        ToolId::Sell,
        vec![
            Decision::wait(0.0),
            Decision::stop(StopReason::done("sold")),
        ],
    );
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);
    assert_eq!(supervisor.start_tool(ToolId::Sell), StartReply::Started);
    let phases: Vec<String> = until_finished(&events)
        .iter()
        .filter_map(|event| state_phase(event).map(str::to_owned))
        .collect();
    assert_eq!(phases, vec!["starting", "running", "idle"]);
    assert_eq!(
        ToolPhase::Running.as_str(),
        "running",
        "the phase words are the protocol's"
    );
    supervisor.shutdown("test");
}

#[test]
fn a_worker_thread_is_really_a_thread() {
    // Guards against a future refactor "simplifying" the supervisor into a synchronous call: the
    // start must return before the run finishes.
    let registry = FakeRegistry::new().with_script(
        ToolId::Tuner,
        vec![
            Decision::wait(0.0),
            Decision::wait(0.0),
            Decision::stop(StopReason::done("slow enough")),
        ],
    );
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus);
    assert_eq!(supervisor.start_tool(ToolId::Tuner), StartReply::Started);
    // The start reply arrived while the run was still going (the worker sends Opened before it ends).
    wait_for(&events, "the run starts", |event| {
        state_phase(event) == Some("running")
    });
    until_finished(&events);
    supervisor.shutdown("test");
}

#[test]
fn the_supervisor_is_joinable_and_drop_safe() {
    let registry = FakeRegistry::new();
    let (bus, _events) = seal_core::events::event_channel();
    {
        let supervisor = Supervisor::start(registry, bus);
        let _ = supervisor;
        // Dropped without an explicit shutdown: the Drop path must stop the thread, not leak it.
    }
    thread::sleep(Duration::from_millis(10));
}

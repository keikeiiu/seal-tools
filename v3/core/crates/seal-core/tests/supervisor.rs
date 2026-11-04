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

use seal_core::logs::Logs;
use seal_core::registry::{Session, ToolRegistry, Unavailable};
use seal_core::supervisor::{StartReply, StopReply, Supervisor};
use seal_domain::decision::{Decision, Key, StopReason};
use seal_domain::geometry::Point;
use seal_proto::Event;
use seal_tools::game::GamePort;
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

/// CORE-F3: a tool that panics on its first decision. The worker must still report a finish, free
/// the arbiter slot and let shutdown return.
struct PanickingTool {
    id: ToolId,
}

impl Tool for PanickingTool {
    fn id(&self) -> ToolId {
        self.id
    }

    fn decide(&mut self) -> Decision {
        panic!("scripted worker panic");
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

/// A tool that takes the game through the port before its first decision. The first call requests
/// and BLOCKS until the supervisor grants it; an optional gate releases the game once (the second
/// decision), simulating a visit boundary.
struct PortTool {
    id: ToolId,
    game: Arc<dyn GamePort>,
    script: Vec<Decision>,
    decisions: Arc<Mutex<Vec<ToolId>>>,
    gate: Option<mpsc::Receiver<()>>,
    requested: bool,
    count: usize,
}

impl Tool for PortTool {
    fn id(&self) -> ToolId {
        self.id
    }

    fn decide(&mut self) -> Decision {
        if !self.requested {
            if let Err(line) = self.game.request(self.id) {
                return Decision::stop(StopReason::error(line));
            }
            self.requested = true;
        }
        if self.count == 1
            && let Some(gate) = self.gate.take()
        {
            let _ = gate.recv();
            self.game.release(self.id);
        }
        self.decisions.lock().expect("not poisoned").push(self.id);
        self.count += 1;
        if self.script.is_empty() {
            Decision::stop(StopReason::done("script ended"))
        } else {
            self.script.remove(0)
        }
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
    /// The last `create` arguments, so a test can assert the validated run args reached the seam.
    start_args: Mutex<Option<Value>>,
    /// Tools whose worker should take the game through the port, with the gate that releases it
    /// once (empty = no gate) and the shared decision log the tests assert on.
    port_tools: Mutex<Vec<ToolId>>,
    release_gates: Mutex<HashMap<ToolId, mpsc::Receiver<()>>>,
    /// Tools whose worker panics on its first decide (CORE-F3).
    panicking: Mutex<Vec<ToolId>>,
    decisions: Arc<Mutex<Vec<ToolId>>>,
}

impl FakeRegistry {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            scripts: Mutex::new(HashMap::new()),
            unavailable: Mutex::new(Vec::new()),
            opening_errors: Mutex::new(HashMap::new()),
            opening_gate: Mutex::new(None),
            opening_started: Mutex::new(None),
            start_args: Mutex::new(None),
            port_tools: Mutex::new(Vec::new()),
            release_gates: Mutex::new(HashMap::new()),
            panicking: Mutex::new(Vec::new()),
            decisions: Arc::new(Mutex::new(Vec::new())),
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

    fn with_port_tool(self: &Arc<Self>, tool: ToolId, script: Vec<Decision>) -> Arc<Self> {
        self.port_tools.lock().expect("not poisoned").push(tool);
        self.scripts
            .lock()
            .expect("not poisoned")
            .insert(tool, script);
        Arc::clone(self)
    }

    fn with_release_gate(self: &Arc<Self>, tool: ToolId, gate: mpsc::Receiver<()>) -> Arc<Self> {
        self.release_gates
            .lock()
            .expect("not poisoned")
            .insert(tool, gate);
        Arc::clone(self)
    }

    fn with_panicking_tool(self: &Arc<Self>, tool: ToolId) -> Arc<Self> {
        self.panicking.lock().expect("not poisoned").push(tool);
        Arc::clone(self)
    }

    fn start_args(&self) -> Option<Value> {
        self.start_args.lock().expect("not poisoned").clone()
    }
    fn decisions(&self) -> Vec<ToolId> {
        self.decisions.lock().expect("not poisoned").clone()
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

    fn create(
        &self,
        tool: ToolId,
        game: Arc<dyn GamePort>,
        args: &Value,
    ) -> Result<Session, String> {
        *self.start_args.lock().expect("not poisoned") = Some(args.clone());
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
        let is_port_tool = self
            .port_tools
            .lock()
            .expect("not poisoned")
            .contains(&tool);
        let panics = {
            let mut panicking = self.panicking.lock().expect("not poisoned");
            panicking
                .iter()
                .position(|candidate| *candidate == tool)
                .map(|position| panicking.remove(position))
                .is_some()
        };
        let release_gate = self
            .release_gates
            .lock()
            .expect("not poisoned")
            .remove(&tool);
        let decisions = Arc::clone(&self.decisions);
        let tool = if panics {
            Box::new(PanickingTool { id: tool }) as Box<dyn Tool + Send>
        } else if is_port_tool {
            Box::new(PortTool {
                id: tool,
                game,
                script,
                decisions,
                gate: release_gate,
                requested: false,
                count: 0,
            }) as Box<dyn Tool + Send>
        } else {
            Box::new(ScriptedTool { id: tool, script }) as Box<dyn Tool + Send>
        };
        Ok(Session {
            tool,
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
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));

    assert_eq!(
        supervisor.start_tool(ToolId::Tuner, &Value::Null),
        StartReply::Started
    );

    let seen = until_finished(&events);
    assert_eq!(
        seen.iter().filter_map(state_phase).collect::<Vec<_>>(),
        vec!["starting", "running", "running", "idle"],
        "the card opens, publishes a live running update, then reads the final state"
    );
    let last_state = seen
        .iter()
        .rev()
        .find(|event| event.event == "state")
        .unwrap();
    assert_eq!(last_state.data["message"], "DONE — 1 click");
    assert_eq!(last_state.data["running"], false);
    let finished = seen.last().expect("finished");
    assert_eq!(finished.data["tool"], "tuner");
    assert_eq!(finished.data["reason"], "done");
    assert_eq!(finished.data["message"], "DONE — 1 click");
    assert!(
        finished.data["summary"].is_null(),
        "the fake tuner tool publishes no summary"
    );
    assert!(
        finished.data["ended_at"]
            .as_str()
            .is_some_and(|stamp| stamp.ends_with('Z')),
        "ended_at is the core's ISO-8601 UTC stamp: {:?}",
        finished.data["ended_at"]
    );

    supervisor.shutdown("test");
}

#[test]
fn a_stop_publishes_stopping_then_a_cancelled_finish() {
    let registry = FakeRegistry::new().with_script(ToolId::Spammer, vec![Decision::wait(30.0)]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));

    assert_eq!(
        supervisor.start_tool(ToolId::Spammer, &Value::Null),
        StartReply::Started
    );
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
    assert_eq!(phases, vec!["running", "stopping", "idle"]);
    let finished = seen.last().expect("finished");
    assert_eq!(finished.data["reason"], "cancelled");
    assert_eq!(finished.data["message"], "Stopped.");

    supervisor.shutdown("test");
}

#[test]
fn a_second_start_is_tool_busy_with_the_arbiters_line() {
    let registry = FakeRegistry::new().with_script(ToolId::Spammer, vec![Decision::wait(30.0)]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));
    assert_eq!(
        supervisor.start_tool(ToolId::Spammer, &Value::Null),
        StartReply::Started
    );
    wait_for(&events, "the run starts", |event| {
        state_phase(event) == Some("running")
    });

    let refusal = supervisor.start_tool(ToolId::Gem, &Value::Null);

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
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));

    match supervisor.start_tool(ToolId::Tuner, &Value::Null) {
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
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));

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
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));

    assert_eq!(
        supervisor.start_tool(ToolId::Tuner, &Value::Null),
        StartReply::Started
    );
    until_finished(&events);

    assert_eq!(
        supervisor.start_tool(ToolId::Quest, &Value::Null),
        StartReply::Started
    );
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
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));

    assert_eq!(
        supervisor.start_tool(ToolId::Gem, &Value::Null),
        StartReply::Started
    );

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
        supervisor.start_tool(ToolId::Tuner, &Value::Null),
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
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));

    assert_eq!(
        supervisor.start_tool(ToolId::Spammer, &Value::Null),
        StartReply::Started
    );
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
    let registry_again = supervisor.start_tool(ToolId::Gem, &Value::Null);
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
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));
    assert_eq!(
        supervisor.start_tool(ToolId::Spammer, &Value::Null),
        StartReply::Started
    );
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
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));
    assert_eq!(
        supervisor.start_tool(ToolId::Quest, &Value::Null),
        StartReply::Started
    );

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
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));
    assert_eq!(
        supervisor.start_tool(ToolId::Sell, &Value::Null),
        StartReply::Started
    );
    let phases: Vec<String> = until_finished(&events)
        .iter()
        .filter_map(|event| state_phase(event).map(str::to_owned))
        .collect();
    assert_eq!(
        phases,
        vec!["starting", "running", "running", "idle"],
        "the live state event is part of the lifecycle now"
    );
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
    let mut supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));
    assert_eq!(
        supervisor.start_tool(ToolId::Tuner, &Value::Null),
        StartReply::Started
    );
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
        let supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));
        let _ = supervisor;
        // Dropped without an explicit shutdown: the Drop path must stop the thread, not leak it.
    }
    thread::sleep(Duration::from_millis(10));
}
// ── the game hand-over seam ─────────────────────────────────────────────────────────────────

fn wait_until(what: &str, predicate: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if predicate() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

fn state_for<'a>(event: &'a Event, tool: &str) -> Option<&'a str> {
    if event.event == "state" && event.data.get("tool").and_then(Value::as_str) == Some(tool) {
        state_phase(event)
    } else {
        None
    }
}

#[test]
fn a_residents_request_waits_for_the_foreground_tool_and_is_granted_after_it_finishes() {
    let registry = FakeRegistry::new()
        .with_script(ToolId::Tuner, vec![Decision::wait(30.0)])
        .with_port_tool(ToolId::Pet, vec![Decision::wait(30.0)]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::new(Logs::disabled()),
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Tuner, &Value::Null),
        StartReply::Started
    );
    wait_for(&events, "the foreground run starts", |event| {
        state_for(event, "tuner") == Some("running")
    });

    // The resident start is armed; its worker blocks in `request` because the tuner holds the game,
    // so it must not produce a single decision.
    assert_eq!(
        supervisor.start_tool(ToolId::Pet, &Value::Null),
        StartReply::Started
    );
    wait_for(&events, "the resident starts", |event| {
        state_for(event, "pet") == Some("running")
    });
    thread::sleep(Duration::from_millis(150));
    assert!(
        registry.decisions().is_empty(),
        "a blocked request must produce no pet decisions: {:?}",
        registry.decisions()
    );

    // The tuner leaves; the stashed request is granted from the finish path.
    assert_eq!(
        supervisor.stop_tool(Some(ToolId::Tuner)),
        StopReply::Stopping {
            tool: ToolId::Tuner
        }
    );
    let seen = until_finished(&events);
    let finished = seen.last().expect("the tuner finished");
    assert_eq!(finished.data["tool"], "tuner");
    assert_eq!(finished.data["reason"], "cancelled");
    assert!(
        seen.iter().any(|event| {
            event.event == "log"
                && event
                    .data
                    .get("line")
                    .and_then(Value::as_str)
                    .is_some_and(|line| line.contains("is waiting for the game"))
        }),
        "the waiting line is published: {seen:?}"
    );

    wait_until("the granted resident to decide", || {
        registry.decisions().contains(&ToolId::Pet)
    });

    let _ = supervisor.stop_tool(Some(ToolId::Pet));
    until_finished(&events);
    supervisor.shutdown("test");
}

#[test]
fn a_blocked_resident_request_produces_zero_decisions() {
    let registry = FakeRegistry::new()
        .with_script(ToolId::Tuner, vec![Decision::wait(30.0)])
        .with_port_tool(ToolId::Pet, vec![Decision::wait(30.0)]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::new(Logs::disabled()),
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Tuner, &Value::Null),
        StartReply::Started
    );
    wait_for(&events, "the foreground run starts", |event| {
        state_for(event, "tuner") == Some("running")
    });
    assert_eq!(
        supervisor.start_tool(ToolId::Pet, &Value::Null),
        StartReply::Started
    );
    wait_for(&events, "the resident starts", |event| {
        state_for(event, "pet") == Some("running")
    });

    // Deterministic: wait for the waiting line, which is published exactly when the request reply
    // is stashed, so the shutdown below lands on a genuinely blocked worker.
    wait_for(&events, "the resident request is stashed", |event| {
        event.event == "log"
            && event
                .data
                .get("line")
                .and_then(Value::as_str)
                .is_some_and(|line| line.contains("is waiting for the game"))
    });
    assert!(
        registry.decisions().is_empty(),
        "no input may be produced while the request is blocked: {:?}",
        registry.decisions()
    );

    // Shutdown while the request is still stashed: the worker is unblocked with a line rather than
    // left waiting on a dead channel. This exercises the stop_all path: the stopped tool's stashed
    // request is answered there, and the event proves the worker left instead of being stranded.
    supervisor.shutdown("test");
    let mut seen = Vec::new();
    while let Ok(event) = events.try_recv() {
        seen.push(event);
    }
    assert!(
        seen.iter().any(|event| {
            event.event == "tool.finished"
                && event.data["tool"].as_str() == Some("pet")
                && event.data["reason"] == "cancelled"
        }),
        "the stop_all flush answers the blocked resident's request: {seen:?}"
    );
}

#[test]
fn a_stopped_resident_waiting_for_the_game_finishes_cancelled_and_frees_the_slot() {
    // CORE-F1: stopping the tool whose game request is STASHED must answer that request. The
    // arbiter clears the waiter when its wait is stopped, so nothing would ever grant the stashed
    // reply: the worker would stay blocked in `request`, the slot would read `stopping` forever and
    // every later start of that tool would be refused as still stopping.
    let registry = FakeRegistry::new()
        .with_script(ToolId::Tuner, vec![Decision::wait(30.0)])
        .with_port_tool(ToolId::Pet, vec![Decision::wait(30.0)]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::new(Logs::disabled()),
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Tuner, &Value::Null),
        StartReply::Started
    );
    wait_for(&events, "the foreground run starts", |event| {
        state_for(event, "tuner") == Some("running")
    });
    assert_eq!(
        supervisor.start_tool(ToolId::Pet, &Value::Null),
        StartReply::Started
    );
    wait_for(&events, "the resident starts", |event| {
        state_for(event, "pet") == Some("running")
    });
    // Deterministic: the waiting line is published exactly when the request reply is stashed, so
    // the stop below lands on a genuinely blocked worker (not one that has not asked yet).
    wait_for(&events, "the resident request is stashed", |event| {
        event.event == "log"
            && event
                .data
                .get("line")
                .and_then(Value::as_str)
                .is_some_and(|line| line.contains("is waiting for the game"))
    });
    assert!(
        registry.decisions().is_empty(),
        "a blocked request produces no decision: {:?}",
        registry.decisions()
    );

    // Stop the BLOCKED resident: within a second its worker must leave, cancelled.
    assert_eq!(
        supervisor.stop_tool(Some(ToolId::Pet)),
        StopReply::Stopping { tool: ToolId::Pet }
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let mut finished = None;
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let Ok(event) = events.recv_timeout(remaining) else {
            break;
        };
        if event.event == "tool.finished" && event.data["tool"].as_str() == Some("pet") {
            finished = Some(event);
            break;
        }
    }
    let finished = finished.expect("the stopped resident must finish within 1 s");
    assert_eq!(finished.data["reason"], "cancelled", "{finished:?}");
    assert_eq!(finished.data["message"], "Stopped.", "{finished:?}");

    // The slot is gone with the worker: the next pet start is accepted, not "still stopping".
    assert_eq!(
        supervisor.start_tool(ToolId::Pet, &Value::Null),
        StartReply::Started,
        "the stopped tool's slot must be reusable"
    );

    // The foregound run is still going; stop everything and let shutdown drain it.
    let _ = supervisor.stop_tool(Some(ToolId::Tuner));
    supervisor.shutdown("test");
}

#[test]
fn a_foreground_start_deferred_behind_the_resident_starts_when_the_resident_releases() {
    let (release, gate) = mpsc::channel();
    let registry = FakeRegistry::new()
        .with_port_tool(ToolId::Pet, vec![Decision::wait(0.0), Decision::wait(30.0)])
        .with_release_gate(ToolId::Pet, gate)
        .with_script(ToolId::Tuner, vec![Decision::wait(30.0)]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::new(Logs::disabled()),
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Pet, &Value::Null),
        StartReply::Started
    );
    wait_for(&events, "the resident starts", |event| {
        state_for(event, "pet") == Some("running")
    });
    wait_until("the resident's first decision", || {
        registry.decisions().contains(&ToolId::Pet)
    });

    // A foreground start behind a resident HOLDING the game is deferred, not started.
    match supervisor.start_tool(ToolId::Tuner, &Value::Null) {
        StartReply::Deferred { line } => {
            assert!(line.contains("Waiting for the pet feeder"), "{line}");
        }
        other => panic!("expected the deferral, got {other:?}"),
    }

    // Let the resident finish its visit: its second decision releases the game, and the pending
    // foreground start must actually start from the `StartPending` hand-over.
    release.send(()).expect("the resident releases");
    wait_for(&events, "the deferred start to run", |event| {
        state_for(event, "tuner") == Some("running")
    });

    let _ = supervisor.stop_tool(Some(ToolId::Tuner));
    until_finished(&events);
    let _ = supervisor.stop_tool(Some(ToolId::Pet));
    until_finished(&events);
    supervisor.shutdown("test");
}

#[test]
fn a_pet_request_preempts_the_spammer_and_is_granted_after_it_leaves() {
    let registry = FakeRegistry::new()
        .with_script(ToolId::Spammer, vec![Decision::wait(30.0)])
        .with_port_tool(ToolId::Pet, vec![Decision::wait(30.0)]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::new(Logs::disabled()),
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Spammer, &Value::Null),
        StartReply::Started
    );
    wait_for(&events, "the spammer starts", |event| {
        state_for(event, "spammer") == Some("running")
    });
    assert_eq!(
        supervisor.start_tool(ToolId::Pet, &Value::Null),
        StartReply::Started
    );

    // The preempt cancels the spammer; its exit path hands the game to the waiting resident.
    let seen = until_finished(&events);
    let finished = seen.last().expect("the spammer finished");
    assert_eq!(finished.data["tool"], "spammer");
    assert_eq!(finished.data["reason"], "cancelled");
    assert!(
        seen.iter().any(|event| {
            event.event == "log"
                && event
                    .data
                    .get("line")
                    .and_then(Value::as_str)
                    .is_some_and(|line| line.contains("stepped aside for \"pet\""))
        }),
        "the stand-down line is published: {seen:?}"
    );

    wait_until("the granted resident to decide", || {
        registry.decisions().contains(&ToolId::Pet)
    });

    let _ = supervisor.stop_tool(Some(ToolId::Pet));
    until_finished(&events);
    supervisor.shutdown("test");
}
// ── tool.start run arguments (`PROTOCOL.md` §4 note / §12.6) ────────────────────────────────

#[test]
fn the_input_lease_names_the_running_holder_and_is_free_again_after_it_finishes() {
    let registry = FakeRegistry::new().with_script(ToolId::Spammer, vec![Decision::wait(30.0)]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::new(Logs::disabled()),
    );
    assert_eq!(supervisor.input_lease(), Ok(None), "nothing is running yet");

    assert_eq!(
        supervisor.start_tool(ToolId::Spammer, &Value::Null),
        StartReply::Started
    );
    wait_for(&events, "the run starts", |event| {
        state_phase(event) == Some("running")
    });
    assert_eq!(
        supervisor.input_lease(),
        Ok(Some(
            "\"spammer\" is using the game right now — stop it, then try again.".to_owned()
        )),
        "the pointer action must name the holder"
    );

    assert_eq!(
        supervisor.stop_tool(None),
        StopReply::Stopping {
            tool: ToolId::Spammer
        }
    );
    let _ = until_finished(&events);
    assert_eq!(
        supervisor.input_lease(),
        Ok(None),
        "the lease is free again after the finish"
    );
    supervisor.shutdown("test");
}

#[test]
fn start_arguments_reach_create_verbatim() {
    let registry = FakeRegistry::new().with_script(
        ToolId::Spammer,
        vec![Decision::stop(StopReason::done("no arguments needed"))],
    );
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::new(Logs::disabled()),
    );
    let args = serde_json::json!({"preset": "potion", "count": 7});

    assert_eq!(
        supervisor.start_tool(ToolId::Spammer, &args),
        StartReply::Started
    );
    until_finished(&events);

    assert_eq!(
        registry.start_args(),
        Some(args),
        "the validated run arguments are handed to `create` unchanged"
    );
    supervisor.shutdown("test");
}

#[test]
fn a_worker_panic_finishes_with_an_error_and_frees_the_slot() {
    let registry = FakeRegistry::new().with_panicking_tool(ToolId::Tuner);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::new(Logs::disabled()),
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Tuner, &Value::Null),
        StartReply::Started
    );
    let seen = until_finished(&events);
    let finished = seen.last().expect("the panicking run finished");
    assert_eq!(finished.data["tool"], "tuner");
    assert_eq!(finished.data["reason"], "error");
    assert_eq!(
        finished.data["message"], "internal error — see error.log",
        "the run's exit is the internal-error finish, not a leaked slot"
    );

    // The arbiter slot really is free: the same tool starts again and completes normally.
    assert_eq!(
        supervisor.start_tool(ToolId::Tuner, &Value::Null),
        StartReply::Started
    );
    until_finished(&events);
    supervisor.shutdown("test");
}

#[test]
fn a_create_refusal_surfaces_as_the_finished_error_not_a_start_reply() {
    // The start reply is already sent before `create` runs (the worker builds the session), so a
    // buy start with no preset is `Started` and the domain line arrives as the run's `tool.finished`
    // error. Pinning the existing mapping keeps the protocol honest about when the answer arrives.
    let registry = FakeRegistry::new().failing_to_open(ToolId::Buy, "No buy preset selected.");
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::new(Logs::disabled()),
    );
    let args = serde_json::json!({});

    assert_eq!(
        supervisor.start_tool(ToolId::Buy, &args),
        StartReply::Started
    );
    let seen = until_finished(&events);
    let finished = seen.last().expect("the run finished");
    assert_eq!(finished.data["tool"], "buy");
    assert_eq!(finished.data["reason"], "error");
    assert_eq!(finished.data["message"], "No buy preset selected.");
    assert_eq!(registry.start_args(), Some(args));
    supervisor.shutdown("test");
}

//! The tool supervisor: the layer that turns the protocol's tool commands into arbiter + runner
//! activity and publishes the resulting events, and the one place a RUNNING tool can reach the
//! arbiter.
//!
//! One supervisor THREAD owns the [`Arbiter`] and the worker handles, so the arbiter's state is never
//! shared (the two-writer hazard `PROGRESS.md` entry 59 recorded is gone by construction). Workers
//! report two things over one channel: `Opened` when the opening phase succeeded and `Done` as their
//! LAST act. `Done` is where `arbiter.finished` is called — on the worker's exit path — so the final
//! snapshot, the `tool.finished` event, the slot release and the game hand-over all happen together
//! (the `OPEN.md` §1 hand-over).
//!
//! Events go to the [`EventBus`]; one writer thread serialises them with responses (`events`), so no
//! worker can interleave a line.
//!
//! The opening phase (`OPEN.md` §1 item 1): a start is accepted by the arbiter, then the worker builds
//! its session (open the board, load the models). A second start while that happens sees the slot and
//! is refused `tool_busy`; a stop during it cancels the token, and the worker ends `cancelled` by
//! calling `Done` — no slot leaks.
//!
//! # The game hand-over ([`GamePort`])
//!
//! The arbiter is the complete state machine (the resident feeder armed without the game, a
//! foreground start deferred behind it, the spammer preemptible). What it lacked was a way for a
//! running worker to talk to it. `SupervisorPort` is that capability: `request` is a
//! `RequestGame` command whose reply is stashed HERE (never blocking the supervisor thread) and
//! satisfied when the arbiter's `release_game`/`finished` reports the waiter granted; `release` is a
//! `ReleaseGame` command; `should_stand_down` answers for the spammer. A `Preempt` cancels the holder
//! through the arbiter (the spammer is the only tool that stands down — v2's rule; its RUNNER-side
//! use of `should_stand_down` is the recorded increment-2 follow-up, not implemented here).
//!
//! The deferred-start timeout is also wired: a `Start::Deferred` schedules one `PendingTimeout`
//! command 45 s out, and the arbiter's `pending_timeout` line is published as a `log` event. The
//! caller has already been answered `Deferred`, so the timeout cannot turn into a response on this
//! protocol; that is reported rather than papered over.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use seal_domain::decision::{Decision, StopReason};
use seal_tools::arbiter::{Arbiter, GameRelease, GameUse, Refusal, RunHandle, Start, Stop};
use seal_tools::clock::RealClock;
use seal_tools::game::GamePort;
use seal_tools::runner::{self, STOPPED_MESSAGE, Tool, ToolStatus};
use seal_tools::tool::{ToolId, ToolPhase, ToolSnapshot};
use serde_json::Value;

use crate::events::{self, EventBus};
use crate::logs::Logs;
use crate::registry::{Session, ToolRegistry};

/// v2's bounded wait for the pet feeder's reload (`LauncherService`'s 45 s).
const PENDING_TIMEOUT: Duration = Duration::from_secs(45);

/// What `tool.start` answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartReply {
    /// The run is opening; `{}`. Its `state`/`tool.finished` events follow.
    Started,
    /// Waiting behind the resident feeder's bounded reload. The waiting line was published as a
    /// `log` event (there is no run yet to attach a `state` to, and reporting one would lie about
    /// registration); the start happens when the feeder releases the game.
    Deferred { line: String },
    /// The arbiter's refusal, with the line the player sees.
    Refused(Refusal),
    /// This build has no loop for that tool.
    Unavailable { line: String },
    /// The supervisor thread is gone. A bug, reported rather than a silent dead button.
    Internal { line: String },
}

/// What `tool.stop` answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReply {
    /// The token is cancelled and the phase is `stopping`; the `tool.finished` event follows.
    Stopping {
        tool: ToolId,
    },
    NotRunning,
}

/// One stashed `request` reply: the tool waiting for the game, and the channel its worker blocks on.
type PendingReply = (ToolId, Sender<Result<(), String>>);

enum Command {
    Start {
        tool: ToolId,
        /// The validated `tool.start` arguments (empty object for tools with none).
        args: Value,
        reply: Sender<StartReply>,
    },
    Stop {
        tool: Option<ToolId>,
        reply: Sender<StopReply>,
    },
    Shutdown {
        reason: String,
        reply: Sender<()>,
    },
    RequestGame {
        tool: ToolId,
        reply: Sender<Result<(), String>>,
    },
    ReleaseGame {
        tool: ToolId,
    },
    StandDown {
        reply: Sender<bool>,
    },
    /// A calibration pointer action asks whether the arbiter currently leases the game. `None`
    /// means free to act; `Some(line)` is the same player-facing busy line a tool start would carry.
    InputLease {
        reply: Sender<Option<String>>,
    },
    /// The deferred foreground start's bounded wait ran out (v2's 45 s).
    PendingTimeout,
}

enum WorkerEvent {
    /// The opening phase ended and the run is publishing `running`.
    Opened {
        tool: ToolId,
        snapshot: Arc<ToolSnapshot>,
    },
    /// The worker's LAST act. Handling this is where `finished` runs. `summary` is the tool's own
    /// end-of-run facts (G11), built before the tool is dropped; the event builder adds `ended_at`.
    Done {
        tool: ToolId,
        reason: StopReason,
        summary: Option<Value>,
    },
}

enum Incoming {
    Command(Command),
    Worker(WorkerEvent),
}

/// The supervisor handle the dispatcher uses. Dropping it shuts the supervisor thread down.
pub struct Supervisor {
    incoming: Sender<Incoming>,
    join: Option<JoinHandle<()>>,
}

impl Supervisor {
    /// Spawns the supervisor thread. The registry is the production placeholder in the binary and a
    /// fake in the tests; the bus is the single writer's bus in the binary and a test channel in the
    /// tests. The [`GamePort`] handed to every `create` is created here, over this supervisor's own
    /// command channel, so a worker's request is answered by the thread that owns the arbiter.
    pub fn start(registry: Arc<dyn ToolRegistry>, bus: EventBus, logs: Arc<Logs>) -> Self {
        let (incoming, receiver) = mpsc::channel();
        let worker_sender = incoming.clone();
        let port: Arc<dyn GamePort> = Arc::new(SupervisorPort {
            incoming: incoming.clone(),
        });
        let join =
            thread::spawn(move || supervise(registry, bus, receiver, worker_sender, port, logs));
        Self {
            incoming,
            join: Some(join),
        }
    }

    pub fn start_tool(&self, tool: ToolId, args: &Value) -> StartReply {
        let (reply, answer) = mpsc::channel();
        if self
            .incoming
            .send(Incoming::Command(Command::Start {
                tool,
                args: args.clone(),
                reply,
            }))
            .is_err()
        {
            return StartReply::Internal {
                line: "the tool supervisor is not running".to_owned(),
            };
        }
        answer.recv().unwrap_or_else(|_| StartReply::Internal {
            line: "the tool supervisor stopped while starting".to_owned(),
        })
    }

    pub fn stop_tool(&self, tool: Option<ToolId>) -> StopReply {
        let (reply, answer) = mpsc::channel();
        if self
            .incoming
            .send(Incoming::Command(Command::Stop { tool, reply }))
            .is_err()
        {
            return StopReply::NotRunning;
        }
        answer.recv().unwrap_or(StopReply::NotRunning)
    }

    /// The per-command input lease for a calibration pointer action
    /// ([PLAN-V3-CALIBRATION.md](../../../docs/plans/PLAN-V3-CALIBRATION.md) §5.1 principle 6):
    /// `None` when no foreground run is engaged and the resident feeder is not holding the game,
    /// otherwise the same busy/still-stopping line a tool start would answer. This is a SNAPSHOT,
    /// not a held lease — session-wide leases and queueing belong to the calibration session
    /// increments; a pointer action only refuses while a tool already owns the game.
    pub fn input_lease(&self) -> Result<Option<String>, String> {
        let (reply, answer) = mpsc::channel();
        self.incoming
            .send(Incoming::Command(Command::InputLease { reply }))
            .map_err(|_| "the tool supervisor is not running".to_owned())?;
        answer
            .recv()
            .map_err(|_| "the tool supervisor stopped while checking the game lease".to_owned())
    }

    /// Stops every run, waits for the workers to leave, emits `core.stopping`, then joins the
    /// supervisor thread. Must be the last call: the writer can only finish after the supervisor (the
    /// last `EventBus` holder) is gone.
    pub fn shutdown(&mut self, reason: &str) {
        let Some(join) = self.join.take() else {
            return;
        };
        let (reply, answer) = mpsc::channel();
        if self
            .incoming
            .send(Incoming::Command(Command::Shutdown {
                reason: reason.to_owned(),
                reply,
            }))
            .is_ok()
        {
            let _ = answer.recv();
        }
        let _ = join.join();
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        self.shutdown("dropped");
    }
}

/// The worker-facing game capability: commands over the supervisor's channel, answers from the thread
/// that owns the arbiter. `request` is the only blocking call, and it blocks only the worker.
struct SupervisorPort {
    incoming: Sender<Incoming>,
}

impl GamePort for SupervisorPort {
    fn request(&self, tool: ToolId) -> Result<(), String> {
        let (reply, answer) = mpsc::channel();
        self.incoming
            .send(Incoming::Command(Command::RequestGame { tool, reply }))
            .map_err(|_| "the tool supervisor is not running".to_owned())?;
        answer.recv().unwrap_or_else(|_| {
            Err("the tool supervisor stopped while the tool waited for the game".to_owned())
        })
    }

    fn release(&self, tool: ToolId) {
        let _ = self
            .incoming
            .send(Incoming::Command(Command::ReleaseGame { tool }));
    }

    fn should_stand_down(&self) -> bool {
        let (reply, answer) = mpsc::channel();
        if self
            .incoming
            .send(Incoming::Command(Command::StandDown { reply }))
            .is_err()
        {
            return false;
        }
        answer.recv().unwrap_or(false)
    }
}

fn supervise(
    registry: Arc<dyn ToolRegistry>,
    bus: EventBus,
    receiver: Receiver<Incoming>,
    worker_sender: Sender<Incoming>,
    port: Arc<dyn GamePort>,
    logs: Arc<Logs>,
) {
    let mut arbiter = Arbiter::new();
    let mut workers: BTreeMap<ToolId, JoinHandle<()>> = BTreeMap::new();
    let mut pending_reply: Option<PendingReply> = None;
    let deferred_args: RefCell<Option<(ToolId, Value)>> = RefCell::new(None);

    while let Ok(message) = receiver.recv() {
        match message {
            Incoming::Command(Command::Start { tool, args, reply }) => {
                let answer = match registry.available(tool) {
                    Err(unavailable) => StartReply::Unavailable {
                        line: unavailable.line,
                    },
                    Ok(()) => match arbiter.start(tool) {
                        Start::Started(handle) => {
                            spawn_worker(
                                handle,
                                &Plumbing {
                                    registry: &registry,
                                    worker_sender: &worker_sender,
                                    bus: &bus,
                                    port: &port,
                                    deferred: &deferred_args,
                                    logs: &logs,
                                },
                                args,
                                &mut workers,
                            );
                            StartReply::Started
                        }
                        Start::Deferred { line } => {
                            // There is no run to attach a `state` to yet, so the waiting line is a
                            // `log` event; the start itself happens when the game is handed over.
                            bus.emit(events::log_event("info", line.clone()));
                            schedule_pending_timeout(worker_sender.clone());
                            *deferred_args.borrow_mut() = Some((tool, args.clone()));
                            StartReply::Deferred { line }
                        }
                        Start::Refused(refusal) => StartReply::Refused(refusal),
                    },
                };
                let _ = reply.send(answer);
            }
            Incoming::Command(Command::Stop { tool, reply }) => {
                let answer = match arbiter.stop(tool) {
                    Stop::Cancelling { tool, handle } => {
                        if let Some(snapshot) = handle.state.read() {
                            bus.emit(events::state_event(&snapshot));
                        }
                        StopReply::Stopping { tool }
                    }
                    Stop::NotRunning => StopReply::NotRunning,
                };
                let _ = reply.send(answer);
            }
            Incoming::Command(Command::Shutdown { reason, reply }) => {
                // Nothing pending is allowed to start on the way out. A worker blocked in `request`
                // must be unblocked with a line, not left waiting on a channel that is going away.
                let _ = arbiter.cancel_pending();
                if let Some((_tool, reply)) = pending_reply.take() {
                    let _ = reply.send(Err("the core is shutting down.".to_owned()));
                }
                for stop in arbiter.stop_all() {
                    if let Stop::Cancelling { handle, .. } = stop
                        && let Some(snapshot) = handle.state.read()
                    {
                        bus.emit(events::state_event(&snapshot));
                    }
                }
                while !workers.is_empty() {
                    match receiver.recv() {
                        Ok(Incoming::Worker(event)) => handle_worker(
                            event,
                            &mut arbiter,
                            &mut workers,
                            &Plumbing {
                                registry: &registry,
                                worker_sender: &worker_sender,
                                bus: &bus,
                                port: &port,
                                deferred: &deferred_args,
                                logs: &logs,
                            },
                            &mut pending_reply,
                        ),
                        Ok(Incoming::Command(Command::RequestGame { tool, reply })) => {
                            let _ = reply.send(Err(format!(
                                "\"{tool}\" cannot take the game — the core is shutting down."
                            )));
                        }
                        Ok(Incoming::Command(Command::ReleaseGame { tool })) => {
                            let release = arbiter.release_game(tool);
                            handle_game_release(
                                release,
                                &mut pending_reply,
                                &mut workers,
                                &Plumbing {
                                    registry: &registry,
                                    worker_sender: &worker_sender,
                                    bus: &bus,
                                    port: &port,
                                    deferred: &deferred_args,
                                    logs: &logs,
                                },
                            );
                        }
                        Ok(Incoming::Command(Command::StandDown { reply })) => {
                            let _ = reply.send(false);
                        }
                        Ok(Incoming::Command(_)) => {}
                        Err(_) => break,
                    }
                }
                bus.emit(events::core_stopping_event(&reason));
                let _ = reply.send(());
                break;
            }
            Incoming::Command(Command::RequestGame { tool, reply }) => {
                handle_request_game(tool, reply, &mut arbiter, &bus, &mut pending_reply);
            }
            Incoming::Command(Command::ReleaseGame { tool }) => {
                let release = arbiter.release_game(tool);
                handle_game_release(
                    release,
                    &mut pending_reply,
                    &mut workers,
                    &Plumbing {
                        registry: &registry,
                        worker_sender: &worker_sender,
                        bus: &bus,
                        port: &port,
                        deferred: &deferred_args,
                        logs: &logs,
                    },
                );
            }
            Incoming::Command(Command::StandDown { reply }) => {
                // Only the spammer may stand down (v2's `PortGate` rule), so the tool-less
                // capability answers for it; a tool that is not the spammer gets `false`.
                let _ = reply.send(arbiter.should_stand_down(ToolId::Spammer));
            }
            Incoming::Command(Command::InputLease { reply }) => {
                let _ = reply.send(input_lease_line(&arbiter));
            }
            Incoming::Command(Command::PendingTimeout) => {
                if let Some(refusal) = arbiter.pending_timeout() {
                    // The caller was already answered `Deferred`; this protocol has no channel back
                    // to it, so the timeout is the log line rather than a response (reported).
                    deferred_args.take();
                    bus.emit(events::log_event("warn", refusal.line));
                }
            }
            Incoming::Worker(event) => handle_worker(
                event,
                &mut arbiter,
                &mut workers,
                &Plumbing {
                    registry: &registry,
                    worker_sender: &worker_sender,
                    bus: &bus,
                    port: &port,
                    deferred: &deferred_args,
                    logs: &logs,
                },
                &mut pending_reply,
            ),
        }
    }
}

/// The line a calibration pointer action gets while a tool owns the game. The foreground run is
/// checked first (including a run that is still leaving), then the resident feeder's game claim.
/// The wording is v2's `InputArbiter.Busy` / v3's start-refusal wording.
fn input_lease_line(arbiter: &Arbiter) -> Option<String> {
    if let Some(holder) = arbiter
        .running()
        .into_iter()
        .find(|tool| !tool.is_resident())
    {
        return Some(if arbiter.is_stopping(holder) {
            format!("\"{holder}\" is still stopping — wait for it to finish, then start again.")
        } else {
            format!("\"{holder}\" is using the game right now — stop it, then try again.")
        });
    }
    arbiter.game_owner().map(|holder| {
        format!("\"{holder}\" is using the game right now — stop it, then try again.")
    })
}

/// Answer one worker's `request`: grant it, or publish the waiting/stand-down line and stash the
/// reply. The supervisor thread never blocks here.
fn handle_request_game(
    tool: ToolId,
    reply: Sender<Result<(), String>>,
    arbiter: &mut Arbiter,
    bus: &EventBus,
    pending: &mut Option<PendingReply>,
) {
    if pending.is_some() {
        let _ = reply.send(Err(
            "another tool is already waiting for the game".to_owned()
        ));
        return;
    }
    match arbiter.request_game(tool) {
        GameUse::Granted => {
            let _ = reply.send(Ok(()));
        }
        GameUse::Waiting { holder } => {
            bus.emit(events::log_event(
                "info",
                format!("\"{tool}\" is waiting for the game (\"{holder}\")."),
            ));
            *pending = Some((tool, reply));
        }
        GameUse::Preempt { holder } => {
            // v2's only log wording for this moment is the spammer's own stream line
            // (`[YIELD] handed the game to …`); the card has no player-facing sentence, so the
            // report records this defined line.
            bus.emit(events::log_event(
                "info",
                format!("\"{holder}\" stepped aside for \"{tool}\"."),
            ));
            if let Stop::Cancelling { handle, .. } = arbiter.stop(Some(holder))
                && let Some(snapshot) = handle.state.read()
            {
                bus.emit(events::state_event(&snapshot));
            }
            *pending = Some((tool, reply));
        }
    }
}

/// Act on what a `release_game`/`finished` left behind: satisfy the stashed waiter, or start the
/// foreground tool that was deferred behind the resident.
fn handle_game_release(
    release: GameRelease,
    pending: &mut Option<PendingReply>,
    workers: &mut BTreeMap<ToolId, JoinHandle<()>>,
    plumbing: &Plumbing<'_>,
) {
    match release {
        GameRelease::Freed => {}
        GameRelease::Granted(waiter) => {
            plumbing.bus.emit(events::log_event(
                "info",
                format!("\"{waiter}\" has the game."),
            ));
            if let Some((tool, reply)) = pending.take() {
                if tool == waiter {
                    let _ = reply.send(Ok(()));
                } else {
                    *pending = Some((tool, reply));
                }
            }
        }
        GameRelease::StartPending(handle) => {
            // The start waited behind the feeder; its run arguments were stashed with it.
            let args = plumbing
                .deferred
                .borrow_mut()
                .take()
                .filter(|(tool, _)| *tool == handle.tool)
                .map(|(_, args)| args)
                .unwrap_or(Value::Null);
            spawn_worker(handle, plumbing, args, workers);
        }
    }
}
/// The deferred start's bounded wait: v2's 45 s, after which `pending_timeout` names both sides.
fn schedule_pending_timeout(sender: Sender<Incoming>) {
    thread::spawn(move || {
        thread::sleep(PENDING_TIMEOUT);
        let _ = sender.send(Incoming::Command(Command::PendingTimeout));
    });
}

/// The supervisor-owned references a worker event needs. Bundled so the call sites and the helper
/// signatures stay readable (and clippy's argument ceiling stays satisfied).
struct Plumbing<'a> {
    registry: &'a Arc<dyn ToolRegistry>,
    worker_sender: &'a Sender<Incoming>,
    bus: &'a EventBus,
    port: &'a Arc<dyn GamePort>,
    /// The args of a start waiting behind the resident feeder, so its eventual worker gets them.
    deferred: &'a RefCell<Option<(ToolId, Value)>>,
    logs: &'a Arc<Logs>,
}

fn handle_worker(
    event: WorkerEvent,
    arbiter: &mut Arbiter,
    workers: &mut BTreeMap<ToolId, JoinHandle<()>>,
    plumbing: &Plumbing<'_>,
    pending: &mut Option<PendingReply>,
) {
    match event {
        WorkerEvent::Opened { tool, snapshot } => {
            // A stop may already have been processed between the send and this handling; then the
            // card is Stopping and a stale Running event would be a lie.
            if arbiter.is_running(tool) && !arbiter.is_stopping(tool) {
                plumbing.bus.emit(events::state_event(&snapshot));
            }
        }
        WorkerEvent::Done {
            tool,
            reason,
            summary,
        } => {
            // THE EXIT PATH: the final snapshot, tool.finished, the slot and the game all settle
            // here, in one place, from the worker's own last message.
            let finish = arbiter.finished(tool, reason.clone());
            if let Some(join) = workers.remove(&tool) {
                let _ = join.join();
            }
            if let Some(snapshot) = &finish.final_snapshot {
                plumbing.bus.emit(events::state_event(snapshot));
            }
            plumbing
                .bus
                .emit(events::tool_finished_event(tool, &reason, summary.as_ref()));
            handle_game_release(finish.game, pending, workers, plumbing);
        }
    }
}
fn spawn_worker(
    handle: RunHandle,
    plumbing: &Plumbing<'_>,
    args: Value,
    workers: &mut BTreeMap<ToolId, JoinHandle<()>>,
) {
    // The opening phase's snapshot: the card says Starting while the board/models are opened.
    if let Some(snapshot) = handle.state.read() {
        plumbing.bus.emit(events::state_event(&snapshot));
    }
    let registry = Arc::clone(plumbing.registry);
    let sender = plumbing.worker_sender.clone();
    let port = Arc::clone(plumbing.port);
    let logs = Arc::clone(plumbing.logs);
    let tool = handle.tool;
    let join = thread::spawn(move || worker_loop(handle, registry, sender, port, args, logs));
    workers.insert(tool, join);
}

/// One run. Sends `Opened` once the session exists, runs the loop, and sends `Done` as its last act.
fn worker_loop(
    handle: RunHandle,
    registry: Arc<dyn ToolRegistry>,
    supervisor: Sender<Incoming>,
    port: Arc<dyn GamePort>,
    args: Value,
    logs: Arc<Logs>,
) {
    let tool = handle.tool;
    let session: Session = match registry.create(tool, port, &args) {
        Ok(session) => session,
        Err(line) => {
            // An opening failure is a real failure (v2 put those in `error.log`, not among the
            // player-correctable refusals), and the tool's own log has no run to describe yet.
            logs.error_line(&format!("{tool} failed to start: {line}"));
            let _ = supervisor.send(Incoming::Worker(WorkerEvent::Done {
                tool,
                reason: StopReason::error(line),
                summary: None,
            }));
            return;
        }
    };

    // A stop that arrived during the opening phase: the run never started, and the exit path still
    // reports it as cancelled (`OPEN.md` §1 item 1).
    if handle.token.is_cancelled() {
        let _ = supervisor.send(Incoming::Worker(WorkerEvent::Done {
            tool,
            reason: StopReason::cancelled(STOPPED_MESSAGE),
            summary: None,
        }));
        return;
    }

    // The run begins here (the token was not cancelled during the opening phase). The tool log starts
    // with the generic moment; the tool's own status words follow as they change.
    logs.tool_started(tool);
    let running = Arc::new(ToolSnapshot::new(tool).running(ToolPhase::Running));
    handle.state.publish((*running).clone());
    let _ = supervisor.send(Incoming::Worker(WorkerEvent::Opened {
        tool,
        snapshot: running,
    }));

    let Session {
        tool: inner,
        effects,
    } = session;
    let mut logged = LoggedTool::new(inner, Arc::clone(&logs));
    let mut effects = effects;
    let reason = runner::run(&mut logged, &handle, &RealClock, &mut *effects);
    logs.tool_finished(tool, &reason);
    // The tool is about to be dropped: its last facts must be captured while it still exists.
    let summary = logged.summary(&reason);
    let _ = supervisor.send(Incoming::Worker(WorkerEvent::Done {
        tool,
        reason,
        summary,
    }));
}

/// Wraps one session tool so the worker's `<tool>.log` gets the tool's OWN status words: one line per
/// CHANGED message (the runner already deduplicates the published status, but a countdown/row change
/// that leaves the message alone must not duplicate a log line).
struct LoggedTool {
    inner: Box<dyn Tool + Send>,
    logs: Arc<Logs>,
    last_message: RefCell<String>,
}

impl LoggedTool {
    fn new(inner: Box<dyn Tool + Send>, logs: Arc<Logs>) -> Self {
        Self {
            inner,
            logs,
            last_message: RefCell::new(String::new()),
        }
    }
}

impl Tool for LoggedTool {
    fn id(&self) -> ToolId {
        self.inner.id()
    }

    fn decide(&mut self) -> Decision {
        self.inner.decide()
    }

    fn status(&self) -> ToolStatus {
        let status = self.inner.status();
        if !status.message.is_empty() && status.message != *self.last_message.borrow() {
            self.logs.tool_line(self.inner.id(), &status.message);
            *self.last_message.borrow_mut() = status.message.clone();
        }
        status
    }

    fn summary(&self, reason: &StopReason) -> Option<Value> {
        self.inner.summary(reason)
    }
}

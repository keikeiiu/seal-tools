//! The tool supervisor: the layer that turns the protocol's tool commands into arbiter + runner
//! activity and publishes the resulting events.
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

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use seal_domain::decision::StopReason;
use seal_tools::arbiter::{Arbiter, GameRelease, Refusal, RunHandle, Start, Stop};
use seal_tools::clock::RealClock;
use seal_tools::runner::{self, STOPPED_MESSAGE};
use seal_tools::tool::{ToolId, ToolPhase, ToolSnapshot};

use crate::events::{self, EventBus};
use crate::registry::{Session, ToolRegistry};

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

enum Command {
    Start {
        tool: ToolId,
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
}

enum WorkerEvent {
    /// The opening phase ended and the run is publishing `running`.
    Opened {
        tool: ToolId,
        snapshot: Arc<ToolSnapshot>,
    },
    /// The worker's LAST act. Handling this is where `finished` runs.
    Done { tool: ToolId, reason: StopReason },
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
    /// tests.
    pub fn start(registry: Arc<dyn ToolRegistry>, bus: EventBus) -> Self {
        let (incoming, receiver) = mpsc::channel();
        let worker_sender = incoming.clone();
        let join = thread::spawn(move || supervise(registry, bus, receiver, worker_sender));
        Self {
            incoming,
            join: Some(join),
        }
    }

    pub fn start_tool(&self, tool: ToolId) -> StartReply {
        let (reply, answer) = mpsc::channel();
        if self
            .incoming
            .send(Incoming::Command(Command::Start { tool, reply }))
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

fn supervise(
    registry: Arc<dyn ToolRegistry>,
    bus: EventBus,
    receiver: Receiver<Incoming>,
    worker_sender: Sender<Incoming>,
) {
    let mut arbiter = Arbiter::new();
    let mut workers: BTreeMap<ToolId, JoinHandle<()>> = BTreeMap::new();

    while let Ok(message) = receiver.recv() {
        match message {
            Incoming::Command(Command::Start { tool, reply }) => {
                let answer = match registry.available(tool) {
                    Err(unavailable) => StartReply::Unavailable {
                        line: unavailable.line,
                    },
                    Ok(()) => match arbiter.start(tool) {
                        Start::Started(handle) => {
                            spawn_worker(handle, &registry, &worker_sender, &bus, &mut workers);
                            StartReply::Started
                        }
                        Start::Deferred { line } => {
                            // There is no run to attach a `state` to yet, so the waiting line is a
                            // `log` event; the start itself happens when the game is handed over.
                            bus.emit(events::log_event("info", line.clone()));
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
                // Nothing pending is allowed to start on the way out.
                let _ = arbiter.cancel_pending();
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
                            &registry,
                            &worker_sender,
                            &bus,
                        ),
                        Ok(Incoming::Command(_)) => {}
                        Err(_) => break,
                    }
                }
                bus.emit(events::core_stopping_event(&reason));
                let _ = reply.send(());
                break;
            }
            Incoming::Worker(event) => handle_worker(
                event,
                &mut arbiter,
                &mut workers,
                &registry,
                &worker_sender,
                &bus,
            ),
        }
    }
}

fn handle_worker(
    event: WorkerEvent,
    arbiter: &mut Arbiter,
    workers: &mut BTreeMap<ToolId, JoinHandle<()>>,
    registry: &Arc<dyn ToolRegistry>,
    worker_sender: &Sender<Incoming>,
    bus: &EventBus,
) {
    match event {
        WorkerEvent::Opened { tool, snapshot } => {
            // A stop may already have been processed between the send and this handling; then the
            // card is Stopping and a stale Running event would be a lie.
            if arbiter.is_running(tool) && !arbiter.is_stopping(tool) {
                bus.emit(events::state_event(&snapshot));
            }
        }
        WorkerEvent::Done { tool, reason } => {
            // THE EXIT PATH: the final snapshot, tool.finished, the slot and the game all settle
            // here, in one place, from the worker's own last message.
            let finish = arbiter.finished(tool, reason.clone());
            if let Some(join) = workers.remove(&tool) {
                let _ = join.join();
            }
            if let Some(snapshot) = &finish.final_snapshot {
                bus.emit(events::state_event(snapshot));
            }
            bus.emit(events::tool_finished_event(tool, &reason));

            match finish.game {
                GameRelease::StartPending(handle) => {
                    spawn_worker(handle, registry, worker_sender, bus, workers);
                }
                GameRelease::Granted(waiter) => {
                    bus.emit(events::log_event(
                        "info",
                        format!("\"{waiter}\" has the game."),
                    ));
                }
                GameRelease::Freed => {}
            }
        }
    }
}

fn spawn_worker(
    handle: RunHandle,
    registry: &Arc<dyn ToolRegistry>,
    worker_sender: &Sender<Incoming>,
    bus: &EventBus,
    workers: &mut BTreeMap<ToolId, JoinHandle<()>>,
) {
    // The opening phase's snapshot: the card says Starting while the board/models are opened.
    if let Some(snapshot) = handle.state.read() {
        bus.emit(events::state_event(&snapshot));
    }
    let registry = Arc::clone(registry);
    let sender = worker_sender.clone();
    let tool = handle.tool;
    let join = thread::spawn(move || worker_loop(handle, registry, sender));
    workers.insert(tool, join);
}

/// One run. Sends `Opened` once the session exists, runs the loop, and sends `Done` as its last act.
fn worker_loop(handle: RunHandle, registry: Arc<dyn ToolRegistry>, supervisor: Sender<Incoming>) {
    let tool = handle.tool;
    let session: Session = match registry.create(tool) {
        Ok(session) => session,
        Err(line) => {
            let _ = supervisor.send(Incoming::Worker(WorkerEvent::Done {
                tool,
                reason: StopReason::error(line),
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
        }));
        return;
    }

    let running = Arc::new(ToolSnapshot::new(tool).running(ToolPhase::Running));
    handle.state.publish((*running).clone());
    let _ = supervisor.send(Incoming::Worker(WorkerEvent::Opened {
        tool,
        snapshot: running,
    }));

    let mut session = session;
    let reason = runner::run(
        &mut *session.tool,
        &handle,
        &RealClock,
        &mut *session.effects,
    );
    let _ = supervisor.send(Incoming::Worker(WorkerEvent::Done { tool, reason }));
}

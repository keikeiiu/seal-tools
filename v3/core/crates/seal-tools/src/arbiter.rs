//! The arbiter: one game, one tool at a time, plus the resident pet feeder's exception.
//!
//! v2 kept this in `LauncherService` and it was the untested seam (`PROGRESS.md` entry 59 — the
//! registry was a plain `Dictionary` read and written by two threads, and no test could be written
//! because the runner map closed over config, the dictionary and the port). Here the rule is a pure
//! state machine: starts, stops and the game claim are decided and returned, while the loop that
//! performs effects lives in `seal-core`. The tests drive it with no board, no game and no threads.
//!
//! Ported rules (all lines are v2's wording):
//!
//! - **One foreground tool at a time.** v3's protocol refuses a second start with `tool_busy`; v2
//!   auto-stopped the first, which is one click away from the same thing but can no longer be
//!   silent. The line is `InputArbiter.Busy`'s: `"tuner" is using the game right now — stop it, then
//!   try again.`
//! - **A stop abandons the work, not the cleanup.** `stop` only cancels and marks the phase
//!   `stopping`; the slot is not cleared until `finished`, so the card keeps saying `Stopping…` and
//!   nothing can start on top of a loop that is still leaving.
//! - **The resident feeder.** The pet feeder is armed while another tool holds the game, keeps its
//!   standing line, and defers: `request_game` announces it is waiting (which is what lets the
//!   spammer stand down), and it receives the game when the holder releases it. A foreground start
//!   behind the feeder's own reload is deferred rather than refused, with v2's
//!   `Waiting for the pet feeder to finish its reload (pet)…`; giving up on that wait is v2's
//!   `Start cancelled.` or `The pet feeder is still using the game (pet) — <tool> was not started.`
//! - **The spammer yields; nothing else does.** `should_stand_down` is `PortGate.ShouldStandDown`'s
//!   one live rule: the holder steps aside only while somebody is actually waiting, and the spammer
//!   is the only tool that acts on it (`SkillSpammer`'s `[YIELD]` path).
//!
//! The boundary against `PortGate`: the arbiter owns both the tool registry and the game claim so
//! the two cannot disagree. v2 needed the gate because a claim can be refused but never interrupted;
//! here the claim is decided in the same place as the registry, and the two-writer hazard is gone by
//! construction (one state machine, one thread).

use std::collections::BTreeMap;
use std::sync::Arc;

use seal_domain::decision::{StopKind, StopReason};

use crate::cancel::CancelToken;
use crate::tool::{StateCell, ToolId, ToolPhase, ToolSnapshot};

/// The line and the machine-readable reason a refusal carries. The protocol maps `Busy` to
/// `tool_busy`; nothing else refuses today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub kind: RefusalKind,
    pub line: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalKind {
    /// Another run holds the game (or the same tool is already engaged).
    Busy,
}

impl Refusal {
    /// v2's `InputArbiter.Busy`, verbatim: it names the holder and says how to proceed.
    fn busy(holder: ToolId) -> Self {
        Self {
            kind: RefusalKind::Busy,
            line: format!("\"{holder}\" is using the game right now — stop it, then try again."),
        }
    }

    /// Defined where v2 has no line: a generic `tool.start` can arrive for a tool whose loop is
    /// still leaving, and the busy line would read as "it is running" rather than "it is going".
    fn still_stopping(tool: ToolId) -> Self {
        Self {
            kind: RefusalKind::Busy,
            line: format!(
                "\"{tool}\" is still stopping — wait for it to finish, then start again."
            ),
        }
    }
}

/// What a `start` did.
#[derive(Debug)]
pub enum Start {
    Started(RunHandle),
    /// Waiting behind the resident feeder's bounded use of the game (v2's `WaitForGameAsync`).
    /// `abandon_pending`/`pending_timeout` end the wait with the line the player sees.
    Deferred {
        line: String,
    },
    Refused(Refusal),
}

/// What a `stop` did.
#[derive(Debug)]
pub enum Stop {
    /// The token is cancelled and the phase is `stopping`. The supervisor must let the tool loop
    /// leave (its cleanups and held-input guard run on the way out) and then call
    /// [`Arbiter::finished`], which clears the slot and releases the game.
    Cancelling {
        tool: ToolId,
        handle: RunHandle,
    },
    NotRunning,
}

/// What a `request_game` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameUse {
    /// The caller holds the game (or just took it). It may drive.
    Granted,
    /// Somebody else holds it and will not be interrupted; the caller is now announced as waiting.
    Waiting { holder: ToolId },
    /// The holder can stand down for the caller: the spammer, which yields to any waiter.
    Preempt { holder: ToolId },
}

/// What a `release_game`/`finished` left behind.
#[derive(Debug, Default)]
pub enum GameRelease {
    /// The game is free and nobody is waiting for it.
    #[default]
    Freed,
    /// The game passed to the announced waiter; the supervisor tells it the game is free.
    Granted(ToolId),
    /// A foreground start was waiting behind the feeder and is now starting: run this handle.
    StartPending(RunHandle),
}

#[derive(Debug, Default)]
pub struct Finish {
    /// The final snapshot (idle, running false, the reason's message) if this tool had a run.
    pub final_snapshot: Option<Arc<ToolSnapshot>>,
    pub game: GameRelease,
}

/// One run's registry entry. The token and state cell travel together and are cloneable, so a
/// supervisor can hand them to the loop while the arbiter keeps its own copy.
#[derive(Debug, Clone)]
pub struct RunHandle {
    pub tool: ToolId,
    pub token: CancelToken,
    pub state: StateCell,
}

#[derive(Debug)]
struct Run {
    handle: RunHandle,
    stopping: bool,
}

/// The tool lifecycle, as a pure state machine.
#[derive(Debug, Default)]
pub struct Arbiter {
    runs: BTreeMap<ToolId, Run>,
    /// The foreground tool the Stop buttons mean, or `None`. Never the resident tool (v2's
    /// `_currentId` rule): the feeder is background furniture, not a watched run.
    current: Option<ToolId>,
    /// Who may drive the game right now.
    game: Option<ToolId>,
    /// Who has announced they want the game.
    waiting: Option<ToolId>,
    /// A foreground start waiting for the feeder's reload to end.
    pending: Option<ToolId>,
}

impl Arbiter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ── reads ────────────────────────────────────────────────────────────────────────────────────

    /// The lifecycle phase the card should render, read from the published snapshot — the arbiter
    /// and the tool loop publish to one cell, so there is one answer rather than two.
    #[must_use]
    pub fn phase_of(&self, tool: ToolId) -> Option<ToolPhase> {
        self.snapshot(tool).map(|snapshot| snapshot.phase)
    }

    /// The foreground tool the Stop buttons mean.
    #[must_use]
    pub fn current(&self) -> Option<ToolId> {
        self.current
    }

    /// The tool holding the game, for a card's "waiting for …" line.
    #[must_use]
    pub fn game_owner(&self) -> Option<ToolId> {
        self.game
    }

    /// The announced waiter, for the card and for `should_stand_down`.
    #[must_use]
    pub fn waiting(&self) -> Option<ToolId> {
        self.waiting
    }

    /// The foreground start waiting behind the feeder's reload.
    #[must_use]
    pub fn pending(&self) -> Option<ToolId> {
        self.pending
    }

    #[must_use]
    pub fn is_running(&self, tool: ToolId) -> bool {
        self.runs.contains_key(&tool)
    }

    /// True between `stop` and `finished`: the loop is still leaving.
    #[must_use]
    pub fn is_stopping(&self, tool: ToolId) -> bool {
        self.runs.get(&tool).is_some_and(|run| run.stopping)
    }

    #[must_use]
    pub fn running(&self) -> Vec<ToolId> {
        self.runs.keys().copied().collect()
    }

    /// The published snapshot for a tool, if it has one.
    #[must_use]
    pub fn snapshot(&self, tool: ToolId) -> Option<Arc<ToolSnapshot>> {
        self.runs.get(&tool).and_then(|run| run.handle.state.read())
    }

    /// `PortGate.ShouldStandDown`'s live rule, with the one holder that acts on it: the SPAMMER
    /// steps aside, and only while somebody else is actually waiting (`SkillSpammer`'s `[YIELD]`
    /// path). Every other tool holds the game for its whole bounded run and does not poll, so a
    /// `true` for them would be a promise nobody keeps.
    #[must_use]
    pub fn should_stand_down(&self, tool: ToolId) -> bool {
        tool == ToolId::Spammer
            && self.game == Some(tool)
            && self.waiting.is_some_and(|waiter| waiter != tool)
    }

    // ── lifecycle ────────────────────────────────────────────────────────────────────────────────

    /// Starts a tool, or says why not.
    ///
    /// A foreground start takes the game for the whole run (v2's `WaitForGameAsync` claim); the
    /// resident feeder is armed without taking it, and requests it when it has work to do. A
    /// foreground start while the feeder is mid-reload is DEFERRED, with v2's waiting line.
    pub fn start(&mut self, tool: ToolId) -> Start {
        if let Some(run) = self.runs.get(&tool) {
            return Start::Refused(if run.stopping {
                Refusal::still_stopping(tool)
            } else {
                Refusal::busy(tool)
            });
        }

        if tool.is_resident() {
            // Armed, not current, and holding nothing: the feeder goes on with its schedule and
            // asks for the game when a row is due.
            return Start::Started(self.register(tool));
        }

        // One foreground tool at a time: anything already running with the game owns it. A holder
        // that is still stopping is named as such — the busy line would read as "it is running".
        if let Some(holder) = self.runs.keys().copied().find(|held| !held.is_resident()) {
            return Start::Refused(if self.is_stopping(holder) {
                Refusal::still_stopping(holder)
            } else {
                Refusal::busy(holder)
            });
        }

        if let Some(holder) = self.game {
            // Only the resident feeder can hold the game with no foreground run, and its reload is
            // BOUNDED — so v2 waited for it rather than refusing. The supervisor owns the timeout
            // (`pending_timeout`); the machine only records the wait.
            self.pending = Some(tool);
            return Start::Deferred {
                line: format!("Waiting for the pet feeder to finish its reload ({holder})…"),
            };
        }

        self.game = Some(tool);
        self.current = Some(tool);
        Start::Started(self.register(tool))
    }

    /// Stops the named tool, or — with `None` — whichever run the player means: the foreground tool,
    /// else the resident feeder ("stops whatever is running", `PROTOCOL.md` §4).
    pub fn stop(&mut self, target: Option<ToolId>) -> Stop {
        let Some(tool) = self.resolve_stop_target(target) else {
            return Stop::NotRunning;
        };
        let Some(run) = self.runs.get_mut(&tool) else {
            return Stop::NotRunning;
        };
        if run.stopping {
            // Already asked; a second stop has nothing to add.
            return Stop::NotRunning;
        }

        let handle = run.handle.clone();
        run.stopping = true;
        handle.token.cancel();
        if self.current == Some(tool) {
            self.current = None;
        }
        // A stop that arrives while the feeder waits behind this tool leaves nothing to grant.
        if self.waiting == Some(tool) {
            self.waiting = None;
        }
        publish_phase(&handle, ToolPhase::Stopping);

        Stop::Cancelling { tool, handle }
    }

    /// Stops every run. v2's shutdown path (`StopAll` with no keep predicate); each stop returns the
    /// handle the caller drains the way `stop` does.
    pub fn stop_all(&mut self) -> Vec<Stop> {
        let ids: Vec<ToolId> = self.runs.keys().copied().collect();
        ids.into_iter()
            .map(|tool| self.stop(Some(tool)))
            .filter(|stop| !matches!(stop, Stop::NotRunning))
            .collect()
    }

    /// The tool loop has left: publish its final state, clear the slot, release the game and hand a
    /// pending start its game. This is the ONLY place a slot is removed — which is what makes "a
    /// stopped tool's cleanup always runs" observable: the slot stays `stopping` until the loop is
    /// genuinely gone.
    pub fn finished(&mut self, tool: ToolId, reason: StopReason) -> Finish {
        let Some(run) = self.runs.remove(&tool) else {
            return Finish::default();
        };
        if self.current == Some(tool) {
            self.current = None;
        }
        if self.waiting == Some(tool) {
            self.waiting = None;
        }

        let final_snapshot = finish_snapshot(&run.handle, reason);
        let game = if self.game == Some(tool) {
            self.release_game_inner()
        } else {
            GameRelease::Freed
        };

        Finish {
            final_snapshot: Some(final_snapshot),
            game,
        }
    }

    // ── the game claim ───────────────────────────────────────────────────────────────────────────

    /// Asks for the game for `tool` — the feeder's reload, v2's `ClaimGame`.
    ///
    /// A holder that cannot be interrupted makes the caller WAIT, and the wait is announced so the
    /// one holder that can step aside (the spammer) does. Nothing is granted here by force.
    pub fn request_game(&mut self, tool: ToolId) -> GameUse {
        if self.game == Some(tool) {
            return GameUse::Granted;
        }
        if let Some(holder) = self.game {
            self.waiting = Some(tool);
            return if holder == ToolId::Spammer {
                GameUse::Preempt { holder }
            } else {
                GameUse::Waiting { holder }
            };
        }

        self.game = Some(tool);
        self.waiting = None;
        GameUse::Granted
    }

    /// The holder gives the game back without finishing (the feeder between reloads; the spammer
    /// standing down). A waiter is served first; otherwise a pending start begins.
    pub fn release_game(&mut self, tool: ToolId) -> GameRelease {
        if self.game != Some(tool) {
            return GameRelease::Freed;
        }
        self.release_game_inner()
    }

    fn release_game_inner(&mut self) -> GameRelease {
        self.game = None;
        if let Some(waiter) = self.waiting.take() {
            self.game = Some(waiter);
            return GameRelease::Granted(waiter);
        }
        if let Some(tool) = self.pending.take() {
            self.game = Some(tool);
            self.current = Some(tool);
            return GameRelease::StartPending(self.register(tool));
        }
        GameRelease::Freed
    }

    // ── giving up on a deferred start ───────────────────────────────────────────────────────────

    /// The player stopped waiting: v2's `LastArduinoError` line for a cancelled start.
    pub fn cancel_pending(&mut self) -> Option<&'static str> {
        self.pending.take().map(|_| "Start cancelled.")
    }

    /// The wait ran out (v2's 45 s): the refusal the Start handler shows, naming both sides.
    pub fn pending_timeout(&mut self) -> Option<Refusal> {
        let tool = self.pending.take()?;
        let holder = self
            .game
            .map_or_else(|| "pet".to_owned(), |holder| holder.to_string());
        Some(Refusal {
            kind: RefusalKind::Busy,
            line: format!(
                "The pet feeder is still using the game ({holder}) — {tool} was not started."
            ),
        })
    }

    // ── internals ───────────────────────────────────────────────────────────────────────────────

    fn resolve_stop_target(&self, target: Option<ToolId>) -> Option<ToolId> {
        match target {
            Some(tool) => Some(tool),
            None => self
                .current
                .or_else(|| self.runs.contains_key(&ToolId::Pet).then_some(ToolId::Pet)),
        }
    }

    fn register(&mut self, tool: ToolId) -> RunHandle {
        let handle = RunHandle {
            tool,
            token: CancelToken::new(),
            state: StateCell::new(ToolSnapshot::new(tool).running(ToolPhase::Starting)),
        };
        self.runs.insert(
            tool,
            Run {
                handle: handle.clone(),
                stopping: false,
            },
        );
        handle
    }
}

fn publish_phase(handle: &RunHandle, phase: ToolPhase) {
    let current = handle.state.read().map_or_else(
        || ToolSnapshot::new(handle.tool),
        |snapshot| (*snapshot).clone(),
    );
    handle.state.publish(current.running(phase));
}

/// The final snapshot of a run: idle, not running, the reason's message, and (for a failure) the
/// error beside it. v2's card fell through to a bare "stopped" when the registry entry vanished
/// before it read the state; the reason is published here so it cannot.
fn finish_snapshot(handle: &RunHandle, reason: StopReason) -> Arc<ToolSnapshot> {
    let error = if reason.kind == StopKind::Error {
        Some(reason.message.clone())
    } else {
        // A normal stop and a completed run are not failures; a guard-triggered stop carries its
        // reason in the message only (the card shows it as the stop line, not an error).
        None
    };
    let snapshot = Arc::new(
        ToolSnapshot::new(handle.tool)
            .stopped(reason.message)
            .countdown_s(None)
            .rows(Vec::new())
            .error(error),
    );
    handle.state.publish((*snapshot).clone());
    snapshot
}

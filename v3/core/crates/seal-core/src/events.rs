//! The event channel and the single writer: every event line a worker can produce goes through one
//! writer, so a worker can never interleave half a line with a response.
//!
//! `PROTOCOL.md` §1 says one JSON object per line on stdout; a worker thread writing directly to
//! stdout could cut a response in half, and the shell would see two unparseable lines instead of one
//! failure. Here workers only ever SEND, and the writer thread is the only thing that touches the
//! destination — responses and events share that fate.
//!
//! Events serialise exactly `PROTOCOL.md` §5: `state` from a [`ToolSnapshot`] (which is field-for-field
//! the event's data), `tool.finished` from the run's `StopReason`, `log` and `core.stopping`.

use std::io::Write;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use seal_domain::decision::{StopKind, StopReason};
use seal_proto::{Event, Response};
use seal_tools::tool::{ToolDetail, ToolId, ToolRow, ToolSnapshot};
use serde_json::{Value, json};

/// One line for the writer: a protocol response or an unsolicited event. They share a channel so the
/// writer can never see them out of order relative to each other.
#[derive(Debug)]
pub enum Outgoing {
    Response(Response),
    Event(Event),
}

/// Where events from the supervisor and its workers go.
///
/// Two backends for the same idea: the binary uses [`Self::Outgoing`] so events and responses pass
/// through the single writer; an in-process test uses [`event_channel`] and reads them directly.
#[derive(Debug, Clone)]
pub enum EventBus {
    Events(Sender<Event>),
    Outgoing(Sender<Outgoing>),
}

impl EventBus {
    /// Sends an event. A closed receiver means the process is going away, which is not an error for
    /// a worker that is about to be stopped by the same shutdown.
    pub fn emit(&self, event: Event) {
        match self {
            Self::Events(sender) => {
                let _ = sender.send(event);
            }
            Self::Outgoing(sender) => {
                let _ = sender.send(Outgoing::Event(event));
            }
        }
    }
}

/// An event-only channel, for the in-process supervisor tests.
pub fn event_channel() -> (EventBus, Receiver<Event>) {
    let (sender, receiver) = mpsc::channel();
    (EventBus::Events(sender), receiver)
}

/// The one writer: a thread that owns the destination and writes whatever is sent to it, one whole
/// encoded line at a time.
pub struct EventWriter {
    outgoing: Sender<Outgoing>,
    join: Option<JoinHandle<()>>,
}

impl EventWriter {
    /// Starts the writer thread over `destination`. The returned [`EventBus`] is the handle workers
    /// use; the [`EventWriter`] is kept by the stdio loop for responses and to stop cleanly.
    pub fn spawn(destination: Box<dyn Write + Send>) -> (Self, EventBus) {
        let (outgoing, receiver) = mpsc::channel();
        let join = thread::spawn(move || write_loop(destination, receiver));
        (
            Self {
                outgoing: outgoing.clone(),
                join: Some(join),
            },
            EventBus::Outgoing(outgoing),
        )
    }

    pub fn response(&self, response: Response) {
        let _ = self.outgoing.send(Outgoing::Response(response));
    }

    /// Drops the sending side, waits for the writer to drain and joins it.
    ///
    /// The writer stops when the LAST sender is gone, so every [`EventBus`] clone must be dropped
    /// first. In the binary the only other holder is the supervisor thread, and shutdown joins that
    /// thread before calling this — which is also what guarantees `core.stopping` is written before
    /// the process exits.
    pub fn finish(mut self) {
        drop(self.outgoing);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn write_loop(mut destination: Box<dyn Write + Send>, receiver: Receiver<Outgoing>) {
    for message in receiver {
        let encoded = match &message {
            Outgoing::Response(response) => seal_proto::framing::encode_line(response),
            Outgoing::Event(event) => seal_proto::framing::encode_line(event),
        };
        let Ok(line) = encoded else {
            eprintln!("seal-core: an outgoing message could not be encoded");
            break;
        };
        if destination
            .write_all(line.as_bytes())
            .and_then(|()| destination.flush())
            .is_err()
        {
            // The pipe is gone (the shell died); nothing useful is left to say.
            break;
        }
    }
}

// ── PROTOCOL.md §5 event shapes ──────────────────────────────────────────────────────────────

/// A `state` event for `snapshot`. The snapshot's fields ARE the event's data (`PROTOCOL.md` §5).
///
/// The rows are v2's `ToolRowState` with its `DateTime? NextAt` as `next_action_in_seconds`: the
/// crate has no calendar, and the shell only counts down, so the moment is expressed relative to the
/// publish. `PROTOCOL.md` §5 prints `rows: [{...}]` without fixing the inner shape; this is the v2
/// field set, and it is reported as an ambiguity for the spec.
pub fn state_event(snapshot: &ToolSnapshot) -> Event {
    Event::new("state", state_data(snapshot))
}

/// The data half of [`state_event`], exposed for tests and for a future golden fixture.
pub fn state_data(snapshot: &ToolSnapshot) -> Value {
    json!({
        "tool": snapshot.tool.as_str(),
        "running": snapshot.running,
        "phase": snapshot.phase.as_str(),
        "message": snapshot.message,
        "countdown_s": snapshot.countdown_s,
        "rows": snapshot.rows.iter().map(row_data).collect::<Vec<Value>>(),
        "detail": snapshot.detail.iter().map(detail_data).collect::<Vec<Value>>(),
        "error": snapshot.error,
    })
}

fn row_data(row: &ToolRow) -> Value {
    json!({
        "name": row.name,
        "status": row.status,
        "detail": row.detail,
        "next_action_in_seconds": row.next_action_in_seconds,
        "next_action": row.next_action.map(|action| action.as_str()),
    })
}

fn detail_data(detail: &ToolDetail) -> Value {
    json!({
        "label": detail.label,
        "value": detail.value,
    })
}

/// A `tool.finished` event: the tool, the protocol's reason word, the message the player reads, the
/// tool's own end-of-run summary (the supervisor's `WorkerEvent::Done`) and `ended_at`
/// (`PROTOCOL.md` §5). The reason is the same [`StopReason`] the final snapshot is published from, so
/// the two cannot disagree.
///
/// `summary` is additive `v:1` (G11): a tool that has nothing to add publishes `null`, and an older
/// core's missing fields are null. `ended_at` is the core's UTC clock at the moment the event is
/// built (the summary says WHAT the run did; this says WHEN) — the shell renders it and never invents
/// a timestamp from its own clock.
pub fn tool_finished_event(tool: ToolId, reason: &StopReason, summary: Option<&Value>) -> Event {
    Event::new(
        "tool.finished",
        json!({
            "tool": tool.as_str(),
            "reason": stop_kind_name(reason.kind),
            "message": reason.message,
            "summary": summary,
            "ended_at": utc_now_rfc3339(),
        }),
    )
}

/// The current UTC instant in `PROTOCOL.md`'s ISO-8601 form (`2026-10-30T19:05:00Z`).
///
/// `std` exposes UTC without a calendar dependency; the conversion is Howard Hinnant's
/// days-to-civil algorithm, the same one `seal-win` uses for its non-Windows fallback. Seconds
/// resolution: the timestamp names the finish, not a frame.
pub(crate) fn utc_now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3_600,
        (of_day % 3_600) / 60,
        of_day % 60
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    (year + i64::from(month <= 2), month, day)
}

/// An informational `log` line (`PROTOCOL.md` §5). v3 defines no level enum in the spec beyond the
/// example; the observed values become `info`/`warn`/`error`.
pub fn log_event(level: &str, line: impl Into<String>) -> Event {
    Event::new("log", json!({"level": level, "line": line.into()}))
}

/// `core.stopping {reason}`: the core is leaving; the shell shows why and disables its controls.
pub fn core_stopping_event(reason: &str) -> Event {
    Event::new("core.stopping", json!({"reason": reason}))
}

/// A `config.changed` event (`PROTOCOL.md` §5): the core wrote a block the UI displays, carrying the
/// new session revision and the block's effective values.
///
/// Emitted only for CORE-side writes — `config.apply`, `ui.window` and `calibration.save`. A
/// `config.reload` is the shell asking to re-read; its answer already carries the whole config, so it
/// emits nothing and the shell cannot mistake its own reload for an external write. The event is
/// queued during dispatch and sent AFTER the response line, because a shell that sees an event before
/// the answer to its own request would have to reconcile against itself (`dispatch::CoreState`).
pub fn config_changed_event(rev: u64, block: &str, values: Value) -> Event {
    Event::new(
        "config.changed",
        json!({"rev": rev, "block": block, "values": values}),
    )
}

fn stop_kind_name(kind: StopKind) -> &'static str {
    match kind {
        StopKind::Done => "done",
        StopKind::Cancelled => "cancelled",
        StopKind::Error => "error",
        StopKind::Guard => "guard",
    }
}

//! The event channel + single writer, pinned against `PROTOCOL.md` §5 and §1.
//!
//! The interleaving test is the reason this exists: a worker thread writing its own line to stdout
//! could split a response in half. Here every event and every response goes through one writer
//! thread, and the test sends both kinds concurrently from two threads to prove the destination only
//! ever receives whole lines.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;

use seal_core::events::{
    EventBus, EventWriter, core_stopping_event, event_channel, log_event, state_data, state_event,
    tool_finished_event,
};
use seal_domain::decision::{StopKind, StopReason};
use seal_proto::Response;
use seal_tools::tool::{ToolDetail, ToolId, ToolPhase, ToolRow, ToolSnapshot};
use serde_json::{Value, json};

/// A `Write` that appends to shared memory, so a test can read exactly what the writer wrote.
#[derive(Clone, Default)]
struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

impl SharedBuffer {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().expect("not poisoned").clone())
            .expect("the writer writes UTF-8")
    }
}

impl Write for SharedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("not poisoned")
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_state_event_is_the_snapshots_fields() {
    let snapshot = ToolSnapshot::new(ToolId::Pet)
        .running(ToolPhase::Running)
        .message("row 2: looking")
        .countdown_s(Some(12.5))
        .rows(vec![
            ToolRow::new("row 1", "boarding").detail(Some("藍天 · 3/min")),
            ToolRow::new("row 2", "waiting for a reload").next_action_in_seconds(Some(30.0)),
        ])
        .detail(vec![
            ToolDetail::labeled("Grade", "DG"),
            ToolDetail::plain("attack speed=+10"),
        ])
        .error(None);

    let event = state_event(&snapshot);

    assert_eq!(event.event, "state");
    assert_eq!(
        event.data,
        json!({
            "tool": "pet",
            "running": true,
            "phase": "running",
            "message": "row 2: looking",
            "countdown_s": 12.5,
            "rows": [
                {
                    "name": "row 1",
                    "status": "boarding",
                    "detail": "藍天 · 3/min",
                    "next_action_in_seconds": null
                },
                {
                    "name": "row 2",
                    "status": "waiting for a reload",
                    "detail": null,
                    "next_action_in_seconds": 30.0
                }
            ],
            "detail": [
                {"label": "Grade", "value": "DG"},
                {"label": null, "value": "attack speed=+10"}
            ],
            "error": null
        })
    );
    // The helper and the event agree; neither is a second hand-kept shape.
    assert_eq!(state_data(&snapshot), event.data);
}

#[test]
fn a_finished_event_carries_the_protocol_reason_words() {
    let cases = [
        (StopKind::Done, "done"),
        (StopKind::Cancelled, "cancelled"),
        (StopKind::Error, "error"),
        (StopKind::Guard, "guard"),
    ];

    for (kind, word) in cases {
        let reason = StopReason::new(kind, "DONE — 4 row(s)");
        let event = tool_finished_event(ToolId::Pet, &reason);

        assert_eq!(event.event, "tool.finished");
        assert_eq!(
            event.data,
            json!({"tool": "pet", "reason": word, "message": "DONE — 4 row(s)"})
        );
    }
}

#[test]
fn log_and_stopping_events_have_the_protocol_shape() {
    assert_eq!(
        log_event("warn", "ocr scan retried").data,
        json!({"level": "warn", "line": "ocr scan retried"})
    );
    assert_eq!(
        core_stopping_event("shutdown").data,
        json!({"reason": "shutdown"})
    );
    assert_eq!(core_stopping_event("shutdown").event, "core.stopping");
}

#[test]
fn an_in_process_event_channel_delivers_events_unchanged() {
    let (bus, receiver) = event_channel();
    bus.emit(log_event("info", "hello"));

    let event = receiver.recv().expect("the event arrived");
    assert_eq!(event.event, "log");
}

/// The single-writer proof: responses and events are SENT concurrently from two threads and every
/// line the destination receives is a whole JSON object. Before the writer existed, a worker could
/// interleave into a response's line and produce two unparseable ones.
#[test]
fn one_writer_serialises_responses_and_events_without_interleaving() {
    const COUNT: usize = 100;

    let buffer = SharedBuffer::default();
    let (writer, bus) = EventWriter::spawn(Box::new(buffer.clone()));

    let event_sender = {
        let event_bus = bus.clone();
        thread::spawn(move || {
            for index in 0..COUNT {
                event_bus.emit(log_event("info", format!("event {index}")));
            }
        })
    };
    for index in 0..COUNT {
        writer.response(Response::ok(&index.to_string(), json!({"index": index})));
    }
    event_sender.join().expect("the event thread ends");
    // `finish` waits for the writer to drain, and the writer only stops when the LAST sender is
    // gone — so every EventBus clone must be dropped first. In the binary that means joining the
    // supervisor thread before finishing the writer, which shutdown does.
    drop(bus);
    writer.finish();

    let text = buffer.text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), COUNT * 2, "every message is exactly one line");
    assert!(
        text.ends_with('\n'),
        "the last line is terminated for the shell"
    );

    let mut responses = 0;
    let mut events = 0;
    for line in lines {
        let value: Value = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("not a whole JSON line ({error}): {line:?}"));
        if value.get("event").is_some() {
            events += 1;
        } else {
            assert!(value.get("id").is_some() || value.get("ok").is_some());
            responses += 1;
        }
    }
    assert_eq!(responses, COUNT);
    assert_eq!(events, COUNT);
}

#[test]
fn a_closed_destination_stops_the_writer_without_panicking() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "the shell went away",
            ))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let (writer, bus) = EventWriter::spawn(Box::new(Broken));
    writer.response(Response::ok("1", json!({})));
    bus.emit(log_event("info", "after the pipe died"));
    writer.finish(); // must not panic; the writer simply stops
}

#[test]
fn the_bus_is_cloneable_and_a_dead_receiver_is_not_an_error() {
    let (bus, receiver) = event_channel();
    let clone = bus.clone();
    drop(receiver);

    clone.emit(log_event("info", "nobody is listening"));
    let _: EventBus = bus;
}

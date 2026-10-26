//! The per-run FILE logs (G8), increment 1: the logger's v2 shapes, OCR JSONL records, capture PNGs,
//! append-not-truncate across boots, and the "a failed write never fails the caller" rule.
//!
//! The real OCR model scan is not exercised here (the models are not on every machine — the existing
//! `ocr_captures.rs` is the model-gated pipeline test). The scan RECORDING path is the same helper
//! `ocr.scan` calls after the pipeline returns, so these tests drive it directly with a synthetic
//! `ModeResult`, which keeps them deterministic and hardware-free.

mod common;

use std::fs;
use std::path::Path;

use common::TestDir;
use seal_core::logs::Logs;
use seal_ocr::modes::{Mode, ModeResult};
use seal_tools::tool::ToolId;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use seal_core::dispatch::{CoreState, handle};
use seal_core::registry::{Session, ToolRegistry, Unavailable};
use seal_core::supervisor::{StartReply, StopReply, Supervisor};
use seal_domain::decision::{Decision, Key, StopReason};
use seal_domain::geometry::Point;
use seal_proto::{Event, Request};
use seal_tools::game::GamePort;
use seal_tools::runner::{EffectError, Effects, Tool, ToolStatus};

/// Magic bytes, not a valid image: the logger copies bytes and never decodes them.
const PNG_BYTES: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x01, 0x02];

fn confirmed_attributes() -> ModeResult {
    ModeResult {
        lines: vec!["GRADE G".to_owned(), "STR +1".to_owned()],
        attributes: Some(vec![
            vec!["STR".to_owned(), "+1".to_owned()],
            vec!["DEX".to_owned(), "+2".to_owned()],
            vec!["INT".to_owned(), "+3".to_owned()],
        ]),
        grade: Some("G".to_owned()),
        remaining: Some(7),
    }
}

fn read_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} reads: {error}", path.display()))
        .lines()
        .map(str::to_owned)
        .collect()
}

fn parse_record(line: &str) -> Value {
    serde_json::from_str(line).unwrap_or_else(|error| panic!("a JSONL line: {error}: {line}"))
}

/// v2's `OcrEngine.ScanOnce` timestamp: `yyyyMMdd_HHmmss_fff`.
fn is_scan_stamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 19
        && bytes.iter().enumerate().all(|(index, byte)| {
            if index == 8 || index == 15 {
                *byte == b'_'
            } else {
                byte.is_ascii_digit()
            }
        })
}

/// v2's text-line prefix date/time part: `yyyy-MM-dd HH:mm:ss`.
fn is_line_stamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 19
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b' ',
            13 | 16 => *byte == b':',
            _ => byte.is_ascii_digit(),
        })
}

#[test]
fn a_completed_scan_writes_one_parseable_jsonl_record() {
    let dir = TestDir::new();
    let logs = Logs::new(dir.path());
    let outcome = confirmed_attributes();

    let result = logs.record_scan(Mode::Attributes, &outcome, None, false);

    let lines = read_lines(&dir.path_of("logs/ocr_log.jsonl"));
    assert_eq!(lines.len(), 1, "one record per completed scan");
    let record = parse_record(&lines[0]);
    let stamp = record["timestamp"]
        .as_str()
        .expect("the record carries v2's timestamp");
    assert!(
        is_scan_stamp(stamp),
        "the timestamp is yyyyMMdd_HHmmss_fff: {stamp}"
    );
    assert_eq!(record["grade"], "G");
    assert_eq!(record["remaining"], 7);
    assert_eq!(record["lines"][0], "GRADE G");
    assert_eq!(record["attributes"][2][1], "+3");
    assert!(record["capture_path"].is_null());
    // The result keeps the protocol's five fields; the timestamp lives only in the file record.
    assert_eq!(result["grade"], record["grade"]);
    assert_eq!(result["remaining"], record["remaining"]);
    assert_eq!(result["capture_path"], Value::Null);
    assert!(result.get("timestamp").is_none());
    // A confirmed attributes scan is not an error record.
    assert!(!dir.exists("logs/ocr_errors.jsonl"));
}

#[test]
fn save_captures_writes_the_png_and_returns_a_relative_capture_path() {
    let dir = TestDir::new();
    let logs = Logs::new(dir.path());

    let result = logs.record_scan(
        Mode::Attributes,
        &confirmed_attributes(),
        Some(PNG_BYTES),
        true,
    );

    let capture_path = result["capture_path"]
        .as_str()
        .expect("save_captures on returns the path");
    assert!(
        capture_path.starts_with("logs/captures/capture_"),
        "relative, forward slashes: {capture_path}"
    );
    assert!(capture_path.ends_with(".png"), "{capture_path}");
    assert_eq!(
        fs::read(dir.path_of(capture_path)).expect("the capture exists"),
        PNG_BYTES,
        "captures are the frame's own PNG bytes"
    );
    let record = parse_record(&read_lines(&dir.path_of("logs/ocr_log.jsonl"))[0]);
    assert_eq!(record["capture_path"], capture_path);
}

#[test]
fn save_captures_off_writes_no_png_and_a_null_capture_path() {
    let dir = TestDir::new();
    let logs = Logs::new(dir.path());

    let result = logs.record_scan(
        Mode::Attributes,
        &confirmed_attributes(),
        Some(PNG_BYTES),
        false,
    );

    assert!(result["capture_path"].is_null());
    assert!(
        !dir.path_of("logs/captures").exists(),
        "no captures directory is created when the flag is off"
    );
    assert!(read_lines(&dir.path_of("logs/ocr_log.jsonl")).len() == 1);
}

#[test]
fn an_unconfirmed_attributes_scan_is_also_recorded_as_an_ocr_error() {
    let dir = TestDir::new();
    let logs = Logs::new(dir.path());
    let unconfirmed = ModeResult {
        lines: Vec::new(),
        attributes: Some(vec![Vec::new(), Vec::new(), Vec::new()]),
        grade: None,
        remaining: None,
    };

    logs.record_scan(Mode::Attributes, &unconfirmed, None, false);

    assert_eq!(read_lines(&dir.path_of("logs/ocr_log.jsonl")).len(), 1);
    let errors = read_lines(&dir.path_of("logs/ocr_errors.jsonl"));
    assert_eq!(errors.len(), 1, "one error record for the unconfirmed scan");
    let record = parse_record(&errors[0]);
    assert!(
        is_scan_stamp(record["timestamp"].as_str().expect("stamped")),
        "the error record carries the same stamp"
    );
}

#[test]
fn text_lines_use_v2_prefixes_and_holdspace_keeps_its_own_shape() {
    let dir = TestDir::new();
    let logs = Logs::new(dir.path());

    logs.tool_line(ToolId::Pet, "reload started");
    logs.holdspace_line("space released");
    logs.error_line("boom");

    let pet = read_lines(&dir.path_of("logs/pet.log"));
    assert_eq!(pet.len(), 1);
    let (stamp, message) = pet[0].split_at(20);
    assert!(is_line_stamp(&stamp[..19]), "the v2 text prefix: {stamp}");
    assert_eq!(stamp.chars().nth(19), Some(' '));
    assert_eq!(message, "reload started");

    let holdspace = read_lines(&dir.path_of("logs/holdspace.log"));
    assert_eq!(holdspace.len(), 1);
    assert!(
        holdspace[0].contains(" [svc] space released"),
        "v2's HoldDiag shape: {}",
        holdspace[0]
    );
    let (hold_stamp, _) = holdspace[0].split_at(12);
    assert_eq!(hold_stamp.len(), 12, "HH:mm:ss.fff: {hold_stamp}");

    let errors = read_lines(&dir.path_of("logs/error.log"));
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0].starts_with(&format!("{}: ", &errors[0][..19])),
        "v2's error.log shape: {}",
        errors[0]
    );
    assert!(errors[0].ends_with(": boom"));
}

#[test]
fn two_boots_append_rather_than_truncate() {
    let dir = TestDir::new();
    {
        let first = Logs::new(dir.path());
        first.tool_line(ToolId::Gem, "boot one");
        first.flush();
    }
    {
        let second = Logs::new(dir.path());
        second.tool_line(ToolId::Gem, "boot two");
    }

    let lines = read_lines(&dir.path_of("logs/gem.log"));
    assert_eq!(lines.len(), 2, "append, never truncate: {lines:?}");
    assert!(lines[0].ends_with("boot one"));
    assert!(lines[1].ends_with("boot two"));
}

#[test]
fn an_unwritable_logs_dir_is_swallowed_and_counted() {
    let dir = TestDir::new();
    // A FILE where the config root should be: `<root>/logs` can never be created beneath it.
    fs::write(dir.path_of("blocked"), b"not a directory").expect("the blocker writes");
    let logs = Logs::new(dir.path_of("blocked"));

    // None of these may panic, and the scan still returns its protocol result.
    let result = logs.record_scan(
        Mode::Attributes,
        &confirmed_attributes(),
        Some(PNG_BYTES),
        true,
    );
    logs.tool_line(ToolId::Pet, "never lands");
    logs.holdspace_line("never lands");
    logs.error_line("never lands");
    logs.flush();

    assert!(result["capture_path"].is_null());
    assert!(
        logs.write_failures() >= 4,
        "every failed write is counted, not propagated: {}",
        logs.write_failures()
    );
}

// ── increment 2: the supervisor's per-tool lines and `error.log` ─────────────────────────────

/// The effects half a scripted run needs: every decision here is `Wait`/`Stop`, whose default
/// methods are already no-ops.
struct NoEffects;

impl Effects for NoEffects {
    fn click(&mut self, _at: Point) -> Result<(), EffectError> {
        Err(EffectError::new("the scripted run never clicks"))
    }

    fn key(&mut self, _key: Key) -> Result<(), EffectError> {
        Err(EffectError::new("the scripted run never presses a key"))
    }
}

/// A run whose decisions and status words are scripted, so the `<tool>.log` sequence is deterministic.
struct ScriptedTool {
    id: ToolId,
    script: Vec<Decision>,
    messages: Vec<String>,
    step: usize,
}

impl Tool for ScriptedTool {
    fn id(&self) -> ToolId {
        self.id
    }

    fn decide(&mut self) -> Decision {
        if self.step < self.script.len() {
            let decision = self.script[self.step].clone();
            self.step += 1;
            decision
        } else {
            Decision::Stop(StopReason::done("script ended"))
        }
    }

    fn status(&self) -> ToolStatus {
        let message = self
            .messages
            .get(self.step.saturating_sub(1))
            .cloned()
            .unwrap_or_default();
        ToolStatus {
            message,
            ..ToolStatus::default()
        }
    }
}

/// The one-run registry the log tests use: either a scripted session or a scripted opening failure.
struct ScriptedRegistry {
    tool: Mutex<Option<Box<dyn Tool + Send>>>,
    opening_error: Mutex<Option<String>>,
}

impl ScriptedRegistry {
    fn scripted(id: ToolId, script: Vec<Decision>, messages: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            tool: Mutex::new(Some(Box::new(ScriptedTool {
                id,
                script,
                messages: messages
                    .iter()
                    .map(|message| (*message).to_owned())
                    .collect(),
                step: 0,
            }))),
            opening_error: Mutex::new(None),
        })
    }

    fn failing(line: &str) -> Arc<Self> {
        Arc::new(Self {
            tool: Mutex::new(None),
            opening_error: Mutex::new(Some(line.to_owned())),
        })
    }
}

impl ToolRegistry for ScriptedRegistry {
    fn available(&self, _tool: ToolId) -> Result<(), Unavailable> {
        Ok(())
    }

    fn create(
        &self,
        _tool: ToolId,
        _game: Arc<dyn GamePort>,
        _args: &Value,
    ) -> Result<Session, String> {
        if let Some(line) = self.opening_error.lock().expect("not poisoned").take() {
            return Err(line);
        }
        let tool = self
            .tool
            .lock()
            .expect("not poisoned")
            .take()
            .expect("one run per registry");
        Ok(Session {
            tool,
            effects: Box::new(NoEffects),
        })
    }
}

fn next_event(events: &std::sync::mpsc::Receiver<Event>) -> Event {
    events
        .recv_timeout(Duration::from_secs(5))
        .expect("an event arrives")
}

fn wait_running(events: &std::sync::mpsc::Receiver<Event>) {
    loop {
        let event = next_event(events);
        assert_ne!(event.event, "core.stopping");
        if event.event == "state" && event.data["phase"] == "running" {
            return;
        }
    }
}

fn wait_finished(events: &std::sync::mpsc::Receiver<Event>) -> Event {
    loop {
        let event = next_event(events);
        assert_ne!(event.event, "core.stopping");
        if event.event == "tool.finished" {
            return event;
        }
    }
}

/// Strip the v2 text prefix (`yyyy-MM-dd HH:mm:ss `) from a `<tool>.log` line.
fn message_of(line: &str) -> String {
    line.chars().skip(20).collect()
}

fn tool_log(dir: &TestDir, tool: ToolId) -> Vec<String> {
    read_lines(&dir.path_of(&format!("logs/{tool}.log")))
}

#[test]
fn a_fake_run_logs_the_start_only_changed_statuses_and_the_finish() {
    let dir = TestDir::new();
    let logs = Arc::new(Logs::new(dir.path()));
    let registry = ScriptedRegistry::scripted(
        ToolId::Tuner,
        vec![
            Decision::wait(0.0),
            Decision::wait(0.0),
            Decision::stop(StopReason::done("DONE — 2")),
        ],
        &["first", "first", "second"],
    );
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::clone(&logs),
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Tuner, &Value::Null),
        StartReply::Started
    );
    let finished = wait_finished(&events);
    supervisor.shutdown("test");

    let messages: Vec<String> = tool_log(&dir, ToolId::Tuner)
        .iter()
        .map(|line| message_of(line))
        .collect();
    assert_eq!(
        messages.join(" | "),
        "run started | first | second | finished — DONE — 2",
        "start, CHANGED messages only, then the final line"
    );
    assert_eq!(finished.data["reason"], "done");
    assert!(logs.write_failures() == 0);
}

#[test]
fn a_stop_logs_a_stopped_final_line_not_a_finished_one() {
    let dir = TestDir::new();
    let logs = Arc::new(Logs::new(dir.path()));
    let registry = ScriptedRegistry::scripted(ToolId::Gem, vec![Decision::wait(30.0)], &[]);
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::clone(&logs),
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Gem, &Value::Null),
        StartReply::Started
    );
    wait_running(&events);
    assert_eq!(
        supervisor.stop_tool(Some(ToolId::Gem)),
        StopReply::Stopping { tool: ToolId::Gem }
    );
    let finished = wait_finished(&events);
    supervisor.shutdown("test");

    let messages: Vec<String> = tool_log(&dir, ToolId::Gem)
        .iter()
        .map(|line| message_of(line))
        .collect();
    assert_eq!(messages.join(" | "), "run started | stopped — Stopped.");
    assert_eq!(finished.data["reason"], "cancelled");
}

#[test]
fn an_opening_failure_writes_error_log_and_no_tool_log() {
    let dir = TestDir::new();
    let logs = Arc::new(Logs::new(dir.path()));
    let registry = ScriptedRegistry::failing("the board could not be opened");
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::clone(&logs),
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Quest, &Value::Null),
        StartReply::Started
    );
    let finished = wait_finished(&events);
    supervisor.shutdown("test");

    assert_eq!(finished.data["reason"], "error");
    let errors = read_lines(&dir.path_of("logs/error.log"));
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0].ends_with("quest failed to start: the board could not be opened"),
        "the opening line is in error.log: {}",
        errors[0]
    );
    assert!(
        !dir.exists("logs/quest.log"),
        "a run that never began has no tool log"
    );
}

#[test]
fn an_internal_command_error_writes_error_log() {
    let dir = TestDir::new();
    let mut state = CoreState::new(dir.path().to_path_buf());
    state.logs = Arc::new(Logs::new(dir.path()));
    let hello = handle(
        &mut state,
        Request::new("1", "hello", serde_json::json!({"protocol": 1})),
    );
    assert!(hello.ok);

    // No supervisor attached in this config-only state: `tool.start` is an internal bug, not a
    // player-correctable refusal, so it belongs in `error.log`.
    let response = handle(
        &mut state,
        Request::new("2", "tool.start", serde_json::json!({"tool": "tuner"})),
    );
    let error = response.error.expect("the internal failure is reported");
    assert_eq!(error.code, "internal");

    let errors = read_lines(&dir.path_of("logs/error.log"));
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0].contains("tool.start")
            && errors[0].contains("the tool supervisor is not running"),
        "the internal line is in error.log: {}",
        errors[0]
    );
}

#[test]
fn a_failed_log_write_never_fails_the_run() {
    let dir = TestDir::new();
    fs::write(dir.path_of("blocked"), b"not a directory").expect("the blocker writes");
    let logs = Arc::new(Logs::new(dir.path_of("blocked")));
    let registry = ScriptedRegistry::scripted(
        ToolId::Tuner,
        vec![Decision::stop(StopReason::done("done anyway"))],
        &["a status that cannot be logged"],
    );
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::clone(&logs),
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Tuner, &Value::Null),
        StartReply::Started
    );
    let finished = wait_finished(&events);
    supervisor.shutdown("test");

    assert_eq!(finished.data["reason"], "done");
    assert_eq!(finished.data["message"], "done anyway");
    assert!(
        logs.write_failures() > 0,
        "the failed writes were swallowed and counted"
    );
}

#[test]
fn an_ordinary_refusal_does_not_write_error_log() {
    let dir = TestDir::new();
    let mut state = CoreState::new(dir.path().to_path_buf());
    state.logs = Arc::new(Logs::new(dir.path()));
    let hello = handle(
        &mut state,
        Request::new("1", "hello", serde_json::json!({"protocol": 1})),
    );
    assert!(hello.ok);

    let response = handle(
        &mut state,
        Request::new("2", "tool.start", serde_json::json!({"tool": "hover"})),
    );
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert!(
        !dir.exists("logs/error.log"),
        "player-correctable refusals must not bury the exception log"
    );
}

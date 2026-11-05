//! The per-run FILE logs (G8), increment 1: the logger's v2 shapes, OCR JSONL records, capture PNGs,
//! append-not-truncate across boots, and the "a failed write never fails the caller" rule.
//!
//! The real OCR model scan is not exercised here (the models are not on every machine — the existing
//! `ocr_captures.rs` is the model-gated pipeline test). The scan RECORDING path is the same helper
//! `ocr.scan` calls after the pipeline returns, so these tests drive it directly with a synthetic
//! `ModeResult`, which keeps them deterministic and hardware-free.

mod common;

use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use common::{Session as ProtocolSession, TestDir, config_root};
use seal_core::logs::Logs;
use seal_ocr::modes::{Mode, ModeResult};
use seal_tools::tool::ToolId;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use seal_board::transport::{Deadline, LineRead, Transport, TransportError};
use seal_core::dispatch::{CoreState, handle};
use seal_core::gem::record_empty_check;
use seal_core::holdspace::SpaceHoldEffects;
use seal_core::registry::{Session, ToolRegistry, Unavailable};
use seal_core::supervisor::{StartReply, StopReply, Supervisor};
use seal_domain::decision::{Decision, Key, StopReason};
use seal_domain::geometry::Point;
use seal_domain::tools::gem::EmptyRead;
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
    assert!(
        std::path::Path::new(capture_path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png")),
        "{capture_path}"
    );
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
    assert_eq!(read_lines(&dir.path_of("logs/ocr_log.jsonl")).len(), 1);
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
    assert_eq!(logs.write_failures(), 0);
}

/// CORE-F12: a status line is logged when its TEXT changes; a countdown that only moves its numbers
/// is the same line. Before this, the pet's "reload in 205 min" wrote one `<tool>.log` line per tick,
/// and every line reopened the file under the one global lock.
#[test]
fn a_countdown_status_writes_one_line_not_one_per_tick() {
    let dir = TestDir::new();
    let logs = Arc::new(Logs::new(dir.path()));
    let registry = ScriptedRegistry::scripted(
        ToolId::Pet,
        vec![
            Decision::wait(0.0),
            Decision::wait(0.0),
            Decision::wait(0.0),
            Decision::stop(StopReason::done("schedule ends")),
        ],
        &[
            "reload in 205 min",
            "reload in 204 min",
            "reload in 203 min",
        ],
    );
    let (bus, events) = seal_core::events::event_channel();
    let mut supervisor = Supervisor::start(
        Arc::clone(&registry) as Arc<dyn ToolRegistry>,
        bus,
        Arc::clone(&logs),
    );

    assert_eq!(
        supervisor.start_tool(ToolId::Pet, &Value::Null),
        StartReply::Started
    );
    let finished = wait_finished(&events);
    supervisor.shutdown("test");

    let messages: Vec<String> = tool_log(&dir, ToolId::Pet)
        .iter()
        .map(|line| message_of(line))
        .collect();
    assert_eq!(
        messages.join(" | "),
        "run started | reload in 205 min | finished — schedule ends",
        "only the countdown's first tick is a distinct line"
    );
    assert_eq!(finished.data["reason"], "done");
}

/// CORE-F12: a file that passes its cap is rotated instead of growing forever, and the retained
/// copies stay bounded. The cap and the number of copies are constructible so the test does not need
/// ten megabytes of lines.
#[test]
fn a_file_past_its_cap_rotates_and_keeps_a_bounded_number_of_copies() {
    let dir = TestDir::new();
    // Each line is exactly 32 bytes: a 19-character stamp, a space, "rotation NNN" and the newline.
    // A 100-byte cap therefore rotates after every fourth line.
    let logs = Logs::with_rotation(dir.path(), 100, 2);

    for index in 1..=7 {
        logs.tool_line(ToolId::Pet, &format!("rotation {index:03}"));
    }
    assert!(
        dir.exists("logs/pet.log.1"),
        "the first four lines moved to the rotated copy"
    );
    assert!(!dir.exists("logs/pet.log.2"), "one rotation is not yet two");
    let retained = format!("{}{}", dir.read("logs/pet.log.1"), dir.read("logs/pet.log"));
    for index in 1..=7 {
        assert!(
            retained.contains(&format!("rotation {index:03}")),
            "line {index} was not lost by the rotation: {retained}"
        );
    }

    for index in 8..=13 {
        logs.tool_line(ToolId::Pet, &format!("rotation {index:03}"));
    }
    assert!(
        dir.exists("logs/pet.log.2"),
        "the second rotation shifted the oldest copy up"
    );
    assert!(
        !dir.exists("logs/pet.log.3"),
        "KEEP_ROTATED=2 bounds the file at three copies"
    );
    let retained = format!(
        "{}{}{}",
        dir.read("logs/pet.log.2"),
        dir.read("logs/pet.log.1"),
        dir.read("logs/pet.log")
    );
    assert!(
        !retained.contains("rotation 001"),
        "the oldest lines are deleted, not kept forever: {retained}"
    );
    for index in 5..=13 {
        assert!(
            retained.contains(&format!("rotation {index:03}")),
            "line {index} is inside the retained window: {retained}"
        );
    }
    assert_eq!(logs.write_failures(), 0);
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

// ── G8's holdspace release-failure line ──────────────────────────────────────────────────────

/// Writes every line except the release; the release fails, which is the failure that happens in
/// `Drop` after a run has ended and therefore has no card left to reach.
struct FailingReleaseTransport {
    writes: Rc<RefCell<Vec<String>>>,
}

impl Transport for FailingReleaseTransport {
    fn write_line(&mut self, wire: &str) -> Result<(), TransportError> {
        if wire == "U\n" {
            return Err(TransportError::new("the port is gone"));
        }
        self.writes.borrow_mut().push(wire.to_owned());
        Ok(())
    }

    fn read_line(&mut self, _deadline: Deadline) -> LineRead {
        LineRead::Silent
    }
}

#[test]
fn a_failed_drop_release_is_written_to_holdspace_log_with_the_domain_line() {
    let dir = TestDir::new();
    let logs = Arc::new(Logs::new(dir.path()));
    let writes = Rc::new(RefCell::new(Vec::new()));
    {
        let mut effects = SpaceHoldEffects::new(
            FailingReleaseTransport {
                writes: Rc::clone(&writes),
            },
            Arc::clone(&logs),
        );
        effects.hold_space().expect("the hold writes P");
    }

    assert_eq!(*writes.borrow(), vec!["P\n"]);
    let lines = read_lines(&dir.path_of("logs/holdspace.log"));
    assert_eq!(lines.len(), 1, "one release-failure line");
    let line = &lines[0];
    assert!(
        line.contains("[svc] Could not release the spacebar"),
        "the domain release-failed wording: {line}"
    );
    assert!(
        line.contains("tap it once in game (the port is gone)"),
        "the transport error is carried: {line}"
    );
}

// ── G8's opt-in gem empty-check diagnostics ─────────────────────────────────────────────────

#[test]
fn gem_empty_check_saves_the_crop_and_writes_the_detail_to_gem_log() {
    let dir = TestDir::new();
    let logs = Logs::new(dir.path());

    let capture = record_empty_check(
        &logs,
        &EmptyRead::DiffFraction(0.12),
        None,
        0.01,
        0.18,
        Some(PNG_BYTES),
    )
    .expect("the capture is saved while the flag is on");

    assert!(
        capture.starts_with("logs/captures/empty_check_crop_"),
        "v2's crop name: {capture}"
    );
    assert!(
        std::path::Path::new(&capture)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png")),
        "{capture}"
    );
    assert_eq!(
        fs::read(dir.path_of(&capture)).expect("the crop exists"),
        PNG_BYTES,
        "the frame's own PNG bytes are the crop"
    );
    let lines = read_lines(&dir.path_of("logs/gem.log"));
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains(
            "empty check: diff=0.120 diff_max=0.010 signature_distance=0.180 empty=false \
             capture=logs/captures/empty_check_crop_"
        ),
        "both thresholds, named by metric, recorded in the tool log: {}",
        lines[0]
    );

    // The weaker read has no crop and says why; the line is still written.
    let no_capture = record_empty_check(&logs, &EmptyRead::NoReference, None, 0.01, 0.18, None);
    assert!(no_capture.is_none());
    let lines = read_lines(&dir.path_of("logs/gem.log"));
    assert_eq!(lines.len(), 2);
    assert!(
        lines[1].contains(
            "diff=n/a (no reference) diff_max=0.010 signature_distance=0.180 empty=false"
        ),
        "{}",
        lines[1]
    );
}

#[test]
fn cleanup_captures_deletes_only_direct_regular_pngs() {
    let root = config_root();
    let captures = root.path().join("logs/captures");
    fs::create_dir_all(&captures).unwrap();
    fs::write(captures.join("a.png"), PNG_BYTES).unwrap();
    fs::write(captures.join("b.PNG"), PNG_BYTES).unwrap();
    fs::write(captures.join("keep.txt"), b"keep").unwrap();
    fs::create_dir_all(captures.join("nested")).unwrap();
    fs::write(captures.join("nested/keep.png"), PNG_BYTES).unwrap();
    let locked = captures.join("locked.png");
    fs::write(&locked, PNG_BYTES).unwrap();
    // An open handle WITHOUT FILE_SHARE_DELETE makes remove_file fail with a sharing violation,
    // which is the real Windows "cannot delete" case. A read-only attribute does not stop deletion.
    #[cfg(windows)]
    let held = {
        use std::os::windows::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&locked)
            .unwrap()
    };

    let mut session = ProtocolSession::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));
    let result = session.result("logs.cleanup_captures", json!({}));

    assert_eq!(result["deleted"], 2, "a.png and b.PNG; the held one failed");
    assert_eq!(result["dir"], "logs/captures");
    assert!(!captures.join("a.png").exists());
    assert!(!captures.join("b.PNG").exists());
    assert!(captures.join("keep.txt").exists(), "non-png untouched");
    assert!(
        captures.join("nested/keep.png").exists(),
        "subdirectories are never recursed"
    );
    assert!(locked.exists(), "the failed file still exists");
    let error_log = fs::read_to_string(root.path().join("logs/error.log")).unwrap_or_default();
    assert!(
        error_log.contains("locked.png") || error_log.contains("could not delete"),
        "the failed delete is logged: {error_log}"
    );

    #[cfg(windows)]
    drop(held);
    // Let the test directory clean itself up.
    let _ = fs::remove_file(&locked);
}

#[test]
fn cleanup_captures_with_no_directory_is_zero_not_an_error() {
    let root = config_root();
    let mut session = ProtocolSession::start(root.path());
    session.result("hello", json!({"protocol": 1, "client": "0.1.0"}));
    let result = session.result("logs.cleanup_captures", json!({}));
    assert_eq!(result["deleted"], 0);
    assert_eq!(result["dir"], "logs/captures");
}

#[test]
fn read_evidence_writer_creates_logs_reads_files() {
    let root = config_root();
    let logs = Logs::new(root.path());
    let relative = logs
        .write_read("scan_page1_20261028_120000.png", b"fake png bytes")
        .expect("evidence is written");
    assert_eq!(relative, "logs/reads/scan_page1_20261028_120000.png");
    assert_eq!(
        fs::read(
            root.path()
                .join("logs/reads/scan_page1_20261028_120000.png")
        )
        .unwrap(),
        b"fake png bytes"
    );
}

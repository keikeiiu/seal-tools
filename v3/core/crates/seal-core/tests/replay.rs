//! `seal-core --replay <script>` end-to-end (`PROTOCOL.md` §13): the committed smoke script's real
//! response/event sequence, ordering, drift, boot refusals, EOF, ignored `--root` and `delay_ms`.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use common::TestDir;
use seal_proto::framing::{decode_line, encode_line};
use seal_proto::{Event, Request, Response};
use serde_json::{Value, json};

/// The committed example: the real widget/golden fixture the shell can drive.
fn smoke_script() -> PathBuf {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../protocol/replay/smoke.json");
    assert!(
        path.is_file(),
        "the example script exists: {}",
        path.display()
    );
    path
}

/// One live `--replay` core process.
struct Replay {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Replay {
    fn start(args: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_seal-core"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the core binary starts");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn start_with_script(script: &Path) -> Self {
        Self::start(&["--replay", script.to_str().expect("a UTF-8 script path")])
    }

    fn send(&mut self, id: &str, cmd: &str, args: Value) {
        let request = Request::new(id, cmd, args);
        let line = encode_line(&request).expect("the request encodes");
        self.write_raw(&line);
    }

    fn write_raw(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin is still open");
        stdin
            .write_all(line.as_bytes())
            .expect("the request writes");
        stdin.flush().expect("the request flushes");
    }

    fn read_line(&mut self) -> String {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("the core answers with a line");
        assert!(!line.is_empty(), "the core closed stdout unexpectedly");
        line
    }

    fn read_value(&mut self) -> Value {
        decode_line(&self.read_line()).expect("every stdout line is protocol JSON")
    }

    fn read_response(&mut self) -> Response {
        decode_line(&self.read_line()).expect("the line is a response")
    }

    fn read_event(&mut self) -> Event {
        decode_line(&self.read_line()).expect("the line is an event")
    }

    fn close_stdin(&mut self) {
        drop(self.stdin.take());
    }

    fn wait(mut self) -> (ExitStatus, String, String) {
        drop(self.stdin.take());
        let status = self.child.wait().expect("the core exits");
        let mut stdout = String::new();
        self.stdout
            .read_to_string(&mut stdout)
            .expect("the remaining stdout reads");
        let mut stderr = String::new();
        if let Some(mut pipe) = self.child.stderr.take() {
            pipe.read_to_string(&mut stderr).expect("stderr reads");
        }
        (status, stdout, stderr)
    }
}

#[derive(serde::Deserialize)]
struct HelloResult {
    protocol: u64,
    core_version: String,
    root: String,
    rev: u64,
    config: Value,
}

#[test]
fn the_smoke_script_serves_the_real_response_and_event_sequence() {
    let mut replay = Replay::start_with_script(&smoke_script());

    replay.send("1", "hello", json!({"protocol": 1, "client": "0.1.0"}));
    let hello = replay.read_value();
    assert!(
        hello.get("event").is_none() && hello["id"] == "1",
        "the handshake answer is a response, not an event: {hello}"
    );
    let response: Response = serde_json::from_value(hello).expect("the envelope is the real type");
    assert!(response.ok);
    let result: HelloResult = serde_json::from_value(
        response
            .result
            .expect("the handshake carries the §3 result"),
    )
    .expect("the handshake result is the real shape");
    assert_eq!(result.protocol, 1);
    assert_eq!(result.core_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(result.root, "replay");
    assert_eq!(result.rev, 0);
    assert!(result.config.is_object());

    replay.send("2", "config.reload", json!({}));
    let reload = replay.read_response();
    assert!(reload.ok);
    assert_eq!(reload.result.expect("reload result")["rev"], 1);

    replay.send("3", "tool.start", json!({"tool": "pet"}));
    let start = replay.read_value();
    assert!(
        start.get("event").is_none() && start["id"] == "3",
        "the start answer arrives BEFORE its events: {start}"
    );
    assert!(start["ok"].as_bool().unwrap_or(false));
    let state = replay.read_event();
    let finished = replay.read_event();
    assert_eq!(state.event, "state");
    assert_eq!(state.data["tool"], "pet");
    assert_eq!(state.data["phase"], "running");
    assert_eq!(finished.event, "tool.finished");
    assert_eq!(finished.data["reason"], "done");
    assert_eq!(finished.data["message"], "DONE — 4 row(s)");

    replay.send("4", "shutdown", json!({}));
    let shutdown = replay.read_response();
    assert!(shutdown.ok);
    let stopping = replay.read_event();
    assert_eq!(stopping.event, "core.stopping");
    assert_eq!(stopping.data["reason"], "shutdown");

    let (status, stdout, stderr) = replay.wait();
    assert!(status.success(), "shutdown exits cleanly: {status:?}");
    assert!(
        stdout.is_empty(),
        "nothing follows the stopping event: {stdout:?}"
    );
    assert!(stderr.is_empty(), "no stderr on a clean script: {stderr:?}");
}

#[test]
fn a_turns_events_are_on_the_wire_after_its_response() {
    let dir = TestDir::new();
    let script = dir.path_of("replay.json");
    std::fs::write(
        &script,
        serde_json::to_string(&json!({
            "protocol": 1,
            "turns": [{
                "expect": {"cmd": "config.reload"},
                "response": {"result": {"rev": 1, "config": {}}},
                "events": [
                    {"v": 1, "event": "log", "data": {"level": "info", "line": "first"}},
                    {"v": 1, "event": "log", "data": {"level": "info", "line": "second"}}
                ]
            }]
        }))
        .expect("the script serialises"),
    )
    .expect("the script writes");
    let mut replay = Replay::start_with_script(&script);
    replay.send("1", "config.reload", json!({}));

    let response = replay.read_value();
    assert!(
        response.get("event").is_none(),
        "response first: {response}"
    );
    let first = replay.read_value();
    let second = replay.read_value();
    assert_eq!(first["event"], "log");
    assert_eq!(first["data"]["line"], "first");
    assert_eq!(second["event"], "log");
    assert_eq!(second["data"]["line"], "second");

    replay.close_stdin();
    let stopping = replay.read_event();
    assert_eq!(stopping.data["reason"], "eof");
    let (status, _, stderr) = replay.wait();
    assert!(status.success(), "{status:?}");
    assert!(stderr.is_empty(), "{stderr}");
}

#[test]
fn a_drift_answers_internal_and_stops_fatally() {
    let dir = TestDir::new();
    let script = dir.path_of("replay.json");
    std::fs::write(
        &script,
        serde_json::to_string(&json!({
            "protocol": 1,
            "turns": [{"expect": {"cmd": "hello"}, "response": {"result": {}}}]
        }))
        .expect("the script serialises"),
    )
    .expect("the script writes");
    let mut replay = Replay::start_with_script(&script);

    replay.send("1", "config.reload", json!({}));
    let response = replay.read_response();
    assert!(!response.ok);
    let error = response.error.expect("the drift is an error");
    assert_eq!(error.code, "internal");
    assert!(
        error.message.contains("expected cmd \"hello\"") && error.message.contains("config.reload"),
        "the line names expected vs actual: {}",
        error.message
    );
    let stopping = replay.read_event();
    assert_eq!(stopping.event, "core.stopping");
    assert_eq!(stopping.data["reason"], "fatal");
    let (status, stdout, stderr) = replay.wait();
    assert!(!status.success(), "drift exits non-zero");
    assert_eq!(stdout, "", "the core already said everything on stdout");
    assert!(
        stderr.contains("replay drift") && stderr.contains("expected cmd"),
        "the reason reaches stderr: {stderr}"
    );
}

#[test]
fn more_requests_than_turns_is_drift_too() {
    let dir = TestDir::new();
    let script = dir.path_of("replay.json");
    std::fs::write(
        &script,
        serde_json::to_string(&json!({
            "protocol": 1,
            "turns": [{"expect": {"cmd": "hello"}, "response": {"result": {}}}]
        }))
        .expect("the script serialises"),
    )
    .expect("the script writes");
    let mut replay = Replay::start_with_script(&script);

    replay.send("1", "hello", json!({"protocol": 1}));
    assert!(replay.read_response().ok, "the first turn is served");
    replay.send("2", "config.reload", json!({}));
    let response = replay.read_response();
    assert!(!response.ok);
    assert!(
        response
            .error
            .expect("an error")
            .message
            .contains("no turn left"),
        "the exhausted script is a drift"
    );
    assert_eq!(replay.read_event().data["reason"], "fatal");
    let (status, _, stderr) = replay.wait();
    assert!(!status.success());
    assert!(stderr.contains("no turn left"), "{stderr}");
}

#[test]
fn a_malformed_script_is_a_boot_refusal_with_empty_stdout() {
    let dir = TestDir::new();
    let script = dir.path_of("bad.json");
    std::fs::write(&script, r#"{"protocol":1,"turns":[{"nope":true}]}"#)
        .expect("the script writes");
    let replay = Replay::start_with_script(&script);
    let (status, stdout, stderr) = replay.wait();
    assert!(!status.success(), "a malformed script exits non-zero");
    assert_eq!(stdout, "", "nothing on stdout before the load succeeds");
    assert!(
        stderr.contains("unknown key") && stderr.contains("turn 1"),
        "the reason names the bad turn: {stderr}"
    );
}

#[test]
fn eof_ends_a_consumed_script_cleanly() {
    let dir = TestDir::new();
    let script = dir.path_of("replay.json");
    std::fs::write(
        &script,
        serde_json::to_string(&json!({
            "protocol": 1,
            "turns": [{"expect": {"cmd": "config.reload"}, "response": {"result": {"rev": 9}}}]
        }))
        .expect("the script serialises"),
    )
    .expect("the script writes");
    let mut replay = Replay::start_with_script(&script);
    replay.send("1", "config.reload", json!({}));
    assert_eq!(
        replay.read_response().result.expect("the scripted result")["rev"],
        9
    );
    replay.close_stdin();
    let stopping = replay.read_event();
    assert_eq!(stopping.event, "core.stopping");
    assert_eq!(stopping.data["reason"], "eof");
    let (status, stdout, stderr) = replay.wait();
    assert!(status.success());
    assert_eq!(stdout, "");
    assert_eq!(stderr, "");
}

#[test]
fn a_bogus_root_is_ignored_in_replay_mode() {
    let bogus = PathBuf::from(r"C:\definitely-not-a-core-root-52");
    assert!(!bogus.exists(), "the bogus root must not exist");
    let smoke = smoke_script();
    let mut replay = Replay::start(&[
        "--replay",
        smoke.to_str().expect("a UTF-8 script path"),
        "--root",
        bogus.to_str().expect("a UTF-8 bogus root"),
    ]);
    replay.send("1", "hello", json!({"protocol": 1, "client": "0.1.0"}));
    let response = replay.read_response();
    assert!(
        response.ok,
        "the root is never touched: {:?}",
        response.error
    );
    replay.close_stdin();
    assert_eq!(replay.read_event().data["reason"], "eof");
    assert!(replay.wait().0.success());
}

#[test]
fn a_scripts_delay_ms_is_honoured() {
    let dir = TestDir::new();
    let script = dir.path_of("replay.json");
    std::fs::write(
        &script,
        serde_json::to_string(&json!({
            "protocol": 1,
            "turns": [{
                "expect": {"cmd": "config.reload"},
                "response": {"result": {"rev": 1}},
                "delay_ms": 40
            }]
        }))
        .expect("the script serialises"),
    )
    .expect("the script writes");
    let mut replay = Replay::start_with_script(&script);
    let started = Instant::now();
    replay.send("1", "config.reload", json!({}));
    assert!(replay.read_response().ok);
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(30),
        "the scripted delay was honoured (took {elapsed:?})"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "the delay is the script's, not a hang ({elapsed:?})"
    );
    replay.close_stdin();
    assert_eq!(replay.read_event().data["reason"], "eof");
    assert!(replay.wait().0.success());
}

#[test]
fn replay_without_its_script_value_is_a_boot_refusal() {
    let replay = Replay::start(&["--replay"]);
    let (status, stdout, stderr) = replay.wait();
    assert!(!status.success());
    assert_eq!(stdout, "");
    assert!(stderr.contains("--replay needs a script path"), "{stderr}");
}

#[test]
fn an_error_body_and_an_empty_body_use_the_typed_envelope() {
    let dir = TestDir::new();
    let script = dir.path_of("replay.json");
    std::fs::write(
        &script,
        serde_json::to_string(&json!({
            "protocol": 1,
            "turns": [
                {
                    "expect": {"cmd": "hello"},
                    "response": {
                        "error": {
                            "code": "bad_version",
                            "message": "protocol 0 is not 1",
                            "detail": {"expected": 1}
                        }
                    }
                },
                {"expect": {"cmd": "config.reload"}}
            ]
        }))
        .expect("the script serialises"),
    )
    .expect("the script writes");
    let mut replay = Replay::start_with_script(&script);

    replay.send("1", "hello", json!({"protocol": 0}));
    let response = replay.read_response();
    assert!(!response.ok);
    let error = response.error.expect("the scripted error");
    assert_eq!(error.code, "bad_version");
    assert_eq!(error.message, "protocol 0 is not 1");
    assert_eq!(error.detail["expected"], 1);

    replay.send("2", "config.reload", json!({}));
    let response = replay.read_response();
    assert!(response.ok);
    assert_eq!(
        response.result.expect("the empty body answers result {}"),
        json!({})
    );

    replay.close_stdin();
    assert_eq!(replay.read_event().data["reason"], "eof");
    assert!(replay.wait().0.success());
}

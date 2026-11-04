//! The walking skeleton's first milestone: spawn the core binary, speak the handshake over its
//! stdio, and get a protocol-shaped answer back — the same transport the Flutter shell will use.
//!
//! The binary now loads the config at startup, so the fixture is a real (minimal) install root: a
//! core that cannot read its config cannot run a tool, and it refuses to start loudly.

mod common;

use common::{TestDir, config_root};
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn the_binary_answers_hello_over_stdio_and_exits_on_eof() {
    let root = config_root();

    let mut child = Command::new(env!("CARGO_BIN_EXE_seal-core"))
        .arg("--root")
        .arg(root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the core binary starts");

    {
        let stdin = child.stdin.as_mut().expect("stdin is piped");
        stdin
            .write_all(b"{\"v\":1,\"id\":\"1\",\"cmd\":\"hello\",\"args\":{\"protocol\":1}}\n")
            .unwrap();
        stdin.flush().unwrap();
    }
    // Closing stdin is the documented shutdown: the core answers, then exits on EOF.
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("the core exits");

    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    let line = stdout.lines().next().expect("one response line");
    let response: Value = serde_json::from_str(line).expect("the response is JSON");
    assert_eq!(response["v"], 1);
    assert_eq!(response["id"], "1");
    assert_eq!(response["ok"], true);
    assert_eq!(response["result"]["protocol"], 1);
    assert!(
        output.status.success(),
        "EOF must exit cleanly, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn the_binary_refuses_to_start_when_the_config_cannot_be_loaded() {
    // An empty directory: `--root` is accepted (it exists), but there is no config to read. A core
    // that started anyway would run every tool against an empty config, silently — v2 fails loudly
    // here too, and `PROTOCOL.md` §1's "core crash" is how the shell learns about it.
    let root = TestDir::new();

    let output = Command::new(env!("CARGO_BIN_EXE_seal-core"))
        .arg("--root")
        .arg(root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("the core binary runs");

    assert!(!output.status.success(), "the core must not start");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot load config"),
        "stderr names the failure: {stderr}"
    );
}

#[test]
fn malformed_requests_are_answered_and_do_not_kill_the_core() {
    let root = config_root();
    let mut child = Command::new(env!("CARGO_BIN_EXE_seal-core"))
        .arg("--root")
        .arg(root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the core binary starts");
    {
        let stdin = child.stdin.as_mut().expect("stdin is piped");
        // CORE-F9: invalid UTF-8 used to end the loop as EOF, and a bad envelope used to answer
        // with an empty id. The core must refuse each line, keep serving, and echo the id it can
        // recover (the numeric id is rendered as a string so the shell can match its call).
        stdin.write_all(b"\xff\n").unwrap();
        stdin
            .write_all(br#"{"id":7,"cmd":"hello","args":{"protocol":1}}"#)
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(br#"{"v":2,"id":"old","cmd":"hello","args":{"protocol":1}}"#)
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin
            .write_all(br#"{"v":1,"id":"ok","cmd":"hello","args":{"protocol":1}}"#)
            .unwrap();
        stdin.write_all(b"\n").unwrap();
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("the core exits");
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let responses: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("every line is JSON"))
        .filter(|value| value.get("event").is_none())
        .collect();
    assert_eq!(responses.len(), 4, "one response per malformed/valid line");
    assert_eq!(responses[0]["error"]["code"], "bad_request");
    assert_eq!(responses[1]["error"]["code"], "bad_request");
    assert_eq!(responses[1]["id"], "7");
    assert_eq!(responses[2]["error"]["code"], "bad_version");
    assert_eq!(responses[2]["id"], "old");
    assert_eq!(responses[3]["ok"], true);
    assert_eq!(responses[3]["id"], "ok");
}

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

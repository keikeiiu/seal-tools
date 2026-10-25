//! The walking skeleton's first milestone: spawn the core binary, speak the handshake over its
//! stdio, and get a protocol-shaped answer back — the same transport the Flutter shell will use.

mod common;

use common::{marker, TestDir};
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn the_binary_answers_hello_over_stdio_and_exits_on_eof() {
    let root = TestDir::new();
    marker(root.path(), "defaults.yaml");

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

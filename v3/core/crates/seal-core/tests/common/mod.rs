//! Shared test scaffolding: a temporary directory that cleans itself up, a root marker writer, and
//! a protocol session that speaks to the real binary over its stdio.
//!
//! Std only on purpose: a temp-dir crate pulls `getrandom` → `windows-sys`, whose build script
//! wants GNU `dlltool`, which the pinned toolchain does not ship (see `rust-toolchain.toml`).
//!
//! The scratch base is Cargo's `CARGO_TARGET_TMPDIR`, which lives inside `target/`. The system
//! temp directory is not usable here: writes from a test process are denied by the harness's
//! sandbox, while `target/` is inside the workspace.

#![allow(dead_code)] // each integration-test binary compiles this module and may use a subset

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use seal_proto::framing::{decode_line, encode_line};
use seal_proto::{Request, Response};
use serde_json::{Value, json};

/// The portable fixture the config command tests start from: enough sections to see a merge and a
/// hand edit, and nothing machine-specific.
pub const BASE_CONFIG: &str = "\
window:
  title: TEST_TITLE
hotkeys:
  start: 123
quest:
  active: base
";

/// The machine-specific fixture beside it — the overlay whose values must show up in `hello`.
pub const LOCAL_CONFIG: &str = "\
tuner:
  guard_px: 8
";

/// A unique directory under Cargo's target scratch area, removed on drop.
pub struct TestDir(PathBuf);

impl TestDir {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("seal-core-test-{id}"));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn path_of(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub fn exists(&self, name: &str) -> bool {
        self.path_of(name).is_file()
    }

    /// Writes a file, creating the parent directories a name may carry.
    pub fn write(&self, name: &str, text: &str) {
        let path = self.path_of(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    pub fn append(&self, name: &str, text: &str) {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path_of(name))
            .unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    pub fn read(&self, name: &str) -> String {
        fs::read_to_string(self.path_of(name)).unwrap()
    }

    pub fn read_bytes(&self, name: &str) -> Vec<u8> {
        fs::read(self.path_of(name)).unwrap()
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A root with a valid `config/`: the shipped defaults and the machine overlay.
pub fn config_root() -> TestDir {
    let root = TestDir::new();
    root.write("config/defaults.yaml", BASE_CONFIG);
    root.write("config/local.yaml", LOCAL_CONFIG);
    root
}

/// Writes `config/<name>`, which is what marks a directory as the application root.
pub fn marker(dir: &Path, name: &str) {
    let path = dir.join("config").join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "").unwrap();
}

/// One live core process, spoken to with the protocol the Flutter shell will use.
///
/// Dropping the session closes stdin, which `PROTOCOL.md` §1 defines as "stop safely": the core
/// answers what it has and exits on EOF. `shutdown` can be sent explicitly as well.
pub struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Session {
    /// Spawns the real binary with `--root`, exactly as the shell will (minus the discovery walk).
    pub fn start(root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_seal-core"))
            .arg("--root")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the core binary starts");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        }
    }

    /// Sends one request and reads its one response line.
    pub fn call(&mut self, cmd: &str, args: Value) -> Response {
        self.next_id += 1;
        let request = Request::new(&self.next_id.to_string(), cmd, args);
        let line = encode_line(&request).expect("a request encodes");
        let stdin = self.stdin.as_mut().expect("stdin is still open");
        stdin
            .write_all(line.as_bytes())
            .expect("the request writes");
        stdin.flush().expect("the request flushes");

        let mut response_line = String::new();
        self.stdout
            .read_line(&mut response_line)
            .expect("the core answers with a line");
        assert!(
            !response_line.is_empty(),
            "the core closed stdout without answering {cmd:?}"
        );
        decode_line::<Response>(&response_line).expect("the response is protocol-shaped")
    }

    /// The handshake, in the shape `PROTOCOL.md` §1 specifies.
    pub fn hello(&mut self) -> Response {
        self.call("hello", json!({"protocol": 1, "client": "0.1.0"}))
    }

    /// The `result` of a call that must have succeeded.
    pub fn result(&mut self, cmd: &str, args: Value) -> Value {
        let response = self.call(cmd, args);
        assert!(
            response.ok,
            "{cmd} must succeed, error: {:?}",
            response.error
        );
        response
            .result
            .expect("a successful response carries a result")
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // EOF is the documented shutdown; waiting here reaps the child and proves the core exits
        // without a failing status.
        drop(self.stdin.take());
        let status = self.child.wait().expect("the core exits");
        assert!(status.success(), "EOF must exit cleanly, got {status:?}");
    }
}

//! The request dispatcher: one place where a [`Request`] becomes a [`Response`].
//!
//! It is deliberately separate from the stdio loop so the handshake and the command surface can
//! be tested without spawning a process (`PROTOCOL.md` §1 and §4).

use seal_config::{ConfigError, ConfigStore};
use seal_proto::{Error, PROTOCOL_VERSION, Request, Response};
use serde_json::{Value, json};
use std::path::PathBuf;

/// The state one connection's dispatcher works on: the root it bound to, the config store that owns
/// the files, the merged config as the shell sees it, and the session revision.
///
/// `rev` is a SESSION revision, not an on-disk counter: it starts at 0 for one core process and is
/// incremented by every successful write or reload. The shell holds the value from `hello` and sends
/// it back with `config.apply`, so a stale copy is refused rather than silently overwriting a block
/// the core wrote in the meantime (`PROTOCOL.md` §7, `PLAN-V3-CORE.md` §4.1).
pub struct CoreState {
    pub root: PathBuf,
    pub store: ConfigStore,
    pub rev: u64,
    pub config: Value,
    pub handshaken: bool,
    pub stopping: bool,
}

impl CoreState {
    /// An empty state, for a dispatcher test that needs no files on disk. The binary uses
    /// [`Self::load`] instead, which is the only constructor that reads the config.
    pub fn new(root: PathBuf) -> Self {
        let store = ConfigStore::new(root.join("config"));
        Self {
            root,
            store,
            rev: 0,
            config: json!({}),
            handshaken: false,
            stopping: false,
        }
    }

    /// The binary's startup: bind to `root` and read the merged config through the store.
    ///
    /// Failing here is fatal rather than a degraded start — v2 fails loudly for the same reason: a
    /// core with no config cannot run a tool, and a silent empty config would look exactly like a
    /// game that changed.
    pub fn load(root: PathBuf) -> Result<Self, ConfigError> {
        let store = ConfigStore::new(root.join("config"));
        let config = store.load()?;
        Ok(Self {
            root,
            store,
            rev: 0,
            config,
            handshaken: false,
            stopping: false,
        })
    }
}

/// Answers one request. Never panics: a bad request is a refusal the client can show.
pub fn handle(state: &mut CoreState, request: Request) -> Response {
    if request.cmd == "hello" {
        return hello(state, request);
    }

    if !state.handshaken {
        return Response::err(
            &request.id,
            Error::new("not_ready", "hello must be the first request", json!({})),
        );
    }

    match request.cmd.as_str() {
        "shutdown" => {
            state.stopping = true;
            Response::ok(&request.id, json!({}))
        }
        other => Response::err(
            &request.id,
            Error::new(
                "bad_request",
                &format!("unknown command {other:?}"),
                json!({}),
            ),
        ),
    }
}

fn hello(state: &mut CoreState, request: Request) -> Response {
    let protocol = request
        .args
        .get("protocol")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    if protocol != u64::from(PROTOCOL_VERSION) {
        return Response::err(
            &request.id,
            Error::new(
                "bad_version",
                &format!("protocol {protocol} is not {PROTOCOL_VERSION}"),
                json!({"expected": PROTOCOL_VERSION}),
            ),
        );
    }

    state.handshaken = true;
    Response::ok(
        &request.id,
        json!({
            "protocol": PROTOCOL_VERSION,
            "core_version": env!("CARGO_PKG_VERSION"),
            "root": state.root.to_string_lossy(),
            "rev": state.rev,
            "config": state.config,
        }),
    )
}

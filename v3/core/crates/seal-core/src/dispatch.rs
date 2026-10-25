//! The request dispatcher: one place where a [`Request`] becomes a [`Response`].
//!
//! It is deliberately separate from the stdio loop so the handshake and the command surface can
//! be tested without spawning a process (`PROTOCOL.md` §1 and §4).

use seal_proto::{Error, PROTOCOL_VERSION, Request, Response};
use serde_json::{Value, json};
use std::path::PathBuf;

/// The state one connection's dispatcher works on. Later increments add the config revision and
/// the tool supervisor; the skeleton only needs the root and the handshake flag.
pub struct CoreState {
    pub root: PathBuf,
    pub rev: u64,
    pub config: Value,
    pub handshaken: bool,
    pub stopping: bool,
}

impl CoreState {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            rev: 0,
            config: json!({}),
            handshaken: false,
            stopping: false,
        }
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

//! The wire types shared by the v3 core and the Flutter shell.
//!
//! The contract is `v3/docs/spec/PROTOCOL.md`; this crate is the Rust side of it. The Dart side
//! mirrors these types, and the golden fixtures under `v3/protocol/fixtures/` are what keeps the
//! two halves from drifting (`PROTOCOL.md` §10).

pub mod framing;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The protocol version carried by every message (`PROTOCOL.md` §2).
pub const PROTOCOL_VERSION: u32 = 1;

/// A request from the shell to the core.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub v: u32,
    pub id: String,
    pub cmd: String,
    #[serde(default)]
    pub args: Value,
}

impl Request {
    pub fn new(id: &str, cmd: &str, args: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.to_owned(),
            cmd: cmd.to_owned(),
            args,
        }
    }
}

/// The core's answer to one [`Request`]. Exactly one of `result` and `error` is present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub v: u32,
    pub id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

impl Response {
    pub fn ok(id: &str, result: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.to_owned(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: &str, error: Error) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.to_owned(),
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}

/// A failure the shell can show, with the machine-readable code and the human message apart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Error {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub detail: Value,
}

impl Error {
    pub fn new(code: &str, message: &str, detail: Value) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
            detail,
        }
    }
}

/// An unsolicited message from the core. Events carry no `id` and are not answered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub v: u32,
    pub event: String,
    pub data: Value,
}

impl Event {
    pub fn new(event: &str, data: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            event: event.to_owned(),
            data,
        }
    }
}

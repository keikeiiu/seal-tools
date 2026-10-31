//! One JSON object per line, in both directions (`PROTOCOL.md` §1).
//!
//! The core reads the shell's requests and writes responses and events; both halves of that use
//! the same framing. The functions are infallible for well-typed messages and return the JSON
//! error rather than panicking, because a bad line from a client is a message to reject, not a
//! reason to drop the connection.

use serde::{Serialize, de::DeserializeOwned};

/// Encodes one message as one newline-terminated line.
///
/// Compact JSON never contains a raw newline, so the result is exactly one line; the caller writes
/// it as-is.
///
/// # Errors
///
/// Returns [`serde_json::Error`] when `message` contains a value serde cannot encode as JSON (for
/// example a map with non-string keys).
pub fn encode_line<M: Serialize>(message: &M) -> Result<String, serde_json::Error> {
    let mut line = serde_json::to_string(message)?;
    line.push('\n');
    Ok(line)
}

/// Decodes one line, tolerating whatever line ending the platform's `lines()` left on it.
///
/// # Errors
///
/// Returns [`serde_json::Error`] when `line` (minus its trailing CR/LF) is not one JSON value of
/// the shape `M` expects.
pub fn decode_line<M: DeserializeOwned>(line: &str) -> Result<M, serde_json::Error> {
    let end = line.trim_end_matches(['\r', '\n']);
    serde_json::from_str(end)
}

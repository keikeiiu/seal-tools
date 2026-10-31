//! `--replay <script>`: serve the stdio protocol from a script, with NO machine behind it.
//!
//! `PROTOCOL.md` §13 is the contract. Replay is a development mode of the SAME binary and the SAME
//! protocol: no config file is loaded, no board opened, no window measured, no frame captured and no
//! OCR model run. One turn answers one request line, in order; an `expect` drift answers that
//! request `internal`, emits a fatal `core.stopping`, writes the reason to stderr and exits non-zero;
//! EOF ends the session like a normal one (`core.stopping {"reason":"eof"}`, exit 0).
//!
//! This module deliberately imports only `std` and `seal-proto`: it cannot reach the config store,
//! the board, `seal-win` or `seal-ocr`, and `main` calls it BEFORE root discovery/config load, so
//! the machine-touching startup is unreachable in this mode by construction rather than by a flag
//! check deep inside it.

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::Duration;

use seal_proto::framing::{decode_line, encode_line};
use seal_proto::{Error, Event, PROTOCOL_VERSION, Request, Response};
use serde_json::{Map, Value, json};

/// The largest `delay_ms` a script may ask for (§13: `0..=10_000`).
const MAX_DELAY_MS: u64 = 10_000;

/// One parsed replay script. Every validation happens in [`Script::load`], before the first request,
/// so a typo cannot half-run.
struct Script {
    turns: Vec<Turn>,
}

struct Turn {
    expect: Option<Expect>,
    response: Body,
    events: Vec<Event>,
    delay: Duration,
}

struct Expect {
    cmd: Option<String>,
    args: Map<String, Value>,
}

enum Body {
    Result(Value),
    Error(Error),
}

impl Script {
    fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|error| format!("cannot read replay script {}: {error}", path.display()))?;
        let value: Value = serde_json::from_str(&text).map_err(|error| {
            format!(
                "replay script {} is not valid JSON: {error}",
                path.display()
            )
        })?;
        let object = value
            .as_object()
            .ok_or_else(|| "the replay script must be a JSON object".to_owned())?;
        for key in object.keys() {
            if !matches!(key.as_str(), "protocol" | "turns") {
                return Err(format!("replay script has unknown top-level key {key:?}"));
            }
        }
        let protocol = object
            .get("protocol")
            .and_then(Value::as_u64)
            .ok_or_else(|| "the replay script needs \"protocol\": 1".to_owned())?;
        if protocol != 1 {
            return Err(format!(
                "replay script protocol {protocol} is not {PROTOCOL_VERSION}"
            ));
        }
        let turns = object
            .get("turns")
            .and_then(Value::as_array)
            .ok_or_else(|| "the replay script needs a \"turns\" array".to_owned())?;
        let turns = turns
            .iter()
            .enumerate()
            .map(|(index, turn)| parse_turn(index, turn))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { turns })
    }
}

fn malformed(index: usize, message: impl Into<String>) -> String {
    format!("turn {}: {}", index + 1, message.into())
}

fn parse_turn(index: usize, value: &Value) -> Result<Turn, String> {
    let object = value
        .as_object()
        .ok_or_else(|| malformed(index, "must be a JSON object"))?;
    for key in object.keys() {
        if !matches!(key.as_str(), "expect" | "response" | "events" | "delay_ms") {
            return Err(malformed(index, format!("unknown key {key:?}")));
        }
    }

    let expect = match object.get("expect") {
        None => None,
        Some(Value::Null) => return Err(malformed(index, "\"expect\" must be an object")),
        Some(value) => {
            let expect_object = value
                .as_object()
                .ok_or_else(|| malformed(index, "\"expect\" must be an object"))?;
            let mut cmd = None;
            let mut args = Map::new();
            for (key, value) in expect_object {
                if key == "cmd" {
                    let Some(name) = value.as_str() else {
                        return Err(malformed(index, "\"expect.cmd\" must be a string"));
                    };
                    cmd = Some(name.to_owned());
                } else {
                    args.insert(key.clone(), value.clone());
                }
            }
            Some(Expect { cmd, args })
        }
    };

    let response = match object.get("response") {
        None | Some(Value::Null) => Body::Result(json!({})),
        Some(value) => parse_body(index, value)?,
    };

    let events = match object.get("events") {
        None | Some(Value::Null) => Vec::new(),
        Some(value) => {
            let array = value
                .as_array()
                .ok_or_else(|| malformed(index, "\"events\" must be an array"))?;
            array
                .iter()
                .enumerate()
                .map(|(event_index, event)| {
                    serde_json::from_value::<Event>(event.clone()).map_err(|error| {
                        malformed(
                            index,
                            format!("event{} is not a protocol event: {error}", event_index + 1),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        }
    };

    let delay_ms = match object.get("delay_ms") {
        None | Some(Value::Null) => 0,
        Some(value) => value
            .as_u64()
            .filter(|milliseconds| *milliseconds <= MAX_DELAY_MS)
            .ok_or_else(|| {
                malformed(
                    index,
                    format!("\"delay_ms\" must be an integer in 0..={MAX_DELAY_MS}"),
                )
            })?,
    };

    Ok(Turn {
        expect,
        response,
        events,
        delay: Duration::from_millis(delay_ms),
    })
}

fn parse_body(index: usize, value: &Value) -> Result<Body, String> {
    let object = value
        .as_object()
        .ok_or_else(|| malformed(index, "\"response\" must be an object"))?;
    for key in object.keys() {
        if !matches!(key.as_str(), "result" | "error") {
            return Err(malformed(
                index,
                format!("\"response\" has unknown key {key:?}"),
            ));
        }
    }
    match (object.get("result"), object.get("error")) {
        (Some(_), Some(_)) => Err(malformed(
            index,
            "\"response\" cannot carry both result and error",
        )),
        (Some(result), None) => Ok(Body::Result(result.clone())),
        (None, Some(error)) => parse_error(index, error).map(Body::Error),
        (None, None) => Ok(Body::Result(json!({}))),
    }
}

fn parse_error(index: usize, value: &Value) -> Result<Error, String> {
    let object = value
        .as_object()
        .ok_or_else(|| malformed(index, "\"response.error\" must be an object"))?;
    for key in object.keys() {
        if !matches!(key.as_str(), "code" | "message" | "detail") {
            return Err(malformed(
                index,
                format!("\"response.error\" has unknown key {key:?}"),
            ));
        }
    }
    let code = object
        .get("code")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(index, "\"response.error\" needs a string \"code\""))?;
    let message = object
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(index, "\"response.error\" needs a string \"message\""))?;
    let detail = object.get("detail").cloned().unwrap_or(Value::Null);
    Ok(Error::new(code, message, detail))
}

impl Expect {
    /// `cmd` when stated, and every other key compared shallowly against the request's `args`.
    /// The error is the sentence the drift report uses: it names expected vs actual.
    fn matches(&self, request: &Request) -> Result<(), String> {
        if let Some(expected) = &self.cmd
            && request.cmd != *expected
        {
            return Err(format!("expected cmd {expected:?}, got {:?}", request.cmd));
        }
        for (key, expected) in &self.args {
            let actual = request.args.get(key);
            if actual != Some(expected) {
                let actual_text = actual.map_or_else(|| "absent".to_owned(), Value::to_string);
                return Err(format!("expected args.{key} {expected}, got {actual_text}"));
            }
        }
        Ok(())
    }
}

/// Runs the replay loop on stdin/stdout. Load/validation errors are boot refusals (nothing has been
/// written); a drift writes its internal answer + fatal event before returning the reason.
///
/// # Errors
///
/// Returns the reason a scripted replay could not load, or the reason the live protocol drifted from the recording.
pub fn run(path: &Path) -> Result<(), String> {
    let script = Script::load(path)?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut turns = script.turns.iter().enumerate();

    loop {
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) | Err(_) => {
                write_event(
                    &mut stdout,
                    &Event::new("core.stopping", json!({"reason": "eof"})),
                )?;
                return Ok(());
            }
            Ok(_) => {}
        }
        if line.trim().is_empty() {
            continue;
        }
        let request: Request = match decode_line(&line) {
            Ok(request) => request,
            Err(error) => {
                let reason = format!("replay drift: the request line is not a request: {error}");
                write_event(
                    &mut stdout,
                    &Event::new("core.stopping", json!({"reason": "fatal"})),
                )?;
                return Err(reason);
            }
        };
        let Some((index, turn)) = turns.next() else {
            let reason = format!(
                "replay drift: no turn left for request {:?} (the script has {} turn(s))",
                request.cmd,
                script.turns.len()
            );
            drift(&mut stdout, &request, &reason)?;
            return Err(reason);
        };
        if let Some(expect) = &turn.expect
            && let Err(detail) = expect.matches(&request)
        {
            let reason = format!("replay drift: turn {} {detail}", index + 1);
            drift(&mut stdout, &request, &reason)?;
            return Err(reason);
        }
        if !turn.delay.is_zero() {
            std::thread::sleep(turn.delay);
        }
        let response = match &turn.response {
            Body::Result(result) => Response::ok(&request.id, result.clone()),
            Body::Error(error) => Response::err(&request.id, error.clone()),
        };
        write_message(&mut stdout, &response)?;
        for event in &turn.events {
            write_event(&mut stdout, event)?;
        }
        stdout.flush().map_err(|error| error.to_string())?;

        // A scripted `shutdown` is the session's last turn, exactly like a normal session: its events
        // say why the core is leaving, and the process exits cleanly. Any other request waits for the
        // next line (or EOF).
        if request.cmd == "shutdown" {
            return Ok(());
        }
    }
}

fn drift(stdout: &mut io::Stdout, request: &Request, reason: &str) -> Result<(), String> {
    write_message(
        stdout,
        &Response::err(
            &request.id,
            Error::new("internal", reason, json!({"replay_drift": true})),
        ),
    )?;
    write_event(
        stdout,
        &Event::new("core.stopping", json!({"reason": "fatal"})),
    )
}

fn write_message<T: serde::Serialize>(stdout: &mut io::Stdout, message: &T) -> Result<(), String> {
    let line = encode_line(message).map_err(|error| error.to_string())?;
    stdout
        .write_all(line.as_bytes())
        .map_err(|error| error.to_string())?;
    stdout.flush().map_err(|error| error.to_string())
}

fn write_event(stdout: &mut io::Stdout, event: &Event) -> Result<(), String> {
    write_message(stdout, event)
}

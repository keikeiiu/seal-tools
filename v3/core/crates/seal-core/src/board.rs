//! `board.probe` and `board.test`: the merged config → the board crate's adapter → the protocol's
//! JSON. No Win32 and no rule lives here; the port decision is `seal-board`'s, the level is the
//! firmware's answer, and the commands are the grammar's.

use seal_board::clock::RealClock;
use seal_board::command::BoardCommand;
use seal_board::enumerate;
use seal_board::firmware;
use seal_board::serial::SerialTransport;
use seal_board::transport::Transport;
use serde_json::{Value, json};

/// The `arduino:` block of the merged config, with v2's shipped defaults when a key is absent
/// (`v2/config/defaults.yaml:43-49`). The config is validated before this runs, so a present value
/// is a number; the defaults here keep a hand-built test config from panicking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub port: Option<String>,
    pub vid: u16,
    pub pid: Vec<u16>,
    pub baud: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: None,
            vid: 9025,               // 0x2341, v2's default
            pid: vec![32822, 32823], // 0x8036 / 0x8047
            baud: 115200,
        }
    }
}

impl Settings {
    pub fn from_config(config: &Value) -> Self {
        let defaults = Self::default();
        let Some(arduino) = config.get("arduino") else {
            return defaults;
        };
        let port = arduino
            .get("port")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|port| !port.is_empty())
            .map(str::to_owned);
        let vid = arduino
            .get("vid")
            .and_then(Value::as_i64)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(defaults.vid);
        let pid = arduino
            .get("pid")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_i64)
                    .filter_map(|value| u16::try_from(value).ok())
                    .collect::<Vec<_>>()
            })
            .filter(|pids| !pids.is_empty())
            .unwrap_or(defaults.pid);
        let baud = arduino
            .get("baud")
            .and_then(Value::as_i64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|baud| *baud > 0)
            .unwrap_or(defaults.baud);
        Self {
            port,
            vid,
            pid,
            baud,
        }
    }
}

/// Why a board command could not answer. `Unavailable` is the protocol's `board_unavailable`: the
/// shell shows its line ("no board found", "the named port is not here", "cannot open COM5").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardFailure {
    Unavailable(String),
    BadRequest(String),
}

/// Find and open the board, or the line that says why not. The choice is `seal-board`'s: a NAMED
/// port that is absent is a refusal rather than a fall-through, and discovery is VID/PID first with
/// v2's name fallback.
fn open_board(settings: &Settings) -> Result<(String, SerialTransport), BoardFailure> {
    let ports = enumerate::available_ports();
    let (chosen, refusal) = enumerate::choose_port(
        settings.port.as_deref(),
        &ports,
        settings.vid,
        &settings.pid,
    );
    let Some(port) = chosen else {
        return Err(BoardFailure::Unavailable(refusal.unwrap_or_else(|| {
            format!(
                "no board found — no serial port reports VID {:04x}, and none names Arduino",
                settings.vid
            )
        })));
    };
    let transport = SerialTransport::open(&port, settings.baud, &RealClock)
        .map_err(|error| BoardFailure::Unavailable(error.to_string()))?;
    Ok((port, transport))
}

fn port_facts(port: &str) -> (Option<u16>, Option<u16>) {
    let ports = enumerate::available_ports();
    ports
        .iter()
        .find(|candidate| candidate.name.eq_ignore_ascii_case(port))
        .map_or((None, None), |info| (info.vid, info.pid))
}

/// `board.probe`: which port would be opened, at what baud, with which IDs, and what the firmware
/// answers for `V`. Silence is level 1 (`PROTOCOL.md` §4: "absent = level 1") — a board that predates
/// version reporting, which cannot drag.
pub fn probe(config: &Value) -> Result<Value, BoardFailure> {
    let settings = Settings::from_config(config);
    let (port, mut transport) = open_board(&settings)?;
    let detected = firmware::detect(&mut transport);
    let level = detected.reported().unwrap_or(1);
    let (vid, pid) = port_facts(&port);
    Ok(json!({
        "port": port,
        "baud": settings.baud,
        "vid": vid.map(|vid| format!("{vid:04x}")),
        "pid": pid.map(|pid| format!("{pid:04x}")),
        "level": level,
    }))
}

/// The command a `board.test` request names. The protocol's example is `{"kind":"key","key":"E"}`,
/// and the grammar's `E` IS Enter — printable characters travel as `K` — so "E"/"enter" maps to the
/// Enter command, `F1`–`F12` to the function table, and any other single printable character to `K`.
fn command(args: &Value) -> Result<BoardCommand, BoardFailure> {
    let bad = |message: &str| BoardFailure::BadRequest(message.to_owned());
    let Some(kind) = args.get("kind").and_then(Value::as_str) else {
        return Err(bad("board.test needs a \"kind\": click, move or key"));
    };
    match kind {
        "click" => Ok(BoardCommand::LeftClick),
        "move" => {
            let coordinate = |name: &str| -> Result<i32, BoardFailure> {
                args.get(name)
                    .and_then(Value::as_i64)
                    .and_then(|value| i32::try_from(value).ok())
                    .ok_or_else(|| {
                        BoardFailure::BadRequest(format!(
                            "board.test move needs an integer {name:?}"
                        ))
                    })
            };
            Ok(BoardCommand::Move {
                dx: coordinate("dx")?,
                dy: coordinate("dy")?,
            })
        }
        "key" => {
            let Some(key) = args.get("key").and_then(Value::as_str) else {
                return Err(bad("board.test key needs a \"key\" string"));
            };
            if key.eq_ignore_ascii_case("enter") || key == "E" {
                return Ok(BoardCommand::Enter);
            }
            if let Some(number) = key
                .strip_prefix('F')
                .and_then(|digits| digits.parse::<u8>().ok())
                && (1..=12).contains(&number)
            {
                return Ok(BoardCommand::FunctionKey {
                    number,
                    fast: false,
                });
            }
            let mut characters = key.chars();
            match (characters.next(), characters.next()) {
                (Some(character), None) if character.is_ascii_graphic() || character == ' ' => {
                    Ok(BoardCommand::Key {
                        character,
                        fast: false,
                    })
                }
                _ => Err(bad(
                    "board.test key takes \"enter\", \"F1\"-\"F12\", or one printable character",
                )),
            }
        }
        other => Err(BoardFailure::BadRequest(format!(
            "unknown board.test kind {other:?}: click, move or key"
        ))),
    }
}

/// `board.test`: send the named command and answer `{"ok":true,"detail":<the line v2 logs>}`.
pub fn test(config: &Value, args: &Value) -> Result<Value, BoardFailure> {
    let command = command(args)?;
    let settings = Settings::from_config(config);
    let (port, mut transport) = open_board(&settings)?;
    let wire = command.encode();
    transport
        .write_line(&wire)
        .map_err(|error| BoardFailure::Unavailable(error.to_string()))?;
    Ok(json!({
        "ok": true,
        "detail": format!("sent {} to {port}", wire.trim()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_to_v2s_shipped_arduino_block() {
        let settings = Settings::from_config(&json!({}));
        assert_eq!(settings.vid, 9025);
        assert_eq!(settings.pid, vec![32822, 32823]);
        assert_eq!(settings.baud, 115200);
        assert_eq!(settings.port, None);
    }

    #[test]
    fn settings_read_the_merged_block_and_ignore_a_blank_port() {
        let settings = Settings::from_config(&json!({
            "arduino": {"vid": 4660, "pid": [1, 2], "baud": 9600, "port": "   "}
        }));
        assert_eq!(settings.vid, 4660);
        assert_eq!(settings.pid, vec![1, 2]);
        assert_eq!(settings.baud, 9600);
        assert_eq!(settings.port, None, "a blank port is no override");
    }

    #[test]
    fn the_test_command_mapping_follows_the_grammar() {
        assert_eq!(
            command(&json!({"kind": "click"})),
            Ok(BoardCommand::LeftClick)
        );
        assert_eq!(
            command(&json!({"kind": "move", "dx": 100, "dy": -5})),
            Ok(BoardCommand::Move { dx: 100, dy: -5 })
        );
        assert_eq!(
            command(&json!({"kind": "key", "key": "E"})),
            Ok(BoardCommand::Enter)
        );
        assert_eq!(
            command(&json!({"kind": "key", "key": "F12"})),
            Ok(BoardCommand::FunctionKey {
                number: 12,
                fast: false
            })
        );
        assert_eq!(
            command(&json!({"kind": "key", "key": "q"})),
            Ok(BoardCommand::Key {
                character: 'q',
                fast: false
            })
        );
        assert!(matches!(
            command(&json!({"kind": "key", "key": "F13"})),
            Err(BoardFailure::BadRequest(_))
        ));
        assert!(matches!(
            command(&json!({"kind": "move", "dx": 1})),
            Err(BoardFailure::BadRequest(_))
        ));
        assert!(matches!(
            command(&json!({"kind": "spin"})),
            Err(BoardFailure::BadRequest(_))
        ));
    }
}

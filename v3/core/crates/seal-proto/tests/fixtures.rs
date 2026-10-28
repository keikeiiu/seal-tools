//! The golden protocol fixtures: the cross-language contract `PROTOCOL.md` §10 promises.
//!
//! One JSON file per message shape lives in `v3/protocol/fixtures/`. Every file is
//! `{ "case", "kind", "message" }`, and `message` is **serialised from the real `seal-proto` types**
//! — so a change to a type's wire shape fails this test until the fixtures (and with them the
//! Flutter shell's decode side) are revisited. A normal run is compare-only; it never writes.
//!
//! Regenerate after an intentional protocol change, from `v3/core`:
//!
//! ```powershell
//! $env:UPDATE_FIXTURES = "1"
//! .\cargo.cmd test -p seal-proto --offline
//! Remove-Item Env:UPDATE_FIXTURES
//! ```
//!
//! Then review the fixture diff with the protocol change.

use seal_proto::{Error, Event, PROTOCOL_VERSION, Request, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The environment variable that turns the compare test into a generator.
const UPDATE_VARIABLE: &str = "UPDATE_FIXTURES";

/// Every command `PROTOCOL.md` §4 defines. Listed here rather than derived from the files, so the
/// suite fails when the protocol grows a command nobody made a fixture for. A command with more than
/// one documented argument shape has one fixture per shape (`board.test`, `ocr.scan`, `tool.stop`).
const REQUEST_COMMANDS: &[&str] = &[
    "hello",
    "config.validate",
    "config.apply",
    "config.reload",
    "window.find",
    "window.measure",
    "capture",
    "calibration.save",
    "calibration.fetch",
    "keys.parse",
    "keys.names",
    "attributes.get",
    "board.probe",
    "board.details",
    "pet.preflight",
    "pet.set_boarding",
    "board.test",
    "ui.window",
    "tool.start",
    "tool.stop",
    "ocr.scan",
    "shutdown",
];

/// The error codes `PROTOCOL.md` §6 defines. Same reason as [`REQUEST_COMMANDS`].
const ERROR_CODES: &[&str] = &[
    "bad_request",
    "bad_version",
    "not_ready",
    "conflict",
    "window_not_found",
    "game_not_foreground",
    "board_unavailable",
    "ocr_unavailable",
    "tool_busy",
    "not_running",
    "io",
    "config_invalid",
    "internal",
];

/// The events `PROTOCOL.md` §5 defines.
const EVENTS: &[&str] = &[
    "state",
    "tool.finished",
    "config.changed",
    "log",
    "core.stopping",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Request,
    Response,
    Event,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Response => "response",
            Self::Event => "event",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "request" => Some(Self::Request),
            "response" => Some(Self::Response),
            "event" => Some(Self::Event),
            _ => None,
        }
    }
}

/// One fixture in memory: the file's `case` name, its kind, and the wire message.
struct Case {
    name: &'static str,
    kind: Kind,
    message: Value,
}

/// The on-disk shape of a fixture file. `message` is the real type's JSON, so the file is the type's
/// wire form plus the two fields a decoder needs to know what it is looking at.
#[derive(Debug, Serialize, Deserialize)]
struct FixtureFile {
    case: String,
    kind: String,
    message: Value,
}

fn request_case(name: &'static str, id: &str, cmd: &str, args: Value) -> Case {
    Case {
        name,
        kind: Kind::Request,
        message: serde_json::to_value(Request::new(id, cmd, args)).expect("a request serialises"),
    }
}

fn ok_case(name: &'static str, id: &str, result: Value) -> Case {
    Case {
        name,
        kind: Kind::Response,
        message: serde_json::to_value(Response::ok(id, result)).expect("a response serialises"),
    }
}

fn error_case(name: &'static str, id: &str, code: &str, message: &str, detail: Value) -> Case {
    Case {
        name,
        kind: Kind::Response,
        message: serde_json::to_value(Response::err(id, Error::new(code, message, detail)))
            .expect("a response serialises"),
    }
}

fn event_case(name: &'static str, event: &str, data: Value) -> Case {
    Case {
        name,
        kind: Kind::Event,
        message: serde_json::to_value(Event::new(event, data)).expect("an event serialises"),
    }
}

/// Every fixture, in a stable order. The payloads are synthetic; nothing here comes from a player's
/// `local.yaml`. The shapes are `PROTOCOL.md` §§3–6's.
fn cases() -> Vec<Case> {
    vec![
        // ── Requests: every command in §4, one per documented argument shape ────────────────────
        request_case(
            "request.hello",
            "1",
            "hello",
            json!({"protocol": 1, "client": "0.1.0"}),
        ),
        request_case(
            "request.config.validate",
            "2",
            "config.validate",
            json!({"sections": {"quest": {"active": "run1"}}}),
        ),
        request_case(
            "request.config.apply",
            "3",
            "config.apply",
            json!({"rev": 7, "sections": {"quest": {"active": "run1"}}}),
        ),
        request_case(
            "request.config.apply.settings",
            "32",
            "config.apply",
            json!({"rev": 7, "sections": {"settings": {"hotkeys": {"start": 66}}}}),
        ),
        request_case(
            "request.config.apply.mixed",
            "33",
            "config.apply",
            json!({"rev": 7, "sections": {"tuner": {"guard_px": 9}, "settings": {"hotkeys": {"start": 66}}}}),
        ),
        request_case(
            "request.config.apply.variant",
            "34",
            "config.apply",
            json!({"rev": 7, "sections": {"settings": {"game": {"variant": "us"}}}}),
        ),
        request_case("request.config.reload", "4", "config.reload", json!({})),
        request_case("request.window.find", "5", "window.find", json!({})),
        request_case("request.window.measure", "6", "window.measure", json!({})),
        request_case(
            "request.capture",
            "7",
            "capture",
            json!({"region": {"x": 12, "y": 47, "w": 300, "h": 200}}),
        ),
        request_case(
            "request.calibration.save",
            "8",
            "calibration.save",
            json!({
                "frame_id": "f1",
                "region": {"x": 12, "y": 47, "w": 300, "h": 200},
                "kind": "bag_grid",
            }),
        ),
        request_case(
            "request.calibration.fetch",
            "9",
            "calibration.fetch",
            json!({"kind": "bag_grid"}),
        ),
        request_case(
            "request.calibration.save.targeted",
            "40",
            "calibration.save",
            json!({"kind": "gem_grade", "point": {"x": 120, "y": 240}, "target": "N"}),
        ),
        request_case(
            "request.calibration.save.empty",
            "41",
            "calibration.save",
            json!({
                "frame_id": "f1",
                "kind": "result_gem_area",
                "region": {"x": 610, "y": 800, "w": 220, "h": 60},
                "empty": true,
            }),
        ),
        request_case(
            "request.calibration.fetch.targeted",
            "42",
            "calibration.fetch",
            json!({"kind": "pet_toggle", "target": "2"}),
        ),
        request_case(
            "request.keys.parse",
            "37",
            "keys.parse",
            json!({"names": {"start": "F12", "quit": "F11", "advance_grade": "G", "pause": "Zulu"}}),
        ),
        request_case(
            "request.keys.names",
            "38",
            "keys.names",
            json!({"codes": {"start": 123, "quit": 122, "advance_grade": 71, "pause": 20}}),
        ),
        request_case("request.attributes.get", "39", "attributes.get", json!({})),
        request_case("request.board.probe", "10", "board.probe", json!({})),
        request_case("request.board.details", "43", "board.details", json!({})),
        request_case("request.pet.preflight", "44", "pet.preflight", json!({})),
        request_case(
            "request.pet.set_boarding",
            "45",
            "pet.set_boarding",
            json!({"row": 1, "running": true}),
        ),
        request_case(
            "request.board.test.click",
            "11",
            "board.test",
            json!({"kind": "click"}),
        ),
        request_case(
            "request.board.test.move",
            "12",
            "board.test",
            json!({"kind": "move", "dx": 100, "dy": 0}),
        ),
        request_case(
            "request.board.test.key",
            "13",
            "board.test",
            json!({"kind": "key", "key": "E"}),
        ),
        request_case(
            "request.ui.window",
            "14",
            "ui.window",
            json!({"left": 100, "top": 100, "width": 900, "height": 700, "pinned": false}),
        ),
        request_case(
            "request.tool.start",
            "15",
            "tool.start",
            json!({"tool": "spammer"}),
        ),
        request_case(
            "request.tool.stop.tool",
            "16",
            "tool.stop",
            json!({"tool": "spammer"}),
        ),
        request_case("request.tool.stop.any", "17", "tool.stop", json!({})),
        request_case(
            "request.ocr.scan.attributes",
            "18",
            "ocr.scan",
            json!({"frame_id": "f1", "mode": "attributes"}),
        ),
        request_case(
            "request.ocr.scan.count",
            "19",
            "ocr.scan",
            json!({"region": {"x": 12, "y": 47, "w": 300, "h": 200}, "mode": "count"}),
        ),
        request_case("request.shutdown", "20", "shutdown", json!({})),
        // ── Success responses: one per distinct result shape the spec prints ────────────────────
        ok_case(
            "response.ok.hello",
            "1",
            json!({
                "protocol": 1,
                "core_version": "0.1.0",
                "root": "C:\\SealTools",
                "rev": 7,
                "config": {
                    "window": {"title": "TW_LIVE"},
                    "game": {"variant": "tw"},
                    "tuner": {"target_grade": "DG"}
                }
            }),
        ),
        ok_case(
            "response.ok.config_apply",
            "3",
            json!({"rev": 8, "sections": {"quest": {"active": "run1"}}}),
        ),
        ok_case(
            "response.ok.config_apply.settings",
            "32",
            json!({"rev": 8, "sections": {"settings": {"hotkeys": {"start": 66}}}}),
        ),
        ok_case(
            "response.ok.config_apply.mixed",
            "33",
            json!({"rev": 8, "sections": {"tuner": {"guard_px": 9}, "settings": {"hotkeys": {"start": 66}}}}),
        ),
        ok_case(
            "response.ok.config_apply.variant",
            "34",
            json!({"rev": 8, "sections": {"settings": {"game": {"variant": "us", "variants": [{"name": "tw", "attributes": "attributes.tw.yaml"}, {"name": "us", "attributes": "attributes.us.yaml"}]}}}}),
        ),
        ok_case(
            "response.ok.config_reload",
            "4",
            json!({"rev": 9, "config": {"window": {"title": "TW_LIVE"}}}),
        ),
        ok_case(
            "response.ok.capture",
            "7",
            json!({
                "frame": {
                    "format": "png",
                    "width": 2865,
                    "height": 1789,
                    "origin": {"x": 12, "y": 47},
                    "data": "iVBORw0KGgo=",
                    "frame_id": "f1"
                },
                "display": {
                    "scale": 1.5,
                    "monitor_dpi": 144,
                    "screen": {"w": 3840, "h": 2160},
                    "frame": {"x": 0, "y": 0, "w": 2865, "h": 1789},
                    "physical_client": {"x": 12, "y": 47, "w": 2865, "h": 1789},
                    "logical_client": {"x": 8, "y": 31, "w": 1910, "h": 1193}
                }
            }),
        ),
        ok_case(
            "response.ok.ocr_scan",
            "18",
            json!({
                "lines": ["攻擊力 +12"],
                "attributes": [["攻擊力", "atk"]],
                "grade": "G",
                "remaining": null,
                "capture_path": "logs/captures/read_0001.png"
            }),
        ),
        ok_case("response.ok.empty", "20", json!({})),
        ok_case(
            "response.ok.pet_preflight",
            "44",
            json!({
                "ready": false,
                "calibration": [{"ok": true, "label": "Pet menu point", "detail": "marked"}],
                "session": [{"ok": true, "label": "food cells", "detail": "4 marked, 1 used (one round loads 2)"}],
                "lines": ["no pet icon queued — the tool will use the return slot instead of looking for the pet by icon"]
            }),
        ),
        ok_case(
            "response.ok.pet_set_boarding",
            "45",
            json!({
                "rev": 1,
                "block": {
                    "return_slot": [0, 5],
                    "slots": [{"boarding_running": true}]
                }
            }),
        ),
        ok_case(
            "response.ok.board_details",
            "43",
            json!({
                "baud": 115200,
                "expected": {"vid": "2341", "pids": ["8036", "8037"], "port": null},
                "ports": [{
                    "port": "COM5",
                    "vid": "2341",
                    "pid": "8036",
                    "product": "Arduino Leonardo",
                    "manufacturer": "Arduino LLC",
                    "matches_ids": true,
                    "chosen": true
                }],
                "choice": {"state": "ready", "port": "COM5", "refusal": null},
                "firmware": {"level": 2, "checked": true}
            }),
        ),
        ok_case(
            "response.ok.calibration_save",
            "8",
            json!({
                "rev": 10,
                "kind": "bag_grid",
                "mark": [12, 47, 300, 200],
                "derived": {"pitch": {"w": 37.5, "h": 25.0}},
                "artifact": null,
            }),
        ),
        ok_case(
            "response.ok.calibration_save.targeted",
            "40",
            json!({
                "rev": 11,
                "kind": "gem_grade",
                "mark": [120, 240],
                "derived": {},
                "artifact": null,
                "target": "N",
            }),
        ),
        ok_case(
            "response.ok.calibration_save.empty",
            "41",
            json!({
                "rev": 12,
                "kind": "result_gem_area",
                "mark": [610, 800, 220, 60],
                "derived": {},
                "artifact": null,
            }),
        ),
        ok_case(
            "response.ok.calibration_fetch",
            "9",
            json!({
                "kind": "bag_grid",
                "mark": [12, 47, 300, 200],
                "artifact": null,
                "rev": 10,
            }),
        ),
        ok_case(
            "response.ok.calibration_fetch.targeted",
            "42",
            json!({
                "kind": "pet_toggle",
                "mark": [100, 200, 40, 40],
                "artifact": null,
                "rev": 13,
                "target": "2",
            }),
        ),
        ok_case(
            "response.ok.keys_parse",
            "37",
            json!({
                "codes": {"start": 123, "quit": 122, "advance_grade": 71},
                "errors": [{
                    "field": "pause",
                    "input": "Zulu",
                    "line": "\"Zulu\" is not a key. Use F1–24, Esc, CapsLock, Space, Tab, Enter, or a single letter/digit.",
                }],
            }),
        ),
        ok_case(
            "response.ok.keys_names",
            "38",
            json!({"names": {"start": "F12", "quit": "F11", "advance_grade": "G", "pause": "P"}}),
        ),
        ok_case(
            "response.ok.attributes_get",
            "39",
            json!({
                "variant": "tw",
                "dictionary": "attributes.yaml",
                "attributes": {
                    "attributes": [],
                    "per_level_stats": [],
                    "negative_attr_markers": [],
                    "text_fixes": {
                        "whole": [],
                        "substring": [],
                        "simplified_traditional": [],
                        "final": [],
                        "regex": [],
                    },
                },
            }),
        ),
        // ── Error responses: every code in §6 ───────────────────────────────────────────────────
        error_case(
            "response.error.bad_request",
            "21",
            "bad_request",
            "malformed envelope, unknown cmd, bad args",
            json!({}),
        ),
        error_case(
            "response.error.bad_version",
            "22",
            "bad_version",
            "protocol 2 is not 1",
            json!({"expected": 1}),
        ),
        error_case(
            "response.error.not_ready",
            "23",
            "not_ready",
            "hello must be the first request",
            json!({}),
        ),
        error_case(
            "response.error.conflict",
            "24",
            "conflict",
            "config changed while you were editing",
            json!({"rev": 12, "sections": {"quest": {"active": "run1"}}}),
        ),
        error_case(
            "response.error.window_not_found",
            "25",
            "window_not_found",
            "the game window was not found",
            json!({"title": "Seal Online"}),
        ),
        error_case(
            "response.error.game_not_foreground",
            "26",
            "game_not_foreground",
            "capture refused because another window is in front",
            json!({}),
        ),
        error_case(
            "response.error.board_unavailable",
            "27",
            "board_unavailable",
            "no board on the configured port",
            json!({"port": "COM5"}),
        ),
        error_case(
            "response.error.tool_busy",
            "28",
            "tool_busy",
            "another tool is running",
            json!({"tool": "pet"}),
        ),
        error_case(
            "response.error.not_running",
            "29",
            "not_running",
            "nothing is running",
            json!({}),
        ),
        error_case(
            "response.error.io",
            "30",
            "io",
            "cannot write config/local.yaml: access denied",
            json!({"path": "config/local.yaml"}),
        ),
        error_case(
            "response.error.config_invalid",
            "35",
            "config_invalid",
            "the config on disk is invalid",
            json!({"errors": ["Window: The Title field is required.", "Check defaults.yaml and local.yaml."]}),
        ),
        error_case(
            "response.error.ocr_unavailable",
            "36",
            "ocr_unavailable",
            "the OCR models are not available",
            json!({"reason": "models_env_unset"}),
        ),
        error_case(
            "response.error.internal",
            "31",
            "internal",
            "internal error",
            json!({}),
        ),
        // ── Events: every one in §5 ─────────────────────────────────────────────────────────────
        event_case(
            "event.state",
            "state",
            json!({
                "tool": "pet",
                "running": true,
                "phase": "running",
                "message": "row 2: looking",
                "countdown_s": null,
                "rows": [
                    {
                        "name": "row 1",
                        "status": "boarding",
                        "detail": "藍天 · 3/min",
                        "next_action_in_seconds": 1800.0
                    },
                    {
                        "name": "row 2",
                        "status": "waiting for a reload",
                        "detail": null,
                        "next_action_in_seconds": null
                    }
                ],
                "error": null
            }),
        ),
        // The row list is empty before the first row exists (the opening snapshot) and for tools
        // with no rows at all: [] means "no rows to show", never "unknown" — the shell must render
        // an empty card rather than guess.
        event_case(
            "event.state.empty",
            "state",
            json!({
                "tool": "tuner",
                "running": true,
                "phase": "starting",
                "message": "",
                "countdown_s": null,
                "rows": [],
                "error": null
            }),
        ),
        event_case(
            "event.tool.finished",
            "tool.finished",
            json!({"tool": "pet", "reason": "done", "message": "DONE — 4 row(s)"}),
        ),
        event_case(
            "event.config.changed",
            "config.changed",
            json!({"rev": 11, "block": "pet", "values": {"boarding_window_may_be_open": true}}),
        ),
        event_case(
            "event.log",
            "log",
            json!({"level": "warn", "line": "ocr scan retried"}),
        ),
        event_case(
            "event.core.stopping",
            "core.stopping",
            json!({"reason": "eof"}),
        ),
    ]
}

#[test]
fn every_fixture_matches_the_types() {
    let cases = cases();
    assert_cases_are_unique(&cases);

    if updating() {
        write_fixtures(&cases);
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    for case in &cases {
        let path = fixture_path(case.name);
        let expected = render(case);
        match fs::read_to_string(&path) {
            Ok(actual) if actual == expected => {}
            Ok(actual) => failures.push(format!(
                "{}:\n{}",
                path.display(),
                first_difference(&expected, &actual)
            )),
            Err(error) => failures.push(format!(
                "{}: {error}\n  run the UPDATE_FIXTURES=1 command in fixtures.rs to create it",
                path.display()
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "the committed protocol fixtures do not match the real seal-proto types.\n\
         If the wire change is intended, regenerate with `$env:UPDATE_FIXTURES = \"1\"; \
         .\\cargo.cmd test -p seal-proto --offline`, review the diff, and update PROTOCOL.md \
         and the Dart side in the same change.\n\n{}",
        failures.join("\n\n")
    );
}

/// The files must decode back into the types they claim to be. This is the direction the Flutter
/// shell will run: read a fixture, decode it, fail if a field is missing or has the wrong type.
#[test]
fn every_fixture_decodes_into_its_type() {
    if updating() {
        return;
    }

    for case in cases() {
        let path = fixture_path(case.name);
        let text = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "{}: {error}\n  run the UPDATE_FIXTURES=1 command in fixtures.rs first",
                path.display()
            )
        });
        let file: FixtureFile = serde_json::from_str(&text)
            .unwrap_or_else(|error| panic!("{} is not a fixture file: {error}", path.display()));

        assert_eq!(file.case, case.name, "{}", path.display());
        assert_eq!(file.kind, case.kind.as_str(), "{}", path.display());
        let kind = Kind::from_name(&file.kind)
            .unwrap_or_else(|| panic!("{} has an unknown kind {:?}", path.display(), file.kind));

        match kind {
            Kind::Request => {
                let request: Request = serde_json::from_value(file.message.clone())
                    .unwrap_or_else(|error| panic!("{} is not a request: {error}", path.display()));
                assert_eq!(request.v, PROTOCOL_VERSION, "{}", path.display());
                assert!(!request.id.is_empty(), "{}", path.display());
                assert!(!request.cmd.is_empty(), "{}", path.display());
                // The case name is `request.<cmd>` or `request.<cmd>.<shape>`: this keeps a fixture
                // named for one command from silently carrying another command's args.
                let exact = format!("request.{}", request.cmd);
                let shaped = format!("{exact}.");
                assert!(
                    case.name == exact || case.name.starts_with(&shaped),
                    "{} is named for {:?} but carries cmd {:?}",
                    path.display(),
                    case.name,
                    request.cmd
                );
            }
            Kind::Response => {
                let response: Response = serde_json::from_value(file.message.clone())
                    .unwrap_or_else(|error| {
                        panic!("{} is not a response: {error}", path.display())
                    });
                assert_eq!(response.v, PROTOCOL_VERSION, "{}", path.display());
                assert!(!response.id.is_empty(), "{}", path.display());
                assert!(
                    response.result.is_some() ^ response.error.is_some(),
                    "{}: exactly one of result and error is present",
                    path.display()
                );
                match response.error.as_ref() {
                    Some(error) => {
                        assert!(!response.ok, "{}", path.display());
                        assert!(!error.code.is_empty(), "{}", path.display());
                        assert!(!error.message.is_empty(), "{}", path.display());
                        let expected_code = format!("response.error.{}", error.code);
                        assert_eq!(case.name, expected_code, "{}", path.display());
                    }
                    None => {
                        assert!(response.ok, "{}", path.display());
                        assert!(
                            case.name.starts_with("response.ok."),
                            "{}: a success fixture is named response.ok.<shape>",
                            path.display()
                        );
                    }
                }
            }
            Kind::Event => {
                let event: Event = serde_json::from_value(file.message.clone())
                    .unwrap_or_else(|error| panic!("{} is not an event: {error}", path.display()));
                assert_eq!(event.v, PROTOCOL_VERSION, "{}", path.display());
                // The case name is `event.<name>` or `event.<name>.<shape>`: one documented event
                // can have several pinned shapes (`state` with rows and empty), and a shaped name
                // must still carry its own event.
                let exact = format!("event.{}", event.event);
                let shaped = format!("{exact}.");
                assert!(
                    case.name == exact || case.name.starts_with(&shaped),
                    "{} is named for {:?} but carries event {:?}",
                    path.display(),
                    case.name,
                    event.event
                );
                assert!(
                    event.data.is_object(),
                    "{}: every §5 event carries an object data",
                    path.display()
                );
            }
        }
    }
}

/// The fixture set must keep up with the protocol: a new command, event or error code without a
/// fixture fails here rather than shipping an untested shape to the shell.
#[test]
fn the_fixture_set_covers_the_protocol_surface() {
    let cases = cases();
    let names: Vec<&str> = cases.iter().map(|case| case.name).collect();

    for command in REQUEST_COMMANDS {
        let exact = format!("request.{command}");
        let shaped = format!("{exact}.");
        assert!(
            names
                .iter()
                .any(|name| **name == exact || name.starts_with(&shaped)),
            "no request fixture for the §4 command {command:?}"
        );
    }

    for code in ERROR_CODES {
        assert!(
            names.contains(&format!("response.error.{code}").as_str()),
            "no error fixture for the §6 code {code:?}"
        );
    }

    for event in EVENTS {
        assert!(
            names.contains(&format!("event.{event}").as_str()),
            "no fixture for the §5 event {event:?}"
        );
    }

    assert!(
        cases.iter().any(|case| case.kind == Kind::Request),
        "the set has requests"
    );
    assert!(
        cases.iter().any(|case| case.kind == Kind::Response),
        "the set has responses"
    );
    assert!(
        cases.iter().any(|case| case.kind == Kind::Event),
        "the set has events"
    );
}

/// Fixtures are compared as bytes, so their line endings are part of the contract: LF, exactly one
/// trailing newline. (`v3/protocol/fixtures/.gitattributes` forces that on checkout so a Windows
/// worktree with `core.autocrlf=true` cannot rewrite them.)
#[test]
fn fixtures_are_lf_json_with_a_trailing_newline() {
    if updating() {
        return;
    }

    for case in cases() {
        let path = fixture_path(case.name);
        let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(
            !bytes.contains(&b'\r'),
            "{} must use LF line endings; run git add --renormalize or check .gitattributes",
            path.display()
        );
        assert_eq!(
            bytes.last(),
            Some(&b'\n'),
            "{} must end with exactly one newline",
            path.display()
        );
        let text = String::from_utf8(bytes).expect("fixtures are UTF-8");
        serde_json::from_str::<FixtureFile>(&text)
            .unwrap_or_else(|error| panic!("{} is not a fixture file: {error}", path.display()));
    }
}

/// A file left behind by a renamed or deleted case would look like part of the contract and never be
/// checked. Compare the directory against [`cases`].
#[test]
fn no_stray_fixture_files() {
    if updating() {
        return;
    }

    let expected: BTreeSet<String> = cases()
        .iter()
        .map(|case| format!("{}.json", case.name))
        .collect();
    let mut found: BTreeSet<String> = BTreeSet::new();
    let directory = fixtures_dir();
    let entries = fs::read_dir(&directory).unwrap_or_else(|error| {
        panic!(
            "{}: {error}\n  run the UPDATE_FIXTURES=1 command in fixtures.rs first",
            directory.display()
        )
    });
    for entry in entries {
        let path = entry.expect("the fixtures directory lists").path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("json")
            && let Some(name) = path.file_name().and_then(|name| name.to_str())
        {
            found.insert(name.to_owned());
        }
    }

    let stray: Vec<&String> = found.difference(&expected).collect();
    let missing: Vec<&String> = expected.difference(&found).collect();
    assert!(
        stray.is_empty() && missing.is_empty(),
        "the fixtures directory does not match the case list.\n  stray files: {stray:?}\n  \
         missing files: {missing:?}\n  regenerate with the UPDATE_FIXTURES=1 command in fixtures.rs"
    );
}

/// True when the compare test should generate instead. A regeneration run only writes; the other
/// tests step aside so a half-written directory cannot be read concurrently.
fn updating() -> bool {
    std::env::var_os(UPDATE_VARIABLE).is_some_and(|value| !value.is_empty())
}

fn assert_cases_are_unique(cases: &[Case]) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for case in cases {
        assert!(
            seen.insert(case.name),
            "the case name {:?} appears twice",
            case.name
        );
    }
}

/// Writes every fixture and removes `.json` files no case names — regeneration is exact, so a
/// renamed case cannot leave its old file behind to rot.
fn write_fixtures(cases: &[Case]) {
    let directory = fixtures_dir();
    fs::create_dir_all(&directory).expect("the fixtures directory is creatable");

    let expected: BTreeSet<String> = cases
        .iter()
        .map(|case| format!("{}.json", case.name))
        .collect();
    for case in cases {
        fs::write(fixture_path(case.name), render(case)).expect("a fixture is writable");
    }
    for entry in fs::read_dir(&directory).expect("the fixtures directory lists") {
        let path = entry.expect("the fixtures directory lists").path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned();
            if !expected.contains(&name) {
                fs::remove_file(&path).expect("a stale fixture is removable");
            }
        }
    }
}

fn render(case: &Case) -> String {
    let file = FixtureFile {
        case: case.name.to_owned(),
        kind: case.kind.as_str().to_owned(),
        message: case.message.clone(),
    };
    let mut text = serde_json::to_string_pretty(&file).expect("a fixture serialises");
    text.push('\n');
    text
}

fn fixture_path(name: &str) -> PathBuf {
    fixtures_dir().join(format!("{name}.json"))
}

fn fixtures_dir() -> PathBuf {
    for directory in Path::new(env!("CARGO_MANIFEST_DIR")).ancestors() {
        if directory.join("v3/docs/spec/PROTOCOL.md").is_file() {
            return directory.join("v3/protocol/fixtures");
        }
    }
    panic!(
        "could not find v3/docs/spec/PROTOCOL.md above {}",
        env!("CARGO_MANIFEST_DIR")
    );
}

/// The first line the committed fixture and the types disagree on, with a pointer to regeneration.
fn first_difference(expected: &str, actual: &str) -> String {
    if expected.lines().eq(actual.lines()) {
        return "the content matches line by line but the bytes differ — fixtures must use LF line \
                endings and exactly one trailing newline"
            .to_owned();
    }

    for (index, (want, got)) in expected.lines().zip(actual.lines()).enumerate() {
        if want != got {
            return format!(
                "first difference at line {}:\n  committed fixture: {got}\n  real types:        {want}",
                index + 1
            );
        }
    }

    format!(
        "the line counts differ: committed fixture has {} line(s), the real types {} \
         (regenerate with UPDATE_FIXTURES=1)",
        actual.lines().count(),
        expected.lines().count()
    )
}

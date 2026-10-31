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
    "calibration.fetch_all",
    "calibration.kinds",
    "calibration.check",
    "calibration.tooltip.hover_capture",
    "calibration.tooltip.save",
    "calibration.tooltip.test",
    "calibration.tuner.check",
    "calibration.tuner.save",
    "calibration.pet.preview",
    "calibration.pet.save",
    "calibration.pet.test_read",
    "calibration.gem.moves.save",
    "calibration.gem.check_result",
    "calibration.buysell.preview",
    "calibration.shop.test_scroll",
    "buy_sell.preflight",
    "keys.parse",
    "keys.names",
    "attributes.get",
    "board.probe",
    "board.details",
    "pet.preflight",
    "pet.select_page",
    "pet.set_boarding",
    "pet.capture_icon",
    "pet.scan_food_page",
    "pet.scan_queue_page",
    "pet.scan_feeding_page",
    "pet.scan_apply_queue",
    "pet.test_read_panel",
    "pet.apply_food_scan",
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
            "request.calibration.save.with_rev",
            "52",
            "calibration.save",
            json!({
                "frame_id": "f1",
                "kind": "bag_grid",
                "rev": 10,
                "region": {"x": 12, "y": 47, "w": 300, "h": 200},
            }),
        ),
        request_case(
            "request.calibration.fetch_all",
            "53",
            "calibration.fetch_all",
            json!({}),
        ),
        request_case(
            "request.calibration.kinds",
            "54",
            "calibration.kinds",
            json!({}),
        ),
        request_case(
            "request.calibration.check",
            "55",
            "calibration.check",
            json!({}),
        ),
        request_case(
            "request.calibration.tooltip.hover_capture",
            "56",
            "calibration.tooltip.hover_capture",
            json!({"point": {"x": 120, "y": 240}, "hover_delay_ms": 700}),
        ),
        request_case(
            "request.calibration.tooltip.save",
            "57",
            "calibration.tooltip.save",
            json!({
                "offset_x": -8,
                "offset_y": -6,
                "width": 320,
                "height": 210,
                "hover_delay_ms": 900,
            }),
        ),
        request_case(
            "request.calibration.tooltip.test",
            "58",
            "calibration.tooltip.test",
            json!({"point": {"x": 120, "y": 240}, "hover_delay_ms": 700, "debug": true}),
        ),
        request_case(
            "request.calibration.tuner.check",
            "59",
            "calibration.tuner.check",
            json!({
                "frame_id": "f1",
                "boxes": {
                    "grade": {"x": 120, "y": 100, "w": 40, "h": 20},
                    "attr": {"x": 100, "y": 130, "w": 200, "h": 60},
                    "remaining": {"x": 110, "y": 200, "w": 150, "h": 20}
                }
            }),
        ),
        request_case(
            "request.calibration.tuner.save",
            "60",
            "calibration.tuner.save",
            json!({
                "frame_id": "f1",
                "boxes": {
                    "grade": {"x": 120, "y": 100, "w": 40, "h": 20},
                    "attr": {"x": 100, "y": 130, "w": 200, "h": 60},
                    "remaining": {"x": 110, "y": 200, "w": 150, "h": 20}
                },
                "row_height": 19,
                "spring_point": {"x": 500, "y": 400},
                "display": {"dpi_scale": 1.5, "monitor_dpi": 144, "screen": {"w": 1920, "h": 1080}}
            }),
        ),
        request_case(
            "request.calibration.pet.preview",
            "61",
            "calibration.pet.preview",
            json!({"draft": {
                "rows": [{"feeder_strip": {"x": 100, "y": 320, "w": 400, "h": 40}, "stacks": 4}],
                "bag_grid": [0, 0, 240, 240]
            }}),
        ),
        request_case(
            "request.calibration.pet.save",
            "62",
            "calibration.pet.save",
            json!({
                "frame_id": "f1",
                "rows": [{
                    "toggle_label": {"x": 10, "y": 200, "w": 60, "h": 20},
                    "boarding_pet_slot": {"x": 100, "y": 200, "w": 100, "h": 100},
                    "feeder_strip": {"x": 100, "y": 320, "w": 400, "h": 40},
                    "stacks": 4,
                    "pet_slot_empty_png": true
                }],
                "points": {
                    "menu": {"x": 10, "y": 10, "w": 20, "h": 20},
                    "feed": {"x": 40, "y": 10, "w": 20, "h": 20},
                    "close": {"x": 70, "y": 10, "w": 20, "h": 20},
                    "page_tabs": [{"x": 300, "y": 20}, {"x": 340, "y": 20}, {"x": 380, "y": 20}]
                },
                "bag": {"grid": [0, 0, 240, 240], "slot": [0, 0, 30, 30]},
                "feeder_count_left_fraction": 0.4,
                "feeder_count_min_score": 0.75,
                "display": {"dpi_scale": 1.5, "monitor_dpi": 144, "screen": {"w": 1920, "h": 1080}}
            }),
        ),
        request_case(
            "request.calibration.pet.test_read",
            "63",
            "calibration.pet.test_read",
            json!({
                "rows": [{"feeder_strip": {"x": 100, "y": 320, "w": 400, "h": 40}, "stacks": 4}],
                "feeder_count_left_fraction": 0.4,
                "feeder_count_min_score": 0.75
            }),
        ),
        request_case(
            "request.calibration.gem.moves.save",
            "64",
            "calibration.gem.moves.save",
            json!({"movements": {
                "register_combine": [18, -4],
                "radio_to_register": {"N": [10, 20], "G": [-5, 6]}
            }}),
        ),
        request_case(
            "request.calibration.gem.check_result",
            "65",
            "calibration.gem.check_result",
            json!({"frame_id": "f1", "area": {"x": 10, "y": 10, "w": 20, "h": 20}}),
        ),
        request_case(
            "request.calibration.buysell.preview",
            "66",
            "calibration.buysell.preview",
            json!({"draft": {
                "bag_grid": [0, 0, 800, 400],
                "bag_slot": [0, 0, 100, 50],
                "shop_region": [100, 300, 400, 400],
                "shop_rows": 10
            }}),
        ),
        request_case(
            "request.calibration.shop.test_scroll",
            "67",
            "calibration.shop.test_scroll",
            json!({"notches": 3, "up": true}),
        ),
        request_case(
            "request.buy_sell.preflight",
            "68",
            "buy_sell.preflight",
            json!({}),
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
            "request.pet.select_page",
            "70",
            "pet.select_page",
            json!({"page": 1}),
        ),
        request_case(
            "request.pet.scan_feeding_page",
            "72",
            "pet.scan_feeding_page",
            json!({"page": 1, "harvest": true}),
        ),
        request_case(
            "request.pet.scan_apply_queue",
            "73",
            "pet.scan_apply_queue",
            json!({"queue": [{"label": "page 1, cell 7", "png": "iVBORw0KGgo=", "rect": [10, 20, 30, 40]}]}),
        ),
        request_case(
            "request.pet.set_boarding",
            "45",
            "pet.set_boarding",
            json!({"row": 1, "running": true}),
        ),
        request_case(
            "request.pet.capture_icon.queue",
            "46",
            "pet.capture_icon",
            json!({"kind": "queue", "page": 1, "cell": 34}),
        ),
        request_case(
            "request.pet.capture_icon.food",
            "47",
            "pet.capture_icon",
            json!({"kind": "food", "page": 1, "cell": 34}),
        ),
        request_case(
            "request.pet.scan_food_page",
            "48",
            "pet.scan_food_page",
            json!({"page": 1}),
        ),
        request_case(
            "request.pet.scan_queue_page",
            "49",
            "pet.scan_queue_page",
            json!({"page": 1}),
        ),
        request_case(
            "request.pet.test_read_panel",
            "51",
            "pet.test_read_panel",
            json!({"page": 1, "cell": 34, "delay_ms": 700}),
        ),
        request_case(
            "request.pet.apply_food_scan",
            "50",
            "pet.apply_food_scan",
            json!({"cells": [[1, 1], [1, 34]]}),
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
            "request.tool.start.buy_dry_run",
            "71",
            "tool.start",
            json!({"tool": "buy", "dry_run": true}),
        ),
        request_case(
            "request.tool.start.quest_test",
            "69",
            "tool.start",
            json!({"tool": "quest", "test": true}),
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
            "response.ok.pet_capture_icon.queue",
            "46",
            json!({
                "kind": "queue",
                "page": 1,
                "cell": 34,
                "rect": [10, 40, 10, 10],
                "png_base64": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
            }),
        ),
        ok_case(
            "response.ok.pet_capture_icon.food",
            "47",
            json!({
                "kind": "food",
                "page": 1,
                "cell": 34,
                "rect": [10, 40, 10, 10],
                "png_base64": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
            }),
        ),
        ok_case(
            "response.ok.pet_scan_food_page",
            "48",
            json!({
                "page": 1,
                "cells": [13, 36],
                "report": ["page 1: 2 cell(s) — 13, 36"],
                "evidence": ["logs/reads/scan_page1_20261028_120000.png", "logs/reads/scan_page1_20261028_120000.txt"]
            }),
        ),
        ok_case(
            "response.ok.pet_scan_queue_page",
            "49",
            json!({
                "page": 1,
                "per_entry": [{"index": 0, "label": "red", "hits": [{"cell": 13, "score": 0.0, "runner_up": 1.0}]}],
                "claimed": [],
                "deduced": ["red ×1"],
                "evidence": ["logs/reads/scan_page1_20261028_120000.png", "logs/reads/scan_page1_20261028_120000.txt"]
            }),
        ),
        ok_case(
            "response.ok.pet_apply_food_scan",
            "50",
            json!({"rev": 1, "block": {"food_slots": [[0, 0], [0, 33]], "food_slots_used": 0}}),
        ),
        ok_case(
            "response.ok.pet_test_read_panel",
            "51",
            json!({
                "region": {"x": 33, "y": 88, "width": 120, "height": 80},
                "lines": ["(6階) 真蔚藍鳳凰 +7 [52.18%]"],
                "panel": {"growth": 7, "exp": 52.18, "line": "stage 6, +7, 52.18%"},
                "numbers": ["6", "7", "52.18"],
                "evidence": "logs/reads/petpanel_20261030_120000.png"
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
                "calibration": {"measured_at": "2026-10-30T12:00:00Z"},
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
                "calibration": {"measured_at": "2026-10-30T12:00:00Z"},
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
                "calibration": {"measured_at": "2026-10-30T12:00:00Z"},
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
        ok_case(
            "response.ok.calibration_kinds",
            "54",
            json!({
                "kinds": [
                    {"name": "bag_grid", "shape": "rect", "block": "buy_sell",
                     "path": ["buy_sell", "bag_grid"], "artifact": null,
                     "targets": "none", "row_key": null},
                    {"name": "pet_toggle", "shape": "rect", "block": "pet",
                     "path": ["pet", "slots"], "artifact": null,
                     "targets": "pet_row", "row_key": "toggle_label"},
                    {"name": "result_gem_area", "shape": "rect", "block": "gem",
                     "path": ["gem", "result_gem_area"], "artifact": "calib_gem_result.png",
                     "targets": "none", "row_key": null}
                ],
                "withdrawn": [
                    {"name": "tuner_region", "reason": "the three-box session is not built yet"}
                ]
            }),
        ),
        ok_case(
            "response.ok.calibration_fetch_all",
            "53",
            json!({
                "rev": 10,
                "calibration": {
                    "dpi_scale": 1.5,
                    "monitor_dpi": 144,
                    "screen": [1920, 1080],
                    "client_size": [1200, 600],
                    "measured_at": "2026-10-30T12:00:00Z"
                },
                "marks": [
                    {"kind": "bag_grid", "target": null, "mark": [12, 47, 300, 200], "artifact": null},
                    {"kind": "pet_toggle", "target": "1", "mark": [100, 100, 20, 20], "artifact": null},
                    {"kind": "result_gem_area", "target": null, "mark": [610, 800, 220, 60],
                     "artifact": {"file": "calib_gem_result.png", "png_base64": "iVBORw0KGgo="}}
                ],
                "withdrawn": [
                    {"name": "focus_point", "reason": "replaced by scroll_point"},
                    {"name": "tuner_region", "reason": "the three-box session is not built yet"}
                ]
            }),
        ),
        ok_case(
            "response.ok.calibration_check",
            "55",
            json!({
                "live": {
                    "scale": 1.5,
                    "monitor_dpi": 144,
                    "screen": {"w": 1920, "h": 1080},
                    "frame": {"x": 0, "y": 0, "w": 1200, "h": 640},
                    "physical_client": {"x": 10, "y": 20, "w": 1200, "h": 600},
                    "logical_client": {"x": 10, "y": 20, "w": 800, "h": 400}
                },
                "stored": {
                    "scale": 1.5,
                    "monitor_dpi": 144,
                    "screen": [1920, 1080],
                    "client_size": [1200, 600],
                    "measured_at": "2026-10-30T12:00:00Z"
                },
                "differences": [],
                "matches": true,
                "note": null
            }),
        ),
        ok_case(
            "response.ok.tooltip_save",
            "57",
            json!({
                "rev": 18,
                "block": {
                    "offset_x": -8,
                    "offset_y": -6,
                    "width": 320,
                    "height": 210,
                    "hover_delay_ms": 900
                }
            }),
        ),
        ok_case(
            "response.ok.tooltip_hover_capture",
            "56",
            json!({
                "frame": {
                    "format": "png",
                    "width": 1200,
                    "height": 600,
                    "origin": {"x": 10, "y": 20},
                    "data": "iVBORw0KGgo=",
                    "frame_id": "f5"
                },
                "display": {
                    "scale": 1.5,
                    "monitor_dpi": 144,
                    "screen": {"w": 1920, "h": 1080},
                    "frame": {"x": 0, "y": 0, "w": 1200, "h": 640},
                    "physical_client": {"x": 10, "y": 20, "w": 1200, "h": 600},
                    "logical_client": {"x": 10, "y": 20, "w": 800, "h": 400}
                },
                "placed": {"x": 120, "y": 240},
                "line": "Placed the cursor at (120, 240) and captured the panel."
            }),
        ),
        ok_case(
            "response.ok.tooltip_test",
            "58",
            json!({
                "lines": [
                    {"text": "(6階) 真蔚藍鳳凰 +7 [52.18%]", "score": 0.93}
                ],
                "region": {"x": 112, "y": 234, "width": 320, "height": 210},
                "debug_path": "logs/reads/tooltip_20261030_120000.png"
            }),
        ),
        ok_case(
            "response.ok.tuner_check",
            "59",
            json!({
                "geometry": {
                    "region": {"left": 100, "top": 100, "width": 200, "height": 120},
                    "grade_area": {"x1": 20, "y1": 0, "x2": 60, "y2": 20},
                    "grade_y": [0, 20],
                    "attr_y": [30, 90],
                    "remaining_y": [100, 120],
                    "attr_x": [0, 200],
                    "remaining_x": [10, 160],
                    "row_height": 19
                },
                "measured_row_height": 19,
                "grade": "DG",
                "remaining": 25000,
                "attributes": [["力量", "+7"], ["敏捷", "+5"], []],
                "matched": [
                    {"name": "力量", "category": "stat", "value": 7, "raw": ["力量", "+7"]}
                ],
                "filter": {
                    "enabled": true,
                    "passed": false,
                    "reason": "力量 7 is below the 10 minimum",
                    "hits": []
                },
                "warnings": []
            }),
        ),
        ok_case(
            "response.ok.tuner_save",
            "60",
            json!({
                "rev": 13,
                "block": {
                    "ocr": {
                    "region": {"left": 100, "top": 100, "width": 200, "height": 120},
                    "grade_area": {"x1": 20, "y1": 0, "x2": 60, "y2": 20},
                    "grade_y": [0, 20],
                    "attr_y": [30, 90],
                    "remaining_y": [100, 120],
                    "attr_x": [0, 200],
                    "remaining_x": [10, 160],
                    "row_height": 19
                },
                    "spring_point": [500, 400],
                    "calibration": {
                        "dpi_scale": 1.5,
                        "monitor_dpi": 144,
                        "screen": {"w": 1920, "h": 1080},
                        "client_size": [1200, 600],
                        "measured_at": "2026-10-30T12:00:00Z"
                    }
                },
                "derived": {
                    "region": {"left": 100, "top": 100, "width": 200, "height": 120},
                    "grade_area": {"x1": 20, "y1": 0, "x2": 60, "y2": 20},
                    "grade_y": [0, 20],
                    "attr_y": [30, 90],
                    "remaining_y": [100, 120],
                    "attr_x": [0, 200],
                    "remaining_x": [10, 160],
                    "row_height": 19
                },
                "artifact": {"file": "calib_tuner.png", "path": "config/calib_tuner.png"}
            }),
        ),
        ok_case(
            "response.ok.pet_preview",
            "61",
            json!({
                "rows": [{
                    "slot_boxes": [[100, 320, 100, 40], [200, 320, 100, 40], [300, 320, 100, 40], [400, 320, 100, 40]],
                    "count_regions": [[140, 320, 60, 40], [240, 320, 60, 40], [340, 320, 60, 40], [440, 320, 60, 40]],
                    "eta_region": [96, 360, 188, 48],
                    "problems": []
                }],
                "bag_centres": [[15, 15]],
                "problem": null
            }),
        ),
        ok_case(
            "response.ok.pet_save",
            "62",
            json!({
                "rev": 17,
                "block": {
                    "menu_button": [10, 10, 20, 20],
                    "feed_icon": [40, 10, 20, 20],
                    "close_button": [70, 10, 20, 20],
                    "page_tabs": [[300, 20], [340, 20], [380, 20]],
                    "bag_grid": [0, 0, 240, 240],
                    "bag_slot": [0, 0, 30, 30],
                    "feeder_count_left_fraction": 0.4,
                    "feeder_count_min_score": 0.75,
                    "slots": [{
                        "enabled": true,
                        "toggle_label": [10, 200, 60, 20],
                        "boarding_pet_slot": [100, 200, 100, 100],
                        "feeder_strip": [100, 320, 400, 40],
                        "stacks": 4,
                        "pet_slot_empty_png": "iVBORw0KGgo="
                    }]
                },
                "artifacts": [{"file": "calib_pet.png", "path": "config/calib_pet.png"}],
                "derived": {"rows": [{
                    "slot_boxes": [[100, 320, 100, 40], [200, 320, 100, 40], [300, 320, 100, 40], [400, 320, 100, 40]],
                    "count_regions": [[140, 320, 60, 40], [240, 320, 60, 40], [340, 320, 60, 40], [440, 320, 60, 40]],
                    "eta_region": [96, 360, 188, 48],
                    "problems": []
                }]},
                "warnings": []
            }),
        ),
        ok_case(
            "response.ok.pet_test_read",
            "63",
            json!({
                "rows": [{
                    "row": 1,
                    "slots": [{
                        "index": 1,
                        "reads": [{"text": "138", "score": 0.99, "upscale": 3}],
                        "count": 138
                    }],
                    "eta_region": [96, 360, 188, 48]
                }],
                "hidden_ms": 420
            }),
        ),
        ok_case(
            "response.ok.gem_moves_save",
            "64",
            json!({
                "rev": 15,
                "block": {"movements": {
                    "register_combine": [18, -4],
                    "radio_to_register": {"N": [10, 20], "G": [-5, 6]}
                }}
            }),
        ),
        ok_case(
            "response.ok.gem_check_result",
            "65",
            json!({
                "composition": {
                    "mean_v": 0.9,
                    "mean_sat": 0.1,
                    "colored_fraction": 0.0,
                    "mean_r": 0.88,
                    "mean_g": 0.9,
                    "mean_b": 0.92
                },
                "distance": 0.012,
                "verdict": "empty",
                "spread": 0.04,
                "dominant": "neutral / pale",
                "crop_png_base64": "iVBORw0KGgo="
            }),
        ),
        ok_case(
            "response.ok.buysell_preview",
            "66",
            json!({
                "bag_centres": [[50, 25], [150, 25]],
                "row_centres": [[300, 320]],
                "pitch": {"w": 100, "h": 50},
                "row_pitch": 40.0,
                "problem": null
            }),
        ),
        ok_case(
            "response.ok.shop_test_scroll",
            "67",
            json!({"line": "clicked the focus point at (1000, 420) and scrolled 3 notch(es) up through COM5"}),
        ),
        ok_case(
            "response.ok.buy_sell_preflight",
            "68",
            json!({
                "ready": true,
                "calibration": [
                    {"ok": true, "label": "Bag grid", "detail": "marked"},
                    {"ok": true, "label": "Bag slot", "detail": "marked"}
                ],
                "session": [{"ok": true, "label": "ready", "detail": "yes"}],
                "lines": []
            }),
        ),
        ok_case(
            "response.ok.pet_scan_feeding_page",
            "72",
            json!({
                "page": 1,
                "cells": [
                    {"cell": 6, "label": "page 1, cell 7", "status": "feedable", "growth": 7, "exp": 52.0, "line": "stage 6, +7, 52%", "match": 0.02, "crop_png_base64": "iVBORw0KGgo="},
                    {"cell": 8, "label": "page 1, cell 9", "status": "unknown", "growth": null, "exp": null, "line": null, "match": 0.03}
                ],
                "summary": "1 feedable, 0 finished, 1 unread",
                "report": ["cell 7: feedable - stage 6, +7, 52%"],
                "evidence": ["logs/reads/pet_feed_page1_20261101_000000_000.png"]
            }),
        ),
        ok_case(
            "response.ok.pet_scan_apply_queue",
            "73",
            json!({
                "rev": 3,
                "block": {"queue": [{"label": "page 1, cell 7", "png": "iVBORw0KGgo=", "rect": [10, 20, 30, 40]}]}
            }),
        ),
        ok_case(
            "response.ok.pet_select_page",
            "70",
            json!({"page": 1, "point": [300, 20], "line": "clicked the ITEM1 tab at (300, 20)"}),
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
                        "next_action_in_seconds": 1800.0,
                        "next_action": "reload"
                    },
                    {
                        "name": "row 2",
                        "status": "waiting for a reload",
                        "detail": null,
                        "next_action_in_seconds": null,
                        "next_action": null
                    }
                ],
                "detail": [],
                "error": null
            }),
        ),
        // One running tool's labelled facts (PLAN-V3-STATUS-DETAIL §1.3): a labelled fact, a
        // label-less attribute line and the filter word. `[]` is "no detail", exactly like `rows`.
        event_case(
            "event.state.detail",
            "state",
            json!({
                "tool": "tuner",
                "running": true,
                "phase": "running",
                "message": "",
                "countdown_s": null,
                "rows": [],
                "detail": [
                    {"label": "Grade", "value": "DG"},
                    {"label": "Remaining", "value": "10"},
                    {"label": "Attempt", "value": "7"},
                    {"label": null, "value": "attack speed=+10"},
                    {"label": "Filter", "value": "disabled"}
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
                "detail": [],
                "error": null
            }),
        ),
        event_case(
            "event.tool.finished",
            "tool.finished",
            json!({
                "tool": "pet",
                "reason": "done",
                "message": "DONE — 4 row(s)",
                "summary": null,
                "ended_at": "2026-10-30T12:00:00Z"
            }),
        ),
        // The per-tool end-of-run facts (G11): the Tuner's last grade/attempts/captures and which
        // stop reason ended it, plus the core's UTC finish instant. Additive v:1.
        event_case(
            "event.tool.finished.summary",
            "tool.finished",
            json!({
                "tool": "tuner",
                "reason": "done",
                "message": "Reached DG after 7 attempts",
                "summary": {
                    "grade": "DG",
                    "attempts": 7,
                    "captures_saved": 0,
                    "stopped_by": "done"
                },
                "ended_at": "2026-10-30T12:34:56Z"
            }),
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

//! The request dispatcher behind the stdio loop: `PROTOCOL.md` §1 (handshake) and §4 (commands).

use seal_core::dispatch::{CoreState, handle};
use seal_core::logs::Logs;
use seal_core::registry::{Session, ToolRegistry, Unavailable};
use seal_core::supervisor::{StartReply, Supervisor};
use seal_domain::decision::{Decision, Key, StopReason};
use seal_domain::geometry::Point;
use seal_proto::Request;
use seal_tools::game::GamePort;
use seal_tools::runner::{EffectError, Effects};
use seal_tools::tool::ToolId;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

fn state() -> CoreState {
    CoreState::new(PathBuf::from(r"C:\repo"))
}

fn hello() -> Request {
    Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"}))
}

#[test]
fn hello_answers_with_the_handshake_data() {
    let mut state = state();

    let response = handle(&mut state, hello());

    assert!(response.ok, "hello must succeed: {:?}", response.error);
    let result = response.result.expect("hello carries a result");
    assert_eq!(result["protocol"], 1);
    assert_eq!(result["core_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["root"], r"C:\repo");
    assert_eq!(result["rev"], 0);
    assert!(result["config"].is_object(), "config starts as an object");
}

#[test]
fn keys_parse_answers_every_code_and_names_every_bad_box() {
    let mut state = state();
    handle(&mut state, hello());

    let response = handle(
        &mut state,
        Request::new(
            "5",
            "keys.parse",
            json!({"names": {
                "start": "F12",
                "quit": "Escape",
                "advance_grade": "g",
                "pause": "Zulu",
            }}),
        ),
    );

    assert!(response.ok, "parse: {:?}", response.error);
    let result = response.result.expect("a result");
    assert_eq!(result["codes"]["start"], 123, "F12 is 0x7B");
    assert_eq!(result["codes"]["quit"], 27, "Escape is 0x1B");
    assert_eq!(
        result["codes"]["advance_grade"], 71,
        "a letter upper-cases to its code"
    );
    assert!(
        result["codes"].get("pause").is_none(),
        "the bad box carries no code"
    );
    let errors = result["errors"].as_array().expect("errors is an array");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["field"], "pause");
    assert_eq!(
        errors[0]["input"], "Zulu",
        "the message quotes what the player typed"
    );
    assert!(
        errors[0]["line"]
            .as_str()
            .expect("a line")
            .contains("\"Zulu\" is not a key"),
        "the domain's line travels verbatim: {:?}",
        errors[0]["line"]
    );
}

#[test]
fn keys_names_round_trips_a_parsed_code_and_shows_the_hex_fallback() {
    let mut state = state();
    handle(&mut state, hello());

    let parsed = handle(
        &mut state,
        Request::new("5", "keys.parse", json!({"names": {"start": "F05"}})),
    );
    let code = parsed.result.expect("a result")["codes"]["start"].clone();
    assert_eq!(code, 116, "F05 is F5, which is 0x74");

    let named = handle(
        &mut state,
        Request::new(
            "6",
            "keys.names",
            json!({"codes": {"start": code, "pause": 5}}),
        ),
    );

    assert!(named.ok, "names: {:?}", named.error);
    let result = named.result.expect("a result");
    assert_eq!(
        result["names"]["start"], "F5",
        "the typed name comes back canonical"
    );
    assert_eq!(
        result["names"]["pause"], "0x5",
        "a code with no name shows its hex, as v2's VkName did"
    );
}

#[test]
fn keys_commands_refuse_unshaped_arguments() {
    let mut state = state();
    handle(&mut state, hello());

    let parse = handle(
        &mut state,
        Request::new("5", "keys.parse", json!({"names": "F12"})),
    );
    assert!(!parse.ok);
    assert_eq!(parse.error.expect("an error").code, "bad_request");

    let names = handle(
        &mut state,
        Request::new("6", "keys.names", json!({"codes": {"start": "F12"}})),
    );
    assert!(!names.ok);
    assert_eq!(names.error.expect("an error").code, "bad_request");
}

#[test]
fn attributes_get_serves_the_loaded_dictionary_with_its_variant() {
    let mut state = state();
    handle(&mut state, hello());
    state.variant = Some("us".to_owned());
    state.dictionary = "attributes.us.yaml".to_owned();
    state.attributes = seal_domain::config::AttributesConfig {
        attributes: vec![seal_domain::config::AttributeDef {
            name: "減少傷害".to_owned(),
            category: "plv1".to_owned(),
            variants: vec!["減少傷害".to_owned(), "减少伤害".to_owned()],
        }],
        ..Default::default()
    };

    let response = handle(&mut state, Request::new("5", "attributes.get", json!({})));

    assert!(
        response.ok,
        "attributes.get must succeed: {:?}",
        response.error
    );
    let result = response.result.expect("attributes.get carries a result");
    assert_eq!(result["variant"], "us");
    assert_eq!(result["dictionary"], "attributes.us.yaml");
    assert_eq!(result["attributes"]["attributes"][0]["name"], "減少傷害");
    assert_eq!(
        result["attributes"]["attributes"][0]["variants"][1],
        "减少伤害"
    );
}

#[test]
fn a_request_before_hello_is_refused() {
    let mut state = state();

    let response = handle(&mut state, Request::new("2", "window.find", json!({})));

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "not_ready");
}

#[test]
fn a_protocol_mismatch_is_refused() {
    let mut state = state();

    let response = handle(
        &mut state,
        Request::new("3", "hello", json!({"protocol": 99, "client": "0.1.0"})),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_version");
}

#[test]
fn an_unknown_command_is_a_bad_request() {
    let mut state = state();
    handle(&mut state, hello());

    let response = handle(&mut state, Request::new("4", "no.such.command", json!({})));

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_request");
}

#[test]
fn shutdown_is_acknowledged_and_stops_the_loop() {
    let mut state = state();
    handle(&mut state, hello());

    let response = handle(&mut state, Request::new("5", "shutdown", json!({})));

    assert!(response.ok, "shutdown is answered before the process exits");
    assert!(
        state.stopping,
        "the loop must stop after the acknowledgement"
    );
}

fn handshaken_state() -> CoreState {
    let mut state = state();
    handle(&mut state, hello());
    state
}

/// The dispatcher-level checks: argument shape, the authoring rules, and the revision contract,
/// without a config folder (the write paths are exercised end-to-end in `tests/config.rs`).
#[test]
fn config_validate_answers_with_empty_arrays_for_a_clean_section() {
    let mut state = handshaken_state();

    let response = handle(
        &mut state,
        Request::new(
            "6",
            "config.validate",
            json!({"sections": {"quest": {"active": "run1"}}}),
        ),
    );

    assert!(response.ok, "shape validation passes: {:?}", response.error);
    assert_eq!(
        response.result.expect("a result"),
        json!({"errors": [], "warnings": []})
    );
}

/// The Quest tab's Save gate: every sequence entry must name an existing flow. The lines are the
/// domain's, exactly as the v2 tab showed them — one player-facing line per array entry.
#[test]
fn config_validate_reports_a_sequence_entry_naming_a_missing_flow() {
    let mut state = handshaken_state();
    let quest = json!({
        "active": "run1",
        "presets": {"run1": {"steps": []}},
        "active_sequence": "seq1",
        "sequences": {
            "seq1": {
                "initial_wait_seconds": 10,
                "loops": 1,
                "entries": [{"preset": "ghost", "times": 2}],
            }
        },
    });

    let response = handle(
        &mut state,
        Request::new(
            "13",
            "config.validate",
            json!({"sections": {"quest": quest}}),
        ),
    );

    assert!(response.ok, "{:?}", response.error);
    let result = response.result.expect("a result");
    assert_eq!(
        result["errors"],
        json!([
            "\"seq1\" entry 1 names the flow \"ghost\", which does not exist",
            "Pick an existing flow for each entry, or remove it, then Save again.",
        ])
    );
    assert_eq!(result["warnings"], json!([]));
}

/// A hotkey is stored as a virtual-key code and any VK code is a legal setting: v2's
/// `ConfigValidator` had no hotkey rule and `GetAsyncKeyState` accepts every code, so a hand-edited
/// config with a code that has no display name (`0x05`) ran fine. Validation must not refuse it —
/// the typed-text rule belongs to the tab's own input, not to the stored config.
#[test]
fn config_validate_accepts_an_unnameable_but_valid_hotkey_code() {
    let mut state = handshaken_state();

    let response = handle(
        &mut state,
        Request::new(
            "14",
            "config.validate",
            json!({"sections": {"settings": {"hotkeys": {
                "start": 5,
                "quit": 7,
                "advance_grade": 71,
                "pause": 20,
            }}}}),
        ),
    );

    assert!(response.ok, "{:?}", response.error);
    let result = response.result.expect("a result");
    assert_eq!(
        result,
        json!({"errors": [], "warnings": []}),
        "a stored VK code is not a typed name and has no rule to break"
    );
}

/// The Setup tab's client switch is a save of `settings.game.variant`, and the rule resolves the
/// name before anything is committed: an unknown client is refused with the known ones listed.
#[test]
fn config_validate_refuses_an_unknown_game_variant() {
    let mut state = handshaken_state();

    let response = handle(
        &mut state,
        Request::new(
            "15",
            "config.validate",
            json!({"sections": {"settings": {"game": {"variant": "nope"}}}}),
        ),
    );

    assert!(response.ok, "{:?}", response.error);
    assert_eq!(
        response.result.expect("a result")["errors"],
        json!(["\u{22}nope\u{22} is not a known game variant. Known: tw, us."])
    );
}

/// A clean `settings` save (valid hotkeys, a known variant) still reports nothing.
#[test]
fn config_validate_accepts_a_clean_settings_save() {
    let mut state = handshaken_state();

    let response = handle(
        &mut state,
        Request::new(
            "16",
            "config.validate",
            json!({"sections": {"settings": {
                "hotkeys": {"start": 123, "quit": 122, "advance_grade": 71, "pause": 20},
                "game": {"variant": "us"},
            }}}),
        ),
    );

    assert!(response.ok, "{:?}", response.error);
    assert_eq!(
        response.result.expect("a result"),
        json!({"errors": [], "warnings": []})
    );
}

/// Apply runs the same checks BEFORE writing: it refuses with the rule lines in `detail.errors`,
/// and a refusal must not consume the revision it was sent.
#[test]
fn config_apply_refuses_a_section_the_rules_reject_without_consuming_a_revision() {
    let mut state = handshaken_state();
    let quest = json!({
        "active": "run1",
        "presets": {"run1": {"steps": []}},
        "sequences": {"seq1": {"entries": [{"preset": "", "times": 1}]}},
    });

    let response = handle(
        &mut state,
        Request::new(
            "17",
            "config.apply",
            json!({"rev": 0, "sections": {"quest": quest}}),
        ),
    );

    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert_eq!(
        error.detail["errors"],
        json!([
            "\"seq1\" entry 1 has no flow chosen",
            "Pick an existing flow for each entry, or remove it, then Save again.",
        ])
    );
    assert_eq!(state.rev, 0, "a refused apply must not consume a revision");
}

#[test]
fn config_validate_refuses_a_block_the_app_does_not_own() {
    let mut state = handshaken_state();

    let response = handle(
        &mut state,
        Request::new(
            "7",
            "config.validate",
            json!({"sections": {"my_notes": {}}}),
        ),
    );

    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "bad_request");
    assert_eq!(error.detail["section"], "my_notes");
}

#[test]
fn config_apply_refuses_a_missing_or_non_numeric_rev() {
    let mut state = handshaken_state();

    for args in [
        json!({"sections": {"quest": {}}}),
        json!({"rev": "1", "sections": {"quest": {}}}),
        json!({"rev": -1, "sections": {"quest": {}}}),
    ] {
        let response = handle(&mut state, Request::new("8", "config.apply", args.clone()));
        assert!(!response.ok, "args {args} must be refused");
        assert_eq!(response.error.expect("a refusal").code, "bad_request");
    }
}

#[test]
fn config_apply_refuses_a_stale_rev_and_carries_the_current_one() {
    let mut state = handshaken_state();
    assert_eq!(state.rev, 0);

    let response = handle(
        &mut state,
        Request::new(
            "9",
            "config.apply",
            json!({"rev": 7, "sections": {"quest": {"active": "x"}}}),
        ),
    );

    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "conflict");
    assert_eq!(error.detail["rev"], 0);
    assert_eq!(state.rev, 0, "a refused call must not consume a revision");
}

#[test]
fn config_apply_refuses_a_section_that_is_not_a_mapping() {
    let mut state = handshaken_state();

    let response = handle(
        &mut state,
        Request::new(
            "10",
            "config.apply",
            json!({"rev": 0, "sections": {"quest": "not a block"}}),
        ),
    );

    assert!(!response.ok);
    assert_eq!(response.error.expect("a refusal").code, "bad_request");
}

#[test]
fn ui_window_refuses_malformed_arguments() {
    let mut state = handshaken_state();

    for args in [
        json!([]),
        json!({"left": 100, "top": 50}),
        json!({"left": "100", "top": 50, "width": 900, "height": 700, "pinned": true}),
    ] {
        let response = handle(&mut state, Request::new("11", "ui.window", args.clone()));
        assert!(!response.ok, "args {args} must be refused");
        assert_eq!(response.error.expect("a refusal").code, "bad_request");
    }
}

#[test]
fn the_config_commands_before_hello_are_not_ready() {
    for cmd in [
        "config.apply",
        "config.validate",
        "config.reload",
        "ui.window",
    ] {
        let mut state = state();
        let response = handle(&mut state, Request::new("12", cmd, json!({})));
        assert!(!response.ok, "{cmd} before hello");
        assert_eq!(response.error.expect("a refusal").code, "not_ready");
    }
}

fn window_state(title: &str) -> CoreState {
    let mut state = state();
    handle(&mut state, hello());
    state.config = json!({"window": {"title": title}});
    state
}

#[test]
fn window_find_reports_not_found_as_a_result_not_an_error() {
    let mut state = window_state("SEAL-TEST-NO-SUCH-WINDOW-7F3A");

    let response = handle(&mut state, Request::new("win1", "window.find", json!({})));

    assert!(response.ok, "not-found is a result: {:?}", response.error);
    assert_eq!(response.result.unwrap()["found"], false);
}

#[test]
fn window_commands_without_a_configured_title_are_bad_requests() {
    let mut state = state();
    handle(&mut state, hello());

    let response = handle(
        &mut state,
        Request::new("win2", "window.measure", json!({})),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_request");
}

#[test]
fn measure_and_capture_for_a_missing_window_are_window_not_found() {
    let mut state = window_state("SEAL-TEST-NO-SUCH-WINDOW-7F3A");

    for command in ["window.measure", "capture"] {
        let response = handle(&mut state, Request::new("win3", command, json!({})));
        assert!(!response.ok, "{command} should fail");
        assert_eq!(
            response.error.unwrap().code,
            "window_not_found",
            "{command}"
        );
    }
}

#[test]
fn capture_refuses_a_malformed_region_before_touching_windows() {
    let mut state = window_state("SEAL-TEST-NO-SUCH-WINDOW-7F3A");

    let response = handle(
        &mut state,
        Request::new("win4", "capture", json!({"region": {"x": "nope"}})),
    );

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_request");
}

fn board_state(port: &str) -> CoreState {
    let mut state = state();
    handle(&mut state, hello());
    state.config = json!({"arduino": {"port": port}});
    state
}

#[test]
fn board_probe_with_a_named_missing_port_is_board_unavailable() {
    let mut state = board_state("COM_DOES_NOT_EXIST_999");

    let response = handle(&mut state, Request::new("bd1", "board.probe", json!({})));

    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "board_unavailable");
    assert!(
        error.message.contains("does not have"),
        "the port_choice refusal names the absent port: {}",
        error.message
    );
}

#[test]
fn board_test_without_a_kind_is_a_bad_request() {
    let mut state = board_state("COM_DOES_NOT_EXIST_999");

    let response = handle(&mut state, Request::new("bd2", "board.test", json!({})));

    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_request");
}

#[test]
fn board_test_move_without_dy_refuses_before_opening_the_port() {
    let mut state = board_state("COM_DOES_NOT_EXIST_999");

    let response = handle(
        &mut state,
        Request::new("bd3", "board.test", json!({"kind": "move", "dx": 10})),
    );

    assert!(!response.ok, "a malformed move must not reach a board");
    assert_eq!(response.error.unwrap().code, "bad_request");
}

// ── CORE-F14: the board diagnostics take the input lease ───────────────────────────────────────

/// A tool that blocks in its first decision, so a foreground run really holds the arbiter while the
/// board command is issued. No hardware and no window.
struct HeldTool {
    release: Mutex<mpsc::Receiver<()>>,
}

impl seal_tools::runner::Tool for HeldTool {
    fn id(&self) -> ToolId {
        ToolId::Tuner
    }

    fn decide(&mut self) -> Decision {
        let _ = self.release.lock().expect("not poisoned").recv();
        Decision::stop(StopReason::done("released"))
    }
}

struct NoEffects;

impl Effects for NoEffects {
    fn click(&mut self, _at: Point) -> Result<(), EffectError> {
        Ok(())
    }

    fn key(&mut self, _key: Key) -> Result<(), EffectError> {
        Ok(())
    }
}

struct HeldRegistry {
    release: Mutex<Option<mpsc::Receiver<()>>>,
}

impl ToolRegistry for HeldRegistry {
    fn available(&self, _tool: ToolId) -> Result<(), Unavailable> {
        Ok(())
    }

    fn create(
        &self,
        _tool: ToolId,
        _game: Arc<dyn GamePort>,
        _args: &Value,
    ) -> Result<Session, String> {
        let release = self
            .release
            .lock()
            .expect("not poisoned")
            .take()
            .expect("one create");
        Ok(Session {
            tool: Box::new(HeldTool {
                release: Mutex::new(release),
            }),
            effects: Box::new(NoEffects),
        })
    }
}

/// CORE-F14: `board.test` and `board.probe` open the port. While a run holds it that used to surface
/// as a raw OS "access denied" under `board_unavailable`, which reads like a broken install. Both now
/// answer the same `tool_busy` line a start gets, naming the holder.
#[test]
fn board_test_and_probe_report_tool_busy_while_a_run_holds_the_board() {
    let (release, release_rx) = mpsc::channel();
    let registry = Arc::new(HeldRegistry {
        release: Mutex::new(Some(release_rx)),
    });
    let (bus, _events) = seal_core::events::event_channel();
    let mut state = board_state("COM_DOES_NOT_EXIST_999");
    state.attach_supervisor(Supervisor::start(registry, bus, Arc::new(Logs::disabled())));
    assert_eq!(
        state
            .supervisor
            .as_ref()
            .expect("attached")
            .start_tool(ToolId::Tuner, &Value::Null),
        StartReply::Started
    );

    for (id, command, args) in [
        (
            "lease-test",
            "board.test",
            json!({"kind": "key", "key": "E"}),
        ),
        ("lease-probe", "board.probe", json!({})),
    ] {
        let response = handle(&mut state, Request::new(id, command, args));
        let error = response
            .error
            .unwrap_or_else(|| panic!("{command} is refused while a run holds the board"));
        assert_eq!(error.code, "tool_busy", "{command}");
        assert_eq!(error.detail["command"], command);
        assert!(
            error.message.contains("\"tuner\""),
            "{command}: {}",
            error.message
        );
    }

    let _ = release.send(());
    state.shutdown_tools("test");
}

fn ocr_state_with_frame() -> CoreState {
    use base64::Engine as _;
    let mut state = state();
    handle(&mut state, hello());
    let bgra = [10u8, 20, 30, 255];
    let png = seal_win::pure::encode_png(1, 1, &bgra).expect("a synthetic frame encodes");
    let data = base64::engine::general_purpose::STANDARD.encode(&png);
    state.frames.insert(
        json!({"format": "png", "width": 1, "height": 1, "origin": {"x": 0, "y": 0}, "data": data}),
    );
    state
}

#[test]
fn ocr_scan_without_a_mode_is_a_bad_request() {
    let mut state = ocr_state_with_frame();
    let response = handle(
        &mut state,
        Request::new("ocr1", "ocr.scan", json!({"frame_id": "f1"})),
    );
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_request");
}

#[test]
fn ocr_scan_with_an_unknown_mode_is_a_bad_request() {
    let mut state = ocr_state_with_frame();
    let response = handle(
        &mut state,
        Request::new(
            "ocr2",
            "ocr.scan",
            json!({"frame_id": "f1", "mode": "tooltip"}),
        ),
    );
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_request");
}

#[test]
fn ocr_scan_without_a_frame_or_region_is_a_bad_request() {
    let mut state = ocr_state_with_frame();
    let response = handle(
        &mut state,
        Request::new("ocr3", "ocr.scan", json!({"mode": "lines"})),
    );
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_request");
}

#[test]
fn ocr_scan_with_an_unknown_frame_id_is_a_bad_request() {
    let mut state = ocr_state_with_frame();
    let response = handle(
        &mut state,
        Request::new(
            "ocr4",
            "ocr.scan",
            json!({"frame_id": "nope", "mode": "lines"}),
        ),
    );
    assert!(!response.ok);
    assert_eq!(response.error.unwrap().code, "bad_request");
}

#[test]
fn ocr_scan_without_models_is_ocr_unavailable() {
    if std::env::var_os("SEAL_OCR_MODELS").is_some() {
        return; // this machine has the models configured; the absent path is not testable here
    }
    let mut state = ocr_state_with_frame();
    let response = handle(
        &mut state,
        Request::new(
            "ocr5",
            "ocr.scan",
            json!({"frame_id": "f1", "mode": "lines"}),
        ),
    );
    assert!(!response.ok);
    let error = response.error.expect("a refusal");
    assert_eq!(error.code, "ocr_unavailable");
    assert_eq!(error.detail["reason"], "models_env_unset");
    assert!(
        error.detail["message"]
            .as_str()
            .is_some_and(|message| message.contains("Tried:")),
        "the failure line reports the attempted directories: {:?}",
        error.detail
    );
}

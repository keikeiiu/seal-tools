//! The request dispatcher: one place where a [`Request`] becomes a [`Response`].
//!
//! It is deliberately separate from the stdio loop so the handshake and the command surface can
//! be tested without spawning a process (`PROTOCOL.md` §1 and §4).
//!
//! The config commands are the shell's only writer-side surface, and their contract is
//! `PROTOCOL.md` §§4 and 7 plus `PLAN-V3-CORE.md` §4.1: the core is the only merger, a save sends
//! **only the blocks the tab owns**, a stale revision is refused rather than replayed, and the rev
//! is a session counter the shell echoes back. Every save also runs the domain authoring rules
//! ([`crate::validate`]) before it writes — no widget keeps a rule. The file semantics themselves
//! live in [`seal_config::ConfigStore`] — this module only speaks protocol to them.
//!
//! The tool commands ([`tool_start`], [`tool_stop`]) are thin: they validate the id against
//! `ToolId::ALL` (the ONE accept-and-dispatch list, `OPEN.md` §1 item 7) and translate the
//! supervisor's typed answer into the protocol's `tool_busy`/`not_running`/`bad_request` and events.

use std::path::Path;

use seal_config::{ConfigError, ConfigStore};
use seal_proto::{Error, PROTOCOL_VERSION, Request, Response};
use seal_tools::tool::ToolId;
use serde_json::{Map, Value, json};

use crate::supervisor::{self, StartReply, StopReply};

/// The protocol's `ui.window` fields, mapped to the names a v2 `local.yaml` already uses.
///
/// `height` is the window height to restore when the config region is expanded — `expanded_height`
/// in v2's `LocalUi` and in the domain model — so it is stored under that name rather than putting a
/// second key into the player's file. The whole block is the five v2 fields, and it is core-owned:
/// the UI only ever reaches it through `ui.window`.
const WINDOW_FIELDS: &[(&str, &str)] = &[
    ("left", "left"),
    ("top", "top"),
    ("width", "width"),
    ("height", "expanded_height"),
    ("pinned", "pinned"),
];

/// The state one connection's dispatcher works on: the root it bound to, the config store that owns
/// the files, the merged config as the shell sees it, and the session revision.
///
/// `rev` is a SESSION revision, not an on-disk counter: it starts at 0 for one core process and is
/// incremented by every successful write or reload. The shell holds the value from `hello` and sends
/// it back with `config.apply`, so a stale copy is refused rather than silently overwriting a block
/// the core wrote in the meantime (`PROTOCOL.md` §7, `PLAN-V3-CORE.md` §4.1).
pub struct CoreState {
    pub root: std::path::PathBuf,
    pub store: ConfigStore,
    pub rev: u64,
    pub config: Value,
    pub handshaken: bool,
    pub stopping: bool,
    /// The tool layer, attached by the binary. `None` in the config-only dispatcher tests, and in a
    /// future core that has no tools yet — a tool command then says so rather than pretending.
    pub supervisor: Option<supervisor::Supervisor>,
}

impl CoreState {
    /// An empty state, for a dispatcher test that needs no files on disk. The binary uses
    /// [`Self::load`] instead, which is the only constructor that reads the config.
    pub fn new(root: std::path::PathBuf) -> Self {
        let store = ConfigStore::new(root.join("config"));
        Self {
            root,
            store,
            rev: 0,
            config: json!({}),
            handshaken: false,
            stopping: false,
            supervisor: None,
        }
    }

    /// The binary's startup: bind to `root` and read the merged config through the store.
    ///
    /// Failing here is fatal rather than a degraded start — v2 fails loudly for the same reason: a
    /// core with no config cannot run a tool, and a silent empty config would look exactly like a
    /// game that changed.
    pub fn load(root: std::path::PathBuf) -> Result<Self, ConfigError> {
        let store = ConfigStore::new(root.join("config"));
        let config = store.load()?;
        Ok(Self {
            root,
            store,
            rev: 0,
            config,
            handshaken: false,
            stopping: false,
            supervisor: None,
        })
    }

    /// Gives the dispatcher its tool layer. Only the binary calls this; the supervisor needs the
    /// writer's [`crate::events::EventBus`], so it cannot be built inside a config-only state.
    pub fn attach_supervisor(&mut self, supervisor: supervisor::Supervisor) {
        self.supervisor = Some(supervisor);
    }

    /// Stops every run, waits for the workers and emits `core.stopping`, then joins the supervisor
    /// thread. Called by the binary after the final response is queued and before the writer is
    /// finished — the supervisor is the other `EventBus` holder, and the writer only stops when the
    /// last one is gone.
    pub fn shutdown_tools(&mut self, reason: &str) {
        if let Some(mut supervisor) = self.supervisor.take() {
            supervisor.shutdown(reason);
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
        "config.validate" => config_validate(state, &request),
        "config.apply" => config_apply(state, &request),
        "config.reload" => config_reload(state, &request),
        "ui.window" => ui_window(state, &request),
        "tool.start" => tool_start(state, &request),
        "tool.stop" => tool_stop(state, &request),
        other => bad_request(
            &request.id,
            &format!("unknown command {other:?}"),
            json!({}),
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

/// `config.validate {sections}`: the pre-Save gate.
///
/// The shape is checked first (owned block names, mappings), then the domain's authoring rules run
/// on a COPY of the merged config with the given sections laid on it ([`crate::validate`]). The
/// answer is one player-facing line per array entry — the same lines the v2 tab showed — and
/// `warnings` is the fixed protocol slot, empty until a rule has an advisory result.
fn config_validate(state: &CoreState, request: &Request) -> Response {
    let sections = match sections_argument(request) {
        Ok(sections) => sections,
        Err(response) => return *response,
    };

    let outcome = crate::validate::validate_sections(state, &sections);
    Response::ok(
        &request.id,
        json!({"errors": outcome.errors, "warnings": outcome.warnings}),
    )
}

/// `config.apply {rev, sections}`: merge the blocks the shell owns into `local.yaml`, bump the
/// revision, answer with the merged sections (`PROTOCOL.md` §4 and §7).
///
/// The SAME authoring checks `config.validate` runs go first, so a Save cannot bypass the gate the
/// shell showed: on any error the write is refused as `bad_request` with the lines in
/// `detail.errors`, and nothing is written or consumed. (Warnings have no channel in this response —
/// the protocol fixes its shape at `{rev, sections}` — so they are reported by `config.validate`
/// only; no current rule produces one.)
fn config_apply(state: &mut CoreState, request: &Request) -> Response {
    let rev = match revision_argument(request) {
        Ok(rev) => rev,
        Err(response) => return *response,
    };
    let sections = match sections_argument(request) {
        Ok(sections) => sections,
        Err(response) => return *response,
    };

    // A stale client is told to reconcile first: its rules may already be moot against the current
    // config, and replaying an old view is the loss this revision exists to prevent.
    if rev != state.rev {
        return conflict(state, &request.id, &sections);
    }

    let outcome = crate::validate::validate_sections(state, &sections);
    if !outcome.errors.is_empty() {
        return bad_request(
            &request.id,
            "the config change was refused by the authoring rules",
            json!({"errors": outcome.errors}),
        );
    }

    let merged = match state.store.apply_sections(&sections) {
        Ok(merged) => merged,
        Err(error) => return config_error_response(&request.id, error),
    };
    state.rev += 1;
    refresh_config(state);

    Response::ok(&request.id, json!({"rev": state.rev, "sections": merged}))
}

/// `config.reload`: re-read the three layers through the store and answer the whole config. A reload
/// is a change the shell must see, so it bumps the revision even though no file was written.
fn config_reload(state: &mut CoreState, request: &Request) -> Response {
    match state.store.load() {
        Ok(config) => {
            state.config = config;
            state.rev += 1;
            Response::ok(
                &request.id,
                json!({"rev": state.rev, "config": state.config}),
            )
        }
        Err(error) => config_error_response(&request.id, error),
    }
}

/// `ui.window {left, top, width, height, pinned}`: the one path window geometry and pin state take
/// into `local.yaml`. The `ui:` block is core-owned — nobody edits it in a tab — so the shell sends
/// it and the core is the writer.
fn ui_window(state: &mut CoreState, request: &Request) -> Response {
    let Some(arguments) = request.args.as_object() else {
        return bad_request(
            &request.id,
            "ui.window needs an object of window fields",
            json!({}),
        );
    };

    let mut ui = Map::new();
    for (protocol_name, stored_name) in WINDOW_FIELDS {
        let Some(value) = arguments.get(*protocol_name) else {
            return bad_request(
                &request.id,
                &format!("ui.window needs {protocol_name:?}"),
                json!({"field": protocol_name}),
            );
        };
        let well_typed = if *protocol_name == "pinned" {
            value.is_boolean()
        } else {
            value.is_number()
        };
        if !well_typed {
            return bad_request(
                &request.id,
                &format!("ui.window field {protocol_name:?} has the wrong type"),
                json!({"field": protocol_name}),
            );
        }
        ui.insert((*stored_name).to_owned(), value.clone());
    }

    let mut sections = Map::new();
    sections.insert("ui".to_owned(), Value::Object(ui));
    match state.store.apply_sections(&sections) {
        Ok(_) => {
            state.rev += 1;
            refresh_config(state);
            Response::ok(&request.id, json!({}))
        }
        Err(error) => config_error_response(&request.id, error),
    }
}

/// `tool.start {tool}`: ask the supervisor to start one of `ToolId::ALL`.
///
/// `bad_request` for an unknown id (this is the ONE list — `OPEN.md` §1 item 7: v2's second whitelist
/// looked wired and did nothing); `tool_busy` for a refusal, with the arbiter's player-facing line in
/// both `message` and `detail.line`; `bad_request` with the named line when this build has no loop
/// for the tool. A deferred start answers `{}` and its waiting line was already emitted as a `log`
/// event — there is no run to attach a `state` to yet, so a state event would misreport registration.
fn tool_start(state: &mut CoreState, request: &Request) -> Response {
    let Some(name) = request.args.get("tool").and_then(Value::as_str) else {
        return bad_request(&request.id, "tool.start needs a \"tool\" id", json!({}));
    };
    let Some(tool) = ToolId::parse(name) else {
        return bad_request(
            &request.id,
            &format!("unknown tool id {name:?}"),
            json!({"tool": name}),
        );
    };
    let Some(supervisor) = state.supervisor.as_ref() else {
        return internal(&request.id, "the tool supervisor is not running");
    };

    match supervisor.start_tool(tool) {
        StartReply::Started => Response::ok(&request.id, json!({})),
        StartReply::Deferred { .. } => Response::ok(&request.id, json!({})),
        StartReply::Refused(refusal) => {
            let line = refusal.line;
            Response::err(
                &request.id,
                Error::new(
                    "tool_busy",
                    &line,
                    json!({"line": line.clone(), "tool": name}),
                ),
            )
        }
        StartReply::Unavailable { line } => Response::err(
            &request.id,
            Error::new(
                "bad_request",
                &line,
                json!({"tool": name, "line": line.clone()}),
            ),
        ),
        StartReply::Internal { line } => internal(&request.id, &line),
    }
}

/// `tool.stop {tool?}`: stop the named tool, or — with no name — whichever run the player means
/// (`PROTOCOL.md` §4: "stops whatever is running"). The response arrives while the loop is still
/// leaving; the `state`(stopping) and `tool.finished` events follow it.
fn tool_stop(state: &mut CoreState, request: &Request) -> Response {
    let tool = match request.args.get("tool") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let Some(name) = value.as_str() else {
                return bad_request(
                    &request.id,
                    "tool.stop's \"tool\" must be a string id",
                    json!({}),
                );
            };
            let Some(tool) = ToolId::parse(name) else {
                return bad_request(
                    &request.id,
                    &format!("unknown tool id {name:?}"),
                    json!({"tool": name}),
                );
            };
            Some(tool)
        }
    };
    let Some(supervisor) = state.supervisor.as_ref() else {
        return internal(&request.id, "the tool supervisor is not running");
    };

    match supervisor.stop_tool(tool) {
        StopReply::Stopping { .. } => Response::ok(&request.id, json!({})),
        // `not_running` is the protocol's non-failure for "nothing to stop" (`PROTOCOL.md` §6).
        StopReply::NotRunning => Response::err(
            &request.id,
            Error::new("not_running", "nothing is running", json!({})),
        ),
    }
}

/// The `rev` argument of `config.apply`: present, and a non-negative integer.
///
/// The refusal is boxed because a [`Response`] is a large value and this is the cold path; returning
/// it unboxed makes every `Result` on the argument path carry the whole response size.
fn revision_argument(request: &Request) -> Result<u64, Box<Response>> {
    match request.args.get("rev") {
        Some(rev) => rev.as_u64().ok_or_else(|| {
            Box::new(bad_request(
                &request.id,
                "rev must be a non-negative integer",
                json!({}),
            ))
        }),
        None => Err(Box::new(bad_request(
            &request.id,
            "rev is required",
            json!({}),
        ))),
    }
}

/// The `sections` argument shared by `config.apply` and `config.validate`: a mapping of owned block
/// names to block mappings. A foreign block is refused rather than silently dropped — the shell can
/// only ever own the blocks `CONFIG.md` names, and a save must never rewrite the player's own.
fn sections_argument(request: &Request) -> Result<Map<String, Value>, Box<Response>> {
    let Some(sections) = request.args.get("sections") else {
        return Err(Box::new(bad_request(
            &request.id,
            "sections is required",
            json!({}),
        )));
    };
    let Some(sections) = sections.as_object() else {
        return Err(Box::new(bad_request(
            &request.id,
            "sections must be an object of config blocks",
            json!({}),
        )));
    };

    for (name, value) in sections {
        if !seal_config::foreign::is_owned_block(name) {
            return Err(Box::new(bad_request(
                &request.id,
                &format!("section {name:?} is not owned by the core"),
                json!({"section": name}),
            )));
        }
        if !value.is_object() {
            return Err(Box::new(bad_request(
                &request.id,
                &format!("section {name:?} must be a mapping"),
                json!({"section": name}),
            )));
        }
    }

    Ok(sections.clone())
}

/// A stale revision: answer with the current one and the current value of every block the shell
/// asked to write, so it can reconcile before saving again (`PROTOCOL.md` §6).
fn conflict(state: &CoreState, id: &str, requested: &Map<String, Value>) -> Response {
    let current: Map<String, Value> = requested
        .keys()
        .map(|name| {
            let value = state.config.get(name).cloned().unwrap_or(Value::Null);
            (name.clone(), value)
        })
        .collect();
    Response::err(
        id,
        Error::new(
            "conflict",
            "config changed while you were editing",
            json!({"rev": state.rev, "sections": current}),
        ),
    )
}

/// The session view is re-read after a successful write so every later command sees the merged
/// result. The write already happened: a re-read failure is logged (stderr is the log channel,
/// `PROTOCOL.md` §1) rather than turned into a failed response the shell would replay against a file
/// that is already correct.
fn refresh_config(state: &mut CoreState) {
    match state.store.load() {
        Ok(config) => state.config = config,
        Err(error) => eprintln!("seal-core: config reload after a write failed: {error}"),
    }
}

/// Maps a store failure to a protocol error: a section the core does not own is a bad request, and
/// every file failure is `io` with the path in `detail` (`PROTOCOL.md` §6).
fn config_error_response(id: &str, error: ConfigError) -> Response {
    if let ConfigError::SectionNotOwned(name) = &error {
        return bad_request(
            id,
            &format!("section {name:?} is not owned by the core"),
            json!({"section": name}),
        );
    }

    let detail = match config_error_path(&error) {
        Some(path) => json!({"path": path.to_string_lossy()}),
        None => json!({}),
    };
    Response::err(id, Error::new("io", &error.to_string(), detail))
}

fn config_error_path(error: &ConfigError) -> Option<&Path> {
    match error {
        ConfigError::MissingFile(path) => Some(path.as_path()),
        ConfigError::Read { path, .. }
        | ConfigError::Parse { path, .. }
        | ConfigError::Serialize { path, .. }
        | ConfigError::Write { path, .. } => Some(path.as_path()),
        ConfigError::SectionNotOwned(_) => None,
    }
}

fn bad_request(id: &str, message: &str, detail: Value) -> Response {
    Response::err(id, Error::new("bad_request", message, detail))
}

/// A bug in the core, not a bad request: the shell shows it and logs the detail (`PROTOCOL.md` §6).
fn internal(id: &str, message: &str) -> Response {
    Response::err(id, Error::new("internal", message, json!({})))
}

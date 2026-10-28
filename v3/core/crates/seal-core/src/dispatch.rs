//! The request dispatcher: one place where a [`Request`] becomes a [`Response`].
//!
//! It is deliberately separate from the stdio loop so the handshake and the command surface can
//! be tested without spawning a process (`PROTOCOL.md` 禮1 and 禮4).
//!
//! The config commands are the shell's only writer-side surface, and their contract is
//! `PROTOCOL.md` 禮禮4 and 7 plus `PLAN-V3-CORE.md` 禮4.1: the core is the only merger, a save sends
//! **only the blocks the tab owns**, a stale revision is refused rather than replayed, and the rev
//! is a session counter the shell echoes back. Every save also runs the domain authoring rules
//! ([`crate::validate`]) before it writes ??no widget keeps a rule. The file semantics themselves
//! live in [`seal_config::ConfigStore`] ??this module only speaks protocol to them.
//!
//! The tool commands ([`tool_start`], [`tool_stop`]) are thin: they validate the id against
//! `ToolId::ALL` (the ONE accept-and-dispatch list, `OPEN.md` 禮1 item 7) and translate the
//! supervisor's typed answer into the protocol's `tool_busy`/`not_running`/`bad_request` and events.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use base64::Engine as _;
use seal_config::{ConfigError, ConfigStore};
use seal_domain::config::AttributesConfig;
use seal_proto::{Error, PROTOCOL_VERSION, Request, Response};
use seal_tools::tool::ToolId;
use serde_json::{Map, Value, json};

use crate::board;
use crate::startup::{self, StartupError};
use crate::supervisor::{self, StartReply, StopReply};
use crate::win;

/// The protocol's `ui.window` fields, mapped to the names a v2 `local.yaml` already uses.
///
/// `height` is the window height to restore when the config region is expanded ??`expanded_height`
/// in v2's `LocalUi` and in the domain model ??so it is stored under that name rather than putting a
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
/// the core wrote in the meantime (`PROTOCOL.md` 禮7, `PLAN-V3-CORE.md` 禮4.1).
pub struct CoreState {
    pub root: std::path::PathBuf,
    /// The merged config as the PRODUCTION REGISTRY reads it. The dispatcher publishes after every
    /// command, so a Start sees the same config the shell does.
    pub shared_config: crate::registry::SharedConfig,
    pub store: ConfigStore,
    pub rev: u64,
    /// The session revision as a shared counter: tool workers can bump it for a mid-run write
    /// (pet run state) without waiting for the next command, and the dispatcher re-reads it at every
    /// request boundary. `rev` is that value's dispatcher-local snapshot.
    pub revision: Arc<AtomicU64>,
    /// Memory-only firmware level cache: `board.details` reports it, `board.probe` and the pet
    /// session's opening `V` detection refresh it. `None` means "not checked yet — normal"; no
    /// command queries `V` just to read the details.
    pub firmware: Arc<RwLock<Option<u32>>>,
    pub config: Value,
    pub handshaken: bool,
    pub stopping: bool,
    /// The tool layer, attached by the binary. `None` in the config-only dispatcher tests, and in a
    /// future core that has no tools yet ??a tool command then says so rather than pretending.
    pub supervisor: Option<supervisor::Supervisor>,
    /// The active game client, when one resolved (`game.variant`), else `None`.
    pub variant: Option<String>,
    /// The attribute dictionary file the active client uses, e.g. `attributes.us.yaml`.
    pub dictionary: String,
    /// The loaded dictionary. Boot-time and every `config.reload` reload it, so a variant switch is
    /// never half-applied.
    pub attributes: AttributesConfig,
    /// The last four captured frames by id (`PROTOCOL.md` 禮8), so `calibration.save` can reference a
    /// frame the calibrator just took instead of re-sending megabytes.
    pub frames: win::FrameCache,
    /// The lazily-loaded OCR pipeline (`ocr.scan`): loading costs ~330 ms, so the first scan
    /// builds it and later scans reuse it. `None` until then, and in dispatcher tests.
    pub ocr: Option<seal_ocr::pipeline::OcrPipeline>,
    /// The per-run FILE logs (`logs.rs`): opened at boot from the config root. Disabled in
    /// `new()` (the config-only dispatcher tests write no files); the binary uses `load()`.
    pub logs: std::sync::Arc<crate::logs::Logs>,
    /// Where `config.changed` events go, when the binary attached the writer's bus. Dispatcher tests
    /// leave it `None` and inspect [`Self::take_queued_events`] instead.
    pub events: Option<crate::events::EventBus>,
    /// Events produced during one dispatch, sent AFTER its response line. A shell must never see a
    /// `config.changed` caused by its own `config.apply` BEFORE the answer to that apply.
    queued_events: Vec<seal_proto::Event>,
}

impl CoreState {
    /// An empty state, for a dispatcher test that needs no files on disk. The binary uses
    /// [`Self::load`] instead, which is the only constructor that reads the config.
    pub fn new(root: std::path::PathBuf) -> Self {
        let store = ConfigStore::new(root.join("config"));
        Self {
            root,
            store,
            shared_config: Default::default(),
            rev: 0,
            revision: Arc::new(AtomicU64::new(0)),
            firmware: Arc::new(RwLock::new(None)),
            config: json!({}),
            handshaken: false,
            stopping: false,
            supervisor: None,
            variant: None,
            dictionary: startup::DEFAULT_DICTIONARY.to_owned(),
            attributes: AttributesConfig::default(),
            frames: win::FrameCache::default(),
            ocr: None,
            logs: std::sync::Arc::new(crate::logs::Logs::disabled()),
            events: None,
            queued_events: Vec::new(),
        }
    }

    /// The binary's startup: bind to `root` and read the merged config through the store.
    ///
    /// Failing here is fatal rather than a degraded start ??v2 fails loudly for the same reason: a
    /// core with no config cannot run a tool, and a silent empty config would look exactly like a
    /// game that changed.
    pub fn load(root: std::path::PathBuf) -> Result<Self, StartupError> {
        let store = ConfigStore::new(root.join("config"));
        let (config, active, attributes) = load_parts(&store)?;
        let logs = std::sync::Arc::new(crate::logs::Logs::new(&root));
        Ok(Self {
            root,
            store,
            shared_config: Default::default(),
            rev: 0,
            revision: Arc::new(AtomicU64::new(0)),
            firmware: Arc::new(RwLock::new(None)),
            config,
            handshaken: false,
            stopping: false,
            supervisor: None,
            variant: active.variant,
            dictionary: active.dictionary,
            attributes,
            frames: win::FrameCache::default(),
            ocr: None,
            logs,
            events: None,
            queued_events: Vec::new(),
        })
    }

    /// Re-reads everything a `config.reload` replaces: the merged config, the validator rules, and the
    /// active variant's dictionary. Nothing is committed unless ALL of it succeeds ??the v2 rule
    /// behind `SwitchVariant` ("the variant is resolved first, its dictionary is LOADED second, and
    /// the config is only left switched once that load succeeded"; `LauncherService.cs:723-747`):
    /// a config pointing at a missing dictionary is one that will not start at all, so a failed reload
    /// must leave the previous, working state in place rather than half-switch it.
    pub fn reload(&mut self) -> Result<(), StartupError> {
        let (config, active, attributes) = load_parts(&self.store)?;
        self.config = config;
        self.variant = active.variant;
        self.dictionary = active.dictionary;
        self.attributes = attributes;
        Ok(())
    }

    /// Gives the dispatcher its tool layer. Only the binary calls this; the supervisor needs the
    /// writer's [`crate::events::EventBus`], so it cannot be built inside a config-only state.
    pub fn attach_supervisor(&mut self, supervisor: supervisor::Supervisor) {
        self.supervisor = Some(supervisor);
    }

    /// Attach the writer's event bus. The binary does this before the stdio loop starts.
    pub fn attach_events(&mut self, events: crate::events::EventBus) {
        self.events = Some(events);
    }

    /// Queue a `config.changed` for `block` at the CURRENT revision, with the block's effective values
    /// from the merged config. Callers queue AFTER they have updated `self.config` and `self.rev`.
    pub(crate) fn queue_config_changed(&mut self, block: &str) {
        let values = self.config.get(block).cloned().unwrap_or(Value::Null);
        self.queued_events
            .push(crate::events::config_changed_event(self.rev, block, values));
    }

    /// Send the events queued during the last dispatch. The binary calls this immediately after
    /// writing the response, so the wire order is answer-then-event.
    pub fn flush_events(&mut self) {
        let Some(bus) = &self.events else {
            self.queued_events.clear();
            return;
        };
        for event in self.queued_events.drain(..) {
            bus.emit(event);
        }
    }

    /// Drop the event bus. The writer thread stops when the LAST sender is gone (`OPEN.md` 禮1), and the
    /// state object outlives `writer.finish()` in `main` unless it lets go first.
    pub fn detach_events(&mut self) {
        self.events = None;
    }

    /// The events queued by the last dispatch, for tests that do not run the writer thread.
    pub fn take_queued_events(&mut self) -> Vec<seal_proto::Event> {
        std::mem::take(&mut self.queued_events)
    }

    /// The shared revision handle, for the production pet run-state sink. A worker's mid-run write
    /// bumps this counter; the dispatcher re-reads it before each request so the next
    /// `config.apply`/`hello` sees the same revision the event carried.
    pub fn revision_handle(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.revision)
    }

    /// The shared memory-only firmware cache, for the production registry's pet opening phase.
    pub fn firmware_handle(&self) -> Arc<RwLock<Option<u32>>> {
        Arc::clone(&self.firmware)
    }

    /// The dispatcher-thread view of the memory-only firmware cache. `None` is "not checked yet".
    pub fn firmware(&self) -> Option<u32> {
        *self.firmware.read().expect("not poisoned")
    }

    /// Record a firmware level detected by the dispatcher (`board.probe`).
    pub fn set_firmware(&self, level: u32) {
        *self.firmware.write().expect("not poisoned") = Some(level);
    }

    /// One revision for a write this dispatcher performed; also publishes it to the shared counter so
    /// a later worker write cannot move the session backwards.
    pub(crate) fn bump_revision(&mut self) -> u64 {
        self.rev += 1;
        self.revision.store(self.rev, Ordering::SeqCst);
        self.rev
    }

    /// Stops every run, waits for the workers and emits `core.stopping`, then joins the supervisor
    /// thread. Called by the binary after the final response is queued and before the writer is
    /// finished ??the supervisor is the other `EventBus` holder, and the writer only stops when the
    /// last one is gone.
    pub fn shutdown_tools(&mut self, reason: &str) {
        if let Some(mut supervisor) = self.supervisor.take() {
            supervisor.shutdown(reason);
        }
    }
}

/// Loads and fully validates the config half of a boot/reload: the merged config, the ported
/// `ConfigValidator` rules, and the active variant's attribute dictionary. All-or-nothing.
fn load_parts(
    store: &ConfigStore,
) -> Result<
    (
        Value,
        startup::Active,
        seal_domain::config::AttributesConfig,
    ),
    StartupError,
> {
    let config = store.load()?;
    startup::validate_config(&config)?;
    let active = startup::active(&config);
    let attributes = startup::load_dictionary(store, &active.dictionary)?;
    Ok((config, active, attributes))
}

fn ocr_scan(state: &mut CoreState, request: &Request) -> Response {
    let Some(mode) = request
        .args
        .get("mode")
        .and_then(Value::as_str)
        .and_then(seal_ocr::modes::Mode::parse)
    else {
        return bad_request(
            &request.id,
            "ocr.scan needs mode: lines|count|attributes|grade",
            json!({"modes": ["lines", "count", "attributes", "grade"]}),
        );
    };
    let region = match win::parse_region(&request.args) {
        Ok(region) => region,
        Err(error) => return win_error_response(&request.id, error),
    };
    let frame = if let Some(frame_id) = request.args.get("frame_id").and_then(Value::as_str) {
        match state.frames.get(frame_id) {
            Some(frame) => frame.clone(),
            None => {
                return bad_request(
                    &request.id,
                    &format!("unknown frame_id {frame_id:?}"),
                    json!({}),
                );
            }
        }
    } else if request.args.get("region").is_some() {
        let title = match win::configured_title(&state.config) {
            Ok(title) => title,
            Err(error) => return win_error_response(&request.id, error),
        };
        match win::capture(&title, region) {
            Ok((frame, _display)) => frame,
            Err(error) => return win_error_response(&request.id, error),
        }
    } else {
        return bad_request(
            &request.id,
            "ocr.scan needs a frame_id or a region",
            json!({}),
        );
    };
    let (width, height, rgba) = match frame_rgba(&frame) {
        Ok(image) => image,
        Err(message) => return internal(&request.id, &message),
    };
    let geometry = ocr_geometry(&state.config);
    if state.ocr.is_none() {
        match seal_ocr::pipeline::OcrPipeline::load_for_root(
            &state.root,
            &state.attributes,
            geometry.row_height,
        ) {
            Ok(pipeline) => state.ocr = Some(pipeline),
            Err(error) => {
                return Response::err(
                    &request.id,
                    Error::new(
                        "ocr_unavailable",
                        "the OCR models are not available",
                        json!({"reason": error.reason, "message": error.message}),
                    ),
                );
            }
        }
    }
    let pipeline = state.ocr.as_mut().expect("the pipeline is loaded");
    let scan = match pipeline.scan_rgba(width, height, &rgba) {
        Ok(scan) => scan,
        Err(error) => return internal(&request.id, &error.message),
    };
    let outcome = seal_ocr::modes::apply(mode, &scan, &state.attributes, &geometry);
    // v2 wrote one `ocr_log.jsonl` record per completed scan and an `ocr_errors.jsonl` record when
    // it stayed unconfirmed; a capture PNG is saved only while `tuner.save_captures` is on. The
    // scan's own PNG bytes are already in the frame, so a capture is a byte copy, not a re-encode.
    let save_captures = state
        .config
        .pointer("/tuner/save_captures")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let frame_png = if save_captures {
        frame
            .get("data")
            .and_then(Value::as_str)
            .and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok())
    } else {
        None
    };
    let result = state
        .logs
        .record_scan(mode, &outcome, frame_png.as_deref(), save_captures);
    Response::ok(&request.id, result)
}

/// The OCR geometry the tuner read uses, including the region-relative v2 `grade_area` when it is
/// present and valid. Public for the committed v2-config replay test; no production caller outside
/// the core uses it.
pub fn ocr_geometry(config: &Value) -> seal_ocr::modes::OcrGeometry {
    let band = |name: &str| -> Option<(i32, i32)> {
        let array = config.pointer(&format!("/tuner/ocr/{name}"))?.as_array()?;
        Some((
            i32::try_from(array.first()?.as_i64()?).ok()?,
            i32::try_from(array.get(1)?.as_i64()?).ok()?,
        ))
    };
    let row_height = config
        .pointer("/tuner/ocr/row_height")
        .and_then(Value::as_u64)
        .map(|value| u32::try_from(value).unwrap_or(10))
        .unwrap_or(10);
    seal_ocr::modes::OcrGeometry {
        attr_x: band("attr_x"),
        attr_y: band("attr_y"),
        grade_y: band("grade_y"),
        grade_area: grade_area(config),
        row_height,
    }
}

/// The v2 `tuner.ocr.grade_area` box, in the same region-relative space as `region` and the bands.
/// v2 cropped the captured region image with it (`DetectGradeColorScores(mat, ga)`), and its
/// `ConfigValidator` checks x2/y2 against the region size. Any missing/negative/out-of-order/outside
/// box returns `None`, which makes the reader fall back to the unchanged `grade_y` band.
fn grade_area(config: &Value) -> Option<seal_ocr::modes::GradeArea> {
    let area = config.pointer("/tuner/ocr/grade_area")?.as_object()?;
    let component = |name: &str| -> Option<i32> { i32::try_from(area.get(name)?.as_i64()?).ok() };
    let (x1, y1, x2, y2) = (
        component("x1")?,
        component("y1")?,
        component("x2")?,
        component("y2")?,
    );
    if x1 < 0 || y1 < 0 || x2 <= x1 || y2 <= y1 {
        return None;
    }
    let region = config.pointer("/tuner/ocr/region")?.as_object()?;
    let width = i32::try_from(region.get("width")?.as_i64()?).ok()?;
    let height = i32::try_from(region.get("height")?.as_i64()?).ok()?;
    if width <= 0 || height <= 0 || x2 > width || y2 > height {
        return None;
    }
    Some(seal_ocr::modes::GradeArea { x1, y1, x2, y2 })
}
pub(crate) fn frame_rgba(frame: &Value) -> Result<(usize, usize, Vec<u8>), String> {
    let data = frame
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(|| "the frame has no base64 data".to_owned())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| format!("the frame data is not base64: {error}"))?;
    let (width, height, rgba) = seal_win::pure::decode_png(&bytes)
        .map_err(|error| format!("the frame PNG could not be decoded: {error}"))?;
    Ok((width as usize, height as usize, rgba))
}
/// Answers one request. Never panics: a bad request is a refusal the client can show.
pub fn handle(state: &mut CoreState, request: Request) -> Response {
    // A mid-run pet write can bump the shared revision between requests; every boundary starts from
    // that value so `hello`, conflict checks and echoes never report a pre-write revision.
    state.rev = state.revision.load(Ordering::SeqCst);
    if request.cmd == "hello" {
        return hello(state, request);
    }

    if !state.handshaken {
        return Response::err(
            &request.id,
            Error::new("not_ready", "hello must be the first request", json!({})),
        );
    }

    let response = match request.cmd.as_str() {
        "shutdown" => {
            state.stopping = true;
            Response::ok(&request.id, json!({}))
        }
        "config.validate" => config_validate(state, &request),
        "config.apply" => config_apply(state, &request),
        "config.reload" => config_reload(state, &request),
        "ui.window" => ui_window(state, &request),
        "window.find" => window_find(state, &request),
        "window.measure" => window_measure(state, &request),
        "capture" => capture_command(state, &request),
        "ocr.scan" => ocr_scan(state, &request),
        "keys.parse" => keys_parse(state, &request),
        "keys.names" => keys_names(state, &request),
        "attributes.get" => attributes_get(state, &request),
        "logs.cleanup_captures" => logs_cleanup_captures(state, &request),
        "calibration.save" => crate::calibration::save(state, &request),
        "calibration.fetch" => crate::calibration::fetch(state, &request),
        "pointer.place" => pointer_command(state, &request, false),
        "pointer.click" => pointer_command(state, &request, true),
        "board.probe" => board_probe(state, &request),
        "board.details" => board_details(state, &request),
        "board.test" => board_test(state, &request),
        "tool.start" => tool_start(state, &request),
        "tool.stop" => tool_stop(state, &request),
        other => bad_request(
            &request.id,
            &format!("unknown command {other:?}"),
            json!({}),
        ),
    };
    // `internal` is a core bug, not a refusal the player can correct: v2's `error.log` held
    // exceptions, and filling it with correctable refusals would bury them.
    if let Some(error) = &response.error
        && error.code == "internal"
    {
        state
            .logs
            .error_line(&format!("{}: {}", request.cmd, error.message));
    }
    state.publish_config();
    response
}

impl CoreState {
    /// Publish the merged config for the production registry. Cheap (a clone of one JSON tree) and
    /// called once per command, so registry and shell can never disagree about which preset is active.
    pub fn publish_config(&self) {
        if let Ok(mut shared) = self.shared_config.write() {
            *shared = self.config.clone();
        }
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
/// answer is one player-facing line per array entry ??the same lines the v2 tab showed ??and
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
/// revision, answer with the merged sections (`PROTOCOL.md` 禮4 and 禮7).
///
/// The SAME authoring checks `config.validate` runs go first, so a Save cannot bypass the gate the
/// shell showed: on any error the write is refused as `bad_request` with the lines in
/// `detail.errors`, and nothing is written or consumed. (Warnings have no channel in this response ??/// the protocol fixes its shape at `{rev, sections}` ??so they are reported by `config.validate`
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

    // The portable half is the one section that is not written as a block: it goes through the
    // settings projection below. Anything else malformed there would reach the projection unfiltered.
    if let Some(settings) = sections.get("settings")
        && !settings.is_object()
    {
        return bad_request(
            &request.id,
            "the settings section must be an object of config blocks",
            json!({"section": "settings"}),
        );
    }

    // The effective config is the merge the shell will see AND the merge the rules just ran on: the
    // echo and the settings projection both come from it, so apply, validate and the answer cannot
    // describe different configs (`PROTOCOL.md` 禮7).
    let effective = crate::validate::effective_config(&state.config, &sections);

    // v2's SwitchVariant order, the only one that cannot half-switch a variant: resolve ??validate ??    // LOAD the dictionary the effective config names ??write. A missing dictionary refuses here,
    // before the first byte is written and before a rev is consumed.
    let active = startup::active(&effective);
    let attributes = match startup::load_dictionary(&state.store, &active.dictionary) {
        Ok(attributes) => attributes,
        Err(error) => return startup_error_response(&request.id, error),
    };

    // ONE read-modify-write: the owned blocks and the portable projection land together, so a failure
    // at any point writes NOTHING and consumes no rev (`PROTOCOL.md` 禮4's promise). Two separate
    // writes could and did leave the file half-saved. `settings` is filtered out of the owned map and
    // written only through the projection.
    let owned: Map<String, Value> = sections
        .iter()
        .filter(|(name, _)| name.as_str() != "settings")
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    if (!owned.is_empty() || sections.contains_key("settings"))
        && let Err(error) = state.store.apply_sections_with_settings(&owned, &effective)
    {
        return config_error_response(&request.id, error);
    }

    // Commit: the files are now the truth, so re-read the merged view; the dictionary was loaded
    // BEFORE the write and lands with the variant, one rev covering both halves.
    let config = match state.store.load() {
        Ok(config) => config,
        Err(error) => return config_error_response(&request.id, error),
    };
    state.config = config;
    state.variant = active.variant;
    state.dictionary = active.dictionary;
    state.attributes = attributes;
    state.bump_revision();
    // One event per block the save wrote, in request order. The portable half writes its inner
    // blocks, so `settings` fans out to the block names it carries.
    let mut changed: Vec<String> = Vec::new();
    for (name, requested) in &sections {
        if name == "settings" {
            if let Some(portable) = requested.as_object() {
                changed.extend(portable.keys().cloned());
            }
        } else {
            changed.push(name.clone());
        }
    }
    for block in changed {
        state.queue_config_changed(&block);
    }

    Response::ok(
        &request.id,
        json!({"rev": state.rev, "sections": echo_sections(&state.config.clone(), &sections)}),
    )
}

/// The apply result echoes the EFFECTIVE sections for the names in the request, in the request's
/// structure: owned blocks at top level, the portable half nested under `settings` (`PROTOCOL.md`
/// 禮4). "Effective" means what the merge resolves the name to, not the overlay the shell sent ??a
/// shell cannot map a merged projection it does not own, and it should not have to guess which fields
/// the core's rules added.
fn echo_sections(effective: &Value, sections: &Map<String, Value>) -> Map<String, Value> {
    let mut echo = Map::new();
    for (name, requested) in sections {
        if name == "settings" {
            let mut portable = Map::new();
            if let Some(requested) = requested.as_object() {
                for inner in requested.keys() {
                    portable.insert(
                        inner.clone(),
                        effective.get(inner).cloned().unwrap_or(Value::Null),
                    );
                }
            }
            echo.insert(name.clone(), Value::Object(portable));
        } else {
            echo.insert(
                name.clone(),
                effective
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| requested.clone()),
            );
        }
    }
    echo
}

/// `config.reload`: re-read the three layers through the store and answer the whole config. A reload
/// is a change the shell must see, so it bumps the revision even though no file was written.
fn config_reload(state: &mut CoreState, request: &Request) -> Response {
    match state.reload() {
        Ok(()) => {
            state.bump_revision();
            Response::ok(
                &request.id,
                json!({"rev": state.rev, "config": state.config}),
            )
        }
        Err(error) => reload_error_response(&request.id, error),
    }
}

/// A reload failure: an INVALID on-disk config is its own protocol answer (`config_invalid`,
/// `PROTOCOL.md` 禮6/禮4) carrying the ported validator's lines, while file-level failures keep the
/// `io`/config mapping. Boot-time invalidity stays on stderr (禮1); this is the reload case only.
fn reload_error_response(id: &str, error: StartupError) -> Response {
    if let Some(errors) = error.invalid_errors() {
        return Response::err(
            id,
            Error::new(
                "config_invalid",
                "the config on disk is invalid",
                json!({"errors": errors}),
            ),
        );
    }
    startup_error_response(id, error)
}

/// `ui.window {left, top, width, height, pinned}`: the one path window geometry and pin state take
/// into `local.yaml`. The `ui:` block is core-owned ??nobody edits it in a tab ??so the shell sends
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
            state.bump_revision();
            refresh_config(state);
            state.queue_config_changed("ui");
            Response::ok(&request.id, json!({}))
        }
        Err(error) => config_error_response(&request.id, error),
    }
}

/// `tool.start {tool}`: ask the supervisor to start one of `ToolId::ALL`.
///
/// `bad_request` for an unknown id (this is the ONE list ??`OPEN.md` 禮1 item 7: v2's second whitelist
/// looked wired and did nothing); `tool_busy` for a refusal, with the arbiter's player-facing line in
/// both `message` and `detail.line`; `bad_request` with the named line when this build has no loop
/// for the tool. A deferred start answers `{}` and its waiting line was already emitted as a `log`
/// event ??there is no run to attach a `state` to yet, so a state event would misreport registration.
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
    // Run arguments are validated HERE, before the supervisor: a shell bug must not silently buy a
    // different preset by passing the field to the wrong tool, and the refusals stay hardware-free.
    let args = match start_arguments(tool, &request.args) {
        Ok(args) => args,
        Err((message, field)) => {
            return bad_request(&request.id, &message, json!({"field": field}));
        }
    };
    let Some(supervisor) = state.supervisor.as_ref() else {
        return internal(&request.id, "the tool supervisor is not running");
    };

    match supervisor.start_tool(tool, &args) {
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

/// Validate `tool.start`'s optional run arguments (`PROTOCOL.md` 禮4 note / 禮12.6) and return the
/// object forwarded to the supervisor's `create`.
///
/// `buy` takes `preset` (a string) and `count` (an integer, at least 1); `sell` takes `slots` (a
/// non-empty array of bag indexes 0..63). A run argument on any other tool is refused by name; a
/// `count` below 1 and an empty/invalid `slots` are refused rather than clamped or defaulted (the
/// protocol says an explicit value the player gave is not silently changed). The error is
/// `(message, field)` so the dispatcher can put the field in `detail`.
fn start_arguments(tool: ToolId, args: &Value) -> Result<Value, (String, String)> {
    let Some(object) = args.as_object() else {
        return Err((
            "tool.start arguments must be an object".to_owned(),
            String::new(),
        ));
    };
    for key in object.keys() {
        if !matches!(key.as_str(), "tool" | "preset" | "count" | "slots") {
            return Err((
                format!("tool.start doesn't take a {key:?} field"),
                key.clone(),
            ));
        }
    }

    match tool {
        ToolId::Buy => {
            if object.contains_key("slots") {
                return Err((
                    "tool.start's \"slots\" is only accepted by \"sell\"".to_owned(),
                    "slots".to_owned(),
                ));
            }
            let mut forwarded = serde_json::Map::new();
            if let Some(value) = object.get("preset") {
                let Some(preset) = value.as_str() else {
                    return Err((
                        "tool.start's \"preset\" must be a string".to_owned(),
                        "preset".to_owned(),
                    ));
                };
                forwarded.insert("preset".to_owned(), Value::String(preset.to_owned()));
            }
            if let Some(value) = object.get("count") {
                let Some(count) = value.as_i64().and_then(|count| i32::try_from(count).ok()) else {
                    return Err((
                        "tool.start's \"count\" must be an integer".to_owned(),
                        "count".to_owned(),
                    ));
                };
                if count < 1 {
                    return Err((
                        "tool.start's \"count\" must be at least 1".to_owned(),
                        "count".to_owned(),
                    ));
                }
                forwarded.insert("count".to_owned(), Value::from(count));
            }
            Ok(Value::Object(forwarded))
        }
        ToolId::Sell => {
            for field in ["preset", "count"] {
                if object.contains_key(field) {
                    return Err((
                        format!("tool.start's {field:?} is only accepted by \"buy\""),
                        field.to_owned(),
                    ));
                }
            }
            let mut forwarded = serde_json::Map::new();
            if let Some(value) = object.get("slots") {
                let slots = slot_selection(value)?;
                forwarded.insert("slots".to_owned(), Value::Array(slots));
            }
            Ok(Value::Object(forwarded))
        }
        _ => {
            for field in ["preset", "count"] {
                if object.contains_key(field) {
                    return Err((
                        format!("tool.start's {field:?} is only accepted by \"buy\""),
                        field.to_owned(),
                    ));
                }
            }
            if object.contains_key("slots") {
                return Err((
                    "tool.start's \"slots\" is only accepted by \"sell\"".to_owned(),
                    "slots".to_owned(),
                ));
            }
            Ok(args.clone())
        }
    }
}

/// The `sell` run's bag selection: a non-empty array of integers 0..63. Order and duplicates are
/// irrelevant; the domain's sell plan dedupes, sorts highest-first and applies `sell_cap`.
fn slot_selection(value: &Value) -> Result<Vec<Value>, (String, String)> {
    let Some(values) = value.as_array() else {
        return Err((
            "tool.start's \"slots\" must be an array of integers 0..63".to_owned(),
            "slots".to_owned(),
        ));
    };
    if values.is_empty() {
        return Err((
            "tool.start's \"slots\" must contain at least one bag slot".to_owned(),
            "slots".to_owned(),
        ));
    }
    let mut selection = Vec::with_capacity(values.len());
    for value in values {
        let Some(slot) = value.as_i64().and_then(|slot| i32::try_from(slot).ok()) else {
            return Err((
                "tool.start's \"slots\" must contain only integers 0..63".to_owned(),
                "slots".to_owned(),
            ));
        };
        if !(0..=63).contains(&slot) {
            return Err((
                "tool.start's \"slots\" must contain only integers 0..63".to_owned(),
                "slots".to_owned(),
            ));
        }
        selection.push(Value::from(slot));
    }
    Ok(selection)
}
/// `tool.stop {tool?}`: stop the named tool, or ??with no name ??whichever run the player means
/// (`PROTOCOL.md` 禮4: "stops whatever is running"). The response arrives while the loop is still
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
        // `not_running` is the protocol's non-failure for "nothing to stop" (`PROTOCOL.md` 禮6).
        StopReply::NotRunning => Response::err(
            &request.id,
            Error::new("not_running", "nothing is running", json!({})),
        ),
    }
}

/// `logs.cleanup_captures`: Tuner's "Clean up capture images". The result carries only the
/// successful count and the ROOT-RELATIVE directory; per-file failures are logged through the normal
/// log path and do not fail the command (`PROTOCOL.md` §2's logging philosophy). The helper performs
/// the actual policy — png-only, direct children, no recursion/reparse following.
fn logs_cleanup_captures(state: &CoreState, request: &Request) -> Response {
    let (deleted, failures) = state.logs.cleanup_captures();
    if failures > 0 {
        state.logs.error_line(&format!(
            "cleanup_captures: {failures} capture file(s) could not be deleted"
        ));
    }
    Response::ok(
        &request.id,
        json!({"deleted": deleted, "dir": "logs/captures"}),
    )
}

/// `pointer.place` / `pointer.click`: a calibration pointer action at a client-relative PHYSICAL
/// point. The per-command input lease is a snapshot of the arbiter's game owner: while a foreground
/// tool is running/stopping (or the resident feeder holds the game) the action is refused with the
/// same `tool_busy` line a start gets. Session-wide leases and queueing are the later calibration
/// session increments; this does not fake one. The action itself is the same closed-loop placement
/// every tool uses ??never `SetCursorPos`.
fn pointer_command(state: &mut CoreState, request: &Request, click: bool) -> Response {
    let point = match crate::pointer::point_arguments(&request.cmd, &request.args) {
        Ok(point) => point,
        Err((message, field)) => {
            return bad_request(&request.id, &message, json!({"field": field}));
        }
    };
    if let Some(supervisor) = state.supervisor.as_ref() {
        match supervisor.input_lease() {
            Ok(Some(line)) => {
                return Response::err(
                    &request.id,
                    Error::new(
                        "tool_busy",
                        &line,
                        json!({"line": line, "command": request.cmd}),
                    ),
                );
            }
            Ok(None) => {}
            Err(line) => return internal(&request.id, &line),
        }
    }
    match crate::pointer::system_perform(&state.config, point, click) {
        Ok(()) => Response::ok(&request.id, json!({})),
        Err(crate::pointer::PointerFailure::Window(error)) => {
            win_error_response(&request.id, error)
        }
        Err(crate::pointer::PointerFailure::Board(error)) => {
            board_error_response(&request.id, error)
        }
        Err(crate::pointer::PointerFailure::Effects(line)) => Response::err(
            &request.id,
            Error::new("board_unavailable", &line, json!({})),
        ),
    }
}

/// `board.probe`: which port, which baud, which IDs, and the firmware level from `V` (silence = level
/// 1). The board crate owns the choice and the open; this only maps its failure to the protocol.
/// `keys.parse`: the Hotkeys tab's typed NAMES to the VK codes the config stores.
///
/// The rule itself is `seal_domain::authoring`'s `parse_virtual_key` (the ported `MainWindow.ParseVk`),
/// not the shell's: v2 kept both directions inside the window and the plan's rule is that no widget
/// keeps a rule. The answer carries the codes it could parse AND one line per box it could not, so a
/// four-box save whose second box is wrong names that box instead of refusing the set anonymously.
fn keys_parse(_state: &CoreState, request: &Request) -> Response {
    let Some(names) = request.args.get("names").and_then(Value::as_object) else {
        return bad_request(
            &request.id,
            "keys.parse needs names, an object of field to typed name",
            json!({}),
        );
    };

    let mut codes = Map::new();
    let mut errors = Vec::new();
    for (field, value) in names {
        let Some(name) = value.as_str() else {
            return bad_request(
                &request.id,
                "keys.parse needs every name to be text",
                json!({"field": field}),
            );
        };
        match seal_domain::authoring::parse_virtual_key(name) {
            Ok(code) => {
                codes.insert(field.clone(), json!(code));
            }
            Err(error) => errors.push(json!({
                "field": field,
                "input": error.input,
                "line": error.message(),
            })),
        }
    }

    Response::ok(&request.id, json!({"codes": codes, "errors": errors}))
}

/// `keys.names`: the display names for the codes the config holds. The inverse rule to
/// [`keys_parse`], from `seal_domain::authoring::virtual_key_name` (v2's `VkName`), including its hex
/// fallback for a code no name covers -- a hand-edited config can carry one, and the tab must show
/// it rather than pretend.
fn keys_names(_state: &CoreState, request: &Request) -> Response {
    let Some(codes) = request.args.get("codes").and_then(Value::as_object) else {
        return bad_request(
            &request.id,
            "keys.names needs codes, an object of field to integer VK code",
            json!({}),
        );
    };

    let mut names = Map::new();
    for (field, value) in codes {
        let Some(code) = value.as_i64().and_then(|number| u32::try_from(number).ok()) else {
            return bad_request(
                &request.id,
                "keys.names needs every code to be a non-negative integer",
                json!({"field": field}),
            );
        };
        names.insert(
            field.clone(),
            json!(seal_domain::authoring::virtual_key_name(code)),
        );
    }

    Response::ok(&request.id, json!({"names": names}))
}

/// `attributes.get`: the loaded dictionary for the active client, for the Attributes surface.
///
/// The whole dictionary is served because the tab shows it verbatim, it is small (a few KB), and
/// the core already holds it ??boot and every variant switch load it, so this reads no disk. `variant`
/// is null when no client resolved: the shell shows that rather than inventing one.
fn attributes_get(state: &CoreState, request: &Request) -> Response {
    Response::ok(
        &request.id,
        json!({
            "variant": state.variant,
            "dictionary": state.dictionary,
            "attributes": state.attributes,
        }),
    )
}

fn board_probe(state: &CoreState, request: &Request) -> Response {
    match board::probe(&state.config) {
        Ok(result) => {
            if let Some(level) = result.get("level").and_then(Value::as_u64)
                && let Ok(level) = u32::try_from(level)
            {
                state.set_firmware(level);
            }
            Response::ok(&request.id, result)
        }
        Err(error) => board_error_response(&request.id, error),
    }
}

/// `board.details`: READ-ONLY. It enumerates ports (the one OS call) and reads the memory-only
/// firmware cache; it NEVER opens or reconfigures a port, so it stays usable while a tool owns the
/// board and a missing board is a normal `choice.state`, not an error.
fn board_details(state: &CoreState, request: &Request) -> Response {
    let settings = board::Settings::from_config(&state.config);
    let ports = seal_board::enumerate::available_ports();
    let firmware = state.firmware();
    Response::ok(&request.id, board::details(&settings, &ports, firmware))
}

/// `board.test`: send the calibrator's test command. A command is sent only after the port opens, so
/// a failure means nothing reached a board.
fn board_test(state: &CoreState, request: &Request) -> Response {
    match board::test(&state.config, &request.args) {
        Ok(result) => Response::ok(&request.id, result),
        Err(error) => board_error_response(&request.id, error),
    }
}

/// A board failure: no usable port is `board_unavailable` (the shell shows the line), a malformed
/// request is `bad_request` before any port is touched.
fn board_error_response(id: &str, error: board::BoardFailure) -> Response {
    match error {
        board::BoardFailure::Unavailable(line) => {
            Response::err(id, Error::new("board_unavailable", &line, json!({})))
        }
        board::BoardFailure::BadRequest(message) => bad_request(id, &message, json!({})),
    }
}

/// `window.find`: a not-found window is a RESULT (`found: false`), not an error ??the calibrator
/// shows found/not-found long before it measures (`PROTOCOL.md` 禮4).
fn window_find(state: &CoreState, request: &Request) -> Response {
    let title = match win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return win_error_response(&request.id, error),
    };
    match win::find(&title) {
        Ok(result) => Response::ok(&request.id, result),
        Err(error) => win_error_response(&request.id, error),
    }
}

/// `window.measure`: the `DisplayInfo` the shell maps frames with. A missing window is
/// `window_not_found`; the game does not have to be in front to measure it.
fn window_measure(state: &CoreState, request: &Request) -> Response {
    let title = match win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return win_error_response(&request.id, error),
    };
    match win::measure(&title) {
        Ok(result) => Response::ok(&request.id, result),
        Err(error) => win_error_response(&request.id, error),
    }
}

/// `capture`: a PNG frame plus the fresh `DisplayInfo`. The frame is cached by id (禮8) before the
/// response is built so the shell can reference it immediately.
fn capture_command(state: &mut CoreState, request: &Request) -> Response {
    let region = match win::parse_region(&request.args) {
        Ok(region) => region,
        Err(error) => return win_error_response(&request.id, error),
    };
    let title = match win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return win_error_response(&request.id, error),
    };
    let (frame, display) = match win::capture(&title, region) {
        Ok(result) => result,
        Err(error) => return win_error_response(&request.id, error),
    };
    let frame_id = state.frames.insert(frame);
    let frame = state
        .frames
        .get(&frame_id)
        .cloned()
        .expect("the frame was just inserted");
    Response::ok(&request.id, json!({"frame": frame, "display": display}))
}

/// Maps a window/capture failure to the protocol's error codes (`PROTOCOL.md` 禮6). The Win32 detail
/// stays in the message so a log is debuggable; the player sees what the shell chooses to show.
fn win_error_response(id: &str, error: win::CommandError) -> Response {
    let line = error.line();
    match error {
        win::CommandError::MissingTitle => bad_request(id, &line, json!({"section": "window"})),
        win::CommandError::NotFound { title } => Response::err(
            id,
            Error::new("window_not_found", &line, json!({"title": title})),
        ),
        win::CommandError::NotForeground => {
            Response::err(id, Error::new("game_not_foreground", &line, json!({})))
        }
        win::CommandError::RegionOutside => bad_request(id, &line, json!({})),
        win::CommandError::BadRequest(_) => bad_request(id, &line, json!({})),
        win::CommandError::Internal(_) => internal(id, &line),
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
/// names to block mappings. A foreign block is refused rather than silently dropped ??the shell can
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
/// asked to write, so it can reconcile before saving again (`PROTOCOL.md` 禮6).
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
/// `PROTOCOL.md` 禮1) rather than turned into a failed response the shell would replay against a file
/// that is already correct.
fn refresh_config(state: &mut CoreState) {
    match state.store.load() {
        Ok(config) => state.config = config,
        Err(error) => eprintln!("seal-core: config reload after a write failed: {error}"),
    }
}

/// Maps a store failure to a protocol error: a section the core does not own is a bad request, and
/// every file failure is `io` with the path in `detail` (`PROTOCOL.md` 禮6).
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

/// A boot/reload failure as a protocol answer. A file-level failure keeps the existing `io`/config
/// mapping; an INVALID config has no protocol code of its own yet (`PROTOCOL.md` 禮6 has no
/// `config_invalid`), so the v2 `ConfigException` text is carried under `io` ??reported to the Lead
/// as a protocol proposal rather than silently inventing a code.
fn startup_error_response(id: &str, error: StartupError) -> Response {
    match error {
        StartupError::Config(error) => config_error_response(id, error),
        other => Response::err(id, Error::new("io", &other.message(), json!({}))),
    }
}
fn bad_request(id: &str, message: &str, detail: Value) -> Response {
    Response::err(id, Error::new("bad_request", message, detail))
}

/// A bug in the core, not a bad request: the shell shows it and logs the detail (`PROTOCOL.md` 禮6).
fn internal(id: &str, message: &str) -> Response {
    Response::err(id, Error::new("internal", message, json!({})))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, mpsc};

    use seal_domain::decision::{Decision, Key, StopReason};
    use seal_domain::geometry::Point;
    use seal_tools::game::GamePort;
    use seal_tools::runner::{EffectError, Effects};
    use seal_tools::tool::ToolId;

    use crate::events::event_channel;
    use crate::logs::Logs;
    use crate::registry::{Session, ToolRegistry, Unavailable};
    use crate::supervisor::Supervisor;

    use super::*;
    use serde_json::json;

    /// A tool that blocks in its first decision until the test releases it. No hardware and no
    /// window: it exists only to put a foreground run in the arbiter for the lease assertion.
    struct BlockingTool {
        release: Mutex<mpsc::Receiver<()>>,
    }

    impl seal_tools::runner::Tool for BlockingTool {
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

    struct BlockingRegistry {
        release: Mutex<Option<mpsc::Receiver<()>>>,
    }

    impl ToolRegistry for BlockingRegistry {
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
                tool: Box::new(BlockingTool {
                    release: Mutex::new(release),
                }),
                effects: Box::new(NoEffects),
            })
        }
    }

    #[test]
    fn a_pointer_action_while_a_foreground_tool_runs_is_tool_busy_naming_the_holder() {
        let (release, release_rx) = mpsc::channel();
        let registry = Arc::new(BlockingRegistry {
            release: Mutex::new(Some(release_rx)),
        });
        let (bus, _events) = event_channel();
        let supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));
        let mut state = CoreState::new(std::path::PathBuf::from(r"C:\repo"));
        state.attach_supervisor(supervisor);
        let _ = handle(
            &mut state,
            Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"})),
        );
        assert_eq!(
            state
                .supervisor
                .as_ref()
                .expect("attached")
                .start_tool(ToolId::Tuner, &Value::Null),
            StartReply::Started
        );

        let response = handle(
            &mut state,
            Request::new("pointer-lease", "pointer.place", json!({"x": 10, "y": 20})),
        );
        let error = response
            .error
            .expect("the pointer action is refused while busy");
        assert_eq!(error.code, "tool_busy");
        assert_eq!(
            error.message,
            "\"tuner\" is using the game right now — stop it, then try again."
        );
        assert_eq!(error.detail["command"], "pointer.place");

        let _ = release.send(());
        state.shutdown_tools("test");
    }

    #[test]
    fn sell_slots_are_forwarded_only_after_validation() {
        let forwarded = start_arguments(
            ToolId::Sell,
            &json!({"tool": "sell", "slots": [3, 7, 12, 3]}),
        )
        .expect("valid bag indexes");
        assert_eq!(
            forwarded,
            json!({"slots": [3, 7, 12, 3]}),
            "order and duplicates are the domain's to normalise, not this layer's"
        );

        assert_eq!(
            start_arguments(ToolId::Sell, &json!({"tool": "sell"})).expect("no slots"),
            json!({}),
            "absent slots leave the run empty; the saved config field is never consulted"
        );

        for (case, slots) in [
            ("non-array", json!("3")),
            ("non-integer", json!([1, "2"])),
            ("out of range", json!([64])),
            ("empty", json!([])),
        ] {
            let error = start_arguments(ToolId::Sell, &json!({"tool": "sell", "slots": slots}))
                .expect_err(case);
            assert_eq!(error.1, "slots", "{case}");
            assert!(error.0.contains("slots"), "{case}: {}", error.0);
        }
    }

    #[test]
    fn a_stale_shell_revision_conflicts_after_a_mid_run_revision_bump() {
        let mut state = CoreState::new(std::path::PathBuf::from(r"C:\\repo"));
        let _ = handle(
            &mut state,
            Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"})),
        );
        state.revision.store(4, std::sync::atomic::Ordering::SeqCst);

        let response = handle(
            &mut state,
            Request::new("stale", "config.apply", json!({"rev": 3, "sections": {}})),
        );

        let error = response.error.expect("the stale write is refused");
        assert_eq!(error.code, "conflict");
        assert_eq!(error.detail["rev"], 4);
    }

    #[test]
    fn board_details_reads_the_memory_only_firmware_cache_without_opening() {
        let mut state = CoreState::new(std::path::PathBuf::from(r"C:\\repo"));
        let _ = handle(
            &mut state,
            Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"})),
        );
        *state.firmware.write().expect("not poisoned") = Some(2);

        let response = handle(
            &mut state,
            Request::new("details", "board.details", json!({})),
        );

        assert!(
            response.ok,
            "details is always a normal answer: {:?}",
            response.error
        );
        let result = response.result.expect("a result");
        assert_eq!(result["firmware"], json!({"level": 2, "checked": true}));
        assert!(result["ports"].is_array());
    }
}

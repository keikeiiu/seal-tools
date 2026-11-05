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
//! The tool commands (`tool_start`, `tool_stop`) are thin: they validate the id against
//! `ToolId::ALL` (the ONE accept-and-dispatch list, `OPEN.md` 禮1 item 7) and translate the
//! supervisor's typed answer into the protocol's `tool_busy`/`not_running`/`bad_request` and events.

use std::panic::{AssertUnwindSafe, catch_unwind};
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
use crate::pet_state::PetStateSink;
use crate::pointer;
use crate::startup::{self, StartupError};
use crate::supervisor::{self, StartReply, StopReply};
use crate::win;
use seal_domain::tools::shop::BagGrid;

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

/// What the cached OCR pipeline was built from (CORE-F7). [`seal_ocr::pipeline::OcrPipeline::load`]
/// bakes in the active dictionary's `text_fixes` (the cleaner's tables) and `tuner.ocr.row_height`
/// (the line bucketing), so both are part of its identity: a variant switch or a tuner calibration
/// must reload the pipeline instead of serving the old dictionary until the core restarts.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrIdentity {
    dictionary: String,
    row_height: u32,
    attributes: AttributesConfig,
}

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
    /// The A1 pet run-state writer, attached by the binary. `pet.set_boarding` goes through this
    /// exact field-precise path; config-only tests leave it absent.
    pub pet_state: Option<Arc<dyn PetStateSink>>,
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
    /// The parts of the config the cached pipeline was BUILT FROM: the active dictionary (its
    /// `text_fixes` feed the cleaner) and the row height. A config change that alters either must
    /// rebuild the pipeline instead of serving the old dictionary until the core restarts
    /// (CORE-F7).
    pub ocr_build: Option<OcrIdentity>,
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
    #[must_use]
    pub fn new(root: std::path::PathBuf) -> Self {
        let store = ConfigStore::new(root.join("config"));
        Self {
            root,
            store,
            shared_config: Arc::default(),
            rev: 0,
            revision: Arc::new(AtomicU64::new(0)),
            firmware: Arc::new(RwLock::new(None)),
            config: json!({}),
            handshaken: false,
            stopping: false,
            supervisor: None,
            pet_state: None,
            variant: None,
            dictionary: startup::DEFAULT_DICTIONARY.to_owned(),
            attributes: AttributesConfig::default(),
            frames: win::FrameCache::default(),
            ocr: None,
            ocr_build: None,
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
    ///
    /// # Errors
    ///
    /// Returns the startup refusal when the config or the active dictionary cannot be loaded.
    pub fn load(root: std::path::PathBuf) -> Result<Self, StartupError> {
        let store = ConfigStore::new(root.join("config"));
        let (config, active, attributes) = load_parts(&store)?;
        let logs = std::sync::Arc::new(crate::logs::Logs::new(&root));
        Ok(Self {
            root,
            store,
            shared_config: Arc::default(),
            rev: 0,
            revision: Arc::new(AtomicU64::new(0)),
            firmware: Arc::new(RwLock::new(None)),
            config,
            handshaken: false,
            stopping: false,
            supervisor: None,
            pet_state: None,
            variant: active.variant,
            dictionary: active.dictionary,
            attributes,
            frames: win::FrameCache::default(),
            ocr: None,
            ocr_build: None,
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
    ///
    /// # Errors
    ///
    /// Returns the startup refusal when the re-read config or dictionary cannot be loaded; the previous working state is kept.
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
    pub fn attach_pet_state_sink(&mut self, sink: Arc<dyn PetStateSink>) {
        self.pet_state = Some(sink);
    }

    /// Drop the writer's `EventBus` clone before `writer.finish()`, exactly like
    /// [`Self::detach_events`]. The production sink holds the bus so a worker write can emit its
    /// `config.changed`; leaving it attached after shutdown would keep the writer's sender count
    /// non-zero and hang the process after its final line.
    pub fn detach_pet_state_sink(&mut self) {
        self.pet_state = None;
    }

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
    #[must_use]
    pub fn revision_handle(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.revision)
    }

    /// The shared memory-only firmware cache, for the production registry's pet opening phase.
    #[must_use]
    pub fn firmware_handle(&self) -> Arc<RwLock<Option<u32>>> {
        Arc::clone(&self.firmware)
    }

    /// The dispatcher-thread view of the memory-only firmware cache. `None` is "not checked yet".
    #[must_use]
    pub fn firmware(&self) -> Option<u32> {
        *self
            .firmware
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Record a firmware level detected by the dispatcher (`board.probe`).
    pub fn set_firmware(&self, level: u32) {
        *self
            .firmware
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(level);
    }

    /// One revision for a write this dispatcher performed; also publishes it to the shared counter so
    /// a later worker write cannot move the session backwards.
    pub(crate) fn bump_revision(&mut self) -> u64 {
        // A worker's mid-run write can bump the shared counter while a command is running. fetch_add
        // gives this write a unique revision instead of overwriting the worker's value with rev+1
        // (CORE-F4).
        let next = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        self.rev = next;
        next
    }

    /// Stops every run, waits for the workers (bounded by the supervisor's shutdown budget) and
    /// emits `core.stopping`, then joins the supervisor thread. Called by the binary after the final
    /// response is queued and before the writer is finished -- the supervisor is the other
    /// `EventBus` holder, and the writer only stops when the last one is gone. The outcome tells
    /// `main` whether a worker outlived the deadline (CORE-F8).
    pub fn shutdown_tools(&mut self, reason: &str) -> crate::supervisor::ShutdownOutcome {
        let Some(mut supervisor) = self.supervisor.take() else {
            return crate::supervisor::ShutdownOutcome::Drained;
        };
        supervisor.shutdown(reason)
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

#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "frame pixel coordinates and counts are far inside the range these conversions can represent"
)]
#[allow(
    clippy::too_many_lines,
    reason = "one protocol handler keeps its ordered read/validate/answer steps in one place"
)]
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
    // CORE-F11: a cached frame is one multi-megabyte base64 string inside a `Value`; it is decoded
    // from the borrow instead of being cloned per scan, and the optional capture PNG is decoded from
    // the same borrow. A region capture is owned here anyway.
    let save_captures = state
        .config
        .pointer("/tuner/save_captures")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let (width, height, rgba, frame_png) =
        if let Some(frame_id) = request.args.get("frame_id").and_then(Value::as_str) {
            let Some(frame) = state.frames.get(frame_id) else {
                return bad_request(
                    &request.id,
                    &format!("unknown frame_id {frame_id:?}"),
                    json!({}),
                );
            };
            let (width, height, rgba) = match frame.rgba() {
                Ok((width, height, rgba)) => (width, height, rgba.to_vec()),
                Err(message) => return internal(&request.id, &message),
            };
            let frame_png = save_captures.then(|| frame.png().to_vec());
            (width, height, rgba, frame_png)
        } else if request.args.get("region").is_some() {
            let title = match win::configured_title(&state.config) {
                Ok(title) => title,
                Err(error) => return win_error_response(&request.id, error),
            };
            let frame = match win::capture(&title, region) {
                Ok((frame, _display)) => frame,
                Err(error) => return win_error_response(&request.id, error),
            };
            let (width, height, rgba) = match frame_rgba(&frame) {
                Ok(image) => image,
                Err(message) => return internal(&request.id, &message),
            };
            (width, height, rgba, png_of(&frame, save_captures))
        } else {
            return bad_request(
                &request.id,
                "ocr.scan needs a frame_id or a region",
                json!({}),
            );
        };
    let geometry = ocr_geometry(&state.config);
    // CORE-F7: one loader for every scan path, so this scan also rebuilds the pipeline when the
    // config changed its dictionary/row height (the duplicated inline load used to serve the old
    // one forever).
    if let Some(response) = ensure_ocr(state, &request.id) {
        return response;
    }
    let Some(pipeline) = state.ocr.as_mut() else {
        return internal(&request.id, "the OCR pipeline is not loaded");
    };
    let scan = match pipeline.scan_rgba(width, height, &rgba) {
        Ok(scan) => scan,
        Err(error) => return internal(&request.id, &error.message),
    };
    // G25: the grade COLOUR fallback needs the grade box's pixels. `grade_area` is region-relative,
    // so this is a crop of the captured image (the OCR pipeline's own coordinate space).
    let grade_image = if mode == seal_ocr::modes::Mode::Grade {
        let colors = state
            .config
            .pointer("/tuner/grade_colors")
            .cloned()
            .and_then(|block| serde_json::from_value(block).ok())
            .unwrap_or_default();
        geometry.grade_area.as_ref().and_then(|area| {
            // `crop_rgba` takes the IMAGE dimensions; the area's own width/height are the crop's
            // size, not the buffer stride. Passing the area size as the image size made every
            // grade area not starting at (0,0) crop out of bounds and silently disable the colour
            // fallback (CORE-F5).
            crop_grade_area(area, width, height, &rgba)
                .map(|(w, h, pixels)| (colors, w as usize, h as usize, pixels))
        })
    } else {
        None
    };
    let outcome = seal_ocr::modes::apply_with_image(
        mode,
        &scan,
        &state.attributes,
        &geometry,
        grade_image.as_ref().map(
            |(colors, width, height, pixels)| seal_ocr::modes::GradeImage {
                rgba: pixels,
                width: *width,
                height: *height,
                colors,
            },
        ),
    );
    // v2 wrote one `ocr_log.jsonl` record per completed scan and an `ocr_errors.jsonl` record when
    // it stayed unconfirmed; a capture PNG is saved only while `tuner.save_captures` is on. The
    // scan's own PNG bytes are already in the frame, so a capture is a byte copy, not a re-encode.
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
        .map_or(10, |value| u32::try_from(value).unwrap_or(10));
    seal_ocr::modes::OcrGeometry {
        attr_x: band("attr_x"),
        attr_y: band("attr_y"),
        grade_y: band("grade_y"),
        grade_area: grade_area(config),
        remaining_x: band("remaining_x"),
        remaining_y: band("remaining_y"),
        row_height,
    }
}

/// Crop the tuner's grade box out of a captured image. The IMAGE's width/height size the source
/// buffer; the area supplies only the crop rectangle. CORE-F5.
fn crop_grade_area(
    area: &seal_ocr::modes::GradeArea,
    image_width: usize,
    image_height: usize,
    rgba: &[u8],
) -> Option<(u32, u32, Vec<u8>)> {
    let image_width = u32::try_from(image_width).ok()?;
    let image_height = u32::try_from(image_height).ok()?;
    let region_width = u32::try_from(area.x2.saturating_sub(area.x1).max(1)).ok()?;
    let region_height = u32::try_from(area.y2.saturating_sub(area.y1).max(1)).ok()?;
    let rect = seal_domain::geometry::Rect::new(
        area.x1,
        area.y1,
        i32::try_from(region_width).ok()?,
        i32::try_from(region_height).ok()?,
    );
    seal_win::pure::crop_rgba(image_width, image_height, rgba, rect).ok()
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

/// The PNG bytes a scan records when `tuner.save_captures` is on, decoded out of the frame's base64
/// once. CORE-F11: the frame itself stays borrowed, never cloned.
fn png_of(frame: &Value, save_captures: bool) -> Option<Vec<u8>> {
    if !save_captures {
        return None;
    }
    let data = frame.get("data").and_then(Value::as_str)?;
    base64::engine::general_purpose::STANDARD.decode(data).ok()
}
/// Answers one request. Never panics: a bad request is a refusal the client can show. CORE-F3: a
/// dispatcher panic is contained and answered with `internal`, instead of killing the core with a
/// tool/COM port still open.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the protocol entry point owns the decoded request; the frame loop and every test call it with a constructed value"
)]
pub fn handle(state: &mut CoreState, request: Request) -> Response {
    let id = request.id.clone();
    if request.v != PROTOCOL_VERSION {
        return Response::err(
            &id,
            Error::new(
                "bad_version",
                &format!("protocol {} is not {}", request.v, PROTOCOL_VERSION),
                json!({"expected": PROTOCOL_VERSION}),
            ),
        );
    }
    match catch_unwind(AssertUnwindSafe(|| handle_inner(state, &request))) {
        Ok(response) => response,
        Err(_) => internal(&id, "internal error — see error.log"),
    }
}

fn handle_inner(state: &mut CoreState, request: &Request) -> Response {
    // A mid-run pet write can bump the shared revision between requests. If it did, the dispatcher's
    // copy is stale: refresh it from the shared mirror BEFORE the command can read or republish it.
    // This is the CORE-F4 fix; it also makes `board.details` (a read-only command) harmless after a
    // worker persisted `food_slots_used`.
    let shared_rev = state.revision.load(Ordering::SeqCst);
    if shared_rev > state.rev {
        if let Ok(shared) = state.shared_config.read() {
            state.config = shared.as_ref().clone();
        }
        state.rev = shared_rev;
    }
    let rev_before = state.rev;
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
        "config.validate" => config_validate(state, request),
        "config.apply" => config_apply(state, request),
        "config.reload" => config_reload(state, request),
        "ui.window" => ui_window(state, request),
        "window.find" => window_find(state, request),
        "window.measure" => window_measure(state, request),
        "capture" => capture_command(state, request),
        "ocr.scan" => ocr_scan(state, request),
        "keys.parse" => keys_parse(state, request),
        "keys.names" => keys_names(state, request),
        "attributes.get" => attributes_get(state, request),
        "logs.cleanup_captures" => logs_cleanup_captures(state, request),
        "calibration.save" => crate::calibration::save(state, request),
        "calibration.fetch" => crate::calibration::fetch(state, request),
        "calibration.fetch_all" => crate::calibration::fetch_all(state, request),
        "calibration.kinds" => crate::calibration::kinds(request),
        "calibration.check" => crate::calibration::check(state, request),
        "calibration.tooltip.hover_capture" => {
            crate::tooltip_session::hover_capture(state, request)
        }
        "calibration.tooltip.save" => crate::tooltip_session::save(state, request),
        "calibration.tooltip.test" => crate::tooltip_session::test(state, request),
        "calibration.tuner.check" => crate::tuner_session::check(state, request),
        "calibration.tuner.save" => crate::tuner_session::save(state, request),
        "calibration.pet.preview" => crate::pet_calibration::preview(state, request),
        "calibration.pet.save" => crate::pet_calibration::save(state, request),
        "calibration.pet.test_read" => crate::pet_calibration::test_read(state, request),
        "calibration.gem.moves.save" => crate::gem_session::save(state, request),
        "calibration.gem.check_result" => crate::gem_session::check_result(state, request),
        "calibration.gem.save" => crate::gem_save::save(state, request),
        "calibration.gem.test_route" => crate::gem_routes_session::test_route(state, request),
        "calibration.gem.cycle" => crate::gem_routes_session::cycle(state, request),
        "calibration.buysell.preview" => crate::buysell_session::preview(state, request),
        "calibration.shop.test_scroll" => crate::buysell_session::test_scroll(state, request),
        "buy_sell.preflight" => Response::ok(
            &request.id,
            crate::buysell_session::preflight(&state.config),
        ),
        "pointer.place" => pointer_command(state, request, false),
        "pointer.click" => pointer_command(state, request, true),
        "board.probe" => board_probe(state, request),
        "board.details" => board_details(state, request),
        "board.test" => board_test(state, request),
        "pet.preflight" => pet_preflight(state, request),
        "pet.set_boarding" => pet_set_boarding(state, request),
        "pet.capture_icon" => pet_capture_icon(state, request),
        "pet.scan_food_page" => pet_scan_page(state, request, false),
        "pet.scan_queue_page" => pet_scan_page(state, request, true),
        "pet.apply_food_scan" => pet_apply_food_scan(state, request),
        "pet.scan_feeding_page" => pet_scan_feeding_page(state, request),
        "pet.scan_apply_queue" => pet_scan_apply_queue(state, request),
        "pet.test_read_panel" => pet_test_read_panel(state, request),
        "pet.select_page" => crate::pet_session::select_page(state, request),
        "tool.start" => tool_start(state, request),
        "tool.stop" => tool_stop(state, request),
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
    // Publish only when THIS command changed the config. A read-only command must not write the
    // dispatcher's old copy over a mid-run worker write (CORE-F4).
    if state.rev != rev_before {
        state.publish_config();
    }
    response
}

impl CoreState {
    /// Publish the merged config for the production registry. Cheap (a clone of one JSON tree) and
    /// called once per command, so registry and shell can never disagree about which preset is active.
    pub fn publish_config(&self) {
        if let Ok(mut shared) = self.shared_config.write() {
            *shared = Arc::new(self.config.clone());
        }
    }

    /// The OCR pipeline's identity under the CURRENT config (CORE-F7).
    fn ocr_identity(&self) -> OcrIdentity {
        OcrIdentity {
            dictionary: self.dictionary.clone(),
            row_height: ocr_geometry(&self.config).row_height,
            attributes: self.attributes.clone(),
        }
    }

    /// Whether the cached pipeline was built from a different dictionary/row height than the config
    /// now names (CORE-F7). `false` only when a cache exists AND matches; `ensure_ocr` pairs this
    /// with the pipeline being present.
    fn ocr_is_stale(&self) -> bool {
        self.ocr_build.as_ref() != Some(&self.ocr_identity())
    }
}

fn hello(state: &mut CoreState, request: &Request) -> Response {
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
        return config_error_response(&request.id, &error);
    }

    // Commit: the files are now the truth, so re-read the merged view; the dictionary was loaded
    // BEFORE the write and lands with the variant, one rev covering both halves.
    let config = match state.store.load() {
        Ok(config) => config,
        Err(error) => return config_error_response(&request.id, &error),
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
        // CORE-F11: `echo_sections` only reads the merged config, so the whole JSON tree is no
        // longer cloned just to be read from.
        json!({"rev": state.rev, "sections": echo_sections(&state.config, &sections)}),
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
        Err(error) => config_error_response(&request.id, &error),
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
        StartReply::Started | StartReply::Deferred { .. } => Response::ok(&request.id, json!({})),
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
#[allow(
    clippy::too_many_lines,
    reason = "the per-tool argument table is one exhaustive match; splitting it would hide which tool accepts what"
)]
fn start_arguments(tool: ToolId, args: &Value) -> Result<Value, (String, String)> {
    let Some(object) = args.as_object() else {
        return Err((
            "tool.start arguments must be an object".to_owned(),
            String::new(),
        ));
    };
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "tool" | "preset" | "count" | "slots" | "test" | "dry_run"
        ) {
            return Err((
                format!("tool.start doesn't take a {key:?} field"),
                key.clone(),
            ));
        }
    }

    let dry_run = if let Some(value) = object.get("dry_run") {
        if !matches!(tool, ToolId::Buy | ToolId::Sell) {
            return Err((
                "tool.start's \"dry_run\" is only accepted by \"buy\" and \"sell\"".to_owned(),
                "dry_run".to_owned(),
            ));
        }
        let Some(flag) = value.as_bool() else {
            return Err((
                "tool.start's \"dry_run\" must be a boolean".to_owned(),
                "dry_run".to_owned(),
            ));
        };
        if !flag {
            return Err((
                "tool.start's \"dry_run\" is a positive flag; omit it for a real run".to_owned(),
                "dry_run".to_owned(),
            ));
        }
        true
    } else {
        false
    };

    if object.contains_key("test") {
        if tool != ToolId::Quest {
            return Err((
                "tool.start's \"test\" is only accepted by \"quest\"".to_owned(),
                "test".to_owned(),
            ));
        }
        let Some(test) = object.get("test").and_then(Value::as_bool) else {
            return Err((
                "tool.start's \"test\" must be a boolean".to_owned(),
                "test".to_owned(),
            ));
        };
        if !test {
            return Err((
                "tool.start's \"test\" is a positive flag; omit it for a normal run".to_owned(),
                "test".to_owned(),
            ));
        }
        return Ok(json!({"test": true}));
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
            if dry_run {
                forwarded.insert("dry_run".to_owned(), Value::Bool(true));
            }
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
            if dry_run {
                forwarded.insert("dry_run".to_owned(), Value::Bool(true));
            }
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
        StopReply::Internal { line } => internal(&request.id, &line),
    }
}

/// `pet.preflight {}`: the Pet tab's two v2 checklists and Ready verdict. READ-ONLY: no window,
/// board or OCR access; a malformed/absent pet block still answers with `ok:false` facts.
fn pet_preflight(state: &CoreState, request: &Request) -> Response {
    Response::ok(&request.id, crate::pet_session::preflight(&state.config))
}

/// `pet.set_boarding {row, running}`: the tick. It writes ONE `pet.slots[row-1].boarding_running`
/// through the A1 field-precise sink (not a whole-block apply), emits one `config.changed`, and
/// answers `{rev, block:<merged pet>}`. A bad row/type is `bad_request`; an I/O failure is logged
/// and answered `io` (the file is never partially written by the store).
fn pet_set_boarding(state: &mut CoreState, request: &Request) -> Response {
    if let Some(refusal) = input_lease_refusal(state, request) {
        return refusal;
    }
    let (row, running) = match crate::pet_session::boarding_arguments(&request.args) {
        Ok(argument) => argument,
        Err((message, field)) => {
            return bad_request(&request.id, &message, json!({"field": field}));
        }
    };
    let setup = crate::pet::setup(&state.config);
    if row >= setup.slots.len() {
        return bad_request(
            &request.id,
            &format!(
                "pet.set_boarding's \"row\" is out of range: this setup has {} row(s)",
                setup.slots.len()
            ),
            json!({"field": "row"}),
        );
    }
    let patch = seal_config::PetRunStatePatch {
        boarding_running: vec![(row, running)],
        ..seal_config::PetRunStatePatch::default()
    };
    write_pet_patch(state, request, &patch)
}

/// `pet.capture_icon {kind,page,cell}`: capture the visible bag page and return ONE cropped icon.
/// The shell has hidden itself and put the page up; this command never clicks or moves the cursor,
/// and it opens no board. The input lease is checked first (a foreground tool owns the game), then
/// the calibrated grid/page-tab prerequisites, then the foreground-gated full-client capture.
fn pet_capture_icon(state: &mut CoreState, request: &Request) -> Response {
    let arguments = match crate::pet_capture::arguments(&request.args) {
        Ok(arguments) => arguments,
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
                        json!({"line": line, "command": "pet.capture_icon"}),
                    ),
                );
            }
            Ok(None) => {}
            Err(line) => return internal(&request.id, &line),
        }
    }
    let rect = match crate::pet_capture::geometry(&state.config, &arguments) {
        Ok(rect) => rect,
        Err(crate::pet_capture::IconError::BadRequest { message, field }) => {
            return bad_request(&request.id, &message, json!({"field": field}));
        }
        Err(crate::pet_capture::IconError::Internal(message)) => {
            return internal(&request.id, &message);
        }
    };
    let title = match win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return win_error_response(&request.id, error),
    };
    let frame = match win::capture(&title, None) {
        Ok((frame, _display)) => frame,
        Err(error) => return win_error_response(&request.id, error),
    };
    match crate::pet_capture::icon_from_frame(&arguments, rect, &frame) {
        Ok(result) => Response::ok(&request.id, result),
        Err(crate::pet_capture::IconError::BadRequest { message, field }) => {
            bad_request(&request.id, &message, json!({"field": field}))
        }
        Err(crate::pet_capture::IconError::Internal(message)) => internal(&request.id, &message),
    }
}

/// `pet.scan_food_page` / `pet.scan_queue_page`: one page, READ-ONLY. Input lease, one
/// foreground-gated capture (the shell hides itself and puts the page up), page PNG + report under
/// `logs/reads/`, and the run's own matcher. Evidence is best-effort and never fails the scan.
fn pet_scan_page(state: &mut CoreState, request: &Request, queue: bool) -> Response {
    let page = match crate::pet_scan::page_argument(&request.args) {
        Ok(page) => page,
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
    if let Err(error) = crate::pet_capture::page_grid(&state.config, page) {
        return icon_error_response(&request.id, error);
    }
    // N2: the food icon is a config prerequisite. Validate/decode it BEFORE the hidden capture and
    // evidence write, so a missing/corrupt crop is a cheap refusal with no orphan PNG.
    let food_icon = if queue {
        None
    } else {
        match crate::pet_scan::food_icon(&state.config) {
            Ok(icon) => Some(icon),
            Err(error) => return scan_error_response(&request.id, error),
        }
    };
    let title = match win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return win_error_response(&request.id, error),
    };
    let frame = match win::capture(&title, None) {
        Ok((frame, _display)) => frame,
        Err(error) => return win_error_response(&request.id, error),
    };
    let image = match crate::pet_scan::frame_image(&frame) {
        Ok(image) => image,
        Err(message) => return internal(&request.id, &message),
    };
    let stamp = crate::logs::read_stamp();
    let (page_name, report_name) = crate::pet_scan::evidence_names(page, &stamp);
    let mut evidence = Vec::new();
    if let Some(data) = frame.get("data").and_then(Value::as_str)
        && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data)
        && let Some(path) = state.logs.write_read(&page_name, &bytes)
    {
        evidence.push(path);
    }

    let (mut result, report) = if queue {
        match crate::pet_scan::queue_scan(&state.config, page, &image) {
            Ok(result) => result,
            Err(error) => return scan_error_response(&request.id, error),
        }
    } else {
        let Some(food_icon) = food_icon.as_ref() else {
            return internal(
                &request.id,
                "the food icon was validated before the capture",
            );
        };
        match crate::pet_scan::food_scan_with_icon(&state.config, page, &image, food_icon) {
            Ok(result) => result,
            Err(error) => return scan_error_response(&request.id, error),
        }
    };
    if let Some(path) = state
        .logs
        .write_read(&report_name, report.join("\n").as_bytes())
    {
        evidence.push(path);
    }
    if let Some(object) = result.as_object_mut() {
        object.insert("evidence".to_owned(), json!(evidence));
    }
    Response::ok(&request.id, result)
}

fn scan_error_response(id: &str, (message, field): (String, &'static str)) -> Response {
    if field == "internal" {
        internal(id, &message)
    } else {
        bad_request(id, &message, json!({"field": field}))
    }
}

fn icon_error_response(id: &str, error: crate::pet_capture::IconError) -> Response {
    match error {
        crate::pet_capture::IconError::BadRequest { message, field } => {
            bad_request(id, &message, json!({"field": field}))
        }
        crate::pet_capture::IconError::Internal(message) => internal(id, &message),
    }
}

/// `pet.test_read_panel {page,cell,delay_ms}`: hover the cell (board closed-loop placement, never a
/// click), settle for the bounded delay, capture the configured tooltip box and OCR it. The panel
/// parser is the same domain `PetPanel::parse` the run uses; a partial OCR is returned as raw lines
/// plus nulls, never invented numbers. No config write.
fn pet_test_read_panel(state: &mut CoreState, request: &Request) -> Response {
    let arguments = match crate::pet_panel::arguments(&request.args) {
        Ok(arguments) => arguments,
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
                        json!({"line": line, "command": "pet.test_read_panel"}),
                    ),
                );
            }
            Ok(None) => {}
            Err(line) => return internal(&request.id, &line),
        }
    }
    let geometry = match crate::pet_panel::geometry(&state.config, &arguments) {
        Ok(geometry) => geometry,
        Err((message, field)) => {
            return bad_request(&request.id, &message, json!({"field": field}));
        }
    };
    if let Some(response) = ensure_ocr(state, &request.id) {
        return response;
    }
    // Hover ONLY: the board's closed-loop placement, no click command anywhere on this path.
    if let Err(failure) = crate::pointer::system_perform(&state.config, geometry.centre, false) {
        return match failure {
            crate::pointer::PointerFailure::Window(error) => win_error_response(&request.id, error),
            crate::pointer::PointerFailure::Board(error) => {
                board_error_response(&request.id, error)
            }
            crate::pointer::PointerFailure::Effects(line) => Response::err(
                &request.id,
                Error::new("board_unavailable", &line, json!({})),
            ),
        };
    }
    std::thread::sleep(std::time::Duration::from_millis(arguments.delay_ms));
    let title = match win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return win_error_response(&request.id, error),
    };
    let frame = match win::capture(&title, Some(geometry.region)) {
        Ok((frame, _display)) => frame,
        Err(error) => return win_error_response(&request.id, error),
    };
    let (width, height, rgba) = match frame_rgba(&frame) {
        Ok(image) => image,
        Err(message) => return internal(&request.id, &message),
    };
    let lines = {
        let Some(pipeline) = state.ocr.as_mut() else {
            return internal(&request.id, "the OCR pipeline is not loaded");
        };
        match pipeline.scan_rgba(width, height, &rgba) {
            Ok(scan) => scan
                .items
                .into_iter()
                .map(|item| item.text)
                .collect::<Vec<String>>(),
            Err(error) => return internal(&request.id, &error.message),
        }
    };
    let mut result = crate::pet_panel::panel_result(&lines);
    if let Some(object) = result.as_object_mut() {
        object.insert(
            "region".to_owned(),
            json!({
                "x": geometry.region.x,
                "y": geometry.region.y,
                "width": geometry.region.w,
                "height": geometry.region.h,
            }),
        );
        // The diagnostic saves the exact region the OCR was given, under the command's own name
        // (best-effort, like the scans' evidence): a wrong region and a pet that is not in the cell
        // can then be told apart by looking at the image. A failed write is `evidence: null`, not a
        // failed read.
        let evidence = frame
            .get("data")
            .and_then(Value::as_str)
            .and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok())
            .and_then(|png| {
                state
                    .logs
                    .write_read(&format!("petpanel_{}.png", crate::logs::read_stamp()), &png)
            });
        object.insert(
            "evidence".to_owned(),
            evidence.map_or(Value::Null, Value::String),
        );
    }
    Response::ok(&request.id, result)
}

/// Load the shared OCR pipeline for the panel diagnostic, if it is not cached yet. Failing here is
/// the existing `ocr_unavailable`; no hover or capture happens first. `Some(response)` means the
/// diagnostic is over before it touched the game.
///
/// CORE-F7: the cache is also invalidated here. The pipeline bakes in the dictionary's `text_fixes`
/// and `tuner.ocr.row_height`, so a variant switch (`config.apply`), a `config.reload` or a
/// `calibration.tuner.save` must rebuild it. Reloading the ONNX sessions is the honest refresh —
/// `seal-ocr` exposes no cheaper `reconfigure` today.
pub(crate) fn ensure_ocr(state: &mut CoreState, id: &str) -> Option<Response> {
    if state.ocr.is_some() && !state.ocr_is_stale() {
        return None;
    }
    let identity = state.ocr_identity();
    let geometry = ocr_geometry(&state.config);
    match seal_ocr::pipeline::OcrPipeline::load_for_root(
        &state.root,
        &state.attributes,
        geometry.row_height,
    ) {
        Ok(pipeline) => {
            state.ocr = Some(pipeline);
            state.ocr_build = Some(identity);
            None
        }
        Err(error) => {
            // The cache cannot be trusted any more: a stale pipeline would read with the dictionary
            // the config no longer names, so it is dropped rather than used behind the refusal.
            state.ocr = None;
            state.ocr_build = None;
            Some(Response::err(
                id,
                Error::new(
                    "ocr_unavailable",
                    "the OCR models are not available",
                    json!({"reason": error.reason, "message": error.message}),
                ),
            ))
        }
    }
}

/// `pet.scan_feeding_page {page, harvest?}`: v2's "Scan bag for feedable pets", one page at a time.
///
/// It is FIND-THEN-HOVER, reusing the run's own matcher: one clean page capture, `score_all_scores`
/// against the queued icons, then hover ONLY the matched cells and OCR the tooltip panel. A panel
/// that will not read is `unknown` and never `finished`. `harvest` crops an icon for each feedable
/// cell out of the CLEAN page image (not a panel-covered one), so a rebuilt queue's icons are the
/// same pixels a hand capture would produce.
#[allow(
    clippy::cast_sign_loss,
    reason = "the hover delay is clamped to at least 200 before it becomes a u64"
)]
#[allow(
    clippy::too_many_lines,
    reason = "one protocol handler keeps its ordered read/validate/answer steps in one place"
)]
fn pet_scan_feeding_page(state: &mut CoreState, request: &Request) -> Response {
    let arguments = match crate::pet_scan::feeding_arguments(&request.args) {
        Ok(arguments) => arguments,
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
    let grid = match crate::pet_capture::page_grid(&state.config, arguments.page) {
        Ok(grid) => grid,
        Err(error) => return icon_error_response(&request.id, error),
    };
    // CORE-F11: this used to clone the WHOLE merged config just to keep it alive across the OCR
    // pipeline borrow; every use below reads `state.config` directly and the borrows do not overlap.
    let tooltip: seal_domain::config::TooltipConfig = state
        .config
        .get("tooltip")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    if !tooltip.is_set() {
        return bad_request(
            &request.id,
            "The tooltip/panel region isn't calibrated - save the hover panel box on the Calibrate tab first.",
            json!({"field": "tooltip"}),
        );
    }
    let point = |path: &str, label: &str| -> Result<(i32, i32), Box<Response>> {
        let values = state
            .config
            .pointer(path)
            .and_then(Value::as_array)
            .filter(|values| values.len() == 2)
            .ok_or_else(|| {
                Box::new(bad_request(
                    &request.id,
                    &format!("{label} isn't calibrated - mark it on the Calibrate tab first."),
                    json!({"field": path}),
                ))
            })?;
        let x = values[0]
            .as_i64()
            .and_then(|value| i32::try_from(value).ok());
        let y = values[1]
            .as_i64()
            .and_then(|value| i32::try_from(value).ok());
        match (x, y) {
            (Some(x), Some(y)) => Ok((x, y)),
            _ => Err(Box::new(bad_request(
                &request.id,
                &format!("{label}'s mark is malformed - re-mark it on the Calibrate tab."),
                json!({"field": path}),
            ))),
        }
    };
    let focus = match point("/buy_sell/scroll_point", "The focus/scroll point") {
        Ok(point) => point,
        Err(response) => return *response,
    };
    let tab_index = arguments.page - 1;
    let tab_path = format!("/pet/page_tabs/{tab_index}");
    let tab = match point(&tab_path, &format!("Bag page {}", arguments.page)) {
        Ok(point) => point,
        Err(response) => return *response,
    };
    if let Some(response) = ensure_ocr(state, &request.id) {
        return response;
    }
    let title = match win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return win_error_response(&request.id, error),
    };
    if let Err(error) = pointer::system_perform(
        &state.config,
        seal_domain::geometry::Point::new(focus.0, focus.1),
        true,
    ) {
        return pointer_failure(&request.id, error);
    }
    std::thread::sleep(std::time::Duration::from_millis(400));
    if let Err(error) = pointer::system_perform(
        &state.config,
        seal_domain::geometry::Point::new(tab.0, tab.1),
        true,
    ) {
        return pointer_failure(&request.id, error);
    }
    std::thread::sleep(std::time::Duration::from_millis(900));
    if let Err(error) = pointer::system_perform(
        &state.config,
        seal_domain::geometry::Point::new(focus.0, focus.1),
        false,
    ) {
        return pointer_failure(&request.id, error);
    }
    std::thread::sleep(std::time::Duration::from_millis(250));
    let (page_frame, _display) = match win::capture(&title, None) {
        Ok((frame, display)) => (frame, display),
        Err(error) => return win_error_response(&request.id, error),
    };
    let page_image = match crate::pet_scan::frame_image(&page_frame) {
        Ok(image) => image,
        Err(message) => return internal(&request.id, &message),
    };
    let mut matched: std::collections::BTreeMap<usize, (String, f64)> =
        std::collections::BTreeMap::new();
    for (label, icon) in crate::pet_scan::queue_entries(&state.config) {
        let Some(icon) = icon else { continue };
        for (cell, score) in crate::pet::score_all_scores(&page_image, &grid, &icon) {
            if score > seal_domain::tools::pet::MATCH_LIMIT {
                break;
            }
            matched.entry(cell).or_insert((label.clone(), score));
        }
    }
    let centres = BagGrid::centres(Some(&grid));
    let pitch_x = BagGrid::pitch_x(&grid);
    let pitch_y = BagGrid::pitch_y(&grid);
    let mut cells = Vec::new();
    let mut report = Vec::new();
    let mut feedable = 0usize;
    let mut finished = 0usize;
    let mut unread = 0usize;
    for (cell, (label, score)) in &matched {
        let Some(centre) = centres.get(*cell) else {
            continue;
        };
        let mut panel = None;
        let mut crop = None;
        if pointer::system_perform(
            &state.config,
            seal_domain::geometry::Point::new(centre.x, centre.y),
            false,
        )
        .is_ok()
        {
            std::thread::sleep(std::time::Duration::from_millis(
                tooltip.hover_delay_ms.max(200) as u64,
            ));
            let region = seal_domain::geometry::Rect::new(
                centre.x + tooltip.offset_x,
                centre.y + tooltip.offset_y,
                tooltip.width,
                tooltip.height,
            );
            if let Ok((panel_frame, _)) = win::capture(&title, Some(region))
                && let Ok((width, height, rgba)) = frame_rgba(&panel_frame)
                && let Some(pipeline) = state.ocr.as_mut()
                && let Ok(scan) = pipeline.scan_rgba(width, height, &rgba)
            {
                let lines: Vec<String> = scan.lines.iter().map(|line| line.text.clone()).collect();
                panel = seal_domain::ocr::PetPanel::parse(&lines);
            }
            if arguments.harvest
                && let Some(crop_image) = crop_pitch(&page_image, *centre, pitch_x, pitch_y)
                && let Some(png) = encode_icon_png(&crop_image)
            {
                crop = Some(png);
            }
        }
        let value = crate::pet_scan::feeding_cell(*cell, label, Some(*score), panel.as_ref(), crop);
        match value["status"].as_str().unwrap_or("unknown") {
            "feedable" => feedable += 1,
            "finished" => finished += 1,
            _ => unread += 1,
        }
        report.push(format!(
            "cell {}: {} - {}",
            cell + 1,
            value["status"].as_str().unwrap_or("unknown"),
            value["line"]
                .as_str()
                .unwrap_or("the hover panel didn't read")
        ));
        cells.push(value);
    }
    let summary = if matched.is_empty() {
        "no queued pet matched on this page".to_owned()
    } else {
        format!(
            "{} - {} feedable, {} finished, {} unread",
            crate::pet_scan::feeding_summary(&cells),
            feedable,
            finished,
            unread
        )
    };
    let stamp = crate::logs::read_stamp();
    let page_name = format!("pet_feed_page{}_{}.png", arguments.page, stamp);
    let report_name = format!("pet_feed_page{}_{}.txt", arguments.page, stamp);
    let mut evidence = Vec::new();
    if let Some(data) = page_frame.get("data").and_then(Value::as_str)
        && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data)
        && let Some(path) = state.logs.write_read(&page_name, &bytes)
    {
        evidence.push(path);
    }
    if let Some(path) = state
        .logs
        .write_read(&report_name, report.join("\n").as_bytes())
    {
        evidence.push(path);
    }
    Response::ok(
        &request.id,
        json!({
            "page": arguments.page,
            "cells": cells,
            "summary": summary,
            "report": report,
            "evidence": evidence,
        }),
    )
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "frame pixel coordinates and counts are far inside the range these conversions can represent"
)]
fn crop_pitch(
    image: &crate::pet::PetImage,
    centre: seal_domain::geometry::Point,
    pitch_x: f64,
    pitch_y: f64,
) -> Option<crate::pet::PetImage> {
    let width = (pitch_x.round() as i32).max(1);
    let height = (pitch_y.round() as i32).max(1);
    let rect = seal_domain::geometry::Rect::new(
        centre.x - width / 2,
        centre.y - height / 2,
        width,
        height,
    );
    let (w, h, rgba) =
        seal_win::pure::crop_rgba(image.width as u32, image.height as u32, &image.rgba, rect)
            .ok()?;
    Some(crate::pet::PetImage {
        width: w as usize,
        height: h as usize,
        rgba,
    })
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "frame pixel coordinates and counts are far inside the range these conversions can represent"
)]
fn encode_icon_png(image: &crate::pet::PetImage) -> Option<String> {
    let mut bgra = Vec::with_capacity(image.rgba.len());
    for pixel in image.rgba.as_chunks::<4>().0 {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    let png = seal_win::pure::encode_png(image.width as u32, image.height as u32, &bgra).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(png))
}

/// `pet.scan_apply_queue {queue:[...]}`: replace `pet.queue` with the sweep's harvested icons. The
/// normal config writer is used (this is not run state), with one rev bump and one `config.changed`
/// after the response. An empty replacement is refused before any write.
fn pet_scan_apply_queue(state: &mut CoreState, request: &Request) -> Response {
    let queue = match crate::pet_scan::queue_apply_arguments(&request.args) {
        Ok(queue) => Value::Array(queue),
        Err((message, field)) => {
            return bad_request(&request.id, &message, json!({"field": field}));
        }
    };
    if let Err(error) = state.store.update_local(|local| {
        if !local.get("pet").is_some_and(Value::is_object) {
            local["pet"] = json!({});
        }
        if let Some(pet) = local.get_mut("pet").and_then(Value::as_object_mut) {
            pet.insert("queue".to_owned(), queue.clone());
        }
    }) {
        return Response::err(&request.id, Error::new("io", &error.to_string(), json!({})));
    }
    // The queue is the only field this command changes, so update the dispatcher's copy directly:
    // re-reading the file could fail on a fixture with no defaults file and would not add a fact.
    if !state.config.get("pet").is_some_and(Value::is_object) {
        state.config["pet"] = json!({});
    }
    if let Some(pet) = state.config.get_mut("pet").and_then(Value::as_object_mut) {
        pet.insert("queue".to_owned(), queue);
    }
    let block = state.config.get("pet").cloned().unwrap_or(Value::Null);
    state.bump_revision();
    state.queue_config_changed("pet");
    Response::ok(&request.id, json!({"rev": state.rev, "block": block}))
}

fn pointer_failure(id: &str, error: pointer::PointerFailure) -> Response {
    match error {
        pointer::PointerFailure::Window(error) => win_error_response(id, error),
        pointer::PointerFailure::Board(error) => board_error_response(id, error),
        pointer::PointerFailure::Effects(line) => {
            Response::err(id, Error::new("board_unavailable", &line, json!({})))
        }
    }
}

/// `pet.apply_food_scan`: explicit write of the scan's chosen cells. Replaces `pet.food_slots` with
/// ZERO-BASED config pairs and resets `food_slots_used`, one A1 field-precise write + one
/// `config.changed` after the response. Empty is refused (nothing found means nothing written).
fn pet_apply_food_scan(state: &mut CoreState, request: &Request) -> Response {
    // A live feeder owns the game; accepting this write would let the run's next fold overwrite the
    // new cell list with its own snapshot (PET-F11). The same lease refusal board diagnostics use.
    if let Some(refusal) = input_lease_refusal(state, request) {
        return refusal;
    }
    let cells = match crate::pet_scan::apply_cells(&state.config, &request.args) {
        Ok(cells) => cells,
        Err((message, field)) => {
            return bad_request(&request.id, &message, json!({"field": field}));
        }
    };
    let patch = seal_config::PetRunStatePatch {
        food_slots: Some(cells),
        ..seal_config::PetRunStatePatch::default()
    };
    write_pet_patch(state, request, &patch)
}

/// The shared A1 command write: validate the sink exists, persist the field patch, refresh the
/// dispatcher's config/rev, queue the one `config.changed` after the response, and answer
/// `{rev, block}`. Used by `pet.set_boarding` and `pet.apply_food_scan`.
fn write_pet_patch(
    state: &mut CoreState,
    request: &Request,
    patch: &seal_config::PetRunStatePatch,
) -> Response {
    let Some(sink) = state.pet_state.as_ref() else {
        return internal(
            &request.id,
            "the pet run-state writer is not attached in this core",
        );
    };
    let rev = match sink.persist_rev(patch) {
        Ok(rev) => rev,
        Err(line) => {
            state.logs.error_line(&format!(
                "{} could not write run state: {line}",
                request.cmd
            ));
            return Response::err(&request.id, Error::new("io", &line, json!({})));
        }
    };
    match state.store.load() {
        Ok(config) => {
            let block = config.get("pet").cloned().unwrap_or(Value::Null);
            state.config = config;
            state.rev = rev;
            state.queue_config_changed("pet");
            Response::ok(&request.id, json!({"rev": rev, "block": block}))
        }
        Err(error) => internal(
            &request.id,
            &format!(
                "{} wrote, but the config could not be re-read: {error}",
                request.cmd
            ),
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

/// The per-command input lease shared by the pointer actions, the captures and the board
/// diagnostics: `Some(refusal)` while a foreground tool is running/stopping or the resident feeder
/// holds the game. `None` when no supervisor is attached (a config-only state has no tools to
/// collide with). The `tool_busy` line names the holder and `detail.command` names the refused
/// command.
fn input_lease_refusal(state: &CoreState, request: &Request) -> Option<Response> {
    let supervisor = state.supervisor.as_ref()?;
    match supervisor.input_lease() {
        Ok(None) => None,
        Ok(Some(line)) => Some(Response::err(
            &request.id,
            Error::new(
                "tool_busy",
                &line,
                json!({"line": line, "command": request.cmd}),
            ),
        )),
        Err(line) => Some(internal(&request.id, &line)),
    }
}

/// `board.probe`: which port, which baud, which IDs, and the firmware level from `V` (silence = level
/// 1). The board crate owns the choice and the open; this only maps its failure to the protocol.
fn board_probe(state: &CoreState, request: &Request) -> Response {
    // CORE-F14: the probe opens the port and sends `V`. While a run holds the board that fails with a
    // raw OS "access denied" as `board_unavailable`, which reads like a broken install; the lease
    // turns it into the same `tool_busy` line a start gets.
    if let Some(refusal) = input_lease_refusal(state, request) {
        return refusal;
    }
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
/// a failure means nothing reached a board. CORE-F14: the lease is checked FIRST, because opening the
/// port behind a running tool is exactly the raw "access denied" this command used to report.
fn board_test(state: &CoreState, request: &Request) -> Response {
    if let Some(refusal) = input_lease_refusal(state, request) {
        return refusal;
    }
    match board::test(&state.config, &request.args) {
        Ok(result) => Response::ok(&request.id, result),
        Err(error) => board_error_response(&request.id, error),
    }
}

/// A board failure: no usable port is `board_unavailable` (the shell shows the line), a malformed
/// request is `bad_request` before any port is touched.
pub(crate) fn board_error_response(id: &str, error: board::BoardFailure) -> Response {
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
    let allow_covered = match win::parse_allow_covered(&request.args) {
        Ok(allow) => allow,
        Err(error) => return win_error_response(&request.id, error),
    };
    let title = match win::configured_title(&state.config) {
        Ok(title) => title,
        Err(error) => return win_error_response(&request.id, error),
    };
    let (frame, display) = match win::capture_with(&title, region, allow_covered) {
        Ok(result) => result,
        Err(error) => return win_error_response(&request.id, error),
    };
    let frame_id = state.frames.insert(frame);
    let Some(frame) = state.frames.get_value(&frame_id) else {
        return internal(&request.id, "the captured frame left the frame cache");
    };
    Response::ok(&request.id, json!({"frame": frame, "display": display}))
}

/// Maps a window/capture failure to the protocol's error codes (`PROTOCOL.md` 禮6). The Win32 detail
/// stays in the message so a log is debuggable; the player sees what the shell chooses to show.
pub(crate) fn win_error_response(id: &str, error: win::CommandError) -> Response {
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
        win::CommandError::RegionOutside | win::CommandError::BadRequest(_) => {
            bad_request(id, &line, json!({}))
        }
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
fn config_error_response(id: &str, error: &ConfigError) -> Response {
    if let ConfigError::SectionNotOwned(name) = error {
        return bad_request(
            id,
            &format!("section {name:?} is not owned by the core"),
            json!({"section": name}),
        );
    }

    let detail = match config_error_path(error) {
        Some(path) => json!({"path": path.to_string_lossy()}),
        None => json!({}),
    };
    Response::err(id, Error::new("io", &error.to_string(), detail))
}

fn config_error_path(error: &ConfigError) -> Option<&Path> {
    match error {
        ConfigError::MissingFile(path)
        | ConfigError::Read { path, .. }
        | ConfigError::Parse { path, .. }
        | ConfigError::Serialize { path, .. }
        | ConfigError::Write { path, .. }
        | ConfigError::SchemaTooNew { path, .. } => Some(path.as_path()),
        // Neither names a file this command was asked to touch: a section refusal has no path, and
        // HW-F4's verify-before-retire message already carries the pointers and the backup.
        ConfigError::SectionNotOwned(_) | ConfigError::MigrationMismatch { .. } => None,
    }
}

/// A boot/reload failure as a protocol answer. A file-level failure keeps the existing `io`/config
/// mapping; an INVALID config has no protocol code of its own yet (`PROTOCOL.md` 禮6 has no
/// `config_invalid`), so the v2 `ConfigException` text is carried under `io` ??reported to the Lead
/// as a protocol proposal rather than silently inventing a code.
fn startup_error_response(id: &str, error: StartupError) -> Response {
    match error {
        StartupError::Config(error) => config_error_response(id, &error),
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

    use seal_domain::config::TextFix;
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
    fn a_dry_run_flag_is_forwarded_only_for_buy_and_sell() {
        assert_eq!(
            start_arguments(ToolId::Buy, &json!({"tool": "buy", "dry_run": true})).unwrap(),
            json!({"dry_run": true})
        );
        assert_eq!(
            start_arguments(
                ToolId::Sell,
                &json!({"tool": "sell", "slots": [1, 2], "dry_run": true})
            )
            .unwrap(),
            json!({"dry_run": true, "slots": [1, 2]})
        );
        for tool in [ToolId::Tuner, ToolId::Quest, ToolId::Pet] {
            let error = start_arguments(tool, &json!({"tool": tool.to_string(), "dry_run": true}))
                .expect_err("only buy/sell have a dry run");
            assert_eq!(error.1, "dry_run");
        }
        let error = start_arguments(ToolId::Buy, &json!({"tool": "buy", "dry_run": false}))
            .expect_err("false is not a dry run");
        assert_eq!(error.1, "dry_run");
    }

    #[test]
    fn a_quest_test_flag_is_forwarded_and_other_tools_refuse_it() {
        assert_eq!(
            start_arguments(ToolId::Quest, &json!({"tool": "quest", "test": true})).unwrap(),
            json!({"test": true})
        );
        let error = start_arguments(ToolId::Tuner, &json!({"tool": "tuner", "test": true}))
            .expect_err("only quest has a test loop");
        assert_eq!(error.1, "test");
        assert!(error.0.contains("only accepted by \"quest\""));
        let error = start_arguments(ToolId::Quest, &json!({"tool": "quest", "test": false}))
            .expect_err("false is not a normal run");
        assert_eq!(error.1, "test");
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

    #[test]
    fn pet_capture_icon_bad_arguments_name_kind_page_or_cell() {
        let mut state = CoreState::new(std::path::PathBuf::from(r"C:\\repo"));
        let _ = handle(
            &mut state,
            Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"})),
        );
        for (args, field) in [
            (json!({"kind": "pet", "page": 1, "cell": 1}), "kind"),
            (json!({"kind": "queue", "page": 0, "cell": 1}), "page"),
            (json!({"kind": "queue", "page": 1, "cell": 65}), "cell"),
        ] {
            let response = handle(
                &mut state,
                Request::new("capture", "pet.capture_icon", args),
            );
            let error = response.error.expect("a bad-argument refusal");
            assert_eq!(error.code, "bad_request");
            assert_eq!(error.detail["field"], field);
        }
    }

    #[test]
    fn pet_capture_icon_missing_grid_is_refused_before_any_capture() {
        let mut state = CoreState::new(std::path::PathBuf::from(r"C:\\repo"));
        state.config = json!({"pet": {"page_tabs": [[10, 10]]}});
        let _ = handle(
            &mut state,
            Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"})),
        );
        let response = handle(
            &mut state,
            Request::new(
                "capture",
                "pet.capture_icon",
                json!({"kind": "queue", "page": 1, "cell": 1}),
            ),
        );
        let error = response.error.expect("the missing grid is refused");
        assert_eq!(error.code, "bad_request");
        assert_eq!(error.detail["field"], "bag_grid");
    }

    #[test]
    fn pet_capture_icon_takes_the_input_lease_like_a_pointer_action() {
        let (release, release_rx) = mpsc::channel();
        let registry = Arc::new(BlockingRegistry {
            release: Mutex::new(Some(release_rx)),
        });
        let (bus, _events) = event_channel();
        let supervisor = Supervisor::start(registry, bus, Arc::new(Logs::disabled()));
        let mut state = CoreState::new(std::path::PathBuf::from(r"C:\\repo"));
        state.attach_supervisor(supervisor);
        state.config = json!({"pet": {"bag_grid": [0, 0, 80, 80], "page_tabs": [[10, 10]]}});
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
            Request::new(
                "capture-lease",
                "pet.capture_icon",
                json!({"kind": "queue", "page": 1, "cell": 1}),
            ),
        );
        let error = response.error.expect("the capture is refused while busy");
        assert_eq!(error.code, "tool_busy");
        assert_eq!(error.detail["command"], "pet.capture_icon");
        assert!(error.message.contains("\"tuner\""), "{}", error.message);

        let _ = release.send(());
        state.shutdown_tools("test");
    }

    #[test]
    fn scan_food_missing_icon_is_refused_before_any_capture_or_evidence() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("scan-no-evidence-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut state = CoreState::new(root.clone());
        state.config = json!({
            "pet": {"bag_grid": [0, 0, 80, 80], "page_tabs": [[10, 10]]}
        });
        state.logs = Arc::new(Logs::new(&root));
        let _ = handle(
            &mut state,
            Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"})),
        );
        let response = handle(
            &mut state,
            Request::new("scan", "pet.scan_food_page", json!({"page": 1})),
        );
        let error = response.error.expect("the missing icon is refused");
        assert_eq!(error.code, "bad_request");
        assert_eq!(error.detail["field"], "food_icon_png");
        assert!(
            !root.join("logs/reads").exists(),
            "no capture/evidence file was written before the icon check"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// CORE-F4: a worker's mid-run config write must survive the next read-only command. The
    /// dispatcher used to republish its stale copy after every command and overwrite the mirror.
    #[test]
    fn a_read_only_command_does_not_republish_over_a_worker_config_write() {
        let mut state = CoreState::new(std::path::PathBuf::from(r"C:\repo"));
        let _ = handle(
            &mut state,
            Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"})),
        );
        let worker_config = json!({"pet": {"food_slots_used": 5}});
        *state.shared_config.write().expect("not poisoned") = Arc::new(worker_config.clone());
        state.revision.fetch_add(1, Ordering::SeqCst);

        let _ = handle(&mut state, Request::new("2", "board.details", json!({})));

        assert_eq!(
            state.config["pet"]["food_slots_used"], 5,
            "the dispatcher refreshed its config from the worker mirror"
        );
        assert_eq!(
            **state.shared_config.read().expect("not poisoned"),
            worker_config,
            "a read-only command must not republish the old dispatcher copy"
        );
    }

    #[test]
    fn bump_revision_uses_the_shared_counter_not_the_local_snapshot() {
        let mut state = CoreState::new(std::path::PathBuf::from(r"C:\repo"));
        state.rev = 3;
        state.revision.store(9, Ordering::SeqCst);
        assert_eq!(state.bump_revision(), 10);
        assert_eq!(state.rev, 10);
        assert_eq!(state.revision.load(Ordering::SeqCst), 10);
    }

    /// CORE-F5: the grade crop must use the captured IMAGE size, not the area's own width/height.
    /// With the old shadowed values an area at (50,50) was cropped out of a 20×20 "image" and the
    /// colour fallback silently never ran.
    #[test]
    fn the_grade_colour_crop_uses_the_image_dimensions() {
        let mut rgba = vec![0u8; 100 * 100 * 4];
        for y in 50..70 {
            for x in 50..70 {
                let offset = (y * 100 + x) * 4;
                rgba[offset] = 200;
                rgba[offset + 1] = 100;
                rgba[offset + 2] = 50;
                rgba[offset + 3] = 255;
            }
        }
        let area = seal_ocr::modes::GradeArea {
            x1: 50,
            y1: 50,
            x2: 70,
            y2: 70,
        };
        let (width, height, pixels) =
            crop_grade_area(&area, 100, 100, &rgba).expect("a valid crop inside the image");
        assert_eq!((width, height), (20, 20));
        assert_eq!(&pixels[0..4], &[200, 100, 50, 255]);
    }

    /// CORE-F7: the cached OCR pipeline bakes in the active dictionary's `text_fixes` (its cleaner)
    /// and `tuner.ocr.row_height` (line bucketing). Every trigger the review names — a variant switch,
    /// `config.reload`, `calibration.tuner.save` — changes one of those, so the cache must be
    /// recognised as stale; before this nothing ever cleared it and the calibrator's test reads kept
    /// the old dictionary until the core restarted.
    #[test]
    fn a_config_change_makes_the_cached_ocr_pipeline_stale() {
        let mut state = CoreState::new(std::path::PathBuf::from(r"C:\repo"));
        state.config = json!({"tuner": {"ocr": {"row_height": 10}}});
        state.dictionary = "attributes.tw.yaml".to_owned();
        state.ocr_build = Some(state.ocr_identity());
        assert!(
            !state.ocr_is_stale(),
            "a cache built from this config is current"
        );

        // A variant switch changes the dictionary file and its text fixes.
        state.dictionary = "attributes.us.yaml".to_owned();
        assert!(
            state.ocr_is_stale(),
            "a variant switch must rebuild the pipeline"
        );

        state.ocr_build = Some(state.ocr_identity());
        assert!(!state.ocr_is_stale());
        // `calibration.tuner.save` derives and writes a new row height.
        state.config["tuner"]["ocr"]["row_height"] = json!(14);
        assert!(
            state.ocr_is_stale(),
            "a tuner calibration must rebuild the pipeline"
        );

        state.ocr_build = Some(state.ocr_identity());
        assert!(!state.ocr_is_stale());
        // `config.reload` can change only the cleanup table, with the same dictionary file name.
        state.attributes.text_fixes.whole.push(TextFix {
            from: "攻孽".to_owned(),
            to: "攻擊".to_owned(),
        });
        assert!(
            state.ocr_is_stale(),
            "an edited text-fix table must rebuild the cleaner"
        );
    }
}

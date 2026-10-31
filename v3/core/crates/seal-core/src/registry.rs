//! The production registry: where a tool's decision half and its effects come from.
//!
//! The first live tool is the **spammer**: its decision half is `seal-domain`'s, its effects are the
//! board's key grammar through `seal-board`, and its opening phase opens the serial port. Every other
//! tool still answers with the named "not available in this build yet" line rather than accepting a
//! start and doing nothing — "wired up and does nothing when Start is pressed" is the v2 failure
//! class (`PROGRESS.md` entry 59: the quest tool looked wired and did nothing).
//!
//! The registry reads the config through a [`SharedConfig`] cell the dispatcher publishes after every
//! command that can change it, so `available`/`create` see the same merged config the shell sees and a
//! variant switch or preset save takes effect on the next Start. `create` runs in the worker: for the
//! spammer that is opening the board, which is why it is not on the supervisor's thread.

use std::sync::{Arc, RwLock};

use seal_board::command::BoardCommand;
use seal_board::serial::SerialTransport;
use seal_board::transport::Transport;
use seal_domain::decision::Key;
use seal_domain::geometry::Point;
use seal_domain::tools::spammer::SpammerPlan;
use seal_tools::game::GamePort;
use seal_tools::runner::{EffectError, Effects, Tool};
use seal_tools::tool::ToolId;
use serde_json::Value;

use seal_config::ConfigStore;

use crate::board::{BoardFailure, Settings, open_board};
use crate::gem::{GemResultSource, GemTool};
use crate::holdspace::{HoldSpaceTool, SpaceHoldEffects};
use crate::pet::{PetReadSource, PetTool};
use crate::pet_state::PetStateSink;
use crate::quest::{QuestEffects, QuestTool, WinQuestSource};
use crate::shop::ShopTool;
use crate::spammer::{RealNow, SpammerTool};
use crate::tuner::TunerTool;

/// The merged config, shared between the dispatcher (publisher) and the registry (reader).
pub type SharedConfig = Arc<RwLock<Value>>;

/// One run's two halves: the decision half the runner calls, and the effects it performs decisions
/// with. They arrive together because a run cannot exist with only one of them.
pub struct Session {
    pub tool: Box<dyn Tool + Send>,
    pub effects: Box<dyn Effects + Send>,
}

/// A start refused before it becomes a run, with the line the player sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable {
    pub line: String,
}

/// Where tools come from.
///
/// [`Self::available`] is the cheap gate the supervisor checks BEFORE the arbiter accepts a start, so
/// an unavailable tool gets a clean refusal instead of an opening phase that immediately fails.
/// [`Self::create`] runs in the worker: for a real registry it is the OPENING phase (open the board,
/// load the OCR models), which is why it is not on the supervisor's thread.
pub trait ToolRegistry: Send + Sync {
    fn available(&self, tool: ToolId) -> Result<(), Unavailable>;
    /// Build one run. The `game` port is the capability a resident tool uses to ask for the game and
    /// give it back; tools that take the game by `start` ignore it.
    fn create(
        &self,
        tool: ToolId,
        game: Arc<dyn GamePort>,
        args: &Value,
    ) -> Result<Session, String>;
}

/// The line every unavailable tool reports. Defined where v3 has no player-facing text yet: the tool
/// is known to the protocol, but its loop is not ported in this build.
pub fn not_available_line(tool: ToolId) -> String {
    format!("\"{tool}\" is not available in this build yet — its tool loop is not ported.")
}

/// The binary's registry: every tool id has a live session; a tool's own plan can still refuse its
/// opening phase with the player-facing line, which is the domain's job, not a placeholder's. The
/// store is here because the tuner's opening phase loads the attribute dictionary the active client
/// names — the same load `config.apply` performs before a variant switch.
pub struct ProductionRegistry {
    shared: SharedConfig,
    store: ConfigStore,
    /// The per-run file logger: the hold-space release failure is the one effect-level failure that
    /// has no card message left to reach (it happens in `Drop`), so the registry hands it the logger.
    logs: Arc<crate::logs::Logs>,
    /// The mid-run pet run-state sink (A1). `None` in registry tests that never start the feeder.
    pet_state: Option<Arc<dyn PetStateSink>>,
    /// The binary's memory-only firmware cache, filled by the pet session's opening `V` detection.
    firmware: Option<Arc<RwLock<Option<u32>>>>,
}

impl ProductionRegistry {
    pub fn new(shared: SharedConfig, store: ConfigStore, logs: Arc<crate::logs::Logs>) -> Self {
        Self {
            shared,
            store,
            logs,
            pet_state: None,
            firmware: None,
        }
    }

    /// Attach the production pet run-state sink. The binary does this before the supervisor starts;
    /// the registry's tests leave it absent.
    pub fn with_pet_state_sink(mut self, sink: Arc<dyn PetStateSink>) -> Self {
        self.pet_state = Some(sink);
        self
    }

    /// Attach the binary's firmware cache, so the pet session's opening `V` read also feeds
    /// `board.details` (no extra query per open).
    pub fn with_firmware_cache(mut self, cache: Arc<RwLock<Option<u32>>>) -> Self {
        self.firmware = Some(cache);
        self
    }

    fn config(&self) -> Value {
        self.shared
            .read()
            .map(|config| config.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    fn buy_session(&self, args: &Value) -> Result<Session, String> {
        let config = self.config();
        let preset = args.get("preset").and_then(Value::as_str);
        let count = args
            .get("count")
            .and_then(Value::as_i64)
            .and_then(|count| i32::try_from(count).ok());
        let plan =
            crate::shop::buy_plan(&config, preset, count).map_err(|reason| reason.message)?;
        let settings = Settings::from_config(&config);
        let (_port, transport) = match open_board(&settings) {
            Ok(opened) => opened,
            Err(BoardFailure::Unavailable(line)) | Err(BoardFailure::BadRequest(line)) => {
                return Err(line);
            }
        };
        // The transaction places the cursor through the closed loop, which needs the display the
        // window was measured in; a missing window is the domain's own line.
        let title = crate::win::configured_title(&config).map_err(|error| error.line())?;
        let hwnd = seal_win::window::find_window(&title)
            .ok_or_else(|| seal_domain::tools::shop::window_missing().message)?;
        let display = seal_win::window::measure(hwnd).map_err(|error| error.to_string())?;
        let effects = crate::effects::system_effects(transport, display);
        Ok(Session {
            tool: Box::new(ShopTool::new(ToolId::Buy, plan)),
            effects: Box::new(effects),
        })
    }
    fn sell_session(&self, args: &Value) -> Result<Session, String> {
        let config = self.config();
        // The per-run selection lives only in the start arguments; it never touches the config.
        let slots = run_slots(args)?;
        let plan = crate::shop::sell_plan_with_slots(&config, slots.as_deref())
            .map_err(|reason| reason.message)?;
        let settings = Settings::from_config(&config);
        let (_port, transport) = match open_board(&settings) {
            Ok(opened) => opened,
            Err(BoardFailure::Unavailable(line)) | Err(BoardFailure::BadRequest(line)) => {
                return Err(line);
            }
        };
        // The transaction places the cursor through the closed loop, which needs the display the
        // window was measured in; a missing window is the domain's own line.
        let title = crate::win::configured_title(&config).map_err(|error| error.line())?;
        let hwnd = seal_win::window::find_window(&title)
            .ok_or_else(|| seal_domain::tools::shop::window_missing().message)?;
        let display = seal_win::window::measure(hwnd).map_err(|error| error.to_string())?;
        let effects = crate::effects::system_effects(transport, display);
        Ok(Session {
            tool: Box::new(ShopTool::new(ToolId::Sell, plan)),
            effects: Box::new(effects),
        })
    }

    fn quest_session(&self, args: &Value) -> Result<Session, String> {
        let config = self.config();
        let test = args.get("test").and_then(Value::as_bool).unwrap_or(false);
        let plan = crate::quest::resolve_with(&config, test.then_some(1))
            .map_err(|reason| reason.message)?;
        let settings = Settings::from_config(&config);
        let (_port, transport) = match open_board(&settings) {
            Ok(opened) => opened,
            Err(BoardFailure::Unavailable(line)) | Err(BoardFailure::BadRequest(line)) => {
                return Err(line);
            }
        };
        Ok(Session {
            tool: Box::new(QuestTool::new(plan, WinQuestSource)),
            effects: Box::new(QuestEffects::new(transport)),
        })
    }

    fn tuner_session(&self) -> Result<Session, String> {
        let config = self.config();
        let root = self
            .store
            .config_dir()
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let plan = crate::tuner::plan(&config)?;
        let settings = Settings::from_config(&config);
        let (_port, transport) = match open_board(&settings) {
            Ok(opened) => opened,
            Err(BoardFailure::Unavailable(line)) | Err(BoardFailure::BadRequest(line)) => {
                return Err(line);
            }
        };
        let active = crate::startup::active(&config);
        let attributes = crate::startup::load_dictionary(&self.store, &active.dictionary)
            .map_err(|error| error.to_string())?;
        let (source, display) =
            crate::tuner::OcrTunerSource::open(&config, attributes, plan.region, &root)?;
        let effects = crate::effects::system_effects(transport, display);
        let (click_enter_delay, ocr_delay) = crate::tuner::timings(&config);
        let tool = TunerTool::new(
            plan,
            source,
            display,
            Box::new(crate::tuner::RealNow::start()),
        )
        .with_timings(click_enter_delay, ocr_delay);
        Ok(Session {
            tool: Box::new(tool),
            effects: Box::new(effects),
        })
    }

    fn gem_session(&self) -> Result<Session, String> {
        let config = self.config();
        let plan = crate::gem::plan(&config)?;
        let settings = Settings::from_config(&config);
        let (_port, transport) = match open_board(&settings) {
            Ok(opened) => opened,
            Err(BoardFailure::Unavailable(line)) | Err(BoardFailure::BadRequest(line)) => {
                return Err(line);
            }
        };
        // The reference crop lives beside the other calibration artifacts, under the root. The store
        // owns the config directory; its parent is that root (`root::find_root`).
        let root = self
            .store
            .config_dir()
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let source = GemResultSource::open(&config, &plan, &root, Arc::clone(&self.logs))?;
        let tool = GemTool::new(plan, source)?;
        // The composer clicks calibrated points, so it needs the display the clicks are measured in.
        let title = crate::win::configured_title(&config).map_err(|error| error.line())?;
        let hwnd = seal_win::window::find_window(&title)
            .ok_or_else(|| seal_domain::tools::gem::window_missing().message)?;
        let display = seal_win::window::measure(hwnd).map_err(|error| error.to_string())?;
        let effects = crate::effects::system_effects(transport, display);
        Ok(Session {
            tool: Box::new(tool),
            effects: Box::new(effects),
        })
    }

    fn pet_session(&self, game: Arc<dyn GamePort>) -> Result<Session, String> {
        let config = self.config();
        let root = self
            .store
            .config_dir()
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let plan = crate::pet::plan(&config, &root)?;
        let settings = Settings::from_config(&config);
        let (_port, mut transport) = match open_board(&settings) {
            Ok(opened) => opened,
            Err(BoardFailure::Unavailable(line)) | Err(BoardFailure::BadRequest(line)) => {
                return Err(line);
            }
        };
        // The board's firmware level decides whether the configured food-load mode can work at all;
        // a level-1 board in drag mode is refused with the domain's sentence by the source's opening
        // phase (the check is `food_load::complaint`, not a second rule here).
        let firmware = seal_board::firmware::detect(&mut transport);
        if let Some(cache) = &self.firmware {
            // Silence is a definite level-1 answer for the CACHE (probe already reports it that
            // way); the run's behaviour still treats it as "no version" via `firmware_level` None.
            *cache.write().expect("not poisoned") =
                Some(crate::board::cached_firmware_level(firmware));
        }
        let firmware_level = firmware.reported().map(|level| level as i32);
        let active = crate::startup::active(&config);
        let attributes = crate::startup::load_dictionary(&self.store, &active.dictionary)
            .map_err(|error| error.to_string())?;
        let source = PetReadSource::open(&config, &plan, &attributes, firmware_level, &root)?;
        let mut tool = PetTool::new(plan, source, game);
        if let Some(sink) = &self.pet_state {
            tool = tool.with_state_sink(Arc::clone(sink));
        }
        // The feeder clicks calibrated points, so it needs the display the clicks are measured in.
        let title = crate::win::configured_title(&config).map_err(|error| error.line())?;
        let hwnd = seal_win::window::find_window(&title)
            .ok_or_else(|| format!("game window not found — is {title:?} running?"))?;
        let display = seal_win::window::measure(hwnd).map_err(|error| error.to_string())?;
        let effects = crate::effects::system_effects(transport, display);
        Ok(Session {
            tool: Box::new(tool),
            effects: Box::new(effects),
        })
    }

    fn hold_space_session(&self) -> Result<Session, String> {
        let config = self.config();
        let settings = Settings::from_config(&config);
        let (_port, transport) = match open_board(&settings) {
            Ok(opened) => opened,
            Err(BoardFailure::Unavailable(line)) | Err(BoardFailure::BadRequest(line)) => {
                return Err(line);
            }
        };
        Ok(Session {
            tool: Box::new(HoldSpaceTool::new()),
            effects: Box::new(SpaceHoldEffects::new(transport, Arc::clone(&self.logs))),
        })
    }

    fn spammer_session(&self) -> Result<Session, String> {
        let config = self.config();
        let plan: SpammerPlan = crate::spammer::plan(&config)?;
        let unsupported = crate::spammer::unsupported_keys(&config);
        if !unsupported.is_empty() {
            return Err(unsupported.join(" "));
        }
        let settings = Settings::from_config(&config);
        let (_port, transport) = match open_board(&settings) {
            Ok(opened) => opened,
            Err(BoardFailure::Unavailable(line)) | Err(BoardFailure::BadRequest(line)) => {
                return Err(line);
            }
        };
        let effects = KeyOnlyEffects { transport };
        let tool = SpammerTool::new(&plan, Box::new(RealNow::start()));
        Ok(Session {
            tool: Box::new(tool),
            effects: Box::new(effects),
        })
    }
}

impl ToolRegistry for ProductionRegistry {
    fn available(&self, tool: ToolId) -> Result<(), Unavailable> {
        match tool {
            ToolId::Spammer => crate::spammer::plan(&self.config())
                .map(|_| ())
                .map_err(|line| Unavailable { line }),
            // The hold needs nothing but a board, and the board is the OPENING phase's business
            // (`create`), so this cheap gate always passes: a start with no board gets the opening
            // phase's own line rather than a refusal invented here.
            ToolId::HoldSpace => Ok(()),
            ToolId::Tuner => crate::tuner::plan(&self.config())
                .map(|_| ())
                .map_err(|line| Unavailable { line }),
            ToolId::Quest => crate::quest::resolve(&self.config())
                .map(|_| ())
                .map_err(|reason| Unavailable {
                    line: reason.message,
                }),
            ToolId::Sell => {
                crate::shop::sell_available(&self.config()).map_err(|reason| Unavailable {
                    line: reason.message,
                })
            }
            ToolId::Gem => crate::gem::plan(&self.config())
                .map(|_| ())
                .map_err(|line| Unavailable { line }),
            ToolId::Pet => {
                let root = self
                    .store
                    .config_dir()
                    .parent()
                    .map(std::path::Path::to_path_buf)
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                crate::pet::plan(&self.config(), &root)
                    .map(|_| ())
                    .map_err(|line| Unavailable { line })
            }
            // Buy is served; a missing run argument is a CREATE-time refusal so it can name the
            // reason (the domain line for no preset / unknown preset).
            ToolId::Buy => Ok(()), // served; args are a create-time concern
        }
    }

    fn create(
        &self,
        tool: ToolId,
        game: Arc<dyn GamePort>,
        args: &Value,
    ) -> Result<Session, String> {
        match tool {
            ToolId::Spammer => self.spammer_session(),
            ToolId::HoldSpace => self.hold_space_session(),
            ToolId::Tuner => self.tuner_session(),
            ToolId::Quest => self.quest_session(args),
            ToolId::Sell => self.sell_session(args),
            ToolId::Gem => self.gem_session(),
            ToolId::Pet => self.pet_session(game),
            ToolId::Buy => self.buy_session(args),
        }
    }
}

/// The spammer's effects: it presses keys and never touches the cursor. A click is an error rather
/// than a silent no-op, so a future spammer change that starts clicking fails loudly.
struct KeyOnlyEffects {
    transport: SerialTransport,
}

impl KeyOnlyEffects {
    fn write(&mut self, key: Key, fast: bool) -> Result<(), EffectError> {
        write_key(&mut self.transport, key, fast)
    }
}

/// The one place a spammer key becomes a wire line, over the transport seam so a test can watch it.
fn write_key(transport: &mut dyn Transport, key: Key, fast: bool) -> Result<(), EffectError> {
    let command = match key {
        Key::Char(character) => BoardCommand::Key { character, fast },
        Key::Function(number) => BoardCommand::FunctionKey { number, fast },
    };
    transport
        .write_line(&command.encode())
        .map_err(|error| EffectError::new(error.message()))
}

impl Effects for KeyOnlyEffects {
    fn click(&mut self, _at: Point) -> Result<(), EffectError> {
        Err(EffectError::new(
            "the spammer presses keys and does not move the cursor",
        ))
    }

    fn key(&mut self, key: Key) -> Result<(), EffectError> {
        self.write(key, false)
    }

    fn key_fast(&mut self, key: Key) -> Result<(), EffectError> {
        self.write(key, true)
    }
}

/// The validated sell selection from `tool.start`; `None` means the run has no selection. The
/// dispatcher already rejects malformed shapes; this re-checks at the creation seam so a direct
/// caller cannot silently fall back to a persisted selection.
fn run_slots(args: &Value) -> Result<Option<Vec<i32>>, String> {
    let Some(value) = args.get("slots") else {
        return Ok(None);
    };
    let Some(values) = value.as_array() else {
        return Err("tool.start's \"slots\" must be an array of integers 0..63".to_owned());
    };
    if values.is_empty() {
        return Err("tool.start's \"slots\" must contain at least one bag slot".to_owned());
    }
    let mut slots = Vec::with_capacity(values.len());
    for value in values {
        let Some(slot) = value.as_i64().and_then(|slot| i32::try_from(slot).ok()) else {
            return Err("tool.start's \"slots\" must contain only integers 0..63".to_owned());
        };
        if !(0..=63).contains(&slot) {
            return Err("tool.start's \"slots\" must contain only integers 0..63".to_owned());
        }
        slots.push(slot);
    }
    Ok(Some(slots))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    fn registry(config: Value) -> ProductionRegistry {
        ProductionRegistry::new(
            Arc::new(RwLock::new(config)),
            seal_config::ConfigStore::new(std::path::PathBuf::from(r"C:\not-used-by-these-tests")),
            Arc::new(crate::logs::Logs::disabled()),
        )
    }

    fn config_with_tuner() -> Value {
        serde_json::json!({
            "window": {"title": "TW_LIVE"},
            "tuner": {
                "grade_order": ["N", "G", "DG"],
                "target_grade": "DG",
                "ocr": {"region": {"left": 10, "top": 20, "width": 300, "height": 320}},
                "spring_mode": "manual",
            },
            "filter": {"enabled": false},
        })
    }

    fn config_with_shop() -> Value {
        serde_json::json!({
            "window": {"title": "TW_LIVE"},
            "buy_sell": {
                "bag_grid": [0, 0, 800, 600],
                "bag_slot": [0, 0, 100, 75],
                "shop_rows": 5,
                "shop_region": [900, 400, 300, 250],
                "scroll_point": [1000, 420],
                "max_button": [1200, 700],
                "sell_slots": [3, 1],
                "sell_cap": 5
            }
        })
    }

    #[test]
    fn the_sell_tool_is_available_without_a_persisted_selection_and_buy_is_served() {
        assert_eq!(registry(config_with_shop()).available(ToolId::Sell), Ok(()));
        // Buy is served now: its missing PRESET is a create-time refusal (so it can carry the
        // domain's line), not an availability refusal.
        assert_eq!(registry(config_with_shop()).available(ToolId::Buy), Ok(()));
        assert_eq!(
            registry(serde_json::json!({})).available(ToolId::Buy),
            Ok(()),
            "availability does not depend on presets/calibration; create answers those"
        );

        // Everything calibrated, but nothing selected: the per-run selection is run data, so the
        // cheap availability gate must let the start through; `create` answers the domain's own
        // selection line rather than the dispatcher inventing a bad_request.
        let without_selection = registry(serde_json::json!({
            "buy_sell": {
                "bag_grid": [0, 0, 800, 600],
                "max_button": [1200, 700],
                "scroll_point": [1000, 420],
                // A stale v3.0.1 Save-selection value: it must still not be a run fallback.
                "sell_slots": [3, 1],
                "sell_cap": 5
            }
        }));
        assert_eq!(
            without_selection.available(ToolId::Sell),
            Ok(()),
            "a saved sell_slots value must not refuse a start that may carry slots"
        );
        let error = match without_selection.create(
            ToolId::Sell,
            Arc::new(seal_tools::game::AllowAll),
            &serde_json::json!({}),
        ) {
            Ok(_) => panic!("a selection-less sell start has nothing to sell"),
            Err(error) => error,
        };
        assert_eq!(error, "No slots are selected to sell.");

        let registry = registry(config_with_shop());
        let error = match registry.create(
            ToolId::Buy,
            Arc::new(seal_tools::game::AllowAll),
            &serde_json::json!({}),
        ) {
            Ok(_) => panic!("an argument-less buy start has no preset"),
            Err(error) => error,
        };
        assert_eq!(error, "No buy preset selected.");
    }

    fn config_with_gem() -> Value {
        serde_json::json!({
            "window": {"title": "TW_LIVE"},
            "gem": {
                "grades": ["N", "G"],
                "start_grade": "N",
                "move_mode": "arduino",
                "empty_mode": "stop",
                "grade_positions": {
                    "N": [10, 10], "G": [20, 20],
                    "Register": [40, 40], "Combine": [50, 50]
                },
                "resource_gems": [[60, 60], [70, 70], [80, 80]]
            },
        })
    }

    #[test]
    fn the_gem_composer_is_available_once_its_calibration_resolves() {
        assert_eq!(registry(config_with_gem()).available(ToolId::Gem), Ok(()));
        // No calibrated grade button: the domain's own calibration line, before any opening work.
        let refusal = registry(serde_json::json!({
            "gem": {
                "grades": ["N"],
                "start_grade": "N",
                "move_mode": "arduino",
                "empty_mode": "stop",
                "grade_positions": {}
            }
        }))
        .available(ToolId::Gem)
        .expect_err("an uncalibrated gem button is refused");
        assert!(
            refusal
                .line
                .contains("Grade position 'N' isn't calibrated yet"),
            "{}",
            refusal.line
        );
        assert!(refusal.line.contains("Grade point"), "{}", refusal.line);
    }

    #[test]
    fn a_gem_run_that_reads_without_a_result_area_is_refused() {
        let refusal = registry(serde_json::json!({
            "gem": {
                "grades": ["N"],
                "start_grade": "N",
                "move_mode": "arduino",
                "empty_mode": "advance_grade",
                "grade_positions": {"N": [10, 10], "Register": [40, 40], "Combine": [50, 50]},
                "empty_signature": {
                    "mean_v": 0.3, "mean_sat": 0.1, "colored_fraction": 0.05,
                    "mean_r": 0.3, "mean_g": 0.3, "mean_b": 0.3
                }
            }
        }))
        .available(ToolId::Gem)
        .expect_err("the read has nowhere to capture");

        assert!(
            refusal.line.contains("gem.result_gem_area"),
            "{}",
            refusal.line
        );
    }

    fn config_with_pet() -> Value {
        let empty_slot = base64::engine::general_purpose::STANDARD.encode(
            seal_win::pure::encode_png(2, 2, &[0, 0, 255, 255].repeat(4)).expect("encodes"),
        );
        serde_json::json!({
            "window": {"title": "TW_LIVE"},
            "buy_sell": {"max_button": [900, 500]},
            "pet": {
                "menu_button": [10, 10],
                "feed_icon": [20, 20],
                "close_button": [30, 30],
                "page_tabs": [[100, 100]],
                "bag_grid": [0, 0, 400, 400],
                "bag_slot": [0, 0, 40, 40],
                "food_slots": [[0, 0], [0, 1]],
                "return_slot": [0, 1],
                "food_load_mode": "right_click",
                "items_per_minute": 3,
                "pet_slot_occupied_above": 0.05,
                "max_button": [900, 500],
                "slots": [{
                    "enabled": true,
                    "toggle_label": [5, 5, 40, 20],
                    "boarding_pet_slot": [50, 60, 30, 40],
                    "feeder_strip": [100, 200, 300, 50],
                    "stacks": 2,
                    "pet_slot_empty_png": empty_slot,
                    "boarding_running": false
                }]
            }
        })
    }

    #[test]
    fn the_pet_feeder_is_available_once_its_marks_resolve() {
        assert_eq!(registry(config_with_pet()).available(ToolId::Pet), Ok(()));
        // No calibrated 目錄 button: the domain's own start line, before any opening work.
        let refusal = registry(serde_json::json!({"pet": {}}))
            .available(ToolId::Pet)
            .expect_err("an uncalibrated pet setup is refused");
        assert!(
            refusal.line.contains("pet menu button isn't calibrated"),
            "{}",
            refusal.line
        );
        assert!(refusal.line.contains("Calibrate tab"), "{}", refusal.line);
    }

    #[test]
    fn a_drag_food_load_config_is_available_but_refused_on_a_level_one_board() {
        let mut config = config_with_pet();
        config["pet"]["food_load_mode"] = serde_json::json!("drag");
        // `available` cannot see the board (it is opened in `create`), so the plan-level gate passes;
        // the level-1 refusal is the domain's sentence, exercised through the same helper `create`
        // calls during its opening phase.
        assert_eq!(registry(config.clone()).available(ToolId::Pet), Ok(()));
        let plan = crate::pet::plan(&config, std::path::Path::new(".")).expect("the plan resolves");
        let line = crate::pet::check_board(&plan, Some(1)).expect_err("firmware 1 has no drag");
        assert!(line.contains("firmware 1"), "{line}");
        assert!(line.contains("reflash the board"), "{line}");
        assert_eq!(crate::pet::check_board(&plan, Some(2)), Ok(()));
    }
    fn config_with_quest() -> Value {
        serde_json::json!({
            "window": {"title": "TW_LIVE"},
            "quest": {
                "active": "run1",
                "guard_px": 8,
                "presets": {"run1": {"initial_wait_seconds": 1.0, "loops": 1, "steps": [{"action": "click"}]}},
            },
        })
    }

    #[test]
    fn the_quest_tool_is_available_once_its_flow_validates() {
        assert_eq!(
            registry(config_with_quest()).available(ToolId::Quest),
            Ok(())
        );
        let refusal = registry(serde_json::json!({"quest": {"presets": {}}}))
            .available(ToolId::Quest)
            .expect_err("a flow with no steps is refused");
        assert!(refusal.line.contains("has no steps"), "{}", refusal.line);
    }

    #[test]
    fn the_tuner_is_available_once_its_region_resolves() {
        assert_eq!(
            registry(config_with_tuner()).available(ToolId::Tuner),
            Ok(())
        );
        let refusal = registry(config_with_spammer())
            .available(ToolId::Tuner)
            .expect_err("no calibrated region in that fixture");
        assert!(
            refusal.line.contains("tuner.ocr.region"),
            "{}",
            refusal.line
        );
    }

    fn config_with_spammer() -> Value {
        serde_json::json!({
            "spammer": {"active": "buff", "presets": {"buff": {"1": 1.0}}},
            "arduino": {"vid": 9025, "pid": [32822], "baud": 115200, "port": ""},
        })
    }

    #[test]
    fn the_spammer_is_available_once_its_preset_resolves() {
        assert_eq!(
            registry(config_with_spammer()).available(ToolId::Spammer),
            Ok(())
        );
    }

    #[test]
    fn a_broken_preset_is_refused_with_the_domain_line_before_any_opening_work() {
        let registry = registry(serde_json::json!({
            "spammer": {"active": "missing", "presets": {"buff": {"1": 1.0}}},
        }));

        let refusal = registry
            .available(ToolId::Spammer)
            .expect_err("no such preset");

        assert!(refusal.line.contains("was not found"), "{}", refusal.line);
        assert!(refusal.line.contains("Spammer tab"), "{}", refusal.line);
    }

    #[test]
    fn the_hold_space_tool_is_available() {
        assert_eq!(
            registry(config_with_spammer()).available(ToolId::HoldSpace),
            Ok(())
        );
    }

    #[test]
    fn every_tool_is_now_served_or_refused_by_its_own_plan() {
        // The pet was the LAST unported loop, so no id answers the generic not-ported line any more.
        // A refused start is the tool's own plan/calibration sentence instead.
        for tool in ToolId::ALL {
            match registry(config_with_spammer()).available(tool) {
                Ok(()) => {}
                Err(refusal) => assert_ne!(
                    refusal.line,
                    not_available_line(tool),
                    "{tool} still says it is not ported"
                ),
            }
        }
    }

    #[test]
    fn a_rotation_with_an_unsendable_key_refuses_at_create_before_opening_the_board() {
        let registry = registry(serde_json::json!({
            "spammer": {"active": "buff", "presets": {"buff": {"1": 1.0, "ab": 2.0}}},
            "arduino": {"vid": 9025, "pid": [32822], "baud": 115200, "port": ""},
        }));

        // `available` only resolves the plan; the key check is the opening phase's.
        assert_eq!(registry.available(ToolId::Spammer), Ok(()));
        let error = registry
            .create(
                ToolId::Spammer,
                Arc::new(seal_tools::game::AllowAll),
                &serde_json::json!({}),
            )
            .err()
            .expect("a bad key refuses the run");
        assert!(error.contains("'ab'"), "{error}");
    }

    #[test]
    fn a_slow_key_is_k_and_a_fast_key_is_lowercase_k() {
        let mut transport = seal_board::transport::FakeTransport::new();

        write_key(&mut transport, Key::Char('1'), false).expect("slow key");
        write_key(&mut transport, Key::Function(2), true).expect("fast function key");

        assert_eq!(transport.writes(), vec!["K 1\n", "f 2\n"]);
    }
}

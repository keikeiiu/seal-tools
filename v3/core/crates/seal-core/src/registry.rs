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
use seal_tools::runner::{EffectError, Effects, Tool};
use seal_tools::tool::ToolId;
use serde_json::Value;

use crate::board::{BoardFailure, Settings, open_board};
use crate::holdspace::{HoldSpaceTool, SpaceHoldEffects};
use crate::spammer::{RealNow, SpammerTool};

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
    fn create(&self, tool: ToolId) -> Result<Session, String>;
}

/// The line every unavailable tool reports. Defined where v3 has no player-facing text yet: the tool
/// is known to the protocol, but its loop is not ported in this build.
pub fn not_available_line(tool: ToolId) -> String {
    format!("\"{tool}\" is not available in this build yet — its tool loop is not ported.")
}

/// The binary's registry: the spammer is live, everything else is refused by name.
pub struct ProductionRegistry {
    shared: SharedConfig,
}

impl ProductionRegistry {
    pub fn new(shared: SharedConfig) -> Self {
        Self { shared }
    }

    fn config(&self) -> Value {
        self.shared
            .read()
            .map(|config| config.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
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
            effects: Box::new(SpaceHoldEffects::new(transport)),
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
            other => Err(Unavailable {
                line: not_available_line(other),
            }),
        }
    }

    fn create(&self, tool: ToolId) -> Result<Session, String> {
        match tool {
            ToolId::Spammer => self.spammer_session(),
            ToolId::HoldSpace => self.hold_space_session(),
            other => Err(not_available_line(other)),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(config: Value) -> ProductionRegistry {
        ProductionRegistry::new(Arc::new(RwLock::new(config)))
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
    fn every_other_tool_says_it_is_not_ported() {
        for tool in ToolId::ALL {
            if matches!(tool, ToolId::Spammer | ToolId::HoldSpace) {
                continue;
            }
            let refusal = registry(config_with_spammer())
                .available(tool)
                .expect_err("only the spammer is live");
            assert_eq!(refusal.line, not_available_line(tool), "{tool}");
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
            .create(ToolId::Spammer)
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

//! The second live tool: **Hold Space**, the spacebar hold.
//!
//! v2's `HoldSpace.cs` ran a forever loop watching the launcher's `state.Running` mirror and its own
//! F12 edge, sending `P` (press and hold) and `U` (release). In v3 the START and STOP *are* the run's
//! lifecycle — the supervisor starts it, `tool.stop` cancels it — so the loop's two external inputs
//! collapse into "the run exists" and the tool is simply:
//!
//! 1. first decision: [`Decision::HoldSpace`] (the wire's `P`);
//! 2. then `Wait` on v2's 50 ms tick until the runner cancels.
//!
//! **The release cannot be forgotten.** The runner drops the effects object when the worker ends
//! (normal stop, `tool.stop`, an effect failure, shutdown), and [`SpaceHoldEffects`]' `Drop` sends `U`
//! if the spacebar is still held — the same rule v2's `finally` implemented, moved to RAII so no exit
//! path can skip it. The firmware's own timeout is the last resort, not the mechanism.
//!
//! The decision half (`seal-domain/src/tools/hold_space.rs`) still owns the ACTION vocabulary
//! (`HoldSpaceAction::wire()` is the `P`/`U` text) and the two failure lines; this module only joins
//! it to the transport and the runner. v2's card-mirror/hotkey edge logic has no v3 counterpart
//! because an F12 edge is a SUPERVISOR concern in v3, not a tool's.

use std::sync::Arc;

use seal_board::transport::Transport;
use seal_domain::decision::Decision;
use seal_domain::geometry::Point;
use seal_domain::tools::hold_space::{HoldSpaceAction, release_failed};
use seal_tools::runner::{EffectError, Effects};
use seal_tools::tool::ToolId;

use crate::logs::Logs;

/// v2's `HoldSpace` loop tick (`HoldSpace.cs`: `SleepCheck(0.05)`).
pub const TICK_SECONDS: f64 = 0.05;

/// The hold loop as the runner sees it.
#[derive(Debug, Default)]
pub struct HoldSpaceTool {
    started: bool,
}

impl HoldSpaceTool {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl seal_tools::runner::Tool for HoldSpaceTool {
    fn id(&self) -> ToolId {
        ToolId::HoldSpace
    }

    fn decide(&mut self) -> Decision {
        if !self.started {
            self.started = true;
            return Decision::HoldSpace;
        }
        // The release is the effects object's Drop, not a decision: the runner never calls `decide`
        // again after a stop, so a decision-based release would be skipped exactly when it matters.
        Decision::Wait(TICK_SECONDS)
    }
}

/// The hold's effects over one transport, with the release-on-drop guard.
///
/// Generic over the transport so the RAII rule can be tested without a board: a fake transport that
/// records the wire text proves the `Drop` release, which is the property the whole tool exists for.
pub struct SpaceHoldEffects<T: Transport> {
    transport: T,
    held: bool,
    logs: Arc<Logs>,
}

impl<T: Transport> SpaceHoldEffects<T> {
    pub fn new(transport: T, logs: Arc<Logs>) -> Self {
        Self {
            transport,
            held: false,
            logs,
        }
    }

    /// A release that failed is the one failure this tool cannot report through a card: it happens in
    /// `Drop`, after the run has ended. The domain's own "tap it once in game" line goes to
    /// `holdspace.log`, where the post-mortem can find it; logging never fails the drop.
    fn log_release_failure(&self, error: &seal_board::transport::TransportError) {
        self.logs.holdspace_line(&release_failed(error.message()));
    }

    fn write(&mut self, action: HoldSpaceAction) -> Result<(), EffectError> {
        self.transport
            .write_line(action.wire())
            .map_err(|error| EffectError::new(error.message()))
    }
}

impl<T: Transport> Effects for SpaceHoldEffects<T> {
    fn click(&mut self, _at: Point) -> Result<(), EffectError> {
        Err(EffectError::new(
            "the Hold Space tool holds the spacebar and does nothing else",
        ))
    }

    fn key(&mut self, _key: seal_domain::decision::Key) -> Result<(), EffectError> {
        Err(EffectError::new(
            "the Hold Space tool holds the spacebar and does nothing else",
        ))
    }

    fn hold_space(&mut self) -> Result<(), EffectError> {
        self.write(HoldSpaceAction::Hold)?;
        self.held = true;
        Ok(())
    }

    fn release_space(&mut self) -> Result<(), EffectError> {
        self.write(HoldSpaceAction::Release)?;
        self.held = false;
        Ok(())
    }
}

impl<T: Transport> Drop for SpaceHoldEffects<T> {
    fn drop(&mut self) {
        if self.held {
            // A `Drop` cannot report to the player, so the failure goes to `holdspace.log` — the
            // diagnostic trail v2's `HoldDiag` wrote — and the firmware's timeout stays the backstop.
            if let Err(error) = self.transport.write_line(HoldSpaceAction::Release.wire()) {
                self.log_release_failure(&error);
            }
            self.held = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use seal_board::transport::{Deadline, LineRead, Transport, TransportError};
    use seal_tools::runner::Tool;

    use super::*;

    /// A transport that records every wire line into shared storage, so the `Drop` release can be
    /// asserted after the effects object is gone.
    struct RecordingTransport {
        writes: Rc<RefCell<Vec<String>>>,
    }

    impl Transport for RecordingTransport {
        fn write_line(&mut self, wire: &str) -> Result<(), TransportError> {
            self.writes.borrow_mut().push(wire.to_owned());
            Ok(())
        }

        fn read_line(&mut self, _deadline: Deadline) -> LineRead {
            LineRead::Silent
        }
    }

    #[test]
    fn the_first_decision_holds_then_the_loop_waits() {
        let mut tool = HoldSpaceTool::new();

        assert_eq!(tool.decide(), Decision::HoldSpace);
        assert_eq!(tool.decide(), Decision::Wait(TICK_SECONDS));
        assert_eq!(tool.decide(), Decision::Wait(TICK_SECONDS));
    }

    #[test]
    fn holding_writes_p_and_releasing_writes_u() {
        let writes = Rc::new(RefCell::new(Vec::new()));
        let mut effects = SpaceHoldEffects::new(
            RecordingTransport {
                writes: writes.clone(),
            },
            Arc::new(Logs::disabled()),
        );

        effects.hold_space().expect("holds");
        assert_eq!(*writes.borrow(), vec!["P\n"]);

        effects.release_space().expect("releases");
        assert_eq!(*writes.borrow(), vec!["P\n", "U\n"]);
    }

    #[test]
    fn dropping_while_held_releases_the_spacebar() {
        let writes = Rc::new(RefCell::new(Vec::new()));
        {
            let mut effects = SpaceHoldEffects::new(
                RecordingTransport {
                    writes: writes.clone(),
                },
                Arc::new(Logs::disabled()),
            );
            effects.hold_space().expect("holds");
        }

        assert_eq!(
            *writes.borrow(),
            vec!["P\n", "U\n"],
            "the guard releases on drop — v2's finally rule as RAII"
        );
    }

    #[test]
    fn dropping_after_a_release_does_not_send_u_twice() {
        let writes = Rc::new(RefCell::new(Vec::new()));
        {
            let mut effects = SpaceHoldEffects::new(
                RecordingTransport {
                    writes: writes.clone(),
                },
                Arc::new(Logs::disabled()),
            );
            effects.hold_space().expect("holds");
            effects.release_space().expect("releases");
        }

        assert_eq!(*writes.borrow(), vec!["P\n", "U\n"]);
    }

    #[test]
    fn clicks_and_keys_are_refused_rather_than_dropped() {
        let mut effects = SpaceHoldEffects::new(
            RecordingTransport {
                writes: Rc::new(RefCell::new(Vec::new())),
            },
            Arc::new(Logs::disabled()),
        );

        assert!(effects.click(Point::new(1, 2)).is_err());
        assert!(effects.key(seal_domain::decision::Key::Char('1')).is_err());
    }
}

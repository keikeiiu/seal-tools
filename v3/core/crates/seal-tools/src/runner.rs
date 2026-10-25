//! The runner seam: the thin loop that turns a tool's decisions into effects, with the fake tool and
//! fake effects driving the whole run in tests.
//!
//! `PLAN-V3-CORE.md` §6.4: "Each tool's decision half becomes a pure function returning a `Decision`
//! (`Click`, `Key`, `Wait`, `Stop(Reason)`) that carries the message to show; the loop only performs
//! effects." v2's equivalent loop was untestable because it closed over the serial port, the config
//! and the OCR engine at once (`PROGRESS.md` entry 59). Here the tool is a trait, the effects are a
//! trait, time is a `Clock`, and the stop is the cancellation token — so a whole run is a
//! single-threaded test.
//!
//! The loop is deliberately small: publish `running`, then `decide` → perform → check the stop. It
//! does not catch anything a tool does; a panic is the process's problem and the arbiter's `finished`
//! is what publishes the final word. The real tool half lives in the tools' crates; the effects half
//! lives in `seal-core` over the board, capture and OCR.
//!
//! The hotkey watches v2's `SleepCheck` also carried (quit/pause) are NOT here: they belong to the
//! core's input loop, which cancels the token when the player presses quit — the loop then stops
//! through the same mechanism as the Stop button, which is what makes the two indistinguishable to
//! the tool.

use seal_domain::decision::{Decision, Key, StopReason};
use seal_domain::geometry::Point;

use crate::arbiter::RunHandle;
use crate::cancel::RunControl;
use crate::clock::Clock;
use crate::tool::{ToolId, ToolPhase, ToolSnapshot};

/// A tool's decision half. One method: what to do next. The loop calls it repeatedly and performs
/// each answer; the tool keeps any state it needs between calls.
///
/// A decision half must not perform I/O — it returns `Decision`s and the runner performs them. That
/// is what makes a whole run testable with a fake tool, and it is why the message a run ends with
/// rides on `Stop`, not on a writer of its own.
pub trait Tool {
    fn id(&self) -> ToolId;

    /// The next thing to do. Returning [`Decision::Stop`] ends the run with its reason; the loop
    /// never calls this again after that.
    fn decide(&mut self) -> Decision;
}

/// The effects one decision needs. `seal-core` implements it over the board and the cursor; the
/// tests record calls.
pub trait Effects {
    fn click(&mut self, at: Point) -> Result<(), EffectError>;
    fn key(&mut self, key: Key) -> Result<(), EffectError>;

    /// Press one key as the fast tap. The default is the ordinary press, so an effects
    /// implementation that does not model the distinction still sends the key rather than dropping
    /// it — the slower tap is a visible-but-working outcome, a dropped key is an invisible bug.
    fn key_fast(&mut self, key: Key) -> Result<(), EffectError> {
        self.key(key)
    }
}

/// An effect failed. The message is what the tool's card shows, so it is a sentence — v2's
/// `Exception.Message` was not, which is why its cards occasionally showed a stack-frame string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectError {
    message: String,
}

impl EffectError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for EffectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for EffectError {}

/// The line a cancelled run ends with. v2 published no message on a normal stop (its card fell to a
/// bare "stopped" made of UI text), and `tool.finished` needs a string — this is that string, defined
/// where v2 has none.
pub const STOPPED_MESSAGE: &str = "Stopped.";

/// The line an effect failure ends with when the error carries no better sentence. Defined where v2
/// only had its card show the raw exception text.
pub const EFFECT_FAILED_MESSAGE: &str = "A board or capture action failed.";

/// Runs a tool to its end and returns the reason it stopped.
///
/// The loop, in order: publish `running`; check the stop; ask for a decision; perform it; repeat.
/// `Wait` sleeps through [`RunControl`]'s ticked sleep, so a stop is noticed DURING a long wait (the
/// pet feeder's ~30 s reload) rather than after it. A failed effect ends the run as an error; a
/// cancelled token ends it as cancelled; `Decision::Stop` ends it with its own reason and message.
///
/// The final snapshot is NOT published here: the arbiter publishes it from the returned
/// [`StopReason`] in `finished`, so the reason and the card cannot disagree, and a stop requested
/// while the loop is leaving is still the arbiter's to finish.
pub fn run(
    tool: &mut dyn Tool,
    handle: &RunHandle,
    clock: &dyn Clock,
    effects: &mut dyn Effects,
) -> StopReason {
    let control = RunControl::new(handle.token.clone());
    handle
        .state
        .publish(ToolSnapshot::new(tool.id()).running(ToolPhase::Running));

    loop {
        if control.check().is_err() {
            return StopReason::cancelled(STOPPED_MESSAGE);
        }

        let decision = tool.decide();
        match decision {
            Decision::Click(at) => {
                if let Err(error) = effects.click(at) {
                    return StopReason::error(effect_message(&error));
                }
            }
            Decision::Key(key) => {
                if let Err(error) = effects.key(key) {
                    return StopReason::error(effect_message(&error));
                }
            }
            Decision::KeyFast(key) => {
                if let Err(error) = effects.key_fast(key) {
                    return StopReason::error(effect_message(&error));
                }
            }
            Decision::Wait(seconds) => {
                if control.sleep(clock, seconds).is_err() {
                    return StopReason::cancelled(STOPPED_MESSAGE);
                }
            }
            Decision::Stop(reason) => {
                // A terminal decision is the tool's own answer; the loop does not question it.
                return reason;
            }
        }
    }
}

fn effect_message(error: &EffectError) -> String {
    if error.message.is_empty() {
        EFFECT_FAILED_MESSAGE.to_owned()
    } else {
        error.message.clone()
    }
}

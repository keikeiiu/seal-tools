//! The runner seam: the thin loop that turns a tool's decisions into effects, with the fake tool and
//! fake effects driving the whole run in tests.
//!
//! `PLAN-V3-CORE.md` §6.4: "Each tool's decision half becomes a pure function returning a `Decision`
//! (`Click`, `Key`, `Wait`, `Stop(Reason)`) that carries the message to show; the loop only performs
//! effects." v2's equivalent loop was untestable because it closed over the serial port, the config
//! and the OCR engine at once (`PROGRESS.md` entry 59). Here the tool is a trait, the effects are a
//! trait, time is a `Clock`, and the stop is the cancellation token —so a whole run is a
//! single-threaded test.
//!
//! The loop is deliberately small: publish `running`, then `decide` —perform —check the stop. It
//! does not catch anything a tool does; a panic is the process's problem and the arbiter's `finished`
//! is what publishes the final word. The real tool half lives in the tools' crates; the effects half
//! lives in `seal-core` over the board, capture and OCR.
//!
//! The hotkey watches v2's `SleepCheck` also carried (quit/pause) are NOT here: they belong to the
//! core's input loop, which cancels the token when the player presses quit —the loop then stops
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
/// A decision half must not perform I/O —it returns `Decision`s and the runner performs them. That
/// is what makes a whole run testable with a fake tool, and it is why the message a run ends with
/// rides on `Stop`, not on a writer of its own.
pub trait Tool {
    fn id(&self) -> ToolId;

    /// The next thing to do. Returning [`Decision::Stop`] ends the run with its reason; the loop
    /// never calls this again after that.
    fn decide(&mut self) -> Decision;

    /// The run's card state: the tool's own words, a countdown it is waiting out, and standing rows.
    /// The runner publishes it when it CHANGES, so a live tool's progress reaches the `state` event
    /// without the tool writing to the bus (one writer, `PROTOCOL.md` §5). The default is empty: a
    /// tool with nothing to say publishes nothing extra.
    fn status(&self) -> ToolStatus {
        ToolStatus::default()
    }

    /// The run's LAST facts for `tool.finished.summary` (G11), built by the tool because only it can
    /// say what the run achieved —e.g. the Tuner's final grade/attempts/captures and which stop
    /// reason ended it. `None` (the default) publishes `"summary": null`; `ended_at` is added by the
    /// event builder, so a tool never carries a clock.
    fn summary(&self, _reason: &StopReason) -> Option<serde_json::Value> {
        None
    }

    /// Extra lines for the tool's per-run FILE log (`logs/<tool>.log`), drained by the supervisor's
    /// `LoggedTool` after each decision. Kept separate from [`Self::status`]: a log can carry the
    /// reasons and the per-row next actions that would bury a card, and a tool whose diagnostics
    /// change more often than its card is not forced to publish duplicate events. Default empty.
    fn drain_log_notes(&mut self) -> Vec<String> {
        Vec::new()
    }

    /// An effect failed. The default ends the run (every counted tool does: a buy pass that cannot
    /// click is over), but a RESIDENT tool outlives a single bad click, so it can ask the loop to
    /// carry on (PET-F8). Called with the failure before anything is decided about the run.
    fn on_effect_error(&mut self, _error: &EffectError) -> EffectRecovery {
        EffectRecovery::Stop
    }

    /// A wait was cut short because the machine SLEPT through it: `actual_seconds` of real time
    /// passed while only the requested wait was accounted for (PET-F12). A tool whose schedule is
    /// time based applies the difference here; the default does nothing, because a tool that reads
    /// its state fresh every decision has nothing to correct.
    fn on_resume(&mut self, _actual_seconds: f64) {}
}

/// What the loop does after an effect failed. See [`Tool::on_effect_error`].
#[derive(Debug, Clone, PartialEq)]
pub enum EffectRecovery {
    /// End the run as an error carrying the failure's line —the default.
    Stop,
    /// The tool absorbed the failure (a strike, a retry window) and the run continues: publish
    /// `message`, wait `after_s` (cancellable), then ask the tool for its next decision.
    Retry { after_s: f64, message: String },
}

/// What a tool wants its card to say between decisions. Compared by value by the runner, so a tool
/// may return an unchanged status as often as it likes without producing an event.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolStatus {
    /// The last thing that happened, in the tool's own words.
    pub message: String,
    /// Remaining seconds when the tool is waiting something out; the card counts down from it.
    pub countdown_s: Option<f64>,
    /// Standing rows (the pet feeder's four rows today).
    pub rows: Vec<crate::tool::ToolRow>,
    /// Labelled facts for the running card (`Grade: DG`, `Attempt: 7`, attribute lines). Empty when
    /// there is nothing to show; additive `state.detail` in `v:1` (PLAN-V3-STATUS-DETAIL §1.3).
    pub detail: Vec<crate::tool::ToolDetail>,
}

impl ToolStatus {
    #[must_use]
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    #[must_use]
    pub fn countdown_s(mut self, countdown_s: Option<f64>) -> Self {
        self.countdown_s = countdown_s;
        self
    }

    #[must_use]
    pub fn rows(mut self, rows: Vec<crate::tool::ToolRow>) -> Self {
        self.rows = rows;
        self
    }

    #[must_use]
    pub fn detail(mut self, detail: Vec<crate::tool::ToolDetail>) -> Self {
        self.detail = detail;
        self
    }
}

/// The effects one decision needs. `seal-core` implements it over the board and the cursor; the
/// tests record calls.
pub trait Effects {
    /// Click at `at`.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] carrying the sentence the tool's card shows when the click cannot be
    /// performed.
    fn click(&mut self, at: Point) -> Result<(), EffectError>;

    /// Press `key`.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] carrying the sentence the tool's card shows when the press cannot be
    /// performed.
    fn key(&mut self, key: Key) -> Result<(), EffectError>;

    /// Press one key as the fast tap. The default is the ordinary press, so an effects
    /// implementation that does not model the distinction still sends the key rather than dropping
    /// it —the slower tap is a visible-but-working outcome, a dropped key is an invisible bug.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] from [`Self::key`], or the implementation's own fast-tap failure.
    fn key_fast(&mut self, key: Key) -> Result<(), EffectError> {
        self.key(key)
    }

    /// Press and hold the spacebar. The default REFUSES rather than approximating with a tap: a tap
    /// would look like the hold worked while auto-pickup never happens, which is the silent failure
    /// class this project keeps paying for. Only the Hold Space effects implement it.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] when the implementation cannot hold the spacebar (the default) or the
    /// board write fails.
    fn hold_space(&mut self) -> Result<(), EffectError> {
        Err(EffectError::new(
            "this effects implementation cannot hold the spacebar",
        ))
    }

    /// Release the spacebar. The default is a no-op: nothing was held, so nothing needs releasing.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] when the release write fails; the default is `Ok(())`.
    fn release_space(&mut self) -> Result<(), EffectError> {
        Ok(())
    }

    /// Press Enter. The default REFUSES rather than approximating with a printable `K E`: those are
    /// different wires (`E` vs `K E`) and the firmware's Enter is the one a game dialog accepts.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] when the implementation cannot press Enter (the default) or the board
    /// write fails.
    fn enter(&mut self) -> Result<(), EffectError> {
        Err(EffectError::new(
            "this effects implementation cannot press Enter",
        ))
    }

    /// Move the cursor to a client-relative PHYSICAL offset without clicking. The default REFUSES: an
    /// implementation that cannot place must not click instead, because the click is what the caller
    /// deliberately did not ask for.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] when the implementation cannot place the cursor (the default) or the
    /// move fails.
    fn place(&mut self, _at: Point) -> Result<(), EffectError> {
        Err(EffectError::new(
            "this effects implementation cannot place the cursor",
        ))
    }

    /// Click wherever the cursor already is (the board's bare `C`). The default REFUSES rather than
    /// trying to place at an origin: a bare click at the wrong spot is worse than a card line.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] when the implementation cannot click without a target (the default) or
    /// the board write fails.
    fn click_here(&mut self) -> Result<(), EffectError> {
        Err(EffectError::new(
            "this effects implementation cannot click without a target",
        ))
    }

    /// Right-click wherever the cursor already is (the board's bare `R`). Default refuses, as above.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] when the implementation cannot right-click without a target (the
    /// default) or the board write fails.
    fn right_click_here(&mut self) -> Result<(), EffectError> {
        Err(EffectError::new(
            "this effects implementation cannot right-click without a target",
        ))
    }

    /// One wheel scroll. Default refuses rather than silently skipping: a skipped scroll would make a
    /// buy pass click the wrong row, which is a wrong PURCHASE, not a cosmetic failure.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] when the implementation cannot scroll the wheel (the default) or the
    /// board write fails.
    fn wheel(&mut self, _notches: i32, _down: bool) -> Result<(), EffectError> {
        Err(EffectError::new(
            "this effects implementation cannot scroll the wheel",
        ))
    }

    /// One raw HID move. Default refuses: the counts are measured per machine, and approximating them
    /// with a placement would be a different move, not the measured one.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] when the implementation cannot make a raw HID move (the default) or
    /// the board write fails.
    fn move_by(&mut self, _dx: i32, _dy: i32) -> Result<(), EffectError> {
        Err(EffectError::new(
            "this effects implementation cannot make a raw HID move",
        ))
    }

    /// Press and hold the left button. Default refuses rather than approximating: a drag that does
    /// not hold drops the item somewhere else.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] when the implementation cannot hold the left button (the default) or
    /// the board write fails.
    fn hold_left(&mut self) -> Result<(), EffectError> {
        Err(EffectError::new(
            "this effects implementation cannot hold the left button",
        ))
    }

    /// Release the left button. Default is a no-op: nothing was held through this seam.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] when the release write fails; the default is `Ok(())`.
    fn release_left(&mut self) -> Result<(), EffectError> {
        Ok(())
    }
}

/// An effect failed. The message is what the tool's card shows, so it is a sentence —v2's
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

    #[must_use]
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
/// bare "stopped" made of UI text), and `tool.finished` needs a string —this is that string, defined
/// where v2 has none.
pub const STOPPED_MESSAGE: &str = "Stopped.";

/// The line an effect failure ends with when the error carries no better sentence. Defined where v2
/// only had its card show the raw exception text.
pub const EFFECT_FAILED_MESSAGE: &str = "A board or capture action failed.";

/// Runs a tool to its end and returns the reason it stopped.
///
/// The loop, in order: publish `running`; check the stop; ask for a decision; check the stop
/// AGAIN (a slow `decide` —the tuner's OCR pass —can outlast a Stop press, and the effect a
/// pending non-terminal decision asks for must not run after it); perform it; repeat.
/// `Wait` sleeps through [`RunControl`]'s ticked sleep, so a stop is noticed DURING a long wait (the
/// pet feeder's ~30 s reload) rather than after it. A failed effect ends the run as an error —unless
/// the tool absorbed it through [`Tool::on_effect_error`], in which case the loop publishes its line
/// and carries on (PET-F8). A cancelled token ends it as cancelled; `Decision::Stop` ends it with its
/// own reason and message.
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

    let mut published_status = ToolStatus::default();
    loop {
        if control.check().is_err() {
            return StopReason::cancelled(STOPPED_MESSAGE);
        }

        let decision = tool.decide();

        // TOOL-F8: a Stop pressed during a slow `decide` (the tuner can spend a second in capture +
        // OCR between the check above and here) must not be followed by the click/Enter/placement the
        // decision asks for. The token is one-way, so a non-terminal decision whose effect has not
        // run yet is abandoned; a TERMINAL decision still wins, because it is the tool's own answer
        // and carries the line the card must show.
        if control.check().is_err() && !decision.is_terminal() {
            return StopReason::cancelled(STOPPED_MESSAGE);
        }

        // Publish the tool's own card state BEFORE performing the decision, so a countdown shows
        // while its wait runs and a message caused by a read is visible as soon as it exists.
        publish_status(handle, tool, &mut published_status, &control);
        let failure = match decision {
            Decision::Wait(seconds) => {
                if control.sleep(clock, seconds).is_err() {
                    return StopReason::cancelled(STOPPED_MESSAGE);
                }
                if let Some(slept) = control.take_resume() {
                    // The machine slept through the wait: the screen may have changed or the client
                    // disconnected. The tool decides what to re-read before acting again (PET-F12),
                    // and its next `status()` publishes whatever it now says.
                    tool.on_resume(slept);
                }
                None
            }
            Decision::Stop(reason) => {
                // A terminal decision is the tool's own answer; the loop does not question it.
                return reason;
            }
            input => perform_input(effects, &input).err(),
        };
        // PET-F8: every counted tool ends the run here (its work is over either way), but a resident
        // tool can absorb the failure and keep the schedule going.
        if let Some(error) = failure {
            match tool.on_effect_error(&error) {
                EffectRecovery::Stop => return StopReason::error(effect_message(&error)),
                EffectRecovery::Retry { after_s, message } => {
                    publish_message(handle, tool, &mut published_status, &control, message);
                    if control.sleep(clock, after_s).is_err() {
                        return StopReason::cancelled(STOPPED_MESSAGE);
                    }
                }
            }
        }
    }
}

/// Performs one INPUT decision —everything except the loop's own two ([`Decision::Wait`] is time
/// and [`Decision::Stop`] is the end, neither of which the effects half owns). Split out of [`run`]
/// so the loop's shape stays readable and the per-decision rows stay a table.
fn perform_input(effects: &mut dyn Effects, decision: &Decision) -> Result<(), EffectError> {
    match decision {
        Decision::Click(at) => effects.click(*at),
        Decision::Key(key) => effects.key(*key),
        Decision::KeyFast(key) => effects.key_fast(*key),
        Decision::HoldSpace => effects.hold_space(),
        Decision::ReleaseSpace => effects.release_space(),
        Decision::Enter => effects.enter(),
        Decision::Place(at) => effects.place(*at),
        Decision::ClickHere => effects.click_here(),
        Decision::RightClickHere => effects.right_click_here(),
        Decision::Wheel { notches, down } => effects.wheel(*notches, *down),
        Decision::MoveBy { dx, dy } => effects.move_by(*dx, *dy),
        Decision::HoldLeft => effects.hold_left(),
        Decision::ReleaseLeft => effects.release_left(),
        // The caller handles both before getting here; performing them as no-ops keeps the exhaust
        // list in one place instead of a catch-all that could swallow a future decision's effect.
        Decision::Wait(_) | Decision::Stop(_) => Ok(()),
    }
}

/// Publishes the tool's own card state when it changed since the last publish. Pulled out of
/// [`run`] so the loop's line count stays readable: the countdown/message/rows/detail replacement is
/// one concern, and the decision/effect dispatch is another.
fn publish_status(
    handle: &RunHandle,
    tool: &dyn Tool,
    published: &mut ToolStatus,
    control: &RunControl,
) {
    let status = tool.status();
    if *published == status {
        return;
    }
    publish(handle, tool.id(), control, status, published);
}

/// Publishes the tool's current status with `message` in place of its own —the retry line an effect
/// failure produced, which the tool's own `status()` cannot show until its next decision (PET-F8).
fn publish_message(
    handle: &RunHandle,
    tool: &dyn Tool,
    published: &mut ToolStatus,
    control: &RunControl,
    message: String,
) {
    let mut status = tool.status();
    status.message = message;
    publish(handle, tool.id(), control, status, published);
}

fn publish(
    handle: &RunHandle,
    tool: ToolId,
    control: &RunControl,
    status: ToolStatus,
    published: &mut ToolStatus,
) {
    *published = status.clone();
    // The tool may have observed a cancellation in `status()`; never paint Running over Stopping
    // (the cell also enforces this monotonic phase for the ordinary Stop path, TOOL-F14).
    let phase = if control.is_cancelled() {
        ToolPhase::Stopping
    } else {
        ToolPhase::Running
    };
    handle.state.publish(
        ToolSnapshot::new(tool)
            .running(phase)
            .message(status.message)
            .countdown_s(status.countdown_s)
            .rows(status.rows)
            .detail(status.detail),
    );
}

fn effect_message(error: &EffectError) -> String {
    if error.message.is_empty() {
        EFFECT_FAILED_MESSAGE.to_owned()
    } else {
        error.message.clone()
    }
}

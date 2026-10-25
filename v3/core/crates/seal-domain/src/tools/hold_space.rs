//! The Hold Space tool's decision half: what one tick of the spacebar-hold loop wants done.
//!
//! v2's tool ran a forever loop that watched two inputs — the launcher's Start/Stop mirror
//! (`state.Running`) and its own hotkey edge — and sent `P` (press and hold) or `U` (release) down the
//! serial port. This module owns the DECISION: given the loop's two inputs and the memory of last
//! tick's hotkey, it says which writes the loop should perform, what to log, and what to beep. The
//! effects (`P`/`U`, the beeps, the console lines) stay with `seal-board`/`seal-core`; the release
//! GUARD — the rule that nothing may leave the spacebar down — is `seal-tools::held`'s, which owns the
//! mechanics.
//!
//! # THE ORACLE IS AUTHORED
//!
//! v2 has no test for `HoldSpace.cs` (the audit found none; the file has none in
//! `SealTools.Tests`), so every case in `tests/hold_space.rs` was written from
//! `v2/SealTools.Spammer/HoldSpace.cs:23-79`, with the line numbers cited in the tests. Where the
//! tests pin something odd, the oddity is v2's and is named in the test.
//!
//! # The two inputs, and why they can both fire in one tick
//!
//! `state.Running` is the launcher's mirror of whether the card says the tool is on; the hotkey is the
//! player pressing F12 directly. The v2 loop checked the mirror FIRST and the hotkey edge SECOND, in
//! the same pass, so one tick can legitimately produce TWO writes — e.g. the launcher starting the
//! tool on the same 50 ms tick the player presses F12 to stop it sends `P` and then `U`. That is v2
//! behaviour; [`HoldSpaceTick::actions`] carries it rather than pretending a tick has one action, and
//! a test pins it.
//!
//! # The press that must not be forgotten
//!
//! Every failure inside v2's `Hold`/`Release` was swallowed into a card message rather than ending the
//! run, because the spacebar being left down is the dangerous state: the release line says what to do
//! about it. [`hold_failed`] and [`release_failed`] are those two lines verbatim. And whenever the run
//! ends — normal stop, quit hotkey, cancellation, port drop — v2's `finally` sent `U`. [`cleanup`] is
//! that rule as a value, so a loop cannot forget which action its teardown owes; the held-input guard
//! enforces the same thing independently (RAII), and both are meant to agree.

/// The frequency/length of a beep the tool sounds. Effect data: the loop performs it, this module only
/// says which one accompanies the action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Beep {
    pub frequency: u32,
    pub duration_ms: u32,
}

/// Rising note on a press (`HoldSpace.cs:49,64`).
pub const HOLD_BEEP: Beep = Beep {
    frequency: 523,
    duration_ms: 100,
};

/// Falling note on a release (`HoldSpace.cs:56,65`).
pub const RELEASE_BEEP: Beep = Beep {
    frequency: 1000,
    duration_ms: 150,
};

/// One port write the tick wants performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldSpaceAction {
    /// Send `P`: hold the spacebar down.
    Hold,
    /// Send `U`: release the spacebar.
    Release,
}

impl HoldSpaceAction {
    /// The byte the board understands (`P\n` / `U\n`). Kept here as the decision's own encoding so the
    /// effect side and the tests cannot disagree about which write an action means; the board crate
    /// owns the transport.
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Hold => "P\n",
            Self::Release => "U\n",
        }
    }
}

/// Why an action happened, so the loop's log line and a test can tell the two inputs apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldSpaceWhy {
    /// The launcher's mirror said the tool is on/off.
    Panel,
    /// The player pressed the start/stop hotkey this tick.
    Hotkey,
}

/// One thing the tick wants done, with everything the loop needs to carry it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldSpaceStep {
    pub action: HoldSpaceAction,
    pub why: HoldSpaceWhy,
    /// The console line v2 printed for this step (`[Panel] HOLDING space`, `[GO] holding space`, …).
    pub log: &'static str,
    pub beep: Beep,
}

/// What one tick decided, and the state the next tick starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldSpaceTick {
    /// The loop's own memory of whether the spacebar is held, for the NEXT tick.
    pub mirror_running: bool,
    /// Up to two writes, in v2's order. Two only when the mirror and the hotkey both changed the state
    /// on the same tick — see the module docs.
    pub actions: Vec<HoldSpaceStep>,
    /// Set when the hotkey edge changed it: the launcher's card state must follow (`state.Running =
    /// running` in v2, `HoldSpace.cs:63`). `None` means the hotkey did not fire, so the card is the
    /// launcher's business.
    pub panel_running: Option<bool>,
    /// A start clears the pending-quit flag (`HoldSpace.cs:46,64`): pressing start after stop is a
    /// fresh run, not a continuation of the quit that just happened.
    pub clear_quit: bool,
}

impl HoldSpaceTick {
    /// Nothing to do, nothing to report.
    pub fn idle(mirror_running: bool) -> Self {
        Self {
            mirror_running,
            actions: Vec::new(),
            panel_running: None,
            clear_quit: false,
        }
    }

    fn hold(
        mirror: &mut bool,
        actions: &mut Vec<HoldSpaceStep>,
        why: HoldSpaceWhy,
        log: &'static str,
    ) {
        *mirror = true;
        actions.push(HoldSpaceStep {
            action: HoldSpaceAction::Hold,
            why,
            log,
            beep: HOLD_BEEP,
        });
    }

    fn release(
        mirror: &mut bool,
        actions: &mut Vec<HoldSpaceStep>,
        why: HoldSpaceWhy,
        log: &'static str,
    ) {
        *mirror = false;
        actions.push(HoldSpaceStep {
            action: HoldSpaceAction::Release,
            why,
            log,
            beep: RELEASE_BEEP,
        });
    }
}

/// One tick of the loop, from the loop's three inputs and its hotkey memory.
///
/// `mirror_running` is the loop's own `running`; `panel_running` is `state.Running` as the launcher last
/// set it; `hotkey_down` is this tick's read and `hotkey_was_down` the previous tick's. The loop's
/// stop checks (quit flag, cancellation) happen BEFORE this call in v2 (`HoldSpace.cs:39-40`) and stay
/// in the loop: a stop abandons the work and the teardown still releases.
pub fn tick(
    mirror_running: bool,
    panel_running: bool,
    hotkey_down: bool,
    hotkey_was_down: bool,
) -> HoldSpaceTick {
    let mut mirror = mirror_running;
    let mut actions = Vec::with_capacity(2);
    let mut clear_quit = false;

    // `HoldSpace.cs:43-50`: follow the launcher's mirror.
    if panel_running && !mirror {
        HoldSpaceTick::hold(
            &mut mirror,
            &mut actions,
            HoldSpaceWhy::Panel,
            "[Panel] HOLDING space",
        );
        clear_quit = true;
    } else if !panel_running && mirror {
        HoldSpaceTick::release(
            &mut mirror,
            &mut actions,
            HoldSpaceWhy::Panel,
            "[Panel] RELEASED space",
        );
    }

    // `HoldSpace.cs:59-67`: the hotkey's RISING edge toggles, and the card follows it.
    let mut panel_follow = None;
    if hotkey_down && !hotkey_was_down {
        if mirror {
            HoldSpaceTick::release(
                &mut mirror,
                &mut actions,
                HoldSpaceWhy::Hotkey,
                "[STOP] released",
            );
        } else {
            HoldSpaceTick::hold(
                &mut mirror,
                &mut actions,
                HoldSpaceWhy::Hotkey,
                "[GO] holding space",
            );
            clear_quit = true;
        }
        panel_follow = Some(mirror);
    }

    HoldSpaceTick {
        mirror_running: mirror,
        actions,
        panel_running: panel_follow,
        clear_quit,
    }
}

/// The teardown's owed action: v2's `finally { Release(); }` (`HoldSpace.cs:70-74`), "never leave the
/// spacebar stuck down". A loop must perform this on every exit path, including after a cancellation;
/// the held-input guard enforces the same independently.
pub const fn cleanup() -> HoldSpaceAction {
    HoldSpaceAction::Release
}

/// The card line when pressing/holding the spacebar failed (`HoldSpace.cs:32`). The run continues: a
/// press that failed leaves nothing down, so it is a message, not a fatal error.
pub fn hold_failed(error: &str) -> String {
    format!("Could not hold the spacebar: {error}")
}

/// The card line when RELEASING failed (`HoldSpace.cs:33`). This is the dangerous case — the host may
/// be left with the spacebar down — so the message also says what to do about it.
pub fn release_failed(error: &str) -> String {
    format!("Could not release the spacebar — tap it once in game ({error})")
}

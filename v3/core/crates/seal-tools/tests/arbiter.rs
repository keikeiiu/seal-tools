//! The arbiter: one game, one tool at a time, and the resident feeder's deferral rules.
//!
//! Everything here is sequential — no threads, no board, no game — which is the point of moving the
//! lifecycle out of v2's `LauncherService` (`PROGRESS.md` entry 59 recorded that the registry was a
//! plain dictionary written by two threads and could not be tested at all). The supervisor that runs
//! a real loop is the next increment; this file decides who may run and what the player is told.

use seal_domain::decision::StopReason;
use seal_tools::arbiter::{Arbiter, GameRelease, GameUse, Start, Stop};
use seal_tools::tool::{ToolId, ToolPhase};

fn started(start: Start) -> seal_tools::arbiter::RunHandle {
    match start {
        Start::Started(handle) => handle,
        other => panic!("expected a started run, got {other:?}"),
    }
}

// ── one game, one tool at a time ──────────────────────────────────────────────────────────────

#[test]
fn one_foreground_tool_at_a_time_is_refused_with_v2s_line() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Tuner));

    let refusal = match arbiter.start(ToolId::Gem) {
        Start::Refused(refusal) => refusal,
        other => panic!("expected a refusal, got {other:?}"),
    };

    assert_eq!(
        refusal.line,
        "\"tuner\" is using the game right now — stop it, then try again."
    );
    // The refused start changed nothing.
    assert_eq!(arbiter.running(), vec![ToolId::Tuner]);
    assert_eq!(arbiter.game_owner(), Some(ToolId::Tuner));
}

#[test]
fn a_started_foreground_tool_takes_the_game_for_its_whole_run() {
    let mut arbiter = Arbiter::new();
    let handle = started(arbiter.start(ToolId::Tuner));
    assert!(!handle.token.is_cancelled());

    assert_eq!(arbiter.game_owner(), Some(ToolId::Tuner));
    assert_eq!(arbiter.current(), Some(ToolId::Tuner));
    assert!(arbiter.is_running(ToolId::Tuner));
    assert!(!arbiter.is_running(ToolId::Gem));

    let snapshot = handle
        .state
        .read()
        .expect("a starting snapshot is published");
    assert_eq!(snapshot.tool, ToolId::Tuner);
    assert_eq!(snapshot.phase, ToolPhase::Starting);
    assert!(snapshot.running, "starting is engaged");
}

#[test]
fn starting_the_same_tool_twice_is_refused_with_the_same_busy_line() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Buy));

    let refusal = match arbiter.start(ToolId::Buy) {
        Start::Refused(refusal) => refusal,
        other => panic!("expected a refusal, got {other:?}"),
    };

    assert_eq!(
        refusal.line,
        "\"buy\" is using the game right now — stop it, then try again."
    );
}

// ── stopping: the slot is held until the loop has actually left ──────────────────────────────

#[test]
fn stop_cancels_the_run_and_keeps_the_slot_until_it_has_finished() {
    let mut arbiter = Arbiter::new();
    let running = started(arbiter.start(ToolId::Quest));

    let stop = arbiter.stop(Some(ToolId::Quest));
    let Stop::Cancelling { tool, handle } = stop else {
        panic!("expected a cancellation, got {stop:?}");
    };

    assert_eq!(tool, ToolId::Quest);
    assert!(handle.token.is_cancelled());
    assert!(running.token.is_cancelled(), "one token, two holders");
    assert!(arbiter.is_running(ToolId::Quest), "still leaving");
    assert!(arbiter.is_stopping(ToolId::Quest));
    assert_eq!(arbiter.current(), None);
    assert_eq!(
        arbiter.phase_of(ToolId::Quest),
        Some(ToolPhase::Stopping),
        "the card must say Stopping, not Idle"
    );

    let final_snapshot = arbiter
        .finished(ToolId::Quest, StopReason::done("DONE — 4 row(s)"))
        .final_snapshot
        .expect("a final snapshot");
    assert_eq!(final_snapshot.phase, ToolPhase::Idle);
    assert!(!final_snapshot.running);
    assert_eq!(final_snapshot.message, "DONE — 4 row(s)");
    assert!(!arbiter.is_running(ToolId::Quest), "the slot is now clear");
}

#[test]
fn a_tool_still_stopping_refuses_a_start_until_it_is_gone() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Tuner));
    let _ = arbiter.stop(Some(ToolId::Tuner));

    let same = match arbiter.start(ToolId::Tuner) {
        Start::Refused(refusal) => refusal,
        other => panic!("expected a refusal, got {other:?}"),
    };
    assert_eq!(
        same.line,
        "\"tuner\" is still stopping — wait for it to finish, then start again."
    );

    let other = match arbiter.start(ToolId::Gem) {
        Start::Refused(refusal) => refusal,
        other => panic!("expected a refusal, got {other:?}"),
    };
    assert_eq!(
        other.line,
        "\"tuner\" is still stopping — wait for it to finish, then start again."
    );

    arbiter.finished(ToolId::Tuner, StopReason::cancelled("stopped"));
    started(arbiter.start(ToolId::Gem));
}

#[test]
fn stop_with_no_name_stops_the_current_tool() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Sell));

    let stop = arbiter.stop(None);

    match stop {
        Stop::Cancelling { tool, .. } => assert_eq!(tool, ToolId::Sell),
        other => panic!("expected a cancellation, got {other:?}"),
    }
}

#[test]
fn stop_with_no_name_falls_back_to_the_resident_feeder() {
    // `PROTOCOL.md` §4: "tool.stop with no tool stops whatever is running". With only the resident
    // feeder running, that is the feeder.
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Pet));

    match arbiter.stop(None) {
        Stop::Cancelling { tool, .. } => assert_eq!(tool, ToolId::Pet),
        other => panic!("expected a cancellation, got {other:?}"),
    }
}

#[test]
fn stopping_what_is_not_running_is_not_running() {
    let mut arbiter = Arbiter::new();

    assert!(matches!(arbiter.stop(Some(ToolId::Gem)), Stop::NotRunning));
    assert!(matches!(arbiter.stop(None), Stop::NotRunning));
}

#[test]
fn a_second_stop_has_nothing_to_add() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::HoldSpace));
    let _ = arbiter.stop(Some(ToolId::HoldSpace));

    assert!(matches!(
        arbiter.stop(Some(ToolId::HoldSpace)),
        Stop::NotRunning
    ));
}

#[test]
fn stop_all_stops_every_run_including_the_resident_feeder() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Pet));
    started(arbiter.start(ToolId::Tuner));

    let stops = arbiter.stop_all();

    assert_eq!(stops.len(), 2);
    for stop in stops {
        let Stop::Cancelling { handle, .. } = stop else {
            panic!("every stop must be a cancellation");
        };
        assert!(handle.token.is_cancelled());
    }
    assert!(arbiter.is_stopping(ToolId::Pet));
    assert!(arbiter.is_stopping(ToolId::Tuner));
}

// ── finishing: the only place a slot is removed ──────────────────────────────────────────────

#[test]
fn a_run_that_ends_by_itself_still_clears_its_slot_and_releases_the_game() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Quest));

    let finish = arbiter.finished(ToolId::Quest, StopReason::done("DONE — 20 loop(s)"));

    assert_eq!(arbiter.game_owner(), None);
    assert_eq!(arbiter.current(), None);
    assert!(!arbiter.is_running(ToolId::Quest));
    assert!(matches!(finish.game, GameRelease::Freed));
    let snapshot = finish.final_snapshot.expect("a final snapshot");
    assert_eq!(snapshot.message, "DONE — 20 loop(s)");
    assert_eq!(snapshot.phase, ToolPhase::Idle);
}

#[test]
fn an_error_reason_rides_beside_the_final_message() {
    let mut arbiter = Arbiter::new();
    let handle = started(arbiter.start(ToolId::Gem));

    let finish = arbiter.finished(
        ToolId::Gem,
        StopReason::error("OCR failed: the crop was blank"),
    );

    let snapshot = finish.final_snapshot.expect("a final snapshot");
    assert_eq!(snapshot.message, "OCR failed: the crop was blank");
    assert_eq!(
        snapshot.error.as_deref(),
        Some("OCR failed: the crop was blank")
    );
    // The same final snapshot is what the card reads.
    assert_eq!(
        handle.state.read().expect("published").error.as_deref(),
        Some("OCR failed: the crop was blank")
    );
}

#[test]
fn a_cancelled_reason_is_a_message_not_an_error() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Spammer));
    let _ = arbiter.stop(None);

    let finish = arbiter.finished(ToolId::Spammer, StopReason::cancelled("stopped"));

    let snapshot = finish.final_snapshot.expect("a final snapshot");
    assert_eq!(snapshot.message, "stopped");
    assert_eq!(snapshot.error, None, "a normal stop is not a failure");
}

#[test]
fn finishing_a_tool_with_no_run_changes_nothing() {
    let mut arbiter = Arbiter::new();

    let finish = arbiter.finished(ToolId::Tuner, StopReason::done("never ran"));

    assert!(finish.final_snapshot.is_none());
    assert!(matches!(finish.game, GameRelease::Freed));
}

// ── the resident pet feeder ──────────────────────────────────────────────────────────────────

#[test]
fn the_feeder_is_armed_beside_a_foreground_tool_and_holds_no_game() {
    let mut arbiter = Arbiter::new();
    let pet = started(arbiter.start(ToolId::Pet));

    // Armed without taking the game: the feeder is background furniture.
    assert_eq!(arbiter.game_owner(), None);
    assert_eq!(arbiter.current(), None, "the feeder is never 'current'");
    assert!(arbiter.is_running(ToolId::Pet));
    let standing_line = pet.state.read().expect("a standing line is published");
    assert!(standing_line.phase.engaged(), "the card keeps a line");

    // A foreground tool may then start and take the game, with the feeder untouched.
    let tuner = started(arbiter.start(ToolId::Tuner));
    assert_eq!(arbiter.game_owner(), Some(ToolId::Tuner));
    assert!(arbiter.is_running(ToolId::Pet));
    assert!(!tuner.token.is_cancelled());
}

#[test]
fn a_resident_feeder_requests_the_game_and_waits_for_a_foreground_holder() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Pet));
    started(arbiter.start(ToolId::Tuner));

    let outcome = arbiter.request_game(ToolId::Pet);

    assert_eq!(
        outcome,
        GameUse::Waiting {
            holder: ToolId::Tuner
        }
    );
    assert_eq!(arbiter.waiting(), Some(ToolId::Pet));
    assert_eq!(arbiter.game_owner(), Some(ToolId::Tuner));
    assert!(
        arbiter.is_running(ToolId::Pet),
        "waiting is not stopping: the feeder stays armed"
    );
    assert!(
        !arbiter.should_stand_down(ToolId::Tuner),
        "only the spammer stands down"
    );
}

#[test]
fn the_spammer_stands_down_for_the_waiting_feeder_and_gets_the_game_back() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Pet));
    started(arbiter.start(ToolId::Spammer));
    assert_eq!(arbiter.game_owner(), Some(ToolId::Spammer));

    // The feeder announces it wants the game; only a holder that CAN stand down is asked to.
    assert_eq!(
        arbiter.request_game(ToolId::Pet),
        GameUse::Preempt {
            holder: ToolId::Spammer
        }
    );
    assert!(arbiter.should_stand_down(ToolId::Spammer));
    assert_eq!(arbiter.waiting(), Some(ToolId::Pet));

    // `SkillSpammer`'s `[YIELD]`: the spammer gives the game up and the feeder gets it.
    assert!(matches!(
        arbiter.release_game(ToolId::Spammer),
        GameRelease::Granted(ToolId::Pet)
    ));
    assert_eq!(arbiter.game_owner(), Some(ToolId::Pet));
    assert!(!arbiter.should_stand_down(ToolId::Spammer));
    assert!(
        arbiter.is_running(ToolId::Spammer),
        "yielding is not stopping"
    );

    // Between reloads the feeder gives the game back; the spammer may take it again.
    assert!(matches!(
        arbiter.release_game(ToolId::Pet),
        GameRelease::Freed
    ));
    assert_eq!(arbiter.game_owner(), None);
    assert_eq!(arbiter.request_game(ToolId::Spammer), GameUse::Granted);
}

#[test]
fn a_foreground_start_behind_the_feeders_reload_is_deferred_with_v2s_line() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Pet));
    assert_eq!(arbiter.request_game(ToolId::Pet), GameUse::Granted);

    let start = arbiter.start(ToolId::Tuner);

    let Start::Deferred { line } = start else {
        panic!("expected a deferred start, got {start:?}");
    };
    assert_eq!(
        line,
        "Waiting for the pet feeder to finish its reload (pet)…"
    );
    assert_eq!(arbiter.pending(), Some(ToolId::Tuner));
    assert!(!arbiter.is_running(ToolId::Tuner), "not started yet");

    // The reload ends: the waiting start begins with the game in its hand.
    let release = arbiter.release_game(ToolId::Pet);
    let GameRelease::StartPending(handle) = release else {
        panic!("expected the pending start to begin, got {release:?}");
    };
    assert_eq!(handle.tool, ToolId::Tuner);
    assert!(matches!(
        handle.state.read().expect("published").phase,
        ToolPhase::Starting
    ));
    assert_eq!(arbiter.game_owner(), Some(ToolId::Tuner));
    assert_eq!(arbiter.current(), Some(ToolId::Tuner));
    assert_eq!(arbiter.pending(), None);
}

#[test]
fn a_deferred_start_can_be_cancelled_or_timed_out_with_v2s_lines() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Pet));
    assert_eq!(arbiter.request_game(ToolId::Pet), GameUse::Granted);
    let _ = arbiter.start(ToolId::Tuner);

    assert_eq!(arbiter.cancel_pending(), Some("Start cancelled."));
    assert_eq!(arbiter.pending(), None);

    // The other ending: the supervisor's 45 s wait runs out, and the refusal names both sides.
    let _ = arbiter.start(ToolId::Tuner);
    let refusal = arbiter.pending_timeout().expect("a timeout refusal");
    assert_eq!(
        refusal.line,
        "The pet feeder is still using the game (pet) — tuner was not started."
    );
    assert_eq!(arbiter.pending(), None);
}

#[test]
fn a_foreground_finished_while_the_feeder_waits_grants_it_the_game() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Pet));
    started(arbiter.start(ToolId::Tuner));
    assert_eq!(
        arbiter.request_game(ToolId::Pet),
        GameUse::Waiting {
            holder: ToolId::Tuner
        }
    );

    let finish = arbiter.finished(ToolId::Tuner, StopReason::done("DONE"));

    assert!(matches!(finish.game, GameRelease::Granted(ToolId::Pet)));
    assert_eq!(arbiter.game_owner(), Some(ToolId::Pet));
    assert_eq!(
        arbiter.waiting(),
        None,
        "the announcement came down with it"
    );
}

#[test]
fn a_feeder_that_finishes_while_a_foreground_start_waits_starts_it() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Pet));
    assert_eq!(arbiter.request_game(ToolId::Pet), GameUse::Granted);
    let _ = arbiter.start(ToolId::Tuner);

    let finish = arbiter.finished(ToolId::Pet, StopReason::done("schedule done"));

    let GameRelease::StartPending(handle) = finish.game else {
        panic!("expected the pending start, got {:?}", finish.game);
    };
    assert_eq!(handle.tool, ToolId::Tuner);
    assert_eq!(arbiter.game_owner(), Some(ToolId::Tuner));
}

#[test]
fn releasing_a_game_you_do_not_hold_is_a_no_op() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Pet));

    assert!(matches!(
        arbiter.release_game(ToolId::Tuner),
        GameRelease::Freed
    ));
    assert_eq!(arbiter.game_owner(), None);
    assert_eq!(arbiter.waiting(), None);
}

#[test]
fn a_second_pet_start_is_refused_busy() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Pet));

    let refusal = match arbiter.start(ToolId::Pet) {
        Start::Refused(refusal) => refusal,
        other => panic!("expected a refusal, got {other:?}"),
    };

    assert_eq!(
        refusal.line,
        "\"pet\" is using the game right now — stop it, then try again."
    );
}

#[test]
fn the_game_owner_is_readable_so_the_card_can_say_who_it_waits_for() {
    let mut arbiter = Arbiter::new();
    started(arbiter.start(ToolId::Pet));
    started(arbiter.start(ToolId::Buy));

    assert_eq!(arbiter.game_owner(), Some(ToolId::Buy));
    assert_eq!(arbiter.waiting(), None);
    assert_eq!(arbiter.current(), Some(ToolId::Buy));
    assert_eq!(arbiter.running(), vec![ToolId::Buy, ToolId::Pet]);
}

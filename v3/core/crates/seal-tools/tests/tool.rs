//! Tool identity and published state: the eight ids, the lifecycle phase, and the immutable snapshot
//! a reader can never see half-updated.
//!
//! The torn-read finding (`PROGRESS.md` entries 63 and 68): `ToolState`'s "reads are atomic" comment
//! was false for its `int?` and `DateTime?` members — the 750 ms UI tick could read a half-updated
//! `Remaining`/`NextActionAt` and draw a garbage countdown. The comment was wrong because the type
//! permitted it. Here a published value is an `Arc<ToolSnapshot>`: the writer replaces a whole
//! snapshot and a reader clones the Arc, so a mixed pair is unrepresentable rather than merely
//! avoided.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

use seal_tools::tool::{StateCell, ToolId, ToolPhase, ToolRow, ToolSnapshot};

// ── identity: the eight protocol ids ───────────────────────────────────────────────────────────

#[test]
fn the_eight_tool_ids_are_the_protocols() {
    let names: Vec<&str> = ToolId::ALL.iter().map(|tool| tool.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "spammer",
            "holdspace",
            "buy",
            "sell",
            "tuner",
            "gem",
            "quest",
            "pet",
        ]
    );
}

#[test]
fn a_tool_id_round_trips_through_its_wire_name() {
    for tool in ToolId::ALL {
        assert_eq!(ToolId::parse(tool.as_str()), Some(tool));
    }

    assert_eq!(ToolId::parse("pet "), None, "no trimming: ids are exact");
    assert_eq!(ToolId::parse("hover"), None);
    assert_eq!(ToolId::parse(""), None);
}

#[test]
fn the_pet_feeder_is_the_resident_tool() {
    // LauncherService.ResidentId: the one tool that owns a schedule rather than an operation.
    assert!(ToolId::Pet.is_resident());
    for tool in ToolId::ALL.iter().filter(|tool| **tool != ToolId::Pet) {
        assert!(!tool.is_resident(), "{tool}");
    }
}

// ── the lifecycle phase ────────────────────────────────────────────────────────────────────────

#[test]
fn every_phase_has_the_lowercase_name_the_shell_renders() {
    assert_eq!(ToolPhase::Idle.as_str(), "idle");
    assert_eq!(ToolPhase::Starting.as_str(), "starting");
    assert_eq!(ToolPhase::Running.as_str(), "running");
    assert_eq!(ToolPhase::Stopping.as_str(), "stopping");
}

#[test]
fn engaged_means_anything_but_idle() {
    // `PROGRESS.md` entry 57: the question that has no window is "is it engaged at all?", not "is it
    // running?" — the toggle was wrong three times by asking the second.
    assert!(!ToolPhase::Idle.engaged());
    assert!(ToolPhase::Starting.engaged());
    assert!(ToolPhase::Running.engaged());
    assert!(ToolPhase::Stopping.engaged());
}

// ── the snapshot ──────────────────────────────────────────────────────────────────────────────

#[test]
fn a_new_snapshot_is_idle_and_empty() {
    let snapshot = ToolSnapshot::new(ToolId::Tuner);

    assert_eq!(snapshot.tool, ToolId::Tuner);
    assert!(!snapshot.running);
    assert_eq!(snapshot.phase, ToolPhase::Idle);
    assert_eq!(snapshot.message, "");
    assert_eq!(snapshot.countdown_s, None);
    assert!(snapshot.rows.is_empty());
    assert_eq!(snapshot.error, None);
}

#[test]
fn a_snapshot_is_built_by_replacing_whole_values() {
    let snapshot = ToolSnapshot::new(ToolId::Pet)
        .running(ToolPhase::Running)
        .message("row 2: looking")
        .countdown_s(Some(12.5))
        .rows(vec![
            ToolRow::new("row 1", "boarding").detail(Some("藍天 · 3/min")),
            ToolRow::new("row 2", "waiting for a reload").next_action_in_seconds(Some(30.0)),
        ])
        .error(None);

    assert_eq!(snapshot.tool, ToolId::Pet);
    assert!(snapshot.running);
    assert_eq!(snapshot.phase, ToolPhase::Running);
    assert_eq!(snapshot.message, "row 2: looking");
    assert_eq!(snapshot.countdown_s, Some(12.5));
    assert_eq!(snapshot.rows.len(), 2);
    assert_eq!(snapshot.rows[0].detail.as_deref(), Some("藍天 · 3/min"));
    assert_eq!(snapshot.rows[1].next_action_in_seconds, Some(30.0));
    assert_eq!(snapshot.error, None);
}

#[test]
fn a_finished_run_publishes_the_reason_the_player_reads() {
    let snapshot = ToolSnapshot::new(ToolId::Quest)
        .stopped("DONE — 4 row(s)")
        .error(None);

    assert!(!snapshot.running);
    assert_eq!(snapshot.phase, ToolPhase::Idle);
    assert_eq!(snapshot.message, "DONE — 4 row(s)");
}

#[test]
fn an_error_rides_beside_the_message_rather_than_replacing_it() {
    let snapshot = ToolSnapshot::new(ToolId::Gem)
        .running(ToolPhase::Running)
        .message("stopping")
        .error(Some("OCR failed: the crop was blank".to_owned()));

    assert_eq!(snapshot.message, "stopping");
    assert_eq!(
        snapshot.error.as_deref(),
        Some("OCR failed: the crop was blank")
    );
}

#[test]
fn snapshots_are_comparable() {
    let left = ToolSnapshot::new(ToolId::Spammer).message("a");
    let right = ToolSnapshot::new(ToolId::Spammer).message("a");
    let different = ToolSnapshot::new(ToolId::Spammer).message("b");

    assert_eq!(left, right);
    assert_ne!(left, different);
}

// ── StateCell: whole snapshots, never a torn pair ─────────────────────────────────────────────

#[test]
fn a_published_snapshot_is_read_back_whole() {
    let cell = StateCell::new(ToolSnapshot::new(ToolId::Tuner));
    let published = ToolSnapshot::new(ToolId::Tuner)
        .running(ToolPhase::Running)
        .message("attempt 3")
        .countdown_s(Some(0.4));

    cell.publish(published.clone());

    assert_eq!(*cell.read().expect("published"), published);
}

#[test]
fn a_read_snapshot_is_immutable_after_a_new_publish() {
    let cell = StateCell::new(ToolSnapshot::new(ToolId::Buy));
    let first = ToolSnapshot::new(ToolId::Buy).message("first");
    cell.publish(first.clone());
    let held = cell.read().expect("published");

    cell.publish(ToolSnapshot::new(ToolId::Buy).message("second"));

    assert_eq!(held.message, "first", "an old read cannot be rewritten");
    assert_eq!(cell.read().expect("published").message, "second");
}

/// The torn-read test: the pair `message` ↔ `countdown_s` ↔ `rows[0].status` is written together and
/// must always be read together. Two threads run through a barrier, the writer publishes 5 000
/// snapshots, and the reader checks every snapshot it sees is internally consistent — the failure a
/// `int?`/`DateTime?` pair allowed is impossible from the types, and this is the proof for this
/// representation.
#[test]
fn a_concurrent_reader_never_sees_a_torn_snapshot() {
    const PUBLISHES: i64 = 5_000;

    /// One snapshot whose three related fields are derived from the same cycle number, so a reader
    /// can check that the fields it read were written together.
    fn marked(cycle: i64) -> ToolSnapshot {
        ToolSnapshot::new(ToolId::Pet)
            .running(ToolPhase::Running)
            .message(format!("cycle {cycle}"))
            .countdown_s(Some(cycle as f64))
            .rows(vec![ToolRow::new("row 1", cycle.to_string())])
    }

    // The initial value is marked too: the reader must be able to check EVERY snapshot it sees,
    // including the one published before either thread started.
    let cell = StateCell::new(marked(-1));
    let done = Arc::new(AtomicBool::new(false));
    let barrier = Arc::new(Barrier::new(2));

    let writer = {
        let cell = cell.clone();
        let done = Arc::clone(&done);
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            for cycle in 0..PUBLISHES {
                cell.publish(marked(cycle));
            }
            done.store(true, Ordering::SeqCst);
        })
    };

    barrier.wait();
    let mut reads = 0u32;
    loop {
        let snapshot = cell.read().expect("a snapshot is published");
        let countdown = snapshot
            .countdown_s
            .expect("every published snapshot carries its countdown");
        assert_eq!(snapshot.message, format!("cycle {countdown}"));
        assert_eq!(
            snapshot.rows[0].status,
            format!("{countdown}"),
            "the row belongs to the same snapshot as the message"
        );
        reads += 1;

        if done.load(Ordering::SeqCst) {
            break;
        }
        assert!(reads < 10_000_000, "the writer must have finished");
    }
    writer.join().expect("the writer thread ends");

    assert!(reads >= 1);
    // And the last read after the join is the last publish, whole.
    assert_eq!(
        cell.read().expect("published").message,
        format!("cycle {}", PUBLISHES - 1)
    );
}

#[test]
fn repeated_reads_return_the_same_immutable_snapshot() {
    // A reader clones the Arc under a READ lock and releases it before rendering. This pins the
    // shape rather than the timing: reads with no publish between them return the same snapshot.
    let cell = StateCell::new(ToolSnapshot::new(ToolId::Sell));
    cell.publish(ToolSnapshot::new(ToolId::Sell).message("final"));

    let first = cell.read().expect("published");
    let second = cell.read().expect("published");

    assert!(Arc::ptr_eq(&first, &second));
}

//! Mid-run pet state persistence: the A1 safety path.
//!
//! v2's pet feeder wrote `food_slots_used`, each row's `boarding_running`, and the close-guard
//! belief back into `local.yaml` WHILE the run was live (`LauncherService.PersistPetState`). Without
//! that, a long run restarted after a crash resumes at the first food cell and can press a boarding
//! toggle with the wrong belief about which way it faces.
//!
//! The write path is deliberately separate from `config.apply`: a tool worker calls
//! [`PetStateSink::persist`], the `ConfigStore` performs its usual gated read-modify-write (one
//! writer, atomic replace), then the session revision is bumped and ONE `config.changed` is emitted.
//! There is no request/response because there is no request: this is the core writing its own run
//! state. Persistence is a safety net, not the run: a failed write is logged and the run continues.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use seal_config::{ConfigStore, PetRunStatePatch};
use serde_json::Value;

use crate::events::{self, EventBus};
use crate::logs::Logs;
use crate::registry::SharedConfig;

/// A tool's sink for pet run-state updates. Implementations must not block the run on I/O failure:
/// the production sink logs and returns. A test can record the patches instead.
pub trait PetStateSink: Send + Sync {
    fn persist(&self, patch: &PetRunStatePatch);

    /// The command-caller half: same write/event path, with the revision returned for the response.
    /// The default keeps a recording test sink source-compatible (it has no revision to report).
    ///
    /// # Errors
    ///
    /// The default recording sink cannot fail, so this returns `Ok` with revision 0; a store-backed sink returns its write failure.
    fn persist_rev(&self, patch: &PetRunStatePatch) -> Result<u64, String> {
        self.persist(patch);
        Ok(0)
    }
}

/// The production sink: store write → revision bump → shared mirror → one event. The revision is the
/// same arc the dispatcher uses, so the next shell command sees the write's revision even though no
/// response was involved.
pub struct ConfigPetStateSink {
    store: ConfigStore,
    revision: Arc<AtomicU64>,
    bus: EventBus,
    logs: Arc<Logs>,
    shared: SharedConfig,
}

impl ConfigPetStateSink {
    pub fn new(
        store: ConfigStore,
        revision: Arc<AtomicU64>,
        bus: EventBus,
        logs: Arc<Logs>,
        shared: SharedConfig,
    ) -> Self {
        Self {
            store,
            revision,
            bus,
            logs,
            shared,
        }
    }

    /// The write half: persist the patch, reload the merged config, refresh the shared mirror, and
    /// bump the shared revision. It deliberately does NOT emit the event — a command caller queues
    /// the same `config.changed` after its response (`config.apply`'s order); a live tool calls
    /// [`Self::persist_once`] and emits immediately.
    ///
    /// # Errors
    ///
    /// Returns the line when the patch is empty or the store refuses the write.
    pub fn write_rev(&self, patch: &PetRunStatePatch) -> Result<u64, String> {
        if patch.is_empty() {
            return Err("an empty pet run-state patch is not a write".to_owned());
        }
        self.store
            .persist_pet_run_state(patch)
            .map_err(|error| error.to_string())?;
        let config = self.store.load().map_err(|error| error.to_string())?;
        if let Ok(mut shared) = self.shared.write() {
            *shared = config.clone();
        }
        Ok(self.revision.fetch_add(1, Ordering::SeqCst) + 1)
    }

    /// The live-tool half: write, then emit the one `config.changed` immediately (there is no
    /// response to order it after).
    ///
    /// # Errors
    ///
    /// Returns the line when the write fails or the config cannot be re-read for the emitted event.
    pub fn persist_once(&self, patch: &PetRunStatePatch) -> Result<u64, String> {
        let rev = self.write_rev(patch)?;
        let config = self.store.load().map_err(|error| error.to_string())?;
        let values = config.get("pet").cloned().unwrap_or(Value::Null);
        self.bus
            .emit(events::config_changed_event(rev, "pet", values));
        Ok(rev)
    }
}

impl PetStateSink for ConfigPetStateSink {
    fn persist_rev(&self, patch: &PetRunStatePatch) -> Result<u64, String> {
        // Command callers queue the event after their response.
        self.write_rev(patch)
    }

    fn persist(&self, patch: &PetRunStatePatch) {
        if let Err(error) = self.persist_once(patch) {
            // v2's rule: losing the state update is bad, but killing a live feeder over it is worse.
            self.logs
                .error_line(&format!("[pet] couldn't persist run state: {error}"));
        }
    }
}

/// A sink for tools/tests that do not persist. Production pet runs get
/// [`ConfigPetStateSink`]; unit tests that only exercise the decision half use this so they do not
/// need a config root.
pub struct NoPetState;

impl PetStateSink for NoPetState {
    fn persist(&self, _patch: &PetRunStatePatch) {}
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::RwLock;
    use std::sync::atomic::AtomicUsize;

    use seal_config::ConfigStore;
    use seal_proto::Event;
    use serde_json::json;

    use super::*;

    /// A directory under the workspace target tree (the harness's system temp is not reliable for
    /// test process writes), removed on drop.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/pet-state-tests")
                .join(format!(
                    "{}-{}",
                    std::process::id(),
                    COUNTER.fetch_add(1, Ordering::Relaxed)
                ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &PathBuf {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (TestDir, ConfigStore) {
        let dir = TestDir::new();
        std::fs::write(
            dir.path().join("defaults.yaml"),
            "window:\n  title: TEST\npet:\n  food_slots_used: 0\n  slots:\n  - boarding_running: false\n  - boarding_running: false\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("local.yaml"),
            "pet:\n  food_slots_used: 0\n  slots:\n  - boarding_running: false\n  - boarding_running: false\nforeign:\n  keep: me\n",
        )
        .unwrap();
        let store = ConfigStore::new(dir.path());
        (dir, store)
    }

    #[test]
    fn one_patch_writes_one_event_with_one_new_revision() {
        let (dir, store) = store();
        let (bus, events) = events::event_channel();
        let shared: SharedConfig = Arc::new(RwLock::new(store.load().unwrap()));
        let revision = Arc::new(AtomicU64::new(0));
        let sink = ConfigPetStateSink::new(
            store.clone(),
            Arc::clone(&revision),
            bus,
            Arc::new(Logs::disabled()),
            shared,
        );

        let foreign_before = "foreign:\n  keep: me\n";
        assert!(dir.path().join("local.yaml").is_file());
        let before = std::fs::read_to_string(dir.path().join("local.yaml")).unwrap();

        let patch = PetRunStatePatch {
            food_slots_used: Some(4),
            boarding_running: vec![(1, true)],
            ..PetRunStatePatch::default()
        };
        let rev = sink.persist_once(&patch).expect("writes");
        assert_eq!(rev, 1);
        assert_eq!(revision.load(Ordering::SeqCst), 1);

        let local = store.load_local().expect("local parses");
        assert_eq!(local["pet"]["food_slots_used"], 4);
        assert_eq!(local["pet"]["slots"][1]["boarding_running"], true);
        assert_eq!(local["foreign"]["keep"], "me", "foreign block preserved");
        let after = std::fs::read_to_string(dir.path().join("local.yaml")).unwrap();
        assert!(
            after.ends_with(foreign_before),
            "the foreign block is carried verbatim: {after}"
        );
        assert_ne!(after, before, "the write did change the file");
        assert_eq!(
            local["pet"]["slots"][0],
            json!({"boarding_running": false}),
            "the rest of the pet block is field-wise untouched"
        );

        let event: Event = events.recv().expect("one event");
        assert_eq!(event.event, "config.changed");
        assert_eq!(event.data["rev"], 1);
        assert_eq!(event.data["block"], "pet");
        assert_eq!(event.data["values"]["food_slots_used"], 4);
        assert!(events.try_recv().is_err(), "exactly one event");
        drop(dir);
    }

    #[test]
    fn a_missing_pet_block_is_created_and_closed_guard_fields_round_trip() {
        let dir = TestDir::new();
        std::fs::write(dir.path().join("defaults.yaml"), "window:\n  title: TEST\n").unwrap();
        std::fs::write(
            dir.path().join("local.yaml"),
            "pet:\n  slots:\n  - boarding_running: false\n",
        )
        .unwrap();
        let store = ConfigStore::new(dir.path());
        let (bus, _events) = events::event_channel();
        let shared: SharedConfig = Arc::new(RwLock::new(store.load().unwrap()));
        let sink = ConfigPetStateSink::new(
            store.clone(),
            Arc::new(AtomicU64::new(0)),
            bus,
            Arc::new(Logs::disabled()),
            shared,
        );

        sink.persist_once(&PetRunStatePatch {
            food_slots_used: Some(7),
            close_button_png: Some(Some("cached".to_owned())),
            boarding_window_may_be_open: Some(true),
            ..PetRunStatePatch::default()
        })
        .expect("writes");

        let local = store.load_local().expect("local parses");
        assert_eq!(local["pet"]["food_slots_used"], 7);
        assert_eq!(local["pet"]["close_button_png"], "cached");
        assert_eq!(local["pet"]["boarding_window_may_be_open"], true);
        drop(dir);
    }

    #[test]
    fn a_concurrent_shell_apply_and_run_update_both_survive() {
        use std::sync::Barrier;
        use std::thread;

        let (_dir, store) = store();
        let barrier = Arc::new(Barrier::new(2));
        let writer_store = store.clone();
        let apply_store = store.clone();
        let writer_barrier = Arc::clone(&barrier);
        let apply_barrier = Arc::clone(&barrier);

        let apply_sections = serde_json::json!({"pet": {"food_load_mode": "drag"}});
        let apply_sections = apply_sections.as_object().unwrap().clone();

        let writer = thread::spawn(move || {
            writer_barrier.wait();
            writer_store
                .persist_pet_run_state(&PetRunStatePatch {
                    food_slots_used: Some(4),
                    ..PetRunStatePatch::default()
                })
                .expect("the run write succeeds");
        });
        let shell = thread::spawn(move || {
            apply_barrier.wait();
            apply_store
                .apply_sections(&apply_sections)
                .expect("the shell apply succeeds");
        });
        writer.join().expect("writer thread");
        shell.join().expect("shell thread");

        let local = store.load_local().expect("local parses");
        assert_eq!(local["pet"]["food_slots_used"], 4);
        assert_eq!(local["pet"]["food_load_mode"], "drag");
        assert_eq!(local["foreign"]["keep"], "me");
    }

    #[test]
    fn a_write_failure_logs_and_does_not_panic() {
        let dir = TestDir::new();
        let bad = dir.path().join("bad-config");
        std::fs::create_dir_all(&bad).unwrap();
        // A directory where local.yaml should be makes the store's read fail before any write.
        std::fs::create_dir_all(bad.join("local.yaml")).unwrap();
        let bad_store = ConfigStore::new(&bad);
        let (bus, _events) = events::event_channel();
        let shared: SharedConfig = Arc::new(RwLock::new(serde_json::Value::Null));
        let logs = Arc::new(Logs::new(dir.path()));
        let sink = ConfigPetStateSink::new(
            bad_store,
            Arc::new(AtomicU64::new(0)),
            bus,
            Arc::clone(&logs),
            shared,
        );

        sink.persist(&PetRunStatePatch {
            food_slots_used: Some(2),
            ..PetRunStatePatch::default()
        });

        let error = std::fs::read_to_string(dir.path().join("logs/error.log"))
            .expect("the failure was logged");
        assert!(
            error.contains("couldn't persist run state"),
            "the log names the failure: {error}"
        );
        // No panic and no process/run state mutation: the caller (the feeder) keeps running.
    }
}

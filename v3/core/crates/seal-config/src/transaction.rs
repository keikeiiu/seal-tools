//! Increment 4 of PLAN-PORTABLE-SETUP: the two-file transaction and crash recovery.
//!
//! A per-file save is one atomic temp+fsync+rename (increment 1). The Setup card (setup/common +
//! machine/arduino) and the Pet card (setup/pet + machine/pet) span TWO files, and one Save must
//! stay one revision with one failure mode: either both files take the new content or neither does.
//!
//! The protocol is a small write-ahead journal:
//!
//! 1. create `.seal-txn/txn-<pid>-<nanos>/`;
//! 2. for every target: copy the old file to `<index>.bak` (or record `existed:false`), serialise
//!    the new content to `<index>.tmp` and fsync it;
//! 3. write `journal.json` atomically — this is the commit point for RECOVERY, but note the targets
//!    are still untouched, so a crash before this leaves no visible change;
//! 4. rename every temp over its target (the commit itself);
//! 5. remove the transaction directory.
//!
//! Recovery ALWAYS ROLLS BACK: a journal that still exists means the commit did not finish, so every
//! target is restored from its backup (or removed when it did not exist). That is deterministic and
//! idempotent: a crash during recovery leaves the journal in place and the next boot restores again.
//! Rolling FORWARD was rejected because "some renames already happened" makes which half is newer
//! ambiguous; the player's guarantee is the simpler "no Save is half-applied".
//!
//! The directory is machine-local bookkeeping (`.seal-txn/`), never player state, and is removed
//! after every successful commit; `recover` sweeps it before a read, a write, or a layout load.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::error::ConfigError;
use crate::file;
use crate::registry::FileDef;

/// The directory under the config root; hidden from the player and never in a release zip.
pub const JOURNAL_DIR: &str = ".seal-txn";

/// The commit marker: a finished commit renames `journal.json` to this BEFORE the best-effort
/// cleanup. Recovery treats it as committed and only cleans up, never rolls back a save the player
/// already saw succeed (HW-F2).
const JOURNAL_DONE: &str = "journal.done";

/// One file's new content, ready to be committed together with the others.
pub struct PendingWrite {
    pub file: &'static FileDef,
    pub content: Value,
}

impl PendingWrite {
    fn path(&self, root: &Path) -> PathBuf {
        file::path_of(root, self.file)
    }
}

fn unique_txn_name() -> String {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("txn-{:x}-{nanos:x}-{sequence:x}", std::process::id())
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    output.write_all(bytes)?;
    output.sync_all()
}

/// Commit up to two files as one unit. On any error before the journal's renames the targets are
/// untouched; an error during the renames triggers a rollback.
///
/// # Errors
///
/// Returns [`ConfigError`] when the journal directory, a backup copy, an encoded temp file, the
/// journal itself, or one of the target renames cannot be written. A rename that fails after an
/// earlier one succeeded is rolled back before the error is returned.
pub fn commit(root: &Path, writes: &[PendingWrite]) -> Result<(), ConfigError> {
    if writes.is_empty() {
        return Ok(());
    }
    recover(root)?;
    let directory = root.join(JOURNAL_DIR).join(unique_txn_name());
    fs::create_dir_all(&directory).map_err(|source| ConfigError::Write {
        path: directory.clone(),
        source,
    })?;

    let mut entries = Vec::with_capacity(writes.len());
    for (index, write) in writes.iter().enumerate() {
        let target = write.path(root);
        let existed = target.is_file();
        let backup_name = format!("{index}.bak");
        let temp_name = format!("{index}.tmp");
        if existed {
            // A backup must be DURABLE before the journal points at it: a power loss must not leave
            // a zero-filled .bak that recovery would copy over the good target (HW-F2).
            let bytes = fs::read(&target).map_err(|source| ConfigError::Read {
                path: target.clone(),
                source,
            })?;
            write_synced(&directory.join(&backup_name), &bytes).map_err(|source| {
                ConfigError::Write {
                    path: directory.join(&backup_name),
                    source,
                }
            })?;
        }
        let text = file::encode(write.file, &write.content)?;
        write_synced(&directory.join(&temp_name), text.as_bytes()).map_err(|source| {
            ConfigError::Write {
                path: directory.join(&temp_name),
                source,
            }
        })?;
        entries.push(json!({
            "path": write.file.path,
            "backup": backup_name,
            "temp": temp_name,
            "existed": existed,
        }));
    }

    let journal = json!({"v": 1, "files": entries});
    let journal_path = directory.join("journal.json");
    let journal_text =
        serde_json::to_string_pretty(&journal).map_err(|error| ConfigError::Serialize {
            path: journal_path.clone(),
            message: error.to_string(),
        })?;
    let journal_temp = directory.join("journal.tmp");
    write_synced(&journal_temp, journal_text.as_bytes()).map_err(|source| ConfigError::Write {
        path: journal_temp.clone(),
        source,
    })?;
    fs::rename(&journal_temp, &journal_path).map_err(|source| ConfigError::Write {
        path: journal_path.clone(),
        source,
    })?;

    let mut failure = None;
    for (index, write) in writes.iter().enumerate() {
        let target = write.path(root);
        if let Some(parent) = target.parent()
            && let Err(source) = fs::create_dir_all(parent)
        {
            failure = Some(ConfigError::Write {
                path: parent.to_path_buf(),
                source,
            });
            break;
        }
        if let Err(source) = fs::rename(directory.join(format!("{index}.tmp")), &target) {
            failure = Some(ConfigError::Write {
                path: target,
                source,
            });
            break;
        }
    }
    if let Some(error) = failure {
        // Roll back both sides; if recovery itself fails the journal stays and the next boot retries.
        let _ = recover(root);
        return Err(error);
    }
    // The renames are the commit. Mark it explicitly and check that step: removing the journal only
    // best-effort would let a surviving journal roll back a save the player was told succeeded.
    let done_marker = directory.join(JOURNAL_DONE);
    fs::rename(&journal_path, &done_marker).map_err(|source| ConfigError::Write {
        path: done_marker,
        source,
    })?;
    let _ = fs::remove_dir_all(&directory);
    // The container is machine-local scratch; leave no empty `.seal-txn/` behind on a clean tree.
    let _ = fs::remove_dir(root.join(JOURNAL_DIR));
    Ok(())
}

/// Sweep every journal left by an interrupted commit, restoring every target from its backup. Returns
/// the number of transactions rolled back (0 when the tree is clean). Idempotent.
///
/// # Errors
///
/// Returns [`ConfigError`] when a journal exists but cannot be read or parsed, when a backup cannot
/// be read back, when a restored file cannot be written, or when the swept transaction directory
/// cannot be removed.
pub fn recover(root: &Path) -> Result<usize, ConfigError> {
    let journal_root = root.join(JOURNAL_DIR);
    let Ok(entries) = fs::read_dir(&journal_root) else {
        return Ok(0);
    };
    let mut recovered = 0;
    for entry in entries.flatten() {
        let directory = entry.path();
        if !directory.is_dir() {
            continue;
        }
        let journal_path = directory.join("journal.json");
        let done_path = directory.join(JOURNAL_DONE);
        if done_path.is_file() {
            // The renames completed; only the best-effort cleanup was interrupted. Never roll back.
            fs::remove_dir_all(&directory).map_err(|source| ConfigError::Write {
                path: directory.clone(),
                source,
            })?;
            recovered += 1;
            continue;
        }
        if !journal_path.is_file() {
            // No journal means no rename happened; only unprepared files are inside.
            let _ = fs::remove_dir_all(&directory);
            continue;
        }
        let text = fs::read_to_string(&journal_path).map_err(|source| ConfigError::Read {
            path: journal_path.clone(),
            source,
        })?;
        let journal = serde_json::from_str::<Value>(&text).map_err(|error| ConfigError::Parse {
            path: journal_path.clone(),
            message: error.to_string(),
        })?;
        let files = journal
            .get("files")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for file_entry in files {
            let Some(path) = file_entry.get("path").and_then(Value::as_str) else {
                continue;
            };
            let target = root.join(path);
            let existed = file_entry
                .get("existed")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if existed {
                let backup = file_entry
                    .get("backup")
                    .and_then(Value::as_str)
                    .map(|name| directory.join(name));
                let Some(backup) = backup else {
                    return Err(ConfigError::Parse {
                        path: journal_path.clone(),
                        message: format!("{path} existed but the journal names no backup"),
                    });
                };
                if !backup.is_file() {
                    // Restoring one side from a missing backup would produce a HALF-applied save.
                    return Err(ConfigError::Parse {
                        path: backup,
                        message: format!(
                            "{path} had a backup in the journal, but it is missing; refusing to \
                             restore a partial save"
                        ),
                    });
                }
                let bytes = fs::read(&backup).map_err(|source| ConfigError::Read {
                    path: backup.clone(),
                    source,
                })?;
                file::write_bytes(&target, &bytes)?;
            } else {
                let _ = fs::remove_file(&target);
            }
            if let Some(temp) = file_entry.get("temp").and_then(Value::as_str) {
                let _ = fs::remove_file(directory.join(temp));
            }
        }
        fs::remove_dir_all(&directory).map_err(|source| ConfigError::Write {
            path: directory.clone(),
            source,
        })?;
        recovered += 1;
    }
    let _ = fs::remove_dir(&journal_root);
    Ok(recovered)
}

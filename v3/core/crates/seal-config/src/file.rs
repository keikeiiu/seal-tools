//! The generalised config-file layer (increment 1 of PLAN-PORTABLE-SETUP).
//!
//! One data file is one class × one domain (the registry names them); this module owns the file
//! FORMAT rules so the later loader/writer only route paths:
//!
//! - a file is a YAML mapping; `schema_version: 1` is the only reserved key. It is stripped on
//!   read (the merged config never sees it) and written back on every save. A newer version is
//!   refused loudly, writing nothing, rather than guessed at;
//! - every save writes a per-file header explaining whether the file travels, and that unknown
//!   keys are kept (they are: the caller's full mapping is written, not a fixed field list);
//! - a save is atomic: temp file in the SAME directory, `sync_all`, rename over the destination.
//!   The rename cannot cross a volume, and a failed write leaves the old file whole.
//!
//! The existing single-file `local.yaml` store is untouched: increment 1 is the layer the
//! multi-file loader/writer will sit on, and its behaviour is pinned by `tests/file_layer.rs`.

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

use crate::error::ConfigError;
use crate::registry::{self, Class, Domain, FileDef, SCHEMA_VERSION};
use crate::yaml;

/// The only reserved key inside a data file.
pub const SCHEMA_KEY: &str = "schema_version";

/// The raw Windows errors a rename meets while another process still holds the destination open:
/// `ERROR_ACCESS_DENIED` (5), `ERROR_SHARING_VIOLATION` (32) and `ERROR_LOCK_VIOLATION` (33). An
/// editor, a cloud-sync client or an AV scanner that opened the file without `FILE_SHARE_DELETE` is
/// a TEMPORARY holder, so the save waits it out instead of being lost (HW-F5).
const TRANSIENT_RENAME_CODES: [i32; 3] = [5, 32, 33];

/// The wait before each retry: five attempts spanning about 300 ms. Long enough for a scanner or a
/// sync client to let go, short enough that a genuinely locked file still reports itself to the
/// player in the same second.
const RENAME_RETRY_MS: [u64; 4] = [20, 40, 80, 160];

/// How old a `.tmp` must be before the loader treats it as a crashed writer's litter. A live writer
/// holds its temp for milliseconds, so an hour cannot race one (HW-F5).
pub const STALE_TEMP_AGE: Duration = Duration::from_hours(1);

/// The on-disk path of `file` under `root` (a config folder).
#[must_use]
pub fn path_of(root: &Path, file: &FileDef) -> PathBuf {
    root.join(file.path)
}

/// Read one data file. `Ok(None)` means the file is absent (a file that is missing contributes
/// nothing; missing is "not stated", exactly like today's overlay). A present file must be a
/// mapping; a newer `schema_version` is [`ConfigError::SchemaTooNew`] and writes nothing.
///
/// # Errors
///
/// Returns [`ConfigError`] when the file exists but cannot be read, when its text is not a YAML
/// mapping, or when its `schema_version` is zero or newer than [`SCHEMA_VERSION`].
pub fn read(root: &Path, file: &FileDef) -> Result<Option<Value>, ConfigError> {
    let path = path_of(root, file);
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
        path: path.clone(),
        source,
    })?;
    let parsed = yaml::parse(&text).map_err(|message| ConfigError::Parse {
        path: path.clone(),
        message,
    })?;
    // A header-only file parses as null; for this layer that is the empty mapping, the same
    // meaning an empty `local.yaml` has today.
    let mut mapping = match parsed {
        Value::Null => Map::new(),
        Value::Object(mapping) => mapping,
        _ => {
            return Err(ConfigError::Parse {
                path,
                message: "a config data file must be a YAML mapping, not a scalar or a list"
                    .to_owned(),
            });
        }
    };
    if let Some(version) = mapping.remove(SCHEMA_KEY) {
        let Some(found) = version.as_u64() else {
            return Err(ConfigError::Parse {
                path,
                message: format!("schema_version must be a positive integer, found {version}"),
            });
        };
        if found < u64::from(SCHEMA_VERSION) && found == 0 {
            return Err(ConfigError::Parse {
                path,
                message: "schema_version must be at least 1".to_owned(),
            });
        }
        if found > u64::from(SCHEMA_VERSION) {
            return Err(ConfigError::SchemaTooNew {
                path,
                found,
                supported: SCHEMA_VERSION,
            });
        }
    }
    Ok(Some(Value::Object(mapping)))
}

/// Read a data file the way a LOADER wants it: a corrupt MACHINE file is quarantined beside itself
/// and skipped with a warning, so one bad measurement file cannot stop the core from starting; a
/// corrupt setup file is the player's portable data and still fails loudly, named with its line
/// (HW-F15). A newer `schema_version` is never quarantined: that file came from a newer build and
/// moving it away would destroy data the player still needs.
///
/// # Errors
///
/// Returns [`ConfigError::Read`] when the file cannot be opened, [`ConfigError::Write`] when a
/// corrupt machine file cannot be renamed aside, and the [`read`] errors for every other class.
pub fn read_or_quarantine(
    root: &Path,
    file: &FileDef,
) -> Result<(Option<Value>, Option<String>), ConfigError> {
    match read(root, file) {
        Ok(content) => Ok((content, None)),
        Err(error @ ConfigError::Parse { .. }) if file.class == Class::Machine => {
            let path = path_of(root, file);
            let quarantine = quarantine_path(&path);
            // The file this pattern hits (a sync client or an editor holding it) is also one whose
            // rename can be refused transiently, so the sharing retry applies here too.
            rename_with_retry(&path, &quarantine).map_err(|source| ConfigError::Write {
                path: path.clone(),
                source,
            })?;
            let name = quarantine.file_name().map_or_else(
                || quarantine.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            Ok((
                None,
                Some(format!(
                    "{error}; the file was moved aside to {name} so the rest of the config still \
                     loads — recalibrate what it held"
                )),
            ))
        }
        Err(error) => Err(error),
    }
}

/// `<file>.corrupt-<stamp>` beside the original: the player's bytes are kept, under a name no load
/// reads, so the evidence survives for a report (the same shape v2 kept, with a unique stamp).
fn quarantine_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".corrupt-{}", unique_stamp()));
    path.with_file_name(name)
}

/// A short unique suffix: seconds since the epoch plus a per-process sequence, so two quarantines in
/// one second cannot overwrite each other.
fn unique_stamp() -> String {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    format!("{seconds:x}-{sequence:x}")
}

/// Write one data file atomically, with its header and `schema_version: 1`. Unknown keys are kept
/// because the whole mapping is written; a reserved `schema_version` supplied by the caller is
/// replaced by the current one.
///
/// # Errors
///
/// Returns [`ConfigError::Serialize`] when `content` is not a JSON object, and [`ConfigError::Write`]
/// when the directory cannot be created or the temp-file write, sync, or rename fails.
pub fn write(root: &Path, file: &FileDef, content: &Value) -> Result<(), ConfigError> {
    let path = path_of(root, file);
    let text = encode(file, content)?;
    write_text(&path, &text)
}

/// Serialise one data file exactly as `write` will: header, reserved `schema_version`, then the
/// caller's mapping. The two-file transaction needs the bytes WITHOUT writing, so the format lives
/// here once for both paths.
///
/// # Errors
///
/// Returns [`ConfigError::Serialize`] when `content` is not a JSON object or the YAML emitter
/// refuses the mapping.
pub fn encode(file: &FileDef, content: &Value) -> Result<String, ConfigError> {
    let path = path_of(Path::new(""), file);
    let Some(mapping) = content.as_object() else {
        return Err(ConfigError::Serialize {
            path,
            message: "a config data file's content must be a JSON object".to_owned(),
        });
    };
    let mut data = Map::new();
    data.insert(SCHEMA_KEY.to_owned(), Value::from(SCHEMA_VERSION));
    for (key, value) in mapping {
        if key != SCHEMA_KEY {
            data.insert(key.clone(), value.clone());
        }
    }
    let body = yaml::to_string(&Value::Object(data))
        .map_err(|message| ConfigError::Serialize { path, message })?;
    Ok(format!("{}{body}", header(file)))
}

/// Replace `path` with `text` atomically: temp file in the same directory, `sync_all`, rename.
pub(crate) fn write_text(path: &Path, text: &str) -> Result<(), ConfigError> {
    atomic_replace(path, text.as_bytes())
}

/// Replace `path` with raw `bytes` atomically. Rollback restores a file byte for byte, so this is
/// the byte-level twin of [`write_text`] (used by the two-file transaction's recovery).
pub(crate) fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
    atomic_replace(path, bytes)
}

/// Replace `path` with `bytes` durably and atomically — the one writer every config save uses
/// (this layer, the legacy `local.yaml` store, the calibration artifacts and the transaction's
/// recovery).
///
/// The order is the durability contract: write the temp, `sync_all` it, then rename it over the
/// destination. The rename is retried over the transient Windows sharing failures, because a
/// destination another process holds open for a moment used to turn a Save into a lost save
/// (HW-F5).
///
/// # Errors
///
/// Returns [`ConfigError::Write`] when the parent directory cannot be created, the temp file cannot
/// be written or synced, or the rename keeps failing; a failed write removes its temp and reports
/// the FIRST failure rather than the cleanup's.
pub fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let temp = temp_path(path);
    let result = (|| -> std::io::Result<()> {
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        output.write_all(bytes)?;
        output.sync_all()?;
        drop(output);
        rename_with_retry(&temp, path)
    })();
    if let Err(source) = result {
        let _ = fs::remove_file(&temp);
        return Err(ConfigError::Write {
            path: path.to_path_buf(),
            source,
        });
    }
    Ok(())
}

/// Rename `from` over `to`, waiting out the transient sharing failures listed in
/// [`TRANSIENT_RENAME_CODES`]. Five attempts, 20→320 ms apart; any other error, or the last attempt,
/// is returned unchanged.
pub(crate) fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    retry_transient_rename(|| fs::rename(from, to))
}

fn retry_transient_rename(mut rename: impl FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
    let mut attempt = 0;
    loop {
        match rename() {
            Ok(()) => return Ok(()),
            Err(error) if attempt < RENAME_RETRY_MS.len() && is_transient_rename(&error) => {
                std::thread::sleep(Duration::from_millis(RENAME_RETRY_MS[attempt]));
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_transient_rename(error: &std::io::Error) -> bool {
    error
        .raw_os_error()
        .is_some_and(|code| TRANSIENT_RENAME_CODES.contains(&code))
}

/// Remove `*.tmp` files a crashed writer left in the config tree: the root, `setup/`, `machine/`
/// and `artifacts/`. Only files older than [`STALE_TEMP_AGE`] are touched, so a live writer's temp
/// is never swept out from under it. Best effort: a file another process still holds is left for
/// the next load rather than failing the read (HW-F5).
pub fn sweep_stale_temps(root: &Path) {
    let now = SystemTime::now();
    for folder in [
        root.to_path_buf(),
        root.join("setup"),
        root.join("machine"),
        root.join("artifacts"),
    ] {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            if !name.to_string_lossy().ends_with(".tmp") {
                continue;
            }
            let stale = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age >= STALE_TEMP_AGE);
            if stale {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// The header a file carries. It names the file's class and domain, so opening it in an editor
/// answers both questions without the docs.
#[must_use]
pub fn header(file: &FileDef) -> String {
    let (label, sentence) = match file.class {
        Class::Defaults => (
            "SHIPPED DEFAULTS",
            "The base every portable key falls back to. Do not copy this file into a setup folder.",
        ),
        Class::Setup => (
            "PORTABLE",
            "Copy config/setup/ (or this one file) to another PC. Calibration lives in machine/.",
        ),
        Class::Machine => (
            "PER-PC",
            "Measured on this machine. Never copy it; recalibrating regenerates it.",
        ),
        Class::User => (
            "YOUR NOTES",
            "Free-form notes/blocks; the core never reads or writes it.",
        ),
        Class::Release => ("RELEASE DATA", "Shipped read-only data; do not hand-edit."),
    };
    format!(
        "# ── Seal Tools config — {label} · {} ───────────────────────────────────────\n\
         # {sentence}\n\
         #\n\
         # schema_version: {SCHEMA_VERSION} is the only reserved key: it is stripped on read and\n\
         # rewritten on every save. unknown keys are kept inside this file; comments are lost on\n\
         # save, so keep prose in config/README.md or config/user.yaml.\n\
         #\n",
        domain_label(file.domain)
    )
}

fn domain_label(domain: Domain) -> &'static str {
    match domain {
        Domain::Common => "common",
        Domain::Tuner => "tuner",
        Domain::Gem => "gem",
        Domain::Spammer => "spammer",
        Domain::Quest => "quest",
        Domain::BuySell => "buy_sell",
        Domain::Pet => "pet",
        Domain::Display => "display",
        Domain::Tooltip => "tooltip",
        Domain::Ui => "ui",
        Domain::Arduino => "arduino",
        Domain::Meta => "meta",
        Domain::Attributes => "attributes",
        Domain::Artifacts => "artifacts",
    }
}

/// The unique temp name for one write: `<file>.<pid><nanos><sequence>.tmp` beside the destination.
/// Unique per call and per process, so a writer that died mid-save cannot leave a name the next one
/// collides with (the same rule the single-file writer already uses).
fn temp_path(path: &Path) -> PathBuf {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let mut name = OsString::from(path.file_name().unwrap_or_default());
    name.push(format!(
        ".{:x}{:x}{sequence:x}.tmp",
        std::process::id(),
        nanos
    ));
    path.with_file_name(name)
}

/// The registry entry for one path, for callers that route by path.
#[must_use]
pub fn file_for(path: &str) -> Option<&'static FileDef> {
    registry::FILES.iter().find(|file| file.path == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sharing_violation() -> std::io::Error {
        std::io::Error::from_raw_os_error(32)
    }

    #[test]
    fn a_rename_locked_twice_is_retried_until_it_succeeds() {
        // The save path that used to be lost: an editor, OneDrive or an AV scanner holds the
        // destination open for a moment, and the bare `fs::rename` reported the sharing violation
        // straight to the player (HW-F5).
        let mut attempts = 0;

        let result = retry_transient_rename(|| {
            attempts += 1;
            if attempts < 3 {
                Err(sharing_violation())
            } else {
                Ok(())
            }
        });

        assert!(result.is_ok());
        assert_eq!(attempts, 3, "two transient failures are waited out");
    }

    #[test]
    fn a_rename_that_never_unlocks_reports_the_last_failure_after_five_attempts() {
        let mut attempts = 0;

        let error = retry_transient_rename(|| {
            attempts += 1;
            Err(sharing_violation())
        })
        .expect_err("a permanently locked destination still fails");

        assert_eq!(attempts, 5, "four waits, five attempts");
        assert_eq!(error.raw_os_error(), Some(32));
    }

    #[test]
    fn a_permanent_error_is_not_retried() {
        let mut attempts = 0;

        let error = retry_transient_rename(|| {
            attempts += 1;
            Err(std::io::Error::from_raw_os_error(2)) // ERROR_FILE_NOT_FOUND
        })
        .expect_err("a missing source is permanent");

        assert_eq!(attempts, 1, "only the sharing failures are waited out");
        assert_eq!(error.raw_os_error(), Some(2));
    }
}

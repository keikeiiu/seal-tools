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
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

use crate::error::ConfigError;
use crate::registry::{self, Class, Domain, FileDef, SCHEMA_VERSION};
use crate::yaml;

/// The only reserved key inside a data file.
pub const SCHEMA_KEY: &str = "schema_version";

/// The on-disk path of `file` under `root` (a config folder).
pub fn path_of(root: &Path, file: &FileDef) -> PathBuf {
    root.join(file.path)
}

/// Read one data file. `Ok(None)` means the file is absent (a file that is missing contributes
/// nothing; missing is "not stated", exactly like today's overlay). A present file must be a
/// mapping; a newer `schema_version` is [`ConfigError::SchemaTooNew`] and writes nothing.
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

/// Write one data file atomically, with its header and `schema_version: 1`. Unknown keys are kept
/// because the whole mapping is written; a reserved `schema_version` supplied by the caller is
/// replaced by the current one.
pub fn write(root: &Path, file: &FileDef, content: &Value) -> Result<(), ConfigError> {
    let path = path_of(root, file);
    let text = encode(file, content)?;
    write_text(&path, &text)
}

/// Serialise one data file exactly as [`write`] will: header, reserved `schema_version`, then the
/// caller's mapping. The two-file transaction needs the bytes WITHOUT writing, so the format lives
/// here once for both paths.
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
        output.write_all(text.as_bytes())?;
        output.sync_all()?;
        drop(output);
        fs::rename(&temp, path)
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

/// Replace `path` with raw `bytes` atomically. Rollback restores a file byte for byte, so this is
/// the byte-level twin of [`write_text`] (used by the two-file transaction's recovery).
pub(crate) fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), ConfigError> {
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
        fs::rename(&temp, path)
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

/// The header a file carries. It names the file's class and domain, so opening it in an editor
/// answers both questions without the docs.
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
pub fn file_for(path: &str) -> Option<&'static FileDef> {
    registry::FILES.iter().find(|file| file.path == path)
}

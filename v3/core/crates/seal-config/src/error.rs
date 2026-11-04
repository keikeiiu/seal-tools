//! The one error type, replacing v2's `ConfigException`.
//!
//! Every variant names the file it is about, because a config failure with no path is the class of
//! message that makes a player's report unusable. `source()` keeps the underlying I/O error
//! reachable rather than flattened into a string.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// A config file could not be found, read, parsed, or written.
#[derive(Debug)]
pub enum ConfigError {
    /// A file the loader requires is absent (and, for the seeded files, has no `.example` either).
    MissingFile(PathBuf),
    /// The file exists but could not be read.
    Read { path: PathBuf, source: io::Error },
    /// The file was read but is not the YAML shape this application can use.
    Parse { path: PathBuf, message: String },
    /// A value could not be emitted as YAML. A bug rather than a file problem, but it names the
    /// destination so the report still says which write failed.
    Serialize { path: PathBuf, message: String },
    /// The atomic write failed. The file is unchanged: the temp file was removed or is ignorable.
    Write { path: PathBuf, source: io::Error },
    /// A caller tried to write a top-level block the application does not own. Writing one would
    /// serialise over the foreign-block text the writer carries through verbatim.
    SectionNotOwned(String),
    /// A data file states a newer `schema_version` than this core understands. Refused without
    /// touching the file: guessing could drop a field a newer build wrote.
    SchemaTooNew {
        path: PathBuf,
        found: u64,
        supported: u8,
    },
    /// The migration's verify-before-retire comparison failed: the new tree would not load back to
    /// the settings the old loader served, so nothing was retired and the files it wrote were
    /// restored from the backup. `differences` names the JSON pointers (HW-F4).
    MigrationMismatch {
        differences: Vec<String>,
        backup: PathBuf,
        message: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFile(path) => write!(f, "missing config file: {}", path.display()),
            Self::Read { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::Parse { path, message } => {
                write!(f, "failed to parse {}: {message}", path.display())
            }
            Self::Serialize { path, message } => {
                write!(f, "failed to serialize {}: {message}", path.display())
            }
            Self::Write { path, source } => write!(f, "cannot write {}: {source}", path.display()),
            Self::SectionNotOwned(name) => write!(
                f,
                "section \"{name}\" is not owned by the app, so a save cannot rewrite it"
            ),
            Self::SchemaTooNew {
                path,
                found,
                supported,
            } => write!(
                f,
                "{} states schema_version {found}, but this core understands at most {supported}; \
                 it was written by a newer build and was not changed",
                path.display()
            ),
            Self::MigrationMismatch {
                differences,
                backup,
                message,
            } => write!(
                f,
                "the layout migration was refused: the new tree would not load back to the settings \
                 the old one served ({}) — nothing was retired and the files it wrote were restored \
                 from {}{message}",
                differences.join(", "),
                backup.display()
            ),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            _ => None,
        }
    }
}

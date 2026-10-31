//! Finding the application root, the way v2 does it.

use std::path::{Path, PathBuf};

/// The marker in a checkout.
pub const DEFAULTS: &str = "config/defaults.yaml";

/// The marker in an install that has taken an update, where the shipped file is a template.
pub const DEFAULTS_TEMPLATE: &str = "config/defaults.yaml.example";

/// Walks up from `start` (or its parent, when it is a file) for the nearest directory that holds
/// either marker. The executable's neighbouring directory is the usual starting point; a test or
/// a future CLI may pass its own.
#[must_use]
pub fn find_root(start: &Path) -> Option<PathBuf> {
    let start = if start.is_dir() {
        start
    } else {
        start.parent()?
    };

    for dir in start.ancestors() {
        if dir.join(DEFAULTS).is_file() || dir.join(DEFAULTS_TEMPLATE).is_file() {
            return Some(dir.to_path_buf());
        }
    }

    None
}

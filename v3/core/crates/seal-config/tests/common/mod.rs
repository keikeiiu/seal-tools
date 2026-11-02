//! Shared test scaffolding for the integration tests.
//!
//! Std only on purpose: the `tempfile` crate pulls `windows-sys`, whose build script wants GNU
//! `dlltool`, which the pinned toolchain does not ship (see `v3/core/rust-toolchain.toml`).
//!
//! The scratch base is Cargo's `CARGO_TARGET_TMPDIR`, which lives inside `target/`. The system
//! temp directory is not usable here: writes from a test process are denied by the harness's
//! sandbox, while `target/` is inside the workspace.

#![allow(dead_code)] // each integration-test binary compiles this module and may use a subset

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

/// A unique directory under Cargo's target scratch area, removed on drop.
pub struct TestDir(PathBuf);

impl TestDir {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("seal-config-test-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn path_of(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub fn exists(&self, name: &str) -> bool {
        self.path_of(name).is_file()
    }

    /// Writes a file, creating parent directories a name may carry.
    pub fn write(&self, name: &str, text: &str) {
        let path = self.path_of(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    pub fn append(&self, name: &str, text: &str) {
        use std::io::Write as _;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path_of(name))
            .unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    pub fn read(&self, name: &str) -> String {
        fs::read_to_string(self.path_of(name)).unwrap()
    }

    pub fn read_bytes(&self, name: &str) -> Vec<u8> {
        fs::read(self.path_of(name)).unwrap()
    }

    /// Copies a committed file from the repository's `v2/config/` folder into this scratch folder —
    /// the real shipped template, the example seed and the dictionary are the fixtures, precisely so
    /// the tests exercise the real key set, comments and all.
    pub fn copy_repo(&self, from: &str, to: &str) {
        let source = repo_config_dir().join(from);
        let target = self.path_of(to);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::copy(source, target).unwrap();
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        clear_readonly(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The repository's committed `v2/config/` folder, found by walking up from this crate.
pub fn repo_config_dir() -> PathBuf {
    let mut dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join("v2").join("config");
        if candidate.join("defaults.yaml").is_file() {
            return candidate;
        }
        dir = dir.parent().unwrap_or_else(|| {
            panic!(
                "could not find v2/config/defaults.yaml above {}",
                env!("CARGO_MANIFEST_DIR")
            )
        });
    }
}

/// The bytes of a committed v2 config file.
pub fn repo_config_text(name: &str) -> String {
    fs::read_to_string(repo_config_dir().join(name)).unwrap()
}

/// Parses YAML in a test, so a fixture or a written file can be inspected as data.
pub fn parse_yaml(text: &str) -> Value {
    seal_config::yaml::parse(text).expect("test fixture should be valid YAML")
}

/// A minimal `local.yaml` for tests that do not care about the overlay: a comment-only file parses
/// as "nothing here", which the loader treats as an empty overlay.
pub const EMPTY_LOCAL: &str = "# nothing overridden in this test\n";

/// Recursively clears the Windows readonly attribute so the directory can be removed on drop.
///
/// The `permissions_set_readonly_false` lint is allowed rather than worked around: this suite is
/// Windows-only (`AGENTS.md`), `set_readonly(false)` there clears the `FILE_ATTRIBUTE_READONLY` flag
/// and does not choose a Unix mode at all.
#[allow(clippy::permissions_set_readonly_false)]
fn clear_readonly(path: &Path) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.is_dir()
        && let Ok(entries) = fs::read_dir(path)
    {
        for entry in entries.flatten() {
            clear_readonly(&entry.path());
        }
    }
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        let _ = fs::set_permissions(path, permissions);
    }
}

/// Marks a file read-only, the way a read-only medium would refuse an atomic replace. Used to pin
/// the fold's and the writer's failure paths.
pub fn set_readonly(path: &Path, readonly: bool) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_readonly(readonly);
    fs::set_permissions(path, permissions).unwrap();
}

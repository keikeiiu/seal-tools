//! Shared test scaffolding: a temporary directory that cleans itself up, and the root marker
//! writer.
//!
//! Std only on purpose: a temp-dir crate pulls `getrandom` → `windows-sys`, whose build script
//! wants GNU `dlltool`, which the pinned toolchain does not ship (see `rust-toolchain.toml`).
//!
//! The scratch base is Cargo's `CARGO_TARGET_TMPDIR`, which lives inside `target/`. The system
//! temp directory is not usable here: writes from a test process are denied by the harness's
//! sandbox, while `target/` is inside the workspace.

#![allow(dead_code)] // each integration-test binary compiles this module and may use a subset

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A unique directory under Cargo's target scratch area, removed on drop.
pub struct TestDir(PathBuf);

impl TestDir {
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("seal-core-test-{id}"));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Writes `config/<name>`, which is what marks a directory as the application root.
pub fn marker(dir: &Path, name: &str) {
    let path = dir.join("config").join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "").unwrap();
}

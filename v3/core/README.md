# Running cargo inside the workspace sandbox

The DSH file sandbox allows writes only under the repository, while cargo normally keeps its
crate cache in `%USERPROFILE%\.cargo`. Point `CARGO_HOME` at an in-repo scratch directory instead,
and everything cargo writes (cache, build output) stays inside the workspace — no approval needed.

`cargo.cmd` does that, then forwards every argument to cargo:

```powershell
# from v3/core
.\cargo.cmd test --workspace --offline
```

(A `.cmd`, not a `.ps1`: Windows PowerShell blocks unsigned script files by execution policy.)

## One-time setup

The scratch cache is populated once, by a cargo run that is allowed network access (the sandbox
blocks TLS from inside it):

```powershell
$env:CARGO_HOME = "<repo>\v3\core\.scratch\cargo-home"
cargo fetch --manifest-path <repo>\v3\core\Cargo.toml --workspace
```

After that, `--offline` works for every build. **Adding a dependency needs that fetch again** (and
therefore a full-access run); after the fetch the whole team can build offline.

## Quality gates (run before every commit)

```powershell
.\cargo.cmd test --workspace --offline
.\cargo.cmd fmt --all -- --check
.\cargo.cmd clippy --workspace --all-targets --offline -- -D warnings
```

All three must be clean. A warning is a failure: this core replaces a codebase whose build gate
was already warnings-as-errors, and `cargo clippy -- -D warnings` keeps that property.

## Deliberate divergences from v2

The port copies behaviour, not accidents. Every divergence gets an entry here, a test, and the
reason.

- **`foreign_blocks` no longer doubles an inter-block comment** (`seal-config/src/foreign.rs`).
  v2's forward walk runs to the next top-level key and its walk-back then starts at that key, so a
  comment sitting between two foreign blocks is emitted by both — and because foreign blocks are
  appended at the end of every save, the extra copy is re-extracted on the next save and doubles
  again. v3 carries each comment once, with the block it introduces. This is a v2 writer bug, not a
  measured behaviour any tool depends on.
- **An unmodelled key inside an owned block is KEPT, not dropped** (`seal-config/src/lib.rs`,
  the `LOCAL_HEADER` sentence). v2 re-serialised from a typed projection, so a key it did not model
  inside an owned block was silently dropped — the failure class `CONFIG.md` documents. v3's config
  is opaque JSON and the writer merges the block it was given, so there is no projection to drop it;
  the header now says KEPT rather than DROPPED. The other half of that warning is still true:
  comments inside the file are lost on every save.

## Toolchain

`rust-toolchain.toml` pins `stable-x86_64-pc-windows-gnu` because this machine has no MSVC linker
yet. Crates that generate import libraries with GNU `dlltool` (anything pulling `windows-sys`) do
not build under that pin; the Win32, serial and OCR crates are blocked until VS Build Tools is
installed and the pin flips back to `stable` (MSVC). Pure crates — protocol, config, domain,
tools' decision halves — are unaffected.


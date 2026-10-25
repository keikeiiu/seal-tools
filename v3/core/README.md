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
- **Deleting the active Quest flow selects the alphabetically first flow** (`authoring::quest`).
  v2 picked its insertion-ordered dictionary's first key; the v3 model uses a `BTreeMap`, so the
  new selection is deterministic-sorted instead. It is a UI selection, not stored calibration, and
  nothing is persisted until the player saves — so no run or coordinate depends on it.
- **A `text_fixes.regex` entry beyond the committed shape is refused at load, by name**
  (`ocr::text_cleaner`, `TextFixError::UnsupportedRegex`). v2 fed those entries to .NET's regex
  engine; v3 has no regex engine (adding one is a network dependency), so it implements exactly
  the committed dictionary's shape — literal text plus `(?!literal)` negative lookaheads — and
  refuses anything else loudly rather than mis-cleaning a reading. Cost if wrong: a hand-written
  dictionary using other regex constructs fails loudly in v3 instead of being applied. This is the
  one place a full-engine dependency (`fancy-regex`) would buy something, and the refusals are
  pinned as tests so swapping one in is a deliberate change.
- **Held inputs are released LIFO, and two new player-facing lines exist for the left button**
  (`seal-tools/src/held.rs`). v2's service-level `ReleaseHeld` always wrote `U` then `l` because it
  did not know what the tool had pressed; the guard knows, so it releases in the reverse of the
  order it pressed — the deliberate divergence is pinned by a test. v2 had no player-facing line at
  all for a failed left-button press or release (the pet only logged); v3 defines
  "Could not hold the left mouse button: …" and "Could not release the left mouse button — click
  once in game (…)" by analogy with the spacebar's existing texts. Cost if wrong: a failed drag
  button now says something to the player where v2 said nothing — a worse outcome only if players
  preferred the silence.
- **A second `tool.start` is refused, not auto-stopped** (`seal-tools/src/arbiter.rs`). v2 stopped
  the running tool when another was started; v3 answers `tool_busy` with the holder's line, and a
  distinct "is still stopping" line when a start arrives as the last run leaves. Cost if wrong: the
  player needs one extra click where v2 did it in one.
- **The placement's unreadable-cursor line is defined, not v2's interpolation**
  (`seal-board/src/placement.rs`). v2 interpolated `(,)` when the final cursor read failed; v3 says
  the cursor position could not be read, so a failed placement is diagnosable from the card alone.
- **A transport write failure surfaces as a typed line, not an escaped exception**
  (`seal-board/src/transport.rs`). v2 let the exception escape to the caller's message; v3 answers
  "the board would not take the move — <line>", which the tool loop can put on the card.
- **The runner defines two lines v2 never had** (`seal-tools/src/runner.rs`): `Stopped.` for a
  cancelled `tool.finished` (v2's "stopped" was UI text) and `A board or capture action failed.` for
  an effect failure that carries no better sentence (v2 showed `Exception.Message`, which on an
  empty message was nothing). Cost if wrong: a sentence where a developer saw a stack frame — the
  safe direction for a player facing a card.

## Toolchain

`rust-toolchain.toml` pins `stable-x86_64-pc-windows-gnu` because this machine has no MSVC linker
yet. Crates that generate import libraries with GNU `dlltool` (anything pulling `windows-sys`) do
not build under that pin; the Win32 and OCR crates, and `seal-board`'s OS serial adapter (the crate
itself is pure and builds), are blocked until VS Build Tools is installed and the pin flips back to
`stable` (MSVC). Pure crates — protocol, config, domain,
tools' decision halves — are unaffected.


# v3 CONFIG — the config layer

This is how the config layer works **now**, in the v3 core. It replaces v2's
[CONFIG.md](../../../v2/docs/spec/CONFIG.md) as the description of the layer — the file split, the
three-layer load, the once-only fold, and the writer. What a key *means* in the game has not changed;
the typed model that names every field is `seal-domain`'s
[config module](../../core/crates/seal-domain/src/config/mod.rs), and the field-by-field reasoning is
still v2's CONFIG.md.

The protocol carries the config as **opaque JSON** ([PROTOCOL.md](PROTOCOL.md) §7): the shell mirrors
the blocks it edits, never the whole file, and the core is the only writer and the only merger. The
YAML crate (`serde_norway`) is confined to `seal-config/src/yaml.rs`, so nothing else depends on it.

The port's oracle is `v2/SealTools.Tests/ConfigLoaderTests.cs`; the v3 suite is
`v3/core/crates/seal-config/tests/{load,fold,write,foreign}.rs`.

## 1. The files

Everything lives in `<root>/config/`; the root is the directory that holds `config/defaults.yaml` or
`config/defaults.yaml.example` ([PROTOCOL.md](PROTOCOL.md) §1). The folder layout and what an update
may overwrite are [PLAN-DEPLOY.md](../../../v2/docs/plans/PLAN-DEPLOY.md) §4's subject; this document
is the behaviour of the files, not the packaging.

| File | Class | Written by | Role |
|---|---|---|---|
| `defaults.yaml.example` | program | nobody | The shipped template. When it exists it is the **base** the load starts from. Every top-level section it carries is "portable" (two names excepted — see below), which is what makes it a template. |
| `defaults.yaml` | user state, legacy | nobody (v3) | On an install that has taken an update: the player's old defaults, read **once** until the fold, then never again. On a checkout (no `.example`) it is the base instead. It is the rollback copy; the app never writes, moves or deletes it. |
| `local.yaml` | user state | the core | The machine's own state: the measurements, the ten owned machine blocks (`calibration`, `tuner`, `gem`, `arduino`, `ui`, `spammer`, `buy_sell`, `pet`, `tooltip`, `quest`) and the `settings:` block — the portable half, which is the fold's home. This is the only file a save rewrites, and it is gitignored and excluded from the public zip. |
| `local.yaml.example` | program | nobody | The seed for a first run. Copied to `local.yaml` **only when that file is absent**, through the same atomic writer as a save — an interrupted copy would leave a truncated file that loads as an empty config. |
| `attributes.yaml` | program | nobody | The TW OCR dictionary; release-maintained, hand-editable by contract. A variant's dictionary is `attributes.<name>.yaml` (user state, grown in place) and is seeded verbatim from `attributes.<name>.yaml.example` when missing. A missing dictionary with no template is **loud**: an empty one matches nothing, and every attribute would read as "no attribute", silently — indistinguishable from the game having changed. |

The complete owned-block list is [`OWNED_LOCAL_BLOCKS`](../../core/crates/seal-config/src/lib.rs) —
eleven names, ported verbatim from `ConfigLoader.OwnedLocalBlocks`. Every other top-level block in
`local.yaml` is **foreign**: carried through a save verbatim, comments included.

The template defines the portable *shape*: a save projects the merged config onto it. Two names are
never projected even if a template carried them — `spammer` (personal rotations must never ship) and
`settings` (where the projection itself lives).

## 2. Loading: three layers and the fold

`ConfigStore::load` returns the merged config as JSON. Sources, weakest first
(`ConfigLoader.Load`'s order, kept deliberately):

1. **The base.** `defaults.yaml.example` when it exists (an install that has taken an update),
   otherwise `defaults.yaml` (a checkout). Neither → `MissingFile`.
2. **The seed.** A missing `local.yaml` is seeded from `local.yaml.example`, atomically, while the
   store holds the save gate. Neither file → `MissingFile`.
3. **The legacy layer.** `defaults.yaml` beside the template is read only when the base is the
   template, the fold marker is unset, and the file exists. Both files are normalised in memory
   first (a pre-presets flat `spammer.keys` becomes a `"default"` preset and the old key is never
   written again), so nothing later meets two shapes of one setting.
4. **The fold**, once. Compare every leaf the legacy file actually states against the template; a
   portable section with any differing stated leaf is carried **whole** — the leaves the legacy file
   states, projected onto the template's shape — into `local.yaml`'s `settings:` block.
   - Reading is per leaf; writing is per section. The apply side assigns a section, so
     `hotkeys.start` alone would drag the default `0` for the other three keys in with it. A leaf
     the legacy file does not state folds nothing, so a class default can never enter `local.yaml`
     wearing the player's name.
   - The marker `settings.legacy_folded: true` is written **unconditionally in the same write**, even
     when nothing differs. Without it, the next release's launch would fold the old template against
     that release's example and pin every default it changed as the player's own.
   - The fold is one gated, atomic read-modify-write. A write that cannot happen returns
     `Ok(false)` rather than throwing; `load` then applies the legacy file as a **values layer** for
     the session (the pre-fold behaviour, kept only as that fallback) and the fold retries next
     launch. A `local.yaml` that cannot be *parsed* is a different failure: it is reported and never
     overwritten.
   - The legacy file is never written, moved, deleted or renamed — it is the rollback copy. After a
     successful fold, `local.yaml` is re-read, because the copy read earlier is stale.
5. **The `settings:` block** — the portable half — is overlaid onto the base.
6. **The machine blocks** are overlaid last, in the owned-block order, so the machine keeps the last
   word wherever the two name the same field.
7. **The variant resolves the title.** A non-empty `game.variant` naming a known variant (the table
   is case-insensitive) with a non-empty `title` sets `window.title`. An unset or unknown variant
   changes nothing: a file written before variants existed carries only `window.title`, and an
   unrecognised name must not blank the target.

The merge is a **recursive field-wise overlay**: objects merge, scalars and lists replace, and `null`
means "not stated" — YAML's spelling of v2's unset optionals, so it leaves the lower value standing.
That keeps v2's documented rule ("a guard is *is it present*, never *is it non-empty*") without a
typed field list: a value the block states, including `0` or `""`, is a choice.

The fold's approximation is stated rather than hidden, and is the safe direction: "the player changed
it" is read as "it differs from the shipped template", so a default a *release* changed under a key
the player never touched is preserved as if they had chosen it. [PLAN-DEPLOY.md](../../../v2/docs/plans/PLAN-DEPLOY.md)
§4 works through the cases; §6 lists the file operations (seed, fold, adoption, and nothing else)
that a first launch may perform.

**Spammer adoption.** Presets used to be saved into the shipped file. In the same `load`, after the
fold, a legacy flat `spammer.keys` map and any presets left in the old file (or in the checkout's
tracked `defaults.yaml` when there is no legacy layer) are adopted once into `local.yaml`'s
`spammer:` block: the guard is the block's presence, and it is re-checked against the file inside the
write (another process may have adopted in the gap). A hand-written legacy `startup_step` folds into
the object shape first, so the adopted copy carries one representation. The source file is not
touched. Unlike the fold, an adoption write that fails is an error — only the fold is non-throwing.

A fresh install seeds `local.yaml`, finds no legacy file, and does no fold; a checkout reads its
tracked `defaults.yaml` as the base and does no fold either. Missing files and unparsable files are
loud; the fold's write failure is the one stated exception.

## 3. The writer

**One writer.** All file writes go through `ConfigStore`. A process-wide `Mutex` (the port of v2's
static `SaveGate`) spans the **whole read-modify-write**, not just the file publish: a writer says
*what to change*, never what the whole file should contain, so two writers from two threads cannot
delete each other's block. Writers from two **processes** are not serialised here — v2's gate is
in-process too, and [PLAN-DEPLOY.md](../../../v2/docs/plans/PLAN-DEPLOY.md) §6.7 names the rule (do
not run two cores on one config folder). The protocol's single-instance lock is a core concern and is
not implemented yet.

**One write**, in order ([`update_local`](../../core/crates/seal-config/src/store.rs)):

1. Read `local.yaml` and **parse it first** — an unreadable file fails the save instead of being
   replaced by whatever the caller had in mind.
2. Extract the foreign top-level blocks as raw text ([`foreign_blocks`](../../core/crates/seal-config/src/foreign.rs)):
   a top-level key is column zero, not a comment, a bare `name:`; the `#` lines immediately above a
   block travel with it, and a blank line stops that walk-back so the file's own header stays out.
3. Keep only the owned top-level keys in the parsed object. This is load-bearing: foreign blocks
   travel as text, and leaving them in the object would serialise them into the body **and** append
   them again, duplicating on every save.
4. Mutate, serialise, prepend the constant header, append the foreign text.
5. Replace the destination atomically: a unique temp name beside it (`<file>.<pid><nanos><seq>.tmp`)
   then a rename. A failure removes the temp and leaves the old file whole; the destination is never
   truncated.

The header every save emits is [`LOCAL_HEADER`](../../core/crates/seal-config/src/lib.rs): the only
prose that survives a save, because the writer serialises values and cannot carry comments. It says
the two things that have cost data — comments are lost, foreign top-level blocks survive — and names
this document. It is v2's text with **one sentence reworded** (see "Deliberate divergences" below).

**What a save does to each part of the file:**

- *Owned blocks* are serialised from the object. Keys are emitted in alphabetical order (the
  in-memory map is ordered) and line endings are LF; both are cosmetic, not a contract.
- *Keys inside an owned block that nothing reads* are **kept**: the config is opaque JSON and a save
  merges the values it was given, so there is no typed projection to drop them.
- *Comments inside owned blocks are lost* on every save. That is unchanged from v2 and is why the
  header says to keep a copy.
- *Foreign top-level blocks* are appended verbatim — comments, quoting and list style intact — after
  the serialised body.
- *`defaults.yaml` and `attributes.yaml` are never written by a save.* Nothing writes the shipped
  template.

**Where a save lands.** `config.apply` from the shell carries only the blocks a tab owns and merges
them **field-wise** into the current top-level block, so a session half a core-side writer put there
a moment ago survives (`ConfigStore::apply_sections`; §4.1 of
[PLAN-V3-CORE.md](../../../v2/docs/plans/PLAN-V3-CORE.md)). The `settings:` block is the portable
layer: it is written by the fold and by `save_settings`, which projects the merged config onto the
template's shape — whole sections, spammer excluded, machine-only fields impossible to leak — while
mutating the block rather than replacing it, so the marker and unknown settings survive.

**Deliberate divergences from v2.** Every one is recorded in
[v3/core/README.md](../../core/README.md) with a test and a reason. This layer owns two:

- `foreign_blocks` carries an inter-block comment **once**, with the block it introduces. v2's
  forward walk and walk-back both claimed it, so every save appended a duplicate that compounded on
  the next save. This is a v2 writer bug, not a measured behaviour.
- An unmodelled key inside an owned block is **kept**, not dropped, and the header says so. v2
  rebuilt owned blocks from a typed projection; v3 merges opaque JSON, so nothing can disappear
  inside an owned block. The header's other half — comments are always lost — is still true.

A third divergence is an authoring rule rather than a file rule, but its *stored value* is config:
deleting the active Quest flow reselects the alphabetically first flow (`quest.active`), because the
v3 model's maps are ordered maps rather than insertion-ordered dictionaries. It is a picker selection,
persisted only if the player saves.

## 4. The API, and the revision the core adds

[`ConfigStore`](../../core/crates/seal-config/src/store.rs) is one `config/` directory and holds no
config in memory: every read goes to disk, every write re-reads under the gate.

| Method | What it does |
|---|---|
| `new(config_dir)` | Binds to a folder. Reads and creates nothing. |
| `load()` | The three-layer merge of §2, as opaque `serde_json::Value`. |
| `load_local()` | Just `local.yaml` (an empty document parses as `null`). |
| `load_attributes()` / `load_attributes_named(name)` | The OCR dictionary, seeding a missing one from `<name>.example` first. |
| `update_local(f)` | Gated read-modify-write with the header, foreign text and atomic replace of §3. The callback sees the file as it is now. |
| `apply_sections(&map)` | Merges owned top-level blocks field-wise into `local.yaml` and returns the merged blocks. Non-owned names are refused rather than written — rewriting a block that is carried as raw text would duplicate it. |
| `save_settings(&config)` | The portable projection into `settings:` (§3). |

Failures are a [`ConfigError`](../../core/crates/seal-config/src/error.rs): `MissingFile`, `Read`,
`Parse`, `Serialize`, `Write` (each names the file) and `SectionNotOwned`. The core maps the file
failures to the protocol's `io` error with `detail.path`, and ownership/shape failures to
`bad_request`.

**The revision on top is a session counter, not stored state.** `seal-core`'s dispatcher
([dispatch.rs](../../core/crates/seal-core/src/dispatch.rs)) starts `rev` at `0`, returns it from
`hello`, and bumps it on every successful write — `config.apply`, `ui.window` — and on
`config.reload`, which is a change the shell must see even though no file was written. A refused
call never consumes a revision. `config.apply` with a stale `rev` is refused as `conflict`, carrying
the current `rev` and the current value of each block the shell asked to write, so it can reconcile.
Startup migrations (seed, fold, adoption) happen before the session begins and do not consume a
revision. The command shapes are [PROTOCOL.md](PROTOCOL.md) §4's; the merge semantics are its §7.

**What is not in this crate.** Authoring/validation rules (they live in
`seal-domain/src/authoring/`), the typed model and its hand-edited-YAML repairs
(`seal-domain/src/config/`), the protocol itself (`seal-core`), and every game/tool rule. `seal-config`
is deliberately a file layer over opaque JSON: it decides *where a value lives and whether a write is
safe*, not *whether a value is allowed*.

## 5. Still open in v3

- **Authoring rules are not wired yet.** `config.validate` checks the shape (owned block names,
  mappings) and returns empty `errors`/`warnings`; the rules exist as pure functions in
  `seal-domain`'s [authoring module](../../core/crates/seal-domain/src/authoring/mod.rs) and are the
  plug-in point. Until that call is added, a save is rule-checked only by the shell.
- **The typed attributes model has landed, but nothing consumes it yet.** The loader returns opaque
  JSON and `seal-domain` now owns the typed dictionary plus its OCR matchers
  ([attributes.rs](../../core/crates/seal-domain/src/config/attributes.rs),
  [ocr/](../../core/crates/seal-domain/src/ocr/mod.rs)); no command consumes a dictionary yet, and
  the variant-dictionary switch (load-by-name + seeding) is not wired into the dispatcher.
- **The `settings:` path has one caller left to gain.** The fold writes the block and `save_settings`
  projects into it, but the current dispatcher's `config.apply` writes owned blocks at the top level;
  a future portable-save command is the natural place for `save_settings`. Both locations are inside
  `local.yaml`, which no update overwrites, so this is an open integration choice rather than a
  safety gap.
- **No cross-process lock.** The save gate is in-process, and the protocol's single-instance lock
  ([PROTOCOL.md](PROTOCOL.md) §1) is not implemented yet. Two cores on one root can interleave.
- **Binary artifacts need their own writer.** The atomic writer carries text only. The calibration
  PNGs that v2 wrote beside `local.yaml` need a temp-and-move sibling for binary data (a
  `File.Copy`-to-temp-and-move shape, not this one) when `calibration.save` lands.
- **The header links a document v3 has not ported yet.** `LOCAL_HEADER` points at
  `docs/spec/CALIBRATION.md` (and this file); the v3 spec tree does not have CALIBRATION.md yet, so
  that inherited pointer resolves only after the docs port.

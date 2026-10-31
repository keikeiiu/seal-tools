# v3 CONFIG — the config layer

This is how the config layer works **now**. It describes two live modes:

- the **portable layout** (`config/setup/`, `config/machine/`, `config/artifacts/`, `config/meta.yaml`)
  — the layout a migrated/shipped install uses;
- the **legacy single file** (`config/local.yaml`) — the path a development checkout and every
  pre-migration tree still use.

Both serve the same opaque JSON on the wire ([PROTOCOL.md](PROTOCOL.md) §7). What a key *means* in
the game has not changed; the typed model that names every field is `seal-domain`'s
[config module](../../core/crates/seal-domain/src/config/mod.rs), and the field-by-field reasoning is
still v2's CONFIG.md.

The YAML crate (`serde_norway`) is confined to `seal-config/src/yaml.rs`. The port's oracle is
`v2/SealTools.Tests/ConfigLoaderTests.cs`; the v3 suite is
`v3/core/crates/seal-config/tests/{load,fold,write,foreign,layout,layout_write,layout_transaction,migration,acceptance,new_layout_store}.rs`.

## 1. The rule a file follows

> **A config file holds one class × one domain. The class says whether it travels; the domain says
> which tool owns it.**

The registry in [`seal-config::registry`](../../core/crates/seal-config/src/registry.rs) is the one
table: path, class, domain, `schema_version`, and the config paths the file owns. A test walks the
typed overlay and fails if a key has no owner.

| class | folder / file | travels? |
|---|---|---|
| shipped defaults | `config/defaults.yaml.example` | shipped with the release; the base every portable key falls back to |
| release data | `config/attributes*.yaml` | shipped; OCR dictionaries |
| **setup** | `config/setup/{common,tuner,gem,pet,spammer,quest,buy_sell}.yaml` | **YES** — copy this folder to another PC |
| **machine** | `config/machine/{display,tuner,gem,buy_sell,pet,tooltip,ui,arduino}.yaml` | NO — measured on this PC, safe to regenerate by calibrating |
| **artifacts** | `config/artifacts/*.png` | NO — the reference crops and learned icons |
| user notes | `config/user.yaml` | YES if you want your notes to travel; the core never reads or writes it |
| bookkeeping | `config/meta.yaml` | NO — layout/migration markers, not hand-edited |

The exact owned paths are in the registry; the player-facing copy/backup guide is
[`config/README.md`](../../config/README.md).

## 2. Loading

`ConfigStore::load` returns the merged config as JSON. It chooses the mode by `config/meta.yaml`:

1. **Portable mode** (a migrated install; `meta.yaml` carries `layout_version`): the load is
   [`layout::load`](../../core/crates/seal-config/src/layout.rs) — the shipped base, then
   `setup/`, then `machine/`, in the registry's explicit order. Missing files contribute nothing.
2. **Legacy mode** (a development checkout, or a tree never migrated): the three-layer load of
   `defaults.yaml.example`/`defaults.yaml`, the once-only fold, spammer adoption and `local.yaml`
   (§6 below).

In both modes the merge is a **recursive field-wise overlay**: objects merge, scalars and lists
replace, and `null` means "not stated" — YAML's spelling of v2's unset optionals. One owner per path
means the only precedence that matters is "the base vs a stated override". The active
`game.variant` resolves `window.title` last, so no call site has to know variants exist.

### 2.1 The reserved key and unknown keys

Every portable/machine data file carries `schema_version: 1`. The loader strips it before merging,
the writer writes it back on every save, and a newer version is refused **loudly** with
`ConfigError::SchemaTooNew` — the core never guesses at a file a newer build wrote. Unknown keys
inside a file are **kept** (a save writes the whole mapping, not a fixed field list). Comments are
still lost on save; prose belongs in `config/README.md` or `config/user.yaml`.

### 2.2 The protocol view

`hello` still answers ONE merged config. The protocol does not expose the file layout: the shell
reads and writes the same blocks it always did, and the core projects the legacy `settings:`
overlay onto the setup files when the shell saves. `config.apply` carries blocks, `config.changed`
is block-level.

## 3. Writing

Every save is a read-modify-write under the process-wide save gate; the caller says *what changed*,
never what a file should contain.

- **Routing.** [`layout::write_sections`](../../core/crates/seal-config/src/layout.rs) sends each
  changed FIELD to the file that owns it. A whole block routes to its one owner when the block is
  not split (`calibration`, `spammer`, `quest`, `tooltip`); split blocks route field by field
  (`buy_sell.presets` setup, `buy_sell.bag_grid` machine; `tuner.target_grade` setup,
  `tuner.ocr` machine; and so on). An unowned path is refused **before any file is written**.
- **Atomicity.** One file is one temp+fsync+rename write. The Setup and Pet card Saves span two
  files; those go through [`transaction`](../../core/crates/seal-config/src/transaction.rs)'s
  write-ahead journal under `config/.seal-txn/`. A crash between the two renames rolls BOTH files
  back on the next read/write; recovery is idempotent.
- **Field-level diffs.** The live `ConfigStore` presents the portable tree as the old overlay shape
  and routes only the fields that actually changed, so saving a preset never rewrites the
  calibration file that happens to live in the same block.
- **Artifacts.** The four inline crop groups (`pet.close_button_png`, `pet.food_icon_png`,
  `pet.slots[].pet_slot_empty_png`, `pet.queue[].png`) are files under `config/artifacts/`, with an
  `artifacts/<name>.png` reference in `machine/pet.yaml`. The pet plan resolves references back to
  base64 before any decoder sees them, so the run and the wire are unchanged; legacy inline values
  still work.
- **`user.yaml`.** Top-level blocks the app does not own move there during migration and are carried
  as data; the core never writes the file.

## 4. Migration from the legacy file

The one-time migration is [`migrate`](../../core/crates/seal-config/src/migrate.rs). It runs on the
first load of a **shipped install** (both `defaults.yaml.example` and `local.yaml.example` present)
that still has a `local.yaml`:

1. back up the whole `config/` folder to `config/.migration-backup-<stamp>/`;
2. route the owned blocks through the writer (machine → `machine/`, portable `settings:` → `setup/`,
   `calibration` → `machine/display.yaml`, foreign blocks → `user.yaml`);
3. extract the inline crops into `config/artifacts/` and leave references;
4. load the new tree successfully **before** retiring the old file;
5. delete `local.yaml` (it is in the backup), then write `meta.yaml`'s markers LAST.

A `meta.yaml` with `layout_version` makes a later run a no-op; a crash after retirement but before
the marker is recognised and only tagged. Rollback is the backup folder; downgrading an old core is
not supported. A development checkout (no `defaults.yaml.example`) is never auto-migrated.

## 5. The API

[`ConfigStore`](../../core/crates/seal-config/src/store.rs) is one `config/` directory; both modes
behind the same methods.

| Method | What it does |
|---|---|
| `new(config_dir)` | Binds to a folder. Reads and creates nothing. |
| `load()` | The merged config, in the mode `meta.yaml` selects. |
| `load_local()` | The overlay view: legacy `local.yaml`, or the portable tree seen as `settings:` + machine blocks. |
| `update_local(f)` | Gated read-modify-write; in portable mode the closure sees the overlay view and only changed fields are routed to their files. |
| `persist_pet_run_state(patch)` | The field-precise pet run-state write, same routing. |
| `apply_sections` / `save_settings` | Legacy-mode composed writers (kept for the dev path). |
| `load_attributes()` / `load_attributes_named(name)` | The OCR dictionary, seeding a missing one from `<name>.example` first. |
| `write_artifact(name, bytes)` | Atomic PNG write beside the config (calibration screenshots). |

Failures are a [`ConfigError`](../../core/crates/seal-config/src/error.rs): `MissingFile`, `Read`,
`Parse`, `Serialize`, `Write` (each names the file), `SectionNotOwned` and `SchemaTooNew`. The core
maps file failures to the protocol's `io` error with `detail.path`.

**The revision on top is a session counter, not stored state.** `seal-core`'s dispatcher starts
`rev` at `0`, returns it from `hello`, and bumps it on every successful write; a refused call
consumes none. `config.apply` with a stale `rev` is refused as `conflict`, carrying the current
`rev` and the current value of each requested block. `calibration.save` supports the same
compare-and-set when the caller sends `rev`. Migration/first-load work happens before the session
and consumes no revision.

## 6. Legacy mode (development checkout and pre-migration trees)

The old single-file semantics remain for the dev tree and for a v3 tree before its first shipped
launch; [`load`](../../core/crates/seal-config/src/store.rs) implements them exactly as before:

- the base is `defaults.yaml.example` when present, else the tracked `defaults.yaml`;
- a missing `local.yaml` is seeded atomically from `local.yaml.example`;
- `defaults.yaml` beside the template is the once-only legacy layer: differing stated leaves are
  folded into `local.yaml`'s `settings:` block with the `settings.legacy_folded: true` marker, and
  the old file stays as the rollback copy;
- spammer presets left in the old shipped file are adopted once;
- top-level blocks the app does not own are foreign: carried through a save verbatim, comments
  included, and they move to `user.yaml` when the tree is migrated;
- the save is one atomic rewrite with `LOCAL_HEADER`, and inside an owned block an unmodelled key is
  kept (v2 dropped those; that is a deliberate divergence, tested and recorded in
  [v3/core/README.md](../../core/README.md)).

## 7. Still open

- **No cross-process lock.** The save gate is in-process; two cores on one root can interleave. The
  protocol's single-instance lock is [PROTOCOL.md](PROTOCOL.md) §1's subject.
- **Annotated calibration screenshots still live beside the config** (`config/calib_*.png`) rather
  than under `artifacts/`; only the four inline crop groups were extracted (increment 6). Their
  readers accept both.
- **Run-learned crops still write inline** for the close button/food/queue images; the calibration
  empty-slot crop already writes an `artifacts/` reference.
- **A top-level block removal** through the portable writer is refused rather than expressed as a
  file deletion; no current command needs it.
- **Comments inside data files are lost on save**, as in v2. A comment-preserving editor is a
  separate future decision.

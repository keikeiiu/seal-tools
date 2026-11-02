# Analysis — config storage: would PostgreSQL / other SQL help?

**Question (player, 2026-10-30):** would moving the tools' config from YAML to PostgreSQL or
another SQL store improve **performance**, **code quality**, and "etc."? Constraint: the on-screen
calibration must stay YAML / config files.

**Scope:** v3 (branch `v3-core`) — the Rust core's [`seal-config`](../../core/crates/seal-config/) and
the Flutter shell that edits config through the protocol. v2's equivalent is
[v2 CONFIG.md](../../../v2/docs/spec/CONFIG.md); its conclusions apply there too, but v2 is the line
v3 is replacing and is not evaluated separately.

**Status: study only. Nothing was changed** — no code, no config, no schema. This document exists so
the decision is not re-argued from memory.

---

## Verdict

| criterion | finding |
|---|---|
| **Performance** | **No gain.** Config is read at boot and on saves, never in a tool loop: a 122 KB machine state loads in ~2.4 ms and a save costs ~4 ms, beside an OCR pipeline load already accepted at ~330 ms ([dispatch.rs:89-91](../../core/crates/seal-core/src/dispatch.rs#L89-L91)). SQLite opens in ~0.9 ms; any SQL engine is noise at this scale. |
| **Code quality** | **No clear gain, likely net loss under the constraint.** ~60% of `seal-config` exists because the config is a *hand-edited text file* (comments cannot survive, foreign blocks are carried verbatim, one-time fold, atomic rename). SQL replaces that with schema + migrations + JSON↔row marshalling; it does not shrink the work. The "calibration stays YAML" constraint splits blocks that today save atomically. |
| **Deployment** | **PostgreSQL is disqualified**, not on speed: the product is a zip with no installer, no admin rights, no service, no network feature, and a release allow-list of exact files. SQLite is the only SQL engine that fits; it buys real transactions/locking/durability, but those are cheaper to add to the current writer. |
| **Recommendation** | **Decline the SQL migration for now; keep YAML and harden the file layer** (§7). Revisit only on a named trigger (§8), the plausible one being multi-machine/account sync — and that is an export/sync feature, not a config database. |

---

## 1. What the config actually is

The layer is described in [CONFIG.md](../spec/CONFIG.md); the numbers below are from the real
machine state a v3 install is built to read, `v2/config/local.yaml` (**122,947 B, 1,066 lines**, 14
top-level blocks).

| file | size | role |
|---|---|---|
| `config/defaults.yaml.example` | 4,846 B | shipped template; the base of the three-layer load |
| `config/local.yaml` | up to 123 KB in a real machine | machine state + personal records + the portable overlay |
| `config/local.yaml.example` | 3,210 B | first-run seed |
| `config/attributes.yaml` | 6,030 B | OCR dictionary (not tool config) |

**Eleven owned blocks** (`calibration`, `tuner`, `gem`, `arduino`, `ui`, `spammer`, `buy_sell`,
`pet`, `tooltip`, `quest`, `settings`); every other top-level block is foreign text carried through a
save verbatim. Three classes are mixed inside those blocks:

- **portable-shipped** — defaults that ride the template (hotkeys, tuner/gem behaviour, game
  variant); editing the template in git is how a new default reaches every user.
- **machine/calibration** — measured geometry, crops, display environment, port.
- **personal records** — spammer presets, quest flows/sequences, buy/sell presets.

The record-shaped part is small. From the real state: **quest 12 flows / 9 sequences / 126 steps;
spammer 4 presets; buy/sell 6 presets; pet 4 rows / 8 queued / 21 food slots** — about **40
identity-bearing entities, with exactly one real foreign key** (a sequence entry names a flow).
That FK is already enforced by the core's authoring rules on save.

**84% of `local.yaml`'s bytes are not config at all:** 102,901 B in 14 base64 PNG lines (the pet
crops — close button, food icon, slot references). A row store would carry the same blobs, or force
a separate artifact channel — which the core already has for calibration PNGs
([`write_artifact`](../../core/crates/seal-config/src/store.rs#L398), used for
`calib_gem_result.png`).

**The calibration boundary cuts through blocks, not between them.** `pet` mixes calibrated geometry,
learned crops, queue/food records and live run state; `buy_sell` mixes geometry with presets;
`tuner`/`gem` mix machine geometry with portable settings that live in a different file. So "tool
config in SQL, calibration in YAML" would need a **field-level** split of blocks that today save in
one atomic read-modify-write
([`apply_sections_with_settings`](../../core/crates/seal-config/src/store.rs#L474)).

## 2. Where config I/O happens

`ConfigStore::load` is called at boot ([dispatch.rs:285](../../core/crates/seal-core/src/dispatch.rs#L285)),
on `config.reload`, after each successful write ([dispatch.rs:659](../../core/crates/seal-core/src/dispatch.rs#L659),
[refresh_config](../../core/crates/seal-core/src/dispatch.rs#L1758)), and twice per Pet run-state write
([pet_state.rs:69-92](../../core/crates/seal-core/src/pet_state.rs#L69-L92)). Writes are user-triggered
saves (`config.apply`, `ui.window`, `calibration.save`, tooltip save) plus the Pet feeder's mid-run
run-state write.

**No tool loop reads config.** A `tool.start` session takes a clone of the in-memory JSON snapshot
([registry.rs:120-125](../../core/crates/seal-core/src/registry.rs#L120-L125)) and resolves its plan
from that clone; the capture/OCR/click loop never touches disk. The only mid-run I/O is the Pet
run-state write (~9 ms, a few times per boarding event, on the feeder's thread). The dispatcher clones
the merged config per command — 0.155 ms, memory only.

## 3. Measurements

Measured on this machine with the core's actual YAML path (`serde_norway` 0.9.42 + `serde_json`,
release build, throwaway harness outside the repo) and SQLite 3.49.1; median of 200 where noted.

| operation | size | median |
|---|---|---|
| YAML parse + JSON convert (template) | 4.8 KB | 0.13 ms |
| YAML parse + JSON convert (real machine state) | 122.9 KB | 2.36 ms |
| full `ConfigStore::load` (122.9 KB + template, incl. foreign scan, read, merge) | — | ~2.4 ms (cold 14.4 ms) |
| `update_local` (read + parse + serialize + write + rename, no-op) | — | ~4 ms |
| file read alone | 122.9 KB | 0.05 ms |
| atomic save as written today (write + rename, **no fsync**) | 122.9 KB | 0.7–0.8 ms |
| `serde_json` parse of the same document | 116 KB | 0.12 ms |
| SQLite connect + pragma + first query | 160 KB DB | 0.9 ms |
| SQLite warm point read / upsert+commit (WAL) | — | 4.7 µs / 5 µs |
| loopback TCP round trip (the floor under any client/server DB) | — | 8 µs |

PostgreSQL was not benchmarked — no server is installed here, and the disqualification is structural
(§6), not a speed question. The numbers that matter: **a boot costs 2.4 ms, a save 4 ms, and the OCR
pipeline load alone is ~330 ms** — an engine swap changes nothing a player can feel.

`seal-config`'s suite — 78 tests in five binaries — passes (each binary 0.00–0.30 s):

```
running 7 tests    ok. 7 passed; 0 failed        (src/yaml.rs unit tests)
running 17 tests   ok. 17 passed; 0 failed       (tests/fold.rs)
running 12 tests   ok. 12 passed; 0 failed       (tests/foreign.rs)
running 18 tests   ok. 18 passed; 0 failed       (tests/load.rs)
running 24 tests   ok. 24 passed; 0 failed       (tests/write.rs)
```

## 4. What SQL would fix, move, or break

| today's cost | SQL effect |
|---|---|
| **Three-layer load + once-only fold** (~230 lines; the fold is a heuristic with its own bug history, see [PLAN-DEPLOY.md](../../../v2/docs/plans/PLAN-DEPLOY.md) and [v2 PROGRESS](../../../v2/docs/records/PROGRESS.md)) | **Eliminated** — replaced by a one-time import migration. The clearest real win. |
| **Foreign-block preservation** (`foreign.rs`: a top-level key is column zero…, comments travel with their block) | Eliminated for rows; the *import*/export path still has to read YAML, and unknown-key policy must be re-decided. |
| **Comments are always lost on save** | Not fixed. The header warning and the "keep a copy" advice move, they do not disappear. |
| **Atomic write, but no fsync; no cross-process lock** (both documented: [CONFIG.md §3](../spec/CONFIG.md), [OPEN §2](../backlog/OPEN.md); lock specified in [PROTOCOL §1](../spec/PROTOCOL.md)) | **Fixed** (SQLite WAL/transactions, locking). Also fixable in the file writer for a fraction of the change. |
| **No schema; unknown keys kept by strategy; one FK enforced by hand-written rules** | Columns/FKs available — but the one FK is already validated, and an unknown-key overflow column would be needed anyway to keep v3's "nothing inside an owned block can disappear" guarantee. |
| **Hand-editable, diffable, copyable config; template edits in git are the release mechanism; v2→v3 migration is a file copy** ([CUTOVER decision 4](../plans/CUTOVER.md)) | **Regressed.** A binary store is not hand-editable or diffable; the template becomes a seed; migration becomes export/import; backup becomes `config.db` + `-wal` + `-shm`. |
| **The v2 data-loss class was write-side projection loss** (whole spammer block, hand-added `pet:` block, `gem.move_mode`, `ocr_retries`) | v3 removed that class by merging **opaque JSON**. A normalised schema is a new typed projection over the same player data — migration mistakes become the new loss vector; a JSON-blob-per-block design avoids that, but then the code-quality win evaporates. |
| **78 tests pin file semantics** (fold once, foreign text verbatim, key order, atomic replace) | Most are rewritten or replaced by schema/migration tests; the shell (~1,600 config lines across 7 tabs and the draft machinery) is unaffected if the protocol stays opaque JSON — so the effort is core-only and the UI wins nothing. |

## 5. Code-quality arithmetic

`seal-config/src` is **~1,003 non-comment code lines**; `store.rs` alone is 794 (79% of the crate), and
**~60% of the crate is "this is a text file humans edit"**: fold/projection, foreign-block text,
comment loss, atomic temp+rename, header. A SQL store of the same data removes that plumbing and adds:
schema DDL, migrations (refinery/sqlx), row↔JSON mapping for the opaque blocks the protocol and shell
are built around, import/export for migration and for the v2 rollback copy, and a bundled engine to
keep current. Net line count could fall only by normalising the data — the typed projection the v3
design deliberately does not rebuild.

The other half of the split matters: `seal-domain`'s typed config model (1,231 non-comment lines) and
the authoring rules stay under either design; the shell stays too. So the claimed code-quality gain is
confined to one crate and is roughly cancelled by what replaces it.

## 6. Deployment filter (the decisive one)

[INSTALL.md:26](../INSTALL.md) promises "no installer, no `.NET` runtime, no administrator elevation,
and every file the app writes lives inside its folder"; line 138 adds "no registry keys, no services,
… no update check or network feature". [`publish.ps1`](../../publish.ps1) stages an **exact
allow-list** (`config/defaults.yaml.example`, `config/local.yaml.example`,
`config/attributes.yaml`, `config/attributes.us.yaml.example`) and denies player state. So:

- **PostgreSQL** — a server install (or `initdb` + `pg_ctl`), a service, port 5432, credentials, a data
  directory, a backup story and hundreds of MB. It cannot be shipped in the zip and cannot pass the
  allow-list. [`sqlx` 0.9](https://crates.io/crates/sqlx) is async and now requires Rust 1.94, while
  this core is synchronous with no async runtime (verified: no tokio/async-std in any crate). It is
  disqualified as the store.
- **MySQL/MariaDB, SQL Server/LocalDB, any Docker/server option** — same disqualification.
- **SQLite** ([rusqlite 0.41](https://crates.io/crates/rusqlite) `bundled` — no DLL, no install,
  public domain; migrations via [refinery 0.10](https://lib.rs/crates/refinery)) — the only engine
  that fits. It buys ACID saves, WAL recovery and cross-process locking (the last closes a
  documented gap). It costs the hand-edit/diff/copy-migration features above and makes the bundled
  engine a version you must track: SQLite has had WAL corruption bugs, fixed in 3.51.3
  ([WAL docs](https://www.sqlite.org/wal.html), March 2026).
- **DuckDB** — single-writer *process*; cross-process writes only through the still-beta
  [Quack protocol](https://duckdb.org/docs/1.5/connect/concurrency) — wrong concurrency model,
  analytics footprint.
- **Firebird embedded** — DLL + plugins/intl tree side-by-side, thin Rust driver, licence notice; not
  justified for 40 records.
- **Embedded KV (redb/fjall/sled)** — no query layer, no migration ecosystem, opaque file; strictly
  worse than one YAML file for this data.
- **"Optional Postgres for power users"** — coherent only as an **export/sync target** the core
  writes to, never as a second store: a second store breaks "the core is the only writer" and creates
  a mode where only power users meet the bugs.

## 7. What to do instead (Path 1 — keep YAML, harden it)

These are small, independent, no protocol or shell change, and each closes a documented gap:

1. **Durability:** flush (`sync_all`/`FlushFileBuffers`) the temp file before the rename, and the
   artifact writer too. The current writer is
   [`fs::write` + `fs::rename` with no fsync](../../core/crates/seal-config/src/store.rs#L733) — a
   power loss can leave the old file or a rename whose data is not yet on disk. Not literally one
   line (the file needs a handle; Windows has no directory-fsync), but a small change.
2. **Cross-process safety:** build the lock [PROTOCOL §1](../spec/PROTOCOL.md) already specifies
   (`config/.seal-core.lock`, exclusive OS lock, second core exits 2). It is named in
   [OPEN §2](../backlog/OPEN.md); it is the same hole SQLite would close, without the rewrite.
3. **Schema evolution:** add a `config_version` key and one explicit migration step per break,
   instead of relying only on the implicit fold marker. This is the part a DB would do better; the
   migration is small once written.
4. **Shrink the file:** move the 14 inline base64 PNGs (84% of the 123 KB) to `config/artifacts/`
   through the existing `write_artifact`, with a load-time migration that accepts the old inline
   form. Leaves a ~20 KB, diffable `local.yaml` — the calibration artifacts already work this way.

Items 1–2 are the ones that would matter if the pain is "a save went wrong". Items 3–4 are quality
of life. None of this requires giving up hand-editing, git-diffable templates, or the file-copy
migration.

## 8. When SQL becomes the right answer

Revisit if one of these becomes a real requirement, not an imagined one:

- **Multi-machine / multi-account sync or sharing.** Then the feature is export/import or a sync
  endpoint — SQLite can be the local cache, but the store of record is still a design for sync, not
  for a config file. Postgres belongs to that design (client/server), not to the zip app.
- **Cross-record querying or full-text search over thousands of rows.** Today there are ~40
  entities and the shell already holds the whole config in memory; a DB indexes nothing anyone asked
  for.
- **Several concurrent writer processes.** Today one core owns the writer; a lock file is enough.
  If a background service ever writes config beside the shell, SQLite's locked writer starts to earn
  its cost.

## 9. Caveats

- Timings are one Windows machine, release build, temp-dir copies; Windows/AV noise gave a 4–11 ms
  spread on the save. The orders of magnitude are what matter.
- The "real" sizes come from `v2/config/local.yaml` (the migration source); the v3 checkout's own
  `config/local.yaml` is a 43-line dev seed, not a machine state.
- PostgreSQL was not measured; its rejection is deployment-shaped, not performance-shaped.
- No repo source or config file was modified by the measurements; the harness and its config copy
  lived in `%TEMP%` and were deleted.

## 10. Open questions for the player (⚑)

1. **Which failure is this about** — a lost/corrupt config write, or a capability you don't have
   (sharing config across machines/accounts, querying it, multiple profiles)? If it is the first,
   §7 items 1–2 are the fix. If it is the second, name the capability — the answer is an
   export/sync feature, not a database.
2. **Are hand-editability, git-diffable templates, and file-copy migration commitments?** If they
   are negotiable, SQLite becomes defensible; if not, it is out under the current product shape.
3. **Is a future client/server direction intended?** If yes, don't spend it on this: design the
   sync/server story first, and let it choose the store.

If the answer to ⚑1 is "none — I wanted to know whether it would help", the honest answer is **no for
now**, and §7 is the only part worth planning.

## 11. Follow-up: could `local.yaml` be split into calibration + tool config first?

The player asked (same day) whether the file could be split so calibration stays YAML and the tool
config could later go to SQL. **Short answer: yes in principle, but not by moving blocks.** The split
line runs through blocks, through one record, and through one existing Save — it is a plan-sized
change, not a file move.

What actually mixes, from [`LocalOverrides`](../../core/crates/seal-domain/src/config/local.rs):

- **Pure tool/personal:** `settings:` (portable overlay), `spammer`, the `quest` records
  (flows/sequences/active), `buy_sell.presets` + `sell_cap`, pet behaviour preferences.
- **Pure machine/calibration:** `calibration`, `tooltip`, `ui`, `arduino.port`, `tuner.ocr` +
  `spring_point`, the `gem` geometry/movements/empty reference, the `buy_sell` geometry, the `pet`
  geometry/crops/run state.
- **Mixed inside one block:** `buy_sell` (geometry + presets), `pet` (geometry/crops + queue/food +
  rows). **Mixed inside one record:** `pet.slots[i]` carries `toggle_label` / `boarding_pet_slot` /
  `feeder_strip` / `pet_slot_empty_png` (calibration), `enabled` / `stacks` (config), and
  `boarding_running` (run state) in one row object.

Three structural blockers, in order of how much they cost:

1. **`pet.slots` is a list, and the merge replaces lists**
   ([`merge_overlay`](../../core/crates/seal-config/src/store.rs#L1034): objects merge, scalars and
   lists replace). Splitting one list across two files would drop one side unless the loader learns
   to merge arrays by index (changes merge semantics for every list) or the rows are re-modelled into
   two aligned lists (schema + protocol/shell work). The other mixed fields are scalars/maps and can
   be routed field-wise.
2. **One Save already spans both halves.** The Pet tab's Save sends `pet` (machine) and
   `settings.pet` (portable) in ONE `config.apply` (`_PetDraftGroup` in `pet_tab.dart`), which today
   lands as one atomic write of one file, one revision. With two files that becomes either a
   two-file commit (journal/rollback, a new failure mode), two shell saves (changes the Save bar's
   semantics), or leaving `settings.pet` in the machine file (breaks the point of the split).
3. **The writer's guarantees are per file.** Header, foreign blocks, atomic replace and the
   `config.changed`/rev contract all assume one file. Two files double that machinery, migration has
   to split an existing `local.yaml` exactly once (and the v2 import path with it), and the publish
   deny-list plus CONFIG/INSTALL/CUTOVER/USER_GUIDE (zh-TW twin) learn a new player-state file.

The options that survive, cheapest first:

| option | what it is | what it buys | what it costs |
|---|---|---|---|
| **C. No split — export/import** | keep one file; add `config.export`/import for the portable tool config (settings, spammer, quest, buy presets) | sharing between machines/accounts, no writer change | a feature, not a split; tool config stays in `local.yaml` |
| **B. Records-only split** | a second file for the clean personal blocks (`spammer`, `quest`, `buy_sell.presets`), settings/pet stay | ~10 KB of shareable presets out of the machine file; Pet Save unaffected | routing for those blocks + migration/docs; not a full calibration/tool split |
| **A. Full field-level split** | `calibration.yaml` vs `tools.yaml`, routed in the core; protocol unchanged, one Pet-Save path changed or a two-file commit | shareable, SQL-ready `tools.yaml`; clean ownership | plan-sized (4-layer load, routing map, two-file commit, migration, tests, docs); new half-save risk to design away |
| **D. Block re-partition first** | rename/split blocks (pet rows → calibration + records) so the split is block-level | the clean end state | largest: protocol, shell, migration and every config test change |

**Recommendation:** choose by goal. If this is for **sharing/sync** between machines, do **C** now and
**B** only if a second-machine workflow makes it worth a file. If it is to make **SQL** possible, do
**not** — §4–5's cost/benefit does not change, and the split adds a prerequisite cost on top. If it is
**file tidiness**, the 84 % win is §7.4 (move the inline PNGs), not a file split. In every case there is
no performance gain: two files cost two parses instead of one, same order as today.

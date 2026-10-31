# PLAN — config layout: one purpose per file (portable setup vs machine calibration)

**Proposal.** Give the player a config layout that is easy to read, easy to copy between PCs, and safe
to keep across core replacements. A file is exactly one **class × domain**: either part of the
**portable setup** (what you configured — travels) or part of the **machine** half (what this PC
measured — never travels), for exactly one tool (or the shared app). Today the same values live in one
`local.yaml` plus a `settings:` overlay, and the split line cuts through blocks. This document is the
layout proposal with the file-by-file contents, the mapping back to the original keys, the reasons, a
database variant for comparison, and the implementation increments. **The player makes the verdict;
nothing is built from this document alone.**

The requirement and the SQL verdict are in [ANALYSIS-CONFIG-STORE.md](../investigations/ANALYSIS-CONFIG-STORE.md);
the earlier two-file version is option **B** in §10 and is kept only as a comparison.

## 1. The rule a file follows

> **A config file holds one class × one domain. The class says whether it travels; the domain says
> which tool owns it. The name answers both questions.**

- **Classes:** `defaults` (shipped), `setup` (portable, player-owned), `machine` (per-PC), `user`
  (free-form notes), release files (`attributes.yaml`).
- **Domains:** `common` (app/board-level), `tuner`, `gem`, `spammer`, `quest`, `buy_sell`, `pet`.

No file mixes a portable value with a per-PC value, and no file holds two tools' records. That is the
whole readability test: "where is my spammer rotation?" → `setup/spammer.yaml`; "what must I copy to
the laptop?" → `setup/`; "what can I delete and recalibrate?" → `machine/` + `artifacts/`; "where does
this default come from?" → `defaults.yaml.example`.

## 2. The new tree

```
v3/config/
├── README.md                      # SHIPPED — how this folder works: what to copy, what to back up
├── defaults.yaml.example          # SHIPPED — the base every portable key falls back to
├── attributes.yaml                # SHIPPED — TW OCR dictionary (release-maintained)
├── attributes.us.yaml.example     # SHIPPED — US OCR dictionary seed
├── user.yaml                      # optional — the player's own notes/extra blocks; core never reads it
├── meta.yaml                      # MACHINE — core bookkeeping: migration/fold markers; not hand-edited
├── setup/                         # PORTABLE — copy this folder to another PC
│   ├── common.yaml                #   shared portable preferences (window, game, board IDs, hotkeys)
│   ├── tuner.yaml                 #   portable tuner behaviour
│   ├── gem.yaml                   #   portable gem behaviour
│   ├── pet.yaml                   #   portable pet behaviour (the two feeder preferences)
│   ├── spammer.yaml               #   the spammer rotations
│   ├── quest.yaml                 #   the quest flows and sequences
│   └── buy_sell.yaml              #   buy presets + sell cap
├── machine/                       # PER-PC — never copied; safe to regenerate by calibrating
│   ├── display.yaml               #   the display environment a calibration was measured in
│   ├── tuner.yaml                 #   tuner OCR geometry + spring point
│   ├── gem.yaml                   #   gem points, move deltas, empty-result reference
│   ├── buy_sell.yaml              #   bag/shop geometry, scroll/MAX points, sell slots
│   ├── pet.yaml                   #   pet points, bag grids, rows, queue, food cells, run state
│   ├── tooltip.yaml               #   hover-panel offset and size
│   ├── ui.yaml                    #   this PC's window placement
│   └── arduino.yaml               #   this PC's COM port
└── artifacts/                     # PER-PC — images the core writes (calibration crops, learned icons)
    ├── calib_gem_result.png
    ├── pet_close.png
    ├── pet_food_icon.png
    ├── pet_slot_empty_1.png …
    └── pet_queue_1.png …
```

Created lazily: a fresh install has no `setup/`, `machine/` or `artifacts/` — the core writes a file
the first time a save touches it. A missing file means "not stated", so every key falls back to
`defaults.yaml.example`, exactly as today. That is also why there are no seed `.example` files for the
new folders: the default *is* the seed, and a fresh install is honestly uncalibrated (each tool refuses
with its existing line until it is).

## 3. Setup files — contents and the original location

`setup/*.yaml` are the portable half. They are created by the core on a Save; the player may hand-edit
them; they are what you copy to another PC. Each file starts with `schema_version: 1` (the one reserved
key; stripped before the config is exposed over the protocol — see §6).

| file | contains | was, in the original layout |
|---|---|---|
| `setup/common.yaml` | `window.title`; `game.variant`, `game.variants.*`; `reference_window.width/height`; `arduino.vid/pid/baud`; `hotkeys.start/quit/advance_grade/pause` | `defaults.yaml`'s `window`, `game`, `reference_window`, `arduino` (minus `port`) and `hotkeys` sections; the player's overrides were in `local.yaml → settings.window / reference_window / game / arduino / hotkeys` |
| `setup/tuner.yaml` | `grade_order`, `target_grade`, `max_retries`, `save_captures`, `ocr_retries`, `spring_mode`, `mouse_guard`, `guard_px`, `recenter_max`, `models.*`, `timing.*`, `grade_colors.*`, `filter.*` | `defaults.yaml → tuner`; overrides in `local.yaml → settings.tuner` |
| `setup/gem.yaml` | `grades`, `start_grade`, `empty_mode`, `empty_streak`, `colored_gap_min`, `save_empty_captures`, `move_mode` | `defaults.yaml → gem`; overrides in `local.yaml → settings.gem` |
| `setup/pet.yaml` | `items_per_minute`, `pet_slot_occupied_above` | `defaults.yaml → pet`; overrides in `local.yaml → settings.pet` |
| `setup/spammer.yaml` | `active`, `presets`, `priority`, `combos`, `startup` (legacy `startup_step` is folded in by the one-time migration) | `local.yaml → spammer` |
| `setup/quest.yaml` | `active`, `presets` (flows + steps), `sequences`, `active_sequence`, `guard_px` | `local.yaml → quest` |
| `setup/buy_sell.yaml` | `presets` (scroll/row/count per named item), `sell_cap` | `local.yaml → buy_sell.presets` + `buy_sell.sell_cap` |

Why these groupings: `common.yaml` is everything edited from the **Setup** and **Hotkeys** cards and
shared by every tool; each tool file is exactly the commands that tool's card writes. The one bundled
file is `common.yaml` — its five sections are tiny, all portable, and all edited from the two "app"
tabs; splitting it into five files would add files without making anything easier to find.

Example — what a setup file looks like:

```yaml
# config/setup/spammer.yaml — PORTABLE. Copy this file/folder to another PC.
schema_version: 1
active: buffs
presets:
  buffs:            # key -> cooldown seconds
    a: 300
    b: 300
priority:
  buffs: [a, b]
combos:
  buffs: [{keys: [c, d], gap: 0.4}]
startup:
  buffs: {lead_in: 1.5, step: 0.2}
```

## 4. Machine files — contents and the original location

`machine/*.yaml` are written by the calibrators (and a few by the shell's own state writers). They are
never copied to another PC and can be regenerated by recalibrating. Same `schema_version: 1` key.

| file | contains | was |
|---|---|---|
| `machine/display.yaml` | `dpi_scale`, `monitor_dpi`, `screen`, `client_size`, `measured_at` | `local.yaml → calibration` |
| `machine/tuner.yaml` | `ocr.region`, `ocr.grade_area`, `ocr.grade_y`, `ocr.attr_y`, `ocr.remaining_y`, `ocr.row_height`, `spring_point` | `local.yaml → tuner` |
| `machine/gem.yaml` | `grade_positions.*`, `resource_gems`, `result_gem_area`, `empty_signature`, `empty_distance`, `movements.*` | `local.yaml → gem` |
| `machine/buy_sell.yaml` | `bag_grid`, `bag_slot`, `shop_rows`, `shop_region`, `scroll_point`, `max_button`, `sell_slots` | `local.yaml → buy_sell` minus `presets`/`sell_cap` |
| `machine/pet.yaml` | `menu_button`, `feed_icon`, `close_button`, `page_tabs`, `bag_grid`, `bag_slot`, `food_slots`, `food_slots_used`, `action_wait_ms`, `wait_after_empty_minutes`, `food_load_mode`, `feeder_count_left_fraction`, `feeder_count_min_score`, `reload_on_start`, `return_slot`, `max_button`, `boarding_window_may_be_open`, `slots[]` (`enabled`, `toggle_label`, `boarding_pet_slot`, `feeder_strip`, `stacks`, `boarding_running`), `queue[]` (`label`, `rect`, `species`) | `local.yaml → pet`; its four inline base64 crops move to `artifacts/` (§5) |
| `machine/tooltip.yaml` | `offset_x`, `offset_y`, `width`, `height`, `hover_delay_ms` | `local.yaml → tooltip` |
| `machine/ui.yaml` | `left`, `top`, `width`, `expanded_height`, `pinned` | `local.yaml → ui` |
| `machine/arduino.yaml` | `port` | `local.yaml → arduino.port` |

Why `pet` stays whole in one machine file rather than splitting calibration from queue/food/run state:
its row list mixes calibrated points and player flags inside one record, its queue icons are crops
captured on this machine, and its food cells are bag coordinates. One file keeps it coherent; the
portable pet behaviour is already in `setup/pet.yaml`.

## 5. Special files

| file | class | contents | was |
|---|---|---|---|
| `defaults.yaml.example` | shipped defaults | every portable key with its default, fully commented | `defaults.yaml` (unchanged content; still the single shipped base) |
| `attributes.yaml` / `attributes.us.yaml.example` | release | OCR dictionaries; a user-grown variant stays a user file | unchanged |
| `user.yaml` | user notes | the player's own extra top-level blocks/notes, **carried verbatim and never read** | the free-form foreign top-level blocks that `local.yaml` used to preserve |
| `meta.yaml` | machine bookkeeping | one-time migration markers (`legacy_folded`, the split marker, layout version) — core-owned, not hand-edited | the `settings.legacy_folded` marker and the fold's once-only state |
| `artifacts/*.png` | per-PC images | the saved calibration crops and learned icons | `config/calib_gem_result.png` plus the inline base64 fields `pet.close_button_png`, `pet.food_icon_png`, `pet.slots[].pet_slot_empty_png`, `pet.queue[].png` |
| `config/README.md` | shipped guide | which folder to copy/back up, what each file is, what is safe to delete | new |

Moving the images out of YAML is the single biggest readability win: on the real machine config,
**102,901 of 122,947 bytes (84%) were 14 base64 PNG lines**. Removing them takes the whole text config
to roughly 20 KB, makes every file diffable, and shrinks a backup to a few readable files.

## 6. Load order, the wire, and the reserved key

- **Load order** is explicit in a file registry, not filesystem order:
  `defaults.yaml.example` (base) → `setup/common`, `setup/tuner`, `setup/gem`, `setup/pet`,
  `setup/spammer`, `setup/quest`, `setup/buy_sell` → `machine/display`, `machine/tuner`,
  `machine/gem`, `machine/buy_sell`, `machine/pet`, `machine/tooltip`, `machine/ui`,
  `machine/arduino` → variant title resolution.
- **One owner per path.** The registry is the authority on which file owns which key; the loader merges
  field-wise (objects merge, scalars/lists replace, `null` = not stated) exactly as today. Precedence
  only exists between the defaults and a stated override, because no two files own the same path. A
  test walks the typed model and fails if a key has no owner or two owners.
- **The protocol shape does not change.** `hello` still answers one merged config; the `settings:`
  block is synthesized from `setup/common` + `setup/tuner` + `setup/gem` + `setup/pet`, the personal
  blocks come from their setup files, and the machine blocks come from their machine files.
  `config.apply` still carries blocks and `config.changed` is still block-level — the shell does not
  need to know the layout.
- **`schema_version`** is the only reserved key inside a data file: the loader strips it before the
  config is merged or exposed, the writer writes it back on every save. It makes a copied file
  self-describing (a core that does not understand the version refuses loudly instead of guessing).
- **Unknown keys inside a file are kept**, as today ("nothing inside an owned block can disappear").
  `user.yaml` replaces the old raw-text foreign-block splicing: the one-time migration moves any
  top-level blocks the core did not own there verbatim, and the core never writes that file again.

## 7. Who writes what

| action | file written |
|---|---|
| Setup card Save (title / variant / board IDs) | `setup/common.yaml` + `machine/arduino.yaml` (two files — one transaction) |
| Hotkeys card Save | `setup/common.yaml` |
| Tuner card Save | `setup/tuner.yaml` |
| Gem card Save | `setup/gem.yaml` |
| Spammer card Save | `setup/spammer.yaml` |
| Quest card Save | `setup/quest.yaml` |
| Buy/Sell card Save | `setup/buy_sell.yaml` |
| Pet card Save (setup half + machine rows) | `setup/pet.yaml` + `machine/pet.yaml` (two files — one transaction) |
| Tuner / Gem / Buy / Pet calibration saves | the one `machine/<domain>.yaml` that owns the mark |
| Tooltip calibration | `machine/tooltip.yaml` |
| Window move/resize (shell) | `machine/ui.yaml` |
| Pet run state (`boarding_running`, food scan apply) | `machine/pet.yaml` |
| Fold / adoption / one-time migration | the setup files; markers in `meta.yaml` |
| Player's own notes | `user.yaml` — the core never writes it |

Every save is **one atomic write per file** (temp + fsync + rename; the fsync closes the durability gap
the study found). The only commands that span two files are the Setup and Pet card Saves; they run as
one transaction (journal + recovery copy), so one Save stays one `rev` and one failure writes neither
file. No command writes more than two files.

## 8. Versioning, migration, and "replace the core, keep my setup"

- **Per-file `schema_version`.** Files are independent: a copy of `setup/spammer.yaml` states what it
  is, and a core that meets a newer version refuses loudly, writing nothing, instead of guessing.
  `meta.yaml` holds the one-time markers and the layout version; it is machine-local, not copied.
- **One-time migration from today's layout**, for a v3 install or a v2 `local.yaml` copied in:
  back up the whole `config/` folder (`config/.migration-backup-<stamp>/`); read `local.yaml`; write
  the setup and machine files; decode the inline base64 crops into `artifacts/`; move foreign blocks
  to `user.yaml`; write the `meta.yaml` markers; verify that the new tree loads; only then retire the
  old `local.yaml` (the backup is the rollback). The migration is idempotent and transactional per
  §7 — a crash rolls back, and the retry sees no new layout yet.
- **The guarantee the player asked for:** `setup/`, `machine/`, `artifacts/` and `user.yaml` are
  **player state — they are not in the release zip**, so replacing the core and the shipped templates
  never touches them. Only an explicit version migration writes them, it backs up first, it is
  additive, and it keeps unknown keys. Acceptance: replace the core and templates in a copied install,
  boot, save nothing → the tree is byte-identical; then open a tool and save one preset → only that
  preset changes.
- **Downgrade** is documented as one-way: an older core does not know the layout and would write its
  own shape; the supported rollback is the backup folder.

## 9. Why this is more readable and more useful

- **Findability:** the path answers the question. Presets are in `setup/<tool>.yaml`, measurements in
  `machine/<tool>.yaml`, defaults in `defaults.yaml.example`, notes in `user.yaml`.
- **Portability is a folder copy.** `setup/` (or one file inside it) goes to another PC; `machine/`
  and `artifacts/` deliberately do not, so a copied setup can never carry stale coordinates.
- **Safety is structural.** A preset save cannot rewrite calibration, and a calibration save cannot
  rewrite presets: the write path is the file. That is stronger than a convention.
- **Diffs and sync are local.** A change to spammer rotations diffs one small file; two PCs editing
  different tools do not conflict; two PCs editing the same tool conflict in one readable file.
- **Backups are obvious.** Back up `setup/` and `user.yaml` for the stuff that is precious; back up
  nothing in `machine/` because recalibrating regenerates it.
- **Text config drops to ~20 KB** once the base64 crops are files, so every file opens instantly and
  diffs cleanly.
- **It replaces the foreign-block text splicer** with `user.yaml` + "unknown keys are kept", removing
  the class of bugs the raw-text carry has produced.
- **Honest costs:** ~15 player files instead of 1 (a registry and routing table is required, and a
  field must be registered when one is added); the one-time migration is real work; two Saves span two
  files; comments inside files are still lost on save (headers and `config/README.md` carry the
  rules — a comment-preserving YAML editor is a separate future decision); no shell redesign is
  expected.

## 10. Alternatives — for the verdict

| option | player-visible files | readable / editable | portability granularity | migration and writer cost | cross-file saves |
|---|---|---|---|---|---|
| **A. Today** (`defaults.yaml` + `local.yaml` + `settings:` overlay) | 1 + shipped | low — ~1,000 lines mixing all tools and both classes | none; copy everything and carry calibration | none | none |
| **B. Two layers** (`setup.yaml` + `local.yaml`, the earlier plan) | 2 + shipped | medium — machine half is one long file | whole setup at once | medium | 2 (Setup, Pet) |
| **C. Class × domain (this proposal)** | ~15, all small | **high** — one tool, one purpose per file | one tool, or the whole `setup/` | medium-high: file registry, routing, one-time split | 2 (Setup, Pet) |
| **D. Setup per tool + one `machine.yaml`** | 8 + shipped | high for setup; machine half one long file | one tool, or the whole `setup/` | medium | 2 |
| **E. SQLite / PostgreSQL store** | 1 database | low — not hand-editable or diffable | copy a binary; sync is unsafe | high; migrations mutate the file | n/a (SQL transaction) |

**Recommendation: C.** It is the only option that makes "what travels" and "which tool" both visible
in a filename, and the cross-file cost stays at the same two Saves as B. D is the fallback if ~15 files
is judged too many for one player; B is the minimum-change alternative; E is addressed in §11.

## 11. The database option — what it would contain, and why not

If the verdict is a database, the schema that carries the same data is:

```text
meta(key, value)                                  -- layout/schema version, fold marker
settings(section, key, value_json)                -- window, game, reference_window, arduino (ids), hotkeys, tuner, gem, pet
hotkeys(action, vk)                               -- start, quit, advance_grade, pause
spammer_presets(preset, active)
spammer_keys(preset, key, cooldown_s)             -- from spammer.presets
spammer_priority(preset, ord, key)                -- from spammer.priority
spammer_combos(preset, ord, keys_json, gap_s)     -- from spammer.combos
spammer_startup(preset, lead_in_s, step_s)        -- from spammer.startup
quest_flows(flow, active)                         -- from quest.presets / quest.active
quest_steps(flow, ord, action, value, delay_s)    -- from a flow's steps
quest_sequences(sequence, active)                 -- from quest.sequences / active_sequence
quest_sequence_entries(sequence, ord, flow)       -- FK → quest_flows
quest_guard(drift_px)                             -- quest.guard_px
buy_presets(name, scroll, row, count)             -- from buy_sell.presets
sell_cap(n)                                       -- buy_sell.sell_cap
calibration_marks(kind, target, block, mark_json, artifact)  -- machine/tuner, gem, buy_sell, pet, tooltip
calibration_env(dpi_scale, monitor_dpi, screen_json, client_size_json, measured_at)
machine_state(key, value_json)                    -- ui placement, COM port
pet_rows(row, enabled, toggle_label_json, boarding_pet_slot_json, feeder_strip_json, stacks, boarding_running)
pet_queue(ord, label, rect_json, species, icon)   -- icon → artifacts
pet_food(ord, page, cell)                         -- pet.food_slots
artifacts(name, role, file)                       -- the PNGs stay files either way
```

Why it is **not** the recommended store (reasons, in the player's terms):

- **"Bring it everywhere" is worse.** A database is one binary plus `-wal`/`-shm` while open; a cloud
  folder or a USB copy of a live database can be stale or corrupt, and two PCs cannot merge edits. A
  copied YAML file is readable and mergeable.
- **"Setup files should remain intact" is worse.** A database is migrated by mutating the player's
  file; a failure leaves a file that needs the backup. YAML migrations are additive and inspectable.
- **It removes the inspection/editing that makes the layout useful.** No diffing a spammer rotation,
  no hand-editing a preset, no `git log` on a setup folder.
- **It does not buy performance.** The study measured a full load at ~2.4 ms and a save at ~4 ms; the
  config is not on a hot path. A database replaces that with an open/transaction cost of the same
  order.
- **PostgreSQL specifically is disqualified by deployment:** the product is a zip with no installer,
  no admin, no service, no network feature. SQLite would fit inside `seal-core.exe`, but its
  transaction/locking benefit is already delivered by the per-file writer + two-file transaction, at
  much lower portability cost.
- **Where a database would genuinely help later:** history/undo of setup changes, a searchable
  cross-account preset library, or a future server/sync — and in those cases it should be a *derived
  index or sync endpoint*, not the store of record. That is a separate decision (§14 ⚑6).

## 12. Implementation increments (if the verdict is C)

0. **Registry and ownership.** One table naming every file, its class/domain, its owned paths and its
   `schema_version`; a test walks the typed model and fails on an unowned or doubly-owned path. No
   behaviour change.
1. **Generalised file layer.** Per-file header, `schema_version`, owned keys, atomic temp+rename with
   fsync, and "unknown keys kept"; the existing single-file tests stay green.
2. **Multi-file loader.** Registry-ordered merge, missing = defaults, unknown files warned; the
   synthesized `settings`/block view preserves today's protocol answers; tests for order, one-owner,
   and a malformed file being loud.
3. **Writer routing and per-file atomic saves,** with one test per save path asserting which file(s)
   changed and which did not.
4. **Two-file transaction + crash recovery** for the Setup and Pet Saves; injection tests for failure
   before, between and during recovery.
5. **One-time migration** from v3 `local.yaml` and from a v2 copy: backup, split, artifacts decode,
   `user.yaml`, `meta.yaml`, verify-then-retire; idempotent and rollback tests.
6. **Artifacts extraction** on the live paths (readers accept the new files; the wire still carries
   base64); no protocol change.
7. **Packaging and docs.** `publish.ps1` allow/deny for the new player-state paths; `config/README.md`;
   `spec/CONFIG.md` rewritten as the file map; `spec/PROTOCOL.md` §7 (routing), `INSTALL.md` (copy/backup/
   upgrade/rollback), `CUTOVER.md` (packaging decisions), `USER_GUIDE.md` + `.zh-TW`; the release row
   only when it ships.
8. **Acceptance and live pass.** Copy `setup/` between two roots (presets appear, calibration absent);
   simulated upgrade (replace core + templates, setup tree byte-identical); one live tool run on the
   copied setup; full core and shell gates green.

## 13. Acceptance

- A copied `setup/` reproduces the portable setup on a second PC; `machine/` and `artifacts/` are
  absent there and the tools calibrate from scratch as today.
- Replacing the core and templates never changes the player-state tree; a version migration backs up
  first, keeps unknown keys, and refuses loudly (writing nothing) for a newer file.
- Every failed save leaves every touched file at pre-save content and consumes no revision, including
  the two-file Saves and a crash between their renames.
- All existing config tests stay green; new registry, routing, migration, transaction and recovery
  tests pass; the publish audits pass with the new paths.
- The docs named in increment 7 describe the layout in the same commits.


### Implementation record

- **2026-10-31 — implementation started, layout C assumed.** The Lead asked for this plan to be
  implemented, so the document's own recommendation is taken as the verdict: layout **C**
  (`config/setup/` + `config/machine/` + `config/artifacts/`), the file names from §2, and one
  `setup/common.yaml` (not split per concern). The still-open ⚑ answers are implemented with the
  plan's stated preferences and can be changed until increment 2 wires the loader: per-file
  `schema_version: 1` (reserved key), `user.yaml` for notes/foreign blocks, artifacts extracted in
  increment 6, comments-in-files deferred (header + `config/README.md`), SQLite rejected as the store
  of record (a derived history/index remains a possible later feature).
- **Increment 0 landed:** `seal-config::registry` is the one table (file, class, domain,
  `schema_version`, owned paths); the coverage test decodes the real v2-era fixture through
  `LocalOverrides` and fails on an unowned typed leaf. No loader/writer behaviour changed yet.
- **Increment 1 landed:** `seal-config::file` is the per-file format layer — a file is a YAML
  mapping with the reserved `schema_version` (stripped on read, rewritten on save, a newer version
  refused loudly writing nothing), a class/domain header, unknown keys kept because the whole
  mapping is written, and temp + `fsync` + rename atomic saves. The old single-file store is still
  the live path; `tests/file_layer.rs` pins the layer, so the loader switch (increment 2) can rely
  on it.
- The player can still reject or amend any of the ⚑ answers before the loader switch; nothing in
  increments 0–1 writes the new tree.

## 14. ⚑ Questions for the verdict

1. **Layout:** C (class × domain, recommended), D (setup per tool + one `machine.yaml`), B (two files),
   or A (stay as today)? Or E (database) despite §11?
2. **File count vs granularity:** is `setup/common.yaml` (five tiny app-level sections) acceptable, or
   should it split into `window.yaml` / `hotkeys.yaml` / `arduino.yaml`?
3. **`schema_version` as the one reserved key** in each file — acceptable, or keep version out of the
   data (header comment) and accept a weaker self-description when copying one file?
4. **`user.yaml` + retiring the old foreign-block mechanism** — acceptable? (Existing foreign content
   migrates verbatim; notes become a normal file.)
5. **Artifacts now?** Moving the four base64 crop groups into `artifacts/*.png` is the biggest
   readability win in the proposal; it changes the stored shape of the pet fields (the wire is
   unchanged).
6. **Comments:** the writer still loses comments inside owned blocks. Accept header + `config/README`
   guidance for now, or should a comment-preserving YAML editor be a follow-up decision?
7. **Database:** reject entirely, or record "a derived SQLite index/history is a possible later
   feature, never the store of record"? (Recommendation: the latter.)
8. **Naming:** `config/setup` + `config/machine` + `config/artifacts`, or different words
   (`portable` / `local` / `images`)?

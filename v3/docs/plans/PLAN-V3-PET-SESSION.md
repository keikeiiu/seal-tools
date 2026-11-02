# Plan — the Pet tab's v2 session surface

**Proposal.** Nothing here is built. This file describes work not done; its state is the row on the
[plans board](README.md).

It closes audit gaps **A3 + B2/B8** ([PARITY-AUDIT-2026-10-28.md](../records/PARITY-AUDIT-2026-10-28.md);
[OPEN.md](../backlog/OPEN.md) §5): v3's Pet tab can edit stored data but cannot **capture, scan, queue
or test-read**, so a player cannot add a queued pet or refresh the food cells at all. The Calibrate
plan deliberately scoped this out (⚑10 of [PLAN-V3-CALIBRATION.md](PLAN-V3-CALIBRATION.md)); this plan
owns it. The feeder's *run state* side (A1) is a separate core task; where it touches this surface it is
marked below.

---

## 1. What exists, and what cannot be done

| Layer | Today |
|---|---|
| Editable setup | rows enabled/stacks, queue **removal**, food cells (page + 8×8 grid), return slot, food-load mode, reload-on-start, action wait, wait-after-empty, count fraction/min score; one Save (`pet_tab.dart:28-43,358-709`) |
| Read-only | calibration marks, `*_png` crops, `food_slots_used`, per-row `boarding_running` (`pet_tab.dart:311-356,593-629`) |
| **Absent** | the entire v2 session workflow: source-cell marking, icon captures, bag scans, panel test read, boarding ticks, Setup-so-far / Ready-to-run checklists |

The core already holds the pieces: `pet.rs` decodes queue/food icons and crops
(`seal-core/src/pet.rs:111-145,285-320,333-410`), exposes a capture/OCR/cursor seam
(`PetReadSource` `:573-724`), and **the v2 icon matcher is ported**: `differing_fraction_images`
(`:875`), `score_cell` (`:982`), `cells_holding` (`:997`), `score_all` (`:962`, currently private),
`MATCH_LIMIT = 0.5` (`seal-domain/src/tools/pet/decision.rs:35`). No new image dependency is needed.

---

## 2. Feature inventory (control by control)

v2 evidence is `v2/SealTools.Launcher/MainWindow.xaml.cs` unless stated. "Rules" is behaviour the v2
guide only partly states.

| v2 control | v3 today | What it must become (and the rules to keep) |
|---|---|---|
| **Page to edit / Mark FOOD cells / Mark RETURN slot / Mark a pet to queue** (`:6905-6926`, `:8197-8244`) | food grid + return-slot fields exist; no source-cell concept | v3 keeps separate editors (deliberate simplification) and adds a **"source cell" selection** (page+cell, shell view state) used by queue capture and the panel test read — v2's modes existed only because one grid served three meanings |
| **Capture food icon** (`:6971-6972`; handler `:9310-9400`) | read-only display | takes the **explicitly selected food cell** (v2 used the most recently marked one — CHANGED, clearer): click that page's ITEM tab, wait, capture, crop the bag-grid pitch box at the cell centre, store `pet.food_icon_png`; preview |
| **Scan the bag for food** / **Scan + update the food cells** (`:6973-6978`; handler `:9404-9500`) | absent | per page: click ITEM tab, **park the cursor off the bag** (scroll point) before the hidden capture, `CellsHolding` in CELL order; look-only reports; write replaces `pet.food_slots` and resets `food_slots_used`; **nothing written if no match** |
| **Queue add by capture** (`:7162-7194`; handler `:9228-9305`) | absent (removal only) | label + species pickers, capture the selected pet cell's icon via the bag grid (queue crop rect = pitch box), append `{label,species,rect,png}`; v2 captures **the current screen** and the hint says "put that page up"; v3 should click the page tab (CHANGED, matches the other captures) |
| **Queue rows editor** (`:7874-7960`) | read-only label/species + Remove | per-entry thumbnail, editable label, per-entry species picker, per-entry ✕; v2 auto-saved each edit — v3 uses the normal Save (CHANGED, draft path) |
| **Remove last** (`:7194-7202`) | per-entry Remove | keep v3's per-entry remove (strictly better); do not add "Remove last" |
| **Scan the bag for these pets** (`:7258`; handler `:9057-9220`) | absent | per page: tab click, park, capture + **save the page image as evidence**, per icon `ScoreAll` (hits ≤ `MATCH_LIMIT`), **claimed-cell conflict** accounting, deduced "page N: label ×n" lines + raw scores/runner-ups to a `logs/reads` report; no bag clicks |
| **Test read the pet panel** (`:7276`; handler `:8490-8610`) | absent | needs the Tooltip calibration; selected cell → click page tab → hover the cell centre (`FocusThenHover`) → read `cell + tooltip.offset`, region `tooltip.width/height`; OCR ×3 with a debug crop; `PetPanel::parse`; report lines **plus every number in context** (diagnostic, not parser) |
| **Scan bag for feedable pets** (`:7290`; handler `:8628-8900`) | absent | every cell of every page: hover + read + classify finished / feedable / unknown; **moves only — a click on a pet switches the equipped pet**; hides the window; ~1.5 s/cell ⇒ ~90 s/page; per-page results; a failed read is **never** called finished |
| **Scan + rebuild the queue** (`:7293`; same handler, `writeQueue`) | absent | the same sweep, then replace the queue with one icon per feedable pet (crop via bag grid, label `page X, cell Y`, no species); **never clear the queue when zero icons were harvested** |
| **Rows and boarding state — per-row ticks** (`:7089-7153`) | only enabled/stacks; boarding state read-only | per row: `run` (already editable) + **`boarding now`** tick (`pet.slots[i].boarding_running`); writes go through the A1 core writer so they cannot race the run's own mid-run updates; the tab renders `config.changed` |
| **Setup so far — this tab** (`:7011-7024`, `:8009-8043`) | absent | session checklist: return slot, food cells (+used, one-round load count), queue, per-row boarding; distinct from Calibrate Pet's calibration checklist |
| **Ready to run** (`:7021-7024`, `:8308-8360`) | absent | zero rows, no return slot **and** no queue, no food cells, food cells < one round of stacks, queue-empty fallback line; "ready" names the per-row cycle minutes |
| **Save** (`:7325-7336`) | one Save already | session half only, geometry untouched — v3's scoping already matches (V2 `LocalPet.ApplySession`) |

Cross-cutting rules: scans need the **bag open**, the game foreground and the page tabs calibrated
(`pet.page_tabs`); ITEM-tab clicks move the cursor and must take the input lease; the hidden capture
happens with the shell window hidden (Flutter host channel); captures park the cursor off the bag, or
the pointer is part of the picture (v2 bag scan `:9103-9112`).

---

## 3. Where each piece lives

| Piece | Home |
|---|---|
| Source-cell selection, previews, checklists rendering, per-entry editors, progress text | **shell UI** (`pet_tab.dart`); no rules |
| ITEM-tab click, park, capture, crop, match, OCR, hover, classification, readiness rules | **core commands** (§4), reusing `PetReadSource`/`seal-win`/board placement and the pet matcher |
| Normal config edits (queue add/edit/remove/rebuild, `food_icon_png`, timings) | **shell draft → `config.apply`** (existing path; A1 never writes queue) |
| Run-state fields (`food_slots_used`, per-row `boarding_running`, food-cell replacement reset) | **A1 core-initiated writer** ([OPEN.md](../backlog/OPEN.md) §5) so a user tick/scan and a mid-run write cannot clobber each other; each write emits `config.changed` |
| Evidence files (scan page PNGs, scan report `.txt`, panel debug crop) | **core**, under `<root>/logs/reads/` — v2's location and what the guides point at; PROTOCOL §1's log list gains `reads/` |
| Calibrate Pet marks/crops (rows, page tabs, bag grid, empty-slot reference) | **not here** — `PLAN-V3-CALIBRATION.md` inc 6 (`calibration.pet.save`); this plan reads them |

---

## 4. Proposed protocol additions

All are stateless like `config.apply`: the shell holds the draft and view state, the core derives and
(where stated) writes. **Reused:** `pointer.place`/`pointer.click` (calibration inc 0, in flight) for
tab clicks and parking; `capture`/`window.measure` semantics (foreground gate, shell hide/show) for
every capture; the existing pet matcher/OCR/cursor seams; `config.apply` + `config.changed`; the
existing error codes (`bad_request` for an uncalibrated prerequisite with the domain's line,
`window_not_found`, `game_not_foreground`, `board_unavailable`, `tool_busy`, `io`) — **no new codes**.

| cmd | args | result |
|---|---|---|
| `pet.preflight` | `{}` | `{"ready":false,"calibration":[{"ok":true,"label":"…"}…],"session":[…],"lines":["…"]}` — computes the two v2 checklists' items and the Ready verdict from the merged config (`PetPlan::plan`/`check_board` are the rules); opens no window, no board |
| `pet.set_boarding` | `{"row":2,"running":true}` | `{"rev":21,"block":{…pet…}}` — one field through the A1 writer, then `config.changed`; `bad_request` when the row does not exist |
| `pet.apply_food_scan` | `{"cells":[[0,12],[1,3]]}` | `{"rev":22,"block":{…pet…}}` — replaces `pet.food_slots`, **zeroes `pet.food_slots_used`** (the set changed; a carried count would skip/repeat), one atomic A1 write + event; `bad_request` on a malformed cell |
| `pet.capture_icon` | `{"kind":"queue"\|"food","page":1,"cell":34}` | `{"kind":…,"page":…,"cell":…,"rect":[x,y,w,h],"png_base64":"…","evidence":"logs/reads/…png"}` — lease, ITEM-tab click, park, capture, bag-grid crop; shell hides/show around it; prerequisite refusals name the missing mark |
| `pet.scan_food_page` | `{"page":1}` | `{"page":1,"cells":[12,34,…],"report":["page 1: 2 cell(s) — 13, 35"],"evidence":["logs/reads/…png"]}` — look-only; input order = CELL order |
| `pet.scan_queue_page` | `{"page":1}` | `{"page":1,"per_entry":[{"index":0,"label":"…","hits":[{"cell":7,"score":0.02,"runner_up":0.31}]}],"claimed":[{"cell":7,"labels":["…","…"]}],"deduced":["… ×1"],"evidence":[…]}` — one page of v2 `PetScanBag`; report written by the core |
| `pet.test_read_panel` | `{"page":1,"cell":34,"delay_ms":700}` | `{"region":{…},"lines":["…"],"panel":{"growth":7,"exp":52,"line":"…"}\|null,"numbers":[…],"evidence":"logs/reads/petpanel_…png"}` — `bad_request` when the Tooltip calibration is unset |
| `pet.scan_feeding_page` | `{"page":1,"harvest":true}` | `{"page":1,"cells":[{"cell":7,"status":"finished"\|"feedable"\|"unknown","growth":9,"exp":100,"lines":[…],"crop_png_base64":"…"?}],"summary":"…","evidence":[…]}` — ~90 s; `harvest` returns icons for feedable cells only; no bag clicks |
| `pet.scan_apply_queue` | `{"queue":[{"label":"page 1, cell 7","png":"…","rect":[…]}]}` | `{"rev":23,"block":{…pet…}}` — replaces the queue only when non-empty (the "never clear on nothing" rule), via the normal config writer + event; `bad_request` on empty |

Rules: every command that touches the game takes the input lease and refuses `tool_busy` while a
foreground tool runs / the resident feeder holds the game (calibration ⚑4's decision); the **shell
hides itself before the request and shows after** (host channel), so each command's hidden window is
bounded to one page (the feeding sweep included — v2 also hid per page); `capture`'s foreground gate
still applies. Per-page commands are the unit of progress and of cancellation (the protocol has no
cancel mid-command; the shell can stop between pages — see ⚑2).

---

## 5. Increments

| # | Deliverable | Acceptance |
|---|---|---|
| 1 | **Preflight + boarding ticks + checklists** (`pet.preflight`, `pet.set_boarding`; Pet-tab UI: boarding ticks, Setup-so-far, Ready-to-run). Depends on A1's write/event channel; can land with read-only ticks if A1 slips, then wire the run updates. | core: preflight cases (zero rows, no return+queue, no food, short round, queue fallback) pinned against v2 `RefreshPetReady:8308` conditions; one field write bumps rev + emits one `config.changed`; shell: ticks/checklist render and a refusal is verbatim; **live: v2 config in → Ready matches v2's own Ready text** |
| 2 | **Icon captures + queue rows/add** (`pet.capture_icon`; queue add with label/species; per-entry label/species editing; food-icon capture; source-cell selection). | protocol: fake window/board/frames — page-tab click, park, crop rect == bag-grid pitch at the cell centre, `png_base64` decodes; refusals for missing bag grid/page tab; shell: add/edit/remove round-trip through Save; **live: capture a real pet and a food stack; the queue survives a restart and the run finds the pet** |
| 3 | **Bag scans** (`pet.scan_food_page`, `pet.apply_food_scan`, `pet.scan_queue_page`, report/evidence writer, previews and look-vs-write buttons). | core: matcher oracles translated from v2 `IconMatchTests`/`PetTool` scans (cell order, limit, claimed-cell conflict, no-write-on-no-match); protocol exact payloads; **live: food scan matches the bag v2 found, and the update resets the used count** |
| 4 | **Panel test read** (`pet.test_read_panel`). | core: region = centre + tooltip offset (assert exact numbers from a seeded tooltip config), parser + "every number" report, debug crop written; `bad_request` when uncalibrated; **live: reports a real pet's growth/EXP like v2's Test read** |
| 5 | **Feedable sweep + queue rebuild** (`pet.scan_feeding_page`, `pet.scan_apply_queue`). | core: per-cell classification against scripted panels (finished / feedable / unknown; failed read never finished), harvest only feedable, per-page evidence; queue replace refuses empty; shell: per-page progress and confirm; **live: sweep one real bag page, compare counts with v2's scan record, then rebuild** |
| 6 | **Parity pass + docs** (no new feature): player runs the v2 Pet-tab workflow end-to-end on the v2 config; update `TOOL-FLOWS.md` Pet Feeder, `USER_GUIDE.md` + `.zh-TW` §5.6/§11, `CONFIG.md` pet block, `PROTOCOL.md` (§1 reads/, §4 rows, §5/§7 A1 wording), prune `OPEN.md` §3/§5. | recorded v2-vs-v3 worksheet; docs gates; no "capture session (OPEN)" wording remains |

Order rationale: safety/readiness first; the capture primitive second (largest usability win); then
matcher scans (cheap, deterministic); then the OCR panel read; the long sweep last.

---

## 6. Risks and dependencies

- **A1 must define the write/event channel before increment 1** (OPEN §5 bullet 1; PROTOCOL §7 already
  anticipates core-initiated writes). Increment 1's ticks and increment 3's food apply use it; without
  it they fall back to `config.apply` drafts, which can race a mid-run write.
- **The icon matcher is ported but `score_all` is private** (`seal-core/src/pet.rs:962`); expose a
  shared sweep (`score_all`/`cells_holding`/`score_cell`) rather than a second implementation. The
  run's own bag sweep (pet reader increment) must use the same helper.
- **Long blocking commands**: the dispatcher is single-threaded; a ~90 s feeding page blocks other
  requests (no cancel). v2 was equally slow, but the UI must show "scanning page N…" and the guide
  must say the app is unresponsive. If unacceptable, the alternative is a background job with
  `state`/`tool.finished` events — a protocol change, not this plan's first shape (⚑2).
- **Hide/lease choreography**: scans click the game and capture it; the shell must be hidden for the
  capture and the game foreground for the clicks. Per-page commands keep both bounded.
- **Prerequisites are marks this plan does not create**: `pet.page_tabs`, `pet.bag_grid`,
  `tooltip.*`, `buy_sell.scroll_point` (parking) — all named refusals, all drawn on Calibrate.
- Docs are in flux (Pet-tab wording being rewritten); increment 6 is the only one that touches
  shipped guides, so wording lands once.
- `logs/reads/` does not exist in v3 (PROTOCOL §1 lists `captures/` only); adding it is a small core
  change and a §1 edit — do it in increment 3, when the first evidence is written.

---

## 7. ⚑ Questions (player/Lead)

**⚑1 — Config writes for session results: core field-writers or shell drafts?** Proposed: queue edits
and `food_icon_png` through the normal save (shell draft); `boarding_running` and food-cell
replacement through A1 field-precise core commands, because the run writes the same fields mid-run.
Confirm; the alternative (draft `config.apply` for everything) risks a lost run update.

**⚑2 — Accept ~90 s/page blocking scans now, or specify a job protocol?** Proposed: per-page blocking
commands with UI progress and no mid-command cancel (v2 parity); the long sweep is the only one where
this matters. Say if a background-job protocol is wanted instead.

**⚑3 — Should v3 click the ITEM tab itself when capturing a queue icon?** v2 required the player to
put the page up first (`:7226-7228`); every other capture in v2 clicks it. Proposed: click it (CHANGED,
fewer silent misses). Confirm.

**⚑4 — Source-cell selection.** v2's three modes (FOOD / RETURN / pet-to-queue) existed because one
grid had three meanings; v3 already splits the editors, so proposed: a separate "source cell" picker
for queue capture and panel read only (view state, not config). Confirm the CHANGED interaction.

**⚑5 — Queue rebuild content.** v2 harvests icons with labels `page X, cell Y` and no species, and
replaces the queue only when at least one icon was cropped. Proposed identical behaviour (species left
for the player to set on the queue rows). Confirm.

**⚑6 — Boarding ticks vs a dirty draft.** Proposal: ticks write immediately through
`pet.set_boarding`; run updates arrive as `config.changed` and the existing external-change banner
(`config_draft.dart:52-96`) prompts a reconcile rather than silently rebasing. Confirm this is the
wanted feel.

**⚑7 — Preflight and the board.** v2's Ready used the cached firmware level; a fresh probe opens the
port. Proposed: `pet.preflight` reports the board row as "unknown" and opens nothing; the card's start
refusal remains the authority. Confirm.

**⚑8 — Feeding scan's hiding granularity.** v2 hid the window per page and came back with that page's
results (`:7305` hint; hidden per page at `:8757,8857`). Proposed: one hide per `pet.scan_feeding_page`, restored after each page's
command. Confirm (the alternative — hide for the whole sweep — leaves the shell blank for minutes).

**⚑9 — `Remove last` stays gone.** v3's per-entry Remove replaces v2's button; the plan keeps that.
Confirm no one relies on the old button's position.

---

## 8. Out of scope

The four v2 Calibrate surfaces and Setup Detect (their own plan, in flight); Calibrate Pet's marks and
the empty-slot crop (`calibration.pet.save`); the feeder's run loop, scheduling and food-load
behaviour; pet-data/feeding-table work; the Sell grid and other audit items (OPEN §5, separate
batches). Nothing here changes the config field set or adds a second config path.

---

## 9. Lead decisions (2026-10-28)

All nine questions are answered; three were settled by what already shipped in increments 1-3, and the
answers below say where the shipped behaviour differs from the proposal on purpose.

1. **^1 Write routing — DECIDED: confirmed.** Queue edits and `food_icon_png` ride the normal shell Save
   (config.apply); `boarding_running` and food-cell replacement go through the A1 field-precise core
   commands. This shipped: pet.set_boarding and pet.apply_food_scan use the shared field-precise writer,
   never a whole-block apply.
2. **^2 Blocking scan surface — DECIDED: per-page blocking commands with visible progress now.** The
   scans are one page per command (no sweep) and the shell shows progress/result; there is no
   mid-command cancel (v2 parity). A background-job protocol is out of scope until the live pass shows
   the long feeding sweep (~90 s/page) is genuinely unacceptable; if it is, the job protocol gets its
   own plan rather than being half-built into these commands.
3. **^3 Clicking the ITEM tab for queue capture — DECIDED: NO, changed from the proposal.** The
   shipped capture/scan commands never click or verify the ITEM tab: the shell hides itself and the
   player/shell puts the page up, the core validates the requested page's 2-point tab mark only, and the
   UI/docs warn that a wrong page gives a plausible wrong crop/result. Rationale: clicking needs a board
   and a second command whose per-command lease snapshot does not cover the capture; a wrong-page
   warning plus the mandatory preview is the smaller honest seam for now. Revisit only if live use
   shows frequent wrong-page captures.
4. **^4 Source-cell selection — DECIDED: confirmed.** A separate source-cell picker (page + cell, view
   state, not config) for queue/food icon capture; shipped as the ITEM chips + single-selection 8x8
   grid. The panel test read (increment 4) uses the same view-state picker.
5. **^5 Queue rebuild — DECIDED: confirmed, deferred to increment 5.** Same as v2: harvested icons get
   `page X, cell Y` labels and no species (the player sets species on the rows), and the queue is
   replaced only when at least one icon was cropped. Never clear on an empty harvest.
6. **^6 Boarding ticks vs a dirty draft — DECIDED: confirmed with one refinement.** The tick writes
   immediately through pet.set_boarding and run updates arrive as config.changed. Refinement shipped:
   while THIS tab's draft has unsaved edits the tick is DISABLED with "Save the row edits first" (or
   "Save the edits first" for portable-only edits), because an A1 write beside a dirty draft would show
   the player their own action as an external change. The external-change banner remains for genuinely
   external updates.
7. **^7 Preflight and the board — DECIDED: confirmed.** pet.preflight never opens the port; the board
   row is informational ("not checked by preflight — normal", ok:true rendered with the information
   marker), and the card's start refusal stays the authority.
8. **^8 Feeding scan hiding granularity — DECIDED: confirmed.** One hide per page command, restored
   after each page; the shell is never blank for a whole sweep. The same one-hide-per-command rule
   applies to every capture/scan command.
9. **^9 Remove last — DECIDED: confirmed.** v3 keeps per-entry Remove; v2's "Remove last" does not come
   back.

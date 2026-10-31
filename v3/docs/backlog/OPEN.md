# v3 — open work

What is genuinely not done in v3, and where it lands. Like v2's backlog, this is decided or named
work — not ideas. **Nothing gets an entry just because it is hard; when a thing ships it leaves this
file** and its reasoning graduates into `spec/` (dated history lives in `records/PROGRESS.md`). The
cut-over steps themselves are [plans/CUTOVER.md](../plans/CUTOVER.md).

## 1. Needs a player measurement or the game

- **Live verification per stage.** The repo's rule: a tool is done when it has run in the game, not
  when it compiles. No card has yet been watched end-to-end against the game in v3, and no calibrator
  pass has been taken from the shell UI. This needs the player and the game (PLAN §8).
- **Global hotkeys — do not build until measured** (PLAN §6.5). The core never owns the foreground
  window, so whether a core-side `GetAsyncKeyState` poller works in game is an open measurement; the
  decision decides between core-side polling and shell-side commands. **v3.0 ships without a global
  poller and says so** — Stop and the shell's buttons always work. This is why there is no pause
  semantics either (v2's `PauseRequested`/`IgnoresQuitHotkey` rules become relevant only with the
  poller).
- **OCR parity recordings** (PLAN §13.10): the archived corpus covers the tuner's full-region path
  only; the feeder-count and pet-panel reads need raw crops recorded from a live session, per read
  path.

## 2. Named code follow-ups (built around, not hidden)

- **Spammer runner-side stand-down.** `GamePort::should_stand_down` exists and the supervisor cancels
  a PREEMPTED spammer through the arbiter (v2's [YIELD] behaviour); the runner loop has no yield hook,
  so a running spammer is stopped rather than asked to step aside in place. The deferred start's 45 s
  timeout is a `log` warning because the caller was already answered `Deferred` (this protocol has no
  channel to update it).
- **Single-instance cross-process lock.** The save gate is in-process; two cores on one root can
  interleave (PROTOCOL §1's lock is specified, not built).
- **OCR no-retry gap.** v3's pipeline scans ONCE; `tuner.ocr_retries` exists in config but is unused,
  so v2's last-retry forced capture and its `refused:` diagnostics branch are not implemented
  (`ocr_errors.jsonl` records a completed-but-unconfirmed scan instead). The grade colour fallback and
  the `remaining`-band read (v2's) are also unported: `remaining` is null and the grade comes from the
  line only.
- **Pet feeder reader follow-ups.** v2's feeder-count 3x/9x/24x upscale ladder is not ported
  (`scan_rgba` has no upscale parameter; one pass per slot plus the v2 warm-colour verdict). There is
  no close-button "window already open" probe and no food-cell re-mark from the bag scan
  (`cells_holding` is ported; the configured food list is used until exhausted). `ToolStatus` has no
  schedule field, so the schedule line travels in the rows' `next_action_in_seconds`.
- **Tuner wording.** The guard's recentre path surfaces a failed placement as the effects' own line
  rather than v2's composed "cursor wouldn't re-centre — <detail>" (the runner performs effects; the
  tool cannot see their failure).
- **Tuner grade-colour fallback (v2 `DetectGradeColorScores`) is not ported.** v2 used
  `tuner.ocr.grade_area` for two jobs: selecting the grade TEXT line (now wired in v3, region-relative)
  and deciding the grade by COLOUR when the text was unclear. v3 does the first and not the second; the
  fallback belongs with the calibration-feature plan's tuner increment. The colour palette
  (`tuner.grade_colors`) is already config and readable.
- **Clock trait duplication.** `seal-tools::clock::Clock` and `seal-board::clock::Clock` are the same
  one-method seconds-only trait (seal-board does not depend on seal-tools); move it to `seal-domain`
  when convenient. The seconds-only unit is deliberate (the eleven-minute-sleep incident).

## 3. Session-shaped features (named, not marks)

- **The tuner's three-box calibration session** — v2 dragged grade area + attribute band + remaining
  band and DERIVED `region`/bands/`row_height`. The one-rect kind was withdrawn rather than faked; a
  session-shaped calibration is the only way to keep the derivation.
- **The tooltip's two-capture hover session** — measure the offset by parking the cursor, hovering and
  capturing, then boxing the panel. A single frame mark cannot express it.
- **The pet's capture session** — reference crops, new row/queue entries and food-cell re-marking from
  the live bag (the close-X crop is learned by the run); the Pet tab edits the setup the config already
  carries, not the captures.
- **The environment-check (Detect) surface** — `CalibrationCheck` is ported and tested in the domain,
  and the Calibrate workspace already shows the stored-vs-live environment mismatch for a capture (it
  re-derives the comparison shell-side; `window.measure` supplies the live side). There is no
  canonical protocol command that returns the comparison, and Setup has no Detect surface; until then
  an environment change is handled by the workspace's warning plus a manual recalibration (the ledger
  row is §6 G09).
- **Mini mode / shell layout questions** — the shell has a stub; the ⚑ questions in
  [PLAN-V3-SHELL.md](../plans/PLAN-V3-SHELL.md) still decide the layout.

## 5. Parity gaps from the v2-to-v3 audit (2026-10-28)

The full matrix and evidence live in [records/PARITY-AUDIT-2026-10-28.md](../records/PARITY-AUDIT-2026-10-28.md).
The gaps still to restore, in the audit's priority order:

**Restored since the audit** (verified in the committed shell/core): the Sell grid's **row 1..8**
shortcuts, **Select all**/**Clear** and three-state cap status line; the **over-cap refusal** (A6 —
the deduped, valid selection is compared with the cap and the start is refused before any click); the
**always-on-top pin** with its portable `settings.ui.always_on_top`; the **remembered window
placement/size** (restored after `hello` from the machine `ui` block, with debounced `ui.window`
saves and an off-screen rectangle clamped into the primary work area); the **Spammer card Rotation
picker**; the **last-preset / last-flow deletion guards**; the **Buy row picker** (1-based labels,
0-based stored, legacy out-of-range marked); the **Arduino connection diagnostics** (read-only
`board.details` + the rebuilt Connection/ports/firmware card); the Tuner's **Clean up capture images**
maintenance row (confirmed, deletes only `logs/captures` PNGs, works mid-run); the **Pet tab's
setup/readiness checklists and per-row Boarding now ticks** (increment 1: read-only `pet.preflight` +
`pet.set_boarding`); the **Pet tab's queue add/edit by icon capture and food-icon capture** (increment
2: `pet.capture_icon`); the **Pet tab's per-page bag scans and explicit food-scan apply** (increment
3: `pet.scan_food_page`/`pet.scan_queue_page` + `pet.apply_food_scan`); and the **Pet Feeder's run-state
persistence** (`food_slots_used` and per-row `boarding_running` are written mid-run, so a restart no
longer resumes on stale state). The pin is the portable `settings.ui.always_on_top`; placement is
independent machine state in `ui` and is not the pin.

- **Buy/Sell dry runs** (click nothing; verify targeting) and **Quest Test one loop** - v2's two
  pre-run safety tests; absent in v3 (need run-argument commands).
- **Pet tab session surface (increment 4+, per
  [PLAN-V3-PET-SESSION](../plans/PLAN-V3-PET-SESSION.md))** - the feedable sweep/queue rebuild (the
  setup-so-far/ready-to-run checklists, boarding ticks, queue add/edit by icon capture, food-icon
  capture, the per-page bag scans + explicit food apply and the panel test read are restored). A
  scan-derived queue rebuild remains later.
- **Tuner card live detail** (grade/remaining/attempt/attributes - needs a ToolStatus/protocol
  extension); **Spammer advanced raw-key editor**.
- **Calibration plan additions**: loading the saved overlay PNG when a calibrate tab reopens (the Gem
  Move mode home is decided and shipped — the Gem tab; its movement-editor backend work is §6 G16).
- **Arduino PnP device id in the connection card** - dropped for now by decision 7: v2 showed it
  only as a tooltip, `serialport` does not expose it, and SetupAPI/WMI would be a new dependency for
  no decision. Ask if it is wanted.
- **Named geometry profiles + setup import/export** - a future config-root feature, not a UI patch
  (PLAN-V3-UI-UX §9 Q6, answered 2026-10-29): one geometry per machine with the warn-and-recalibrate
  story stands for now; named profiles and importing/exporting a setup need a core/config-root
  concept before the shell can offer them.

## 4. Release inputs

- **DirectML redistribution is RESOLVED and recorded**: Microsoft's license terms for DirectML,
  clause 1(a), permit distributing the unmodified DLL inside Windows applications; the payload ships
  `docs/THIRD-PARTY-NOTICES.md` and `-Release` still requires the explicit acknowledgement flag.
- **Capture/hide lifecycle is CLOSED**: the runner's `seal/host` channel and the Calibrate
  hide → one frame → capture → show path landed (`CUTOVER.md` decision 8), so calibrating from the UI
  works.
- **Sell-slot selection is CLOSED (v2 parity restored)**: the per-run selection is implemented —
  the Sell tab's 8×8 grid is memory-only, the **Sell card's Start** sends it as `tool.start`'s `slots`
  (PROTOCOL §12.6), and `buy_sell.sell_slots` is never read by a run (migration-compatible field only).
  No open sell-slot work remains.
- **What remains is not a release INPUT**: the player's live acceptance pass (the repo rule: a tool is
  done when it has run) and the publishing steps (merge/tag/GitHub release/root README) listed in
  [CUTOVER.md](../plans/CUTOVER.md)'s publishing checklist.

## 6. Backend work the finished UI needs

The v3 shell UI is finished to the approved design ([spec/UI.md](../spec/UI.md)), including the
narrow (960 px) layout. What is left is **backend** — the Rust core / protocol / config work behind
disabled or absent affordances — plus the small shell wiring a shipped core contract still asks for
(those rows say so explicitly). **P1** = the shipped UI has a dead-ended or absent affordance;
**P2** = it works today but the protocol fact is fragile, duplicated, or re-derived shell-side.
Where a row says "plan", the shape is already written in
[PLAN-V3-CALIBRATION §5.2](../plans/PLAN-V3-CALIBRATION.md) or
[PLAN-V3-PET-SESSION §4–§6](../plans/PLAN-V3-PET-SESSION.md); do not reinvent it here. The Calibrate
"Run session" panel (`v3/shell/lib/ui/tabs/calibration/session_panel.dart`) and its locked tree rows
are the visible scoreboard for G01–G03.

| ID | Waiting UI surface | Backend contract needed | Acceptance (observable) | Pri |
|---|---|---|---|---|
| G01 | Calibrate · tooltip offset session (disabled **Run session**, locked tree row) | **core contract shipped 2026-10-30**: `calibration.tooltip.hover_capture` (focus click on the inert scroll point, hover WITHOUT clicking the item, capture + cached frame), `.save` (offset/size/delay, ≥20 px, one `tooltip` block + rev + event), `.test` (same hover, OCR the configured region, best-effort debug PNG); fixtures + protocol ruling 11; **the Calibrate Tooltip surface driving them is the frontend session's step** | core tests: save block/refusals, missing focus/tooltip named; live acceptance (reads both a pet and a food panel) is the player-run step; surface wiring frontend | P1 |
| G02 | Calibrate · tuner three-box session (disabled Run session; `tuner_region` refused) | **core contract shipped 2026-10-31**: `calibration.tuner.check` (three boxes → bounding region + region-relative bands + measured row pitch + OCR/matched/filter read on the frame) and `.save` (writes `tuner.ocr`, optional `tuner.spring_point`, calibration environment; annotated `calib_tuner.png`; one rev + `config.changed`), domain formula pinned by unit tests, fixtures + PROTOCOL ruling 12; **the Calibrate Tuner surface drawing/driving them is the frontend session's step** | core: geometry/row-height unit tests + save round-trip/artifact bands + refusal tests; live Check OCR-first and drag/save-without-changing-numbers is the player-run step; surface wiring frontend | P1 |
| G03 | Calibrate · pet crops/rows session (disabled Run session) | **core contract shipped 2026-10-31**: `calibration.pet.preview` (pure strip→slot/count/ETA derivation + 64 bag centres), `.save` (marks + empty-slot crops from the frame + annotated `calib_pet.png`, calibration half only, siblings preserved, one rev) and `.test_read` (one live capture, v2 3×/9×/24× ladder, per-slot text/score/count); fixtures + PROTOCOL ruling 13; **the Calibrate Pet surface driving them is the frontend session's step** | core: preview derivation, save round-trip/sibling preservation/crop/annotation/refusals, ladder pixel unit; live Test read on four rows and from-scratch recalibration are the player-run steps; surface wiring frontend | P1 |
| G14 | Pet · feedable sweep + queue rebuild (absent) | `pet.scan_feeding_page` + `pet.scan_apply_queue` (plan §4/§6) | scripted panels classify correctly (a failed read is never `finished`); harvest returns icons for feedable cells only; an empty queue replace is refused; live one-page sweep | P1 |
| G16 | Gem · movement editor (Movement section is a mode dropdown only) | **core contract shipped 2026-10-31**: `calibration.gem.moves.save` validates every leg (exactly two integers, bounded; v2's silent 0 fallback refused naming the route), merges into the existing `gem.movements` so unmentioned routes survive, writes only that block, one rev + `config.changed`; fixture + PROTOCOL row; **the Movement editor sending it is the frontend session's step** | core: partial merge, malformed leg/empty/unknown refusals, one-file write + event; shell wiring frontend | P1 |
| G17 | Gem · route / cycle pointer tests (no actions) | `calibration.gem.test_route` + `calibration.gem.cycle {confirm:true}` (plan §5.2) | the route sends the exact tuned/arduino steps; cycle refuses without `confirm` then performs 21 steps; live both routes + cycle | P1 |
| G19 | Buy/Sell · overlays + test scroll + Setup-so-far (aside is a read-only mark list) | `calibration.buysell.preview`, `calibration.shop.test_scroll`, and `buy_sell.preflight` (modelled on `pet.preflight`); preview/scroll shapes are plan §5.2 | preview equals v2's 64 centres; test scroll clicks the focus point first and refuses >400; preflight returns ok/-- per mark + a ready verdict | P1 |
| G20 | Buy/Sell · dry runs (absent) | `tool.start {..., "dry_run":true}` (or `buy_sell.dry_run`): validates like a run, moves the cursor, never clicks | fake effects show placements/scrolls with zero clicks; wrong-tool arguments are `bad_request`; a normal start is unchanged | P1 |
| G21 | Quest · Test one loop (absent) | `tool.start {"tool":"quest","test":true}` (or `quest.test_loop`) | exactly one loop then `tool.finished`; a normal start is unchanged; a non-quest tool refuses the argument | P1 |
| G04 | Calibrate · reference gallery + saved overlay on reopen (v2-only / not-captured states today) | core draws the annotated rects into artifacts (decided) and `calibration.fetch` returns them for artifact-bearing kinds; reopening a surface loads the saved overlay | tuner/gem save writes a natural-size annotated PNG; fetch echoes it; the gallery says present after a restart; bytes are stable | P2 |
| G05 | Calibrate · derived canvas overlays (only the selected mark is drawn) | `calibration.buysell.preview` / `calibration.pet.preview` (plan §5.2) returns the 64 centres, row dots, slot boxes — never shell arithmetic | a fixture draft returns the exact v2 geometry; a bad draft returns `problem`; the canvas renders the answer | P2 |
| G06 | Calibrate · save compare-and-set (last-write-wins today) | **core contract shipped 2026-10-30**: optional `rev` on `calibration.save`; stale → `conflict` with current rev + touched block (`config.apply` shape), refused before validation/artifact/write; fixture `request.calibration.save.with_rev` + `response.error.calibration_conflict`; **shell compare-and-set wiring is the frontend session's step** | stale refused writing nothing (core test); the shell shows the current rev and refreshes (after frontend wiring) | P2 |
| G07 | Calibrate · environment echo + `measured_at` (shell re-derives it today) | **core contract shipped 2026-10-30**: every save stamps `calibration.measured_at` (ISO-8601 UTC) and echoes the written block as `result.calibration`; save fixtures pin it; **shell reads the echo instead of re-deriving (frontend session)** | the save result carries the environment incl. `measured_at` (core test); shell stops re-deriving (frontend) | P2 |
| G08 | Calibrate · open latency + copied kind table (35 fetches on open) | **core contract shipped 2026-10-30**: `calibration.fetch_all` (one local read, every mark + environment + artifacts) and `calibration.kinds` (core-owned catalog incl. withdrawal reasons); fixtures pin both; **the shell deleting its copied table + fetch loop is the frontend session's step** | adding a core kind appears with no shell edit (after frontend); one request replaces 35 (core test); golden fixtures | P2 |
| G09 | Calibrate/Setup · canonical Detect (shell re-derives the comparison) | **core contract shipped 2026-10-30**: `calibration.check {}` → live/stored/differences/matches/note, using the ported domain comparison; closed game is a result, not an error; fixture `response.ok.calibration_check`; **the Setup/Calibrate surfaces deleting their comparison copy and rendering it are the frontend session's steps** | one command yields nothing-stored / game-closed / matches / differs (core unit tests); surface rendering after frontend | P2 |
| G18 | Gem · check-result diagnostic (absent) | `calibration.gem.check_result` (plan §5.2) | a known crop's verdict/distance match v2 within tolerance; no reference reads `no_reference`; fixture pins the fields | P2 |
| G22 | Pet · ITEM page-up lease (PROTOCOL says the shell puts the page up; no UI code clicks) | either wire the existing `pointer.click` + the calibrated page-tab mark before capture/scan (UI-only), or add a session command holding the input lease; PROTOCOL §4's wording is fixed to today's truth | the requested page is clicked, or the command refuses when it cannot be verified; the wrong-page caveat disappears | P2 |
| G25 | Tuner · OCR fidelity (gates the Tuner live-detail/Last-run acceptance) | no protocol change: fill `ocr.scan`'s `grade`/`remaining`, add v2's grade-colour fallback and the retry/last-retry capture (`tuner.ocr_retries` is currently unused) | archived/v2-recorded tuner frames produce the same grade, remaining and attributes as v2; the retry path is exercised | P2 |

**Counts:** 9 P1 and 9 P2. The additive status-detail set (G10–G12: `state.detail`, `rows[].next_action`,
`tool.finished.summary`/`ended_at`) shipped core-side and was rendered shell-side on 2026-10-30, so it
has left this list; G06–G09's core contracts are shipped and their rows now ask only for the shell
wiring. The three highest-leverage remaining commands are the three session writers that
clear the whole disabled Calibrate panel — `calibration.pet.save` (+ preview/test_read),
`calibration.tuner.check`/`save`, and `calibration.tooltip.hover_capture`/`save`/`test`.

### Decisions that gate rows (player/Lead, not engineering tasks yet)

- **Named geometry profiles vs one geometry + recalibrate**, and **setup import/export**: a config-root
  concept before any UI (§5; PLAN-V3-UI-UX §9 Q6).
- **Frame/capture persistence beyond the core's in-memory four-frame FIFO**, and whether a saved mark
  should persist frame provenance (today it deliberately stores mark + artifact + environment, never
  the frame).
- **Pet sweep protocol**: per-page blocking commands vs a background job/cancel channel (decided:
  blocking until the live pass, PLAN-V3-PET-SESSION §10).
- **Global hotkeys poller** (§1's measurement), **PNG-vs-BGRA frame payload** (PROTOCOL §11 item 3),
  **Arduino PnP id** (§5), **Tuner save-clears-spring-point parity** (PLAN-V3-CALIBRATION §10).
- **Player-run acceptance gates**: the live watch per stage, the OCR parity recordings (tuner
  full-region only today) and the v2-oracle worksheet (§1) gate G02/G03/G25's *acceptance*, not their
  implementation.

The UI-only leftovers are not on this list: copy-as-YAML (clipboard), narrow-width aside stacking, the
Calibrate header alignment and the 1280/960 render sweep are finished in the shell (`spec/UI.md`).

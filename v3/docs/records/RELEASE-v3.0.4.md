# Release v3.0.4 — the Arduino card returns, and the Pet tab can check itself (supersedes v3.0.3)

**Date:** 2026-10-28 · **Branch:** `v3-core` · **Built from:** `7ec3539`

The parity audit's remaining diagnostics gap is closed: the Arduino tab is again v2's Connection card,
but read-only — it enumerates the ports and answers "what would Start do" without opening the board,
so it is safe while a tool runs. The Pet tab gains the first Pet-session increment: the core's
Setup-so-far/Ready-to-run checklists (read-only `pet.preflight`) and the per-row **Boarding now** tick,
which persists through the A1 field-precise writer the instant it is clicked. The builder-check of that
increment found real parity bugs in the first cut (the ready line's format, calibration gates that did
not match Start's, a missing unticked-rows line); they are fixed here, along with the shell/docs
corrections they forced. v3.0.4 supersedes v3.0.0–v3.0.3; nothing has been published.

## Artifacts (built 2026-10-28, not yet published)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.4.zip` | 45,005,039 | `51308D181D1708C11B292A54F750EFB300BBC3EF7FE01C66E27CCF40589385B6` |
| `dist/SealTools-v3.0.4-firmware.zip` | 5,254 | `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B` |

Same one-top-folder payload as before; the stage allow-list audit, the app-zip audit and the firmware
source+zip audit all PASS. **Firmware byte-identical** to v3.0.0–v3.0.3 — no reflash.

## What changed from v3.0.3

- **Arduino connection card, rebuilt on the read-only command** (`ea3ae90`, review follow-up `1f166e0`):
  the tab opens on `board.details` (enumerates serial ports, never opens the board, safe while a tool
  runs) and shows v2's three-state Connection card — ready / named port missing (the core's refusal
  verbatim) / no Arduino found (the charge-only-cable advice) — plus the chosen-first **●/○** port list
  with "matches the expected ID", "What the launcher looks for" (VID/PID/baud/named-port override), and
  the cached **protocol level** (not v2's drag verdict) with **Query firmware** as the only action in the
  tab that opens the port, disabled with the reason while a tool runs. **Refresh** re-reads; Click/Move/
  Send key are unchanged; Setup's **Probe board** remains the explicit open-and-ask. The core command
  shipped in v3.0.3; this release brings the surface. Review fix: `not_found` now renders the core's
  refusal with v2's sentence as a documented fallback, and the additive unknown-`choice.state` decode is
  unit-tested.
- **Pet-session increment 1 — `pet.preflight` (read-only) and `pet.set_boarding`** (`110dcc7` core,
  PROTOCOL `c31bab3`): preflight answers the tab's Setup-so-far/Ready-to-run facts — the calibration
  list, the session list (return slot, food cells marked/used/round, queue or return-slot fallback, food
  load mode, per-row boarding, board informational) and v2's readiness **lines verbatim**; it never
  writes, opens the board, or reads the game (an e2e asserts the config file is byte-unchanged). The
  boarding tick writes exactly one field (`pet.slots[row-1].boarding_running`) through the A1
  field-precise writer, bumps the shared revision, returns `{rev, block}` and emits one
  `config.changed` after the response; bad/out-of-range rows are refused by name and an I/O failure
  answers `io` with the file untouched.
- **Preflight parity fixes from the builder-check** (`17e1ff2`): the ready line is v2's exact format for
  any row count ("…reloading every 205 min / 410 min."); calibration completeness now mirrors Start's
  gates — used page tabs only (with 2-point validation and precise missing-page details), the food strip
  only in drag mode via the run's own `FeederLayout::slot_boxes` derivation, and non-gate facts (row
  boarding slot, empty-slot reference) present but unable to false-block ready; a new "every row is
  unticked — nothing to feed" line; malformed/absent pet config answers facts. Also closes the B3 review's
  silent-board cache bug: a pet run and `board.probe` now agree (silent = level 1) in `board.details`.
- **Shell Pet tab** (`5a14471`): the checklists (✓ passed / ✗ failed / ℹ = the shell's marker for an
  `ok` item the core did not check, e.g. board firmware), the core's lines rendered verbatim, and a
  shell verdict from `ready`; fetched on open and explicit Refresh only, marked stale while the draft is
  dirty ("Based on the saved config — Save to refresh") and re-read after a successful Save or tick.
  Per-row **Boarding now** ticks render from the config mirror (the feeder's own mid-run writes appear),
  send exactly `pet.set_boarding` (no config.apply), are disabled while any edit is unsaved with the two
  reasons, allow one in flight, and render a refusal under the rows. Two gate fixes by the Lead: portable
  edits rebuild the State (stale caption/tick gating are State-built) and a test literal's concrete type.
- **Docs**: both guides (38 = 38 headings) now document the rebuilt Arduino card (including that firmware
  is the cached protocol level, not a drag verdict, and that Query firmware is the only port-opening
  action) and the Pet preflight/ticks with the corrected scope — ready means the core expects Start to
  accept the SAVED setup, a ✗ is not automatically "not ready" (the calibration list mixes gates and
  facts), ℹ is a shell marker not a third core state, and the checklists are not live while editing.
  PROTOCOL gained the two pet command rows and the current implemented-command list; TOOL-FLOWS notes the
  boarding tick is `boarding_running`'s manual writer; OPEN records increment 1 restored, the remaining
  increment-2+ session surface, and the dropped PnP id.

## Gates at this commit

- Rust workspace: **79 result lines, 0 failed** (core lib 158; `pet_session` 13/13), clippy `-D warnings`
  0, `cargo fmt --all -- --check` 0.
- Flutter: **243/243 tests**, `flutter analyze` No issues, `dart format` 88 files clean.
- `flutter build windows --release` 0 (the window-placement host methods from v3.0.3 and this release's
  shell code compile); `publish.ps1 -Release` stage/app-zip/firmware audits PASS; payload hash-match.
- Protocol fixtures: 71 (32 request / 33 response / 6 event), shell test decoders cross-checked with
  0 mismatches.
- Builder-checks: task-103 (B3) and task-109 (Pet increment 1 re-check) — the only code blockers found
  were fixed before this build; the remaining notes are cheap test gaps and hand-edited-config edges
  recorded for the next increments, not release blockers.

## Known scope limits (honest)

- **Preflight mirrors Start's gates for the saved, UI-shaped config.** Hand-edited malformed values
  (negative mark pages, malformed food cells with no page tabs) can still be refused by Start after a
  "Ready to run"; the guides say so.
- **ℹ is a shell text-prefix rendering** of the core's "not checked by preflight" detail (only board
  firmware today), not a structured protocol field; a structured flag is a future cleanup.
- **Tick-while-a-tool-runs** is correct by construction (no arbiter gating; the command takes no
  game/board lease) but has no widget test; the silent-board cache write needs a real board.
- **Not yet in this release:** Pet session increment 2+ (source-cell marks, icon captures, bag scans,
  queue add, panel test read), Tuner card live detail (B5, v3.1), Buy/Sell dry runs and Quest
  Test-one-loop, the spammer raw-key editor, and dry-run/quest command batch — all tracked in OPEN §5.

## Publish notes

`v3/docs/plans/CUTOVER.md` ends with the v3.0.4 publish checklist (merge/tag, release assets, root
README row, plan archival, post-publish download check). The push is the player's call; this release is
local-only until then.

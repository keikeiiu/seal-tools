# Release v3.0.6 — the Pet Feeder's bag scans return (supersedes v3.0.5)

**Date:** 2026-10-28 · **Branch:** `v3-core` · **Built from:** `7d2829b`

v2's last automatic Pet-tab maintenance workflow is back: the **bag scans**. Each ITEM page can be
scanned read-only (one page per command — no sweep) against the saved food icon or the queued pets'
icons, with the core's report lines and best-effort evidence under `logs/reads/`; and a separate,
confirmed **Apply N cells to the food list** action is the only write — it replaces the food-cell list
and resets the used-cell count through the single A1 writer, effective immediately even during a run.
The increment was builder-checked twice; the fixes that landed before this build are the scan rows'
page-up/wrong-page caveat, page-specific evidence names, a cheap food-icon refusal before any capture,
and two docs clarifications (scans read the SAVED config; Apply uses the scanned page, not the chip
selected afterwards). v3.0.6 supersedes v3.0.0–v3.0.5; nothing has been published.

## Artifacts (built 2026-10-28, not yet published)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.6.zip` | 45,073,095 | `21D5339925F01E4617D304C764DA17F3432200A6ED8860E8B983D29C3B1ACEEE` |
| `dist/SealTools-v3.0.6-firmware.zip` | 5,254 | `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B` |

Same one-top-folder payload as before; the stage allow-list audit, the app-zip audit and the firmware
source+zip audit all PASS. **Firmware byte-identical** to v3.0.0–v3.0.5 — no reflash.

## What changed from v3.0.5

- **`pet.scan_food_page {"page"}` / `pet.scan_queue_page {"page"}` — READ-ONLY bag scans** (`253d7db`,
  PROTOCOL `4a0e856`): input lease → bag-grid/page-tab validation → one foreground-gated capture → the
  EXISTING v2 matcher (one unfiltered sweep shared with the run — no second scorer) → best-effort
  evidence under `logs/reads/` (`scan_page{N}_{stamp}.png` plus the page-specific report
  `scan_page{N}_{stamp}.txt`) → result. Food results are 1-based in CELL order with the report lines and
  runner-up; queue results are queue-order `per_entry` rows with hit cells (1-based), scores, runner-up,
  claimed-cell conflicts and v2's deduced "label ×n" lines; no queue icons = empty arrays, not an error.
  They change no config, click nothing and open no board. The shell hides itself and puts the requested
  page up; the core validates the page's 2-point tab mark but does NOT click or verify the displayed
  page, so a wrong page yields a plausible result under the requested page — the docs and UI say so.
- **`pet.apply_food_scan {"cells":[[1-based page,1-based cell],...]}` — the explicit write** (`253d7db`):
  converts to the stored 0-based `pet.food_slots`, replaces them and resets `pet.food_slots_used` to 0
  in ONE A1 field-precise write (never a whole-block apply), returns `{rev, block}` and emits exactly one
  `config.changed` AFTER the response. An EMPTY selection is refused (nothing-found must not clear a
  live run's cells); malformed/out-of-range pairs and missing page-tab marks are `bad_request` naming
  `cells` plus the index; an I/O failure answers `io` with the file untouched and no event.
- **Robustness fixes from the builder-check** (`6eba880`): evidence names now include the page in BOTH
  files so a same-second sweep of ITEM1–3 cannot overwrite a report; `pet.scan_food_page` decodes
  `pet.food_icon_png` BEFORE the capture/evidence step, so a missing/corrupt icon refuses cheaply and
  writes nothing; an apply-specific blocked-`local.yaml` e2e pins io + byte-unchanged file + no event.
- **Shell scan/apply surface** (`771a60a`): the capture flow was generalised into one
  `_runHiddenWindow` (hide → one frame → command → show in a finally) shared by captures and scans, with
  `_windowBusy` making captures/scans/apply mutually exclusive. A Scan-the-bag section (ITEM1–3, one
  command per press, no automatic sweep) renders the page echo, found cells ("Found no food on this
  page." is a SUCCESS), the core's report/evidence verbatim, per-entry hits/scores/runner-up, claimed
  conflicts, deduced lines, and the neutral caption that a `hits: []` pet may be absent OR its stored
  icon may not decode. **Apply** is disabled at 0 cells, while a window action runs, or while the Pet
  draft is dirty ("Save the edits first"), and its confirmation says REPLACES the food-cell list /
  RESETS the used-cell count / applies immediately even during a run / check the report because a wrong
  page gives a plausible wrong list; success shows the rev, refreshes the food list from the core's one
  `config.changed` and re-reads the preflight once. Scans/apply send no `config.apply`/`config.validate`.
  8 new widget tests (35 in the file; suite 264), including order/payloads, a refused scan, the apply
  dialog/Cancel/send/refresh, dirty and empty apply states, a refused apply, and the missing-icon
  caption.
- **Docs** (both guides 38 = 38, TOOL-FLOWS, OPEN §5): the scan/apply workflow with the page-up and
  wrong-page caveats, best-effort evidence, empty-is-success, `hits: []` meaning, Apply's
  replace/reset/immediate/dirty-gated rules, "scans read the SAVED setup" and "Apply uses the scanned
  page, not the chip"; OPEN moves increment 3 to the restored list and leaves the panel test read plus
  the feedable sweep/queue rebuild as increment 4+.

## Gates at this commit

- Rust workspace: **79 result lines, 0 failed** (core lib 176), clippy `-D warnings` 0,
  `cargo fmt --all -- --check` 0.
- Flutter: **264/264 tests**, `flutter analyze` No issues, `dart format` 88 files clean.
- `publish.ps1 -Release`: stage/app-zip/firmware audits PASS; payload hash-match.
- Protocol fixtures: 81 (37 request / 38 response / 6 event), shell test decoders cross-checked with
  0 mismatches.
- Builder-checks: task-123 (core) and task-125 (shell + robustness) — no code blocker; the docs
  corrections they required (`2ec2897`, `7d2829b`) are in this build. The only discrepancy left
  recorded is a commit-message test-count typo (8 new widget tests / 35 in file, not 9/36), corrected
  here in the record.

## Known scope limits (honest)

- **Scans read the SAVED config** (scans are not dirty-gated; only Apply is). Unsaved tab edits are
  ignored by a scan — the docs say to Save first. Apply uses the page named in its confirmation (the
  scanned result's page), not the ITEM chip selected afterwards.
- **The core cannot verify the displayed bag page** or the content of the scored cells; a wrong page
  gives a plausible wrong result. The report/evidence review is the player's step; nothing is applied
  automatically.
- **Evidence is best-effort** (`logs/reads/`): may be absent or partial; report names are page-specific
  now, but the directory being writable is assumed.
- **The queue scan's report body is a file path only** (the UI shows paths; a no-hit entry is ambiguous
  by design and the caption says so).
- **Live-only:** the actual Win32 capture + hidden-launcher foreground/frame timing and real matcher
  scores; the scans have no e2e without a game window.
- **Not in this release:** the panel test read and the feedable sweep/queue rebuild (increment 4+, the
  sweep's long per-page surface decision is still open — per-page blocking commands with progress vs a
  background job), B5 card live detail (v3.1), dry runs / Quest Test-one-loop, the spammer raw-key
  editor — OPEN §5.

## Publish notes

`v3/docs/plans/CUTOVER.md` ends with the v3.0.6 publish checklist (merge/tag, release assets, root
README row, plan archival, post-publish download check). The push is the player's call; this release is
local-only until then.

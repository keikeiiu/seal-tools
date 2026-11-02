# Release v3.0.5 — the Pet Feeder can queue pets again (supersedes v3.0.4)

**Date:** 2026-10-28 · **Branch:** `v3-core` · **Built from:** `ecfbd2e`

The parity audit's "the player cannot queue a new pet at all" gap is closed: the Pet tab now adds a
queue pet by capturing a bag cell's icon (label, species, rect and PNG), edits each entry's label and
species in place, removes entries before saving, and captures the food icon the future food scan will
use as its reference. This is Pet-session increment 2, built as capture-only commands on the run's own
bag-cell geometry: the core takes one foreground-gated shot and crops in memory, never clicking or
moving the cursor, and the shell hides itself for the shot and shows again in a finally path so a
refusal can never leave it hidden. The builder-check found one guard edge (a stale food-icon source
after a draft Cancel) and two docs overclaims; both were fixed before this build. v3.0.5 supersedes
v3.0.0–v3.0.4; nothing has been published.

## Artifacts (built 2026-10-28, not yet published)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.5.zip` | 45,033,252 | `BFB0870C806C1920E6077F8863DC10FD8C8816C1D670B5E445D4AE684A34BE72` |
| `dist/SealTools-v3.0.5-firmware.zip` | 5,254 | `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B` |

Same one-top-folder payload as before; the stage allow-list audit, the app-zip audit and the firmware
source+zip audit all PASS. **Firmware byte-identical** to v3.0.0–v3.0.4 — no reflash.

## What changed from v3.0.4

- **`pet.capture_icon`** (`a10726f`, shared geometry `4d593b8`, PROTOCOL `562a808`/`6748f7b`): the
  Pet-session icon-capture primitive. `{"kind":"queue"|"food","page":<1-based>,"cell":<1-based>}` →
  `{kind,page,cell,rect,png_base64}`; it validates the input lease, the calibrated `pet.bag_grid` and
  the page's 2-point tab mark, takes ONE foreground-gated client capture and crops in memory with the
  run's own `BagGrid` centre/pitch box (now a shared domain helper used by the run and the command).
  Capture-only: no clicks, no cursor moves, no board open, no logs/reads, no config write. Errors keep
  the established codes (`bad_request` naming kind/page/cell/bag_grid, `window_not_found`,
  `game_not_foreground`, `tool_busy` with the holder's line, `internal` for frame/encode bugs). The
  pure crop path is byte-tested (a synthetic frame's pixels decode exactly); the Win32 capture path is
  inspection-only without hardware.
- **Pet tab queue surface** (`39ce9fe`): captured-icon previews on queue entries, editable label (log/
  list only) and species (free-text feeding-table line; blank/unmatched = no minute estimate), Remove;
  **Add a pet by icon** (label/species, ITEM1–3 page chips, single-selection 8×8 source-cell grid,
  preview plus the echoed page/cell/crop size, and the explicit warning that a wrong page gives a
  plausible wrong crop); **Capture food icon** using the newest marked food cell; all still written by
  the tab's ONE `config.apply` — captures never apply config, and `pet.food_icon_png` rides the draft
  to Save. The hide→one-frame→capture→show flow reuses the Calibrate pattern with the show in a
  `finally`; the shell never clicks the ITEM tab and does not verify the page.
- **Builder-check fixes** (`35600cd`, docs `413c4ec`): the food-icon source is now reconciled against
  the current draft marks on every seed revision, so a Cancel/reseed can never leave the capture button
  enabled against an unmarked cell; the docs were corrected to say a captured entry appears in the
  queue list as a DRAFT immediately (only persistence waits for Save), that the food icon is the
  reference a FUTURE food scan will use (the scans are increment 3+, not built), that species seeds from
  the last NON-EMPTY species as free text, and that the capture controls are absent only when the bag
  grid is uncalibrated — with a calibrated grid but an unmarked page tab or a running tool the buttons
  are shown and the CORE refuses.
- **Docs** (both guides, 38 = 38 headings): the queue add/edit-by-icon and food-icon workflows, the
  wrong-page caveat, the one-capture/one-Save rules, and the corrected guard/scan wording; TOOL-FLOWS'
  Pet queue description; OPEN §5 moves increment 2 to the restored list and leaves the bag scans /
  panel test read / feedable sweep as increment 3+.

## Gates at this commit

- Rust workspace: **79 result lines, 0 failed** (core lib 168), clippy `-D warnings` 0,
  `cargo fmt --all -- --check` 0.
- Flutter: **251/251 tests**, `flutter analyze` No issues, `dart format` 88 files clean.
- `flutter build windows --release` 0 (inside the release build); `publish.ps1 -Release` stage/app-zip/
  firmware audits PASS; payload hash-match.
- Protocol fixtures: 75 (34 request / 35 response / 6 event), shell test decoders cross-checked with
  0 mismatches.
- Builder-checks: task-114 (command) and task-117 (shell surface) — no data-corruption blocker; the one
  guard edge and the docs overclaims they found were fixed before this build.

## Known scope limits (honest)

- **The bag/food scans are not built yet** (increment 3+): the captured food icon is the reference a
  future scan will use; nothing in this release scans the bag.
- **The core cannot verify the displayed bag page** — it validates that the requested page's tab mark
  exists but never clicks it, so a wrong page gives a plausible wrong crop; the preview and echoed
  page/cell/size are the only checks and are the player's judgement.
- **Wrong-page/foreground/capture timing is live-only**; the pure crop is byte-tested but the Win32
  capture path needs the game. The input lease is a per-command snapshot (session-wide holding is a
  later increment).
- **Species has no v2 CSV picker/validation** — free text; unknown/empty simply has no minute estimate.
- **Not in this release:** the bag scans / panel test read / feedable sweep (increment 3+), Tuner card
  live detail (v3.1), Buy/Sell dry runs and Quest Test-one-loop, the spammer raw-key editor — OPEN §5.

## Publish notes

`v3/docs/plans/CUTOVER.md` ends with the v3.0.5 publish checklist (merge/tag, release assets, root
README row, plan archival, post-publish download check). The push is the player's call; this release is
local-only until then.

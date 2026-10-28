# Release v3.0.3 — parity restored, calibration foundation, pet safety (supersedes v3.0.2)

**Date:** 2026-10-28 · **Branch:** `v3-core` · **Built from:** `898f861`

The third live pass closed the loop the first two opened. The player's report that the Sell grid had
lost v2 controls ("for the sell grid, there are still features you missed from the original v2") led to
a full v2→v3 parity audit, and v3.0.3 fixes the safety and control gaps it found: per-run Sell
selection with v2's row/all/clear controls and a deduped over-cap refusal; a configurable
always-on-top window plus remembered placement/size; deletion guards, the Spammer card rotation picker
and the Buy row labelled picker; and — the audit's highest-priority safety gap — the Pet Feeder now
persists its run state mid-run, so a restart no longer resumes aiming at used food cells. It also
carries calibration increment 0 (the pointer/artifact foundation every Calibrate session needs), the
Tuner's clean-up-captures command and button, and the read-only `board.details` command behind the
next Arduino-tab rebuild. v3.0.3 supersedes v3.0.0, v3.0.1 and v3.0.2; nothing has been published.

## Artifacts (built 2026-10-28, not yet published)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.3.zip` | 44,949,054 | `C19B85ABD084A2B85F25151B8C63C2C694DA131874E217467C749BEC441EE4F5` |
| `dist/SealTools-v3.0.3-firmware.zip` | 5,254 | `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B` |

Same one-top-folder payload as before (Flutter release bundle incl. `native_assets.json`,
`seal-core.exe`, `DirectML.dll`, three OCR models, config templates only, docs incl. EN + zh-TW
guides, INSTALL, third-party notices). **Firmware byte-identical** to v3.0.0/3.0.1/3.0.2 — no reflash.
The stage allow-list audit, the zip-entry audit and the firmware source+zip audit all PASS; the
firmware zip checksum matches the previous releases exactly. (The v3.0.2 record's firmware hash had a
transcription typo — corrected there with a note; the artifact was always this byte-identical file.)

## What changed from v3.0.2

- **Sell grid, v2 parity** (`0deda95`): per-row `row 1..8` shortcuts (a full row clears, a partial row
  is completed), **Select all**/**Clear**, a three-state cap status line (none / within / over, the
  over-cap warning in the error colour, live on cap edits) and "Bag slot N" tooltips. Selection stays
  per-run and memory-only; the form/Save machinery is unchanged.
- **Over-cap sell is refused, not truncated** (`66d0f22`): the DEDUPED valid selection (0..63,
  highest-first) is compared with `sell_cap`; over cap the run finishes with
  "{n} slots selected but the per-run cap is {cap}. Raise the cap on purpose, or narrow the
  selection." before any click. v2's parity intent, now with the right count.
- **Always-on-top** (`b59ef12`, `b0af630`): the player asked for v2's always-in-front launcher as a
  new config. It is the portable `settings.ui.always_on_top` bool, applied once after `hello` and
  toggled by the app-bar pin; the Windows runner does SetWindowPos(HWND_TOPMOST) without disturbing
  the capture hide/show. Accepting the setting exposed a latent projection gap: `settings.ui` was
  silently dropped by the core, so the pin would have looked saved and vanished — fixed by making
  `ui` a real post-template portable section (one writable leaf).
- **Parity batch** (`93d1099`): v2's deletion guards with its verbatim lines (last spammer preset,
  only quest flow), the Spammer card **Rotation** picker (saves `spammer.active`; Start remains
  `{"tool":"spammer"}`), and the Buy **Row 1..Row N** labelled picker (1-based labels, 0-based stored
  index, legacy out-of-range row marked and warned — the off-by-one cannot be typed).
- **Pet run-state persistence (A1 safety)** (`b6394e2`, PROTOCOL `728a15a`): the feeder persists
  `pet.food_slots_used`, per-row `pet.slots[i].boarding_running` (and the close-guard fields, whose
  writer is ready but has no producer yet) mid-run through the config store's single writer, bumping
  the session revision and emitting one `config.changed {block:"pet"}` with no request/response. A
  failed write logs and never fails the run; a stale shell rev conflicts; a restart now resumes the
  cells and boarding state actually reached. (Close-guard fields await the close-probe port — OPEN.)
- **Window placement/size** (`7dc70ea`): restored after `hello` from the machine `ui` block
  (`expanded_height` is the restore height), applied through new `seal/host` methods (DPI conversion
  only in the runner), clamped into the primary work area when the saved monitor is gone with a log
  line; player moves/resizes save via the existing `ui.window` command with a ~600 ms debounce and
  never while the capture has the window hidden. `ui.pinned` rides through unchanged; it is not the
  app-bar pin. Known limits: no clean-shutdown hook (a last-moment move can be lost), maximized state
  not remembered, mixed-DPI clamping approximate.
- **Calibration increment 0** (`1485638`, `6164497`, plan `d7e477e` for the next plan): core
  `pointer.place`/`pointer.click` (flat x/y integers >= 0, closed-loop board placement, never
  SetCursorPos, per-command input lease with `tool_busy` naming the holder — PROTOCOL §12.9) and the
  PNG annotation writer for the session increments' artifacts; the shell `CalibrationSession` draft and
  pointer client actions. No player-visible Calibrate surface yet: increment 1 (Setup Detect) is next.
- **Tuner clean-up captures** (`27b9f56` core, `10ed375` shell): the explicit
  `logs.cleanup_captures` command (PROTOCOL §2 exception; only direct regular `*.png` under
  `logs/captures`, missing dir = 0, failures logged) and v2's maintenance button with confirmation,
  count+dir result line, and no tool-busy gate (housekeeping opens no game/board).
- **Read-only `board.details`** (`82d640c`, PROTOCOL `e719e4a`): enumerates ports and answers what a
  Start would use (expected ids/port, chosen-first port list, ready/named_missing/not_found with the
  core's refusal sentence, memory-only firmware cache) without ever opening the port. The Arduino-tab
  rebuild that consumes it is v3.0.4; B5 card live detail is v3.1 (`PLAN-V3-STATUS-DETAIL.md`).
- **The parity audit itself** (`5ff5b4f`, `records/PARITY-AUDIT-2026-10-28.md`, OPEN §5): a
  surface-by-surface v2→v3 matrix with evidence, which drives the v3.0.4/v3.1 batches still listed in
  OPEN (Buy/Sell dry runs, Quest Test-one-loop, the Pet tab session surface, card live detail,
  Arduino diagnostics, spammer raw-key editor, calibration-plan additions).
- **Docs**: both guides (38 = 38 headings) and INSTALL/OPEN/CONFIG now describe the per-run Sell
  controls and over-cap refusal, the pin and its config field, the guards/pickers, the remembered
  placement with its honest limits, pet run-state persistence, and the Tuner cleanup. PROTOCOL gained
  pointer commands/ruling §12.9, the mid-run run-state write contract, `logs.cleanup_captures` and
  `board.details`; TOOL-FLOWS' Sell flow now mirrors the deduped over-cap rule.

## Gates at this commit

- Rust workspace: **79 result lines, 0 failed** (core lib 143), `cargo clippy --workspace
  --all-targets -- -D warnings` 0, `cargo fmt --all -- --check` 0.
- Flutter: **223/223 tests**, `flutter analyze` No issues, `dart format` 85 files clean.
- `flutter build windows --release` 0 (the new C++ host methods compile).
- `publish.ps1 -Selftest`: 26/26 checks; `-Release` stage/zip/firmware audits PASS, payload
  hash-match verified.
- Protocol fixtures: 67 files; the shell's fixture cross-check is 0 mismatches.

## Publish notes

`v3/docs/plans/CUTOVER.md` ends with the v3.0.3 publish checklist (merge/tag, release assets, root
README row, plan archival, post-publish download check). The push is the player's call; this release is
local-only until then.

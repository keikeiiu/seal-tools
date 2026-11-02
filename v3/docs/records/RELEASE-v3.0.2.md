# Release v3.0.2 — per-run Sell selection (supersedes v3.0.1)

**Date:** 2026-10-28 · **Branch:** `v3-core` · **Built from:** `1169c0d`

v3.0.1 shipped a Sell grid whose selection was **persisted** into `buy_sell.sell_slots`. The player
corrected the design in the second live pass: *"sell selection could be different every sell."* A
stored selection can silently sell the slots chosen last time — data loss, not convenience — so v3
now keeps v2's per-run behaviour. v3.0.2 is the release to publish; it supersedes v3.0.1 (and v3.0.0).
Nothing has been published.

## Artifacts (built 2026-10-28, not yet published)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.2.zip` | 44,838,165 | `7C8D79600D0DE9349B1DF00E18789A91438AB57A77E2E5EE7CD30364BB6A3084` |
| `dist/SealTools-v3.0.2-firmware.zip` | 5,254 | `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B` |

*(Corrected 2026-10-28: the firmware row above previously had a transcription typo in its SHA-256 (a missing `CC`); the artifact is byte-identical to v3.0.0/v3.0.1 and its real hash is shown now.)*

Same one-top-folder payload as before (29 entries: Flutter release bundle incl `native_assets.json`,
`seal-core.exe`, `DirectML.dll`, three OCR models, config templates only, docs incl EN + zh-TW
guides, INSTALL, third-party notices, `pet-data.csv`). **Firmware byte-identical** — no reflash.
All three publish audits passed and every checked payload file hash-matches its source.

## What changed from v3.0.1

- **PROTOCOL (ruling §12.6 + §4):** `tool.start` takes `{"tool":"sell","slots":[...]}` — the run's
  selection. `slots` is accepted only by `sell`, must be a non-empty array of integers 0..63 (wrong
  shapes are `bad_request` with `detail.field:"slots"`), is used for that run only and never written.
  A start with no `slots` has **no selection** and answers the tool's own
  `No slots are selected to sell.`; `buy_sell.sell_slots` is never read by a run (it stays in the
  typed config for migration compatibility, with no writer).
- **Core:** validated argument forwarding before the supervisor; `sell_plan_with_slots` replaces
  `ShopSettings.sell_slots` for that plan only; the supervisor's availability probe is
  calibration-only so per-run slots can arrive; the last config-fallback path removed.
- **Shell:** the 8×8 Bag grid keeps its look, count/pitch header and Bag-grid gating, but the
  selection is process-memory view state on `ShellModel` (survives tab switches, starts empty each
  launch, never clears on Start). The Save-selection button and its draft/config path are gone; the
  **Sell card's Start** sends the selection, or plain `{"tool":"sell"}` when empty.
- **Docs:** both guides (38 = 38 headings), INSTALL, OPEN ("CLOSED — v2 parity restored"),
  TOOL-FLOWS Sell flow and CONFIG wording now describe the per-run selection and never claim a saved
  selection or a config fallback.
- Product version **3.0.2**; protocol client identity stays `0.1.0`.

## Verified before packaging

| gate | result |
|---|---|
| `cargo test --workspace` | **79 result lines, 0 failed** |
| clippy `-D warnings` / `cargo fmt --check` | 0 / clean |
| Flutter `analyze` / `test` / `format` | No issues / **180/180** / 81 files clean |
| `publish.ps1 -Selftest` / `-Verify -Version v3.0.2` | exit 0 / exit 0 |
| `publish.ps1 -Release …` | exit 0; stage, app-zip and firmware audits PASS; hashes above |
| payload integrity | 29 entries, one top folder, no player state; every checked payload file hash-matches |
| launch | staged build starts with a fresh migrated v2 config (shell + core, window "Seal Tools v3") |

## Not in this release

The calibration workstream is still planned, not built
(`v3/docs/plans/PLAN-V3-CALIBRATION.md`, 8 increments); the second live pass on v3.0.2 and the player's
recorded v2 parity worksheet are the next acceptance steps. No global hotkey poller, no pet capture
session, no tuner colour fallback, no OCR retries, no single-instance lock, no installer — all named
in `OPEN.md`. DirectML ships unmodified under Microsoft's licence terms (notices in the zip).

## Publishing

Merge `v3-core` → `main`, tag **v3.0.2**, attach `SealTools-v3.0.2.zip` +
`SealTools-v3.0.2-firmware.zip`, use this record as the release body, add the v3.0.2 row to the root
README, archive the plans. Exact steps and hashes: `v3/docs/plans/CUTOVER.md`.

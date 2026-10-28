# Release v3.0.1 — the first live-pass fix release

**Date:** 2026-10-28 · **Branch:** `v3-core` · **Built from:** `6f3ae99`

v3.0.0 was code-complete and packaged, then the player's first live pass found real defects — the
worst of them a migrated v2 config that v3 refused to run. v3.0.1 fixes them at source and is the
release to publish. It supersedes `v3.0.0`; nothing from either was ever published.

## Artifacts (built 2026-10-28, not yet published)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.1.zip` | 44,841,808 | `85AAAF9833E53D1AC7E9894E09C4556B2CB514BAA4D5F537BF15BF43AFB08961` |
| `dist/SealTools-v3.0.1-firmware.zip` | 5,254 | `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B` |

The app zip is the same one-top-folder payload as v3.0.0 (29 entries): the Flutter release bundle
(`SealTools.exe`, `data/`, `flutter_windows.dll`, `native_assets.json`), `seal-core.exe`,
`DirectML.dll`, the three OCR models, `config/` templates only, and `docs/` (guides EN + zh-TW,
INSTALL, third-party notices, `pet-data.csv`). **Firmware is byte-identical** (the sketch has zero
commits since `main`, so no reflash). All three publish audits passed and every payload file
hash-matches its source.

**Superseded for the Sell selection:** this release persisted the Sell grid selection into
`buy_sell.sell_slots`; the player then clarified that the selection changes every sell, and v3.0.2
replaced that with a per-run `tool.start` argument (memory-only UI). Everything else in this record
still describes v3.0.2's behaviour; v3.0.2 is the release to publish.

## What the live pass found, and what changed

1. **"Quest cannot select the flow/sequence; hand-in doesn't work with my old config."** Root cause:
   v2 wrote quest key steps as `value: 1` (an integer); v3's typed `QuestStep.value` expected a
   string, so the whole quest block failed to decode and every quest save/run was refused. The scalar
   decoder now accepts string/number/bool (normalised to string) and null, the save path is covered,
   and the full typed config was audited against the player's actual file: that was the only
   decode-class mismatch. A probe on a copy of their config now answers
   `config.validate {quest:{active:god8}}` with `errors: []`; committed **sanitised v2-era replay
   fixtures** pin typed decode and the real reader paths (tuner geometry, pet slots/queue/food cells,
   buy/sell geometry, gem, spammer). Every merged pointer matched the v2 file.
2. **"Why does the tuner have add-grade? The grades are fixed; the rules/attrs should be
   selectable."** Tuner: the grade ladder is read-only (from `tuner.grade_order`), target/require are
   selected from it, and filter rule names are now a dropdown fed by the loaded attribute dictionary
   (stored names outside it stay selected; Add is disabled with the reason while it loads/fails).
   Gem: the grade ladder is fixed/read-only with Start grade selected from it.
3. **"Setup — should the window name be editable? I think not."** The game window title is now
   read-only; **Find window** detects it and a new **Use detected title** action saves it through the
   normal config path.
4. **"The tuner OCR doesn't use my geometry" (found by the config audit).** `tuner.ocr.grade_area`
   was decoded but ignored. Proved from v2 source that it is **region-relative inside the OCR
   region**; the reader now selects the grade line from it (half-open) with the `grade_y` band as
   fallback. Honest gap recorded: v2 also used the box for its grade-**colour** fallback, which is not
   ported (OPEN).
5. **"The sell grid isn't finished."** The 8×8 bag-cell grid is selectable and **Save selection**
   writes `buy_sell.sell_slots` through validate/apply (machine block, persisted — a deliberate
   replacement of v2's memory-only selection, recorded in the calibration plan). Uncalibrated bag
   grid disables it with the core's sentence; the run sells highest slot first under the cap.
6. **"The Pet tab has no setup."** The machine half is editable again: rows enabled/stacks, queue
   removal, per-page 8×8 food-cell grid, return slot, food-load mode, reload-on-start and the timing
   thresholds — one Save = one `config.apply` carrying the machine and portable halves, with
   capture-derived crops/rects preserved byte-for-byte and `food_slots_used` left as the run reading.
   The refusal strings that still claimed the tab was read-only were rewritten to point at it (the
   genuine exceptions — re-capturing crops, adding rows/queue entries — say so honestly).
7. **Docs** (EN + zh-TW + INSTALL) were rewritten to match all of the above, heading parity 38 = 38,
   links intact; `CONFIG.md`/`OPEN.md`/`TOOL-FLOWS.md` updated; the product version is **3.0.1**
   (protocol client identity stays 0.1.0).

## Verified before packaging

| gate | result |
|---|---|
| `cargo test --workspace` | **79 result lines, 0 failed** |
| `cargo clippy --workspace --all-targets -- -D warnings` / `cargo fmt --check` | 0 / clean |
| Flutter `analyze` / `test` / `format` | No issues / **181/181** / 81 files clean |
| `v3/publish.ps1 -Selftest` | exit 0 (26/26 guards) |
| `v3/publish.ps1 -Verify -Version v3.0.1 -ModelsDir v2\models` | exit 0, "All release sources are present." |
| `v3/publish.ps1 -Release …` | exit 0; stage + app-zip + firmware audits PASS; hashes above |
| payload integrity | 29 entries, one top folder, no player state; every checked payload file hash-matches its source |
| v2 config acceptance | `config.validate {quest:{active:god8}}` on a copy of the player's migrated config → `{"errors":[],"warnings":[]}`; merged pointers identical to the v2 file |
| launch | staged build starts (shell + core, window "Seal Tools v3"); clean-unzip install in a temp folder starts and seeds `local.yaml` |

## Not verified / still not built

- **The second live pass** on v3.0.1 (the acceptance for the fixes above) is pending; the player's
  recorded **v2 parity worksheet** (Detect lines, Check OCR, result-colour verdicts, Show-64-centres,
  Pet/Tooltip Test read) is also outstanding and is the oracle for the calibration work.
- **Full v2 calibration surfaces** are not in this release: the four Calibrate sessions, Setup Detect
  and annotated artifacts are planned in `v3/docs/plans/PLAN-V3-CALIBRATION.md` (8 increments,
  starting after this release reaches the player).
- No global hotkey poller (v3.0 limitation), no pet capture session (rows/queue add, crop re-capture),
  no tuner colour fallback, no OCR retries, no single-instance lock, no installer/auto-update — all
  named in `OPEN.md`.
- DirectML.dll ships unmodified under Microsoft's licence terms; the notices are in the zip.

## Publishing

v3.0.1 is the release to publish: merge `v3-core` → `main`, tag **v3.0.1**, attach
`SealTools-v3.0.1.zip` + `SealTools-v3.0.1-firmware.zip`, use this record as the release body, and add
the v3.0.1 row to the root README. The full checklist (including the post-publish hash check) is in
`v3/docs/plans/CUTOVER.md`.

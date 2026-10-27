# Release v3.0 — the Rust core + Flutter shell

**Date:** 2026-10-27 · **Branch:** `v3-core` · **Built from:** `41faf1a` plus this record's commit

The first release of the v3 shape: a headless Rust core (`seal-core.exe`) and a Flutter Windows
shell (`SealTools.exe`) that spawns it, talking the protocol in
[../spec/PROTOCOL.md](../spec/PROTOCOL.md). No `.NET` runtime, no installer — one folder, unzip and
run. **The firmware is unchanged: no reflash is needed from v2.15+.**

## Artifacts (built 2026-10-27, not yet published)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.zip` | 44,811,539 | `990C262906966DBE86AB1FF2667063492A00FF1C715B57DE1A1CD1E7995F483A` |
| `dist/SealTools-v3.0-firmware.zip` | 5,254 | `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B` |

**Re-cut (2026-10-27).** These are the hashes of the SECOND cut. The first cut
(`B38E673C...C9CA`, 44,808,739 B) was superseded the same day by the pre-publish conformance audit:
it found shipped strings and shipped-guide sentences that told the player to use calibration tabs and
a Pet-tab editor v3 does not have, plus contract-doc contradictions and three commands with no golden
fixture. Those were fixed at source (core messages, guides EN+zh, PROTOCOL/CALIBRATION/OPEN,
TOOL-FLOWS, fixture coverage), the tests were re-run, and the zips re-cut. The firmware zip is
byte-identical across both cuts (`498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B`) because the sketch never changed. Nothing was published
from either cut; the re-cut is the one to ship.

The app zip holds one top folder `SealTools-v3.0/`: `SealTools.exe` + `data/` + `flutter_windows.dll`
(the Flutter release bundle), `seal-core.exe`, `DirectML.dll`, `models/` (three PP-OCRv4 `.onnx`),
`config/` templates only (`defaults.yaml.example`, `local.yaml.example`, `attributes.yaml`,
`attributes.us.yaml.example` — **never a real `local.yaml`**), and `docs/` (user guide EN + zh-TW,
INSTALL, third-party notices, `pet-data.csv`). The firmware zip holds the unchanged sketch under
`seal_mouse/`. Both zips passed the publish script's allow-list and post-zip audits; the audit output
and the exact staged file list are in the build log.

## What v3.0 is

- **All eight tools live**: Skill Spammer, Magic Tuner, Buy (per-run preset/count picker),
  Sell, Gem Composer, Quest, Pet Feeder (resident), and the Hold Space toggle. Each has its decision
  half in `seal-domain` and a live effect loop; every refusal is the tool's own sentence.
- **The core owns the machine**: config (three-layer, fold-once, foreign blocks preserved, atomic
  write, rev/conflict), the board (serial, closed-loop placement, firmware level), window/capture,
  OCR (ONNX Runtime + DirectML), calibration artifacts, per-run file logs, and the supervisor/arbiter
  with the resident-feeder hand-over.
- **The shell is a protocol client**: 12 surfaces (Cards + 11 tabs), config-draft machinery, live
  card message/countdown/rows, calibration with 18 marks and named-instance targets, a logs panel and
  crash view, and the capture/hide lifecycle that makes calibrating from the UI possible.
- **OCR model resolution** needs no environment: `SEAL_OCR_MODELS` when set (wins or fails loudly),
  then `<root>/models`, then the folder beside `seal-core.exe`; a release install uses the last.
- **File logs** under `<root>/logs/`: per-tool `<tool>.log`, `holdspace.log`, `error.log`,
  `ocr_log.jsonl`, optional `ocr_errors.jsonl` and captures. Logging never fails a command or a run.
- **Replay core** (`seal-core --replay script.json`) for development and golden tests, with no
  config/board/window/OCR.

## Verified before packaging

| gate | result |
|---|---|
| `cargo test --workspace` | 76 result lines, all green (core lib 120, calibration 19, replay 10, supervisor 19, logs 15, tools e2e 11, domain gem 26, seal-ocr suites incl. the model-gated run with `SEAL_OCR_MODELS=v2\models`) |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `cargo fmt --all -- --check` | clean |
| Flutter `analyze` / `test` / `format` | No issues / 165 tests / 81 files clean |
| `flutter build windows --debug` and `--release` | built; release renamed `SealTools.exe` |
| `cargo test -p seal-proto` (golden fixtures) | 13 tests green; **65** fixture files (29 request / 30 response / 6 event) with 0 shell-guard mismatches |
| `v3/publish.ps1 -Selftest` | exit 0, 26/26 (version-schema, deny, hidden/reparse, post-zip, firmware guards) |
| `v3/publish.ps1 -Verify -ModelsDir v2\models` | exit 0 "All release sources are present." |
| `v3/publish.ps1 -Release …` | exit 0; stage audit, app-zip audit, firmware audit all PASS; SHA-256 above |
| staged release LAUNCHED | shell resolved `seal-core.exe` beside itself **with no environment**; the core seeded `config/local.yaml` from `local.yaml.example`; window came up as "Seal Tools v3" (core process observed) |
| **release ZIP clean-unzip launch** | `SealTools-v3.0.zip` extracted to a fresh temp folder and launched there: the extracted `SealTools.exe` spawned the extracted `seal-core.exe` (paths observed), seeded a fresh `config/local.yaml` (3,210 B from the template) and opened the window — i.e. the artifact works as an install, not just as the staging folder; the temp copy was then closed and removed |

## Artifact integrity (checked after the zips were built)

- **Structure:** the app zip has 29 entries, exactly ONE top folder (`SealTools-v3.0/`; the re-cut bundle carries the Flutter `native_assets.json` manifest beside the exe), and zero
  forbidden entries (no `local.yaml`, no `logs/`, no `*.backup-*`, no `calib_*.png`).
- **Payload equals sources:** extracting and hashing the zip's `docs/USER_GUIDE.md`,
  `docs/USER_GUIDE.zh-TW.md`, `docs/INSTALL.md`, `docs/THIRD-PARTY-NOTICES.md`, `docs/pet-data.csv`,
  `seal-core.exe`, `DirectML.dll` and a model file against their repo sources gives identical
  SHA-256s — the zip carries exactly the committed bytes, not a stale copy.
- **Firmware unchanged:** the firmware zip contains only `seal_mouse/seal_mouse.ino`, whose SHA-256
  (`AA6D71A3…8940`) is byte-identical to the repo sketch, and `arduino/` has **zero commits** on
  `v3-core` since `main` — the "no reflash" claim is structural, not asserted.
- **The packaged binary works:** the zip's own `seal-core.exe` ran the committed replay script
  (`v3/protocol/replay/smoke.json`) and produced the scripted hello / config.reload / pet start /
  state+finished / shutdown sequence, exit 0.
- **Gates re-run at the recorded commit (`680199f`):** Rust workspace 76 result lines, 0 failed,
  clippy `-D warnings` 0, fmt clean; Flutter analyze No issues, 165/165 tests, format 81 files clean.

## Pre-publish conformance audit (2026-10-27)

An independent read-only audit (the plan's builder-check role) compared PROTOCOL, CALIBRATION, OPEN,
TOOL-FLOWS, the two guides and INSTALL against the code and the built payload. It found no packaging
drift -- the zip's docs were byte-identical to the repo -- but it found **truth defects**, and they
were fixed rather than shipped:

- **CONTRADICTION, shipped binary:** refusal messages sent the player to "Calibrate Gem/Tuner/Pet",
  "Calibrate Buy/Sell" and a "Save Composer Moves" button; the Pet-tab messages told the player to
  tick rows and edit food load on a tab that is read-only in v3.0. Every message now names the MARK to
  save on the single **Calibrate** tab, or the config key to edit in `config/local.yaml`, and says the
  editor it replaces is not built.
- **CONTRADICTION, shipped guides:** a quoted phantom refusal, Hold Space release failure claimed on
  the card (it is `holdspace.log`-only), a shell-wide Start disable that does not exist, an "installer
  copied a seed" claim with no installer, a capture table that merged two different files, a
  `model_load_failed:<file>` reason that does not exist, repository-only paths implied to ship in the
  zip, a missing `THIRD-PARTY-NOTICES.md` in INSTALL's tree, a consumed-vs-harmless `calib_*.png`
  distinction, a read-only Pet tab that the workflow told the player to edit, unused OCR retries
  presented as working, Settings-vs-Setup, and zh-TW boarding terminology. All corrected in EN and
  zh-TW (heading parity 38 = 38) and in INSTALL.
- **Contract docs:** PROTOCOL's model-failure reasons, `io` meaning, frame-cache "not built" line,
  `fatal` producer, §11 hotkey item, §12.7 pending-plan sentence, §7 writers/`config.changed`,
  §4 resident/deferred `tool.start` rules and a single-instance pointer; CALIBRATION §8 vs §6 on
  display-scale changes; OPEN's stale release-inputs section and the missing Detect gap; TOOL-FLOWS'
  quoted refusals.
- **Golden fixtures (T1):** `keys.parse`, `keys.names`, `attributes.get` had no fixture, and both
  calibration exemplars used a kind the core refuses (`bag`). The generator now covers all 29 §4
  commands and the accepted kinds: **65 fixtures** (29 request / 30 response / 6 event, 0 shell-guard
  mismatches), with the shell's test-only decoders extended to match.

Final gates for the re-cut: Rust workspace **76 result lines, 0 failed**, clippy `-D warnings` 0, fmt
clean; Flutter analyze **No issues**, **165/165** tests, format 81 files clean; `-Selftest` 26/26,
`-Verify` exit 0, `-Release` exit 0 with all three audits PASS; payload hash-match, packaged-core
replay smoke, and a clean-unzip launch re-checked against the new artifact.

## Not verified / deliberately not in v3.0

- **No live tool run against the game yet** in v3 — the repo's rule is that a tool is done when it
  has run, so this release is code-complete and packaging-complete but awaits the player's live pass
  (one run per card and one calibrator pass from the UI). That is the one item this record cannot
  claim.
- **No global hotkeys**: the config stores VK codes and the Hotkeys tab edits names, but no poller
  exists in v3.0; in-game F12/F11-style hotkeys do not fire. Stop and the shell controls always work.
  PLAN §6.5 makes the hotkey path a measurement first; OPEN carries it.
- **No tooltip two-capture session, no pet crop/cell editor, no sell-slot selection editor, no tuner
  three-box session, no Detect surface, no single-instance guard, no installer/auto-update.** All are
  named in [../backlog/OPEN.md](../backlog/OPEN.md); the guides say so in the "not built" sections.
- **OCR parity beyond the archived corpus** still needs the player's raw-crop recordings (PLAN
  §13.10); the ten live captures used for the pipeline are the measured base.
- **DirectML.dll** ships unmodified under the Microsoft Software License Terms for DirectML (clause
  1(a) permits distribution in Windows applications; the other clauses apply). The notices are in the
  zip at `docs/THIRD-PARTY-NOTICES.md`; this is a licence statement, not legal advice.

## Upgrading from v2

Copy `config/local.yaml`, the `calib_*.png` files beside it, and the attribute files into the v3
folder's `config/`. Formats are unchanged. Then run `SealTools.exe` — the core seeds anything
missing on first launch. Uninstalling is deleting the folder.

## Publishing (player's step)

The zips are built locally on `v3-core`; they are **not** published. Publishing is deliberately not
done here: merge `v3-core` → `main`, tag `v3.0`, attach the two zips to a GitHub release, add the
v3.0 row to the root README's version table, and move the plans to the archive per the repo rule.
Once published, this record becomes the release's dated rationale and the root README becomes the
download surface.

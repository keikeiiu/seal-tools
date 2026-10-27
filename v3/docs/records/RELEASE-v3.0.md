# Release v3.0 — the Rust core + Flutter shell

**Date:** 2026-10-27 · **Branch:** `v3-core` · **Built from:** `41faf1a` plus this record's commit

The first release of the v3 shape: a headless Rust core (`seal-core.exe`) and a Flutter Windows
shell (`SealTools.exe`) that spawns it, talking the protocol in
[../spec/PROTOCOL.md](../spec/PROTOCOL.md). No `.NET` runtime, no installer — one folder, unzip and
run. **The firmware is unchanged: no reflash is needed from v2.15+.**

## Artifacts (built 2026-10-27, not yet published)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.zip` | 44,808,739 | `B38E673C0CA75892C30AAD6239222582A88ADDDCD58C86CDAD50EC59F529C9CA` |
| `dist/SealTools-v3.0-firmware.zip` | 5,254 | `498E6A4BA1F8552C2BBE8B828AE892B338CC675BA5016CACE0089A3C35CCEF3B` |

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
| `v3/publish.ps1 -Selftest` | exit 0, 26/26 (version-schema, deny, hidden/reparse, post-zip, firmware guards) |
| `v3/publish.ps1 -Verify -ModelsDir v2\models` | exit 0 "All release sources are present." |
| `v3/publish.ps1 -Release …` | exit 0; stage audit, app-zip audit, firmware audit all PASS; SHA-256 above |
| staged release LAUNCHED | shell resolved `seal-core.exe` beside itself **with no environment**; the core seeded `config/local.yaml` from `local.yaml.example`; window came up as "Seal Tools v3" (core process observed) |
| **release ZIP clean-unzip launch** | `SealTools-v3.0.zip` extracted to a fresh temp folder and launched there: the extracted `SealTools.exe` spawned the extracted `seal-core.exe` (paths observed), seeded a fresh `config/local.yaml` (3,210 B from the template) and opened the window — i.e. the artifact works as an install, not just as the staging folder; the temp copy was then closed and removed |

## Artifact integrity (checked after the zips were built)

- **Structure:** the app zip has 28 entries, exactly ONE top folder (`SealTools-v3.0/`), and zero
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

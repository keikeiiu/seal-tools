# Release v3.0.11 - PP-OCRv6 trial and a readable Tuner result bar

**Date:** 2026-11-11 - **Branch:** `main` - **Built from:** `e58cb5220e31c3b92345ae239ee50d66fc4fbe13`

v3.0.11 makes the PP-OCRv6 small trial a real Setup option and finishes the Tuner mini bar so its OCR
result can actually be read during a run. v4 remains the default; v6 is opt-in.

## Artifacts (built 2026-11-11, published as GitHub release assets)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.11.zip` | 72,293,504 | `DC6DE8255FB9BB6687E3E2B6090E81F66F3202C8F8BA97A2002BA4B2D9D55EA9` |
| `dist/SealTools-v3.0.11-firmware.zip` | 6,574 | `F7BDD22E0395C5EC83FD7842B522C3BFC0F5BF347844032612D7C7EB8C425DD4` |

`dist/SHA256SUMS.txt` carries the lowercase hashes. The stage, app-zip and firmware audits all PASS.
Signing is NOT performed (SHELL-F9): the payload is unsigned; verify the SHA-256 before running.

## What changed from v3.0.10

### PP-OCRv6 small is a shipped trial

- **Setup -> OCR model** offers **PP-OCRv4 (current)** and **PP-OCRv6 small (trial)**. The choice is the
  portable `ocr.model_version`, written to `setup/common.yaml`; the next OCR read rebuilds the pipeline.
- **Both model sets ship in `models/`.** v6 adds the small detector, recogniser and the recogniser's
  dictionary sidecar (`PP-OCRv6_small_rec_dict.txt`); the ONNX graph does not embed the dictionary, so
  the sidecar is mandatory.
- **No silent fallback:** selecting v6 when its files are missing is `ocr_unavailable`, naming what was
  tried. v4 remains the default.
- **v6 is still a trial:** the live corpus/A-B gate is open. The one fixture trial measured v4 exact 0/1,
  CER 0.2105, 679 ms against v6 small exact 0/1, CER 0.2895, 806 ms; that is not enough to change the
  default.
- **Diagnostics:** the shell logs the launcher/core root, connection and OCR choice to
  `logs/shell-YYYYMMDD.log`; the core logs `loaded v4` / `loaded v6-small` to `logs/ocr_model.log`
  from the dispatcher cache, Tuner and Pet. Tuner `ocr_log.jsonl` now carries `raw_lines` beside the
  cleaned `lines`, so a fix can be told apart from line grouping.

### The Tuner mini bar is OCR-first

- **460 x 168** (was 360 x 96), with the OCR result as the centrepiece: Grade/Left/Try metrics in the
  footer, each attribute on its own line, and the Filter on its own line.
- The last live OCR facts stay on the bar after `tool.finished`, so the stopped/finished result can be
  checked without reopening the full window. The full fact set remains in the hover tooltip.
- The bar keeps one primary action; Stop stays fixed in the footer, and the finished state offers the
  dock/full-window actions.
- The quick dock's size presets, keep-open behaviour, collapsible icons and Setup sync from the same
  wave also ship here.

### Docs

- The finished mini-bar design studies are versioned under `v3/docs/design/finished/`.
- INSTALL now lists both OCR model sets; USER_GUIDE (EN and zh-TW) says the trial needs no manual model
  download and that v4 remains the default.

## Verification

- Rust workspace tests: **1,366 passed**, 0 failed (the 4 live-only board tests remain ignored by design).
- Shell gate: format clean, `flutter analyze --fatal-infos` clean, **394/394 tests**, Windows debug build.
- Release pipeline: `cargo test --workspace --locked`, Flutter release bundle, stage audit, app-zip audit,
  firmware source+zip audit all PASS.
- `build-info.json` records `product_version 3.0.11`, `git_commit e58cb52`, branch `main`,
  `git_dirty false`.
- Payload: 40 zip entries, one top folder, including all six OCR model files.

## Upgrade notes

- Config format is unchanged; no migration is required.
- v4 stays selected unless you choose v6 in Setup. Both sets are present in a fresh unzip, so trying v6
  needs no download.
- If you previously copied only v4 models into a custom `SEAL_OCR_MODELS` directory, add the three
  `PP-OCRv6_small_*` files there before selecting v6.
- The firmware zip is unchanged from v3.0.9/v3.0.10; no reflash is required for this release.

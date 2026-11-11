# Release v3.0.12 - PP-OCRv6 becomes the default

**Date:** 2026-11-11 - **Branch:** `main` - **Built from:** TBD

v3.0.12 changes the shipped OCR default from PP-OCRv4 to **PP-OCRv6 small** and brings the project docs
onto the v3 baseline. v4 remains selectable as the fallback.

## Artifacts (built 2026-11-11, published as GitHub release assets)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.12.zip` | TBD | TBD |
| `dist/SealTools-v3.0.12-firmware.zip` | TBD | TBD |

`dist/SHA256SUMS.txt` carries the lowercase hashes. The stage, app-zip and firmware audits must PASS.
Signing is NOT performed (SHELL-F9): the payload is unsigned; verify the SHA-256 before running.

## What changed

### OCR default

- **PP-OCRv6 small is now the default** (`ocr.model_version: v6-small`). The trial sample read the
  Traditional-Chinese attribute lines more cleanly than v4, including correct `所需希幣` and spacing.
- **v4 remains the shipped fallback** and is still selectable in **Setup -> OCR model**; both model
  sets ship in `models/`.
- If a selected model set is missing, OCR still fails with `ocr_unavailable` instead of silently
  switching sets.
- The Setup copy now labels v6 **current** and v4 **fallback**.

### Docs baseline

- Root `README.md` is now the v3.0.12 baseline: v3 download/quick start/tools/portable config, with v2
  as the previous stable fallback and v1 as legacy.
- `INSTALL.md`, `USER_GUIDE.md`, `USER_GUIDE.zh-TW.md`, the spec and third-party notices use the
  v3.0.12 version and the v6-default wording.
- The user guides now describe the portable config tree (`setup/`, `machine/`, `artifacts/`,
  `meta.yaml`) and the one-time `local.yaml` migration, instead of only the legacy file.
- The OCR plan board records v6 as the default with v4 retained as fallback.

## Verification

- Rust workspace tests: TBD.
- Shell gate: TBD.
- Release pipeline: stage, app-zip and firmware audits TBD.
- `build-info.json` must record `product_version 3.0.12`, the source commit, branch `main` and
  `git_dirty false`.

## Upgrade notes

- Config format is unchanged; no migration is required.
- An existing explicit `ocr.model_version` keeps its value. Configs without the setting now resolve to
  v6-small; both model sets are present in a normal install.
- If you use a custom `SEAL_OCR_MODELS` directory, add the three `PP-OCRv6_small_*` files before
  upgrading.
- The firmware zip is unchanged; no reflash is required.

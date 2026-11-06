# Release v3.0.10 - fresh-install Buy fix

**Date:** 2026-11-07 - **Branch:** `main` - **Built from:** `9860160`

v3.0.10 is a small patch release. A freshly unzipped v3 package has no `config/machine/` yet, and
`buy_sell.shop_rows` was absent from the shipped defaults. The core therefore resolved the count as
zero and Buy refused with:

> Shop rows aren't calibrated — the configured row count is 0.

The visible shop-list row count is a property of the game, not a measurement of the PC, so it is now
a static default instead of a machine-calibrated value.

## Artifacts (built 2026-11-07, published as GitHub release assets)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.10.zip` | 46,132,312 | `8CF2E227767F8E5F9A3D1B69FA4BDB22CDB1405380DBF7E2D31FC1D16F2E3CFF` |
| `dist/SealTools-v3.0.10-firmware.zip` | 6,574 | `F7BDD22E0395C5EC83FD7842B522C3BFC0F5BF347844032612D7C7EB8C425DD4` |

`dist/SHA256SUMS.txt` carries the lowercase hashes. Stage, app-zip and firmware audits all PASS.
Signing is NOT performed (SHELL-F9); verify the SHA-256 before running.

## What changed

- **Static default:** `v3/config/defaults.yaml` now ships `buy_sell.shop_rows: 10`, so a clean package
  has the value before any config is written. The fresh-package zip contains it in
  `config/defaults.yaml.example`.
- **Ownership:** `buy_sell.shop_rows` moved from `machine/buy_sell.yaml` to `setup/buy_sell.yaml` in
  the config registry. It is a game constant, not a measured field; the setup owner exists only so a
  config write can route it. The player never calibrates it.
- **Core fallback:** a missing `shop_rows` resolves to `ShopGeometry::DEFAULT_ROWS` (10). An
  explicitly configured `0` still reaches the domain's refusal.
- **Docs:** CONFIG now states the machine rule explicitly: `machine/` holds only values measured on
  this PC; a game constant belongs in shipped defaults, a portable player setting in `setup/`.

No shell UI or protocol change. The firmware zip is unchanged.

## Verification

- Rust format, clippy and rustdoc clean; **95 suites, 1,359 passed, 0 failed, 6 ignored**.
- Shell gate passed: format, analyze, **380/380 tests**, Windows debug build.
- Release pipeline: release core with `--locked`, Flutter release bundle, stage audit, app-zip audit,
  firmware source+zip audit; all PASS.
- `build-info.json` records `product_version 3.0.10`, `git_commit 9860160`, branch `main`,
  `git_dirty false`.

## Upgrade notes

- Config format is unchanged; no migration is required.
- A fresh install's Buy works without editing any config.
- Existing configs with a stale `machine/buy_sell.yaml` value keep loading; the field is no longer
  owned by the machine file, so it should be removed by a later cleanup rather than re-saved there.

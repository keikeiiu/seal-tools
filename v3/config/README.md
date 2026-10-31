# Your Seal Tools config folder

This folder is the core's state. The core takes a **root** whose `config/` subdirectory holds these
files (`seal-core.exe --root <folder>`); a packaged install has the exe and `seal-core.exe` at the
top of the folder, beside `config/`.

## The split: what travels, what does not

A file holds one **class** (does it travel?) for one **tool**. The short version:

| folder / file | copy to another PC? | what it is |
|---|---|---|
| `setup/` | **YES** | everything you configured: window title, game variant, board IDs, hotkeys, tuner/gem behaviour, spammer rotations, quest flows, buy presets, pet preferences |
| `config/user.yaml` | yes, if you want your notes to travel | your own extra blocks/notes; the core never reads or writes it |
| `machine/` | **NO** | this PC's measurements: calibration environment, OCR geometry, gem/buy/pet marks, tooltip offset, window placement, COM port override |
| `artifacts/` | **NO** | reference crops and learned icons (`pet_*.png`); recapture by calibrating |
| `meta.yaml` | **NO** | the core's bookkeeping (migration/layout markers), not hand-edited |
| `defaults.yaml.example` | — | the shipped base every portable key falls back to |
| `attributes*.yaml` | — | OCR dictionaries shipped with the release |

To move your setup to another PC: close the app and copy the `setup/` folder (and optionally
`user.yaml`). Do **not** copy `machine/` or `artifacts/` — a stale coordinate or display
measurement is worse than calibrating again. A fresh PC calibrates normally.

To back up everything precious: copy `setup/` and `user.yaml`. `machine/` needs no backup because
calibrating regenerates it.

## First run, upgrades, and the one-time migration

- A fresh install starts from the shipped templates. Tools refuse with their own calibration line
  until you calibrate; nothing is silently seeded with another PC's coordinates.
- An older install with a single `config/local.yaml` is migrated automatically on the first launch
  of this version: the whole `config/` folder is copied to
  `config/.migration-backup-<stamp>/`, the old file is split into `setup/` and `machine/`, inline
  crop images are written to `artifacts/`, foreign blocks move to `user.yaml`, and only then is the
  old `local.yaml` retired. The backup folder is your rollback; delete it once you are happy.
- Replacing the exe/templates with a newer release never touches `setup/`, `machine/`, `artifacts/`
  or `user.yaml`. A release zip carries only the templates, the dictionaries and the docs.
- Downgrading is one-way: an older core does not understand this folder. To roll back, restore the
  `.migration-backup-*` folder and the older exe.

## Files the core may add

| file | meaning |
|---|---|
| `meta.yaml` | layout and one-time migration markers; the core writes it, do not hand-edit |
| `.migration-backup-<stamp>/` | the pre-migration copy; safe to delete after a successful launch |
| `.seal-txn/` | a short-lived write journal while a two-file save is in flight; never edit or back it up |
| `local.yaml` | the OLD single-file layout. A development checkout and a pre-migration tree still use it; it is retired by the migration |

## Hand-editing

The files are YAML and readable. A save rewrites the file it changes: **comments inside a data
file are lost**, but keys the core does not read are **kept**, and a save never touches a file whose
values did not change. Keep prose here or in `user.yaml`; the core does not read this README.
`schema_version: 1` is reserved and rewritten on every save; a newer version is refused loudly
rather than guessed at.

## Dev checkout

This committed folder is the development tree: it has `defaults.yaml` (base) and
`local.yaml.example` (the first-run seed); `local.yaml` is gitignored machine state. Recreate a dev
`local.yaml` with:

```powershell
Copy-Item v3\config\local.yaml.example v3\config\local.yaml
```

A development checkout does NOT auto-migrate (it has no `defaults.yaml.example`); the live
single-file path stays in use there. The packaged install ships the template and migrates on first
launch.

## Packaging

`v3/publish.ps1` stages only the allow-listed release files (templates, dictionaries, docs, models,
exes) and audits the result: `local.yaml`, `setup/`, `machine/`, `artifacts/`, `user.yaml`,
`meta.yaml`, `.migration-backup-*`, `.seal-txn/`, `logs/` and `calib_*.png` are never shipped.
Run `.\v3\publish.ps1 -Verify` from the repo root to see missing release inputs.

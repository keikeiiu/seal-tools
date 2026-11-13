# Release v3.0.13 - one launcher, and it opens on screen

**Date:** 2026-11-13 - **Branch:** `main` - **Built from:** `406fba87666ca4f6bba4bfcdffd42aee44752042`

v3.0.13 adds the single-instance guard the shell has been missing and makes the first window fit the
monitor's work area.

## Artifacts (built 2026-11-13, published as GitHub release assets)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.13.zip` | 72,293,813 | `400D751A658AAA4E89DC9505DA2421C0224ADC266CCC2851A64016E3A507B436` |
| `dist/SealTools-v3.0.13-firmware.zip` | 6,574 | `F7BDD22E0395C5EC83FD7842B522C3BFC0F5BF347844032612D7C7EB8C425DD4` |

`dist/SHA256SUMS.txt` carries the lowercase hashes. The stage, app-zip and firmware audits must PASS.
Signing is NOT performed (SHELL-F9): the payload is unsigned; verify the SHA-256 before running.

## What changed

### One launcher per session

- The Windows runner holds a named local mutex for the process lifetime. A second launch finds the
  existing `Seal Tools v3` window (retrying briefly while the first instance creates it), restores it
  if minimized, focuses it and exits with success — before a second Flutter engine or core can start.
- The mutex is `Local\`, so a second Windows user in another session can run their own copy.
- This is a **shell-level** guard. The protocol's core-held cross-process lock is still open; starting
  `seal-core.exe` directly twice remains unsupported and is named in OPEN.md.

### The first window is clamped to the screen

- `Win32Window::Create` now measures the target monitor's work area and clamps the initial 1280×720
  window into it after DPI scaling. On a 1366×768 desktop with a taskbar, or at 125%/150% scaling, the
  launcher no longer opens partly below or beyond the usable screen.
- Saved placements were already clamped into the primary work area on restore; the initial rect now
  follows the same visibility rule.
- Full-screen/maximized runs remain inside the Windows work area; the window keeps its normal
  maximize/restore behaviour.

### Docs

- INSTALL's "two copies" row and both user guides now state that a second launch focuses the running
  window and exits, and that the direct-core lock is still open.
- DESIGN/OPEN distinguish the new shell guard from the still-open core cross-process lock.

## Verification

- Rust workspace tests: **1,366 passed**, 0 failed.
- Shell gate: format clean, `flutter analyze --fatal-infos` clean, **394/394 tests**, Windows debug build.
- Manual native check: first launch keeps one `SealTools.exe` + one `seal-core.exe`; second launch exits
  with code 0 and leaves exactly one window/process.
- Release pipeline: stage, app-zip and firmware audits **PASS**.
- `build-info.json` records `product_version 3.0.13`, `git_commit 406fba8`, branch `main`,
  `git_dirty false`.

## Upgrade notes

- Config format is unchanged; no migration is required.
- The firmware zip is unchanged; no reflash is required.
- Closing the only launcher still stops its core. A second launch is safe: it focuses the running copy
  instead of spawning another core.

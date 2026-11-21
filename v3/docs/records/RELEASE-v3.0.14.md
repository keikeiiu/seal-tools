# Release v3.0.14 - installer, diagnostics and update check

**Date:** 2026-11-14 - **Branch:** `main` - **Built from:** `0843276e2508d8aab12b58dab8568bca1fb4b238`

v3.0.14 adds the per-user installer, a one-click diagnostics bundle and a check-only update entry. The
portable zip remains available for local self-install.

## Artifacts (built 2026-11-14, published as GitHub release assets)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.14.zip` | 72,421,165 | `BF633B8148D47EC7C22029EE6A770AB87553B5601C06D4E18256BC12D7D7CF79` |
| `dist/SealTools-Setup-v3.0.14-x64.exe` | 65,410,435 | `B65F457C3354A2B3CB7E03554183BB2D3A832BE372D0387A900699E49F38B3F6` |
| `dist/SealTools-v3.0.14-firmware.zip` | 6,574 | `F7BDD22E0395C5EC83FD7842B522C3BFC0F5BF347844032612D7C7EB8C425DD4` |

`dist/SHA256SUMS.txt` carries the lowercase hashes. The stage, app-zip, installer and firmware audits
must PASS. Signing is NOT performed (SHELL-F9): the payload is unsigned; verify the SHA-256 before
running.

## What changed

### Per-user installer

- `SealTools-Setup-v3.0.14-x64.exe` installs under `%LOCALAPPDATA%\Programs\SealTools`, with no
  administrator/UAC.
- `config/` and `logs/` stay beside the app. An upgrade replaces program files only; the uninstaller
  keeps `config/` and `logs/` by default.
- Optional desktop/start-menu shortcuts; start-with-Windows is opt-in and unchecked by default.
- The **portable zip still ships** as the no-install path; the installer is an additional artifact.
- The release pipeline now builds the installer, verifies it exists, and includes it in
  `SHA256SUMS.txt`.

### Diagnostics export

- **Logs → Export diagnostics** writes `logs/SealTools-diagnostics-<stamp>.zip` and reveals it in the
  logs folder.
- The bundle contains the manifest (app/core versions, root, OS), shell/core text logs, OCR logs and a
  config-file name list.
- Personal calibration values, reference images, screenshots/captures and config values are excluded.

### Update check

- Setup → **About** shows the shell/core versions and **Check for updates**.
- The check reads the public GitHub release feed, compares the semantic version, and offers **Open
  vX.Y.Z**.
- It never downloads, replaces or restarts the running app; the installer and portable zip remain the
  update paths.

### Docs

- README and INSTALL list both installer and portable zip, with config-preserving upgrade/uninstall
  rules.
- USER_GUIDE (EN and zh-TW) adds an installer-vs-portable section and the About/update-check note.

## Verification

- Rust workspace tests: **1,366 passed**, 0 failed.
- Shell gate: format clean, `flutter analyze --fatal-infos` clean, **397/397 tests**, Windows debug build.
- Release pipeline: stage, app-zip, installer and firmware audits **PASS**.
- Installer smoke test on the final artifact: fresh install layout correct, upgrade preserved a
  `config/` marker, autostart was off by default, uninstall removed program files and kept `config/`.
- `build-info.json` records `product_version 3.0.14`, `git_commit 0843276`, branch `main`,
  `git_dirty false`.

## Upgrade notes

- Config format is unchanged; no migration is required.
- Existing portable installs can either keep using the zip or switch to the per-user installer; the
  installer preserves `config/` when upgrading over an existing install.
- The firmware zip is unchanged; no reflash is required.

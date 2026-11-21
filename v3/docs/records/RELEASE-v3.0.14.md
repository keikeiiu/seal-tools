# Release v3.0.14 - installer, diagnostics and update check

**Date:** 2026-11-14 - **Branch:** `main` - **Built from:** `2d1a0a95da8d74d841d81f10764c95ddf5cb1215`

v3.0.14 adds the per-user installer, a one-click diagnostics bundle and a check-only update entry. The
portable zip remains available for local self-install.

## Artifacts (built 2026-11-14, published as GitHub release assets)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.14.zip` | 72,421,214 | `B1BDD7113F982307F3E257A9B14B76B076F4D327364DFBFCF129C47D80080E34` |
| `dist/SealTools-Setup-v3.0.14-x64.exe` | 65,351,507 | `BF13030B4E4DE655DD065C3629F9DB0909767A6B0053085BC2B320CA3ED2AA61` |
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
- `build-info.json` records `product_version 3.0.14`, `git_commit 2d1a0a9`, branch `main`,
  `git_dirty false`.

## Upgrade notes

- Config format is unchanged; no migration is required.
- Existing portable installs can either keep using the zip or switch to the per-user installer; the
  installer preserves `config/` when upgrading over an existing install.
- The firmware zip is unchanged; no reflash is required.

# Release v3.0.14 - installer, diagnostics and update check

**Date:** 2026-11-14 - **Branch:** `main` - **Built from:** TBD

v3.0.14 adds the per-user installer, a one-click diagnostics bundle and a check-only update entry. The
portable zip remains available for local self-install.

## Artifacts (built 2026-11-14, published as GitHub release assets)

| artifact | bytes | SHA-256 |
|---|---|---|
| `dist/SealTools-v3.0.14.zip` | TBD | TBD |
| `dist/SealTools-Setup-v3.0.14-x64.exe` | TBD | TBD |
| `dist/SealTools-v3.0.14-firmware.zip` | TBD | TBD |

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

- Rust workspace tests: TBD.
- Shell gate: TBD.
- Release pipeline: stage, app-zip, installer and firmware audits TBD.
- Installer smoke test: fresh layout, silent upgrade preserving `config/`, uninstall keeping `config/`,
  and no autostart selected by default must be verified on the staged payload.
- `build-info.json` must record `product_version 3.0.14`, the source commit, branch `main` and
  `git_dirty false`.

## Upgrade notes

- Config format is unchanged; no migration is required.
- Existing portable installs can either keep using the zip or switch to the per-user installer; the
  installer preserves `config/` when upgrading over an existing install.
- The firmware zip is unchanged; no reflash is required.

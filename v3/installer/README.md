# Seal Tools installer

The installer is a **per-user** NSIS install for future releases. The portable zip remains the
self-install path and is still built by `v3/publish.ps1`; this adds `SealTools-Setup-vX.Y.Z-x64.exe`
beside it.

## Install location

```text
%LOCALAPPDATA%\Programs\SealTools
```

No administrator elevation. `config/` and `logs/` stay beside the app so the core can keep writing
them without a data-directory migration.

## Config preservation

- A fresh install seeds `config/` from the staged payload.
- An upgrade **never replaces** `config/`; unknown files survive.
- Uninstall removes program files and keeps `config/` + `logs/`.
- The portable zip still works on its own; the installer does not change the zip's layout.

## Build

NSIS 3 is required (`makensis.exe`). The release pipeline finds it via `MAKENSIS` or the default
`Program Files (x86)\NSIS` path.

```powershell
makensis /DVERSION=3.0.14 /DPAYLOAD=<dist\SealTools-v3.0.14> /DOUTFILE=<dist\SealTools-Setup-v3.0.14-x64.exe> install.nsi
```

`DPAYLOAD` is the audited stage directory, never a git checkout.

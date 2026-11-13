# Seal Tools v3.0.13

Windows automation for Seal Online. A Rust core drives an Arduino Pro Micro as a real USB
mouse/keyboard; a Flutter shell provides the UI. OCR reads the game screen where needed.

**v3 is the current baseline.** v2 (C#/.NET) is the previous stable fallback; v1 is legacy.

## Download

- **Recommended:** **[SealTools-Setup-v3.0.13-x64.exe](https://github.com/keikeiiu/seal-tools/releases/download/v3.0.13/SealTools-Setup-v3.0.13-x64.exe)** — per-user installer; keeps `config/` on upgrade and uninstall.
- **[SealTools-v3.0.13.zip](https://github.com/keikeiiu/seal-tools/releases/download/v3.0.13/SealTools-v3.0.13.zip)** — portable; unzip and run `SealTools.exe`.
- **[SealTools-v3.0.13-firmware.zip](https://github.com/keikeiiu/seal-tools/releases/download/v3.0.13/SealTools-v3.0.13-firmware.zip)** — only for a new board; the firmware is unchanged since v2.15.

The installer is per-user (`%LOCALAPPDATA%\Programs\SealTools`) and needs no administrator rights. The
portable zip remains the self-install path. No .NET runtime is required; the app starts `seal-core.exe`
itself.

Previous generation: [v2.16](https://github.com/keikeiiu/seal-tools/releases/download/v2.16/SealTools-v2.16.zip).

## Requirements

| | |
|---|---|
| PC | Windows 10/11, 64-bit |
| Board | Arduino Pro Micro (ATmega32U4, 5 V / 16 MHz) |
| Cable | USB **data** cable |
| Game | Seal Online TW or US, windowed |

## Quick start

1. Unzip the app zip and run `SealTools.exe`.
2. New board only: flash `seal_mouse/seal_mouse.ino` from the firmware zip.
3. Open **Setup** and find the game window; choose the client variant and board port.
4. Open **Calibrate** and save the marks the tools you use need.
5. Pick the OCR model in **Setup** — v6 small is the default; v4 remains the fallback.
6. Start a tool from its card.

Full walkthrough: [Install guide](v3/docs/INSTALL.md).

## Tools

| Tool | What it does |
|---|---|
| **Magic Tuner** | Clicks the tuner UI and OCRs grade/attributes until the target or filter matches. |
| **Gem Composer** | Combines gems grade by grade until the result box is empty. |
| **Skill Spammer** | Presses a cooldown-governed key rotation. |
| **Buy** | Re-buys configured shop items in one counted pass. |
| **Sell** | Sells the slots selected for that run, highest first, under a cap. |
| **Pet Feeder** | Resident feeder that reloads each boarding row on schedule. |
| **Quest Hand-in** | Replays authored click/key flows at the quest NPC. |

Also on the Cards screen: **Hold Space**, a toggle for holding the spacebar.

## Files and updates

Everything stays inside the app folder:

```text
SealTools.exe / seal-core.exe
models/            OCR model sets (v4 and v6 small)
config/setup/      portable settings and presets
config/machine/    this PC's calibration
config/artifacts/  calibration images
logs/              run logs and OCR captures
docs/              install and user guides
```

The zip carries program files and config templates only. Unzipping an update does **not** overwrite
`config/setup/`, `config/machine/` or `config/artifacts/`.

## Documentation

- [Install guide](v3/docs/INSTALL.md)
- [User guide](v3/docs/USER_GUIDE.md) · [繁體中文](v3/docs/USER_GUIDE.zh-TW.md)
- [Calibration guide](v3/docs/CALIBRATION-GUIDE.md) · [繁體中文](v3/docs/CALIBRATION-GUIDE.zh-TW.md)
- [Architecture and specs](v3/docs/spec/DESIGN.md)
- [Release records](v3/docs/records/)

The v2 build remains available with its own [docs](v2/docs/) and [README](v2/README.md).

## Build from source

```powershell
cd v3/core
.\cargo.cmd test --workspace --offline
.\cargo.cmd clippy --workspace --all-targets --offline -- -D warnings

cd ..\shell
pwsh -File tool/check.ps1

cd ..\
.\publish.ps1 -Release -AcceptDirectMLRedistribution
```

## Disclaimer

Automation may violate the game's Terms of Service and can lead to account penalties. Use at your own
risk, on your own accounts.

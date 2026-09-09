# Seal Tools v2.1 (C# / .NET 8 WPF)

A full C#/.NET 8 rebuild of the Seal Online automation tools — a **native Windows desktop app**
(WPF + WPF-UI) that ships as a **single self-contained `.exe`**. No Python, no pip, no runtime install.

> The original Python build lives under [`v1/`](v1/) — its launcher, tools, docs and tests — and is
> documented in [v1/README.md](v1/README.md). v2 lives entirely under [`v2/`](v2/).

## Download

**[Download SealTools-v2.1.zip](https://github.com/keikeiiu/seal-tools/releases/latest/download/SealTools-v2.1.zip)** — the self-contained distributable (exe + OCR models + config templates).

> The zip is built from `v2/publish.bat` (~144 MB of binaries), so it's published as a
> **GitHub Release asset** rather than committed to the repo. To build from source, see [v2/README.md](v2/README.md).

## Quick start

1. Unzip `SealTools-v2.1.zip`.
2. Run `SealTools.Launcher.exe` — **as Administrator** is recommended (serial access; on some setups the in-game hotkeys need it too). On first run it auto-creates `config\local.yaml` from the example.
3. Use the in-app **Calibrate** tabs (Tuner + Gem) to set your machine's coordinates once.

> **Hotkeys need the launcher focused:** the game's anti-cheat blocks background key reads, so
> F11/F12/CapsLock do nothing while you are in-game — click the launcher first, then the key.

## What's inside

Three tools, driven by an Arduino Pro Micro (USB HID mouse/keyboard) over a COM port, plus OCR:

| Tool | What it does |
|------|--------------|
| **Magic Tuner** | Rolls the 發條 UI — Arduino click+Enter, OCR reads grade + attributes, auto-stops at the target grade / filter match. |
| **Gem Composer** | Clicks the gem-combine UI (N/G/DG radio + Register + Combine). |
| **Skill Spammer** | Presses configured keys, each on its own cooldown. |

## Full docs

- **[v2/docs/USER_GUIDE.md](v2/docs/USER_GUIDE.md)** — every tab and button explained.
- **[v2/docs/CALIBRATION.md](v2/docs/CALIBRATION.md)** — per-machine calibration walkthrough.
- **[v2/README.md](v2/README.md)** — architecture, config files, logging and build instructions.

## Disclaimer

Automation may violate the game's Terms of Service and can lead to account penalties. Use at your own risk, on your own accounts.

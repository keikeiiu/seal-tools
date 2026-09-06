# Seal Tools v2 (C# / .NET 8 WPF)

A full C#/.NET 8 rebuild of the Seal Online automation tools — a **native Windows desktop app**
(WPF + WPF-UI) that ships as a **single self-contained `.exe`**. No Python, no pip, no runtime install.

> The original Python build (`launcher.py`, `tuner/`, `gem_composer/`, `skill_spammer/`, `checkin/`)
> still lives in this repo and is documented in [README.md](README.md). v2 lives entirely under [`v2/`](v2/).

## Download

**[Download SealTools-v2.zip](https://github.com/keikeiiu/seal-tools/releases/latest/download/SealTools-v2.zip)** — the self-contained distributable (exe + OCR models + config templates).

> The zip is built from `v2/publish.bat` (~144 MB of binaries), so it's published as a
> **GitHub Release asset** rather than committed to the repo. To build from source, see [v2/README.md](v2/README.md).

## Quick start

1. Unzip `SealTools-v2.zip`.
2. Copy `config\local.yaml.example` → `config\local.yaml`.
3. Run `SealTools.Launcher.exe` — **as Administrator** so the hotkeys work while the game is focused.
4. Use the in-app **Calibrate** tabs (Tuner + Gem) to set your machine's coordinates once.

## What's inside

Three tools, driven by an Arduino Pro Micro (USB HID mouse/keyboard) over a COM port, plus OCR:

| Tool | What it does |
|------|--------------|
| **Magic Tuner** | Rolls the 發條 UI — Arduino click+Enter, OCR reads grade + attributes, auto-stops at the target grade / filter match. |
| **Gem Composer** | Clicks the gem-combine UI (N/G/DG radio + Register + Combine). |
| **Skill Spammer** | Presses configured keys, each on its own cooldown. |

## Full docs

Architecture, config files, calibration workflow, logging, and build instructions are in
**[v2/README.md](v2/README.md)**.

## Disclaimer

Automation may violate the game's Terms of Service and can lead to account penalties. Use at your own risk, on your own accounts.

# Seal Tools v2.3 (C# / .NET 8 WPF)

A full C#/.NET 8 rebuild of the Seal Online automation tools — a **native Windows desktop app**
(WPF + WPF-UI) that ships as a **single self-contained `.exe`**. No Python, no pip, no runtime install.

> The original Python build lives under [`v1/`](v1/) — its launcher, tools, docs and tests — and is
> documented in [v1/README.md](v1/README.md). v2 lives entirely under [`v2/`](v2/).

## Downloads

| Version | File | What it is |
|---|---|---|
| **v2.3** (latest) | [SealTools-v2.3.zip](https://github.com/keikeiiu/seal-tools/releases/download/v2.3/SealTools-v2.3.zip) | WPF-UI launcher: card-based tabs, collapsible config, pin-on-top and a running-tool mini view |
| v2.2 | [SealTools-v2.2.zip](https://github.com/keikeiiu/seal-tools/releases/download/v2.2/SealTools-v2.2.zip) | Arduino closed-loop cursor + moves, pixel-based empty-result check, run ends after the last grade |
| v2.1 | [SealTools-v2.1.zip](https://github.com/keikeiiu/seal-tools/releases/download/v2.1/SealTools-v2.1.zip) | Physical-pixel capture, Setup tab, spammer presets, [user guide](v2/docs/USER_GUIDE.md) |
| v2.0 | [SealTools-v2.zip](https://github.com/keikeiiu/seal-tools/releases/download/V2.0.0/SealTools-v2.zip) | First C#/.NET release |

Self-contained distributables (exe + OCR models + config templates) — unzip and run. All releases:
[github.com/keikeiiu/seal-tools/releases](https://github.com/keikeiiu/seal-tools/releases).

> The zips are built from `v2/publish.bat` (~144 MB of binaries), so they're published as
> **GitHub Release assets** rather than committed to the repo. To build from source, see [v2/README.md](v2/README.md).

## Quick start

1. Unzip `SealTools-v2.3.zip`.
2. Run `SealTools.Launcher.exe` — **as Administrator** is recommended (serial access; on some setups the in-game hotkeys need it too). On first run it auto-creates `config\local.yaml` from the example.
3. Use the in-app **Calibrate** tabs (Tuner + Gem) to set your machine's coordinates once.

> **Hotkeys need the launcher focused:** the game's anti-cheat blocks background key reads, so
> F11/F12/CapsLock do nothing while you are in-game — click the launcher first, then the key.

## What's inside

Three tools, driven by an Arduino Pro Micro (USB HID mouse/keyboard) over a COM port, plus OCR:

| Tool | What it does |
|------|--------------|
| **Magic Tuner** | Rolls the 發條 UI — Arduino click+Enter, OCR reads grade + attributes, auto-stops at the target grade / filter match. |
| **Gem Composer** | Clicks the gem-combine UI (N/G/DG radio + Register + Combine) at calibrated points; combines each grade until the result box reads empty, then advances, and ends after the last grade. |
| **Skill Spammer** | Presses configured keys, each on its own cooldown. |

## Full docs

- **[v2/docs/USER_GUIDE.md](v2/docs/USER_GUIDE.md)** — every tab and button explained.
- **[v2/docs/CALIBRATION.md](v2/docs/CALIBRATION.md)** — per-machine calibration walkthrough.
- **[v2/docs/MOVE-SETS.md](v2/docs/MOVE-SETS.md)** — the composer's two move sets and how the closed-loop move works.
- **[v2/docs/PROGRESS.md](v2/docs/PROGRESS.md)** — dated log of what was done and why.
- **[v2/README.md](v2/README.md)** — architecture, config files, logging and build instructions.

## Disclaimer

Automation may violate the game's Terms of Service and can lead to account penalties. Use at your own risk, on your own accounts.

# Seal Tools v2.9 (C# / .NET 8 WPF)

A full C#/.NET 8 rebuild of the Seal Online automation tools — a **native Windows desktop app**
(WPF + WPF-UI) that ships as a **single self-contained `.exe`**. No Python, no pip, no runtime install.

> The original Python build lives under [`v1/`](v1/) — its launcher, tools, docs and tests — and is
> documented in [v1/README.md](v1/README.md). v2 lives entirely under [`v2/`](v2/).

## Download

**Take the latest — [SealTools-v2.9.zip](https://github.com/keikeiiu/seal-tools/releases/download/v2.9/SealTools-v2.9.zip).**
Self-contained (exe + OCR models + config templates) — unzip and run. Installation, including flashing
the board, is in **[v2/docs/INSTALL.md](v2/docs/INSTALL.md)**.

Older releases are kept only when they are a useful fallback — there is currently one:

| Version | File | |
|---|---|---|
| **v2.9** | [SealTools-v2.9.zip](https://github.com/keikeiiu/seal-tools/releases/download/v2.9/SealTools-v2.9.zip) | Latest |
| v2.3 | [SealTools-v2.3.zip](https://github.com/keikeiiu/seal-tools/releases/download/v2.3/SealTools-v2.3.zip) | The last build before buy/sell |

**Firmware.** The Arduino sketch ships alongside the app, not inside it:
[SealTools-v2.9-firmware.zip](https://github.com/keikeiiu/seal-tools/releases/download/v2.9/SealTools-v2.9-firmware.zip).
Unzip it and open `seal_mouse\seal_mouse.ino` in the Arduino IDE. From v2.9 on this is published with
every release — before that it was only in the repo, so a downloaded zip had no way to flash a board.

> The zips are built from `v2/publish.bat` (~144 MB of binaries), so they're published as
> **GitHub Release assets** rather than committed to the repo. To build from source, see [v2/README.md](v2/README.md).

## Version history

What went into each build. **Not every version has a download** — older release assets are removed
once they stop being useful, and v2.4 and v2.6 were never published as releases at all. Every version
is a git tag, so `git show v2.6` still works, and the full reasoning behind each is in
[v2/docs/PROGRESS.md](v2/docs/PROGRESS.md).

| Version | Date | What it added |
|---|---|---|
| **v2.9** | 2026-09-14 | **Buy and Sell.** Bulk-buy the items you re-buy constantly and bulk-sell what you don't keep, both verified on a live game. Buy items are presets (row, scroll, usual count) kept in `local.yaml`; the sell selection is deliberately never saved. **First release to ship the Arduino firmware** — before this the sketch was only in the repo, so a downloaded zip had no way to flash a board. |
| v2.6 | 2026-09-13 | Safety and correctness pass, none of it visible: one tool start at a time, a stop during a cold start honoured, a failed config save reported instead of leaving the click looking inert, and the tuner stopping on a failed OCR read rather than below the target grade. Spammer presets left in `defaults.yaml` are now adopted into `local.yaml` instead of being deleted by the next save from any other tab. Hold Space releases the spacebar on stop; the firmware releases held keys when the host disappears. *(No release published; tag only.)* |
| v2.4 | 2026-09-12 | Tuner cursor placement (`spring_mode: hid`) and the mouse guard (`off` / `stop` / `recenter`). OCR fixes: `DG`/`XG`/`SG` no longer parsed as a bare `G`, and attribute lines the matcher used to drop are restored. *(No release published; tag only.)* |
| v2.3 | 2026-09-11 | The WPF-UI launcher: card-based tabs, pin-on-top, shrinking to the running tool's card, and placement remembered across restarts. Arduino closed-loop cursor placement. Pixel-based empty-result detection, comparing only the box's interior so a moved window can't stall the composer. A run ends after the last grade. Spammer presets moved to `local.yaml` so they stop shipping in the public zip. |
| v2.1 | 2026-09-10 | Capture actually worked: `PrintWindow` returned a black frame for this game and was replaced by `CopyFromScreen`, which also fixed a silently cropped OCR region. Coordinates became physical pixels with the display environment recorded beside them (the **Setup** tab). Spammer presets and the first [user guide](v2/docs/USER_GUIDE.md). |
| v2.0 | 2026-09-06 | First C#/.NET release — a full native rebuild of the Python tools under [`v1/`](v1/). |


## Quick start

1. Unzip `SealTools-v2.9.zip`.
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
| **Buy** | Re-buys the items you go through constantly: right-click the shop row, MAX, Enter, Enter, for the count set on the card. |
| **Sell** | Sells the bag slots you click on an 8×8 grid, highest slot first, with a per-run cap and a dry run that clicks nothing. |

## Full docs

- **[v2/docs/INSTALL.md](v2/docs/INSTALL.md)** — board, app, first calibration, updating.
- **[v2/docs/USER_GUIDE.md](v2/docs/USER_GUIDE.md)** — every card, tab and button explained.
- **[v2/docs/CALIBRATION.md](v2/docs/CALIBRATION.md)** — per-machine calibration walkthrough.
- **[v2/docs/MOVE-SETS.md](v2/docs/MOVE-SETS.md)** — the composer's two move sets and how the closed-loop move works.
- **[v2/docs/PROGRESS.md](v2/docs/PROGRESS.md)** — dated log of what was done and why.
- **[v2/README.md](v2/README.md)** — architecture, config files, logging and build instructions.

## Disclaimer

Automation may violate the game's Terms of Service and can lead to account penalties. Use at your own risk, on your own accounts.

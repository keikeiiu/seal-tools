# Seal Tools v2.1

Native Windows automation for Seal Online — Magic Tuner, Gem Composer and Skill Spammer, driven by an
Arduino Pro Micro that presents as a genuine USB HID mouse/keyboard. Ships as a single self-contained
`.exe`: no Python, no .NET install.

**Download:** `SealTools-v2.1.zip` (144 MB) — unzip and run `SealTools.Launcher.exe`.

## Highlights

**Capture actually works now.** The calibrator and OCR capture the whole game window. Previously the
calibration screenshot came back black (`PrintWindow` returns nothing for this game) and the region
capture silently cropped to the top-left two-thirds of the window — which also fed OCR and the
composer's empty-box check.

**Calibration is portable.** Coordinates are stored in physical pixels with the display environment
(scale, DPI, screen size, client size) recorded alongside, so another machine can tell whether it can
reuse them or must recalibrate.

**New Setup tab.** One button measures the monitor scale and the game window rects; the scale and the
reference client size stay editable, and it warns when the game window size no longer matches the
calibration.

**Skill Spammer presets.** Save named key sets and switch between them from a dropdown — add, rename
and delete presets, edit key/delay rows, or drop to the raw list in Advanced mode. Also fixes Start
leaving the spammer paused.

**Better feedback.** Check OCR prints the grade, the spring count, the three attribute lines, the
matched attributes and the filter verdict — with a fresh drag or the saved calibration. A missing
calibration or an unplugged Arduino is now reported on the tool card instead of silently stopping.

**Repo layout.** The Python v1 tools now live under `v1/`; the root README is the v2 landing page.

## First run

1. Unzip `SealTools-v2.1.zip`.
2. Run `SealTools.Launcher.exe` — `config\local.yaml` is created from the example on first run.
3. **Setup** tab → **Detect** → **Save Setup**.
4. **Calibrate Tuner** and **Calibrate Gem** once for your machine.

## Known limitations

- **Hotkeys only work when the game is *not* focused.** The anti-cheat blocks background key reads, so
  F11/F12/CapsLock do nothing while the game has focus — click the launcher first, then the key, or
  use the Stop button.
- **Turn off "Enhance pointer precision"** for the Gem Composer; its relative moves are hand-tuned HID
  counts that assume a fixed pointer speed.
- **Recalibrate if the game window size changes** — coordinates are client-relative physical pixels.
- Upgrading from an older build: recalibrate once (old coordinates were logical pixels). Hand-tuned
  `gem.movements` are unaffected.

## Requirements

- Windows 10/11 (64-bit)
- Arduino Pro Micro flashed with `arduino/seal_mouse/seal_mouse.ino`
- The game running in windowed mode

## Disclaimer

Automation may violate the game's Terms of Service and can lead to account penalties. Use at your own
risk, on your own accounts.

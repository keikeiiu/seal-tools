# Seal Tools v2 — Install Guide

From nothing to a working install: the board, the app, and the first calibration.

If you already have it running and want to know what a button does, that is
[USER_GUIDE.md](USER_GUIDE.md). If you are calibrating, [CALIBRATION.md](CALIBRATION.md).

**The calibration is the long part**, and it is the part you cannot skip — see
[Why calibration is not optional](#why-calibration-is-not-optional). Everything before it (flashing
the board, unzipping, first launch) is a few minutes.

---

## What you need

| | |
|---|---|
| **PC** | Windows 10 or 11, 64-bit. Nothing else — the app is self-contained and needs no .NET, Python or runtime install. |
| **Board** | An **Arduino Pro Micro (ATmega32U4), 5V / 16 MHz**. This is the only board that works. |
| **Cable** | A USB cable that carries **data**. Charge-only cables are the single most common reason a board is never found. |
| **Game** | Seal Online (希望戀曲 / 希望Online TW), running windowed. |
| **Disk** | ~200 MB for the app, plus a few MB per capture if you turn capture logging on. |

> **Why a Pro Micro specifically.** The game's anti-cheat blocks synthetic input from software —
> `SendInput`, and `SetCursorPos` while the game is focused. The Pro Micro has a native USB
> controller and enumerates as a *real* USB HID mouse and keyboard, so its clicks and keypresses are
> indistinguishable from a human's at the driver level. That is the entire reason for the board.
> **An Uno, Nano or Mega will not work** — they have no native USB HID, so they cannot present as a
> mouse at all.

---

## 1. Download

From [Releases](https://github.com/keikeiiu/seal-tools/releases). Pick one app zip **and** the
firmware zip:

| File | What it is | When you want it |
|---|---|---|
| `SealTools-v2.9.zip` | The app, with **template config only**. | **Normal install.** You calibrate on this machine. |
| `SealTools-v2.9-firmware.zip` | The Arduino sketch, `seal_mouse\seal_mouse.ino`. | Whenever the board is not already flashed. |
| `SealTools-v2.9-local.zip` | The app with a **full `config\`** — someone's real calibration and `local.yaml`. | **Only** to restore *this* machine after wiping it. See the warning below. |

> ⚠ **Do not use the `-local` zip on a different machine.** It carries coordinates measured on the PC
> it was built from. On a machine with a different resolution, DPI scale or game client size, every
> click goes to the wrong place and *nothing detects the mismatch* — the run reports success. For
> **Buy** that wastes gold. For **Sell**, the one action here that cannot be undone, it sells the
> wrong stacks. If in any doubt, use the public zip and calibrate.

---

## 2. Flash the Arduino

Skip to step 3 if your board already runs this firmware.

### 2.1 Install the IDE and board support

1. Install the **Arduino IDE** — <https://www.arduino.cc/en/software> (2.x is fine).
2. The Pro Micro is **not** in the default board list. Two ways:
   - **Easiest, no extra install:** `Tools → Board → Arduino AVR Boards → **Arduino Leonardo**`
     (or **Arduino Micro** — same ATmega32U4 chip, and most Pro Micro clones ship a
     Leonardo/Micro-compatible bootloader).
   - **Proper SparkFun package:** `File → Preferences → Additional Boards Manager URLs` add
     ```
     https://raw.githubusercontent.com/sparkfun/Arduino_Boards/main/IDE_Board_Manager/package_sparkfun_index.json
     ```
     then `Tools → Board → Boards Manager` → search **"SparkFun AVR Boards"** → Install, and select
     `SparkFun Pro Micro`, processor **ATmega32U4 (5V, 16 MHz)**.
3. Plug the board in and select its **port** under `Tools → Port` (e.g. `COM5`).
   If no port appears, install the driver: `arduino.inf` from the Arduino install directory, or the
   **CH340** driver if your clone uses a CH340 USB-serial chip (very common on cheap clones).

### 2.2 Upload

1. Unzip the firmware zip, then `File → Open` → `seal_mouse\seal_mouse.ino`.
   (Open it from inside its folder — the Arduino IDE requires a sketch to sit in a directory of the
   same name, which is why the zip preserves `seal_mouse\`.)
2. Confirm the **board** and **port**, then click **Upload** (the → arrow).

**If the upload fails:** the Pro Micro's bootloader only listens for about **8 seconds** after a
reset. Click **Upload first**, then quickly **tap `RST` twice** (or short RST to GND twice on boards
with no button) — the upload then proceeds. If it still fails, try a different USB cable, then a
USB 2.0 port; some USB 3 hubs are unreliable with the 32U4 bootloader.

### 2.3 Verify before going further

Arduino IDE → `Tools → Serial Monitor`, baud **115200**, type `C` and press Enter. **A left click
should fire.** Do this now: it separates "the board is wrong" from "the app is misconfigured", and
those two look identical later.

---

## 3. Install the app

1. Unzip `SealTools-v2.9.zip` anywhere — Desktop, `C:\Games\SealTools`, a USB stick. There is no
   installer and nothing is written outside the folder.
2. Run `SealTools.Launcher.exe`.
   **Recommended: right-click → Run as administrator.** The app opens a serial port, and on some
   setups the global hotkeys need elevation too.

The launcher opens as five tool cards with **▸ Configuration** below them. The config tabs are
hidden until you need them.

### First run creates your config

The zip ships `config\defaults.yaml`, `config\attributes.yaml` and `config\local.yaml.example`. On
first launch, `ConfigLoader` copies the example to `config\local.yaml` if it does not exist.

- **`defaults.yaml`** — portable settings, the same for everyone (window title, hotkeys, filter rules,
  VID/PID). Written by the **Tuner**, **Gem** and **Hotkeys** tabs.
- **`local.yaml`** — everything measured on *this* machine, plus your personal spammer presets.
  Written by the calibrators and the **Spammer** tab. Never commit it, never share it.
- **`logs\`** — runtime logs, next to `config\`. `logs\captures\` fills up only if you turn capture
  logging on.

The seeded `local.yaml` holds placeholder coordinates from an old setup. **They will be wrong.**
That is expected — step 4 replaces them.

---

## 4. Calibrate

Full walkthrough: [CALIBRATION.md](CALIBRATION.md). The short version:

1. **Setup** tab → **Detect** → check the scale and client size look right → **Save Setup**.
2. **Calibrate Tuner** → **Capture 發條 window** → drag the three boxes → **Check OCR** (it must read
   a real grade, the count and three attribute lines) → **Save Tuner**.
3. **Calibrate Gem** → **Capture gem window** → click the points and drag the result box →
   **Save Gem Composer**.
4. **Calibrate Buy / Sell** → **Capture bag window** (shop open, bag open) → draw the bag grid and one
   slot → drag the shop list region → mark the focus point and MAX → **Save Calibration**.

Before calibrating anything that clicks, do the two prerequisites:

- **Fixed Windows pointer precision.** `Settings → Bluetooth & devices → Mouse → Additional mouse
  options → Pointer Options` → uncheck **"Enhance pointer precision"**. The game is an OS-cursor
  title, so the distance a relative move travels depends on this setting; leaving it on makes the
  Gem Composer drift between runs. See [CALIBRATION.md §4](CALIBRATION.md).
- **The game window title must match `window.title`** in `defaults.yaml` — `TW_LIVE` by default.
  Nothing will find the window otherwise, and the symptom is "game window not found" from every
  capture button. Edit `defaults.yaml` if your client is titled differently.

### Why calibration is not optional

Coordinates are stored as **physical pixels relative to the game window's client area**. They depend
on the monitor resolution, the Windows display scale, the game window size and where the window sits.
None of that travels between machines, and none of it is detected and corrected for you.

The **Setup** tab is how you tell whether a calibration still applies: it stores the scale and client
size a calibration was measured against, and shows a **⚠ warning** when your live client size differs.

---

## 5. Check it works, before trusting it

Do these in order. Each one is read-only or costs nothing:

1. **Arduino** tab → **Refresh** → the light is green and a port is marked `>>`.
   → **Send a test click** → a click fires at the cursor.
2. **Calibrate Tuner** → **Check OCR** on a real item. It prints the grade, the spring count and the
   three attribute lines, and what the filter made of them.
3. **Calibrate Gem** → **Test Click** a point, and **Check Result Colour**.
4. **Calibrate Buy / Sell** → **Show 64 centres**. The magenta dots must sit on the bag slots.
5. **Sell** tab → pick a few slots → **Dry run**. It moves the cursor to each slot in the order it
   would sell them, and **clicks nothing**.
6. **Buy** tab → pick an item → **Dry run**. It scrolls and parks the cursor on the row. With the list
   scrolled to the top, the pointer must land on the item you picked.

> **Run Sell for real last.** Buying's worst case is spent gold; selling's is gone items. Watch a dry
> run through completely before the first live one, and keep the per-run cap low until you trust it.

---

## Updating

Unzip the new version over the old one **except for `config\`**, or simply unzip it to a new folder
and copy your `config\local.yaml` across. Your calibration and presets live there and in nothing else.

Do not copy `defaults.yaml` from an old install unless you have customised it — it is the file that
carries the portable behavior defaults, and a stale one can silently miss new keys. Compare the old
and new side by side if you have edited it.

## Uninstalling

Delete the folder. Nothing is installed elsewhere — no registry keys, no AppData, no services. Your
calibration goes with it, so back up `config\local.yaml` first if you may come back.

---

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| **"Arduino not found"** on Start | Check the **Arduino** tab first: is a port listed and marked `>>`? If nothing is listed, it is a cable (charge-only), a missing driver (CH340), or a board that is not a 32U4. |
| No COM port appears at all | Install the driver (§2.1). Cheap clones usually need CH340. |
| Upload fails / board vanishes mid-upload | Bootloader window — see §2.2: click Upload, then double-tap `RST`. Try a USB 2.0 port. |
| App will not start / closes instantly | Check `logs\error.log` for a `ConfigException` naming the field. A hand-edited `defaults.yaml` or `local.yaml` with a missing or malformed value is the usual cause. |
| Capture shows the launcher, or a partly black frame | The capture reads the screen, so the game must be visible. Move the launcher off it. `PrintWindow` returns a black frame for this game — that is measured, not a bug. |
| Everything is offset after moving to a new monitor/resolution | Expected. Open **Setup**, compare, and recalibrate. |
| A tool start did nothing at all | Most often the game is not focused, or a coordinate is wrong — and those look identical, because the transaction is fire-and-forget. Check the **Arduino** light and the tab's **Dry run** first. |
| Hotkeys do nothing while playing | Expected — the game's anti-cheat blocks background key reads. Click the launcher first. See [USER_GUIDE.md](USER_GUIDE.md#hotkeys-tab). |
| Windows Defender / SmartScreen warning on first run | Unsigned build. "More info" → "Run anyway", or run it from a folder you have excluded. |

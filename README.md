# Seal Online Automation Tools

A bundle of **GameGuard-safe** automation tools for Seal Online (希望戀曲 / 希望Online TW), driven by an **Arduino Pro Micro** acting as a real USB mouse + keyboard.

> **Why an Arduino?** Seal Online uses GameGuard, which blocks synthetic input from software (`SendInput`, `SetCursorPos` while focused). An Arduino Pro Micro (ATmega32U4) enumerates as a *genuine USB HID device*, so its clicks/moves are indistinguishable from a real human — GameGuard cannot block them.

---

## What's Included

| File | Purpose |
|------|---------|
| `launcher.py` + `launcher.html` | **Unified web panel** (:5002) to load/start/stop/quit all tools |
| `tuner/seal_tuner.py` | **Magic Tuner** — auto-click 發條 tuning, OCR grade/attr reading, auto-stop at target grade + filter |
| `gem_composer/gem_composer.py` | **Gem Composer** — auto gem-combine via calibrated Arduino movements |
| `skill_spammer/skill_spammer.py` | **Skill Spammer** — auto-press skill keys at a fixed interval |
| `tuner/attr_matcher.py` | Attribute name matching (OCR → standard names) + filter logic |
| `tuner/ocr_engine.py` | RapidOCR (ONNX) engine for reading Chinese game text |
| `tuner/web_panel.py` | Tuner's own Flask panel (:5000) |
| `tuner/config.yaml` | Tuner config (filter rules, target grade, timing) |
| `gem_composer/gem_composer_config.yaml` | Gem composer config (grade positions, movement deltas) |
| `skill_spammer/skill_spammer_config.yaml` | Spammer config (keys, interval) |
| `docs/ATTRIBUTE_REFERENCE.md` | Full attribute list + probabilities per grade |
| `docs/CALIBRATION.md` | **How to re-tune screen coords on another PC** (OCR region + gem composer) |
| `arduino/seal_mouse/seal_mouse.ino` | Arduino firmware (upload this) |
| `requirements.txt` | Python dependencies |
| `setup.bat` | One-click Windows setup |

## Project Layout

```text
seal-tools/
├── launcher.py, launcher.html    ← entry point (web panel :5002)
├── tuner/                        ← Magic Tuner + shared OCR/attr/web modules
├── gem_composer/                 ← Gem Composer
├── skill_spammer/                ← Skill Spammer
├── macro_recorder/               ← mouse macro recorder
├── checkin/                      ← daily check-in
├── tests/                        ← batch test runner
├── arduino/                      ← Arduino Pro Micro firmware
├── docs/                         ← attribute reference, rankings, notes
└── reference/                    ← local calibration screenshots (not tracked)
```

---

## Hardware Required

- **Arduino Pro Micro (ATmega32U4)** — the only board that works (must support native USB HID). *A plain Arduino Uno/Nano will **not** work* (no HID support).
- A USB cable to connect it to the PC.
- **Windows 10/11** (the tools use `ctypes.windll`, `winsound`, `taskkill` — Windows-only).
- **Python 3.12 (64-bit)** — newer 3.13+ isn't supported by the OCR dependency (`onnxruntime`) yet. 3.10–3.12 should work.

### Software dependencies (`requirements.txt`)

Installed automatically by `setup.bat` (or `pip install -r requirements.txt`):

| Package | What it's for |
|---------|---------------|
| `flask` | Launcher web panel (:5002) + tuner panel (:5000) |
| `pyyaml` | Reads/writes all the YAML config files |
| `pyserial` | Talks to the Arduino over the COM port |
| `mss`, `opencv-python`, `numpy`, `Pillow` | Tuner screen capture + OCR image processing |
| `rapidocr-onnxruntime` | The OCR engine (reads Chinese game text) |
| `playwright` | Check-in browser automation |

> ⚠️ **On a fresh Windows PC**, the OCR engine (`rapidocr-onnxruntime` → `onnxruntime`) may fail to load with `ImportError: DLL load failed while importing onnxruntime_pybind11_state`. That's a **missing Microsoft Visual C++ Redistributable**, not a Python problem. Install it once: <https://aka.ms/vs/17/release/vc_redist.x64.exe>, then re-run `setup.bat` (which now detects this and tells you).

> `playwright` also needs its browser downloaded once: `python -m playwright install chromium` (`setup.bat` does this for you).

---

## Quick Start

1. **Upload the firmware** (see below).
2. **Install dependencies** — double-click `setup.bat`, or:
   ```bash
   python -m pip install -r requirements.txt   # or: py -m pip install -r requirements.txt
   ```
3. **Configure** each tool (see Configuration).
4. **Run the launcher**:
   ```bash
   python launcher.py        # or: py launcher.py
   ```
5. Open **http://127.0.0.1:5002** and control all three tools from one panel.

## Setup for friends (dumb-proof)

Hand this section to a friend who has never touched Python or an Arduino. Do every step in order.

### What they need
- A **Windows 10/11** PC.
- An **Arduino Pro Micro (ATmega32U4)** — this exact board. Uno / Nano will **not** work.
- A **USB cable that carries data** (a charge-only cable won't work).
- Internet access.

### 1. Install Python
1. Go to <https://www.python.org/downloads/> and download **Python 3.12 (64-bit)** for Windows.
2. Run the installer. **Tick "Add Python to PATH"**, then click **Install Now**.
3. Check it worked: open Command Prompt and type `python --version` — you should see a version number.
   - **If `python` does nothing (or opens the Microsoft Store) but `py` works — that's normal for some installs.** Just use `py` everywhere instead of `python` in the steps below. `setup.bat` handles this automatically.

### 2. Get the tools
1. Download this repo (green **Code → Download ZIP** button on GitHub).
2. Unzip it to a simple path, e.g. `C:\seal-tools`.

### 3. Install the dependencies
1. Double-click **`setup.bat`** inside the folder. It installs everything and reports whether the Arduino is detected.
   - (Manual alternative: open a terminal in the folder and run `py -m pip install -r requirements.txt`.)

### 4. Put the firmware on the Arduino
1. Install the **Arduino IDE** from <https://www.arduino.cc/en/software>.
2. Plug the Pro Micro into the PC.
3. Follow the **"Arduino IDE — step-by-step"** section above: pick **Leonardo / Micro** (or **SparkFun Pro Micro**), choose the COM port, open `arduino/seal_mouse/seal_mouse.ino`, and click **Upload**.

### 5. Start the launcher
1. In the folder, open a terminal and run:
   ```bash
   py launcher.py         # or: python launcher.py
   ```
   (If `python` doesn't work for you, use `py` — see the note above.)
2. Open **http://127.0.0.1:5002** in a browser. You'll see three cards: **Magic Tuner**, **Gem Composer**, **Skill Spammer**.

### 6. Use a tool (the important part)
- **Only one tool runs at a time** — they share the same Arduino.
- Each card has **Load → Start → Stop → Quit**.
- **Position the mouse where you want it to act *before* you press Start.** Once the game is focused, its anti-cheat (GameGuard) blocks the software from moving the cursor — so you must hover the mouse yourself first.

| Tool | Get ready | Hotkeys |
|------|-----------|---------|
| **Magic Tuner** | Open the 發條 (tuning) window and hover the mouse over the confirm button | **F12** start/stop · **F11** quit |
| **Gem Composer** | Open the gem-combine window — **needs custom calibration (see note)** | **F12** start/stop · **F9** next grade · **F11** quit |
| **Skill Spammer** | Set the keys in `skill_spammer/skill_spammer_config.yaml` | **F12** start/stop · **F11** quit |

> ⚠️ **Gem Composer will likely not work on a fresh machine.** It uses hardcoded screen coordinates, so it depends on the exact **monitor resolution, display scale/ratio, and game window size**. You must re-measure the coordinates in `gem_composer/gem_composer_config.yaml` for the specific setup (see the Configuration section) — otherwise the clicks land in the wrong place. **The same applies to the Tuner's OCR capture region.** Full steps to re-tune both on a new PC: **`docs/CALIBRATION.md`**.

### If it doesn't work
- **"Arduino not found"** → the USB cable is probably charge-only, or the board isn't a **Pro Micro (ATmega32U4)**.
- **Tool crashes right after Load** → another tool is still holding the COM port. Click **Quit** on the other card and retry. See `tool_error.log` for details.
- **Nothing clicks in-game** → the game window must be focused, and the mouse must be positioned before you press Start.
- **OCR reads garbage** → adjust the capture region in `tuner/ocr_engine.py` for your screen resolution.

---

## 1. Set Up the Arduino Pro Micro

### Arduino IDE — step-by-step

1. **Install the Arduino IDE** — download from <https://www.arduino.cc/en/software> (2.x is fine).
2. **Add board support** (the Pro Micro is not in the default board list):
   - *Easiest, no extra install:* select **Arduino Leonardo** or **Arduino Micro** — same ATmega32U4 chip.
   - *Or add SparkFun:* **File → Preferences → "Additional Boards Manager URLs"** → paste the SparkFun JSON URL, then **Tools → Board → Boards Manager** → search **"SparkFun AVR Boards"** → **Install**.
3. **Select the board** — **Tools → Board** → **Arduino Leonardo / Micro**, or **SparkFun Pro Micro** (processor **ATmega32U4 (5V, 16 MHz)**).
4. **Select the port** — **Tools → Port** → choose the COM port (e.g. `COM5`).
5. **Open the firmware** — **File → Open** → `arduino/seal_mouse/seal_mouse.ino`.
6. **Upload** — click the **→ Upload** arrow (or **Sketch → Upload**). If it fails, see the bootloader-reset note below.
7. **Verify** — open **Tools → Serial Monitor** at **115200**, type `C`, press Enter → a left-click fires.

### Which board to get

- **Arduino Pro Micro (ATmega32U4), 5V / 16 MHz** — this is the one you want.
- Avoid the **3.3V / 8 MHz** variant (slower; some clones misreport).
- **Uno / Nano / Mega will NOT work** — they have no native USB HID.

### 1.1 Plug it in + install drivers

1. Plug the Pro Micro into a USB port.
2. Windows usually installs the driver automatically — it appears as a **COM port** (e.g. `COM5`).
3. If no COM port shows up:
   - Install the Arduino USB driver: `C:\Program Files (x86)\Arduino\drivers\arduino.inf` (right-click → Install), **or**
   - Install the **CH340** driver if your clone uses a CH340 USB-serial chip (very common on cheap Pro Micro clones).

### 1.2 Add the board in Arduino IDE

The Pro Micro is **not** in the default Arduino AVR boards. Two options:

**Option A (easiest) — use "Arduino Leonardo"**
Many Pro Micro clones ship with the Leonardo/Micro bootloader. In the Arduino IDE:
- **Tools → Board → Arduino AVR Boards → Arduino Leonardo** (or **Arduino Micro** — same ATmega32U4 chip).

**Option B (proper "SparkFun Pro Micro")**
1. **File → Preferences →** add to "Additional Boards Manager URLs":
   ```
   https://raw.githubusercontent.com/sparkfun/Arduino_Boards/main/IDE_Board_Manager/package_sparkfun_index.json
   ```
2. **Tools → Board → Boards Manager** → search **"SparkFun AVR Boards"** → **Install**.
3. Select **Tools → Board → SparkFun AVR Boards → SparkFun Pro Micro**, processor **ATmega32U4 (5V, 16 MHz)**.

### 1.3 Upload the firmware

1. Open `arduino/seal_mouse/seal_mouse.ino`.
2. Set the correct **Port** (e.g. `COM5`).
3. Click **Upload** (the → arrow).

### 1.4 If upload fails / board not detected (bootloader reset)

The Pro Micro's bootloader only listens for **~8 seconds** after a reset. To re-enter bootloader:

1. Click **Upload** first, then
2. **Quickly tap the `RST` (reset) button twice** (or short RST to GND twice on boards without a button), then the upload proceeds.

Still failing? Try:
- A **different USB cable** — many are charge-only with no data lines.
- A **USB 2.0 port** (some USB 3 hubs are flaky with the 32U4 bootloader).

### 1.5 Verify it works

After upload, open the Arduino serial monitor at **115200** and send `C\n` — a left-click should fire. The tools auto-detect the board by USB VID `0x2341`; `setup.bat` reports whether it's found.

### Serial protocol (for reference)

The firmware listens on **115200 baud** and parses newline-terminated commands:

| Command | Meaning |
|---------|---------|
| `C` | Left click |
| `R` | Right click |
| `E` | Enter key |
| `T` / `S` | Tab / Space |
| `K n` | Number key `0-9` |
| `F n` | Function key `F1-F10` |
| `D dx dy` | Straight-line mouse move |
| `H dx dy ms` | Human-like (Bezier) move |
| `X` | Alt+Tab |
| `W ms` | Wait (milliseconds) |

All input has randomized micro-delays (human-like). The tools auto-detect the Arduino by USB VID `0x2341`.

---

## 2. Configuration

All config is **YAML**, reloaded live by the tools (so you can edit while running).

### `config.yaml` — Magic Tuner

```yaml
arduino_port: COM5
ocr:                          # OCR capture box (pixel coords - measure per PC, see docs/CALIBRATION.md)
  region: {left: 1140, top: 840, width: 300, height: 320}
  grade_area: {x1: 149, y1: 1, x2: 232, y2: 44}
  grade_y: [1, 44]
  attr_y: [42, 140]
  remaining_y: [190, 235]
  row_height: 25
target_grade: DG              # stop when this grade is reached (N/G/DG/XG/SG)
max_retries: 999999           # safety cap on rolls
save_captures: true

timing:
  click_enter_delay: 0.4      # seconds between click and Enter
  ocr_delay: 0.8              # seconds to wait for OCR

filter:
  enabled: true
  match_mode: per_attr        # any | all | per_attr
  require_grade: DG           # filter only applies at/above this grade
  rules:                      # main goal — must ALL be satisfied
    - name: 減少傷害
      count: 2                # need 2 slots matching this attr
      min: 1
      max: 1
  override_rules:             # "too good to miss" — instant stop if ANY matches
    - name: 增加傷害
      count: 3
      min: 1
      max: 1
```

- `rules` = the goal you're rolling for. `count` = how many of the 3 slots must match that attribute.
- `override_rules` = alternative rolls worth keeping immediately.
- Attribute names must match those in `ATTRIBUTE_REFERENCE.md`.
- **`ocr:` (the first block) is the machine-specific part — re-measure it on any new PC.** Full steps: [`docs/CALIBRATION.md`](docs/CALIBRATION.md).

### `gem_composer_config.yaml` — Gem Composer

```yaml
start_grade: N                # N / G / DG — which grade radio to start on

grade_positions:              # absolute screen coords (pixels)
  N: [727, 696]
  G: [777, 696]
  DG: [827, 696]
  Register: [835, 724]
  Combine: [785, 871]

movements:                    # relative mouse deltas (Arduino D commands)
  radio_to_register:
    N: [65, 20]
    G: [35, 20]
    DG: [10, 20]
  register_combine: [-30, 80]
  combine_register: [30, -80]
```

> ⚠️ **Gem Composer will likely not work as-is on a different machine.** `grade_positions` and `movements` are hardcoded for a specific **monitor resolution, display scale/ratio, and game window size**. Re-measure them for your setup before using the tool — otherwise the clicks will land in the wrong place.

### `skill_spammer_config.yaml` — Skill Spammer

```yaml
keys: ["F2", "5", "6"]        # keys to press in sequence
interval: 1.0                 # seconds between presses
```

---

## 3. Using the Launcher

The launcher (http://127.0.0.1:5002) shows one card per tool, each with:

| Button | Action |
|--------|--------|
| **Load** | Launch the tool's Python process (shows "IDLE") |
| **Start** | Begin the automation (shows "ON", live stats) |
| **Stop** | Pause the automation |
| **Quit** | Stop + kill the process |

**Only one tool runs at a time** — they share the single Arduino COM port. Loading a new tool automatically kills the previous one.

---

## 4. Tools & Hotkeys

### Magic Tuner (`tuner/seal_tuner.py`)
Auto-clicks the 發條 confirm button + Enter, OCR-scans the grade and 3 attribute lines, and auto-stops at `target_grade` + filter match.

- **F12** — start / stop
- **F11** — quit
- Mouse must be **positioned over the confirm button before pressing F12** (GameGuard blocks cursor moves while the game is focused).

### Gem Composer (`gem_composer/gem_composer.py`)
Auto-combines gems using calibrated `D` movements. `cycle` counter climbs live in the panel.

> ⚠️ Requires custom calibration — the coordinates in `gem_composer/gem_composer_config.yaml` are specific to a particular monitor resolution, display ratio, and game window size. Likely won't work on a different machine without re-measuring.

- **F12** — start / stop
- **F9** — advance grade (N → G → DG)
- **F11** — quit

### Skill Spammer (`skill_spammer/skill_spammer.py`)
Presses the configured keys in a loop.

- **F12** — start / stop
- **F11** — quit

---

## 5. Troubleshooting

| Symptom | Fix |
|---------|-----|
| `[!] Arduino not found` | Check USB cable; confirm board is a **Pro Micro/Leonardo** (ATmega32U4), not Uno/Nano. |
| Tool crashes on Load | Read `tool_error.log` in the tool folder. Common cause: another tool/process holding the COM port. |
| Tool "ends immediately" after Load | A stale `control.txt` left a `quit` command. The launcher clears it automatically; you can also delete `control.txt` / `gem_control.txt` / `spammer_control.txt`. |
| Zombie process / COM port busy | Quit (not just Stop) to kill the full process tree, or run `taskkill /F /IM python.exe` in a worst case. |
| OCR reads garbage / wrong grade | Adjust the capture region in `tuner/ocr_engine.py` for your resolution/DPI. |
| Tool crashes on Load with `DLL load failed ... onnxruntime_pybind11_state` | Missing Microsoft **Visual C++ Redistributable**. Install <https://aka.ms/vs/17/release/vc_redist.x64.exe>, then reinstall: `py -m pip install --force-reinstall rapidocr-onnxruntime`. |
| Nothing clicks in-game | Game window must be focused; the Arduino must be the active HID device. |

### Manual fix: `onnxruntime` "DLL load failed" (missing Visual C++ runtime)

On a brand-new Windows PC the OCR engine may crash as soon as a tool loads, with an error in `tool_error.log` ending in:

```
ImportError: DLL load failed while importing onnxruntime_pybind11_state: 找不到指定的模組
```

(`找不到指定的模組` means "The specified module could not be found".) This is **not** a Python-version problem — it's a missing **Microsoft Visual C++ Redistributable**, which `onnxruntime`'s native library needs. The package is installed; the runtime DLL (`vcruntime140.dll`, `msvcp140.dll`) is not.

`setup.bat` fixes this automatically (step `[3/5]`). If you need to do it by hand:

1. **Download** the 64-bit redistributable:
   ```
   https://aka.ms/vs/17/release/vc_redist.x64.exe
   ```
2. **Run it** and click **Install**. (If it won't install, right-click → **Run as administrator**.) No reboot needed.
3. **Verify** it worked:
   ```
   py -c "import onnxruntime; print(onnxruntime.__version__)"
   ```
   A version number (e.g. `1.28.0`) means it's fixed. The same DLL error means it isn't — see step 4.
4. **If it still fails**, `onnxruntime` may be a half-installed leftover from an interrupted `pip install`. Reinstall it cleanly:
   ```
   py -m pip uninstall -y onnxruntime rapidocr-onnxruntime
   py -m pip install --force-reinstall rapidocr-onnxruntime
   ```
5. **Reload** the tool — it should now start.

> Note: use `py` in the above if `python` doesn't work on your machine (see the note in the setup section).

---

## 6. Disclaimer

These tools automate a game client and are intended **only for your own accounts and your own machine**, for personal convenience. Automation may violate the game's Terms of Service and may result in account penalties. Use at your own risk.

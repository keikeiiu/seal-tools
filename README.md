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
- **Python 3.10+**.

---

## Quick Start

1. **Upload the firmware** (see below).
2. **Install dependencies** — double-click `setup.bat`, or:
   ```bash
   pip install -r requirements.txt
   ```
3. **Configure** each tool (see Configuration).
4. **Run the launcher**:
   ```bash
   python launcher.py
   ```
5. Open **http://127.0.0.1:5002** and control all three tools from one panel.

---

## 1. Set Up the Arduino Pro Micro

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

> These coordinates are **resolution/DPI-specific**. If the game window size differs from the original calibration, re-measure them.

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
| Nothing clicks in-game | Game window must be focused; the Arduino must be the active HID device. |

---

## 6. Disclaimer

These tools automate a game client and are intended **only for your own accounts and your own machine**, for personal convenience. Automation may violate the game's Terms of Service and may result in account penalties. Use at your own risk.

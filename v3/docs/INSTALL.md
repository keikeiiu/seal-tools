# Seal Tools v3.0 — Install Guide

From nothing to a working install: the board, the app and the first calibration. If you already have it
running and want to know what a button does, that is [USER_GUIDE.md](USER_GUIDE.md). If you are curious
*why* calibration is mandatory, the repository's `v3/docs/spec/CALIBRATION.md` explains the coordinate
contract. (The zip ships only these guides, `THIRD-PARTY-NOTICES.md` and `docs/pet-data.csv`, not the
spec tree.)

**Nothing in v3.0 needs a reflash.** The Arduino sketch is unchanged since v2.15 (firmware level **2**):
v3.0 publishes the same `seal_mouse.ino` in `SealTools-v3.0-firmware.zip`, so a board already flashed for
v2.15 or later keeps working. The firmware zip exists for a **new board**, not for an update.

**The calibration is the long part**, and it is the part you cannot skip. Everything before it (flashing
a new board, unzipping, first launch) is a few minutes.

---

## What you need

| | |
|---|---|
| **PC** | Windows 10 or 11, 64-bit. |
| **Board** | An **Arduino Pro Micro (ATmega32U4, 5 V / 16 MHz)** — the only board that works. Uno/Nano/Mega have no native USB HID and cannot present as a mouse. |
| **Cable** | A USB cable that carries **data**. Charge-only cables are the most common reason a board is never found. |
| **Game** | Seal Online (TW or US client), running **windowed**. |
| **Rights** | None special: no installer, no `.NET` runtime, no administrator elevation, and every file the app writes lives inside its folder (`config/` and `logs/`). |

> **Why the Pro Micro.** The game's anti-cheat blocks synthetic input from software (`SendInput`, and
> `SetCursorPos` while the game is focused). The Pro Micro has a native USB controller and enumerates as
> a *real* USB HID mouse and keyboard, so its input is indistinguishable from a human's at the driver
> level. That is the entire reason for the board.

---

## 1. Download and unzip

From the release page, pick the app zip — and the firmware zip **only if your board is new**:

| File | What it is | When you want it |
|---|---|---|
| `SealTools-v3.0.zip` | The app: shell, core, OCR models, config templates and these docs. | Always. |
| `SealTools-v3.0-firmware.zip` | The Arduino sketch, `seal_mouse\seal_mouse.ino` (unchanged). | Only to flash a board that is not already running the v2.15+ firmware. |

There is no installer and no `-local` zip in v3.0: the app zip carries **program files, templates and the
guides only** — never a real `local.yaml`. Your settings live in `config/local.yaml`, a file the zip never
contains, so unzipping an update cannot cover them.

Unzip the app zip anywhere — Desktop, `C:\Games\SealTools`, a USB stick. There is no installation step
and nothing is registered with Windows. One top folder appears:

```
SealTools-v3.0\
  SealTools.exe                 the window you run
  seal-core.exe                 the background core SealTools.exe starts
  DirectML.dll                  the OCR runtime's GPU-side library
  flutter_windows.dll           the shell's Flutter runtime
  data\                         the shell's Flutter assets
  models\                       the three OCR model files
    ch_PP-OCRv4_det_infer.onnx
    ch_PP-OCRv4_rec_infer.onnx
    ch_ppocr_mobile_v2.0_cls_infer.onnx
  config\
    defaults.yaml.example       the shipped template (base config)
    local.yaml.example          the first-run seed for YOUR file
    attributes.yaml             the TW OCR dictionary (release-maintained)
    attributes.us.yaml.example  the seed for the US dictionary
  docs\
    pet-data.csv                the pet feeding table the feeder reads
    USER_GUIDE.md               this app's user guide
    USER_GUIDE.zh-TW.md         the same guide in 繁體中文
    INSTALL.md                  this file
    THIRD-PARTY-NOTICES.md      the bundled components' licences and terms
```

Keep the files together: `SealTools.exe` finds `seal-core.exe` **beside itself** (never on `PATH`), and
the core finds `config/` in that same folder and the models under `models/`. Moving a single file out of
the folder is the usual cause of "the core is not running" or `ocr_unavailable`.

---

## 2. Flash the Arduino (new board only)

Skip this section if your board already runs the v2.15+ firmware; the firmware is unchanged and no
reflash is needed for v3.0.

### 2.1 Install the IDE and board support

1. Install the **Arduino IDE** — <https://www.arduino.cc/en/software> (2.x is fine).
2. The Pro Micro is not in the default board list. The easiest route: `Tools → Board → Arduino AVR
   Boards → **Arduino Leonardo**` (or **Arduino Micro** — same ATmega32U4 chip; most clones ship a
   Leonardo/Micro-compatible bootloader).
   The proper route: add SparkFun's board URL in `File → Preferences → Additional Boards Manager URLs`,
   install **SparkFun AVR Boards**, select **SparkFun Pro Micro**, processor **ATmega32U4 (5 V,
   16 MHz)**.
3. Plug the board in and select its **port** under `Tools → Port` (e.g. `COM5`). If no port appears,
   install the driver: `arduino.inf` from the Arduino install directory, or the **CH340** driver if
   your clone uses a CH340 USB-serial chip.

### 2.2 Upload

1. Unzip the firmware zip, then `File → Open` → `seal_mouse\seal_mouse.ino`. Open it from inside its
   folder: the IDE requires a sketch to sit in a directory of the same name, which is why the zip keeps
   `seal_mouse\`.
2. Confirm the **board** and **port**, then click **Upload**.

If the upload fails: the Pro Micro's bootloader only listens for about **8 seconds** after a reset.
Click **Upload first**, then quickly **tap `RST` twice** (or short RST to GND twice on boards with no
button). If it still fails, try a different data cable, then a USB 2.0 port — some USB 3 hubs are
unreliable with the 32U4 bootloader.

### 2.3 Verify the board before going further

Arduino IDE → `Tools → Serial Monitor`, baud **115200**, type `C` and press Enter: **a left click should
fire**. Do this now; it separates "the board is wrong" from "the app is misconfigured", and those two
look identical later.

---

## 3. First run

Run **`SealTools.exe`**. There is no installer and no administrator requirement, and nothing is written
outside the folder (its `config/` and `logs/`). The shell starts `seal-core.exe` for you and connects;
you never start the core by hand.

On first launch the core creates **`config/local.yaml`** by copying `local.yaml.example` (only when the
file is absent, atomically). That seed contains **placeholder measurements from an old setup**; they
will be wrong on your machine and the tools will refuse until you calibrate. That is expected — step 5
replaces them.

What the first launch may write, and nothing else:

- `config/local.yaml` — your machine's state (the only file a save rewrites; gitignored, never in the
  zip, kept on update).
- `config/attributes.<variant>.yaml` — seeded from its `.example` when the US variant is first used and
  the file is missing (the TW dictionary ships maintained).
- `logs/` — created when the first run writes a log.

Nothing else: no registry keys, no services, no tray icon, no update check or network feature, and the
game is never modified (input comes from the board; reading is screen capture and OCR).

If the whole window says **"The core is not running"**, the core could not start. The window shows the
reason and the core's **stderr tail** — the usual causes are a moved/renamed core, a locked antivirus
quarantine, or a `config/` that cannot be read. The full list is in **Troubleshooting** below.

---

## 4. Configure

Open **Setup**. The client variant, the Arduino IDs and the port are editable and saved with **Save**;
the game window title is **read-only** and is set with the actions below.

| Field | What to put there |
|---|---|
| **Game window title** | **Read-only.** The game window's title (matching is a **substring**, so the distinctive part is enough). It is set with **Find window**, then **Use detected title** — the title identifies the window, so it is not typed. |
| **Client variant** | `tw` or `us` (or a variant your config defines). The variant supplies the window title **and** the attribute dictionary the OCR matches against; a wrong variant is "game window not found" or "the tuner matches nothing". |
| **Arduino VID / PID list / Baud** | How the board is found when no port is named. The shipped values fit the Pro Micro; change only for a different board. |
| **COM port override** | Optional — leave empty to find the board by IDs. A named port that does not exist is a **refusal**, not a silent fallback. |

Then use the actions:

- **Find window** — does the core see the game, and what title does it report?
- **Measure** — reports the frame and client rectangles it measured.
- **Probe board** — opens the board and reports its firmware reply. This is the same probe as the
  **Arduino** tab.
- **Use detected title** — saves the title the last Find detected (enabled only after a successful Find
  with a non-empty title; this is the one action that writes config, through the normal save path).

Press **Save** on Setup. A variant switch is resolved, validated, its dictionary loaded, and only then
written; it cannot half-switch.

---

## 5. Calibrate

Open **Calibrate**. This is the one surface for every tool's marks. The short version:

1. Put the game window in front and press **Capture**. The window blinks: the shell hides itself for
   one frame so the capture is of the game, then reopens. A frame and the measured display summary
   appear.
2. Choose a **Kind** from the grouped picker. For a named-instance kind choose a **Target instance** too
   (Save stays disabled until you do).
3. Draw the mark on the picture (click = point, drag = rectangle) or type **x / y / w / h**; the fields
   are authoritative.
4. For **Result gem area** answer the empty-result question first — is the result box empty right now?
   `empty: true` saves the crop and samples the signature; `false` samples the signature only.
5. **Save mark**; the stored mark, derived pitch (bag grid) and artifact crop appear below.
6. **Refresh stored** re-reads what the core holds.

Which marks to draw depends on the tools you will use — the picker groups them:

- **Shop** (Buy/Sell): Bag grid, Bag slot, Shop list region, Focus / scroll point, MAX button.
- **Gem** (composer): Result gem area (with the empty answer), Grade point (targets N, G, DG, Register,
  Combine), Resource slot point (targets "1".."3").
- **Pet** (feeder): the menu/feed/close points, Bag page tab (ITEM1..3), Boarding bag grid/slot, and the
  per-row marks Row start/end button, Row boarding pet slot and Row food strip (targets "1".."4").
- **Tuner**: Spring point only.

**Calibration prerequisites**

- Turn **off** Windows' *Enhance pointer precision* (`Settings → Bluetooth & devices → Mouse →
  Additional mouse options → Pointer Options`) before using hand-tuned relative moves (the Gem
  Composer's `tuned` move mode). It changes how far a relative move travels.
- The marks are **physical pixels relative to the game window's client area**; they do not travel
  between machines, display scales, game window sizes or screens. After any of those change,
  recalibrate — v3.0 does not yet show an automatic "your display moved" check.
- The tuner's OCR region/bands and the tooltip panel are **not** drawable in v3.0 (they need
  session-shaped calibration); they stay hand-edited in `config/local.yaml` under `tuner.ocr` /
  `tooltip`. The pet's reference crops are also not drawable marks yet, but the **Pet** tab edits the
  rows and food cells the config already carries once its boarding bag grid/slot are calibrated, and
  the **Buy/Sell** tab's Sell section has its own bag-cell grid that saves `sell_slots`. See the
  guide's sections 5.4 and 5.6.

**Do one short run before trusting anything.** Press **Probe board** on Arduino/Setup, then start the
shortest run you have configured on the Cards page — a Spammer run after adding a preset on the
Spammer tab, or the tool you just calibrated — watch the card, and press **Stop**. Then read
`logs/<tool>.log` (the status strip prints the root folder) — it proves what the run actually did.
Starting a second tool while one runs is refused by design; the Pet Feeder is the resident exception.

---

## Migrating from v2

The formats are unchanged: **migration is a copy, there is no converter.** Keep your v2 folder until the
v3 side is verified.

1. Unzip v3.0 somewhere and run it once so the folder exists (it will create a seed `local.yaml` — that
   seed is replaced in step 2).
2. Close Seal Tools. Then copy into `SealTools-v3.0\config\`:
   - `local.yaml` — your machine's state: calibration, presets, pet rows, quest flows and so on.
   - `calib_gem_result.png` — **the only calibration image v3 consumes** (the result-gem crop for the
     empty test). Other `calib_*.png` files beside it are v2's drawn-over screenshots: copying them is
     harmless history, but no v3 tool reads them.
   - Your attribute files if you edited or grew them: `attributes.yaml` (TW) and/or
     `attributes.us.yaml` (US). Do **not** copy `attributes.us.yaml.example`.
   - Optionally `defaults.yaml`, **only if your v2 folder predates v2.16 and you had edited portable
     settings in its template**. The core reads it once as the legacy template and folds your changed
     leaves into `local.yaml` (the shipped `defaults.yaml.example` stays the base). A newer v2 install
     already carries those values inside `local.yaml`, so leave it there; if you never edited the
     template, skip it.
3. Start `SealTools.exe`, open **Setup**, and confirm the title/variant and **Probe board**; open
   **Calibrate → Refresh stored** and confirm your marks are there (the same machine and display keep
   them valid). Nothing needs reflashing — the firmware is unchanged.
4. Keep the v2 folder until a run has gone as expected in v3; then delete it, or keep it as a backup.

If you move to a **different machine or display**, copy the same files but recalibrate: the stored marks
are physical client-relative offsets.

---

## Updating

1. Close Seal Tools.
2. Back up `config/local.yaml`, `config/attributes*.yaml` and `config/calib_gem_result.png` (plus any older
   `calib_*.png` you want to keep; v3 reads only the result crop) if you want a safety copy.
3. Unzip the new release. You can replace the program files in the existing folder or use a fresh folder
   and copy your `config/` files across.
4. Start it. **The zip never contains `local.yaml`**, so an update cannot overwrite your settings; the
   only files it replaces are program files and the shipped templates/dictionaries. If a release changes
   a default, the core's once-only fold carries your old stated values into `local.yaml` (see the
   repository's `v3/docs/spec/CONFIG.md`).
5. There is no auto-update and no installer: replacing files is the whole update.

## Uninstalling

Close Seal Tools and delete its folder. Nothing was installed: no registry keys, no services, no
Program Files data, no user-profile data outside the folder. The Arduino keeps its firmware — flashing
is independent of the app — and your `config/local.yaml` leaves with the folder, so copy it out first if
you want to keep your calibration.

---

## Troubleshooting

| Symptom | What it means / what to do |
|---|---|
| **"The core is not running"** with a **Stderr tail** | `seal-core.exe` could not start or exited. Read the tail: a boot refusal (missing/unreadable config, an invalid merged config, a dictionary that cannot load) prints there, not as a protocol message. **Restart core** retries. Common causes: `seal-core.exe` moved out of the folder, antivirus blocked it, or `config/` is unreadable. |
| `bad_version` | The shell and core are from different versions. Replace **both** files from the same zip; Restart will not fix a mismatch. |
| `game window not found` | Wrong variant, or a title that no longer matches the game (the title is read-only — use **Find window**, then **Use detected title**), or the game is not running/windowed/restored. Check **Setup** and press **Find window**. |
| `game_not_foreground` | Something is in front of the game. Bring the game forward; the Calibrate capture hides the shell itself for one frame. |
| `ocr_unavailable` | The OCR pipeline cannot serve this read on this machine. `detail.reason` is the code's reason for a pipeline that could not be built — one of `models_env_unset`, `models_dir_missing`, `text_fixes_invalid`, `model_load_failed`, `runtime_failed` — and `detail.message` names what failed (for a missing-models failure it lists **every directory tried**). A failure while scanning an already-built pipeline (`bad_image`, `det_output_shape`) arrives as `internal` with the OCR line. Order for models: `SEAL_OCR_MODELS` if set (it wins or fails loudly), then `<root>/models`, then `<seal-core.exe folder>/models`; a directory counts only with all three `.onnx` files. Restore the shipped `models/`. |
| `board_unavailable` | No board on the configured port, or the port cannot be opened. Check the data cable, the USB driver (CH340 clones), the **VID/PID/port** on Setup, and press **Probe board**. Only one program may hold the COM port. |
| A card line saying a mark isn't calibrated | The line names the **mark** to save and the single **Calibrate** tab; choose that kind there (with its target instance if the kind is a named one) and save it. |
| Hotkeys do not fire in game | Expected in v3.0: there is no global hotkey poller. Use **Stop** and the shell's controls. |
| “The window blinks when I press Capture” | Expected: one frame of hide so the capture sees the game (v2 blinked ~0.3 s). |
| A run did something unexpected | Read `<root>/logs/<tool>.log`, then `error.log` / `ocr_errors.jsonl` beside it. The status strip prints `<root>`. |
| The config cannot be parsed / boot refusal after hand-editing | Restore your last `config/local.yaml` copy. Deleting it makes the next start seed a fresh one **and loses your calibration and presets** — last resort only. |
| Two copies of Seal Tools | v3.0 has no single-instance guard yet. Run one copy at a time; two cores on one config folder can interleave writes. |

---

## What this guide does not cover

- **The window and the tabs**: [USER_GUIDE.md](USER_GUIDE.md) (and its zh-TW twin).
- **Config files, loading order, the fold**: the repository's `v3/docs/spec/CONFIG.md`.
- **Coordinates, DPI and calibration rules**: the repository's `v3/docs/spec/CALIBRATION.md`.
- **The live not-built list**: the repository's `v3/docs/backlog/OPEN.md`.

# Seal Tools v2 (C# / .NET WPF)

**Version 2.11** (2026-09-21) — the current release; see the tag `v2.11`.

The **version history is not repeated here.** It lives in one place —
[../README.md](../README.md#version-history) has the one-line-per-release table, and
[docs/PROGRESS.md](docs/PROGRESS.md) has the dated reasoning behind each. This file kept its own copy
until v2.11 and the copy had gone eight releases stale, which is the argument for not having two.

A full C#/.NET 8 rebuild of the Seal Online automation tools, replacing the Python build with a
**native Windows desktop app** (WPF + WPF-UI) that ships as a **single self-contained `.exe`**.

> The original Python build lives under `v1/` (`launcher.py`, `tuner/`, `gem_composer/`,
> `skill_spammer/`, `checkin/`), is untouched, and still works. v2 lives entirely under this `v2/` folder.

---

## What it does

Five tools, driven by an Arduino Pro Micro (USB HID mouse/keyboard) over a COM port, plus OCR. The
launcher shows them as **six cards** — Buy Items and Sell Items are one tool with a card each:

| Tool | Card(s) | What it does |
|------|---------|--------------|
| **Magic Tuner** | Magic Tuner | Rolls the 發條 (magic tuning) UI — Arduino click+Enter, OCR reads the grade (N/G/DG/XG/SG) + 3 attribute lines, matches them against a config dictionary, applies filter rules, stops at the target grade. |
| **Gem Composer** | Gem Composer | Clicks the gem-combine UI (N/G/DG radio + Register + Combine) at calibrated points, moving between them with the Arduino (closed loop by default, hand-tuned counts available via `gem.move_mode`). Combines each grade until the result box reads empty, then advances; ends after the last grade. |
| **Skill Spammer** | Skill Spammer | Presses configured keys, each on its own cooldown. |
| **Buy / Sell** | Buy Items · Sell Items | Bulk-buys the items you go through constantly, and sells the bag slots you mark on an 8×8 grid. |
| **Pet Feeder** | Pet Feeder | Keeps a boarded pet fed while you are not watching, reloading the feeder on a per-row schedule. **Resident** — starting another tool leaves it running. |

At most one tool runs at a time (they share the single Arduino COM port) — with one exception: the
**Pet Feeder** is resident. It holds nothing between reloads, so starting another tool leaves it
running, and it waits for the game when a reload comes due while that tool is mid-run. Starting a
second *foreground* tool still stops the first.

---

## Architecture

```
v2/
  SealTools.sln
  SealTools.Core/        # config loader/validator, Arduino serial, Win32 window/DPI, hotkeys, screen capture,
                         # file logging, ToolState, and everything shared: OcrEngine (RapidOCRSharpOnnx),
                         # TextCleaner, IconMatch, PetPanel, FeederLayout/FeederCount/FeederEta, BagGrid
  SealTools.Tuner/       # SealTuner, AttrMatcher
  SealTools.GemComposer/ # GemComposer
  SealTools.Spammer/     # SkillSpammer, HoldSpace
  SealTools.Shop/        # ShopTool (buy + sell)
  SealTools.Pet/         # PetTool (the resident feeder)
  SealTools.Launcher/    # WPF-UI desktop app (the UI + tool lifecycle)
  SealTools.Tests/       # xUnit tests — 172 cases across 17 files
  config/                # defaults.yaml, attributes.yaml, local.yaml.example (+ local.yaml, gitignored)
  models/                # PP-OCRv4 ONNX models (gitignored, copied from rapidocr-onnxruntime)
  docs/INSTALL.md        # install: board, firmware, app, first calibration, updating
  docs/USER_GUIDE.md     # every card, tab and button explained
  docs/CURSOR-INVESTIGATION.md  # why the cursor is placed with the Arduino, not SetCursorPos
  docs/MOVE-SETS.md      # the composer's two move sets: tuned counts vs arduino point placement
  docs/PROGRESS.md       # dated log of what was done and why (append per session)
  docs/DESIGN.md         # how it works and why — the doc a plan's reasoning graduates into
  docs/PET-TAB-DESIGN.md # the Pet tab, the schedule and the reload flow
  docs/ANALYSIS-UI.md    # the launcher UI, measured — the start for a next UI pass
  docs/PLAN-TUNER-SPRING.md  # the tuner spring + cursor guard reasoning (built; shipped in v2.4)
  docs/PLAN-UI-CLEANUP.md    # launcher UI rework — steps 0-9 shipped, shell reverted
  docs/IDEAS.md          # wider backlog: reliability, safety, UX, code health, mini-features
  docs/HANDOVER.md       # prompt for the next session: state, conventions, open threads
  docs/TODO.md           # what is still open
  docs/CALIBRATION.md    # per-machine calibration guide
  docs/COORDINATES.md    # coordinate space, DPI and capture — read before touching them
  docs/archive/          # finished plans, kept for their reasoning
  publish.bat            # build + package the .exe
```

### Key libraries
- **WPF-UI** (`WPF-UI` 4.3.0) — FluentWindow + Mica backdrop + Fluent buttons + dark theme.
- **RapidOCRSharpOnnx** + **Microsoft.ML.OnnxRuntime** + **OpenCvSharp4** — OCR (same PP-OCRv4 ONNX models as v1, so accuracy is identical).
- **YamlDotNet** — config.
- **System.IO.Ports** + **System.Management** — Arduino serial + VID/PID detection.

---

## Design principles (decided during the build)

1. **All config outside code** — no hardcoded values. Everything (OCR geometry, attribute "wordings",
   grade-color thresholds, hotkeys, Arduino VID/PID, timing, filter rules, model paths) lives in YAML.
2. **In-memory control + state** — the tools run in-process in the launcher; control is a
   `CancellationToken`, live status is a shared `ToolState` object. No `control.txt` / `state.json`
   files (those were v1 leftovers for separate processes).
3. **Logs are for future reference only** — the tools `Console.WriteLine` tuning results, and also
   write files to `logs/` (see below). Logs are never read back to control the program.
4. **Windows-native, one .exe** — no Python, no pip, no runtime install. The whole point of the C# rewrite.

---

## Config files

| File | Purpose | Committed? |
|------|---------|-----------|
| `config/defaults.yaml` | Portable defaults: window title, Arduino VID/PID, hotkeys, grade order, grade-color thresholds, timing, filter rules, model paths, reference window size. | ✅ |
| `config/attributes.yaml` | The OCR "wordings": attribute dictionary (name/category/OCR-variants), per-level stats, negative-attr markers, and the full `clean_text` fix table. | ✅ |
| `config/local.yaml.example` | Machine-specific template (OCR region + sub-bands, gem click points, port). | ✅ |
| `config/local.yaml` | Your actual machine coordinates (created by the calibrator). | ❌ (gitignored) |

---

## Calibration workflow (per machine)

The calibrator is in the launcher, split into **one tab per game screen** — five of them:

1. **Calibrate Tuner** — open the 發條 window → **Capture** → **drag a box** around the grade + 3 attribute lines
   → **Check OCR** (verify it reads the correct grade + attributes) → **Save Tuner**.
2. **Calibrate Gem** — open the gem-combine window → **Capture** → **click** N / G / DG / Register / Combine,
   the 3 resource slots, and **drag a box** around the composed result gem → **Save Gem Composer**.
   Composer moves (raw `dx`/`dy`) are saved separately with **Save tuned counts**.
3. **Buy / Sell** — the shop list, the bag grid, the MAX button and the scroll point.
4. **Calibrate Pet** — the 目錄 button, the feed icon, the boarding window's X, the page tabs, the start/end
   toggle, the boarding pet slot, the feeder slots, and the boarding bag's **own** grid.
5. **Calibrate Tooltip** — the hover-panel offset, so the feeder can read a pet's growth and EXP% before
   boarding it.

You only need the tabs for the tools you actually run.

Coordinates are **client-area-relative physical pixels** (game window client top-left = origin). The
process runs **DPI-unaware**, so captures and measurements switch one thread to per-monitor-aware
briefly to get real physical pixels — see `docs/COORDINATES.md` for the full model and the measured
evidence. Relative `D dx dy` moves are **raw hand-tuned HID counts** — not computed from pixel deltas
and never scaled (see prerequisite below). Full step-by-step detail, including the verification
buttons, is in `docs/CALIBRATION.md`.

The **Setup** tab shows the detected environment (monitor size/scale, game window rects) and stores
the scale and reference client size a calibration was measured against; **Detect** fills it in and the
values stay editable.

### Prerequisite: fixed Windows mouse precision (Gem Composer)

The Gem Composer sends relative `D dx dy` moves. Their on-screen distance depends on the **Windows pointer
precision** mapping (raw HID counts → pixels). This title is an OS-cursor game (early Windows, not
raw-input), so the mapping is velocity-dependent while "Enhance pointer precision" is on. For the moves
to be reproducible the pointer must be in a fixed, linear state — same on every run and after a reboot:

1. **Disable acceleration:** `Settings → Bluetooth & devices → Mouse → Additional mouse options →
   Pointer Options →` uncheck **"Enhance pointer precision"**, and leave the speed notch where it is.
2. **Hand-tune the raw `dx`/`dy`** in the Gem calibrate tab's **Composer moves** grid until **Test Move**
   lands on its target, then **Save tuned counts**.

There is no scale factor to measure and none is stored: the raw counts *are* the tuning. (An earlier
build computed moves as `(point − Register) × scale / 100`; that approach was removed because it made the
moves drift with pointer speed.) If pointer acceleration is left on, the composer drifts — fix the
pointer precision, then re-tune the counts. Not a code bug.

### Focus: how the game gets focused (Gem Composer)

The composer does **no focus handling at all** — it mirrors v1's sequence exactly:
`place cursor on grade` + `C` → `D dx dy` + `C`. Focus is a side effect of that, and the lifecycle is:

- **At start the game is unfocused.** You press **Start** in the launcher, so the launcher window is the
  foreground window when the composer begins.
- **The first click focuses it.** That `place cursor on grade` + `C` does both jobs at once: it activates
  the game window *and* presses the grade button. No separate click-to-focus is needed.
- **It stays focused for the rest of the run**, because nothing else is clicked while the tool works.
- **Any later click re-focuses it.** If you click back to the launcher mid-run (e.g. to press **Stop**),
  the game loses focus; the next click on a game button activates it again. Every composer action starts
  with a click on a game control, so the composer never depends on the game *already* being focused.

Guardrails — each of these was tried and is wrong:

- **No dedicated focus-click.** It is redundant and clicks the same button twice (removed in this pass).
- **No Win32 focus API** (`SetForegroundWindow` / `BringWindowToTop`) — v1 never calls one, and neither
  does v2.
- **No centre-click to focus.** A raw-input game captures the cursor and pins the in-game pointer at
  centre, so every later click lands on centre instead of the button.

---

## Logging (for future reference)

Under `logs/`, and **the two groups are in different places** — worth knowing before you go looking:

`logs/` beside `config/` (the app root, found by walking up for `config/defaults.yaml`):

- `run_<timestamp>.jsonl` + `run_<timestamp>.txt` — every tuner attempt (grade, remaining, matched attributes).
- `ocr_log.jsonl` — every OCR scan.
- `ocr_errors.jsonl` — OCR scans that threw, kept apart from the successes.
- `captures/capture_<timestamp>.png` — the OCR capture region (when `save_captures: true`).

`logs/` next to the exe (`AppContext.BaseDirectory`):

- `pet.log` — the feeder's own log, one line per decision.
- `reads/` — the crops the feeder saved for a read, and its scan reports.
- `error.log` — any unhandled startup exception (helps diagnose silent crashes).

In a dev tree these are two different directories (`v2/logs/` and
`SealTools.Launcher/bin/<config>/net8.0-windows/logs/`); in a published build, where the exe sits
beside `config/`, they are the same one.

---

## Build / Run / Publish

```bat
:: build (all projects)
dotnet build

:: run tests
dotnet test

:: run the launcher (dev)
dotnet run --project SealTools.Launcher

:: publish a self-contained single-file exe (+ config + models)
publish.bat          :: public build  -> dist\SealTools-v<version>.zip        (template config only)
publish.bat local    :: personal build -> dist\SealTools-v<version>-local.zip (your full calibration)
:: both modes also write dist\SealTools-v<version>-firmware.zip (the Arduino sketch)
:: output also left in SealTools.Launcher\bin\Release\net8.0-windows\win-x64\publish\
```

The published `publish\` folder is the distributable: `SealTools.Launcher.exe` + native OCR DLLs +
`config\` + `models\`. Copy it to the target PC and run the exe — no install.

> **Two packaging modes.** `publish.bat` (public, the default) ships only the config *templates* —
> `attributes.yaml`, `defaults.yaml`, `local.yaml.example` — so the zip is safe to share without
> leaking your calibration. `publish.bat local` ships your full `config\` (including `local.yaml` and
> the `calib_*.png` screenshots) for reinstalling on the same machine already calibrated. The exe
> re-seeds `config\local.yaml` from the example on first run, and a fresh machine calibrates its own.
> The script clears the previous mode's config before copying, so a public build can never carry a
> `local.yaml` that a local build left behind.

---

## Code quality

- `.NET analyzers`: `AnalysisLevel=latest`, `AnalysisMode=Recommended`, `TreatWarningsAsErrors=true`
  (in `Directory.Build.props`). Build must be 0 warnings / 0 errors.
- `dotnet format` applied.
- Public methods carry XML docs; disposables use `using`/`Dispose`.
- CI (`.github/workflows/build.yml`) runs the same build + test on Windows for every push to `main`.

---

## Progress, and what is open

Not kept here. This file carried its own progress log and list of open gaps until v2.11, and both had
gone stale against the work — so the answer lives in the documents that own it:

- **What was done, and why** — [docs/PROGRESS.md](docs/PROGRESS.md), dated, newest first.
- **How it works now** — [docs/DESIGN.md](docs/DESIGN.md).
- **What is still open** — [docs/TODO.md](docs/TODO.md).
- **The guardrails** (measured rules that look like improvements to reverse) — [docs/REVIEW.md](docs/REVIEW.md)
  Part D and [docs/DESIGN.md](docs/DESIGN.md) §5.

Two things from the old local list stay here because they are cheap to get wrong and expensive to
rediscover:

- **Capture is `CopyFromScreen` in physical pixels** everywhere, on a thread briefly switched to
  per-monitor-aware. `PrintWindow` returns a **black frame** for this game — do not reintroduce it.
- **A `local.yaml` written before v2.1 holds logical, not physical, coordinates** — recalibrate once
  (see [docs/COORDINATES.md](docs/COORDINATES.md)). Hand-tuned `gem.movements` are HID counts and survive.

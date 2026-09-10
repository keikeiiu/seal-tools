# Seal Tools v2 (C# / .NET WPF)

**Version 2.2** (2026-09-10) — the cursor is positioned by the **Arduino** in a closed loop instead
of `SetCursorPos` (which this process is intermittently refused — see
[docs/CURSOR-INVESTIGATION.md](docs/CURSOR-INVESTIGATION.md)), the Gem Composer moves between
calibrated points instead of hand-tuned counts by default ([docs/MOVE-SETS.md](docs/MOVE-SETS.md)),
empty-result detection compares **pixels** against the saved empty crop instead of a colour average,
and a composer run is N → G → DG **once** and ends.

**Version 2.1** (2026-09-09) — the first release verified end to end on a live game: physical-pixel
coordinates with the measured display environment stored alongside the calibration, a **Setup** tab,
spammer **presets**, and both the Magic Tuner and the Gem Composer confirmed working. See
[docs/STATUS.md](docs/STATUS.md) for the change list and [docs/COORDINATES.md](docs/COORDINATES.md)
for the coordinate model.

A full C#/.NET 8 rebuild of the Seal Online automation tools, replacing the Python build with a
**native Windows desktop app** (WPF + WPF-UI) that ships as a **single self-contained `.exe`**.

> The original Python build lives under `v1/` (`launcher.py`, `tuner/`, `gem_composer/`,
> `skill_spammer/`, `checkin/`), is untouched, and still works. v2 lives entirely under this `v2/` folder.

---

## What it does

Three tools, driven by an Arduino Pro Micro (USB HID mouse/keyboard) over a COM port, plus OCR:

| Tool | What it does |
|------|--------------|
| **Magic Tuner** | Rolls the 發條 (magic tuning) UI — Arduino click+Enter, OCR reads the grade (N/G/DG/XG/SG) + 3 attribute lines, matches them against a config dictionary, applies filter rules, stops at the target grade. |
| **Gem Composer** | Clicks the gem-combine UI (N/G/DG radio + Register + Combine) at calibrated points, moving between them with the Arduino (closed loop by default, hand-tuned counts available via `gem.move_mode`). Combines each grade until the result box reads empty, then advances; ends after the last grade. |
| **Skill Spammer** | Presses configured keys, each on its own cooldown. |

Only one tool runs at a time (they share the single Arduino COM port).

---

## Architecture

```
v2/
  SealTools.sln
  SealTools.Core/        # config loader, Arduino serial, Win32 window/DPI, hotkeys, screen capture, file logging, ToolState
  SealTools.Tuner/       # OcrEngine (RapidOCRSharpOnnx), AttrMatcher, TextCleaner, SealTuner
  SealTools.GemComposer/ # GemComposer
  SealTools.Spammer/     # SkillSpammer
  SealTools.Launcher/    # WPF-UI desktop app (the UI + tool lifecycle)
  SealTools.Tests/       # xUnit tests (config loader)
  config/                # defaults.yaml, attributes.yaml, local.yaml.example (+ local.yaml, gitignored)
  models/                # PP-OCRv4 ONNX models (gitignored, copied from rapidocr-onnxruntime)
  docs/USER_GUIDE.md     # every tab and button explained
  docs/CURSOR-INVESTIGATION.md  # why the cursor is placed with the Arduino, not SetCursorPos
  docs/MOVE-SETS.md      # the composer's two move sets: tuned counts vs arduino point placement
  docs/PROGRESS.md       # dated log of what was done and why (append per session)
  docs/PLAN-TUNER-SPRING.md  # planned: tuner spring placement + cursor guard (not built)
  docs/PLAN-UI-CLEANUP.md    # planned: launcher UI rework — study/decide before coding
  docs/IDEAS.md          # wider backlog: reliability, safety, UX, code health, mini-features
  docs/CALIBRATION.md    # per-machine calibration guide
  docs/COORDINATES.md    # coordinate space, DPI and capture — read before touching them
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

The calibrator is in the launcher, split into **two separate tabs** because the two tools use
different game screens:

1. **Calibrate Tuner** — open the 發條 window → **Capture** → **drag a box** around the grade + 3 attribute lines
   → **Check OCR** (verify it reads the correct grade + attributes) → **Save Tuner**.
2. **Calibrate Gem** — open the gem-combine window → **Capture** → **click** N / G / DG / Register / Combine,
   the 3 resource slots, and **drag a box** around the composed result gem → **Save Gem Composer**.
   Composer moves (raw `dx`/`dy`) are saved separately with **Save Composer Moves**.

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
   lands on its target, then **Save Composer Moves**.

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

Written next to the exe under `logs/`:

- `run_<timestamp>.jsonl` + `run_<timestamp>.txt` — every tuner attempt (grade, remaining, matched attributes).
- `ocr_log.jsonl` — every OCR scan.
- `captures/capture_<timestamp>.png` — the OCR capture region (when `save_captures: true`).
- `error.log` — any unhandled startup exception (helps diagnose silent crashes).

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
publish.bat
:: output: SealTools.Launcher\bin\Release\net8.0-windows\win-x64\publish\SealTools.Launcher.exe
```

The published `publish\` folder is the distributable: `SealTools.Launcher.exe` + native OCR DLLs +
`config\` + `models\`. Copy it to the target PC and run the exe — no install.

> **Packaging note (deliberate):** `publish.bat` copies the whole `config\` folder, so the
> distributable **includes your `local.yaml`** — your machine's calibration. That is intentional for
> personal / same-machine use: the exe arrives already calibrated. Before giving the folder to
> someone else, delete `config\local.yaml` (and any `local.yaml.corrupt-backup`) from the published
> copy; the exe re-seeds it from `config\local.yaml.example` on first run, and they calibrate their
> own. Revisit this once the build is genuinely release-ready.

---

## Code quality

- `.NET analyzers`: `AnalysisLevel=latest`, `AnalysisMode=Recommended`, `TreatWarningsAsErrors=true`
  (in `Directory.Build.props`). Build must be 0 warnings / 0 errors.
- `dotnet format` applied.
- Public methods carry XML docs; disposables use `using`/`Dispose`.

---

## Progress log

**Done:**
- Config layer (externalized, validated) + unit tests.
- OCR pipeline (capture → RapidOCR → grade color + OCR → line reconstruction → attribute match + filter).
- All three tools ported to C#, config-driven, client-area-relative, in-memory control/state.
- WPF-UI launcher: tool cards + live status (structured multi-line), config editing tabs, attribute list,
  calibrator (two tabs: Tuner drag-box + Gem click-points, with Check OCR).
- File logging (run logs + ocr log).
- Self-contained single-file publish.
- Lint/analyzers clean, `dotnet format` applied.
- Fix pass on branch `v2-saving-attempt` — `docs/STATUS.md` has the full commit list and the items
  still deferred.

**Known gaps / to verify (next):**
- The tuner's `remaining_y` band (spring count) is derived proportionally and may need a manual nudge per machine.
- Gem composer moves are raw hand-tuned `dx`/`dy` per route (saved by **Save Composer Moves**); verify them
  with the **Test Move** button after turning pointer acceleration off.
- Calibration and OCR still need a real end-to-end pass on a live game.
- Capture uses `CopyFromScreen` in **physical pixels** everywhere (calibration, OCR, composer), on a
  thread briefly switched to per-monitor-aware. `PrintWindow` was measured to return a **black frame**
  for this game; do not reintroduce it. The launcher hides itself for the grab, so you no longer have
  to move windows — but keep the game unobstructed by anything else.
- Coordinates in an existing `local.yaml` written before this change are **logical**, not physical —
  recalibrate once (see `docs/COORDINATES.md`). Hand-tuned `gem.movements` are HID counts and survive.
- OCR row-bucket pooling: a fixed `row_height` grid can merge two attribute rows — needs real unconfirmed
  frames as evidence before changing (see `docs/REVIEW.md`).
- Check-in remains the standalone Python script (`v1/checkin/checkin.py`) — out of scope for v2.
- `v1/skill_spammer/skill_spammer_config.yaml` has an unrelated uncommitted modification — not part of v2.

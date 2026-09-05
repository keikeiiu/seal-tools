# Seal Tools v2 (C# / .NET WPF)

A full C#/.NET 8 rebuild of the Seal Online automation tools, replacing the Python build with a
**native Windows desktop app** (WPF + WPF-UI) that ships as a **single self-contained `.exe`**.

> The original Python build (`launcher.py`, `tuner/`, `gem_composer/`, `skill_spammer/`, `checkin/`)
> is untouched and still works. v2 lives entirely under this `v2/` folder.

---

## What it does

Three tools, driven by an Arduino Pro Micro (USB HID mouse/keyboard) over a COM port, plus OCR:

| Tool | What it does |
|------|--------------|
| **Magic Tuner** | Rolls the 發條 (magic tuning) UI — Arduino click+Enter, OCR reads the grade (N/G/DG/XG/SG) + 3 attribute lines, matches them against a config dictionary, applies filter rules, stops at the target grade. |
| **Gem Composer** | Clicks the gem-combine UI (N/G/DG radio + Register + Combine) using calibrated absolute points + relative Arduino "D" moves. |
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
  docs/CALIBRATION.md    # per-machine calibration guide
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
   - **Auto-anchor** scales the reference coords to the current window size as a first guess.
2. **Calibrate Gem** — open the gem-combine window → **Capture** → **click** N / G / DG / Register / Combine → **Save Gem Composer**.

Coordinates are **client-area-relative** (game window client top-left = origin), and the process is
**DPI-aware** (physical pixels). Relative "D" moves are empirical (affected by Windows pointer speed).

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
  calibrator (two tabs: Tuner drag-box + Gem click-points, with Check OCR + Auto-anchor).
- File logging (run logs + ocr log).
- Self-contained single-file publish.
- Lint/analyzers clean, `dotnet format` applied.

**Known gaps / to verify (next):**
- The tuner's `remaining_y` band (spring count) is derived proportionally and may need a manual nudge per machine.
- Gem composer `movements` (D deltas) are **not** auto-derived — verify by watching one cycle and nudge in `local.yaml`.
- Check OCR / title-bar drag / TitleBar buttons were added late and still need a real end-to-end pass by the user.
- Check-in remains the standalone Python script (`checkin/checkin.py`) — out of scope for v2.
- `skill_spammer/skill_spammer_config.yaml` (v1) has an unrelated uncommitted modification — not part of v2.

# Seal Tools v2 — Per-Machine Calibration

The tools locate things on screen by **pixel coordinates** that depend on the monitor resolution, Windows
display scale (DPI), and the game window size. **They are not automatic** — each machine must be calibrated
once. v2 makes this much easier than v1: coordinates live in a single machine-specific file, and there is an
in-browser calibrator plus a one-shot auto-anchor.

Everything else (the `.exe`, models, behavior flags) is portable.

---

## 0. What changed from v1

| v1 | v2 |
|----|----|
| Coordinates scattered across `tuner/config.yaml` + `gem_composer_config.yaml` | One file: `config/local.yaml` (gitignored) |
| Manual YAML edit + `grid_last.png` | In-browser calibrator (click where things are) |
| Hand-computed gem `movements` deltas | Click the points; deltas derived |
| DPI-unaware (logical pixels) | DPI-aware physical pixels + client-area-relative |

**Coordinate model:** every anchor is a **client-area-relative offset** from the game window's top-left
(client area, i.e. excluding the title bar). The process is DPI-aware, so all values are physical pixels.

## 1. Config files

- `config/defaults.yaml` — portable defaults (window title, Arduino VID/PID, hotkeys, grade order, grade-color
  thresholds, timing, filter rules, model paths, reference window size). Committed.
- `config/attributes.yaml` — the OCR "wordings": attribute dictionary + OCR-garbled variants + text fixes +
  negative-attribute markers. Committed.
- `config/local.yaml` — **machine-specific coordinates** (OCR region + sub-bands, gem click points, optional
  port). Gitignored; created from `local.yaml.example`.

## 2. First run

1. `copy config\local.yaml.example config\local.yaml`
2. Run the auto-anchor: `SealTools.Launcher.exe --autoanchor`

The auto-anchor detects the game window size and scales the reference coordinates (measured at
`reference_window` in `defaults.yaml`) to the current window. It is a **first guess** — if the game window was
resized or the in-game layout differs, fine-tune with the visual calibrator.

## 3. Visual calibrator (recommended)

1. Start the panel: `SealTools.Launcher.exe` → open http://127.0.0.1:5003
2. Open the game's 發條 (tuning) window.
3. Click **"1. Capture window"** — a screenshot of the game window appears.
4. Click, in order:
   - the **top-left** then **bottom-right** corner of the OCR box (the grade letter + 3 attribute lines),
   - the gem **N**, **G**, **DG** radio buttons, **Register**, and **Combine** buttons.
5. Click **"3. Save"** — the coordinates (and derived sub-regions/deltas) are written to `config/local.yaml`.

## 4. Verify

`SealTools.Launcher.exe --diagnose` captures the region and prints the detected grade, color scores, and
attribute lines. A correct calibration shows a real grade letter and 3 attribute lines.

## 5. Run

`SealTools.Launcher.exe` starts the unified panel (http://127.0.0.1:5003). Only one tool runs at a time
(they share the Arduino COM port). Magic Tuner / Gem Composer / Skill Spammer can be started/stopped from the
panel; they also respond to the hotkeys configured in `defaults.yaml` (F12 start, F11 quit, F9 advance grade).

## 6. Machine-specific values (all in `local.yaml`)

- `tuner.ocr.region` — capture box, client-area-relative.
- `tuner.ocr.grade_area`, `grade_y`, `attr_y`, `remaining_y`, `row_height` — capture-relative sub-bands.
- `gem.grade_positions` — absolute (client-area-relative) click points.
- `gem.movements.*` — Arduino "D" relative-move deltas (auto-derived by the calibrator).
- `arduino.port` — optional override (empty = auto-detect by VID/PID).
- `display.dpi_scale` — optional manual DPI override (usually auto-detected).

> Note: relative D-moves are affected by Windows pointer speed ("Enhance Pointer Precision"), so the D↔pixel
> relationship is empirical — verify the gem composer by watching one cycle, and nudge `movements` if needed.

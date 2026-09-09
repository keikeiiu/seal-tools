# Seal Tools v2 — Per-Machine Calibration

The tools locate things on screen by **pixel coordinates** that depend on the monitor resolution,
Windows display scale, the game window size, and where the window sits. **They are not automatic** —
each machine must be calibrated once. v2 keeps every machine-specific value in a single file,
`config/local.yaml` (gitignored), written by the launcher's two calibration tabs.

Everything else (the `.exe`, models, behavior flags) is portable.

---

## Coordinate model

- **Client-area-relative.** The origin is the game window's client-area top-left
  (`GetClientRect` + `ClientToScreen`), *excluding* the title bar and border. Not window-relative,
  not screen-absolute.
- **Physical pixels.** Values are real screen pixels. The process stays **DPI-unaware** (PerMonitorV2
  makes `SetCursorPos` fail — see `docs/COORDINATES.md`); captures and measurements briefly switch one
  thread to per-monitor-aware to get true physical coordinates, and the cursor is positioned by the
  **Arduino** (a closed `D dx dy` loop against `GetCursorPos` — see `docs/CURSOR-INVESTIGATION.md`).
- **Absolute clicks:** position the cursor at the calibrated client-relative physical point, then an
  Arduino `C` (HID click).
- **Relative moves:** raw Arduino HID counts, sent as `D dx dy`. They are **hand-tuned values, not
  computed from pixel distances** — see the pointer-precision prerequisite below.

## 1. First run

1. If `config/local.yaml` is missing, `ConfigLoader` seeds it from `config/local.yaml.example`
   automatically. (You can also copy it by hand.)
2. Open the launcher and go to the **Calibrate Tuner** and **Calibrate Gem** tabs.

> The seeded values are v1 window-relative measurements. They are a starting point only and **will
> be off** until you recalibrate — expect to do both tabs once per machine.

> A `local.yaml` written before the physical-pixel change holds **logical** coordinates, which are
> wrong in the new space — recalibrate. Hand-tuned `gem.movements` (HID counts) are unaffected.

The **Setup** tab shows the detected monitor scale and game client size and stores them with the
calibration, so a different machine can tell whether it must recalibrate. See `docs/COORDINATES.md`.

## 2. Calibrate Tuner

1. Open the game's 發條 (tuning) window.
2. **Capture 發條 window** — a screenshot of the game window appears. (The capture reads the screen, so
   keep the game visible — don't let the launcher window cover it.)
3. Drag three boxes in order: the **grade letter**, the **3 attribute lines**, and the **spring
   count**.
4. **Check OCR** — confirm it reads the right grade and three attribute rows. This also measures the
   real attribute line pitch and stores it as `row_height` (a tighter, more accurate value than the
   `attr.Height / 3` fallback used if you skip this step).
5. **Save Tuner** — writes `tuner.ocr` (region + sub-bands + `row_height`) to `local.yaml`, refreshes
   the in-memory config so the next run uses it immediately, and saves a reference PNG
   (`config/calib_tuner.png`).

## 3. Calibrate Gem

1. Open the gem-combine window.
2. **Capture gem window** — keep the game visible (not covered by the launcher).
3. Click, in order: **N**, **G**, **DG**, **Register**, **Combine**, then the **3 resource slots**,
   then drag a box around the **composed result gem**.
4. **Save Gem Composer** — writes `gem.grade_positions`, `gem.resource_gems` and
   `gem.result_gem_area` to `local.yaml`. It then asks whether the result box is currently **empty**;
   answer **Yes** to sample the empty-box colour reference (`gem.empty_signature`), which is what
   empty-result detection compares against. Answer No and empty detection stays off until you re-save
   with the box empty.
5. **Composer moves are saved separately.** The **Composer moves** grid holds the raw `dx`/`dy`
   counts the composer sends for each route (grade → Register, Register → Combine, and so on). Edit
   them and press **Save Composer Moves** to write `gem.movements` to `local.yaml`. *Save Gem
   Composer* does not touch them.
6. **Coordinates can also be typed directly.** The **Coordinates** grid (N/G/DG/Register/Combine,
   Resource1-3, result area) accepts edits — press **Save Coordinates** to persist them.

### Verifying without running a full cycle

- **Test Click** — moves the cursor to the selected point and clicks it.
- **Test Move (rel)** — clicks the `from` point, sends that route's raw `D dx dy`, then clicks the
  `to` point. If it lands, the composer's move for that route is correct.
- **Check Result Colour** / **Test Result Gem** — sample the result box and report its colour
  composition, the distance to the empty reference, and the empty/has-gem verdict.

## 4. Prerequisite: fixed Windows pointer precision (Gem Composer)

The game is an OS-cursor title (early Windows, not raw-input), so the on-screen distance a relative
`D dx dy` travels depends on the Windows pointer-precision mapping. With "Enhance pointer precision"
on, that mapping is velocity-dependent and the composer drifts between runs.

1. `Settings → Bluetooth & devices → Mouse → Additional mouse options → Pointer Options` → uncheck
   **"Enhance pointer precision"**, and leave the speed notch where it is.
2. Then **hand-tune** the raw `dx`/`dy` values in the Composer moves grid until **Test Move** lands
   on its target.

There is no scale factor to measure and none is stored — the raw counts *are* the tuning. This is
why the moves are hand-tuned rather than computed from pixel deltas.

## 5. Files written by calibration

| File | Written by |
|------|-----------|
| `config/local.yaml` | Save Tuner / Save Gem Composer / Save Coordinates / Save Composer Moves |
| `config/calib_tuner.png` | Save Tuner (reference screenshot with the bands drawn on it) |
| `config/calib_gem.png` | Save Gem Composer (reference screenshot with the click points + result box) |
| `config/calib_gem_result.png` | Save Gem Composer (crop of the result-gem area) |

## 6. Removed v1 mechanisms (do not look for them)

The old Python build's calibration aids no longer exist in v2, and older copies of this document
described them:

- **No web panel** — there is no `http://127.0.0.1:5003` UI; calibration is in the WPF launcher.
- **No `--autoanchor` / `--diagnose` flags** — the launcher takes no arguments. Auto-anchor is
  documented as a planned feature but is **not implemented**; `reference_window` in `defaults.yaml`
  is currently unused.
- **No `gem.mouse_scale` and no "Probe scale" button** — the scale-based relative-move computation
  was removed; moves are raw hand-tuned counts.
- **No `display.dpi_scale`** — the process is DPI-unaware and the key was removed.

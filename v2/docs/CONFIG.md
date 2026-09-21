# Seal Tools v2 — Config Files & What Goes Where

The config is split across two on-disk files plus transient in-memory state. This document
explains what belongs in each and why.

## The three layers

### 1. `config/defaults.yaml` — portable defaults (committed)

Settings that are **the same on every machine and every user**. This file is checked into git.

- `window.title`, `arduino.*` (VID/PID/baud)
- `hotkeys.*`
- `tuner.grade_order`, `tuner.target_grade`
- `tuner.grade_colors.*`, `tuner.models.*`, `tuner.filter.*`
- `reference_window.*`
- **Behavior preferences** (not tied to hardware):
  - `gem.start_grade`
  - `gem.empty_mode` — `"stop"` | `"advance_grade"` | `"advance_grade_clear"`
  - `gem.empty_streak` — consecutive empties before acting
  - `gem.move_mode` — `"tuned"` | `"arduino"`, which move set the composer uses (see MOVE-SETS.md)

### 2. `config/local.yaml` — machine-specific (gitignored)

Settings **measured or tuned on this machine**. This file is NOT committed (it's the per-machine
overlay created from `local.yaml.example`).

- `tuner.ocr.*` — OCR region + sub-bands
- `gem.grade_positions`, `gem.resource_gems`, `gem.result_gem_area`
- `gem.movements.*` — hand-tuned HID deltas
- `gem.empty_signature` — the sampled empty-box colour. **The fallback**, used only when the saved
  empty crop below is missing
- `gem.empty_distance` — the difference threshold: pixels-vs-crop when the crop exists, colour distance
  against the signature when it does not
- `arduino.port` (optional override)
- `spammer.active` + `spammer.presets.*` — **your own** key rotations. Not machine-specific, but
  personal: `defaults.yaml` is the file `publish.bat public` copies into the release, so a preset
  written there would ship with the next zip. See the note under "Why the split?" below.

### 3. In-memory only (never written to disk)

Transient runtime state that must not survive a restart.

- the running tool's status (`running`, `cycle`, current `grade`)
- the *un-saved* calibration (`_gemPoints` / `_gemResultBox` before you hit Save)
- cursor position, focus, quit/pause flags

## Why the split?

| layer | rule of thumb | committed? |
|---|---|---|
| `defaults.yaml` | "what's true for everyone" | yes |
| `local.yaml` | "what's specific to this machine — or to you" | no |
| in-memory | "transient state, discard on restart" | n/a |

`local.yaml` holds two kinds of thing that are not the shared template: values measured on **this
machine** (calibration, coordinates, port) and values that are simply **yours** (spammer presets).
Both live there for the same reason — `publish.bat public` excludes the file, so neither leaks into
someone else's install. A setting that is personal but not machine-specific has no third home.

## Comments do not survive a save

Both files are written by serialising an object graph, which cannot carry comments. So:

- **Every comment in either file is destroyed by the next save** — including ones you added by hand.
- **Any key the launcher does not know about is dropped**, silently. This is not hypothetical: it is
  how a machine's spammer presets were deleted outright. The block lived in `defaults.yaml`, a Save
  from the Tuner tab rewrote the file without it, and nothing reported it. Presets live in
  `local.yaml` now and the adoption path catches strays, but the underlying behaviour is unchanged.

Because of that, the writer emits its **own** header on every save — the only prose that can survive.
It says both of the above, so the warning travels with the file rather than living only here. If you
want to hand-edit either file, back it up first, and put enduring notes in this document instead.

## How saves work

- The launcher's **Save** writes the portable sections to `defaults.yaml` (`SaveDefaults`) and the
  machine-specific parts to `local.yaml` (`SaveLocal`).
- The gem calibrator's **Save Gem Composer** writes positions / resource gems / result-area /
  the empty crop / `empty_signature` / `empty_distance` to `local.yaml`. **Save tuned counts** writes
  `gem.movements` separately, and **Save Coordinates** writes the typed-in positions. `empty_mode` /
  `empty_streak` live in `defaults.yaml`.
- The spammer tab's **Save Preset** writes `spammer.active` + `spammer.presets` to `local.yaml`.
  There is no `spammer` block in `defaults.yaml` on purpose: `SaveDefaults` rewrites that file from an
  explicit field list, so a seed preset put there would be deleted the first time any *other* tab
  saved — and an install whose presets lived only there would lose them silently. On load,
  `ConfigLoader` adopts any presets it still finds in `defaults.yaml` into `local.yaml`.

## The empty-detection fields (gem)

| field | file | meaning |
|---|---|---|
| `empty_mode` | `defaults.yaml` | what to do when the result box is empty |
| `empty_streak` | `defaults.yaml` | consecutive empty reads before acting |
| `empty_distance` | `local.yaml` | the difference threshold below which the box is "empty" |
| *(the saved empty crop)* | `config/calib_gem_result.png` — an image, not a YAML key | **the primary test**: the fraction of pixels differing from the saved empty-box crop. Colour- and shape-blind, so any gem reads the same. Measured empty `0.000` against a gem at `0.357`, threshold `0.18` |
| `empty_signature` | `local.yaml` | **the fallback**, used only when the crop is missing: the average colour of the empty box. The weaker test — a gem whose colour is close to the empty slot's can hide in it |

`GemComposer.IsResultBoxEmpty` uses the crop when it exists and the signature otherwise, and answers
"not empty" when neither is available, so the composer never advances a grade on a missing reference.
Both are written by **Save Gem Composer**; [CALIBRATION.md](CALIBRATION.md) covers the capture itself.

> Note on capture: everything uses `CopyFromScreen` (a screen region) — the calibrator's **Capture**
> button, OCR and the composer loop. `PrintWindow` was tried for calibration and returns a **black
> frame** for this game (measured with the "Diagnose capture" button); do not reintroduce it. Because
> the capture reads the screen, the game must be visible — not covered by the launcher — when you
> press Capture.

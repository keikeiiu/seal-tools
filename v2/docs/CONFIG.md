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

### 2. `config/local.yaml` — machine-specific (gitignored)

Settings **measured or tuned on this machine**. This file is NOT committed (it's the per-machine
overlay created from `local.yaml.example`).

- `tuner.ocr.*` — OCR region + sub-bands
- `gem.grade_positions`, `gem.resource_gems`, `gem.result_gem_area`
- `gem.movements.*` — hand-tuned HID deltas
- `gem.empty_signature` — the sampled empty-box colour (filled by the calibrator)
- `gem.empty_distance` — colour-distance threshold tuned against the empty signature
- `arduino.port` (optional override)

### 3. In-memory only (never written to disk)

Transient runtime state that must not survive a restart.

- the running tool's status (`running`, `cycle`, current `grade`)
- the *un-saved* calibration (`_gemPoints` / `_gemResultBox` before you hit Save)
- cursor position, focus, quit/pause flags

## Why the split?

| layer | rule of thumb | committed? |
|---|---|---|
| `defaults.yaml` | "what's true for everyone" | yes |
| `local.yaml` | "what's specific to this machine" | no |
| in-memory | "transient state, discard on restart" | n/a |

## How saves work

- The launcher's **Save** writes the portable sections to `defaults.yaml` (`SaveDefaults`) and the
  machine-specific parts to `local.yaml` (`SaveLocal`).
- The gem calibrator's **Save Gem Composer** writes positions / resource gems / result-area /
  `empty_signature` / `empty_distance` to `local.yaml`. **Save Composer Moves** writes
  `gem.movements` separately, and **Save Coordinates** writes the typed-in positions. `empty_mode` /
  `empty_streak` live in `defaults.yaml`.

## The empty-detection fields (gem)

| field | file | meaning |
|---|---|---|
| `empty_mode` | `defaults.yaml` | what to do when the result box is empty |
| `empty_streak` | `defaults.yaml` | consecutive empty reads before acting |
| `empty_distance` | `local.yaml` | colour-distance threshold below which the box is "empty" |
| `empty_signature` | `local.yaml` | the sampled empty-box colour reference |

> Note on capture: the calibrator's **Capture** button uses `PrintWindow`, which grabs the whole game
> window (title bar included) so you can click points on it. The runtime paths — OCR and the composer
> loop — use `CopyFromScreen` on a screen region instead. Keep these two separate: switching
> calibration back to `CopyFromScreen` is what broke the DirectX capture.

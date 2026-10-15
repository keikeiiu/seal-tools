# Seal Tools v2 — Config Files & What Goes Where

The config is split across two on-disk files plus transient in-memory state. This document
explains what belongs in each and why.

## The three layers

### 1. `config/defaults.yaml.example` — portable defaults (the shipped template)

Settings that are **the same on every machine and every user**. The repo tracks the file as
`config/defaults.yaml`; the release ships it under the template name `config/defaults.yaml.example`,
and that is the file an install reads as its **base**. On an install that has taken an update,
`config/defaults.yaml` is instead your own legacy copy: read once on the first launch, folded into
`local.yaml`, then never read again.

- `game.variant` + `game.variants.*` — **which client** (tw / us), each with its window title and its
  attribute dictionary. The variant **sets `window.title` at load**, so `window.title` is only the
  source when no variant is set. Also switchable at runtime from the **Setup** tab, one button per
  variant, which writes this key. See [PLAN-US-CLIENT.md](../plans/PLAN-US-CLIENT.md)
- `window.title`, `arduino.*` (VID/PID/baud)
- `hotkeys.*`
- `tuner.grade_order`, `tuner.target_grade`
- `tuner.grade_colors.*`, `tuner.models.*`, `tuner.filter.*`
- `reference_window.*`
- **Behavior preferences** (not tied to hardware):
  - `pet.items_per_minute` — **the FALLBACK rate only.** The boarding window prints the feeding rate
    beside every row (`每1分 攝取3個`) and the pet feeder reads it, per row, because the rate follows
    the pets' feeding stage and rows need not hold pets at the same stage. This value answers only
    when that line could not be read.
  - `gem.start_grade`
  - `gem.empty_mode` — `"stop"` | `"advance_grade"` | `"advance_grade_clear"`
  - `gem.empty_streak` — consecutive empties before acting
  - `gem.move_mode` — `"tuned"` | `"arduino"`, which move set the composer uses (see MOVE-SETS.md)

### 2. `config/local.yaml` — machine-specific, plus your portable settings (gitignored)

Settings **measured or tuned on this machine** — and, in its `settings:` block, your own copy of the
portable settings. This file is NOT committed (it's the per-machine overlay created from
`local.yaml.example`).

- `tuner.ocr.*` — OCR region + sub-bands
- `gem.grade_positions`, `gem.resource_gems`, `gem.result_gem_area`
- `gem.movements.*` — hand-tuned HID deltas
- `gem.empty_signature` — the sampled empty-box colour. **The fallback**, used only when the saved
  empty crop below is missing
- `gem.empty_distance` — the difference threshold: pixels-vs-crop when the crop exists, colour distance
  against the signature when it does not
- `arduino.port` (optional override)
- `pet.close_button_png`, `pet.boarding_window_may_be_open` — the pet feeder's close guard, **written
  by the run itself** (never hand-edit): a crop of the boarding window's X, learned from a window
  whose contents the run had just read, and whether the run believes that window may still be on
  screen after a close it could not send. See archive/PLAN-PET-CLOSE-GUARD.md.
- `spammer.active` + `spammer.presets.*` — **your own** key rotations. Not machine-specific, but
  personal: `defaults.yaml` is the source `publish.bat public` copies into the release — under the
  name `config/defaults.yaml.example` — so a preset written there would still ship with the next
  zip. See the note under "Why the split?" below.

### 3. In-memory only (never written to disk)

Transient runtime state that must not survive a restart.

- the running tool's status (`running`, `cycle`, current `grade`)
- the *un-saved* calibration (`_gemPoints` / `_gemResultBox` before you hit Save)
- cursor position, focus, quit/pause flags

## Why the split?

| layer | rule of thumb | committed? |
|---|---|---|
| `defaults.yaml.example` (tracked as `defaults.yaml`) | "what's true for everyone" | yes |
| `local.yaml` | "what's specific to this machine — or to you", plus your `settings:` block | no |
| in-memory | "transient state, discard on restart" | n/a |

`local.yaml` holds three kinds of thing that are not the shared template: values measured on **this
machine** (calibration, coordinates, port), values that are simply **yours** (spammer presets), and
your own copy of the portable settings, in its `settings:` block. All three live there because it is
the player's own file: `publish.bat public` excludes it, so nothing there leaks into someone else's
install, and no update replaces it. A setting that is personal but not machine-specific has no third
home.

## Comments do not survive a save

`local.yaml` is written by serialising an object graph, which cannot carry comments. So:

- **Every comment in it is destroyed by the next save** — including ones you added by hand.
- **Inside a block the launcher owns, a key it does not know about is dropped**, silently. This is not
  hypothetical: it is how a machine's spammer presets were deleted outright. The block lived in the
  then-shipped `defaults.yaml`, a Save from the Tuner tab rewrote the file without it, and nothing
  reported it.
- **A whole top-level block the launcher does not own survives a save**, carried through verbatim with
  its comments — so a section of your own is safe there. Nothing reads its keys, and a block whose name
  collides with one the launcher owns is rewritten like any other. The list of owned blocks is
  `ConfigLoader.OwnedLocalBlocks`, and a test walks `LocalOverrides` to keep it complete.

Because of that, the writer emits its **own** header on every save — the only prose that can survive.
It says both of the above, so the warning travels with the file rather than living only here. If you
want to hand-edit either file, back it up first, and put enduring notes in this document instead.

## How saves work

- The launcher's **Save** writes the portable sections into `local.yaml`'s `settings:` block
  (`SaveSettings`), and the machine-specific parts into the same file (`UpdateLocal`). Nothing writes
  `defaults.yaml` any more.
- The gem calibrator's **Save Gem Composer** writes positions / resource gems / result-area /
  the empty crop / `empty_signature` / `empty_distance` to `local.yaml`. **Save tuned counts** writes
  `gem.movements` separately, and **Save Coordinates** writes the typed-in positions. `empty_mode` /
  `empty_streak` ship in `defaults.yaml.example`; your own values are written to `local.yaml`'s
  `settings:` block.
- The spammer tab's **Save Preset** writes `spammer.active` + `spammer.presets` to `local.yaml`.
  There is no `spammer` block in `defaults.yaml` on purpose: presets are personal, and that file is
  the shared template that ships to every user, so a preset put there would leak into someone else's
  install. On load, `ConfigLoader` adopts any presets it still finds in `defaults.yaml` into
  `local.yaml`.

## The empty-detection fields (gem)

| field | file | meaning |
|---|---|---|
| `empty_mode` | `defaults.yaml.example` (shipped) / `local.yaml` `settings:` (yours) | what to do when the result box is empty |
| `empty_streak` | `defaults.yaml.example` (shipped) / `local.yaml` `settings:` (yours) | consecutive empty reads before acting |
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

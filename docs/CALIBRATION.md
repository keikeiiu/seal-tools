# Per-Machine Calibration Guide

The tools locate things on screen by **pixel coordinates** that were measured on the *original* development machine. They are **not automatic** — they depend on the exact **monitor resolution, Windows display scale (DPI), and the game window size**. If any of those differ on a new PC, the clicks and the OCR capture land in the wrong place.

**There is no "setup" for this that you skip — you must re-measure the coordinates for each machine** before the tools will work. This guide walks through both places this matters:

1. **Magic Tuner OCR region** — where the tool reads the grade + attribute text.
2. **Gem Composer movements** — where the tool clicks to combine gems.

Everything else (install, config format) is not machine-specific.

---

## 0. The three things that shift coordinates

| Setting | In Windows / the game | Effect |
|---------|----------------------|--------|
| **Resolution** | Settings → Display | Whole coordinate grid scales. 1366×768 vs 1920×1080 vs 4K |
| **Display scale (DPI)** | Settings → Display → Scale (e.g. 100%, 150%, 200%) | Windows stretches everything; a window at "150%" is physically bigger |
| **Game window size** | In-game resolution (fullscreen vs windowed) | The game window's own top-left + size changes |

**Key fact:** the offsets are measured **relative to the game window's top-left corner** (not the screen origin). So the window size and its position on screen both matter. Match the game window to the same size/placement as the machine you calibrated on, and the rest is unchanged.

---

## 1. Magic Tuner — OCR capture region

The tuner captures a fixed box and runs OCR on it. The box is defined in [`tuner/ocr_engine.py`](../tuner/ocr_engine.py):

```python
TUNING_REGION = {"left": 1140, "top": 840, "width": 300, "height": 320}
```

`left`/`top` are **pixel offsets from the game window's top-left**. `width`/`height` is the box size. If this box isn't over the 發條 tuning window, OCR reads empty or garbage.

### 1.1 Run the diagnostic (game open, 發條 window open)

From the repo folder:

```bat
py -m tuner.ocr_engine
```

It prints:

```
Screen: 1920 x 1080
Game window (TW_LIVE): (0, 0, 1920, 1080)
TUNING_REGION: {'left': 1140, 'top': 840, 'width': 300, 'height': 320}
-> capture on-screen (1140, 840), size 300x320
Grade: ...
```

It also saves a grid overlay to **`tuner\logs\captures\grid_last.png`**.

### 1.2 Interpret the result

- **"ERROR: TW_LIVE window not found!"** → the game client isn't running, or the window title isn't `TW_LIVE` (see `GAME_WINDOW` in `ocr_engine.py`). Open the game.
- **Grade is a real letter** (N/G/DG/XG/SG) and attributes show correctly → the region is fine, ignore the rest.
- **Grade is `None`** and attributes are empty → the box is off-target. Open `grid_last.png`.

### 1.3 Read the grid overlay

`grid_last.png` is the **current capture box**, with a 50px grid and pixel labels drawn on top. Look at it:

- **Box is right** on the 發條 window (grade letter + 3 attribute lines visible) → done.
- **Box is empty/black or over the wrong part of the screen** → it's misaligned. The grid labels give you the **offset within the box** where the grade + attributes currently appear, which tells you how far to shift `left`/`top`.

### 1.4 Adjust `TUNING_REGION`

Edit `tuner/ocr_engine.py`:

```python
TUNING_REGION = {"left": <L>, "top": <T>, "width": 300, "height": 320}
```

- If the box is too far **right**, decrease `left`. Too far **left**, increase `left`. Same for `top` (up/down).
- Keep `width`/`height` enough to cover the grade + the 3 attribute lines (300×320 worked on the source machine; adjust if your font/window size is different).
- After each change, re-run `py -m tuner.ocr_engine` to re-check.

**Tip:** keep `save_captures: true` in `tuner/config.yaml` so every scan writes a PNG you can inspect.

### 1.5 If grade is still wrong (color detection)

The tuner also guesses the grade by **pixel color** of the grade letter in a fixed sub-area:

```python
GRADE_AREA = {"x1": 149, "y1": 1, "x2": 232, "y2": 44}   # capture-relative coords
```

If OCR is fine but the color-based grade is wrong, nudge this box so it only covers the grade letter. (N=white, G=blue, DG=yellow, XG=red, SG=purple.)

---

## 2. Gem Composer — movement coordinates

The Gem Composer moves the mouse with **Arduino "D" commands** (relative USB-HID moves) and clicks radio buttons a fixed number of pixels. All values live in [`gem_composer/gem_composer_config.yaml`](../gem_composer/gem_composer_config.yaml):

```yaml
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

**These are the hardest to transfer** — they were tuned for a specific screen (see `docs/calibrated_movements.md`, tested at 4K 150%). On a different resolution/scale they will click the wrong spot.

### 2.1 How to re-measure

Because the Arduino moves in **relative** units, the easiest approach is to measure a **reference point** and compute the deltas, rather than eyeballing every click.

1. Open the gem-combine window on the fresh PC.
2. Record the **on-screen pixel position** of each target (grade radio N/G/DG, Register, Combine). You can get pixel coords with any screenshot tool that shows X/Y, or a small screen-py helper.
3. Set `grade_positions` to those **absolute** coordinates.
4. Compute the `movements` as **differences between those positions** in Arduino D units (1 D ≈ 1 pixel at 100% scale; if your scale ≠ 100%, multiply by the scale factor). E.g. `radio_to_register.N = Register − N = [835−727, 724−696] = [108, 28]`.

### 2.2 Sanity check

Start Gem Composer at grade **N**, watch where the mouse goes. If each click is consistently off by a fixed amount, adjust the deltas by that amount. If the clicks are wildly off, re-measure from scratch.

---

## 3. Summary

| What | File | What to change | Machine-specific because |
|------|------|----------------|--------------------------|
| Tuner OCR box | `tuner/ocr_engine.py` | `TUNING_REGION` | Resolution, display scale, game window size |
| Tuner grade color area | `tuner/ocr_engine.py` | `GRADE_AREA` | Same |
| Tuner captures | `tuner/config.yaml` | `save_captures` | enable for debugging |
| Gem Composer clicks | `gem_composer/gem_composer_config.yaml` | `grade_positions`, `movements` | Resolution, display scale, window size |

**Bottom line:** install (`setup.bat`) and config are portable; the on-screen coordinates are **not**. Re-measure them per machine, or the tools will act at the wrong place.

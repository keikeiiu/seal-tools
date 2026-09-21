# Seal Tools v2 — Per-Machine Calibration

The tools locate things on screen by **pixel coordinates** that depend on the monitor resolution,
Windows display scale, the game window size, and where the window sits. **They are not automatic** —
each machine must be calibrated once. v2 keeps every machine-specific value in a single file,
`config/local.yaml` (gitignored), written by the launcher's calibration tabs.

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
2. Open the launcher and go to the tabs for the tools you intend to run — **Calibrate Tuner**,
   **Calibrate Gem**, **Buy / Sell**, and for the feeder **Calibrate Pet** and **Calibrate Tooltip**.
   Each one calibrates only its own tool; none of them requires the others.

> The seeded values are v1 window-relative measurements. They are a starting point only and **will
> be off** until you recalibrate — expect to do each tab once per machine.

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
4. **Click the 發條 button** in the capture to record its point (`tuner.spring_point`). This is only
   needed if you want the tuner to place the cursor for you (`spring_mode: hid`); skip it to keep
   moving the mouse yourself. **Test Click (發條)** moves the cursor there so you can confirm it lands.
5. **Check OCR** — confirm it reads the right grade and three attribute rows. This also measures the
   real attribute line pitch and stores it as `row_height` (a tighter, more accurate value than the
   `attr.Height / 3` fallback used if you skip this step).
6. **Save Tuner** — writes `tuner.ocr` + `tuner.spring_point` to `local.yaml`, refreshes the in-memory
   config so the next run uses it immediately, and saves a reference PNG (`config/calib_tuner.png`).

## 3. Calibrate Gem

1. Open the gem-combine window.
2. **Capture gem window** — keep the game visible (not covered by the launcher).
3. Click, in order: **N**, **G**, **DG**, **Register**, **Combine**, then the **3 resource slots**,
   then drag a box around the **composed result gem**.
4. **Save Gem Composer** — writes `gem.grade_positions`, `gem.resource_gems` and
   `gem.result_gem_area` to `local.yaml`. It then asks whether the result box is currently **empty**;
   answer **Yes** and it saves the empty-box crop to `config/calib_gem_result.png` and samples the
   colour reference (`gem.empty_signature`). **The crop is what empty-result detection actually
   compares against** — the signature is only the fallback for when the crop is missing, so answering
   **No** leaves the weaker test in place. See [CONFIG.md](CONFIG.md#the-empty-detection-fields-gem).
5. **Composer moves are saved separately.** The **Moves — tuned (hand-tuned counts)** grid holds the
   raw `dx`/`dy` counts the composer sends for each route (grade → Register, Register → Combine, and
   so on). Edit them and press **Save tuned counts** to write `gem.movements` to `local.yaml`.
   *Save Gem Composer* does not touch them.
6. **Coordinates can also be typed directly.** The **Coordinates** grid (N/G/DG/Register/Combine,
   Resource1-3, result area) accepts edits — press **Save Coordinates** to persist them.
7. **Which move set the composer uses** is **Composer move mode** on the **Move set** card:
   `tuned` (the counts above) or `arduino` (the cursor is placed on each route's destination point,
   closed loop, so nothing needs tuning). Whichever set is not active is dimmed. See
   [MOVE-SETS.md](MOVE-SETS.md).

### Verifying without running a full cycle

- **Place cursor + click** — moves the cursor to the selected point and clicks it.
- **Test tuned move** — clicks the `from` point, sends that route's raw `D dx dy`, then clicks the
  `to` point. If it lands, the composer's move for that route is correct.
- **Run** in the *Moves — arduino* grid — the same for the other move set: click the source, place
  the cursor on the destination, click.
- **Check Result Colour** / **Sample result gem** — sample the result box and report its colour
  composition, the distance to the empty reference, and the empty/has-gem verdict.
- **Run one full cycle** — one whole composer cycle using the arduino moves; 21 real clicks. Use it to
  check that move set survives a real run before switching the composer over to it.

## 4. Buy / Sell (the shop and the bag)

Two activities that share one tab, because they are measured from the same frame: **capture with the
shop open, the bag open, and the count dialog showing.**

### Sell side — the bag grid

1. **Capture bag window.**
2. **Draw grid area** — drag a box around the **whole 8 × 8 bag**.
3. **Draw one slot** — then drag a box around **one slot**.
4. **Show 64 centres** — check the magenta dots sit on the slots. This is the verification step: the
   tool derives all 64 click points from a single uniform pitch, and 64 dots at once is the only way
   to see whether the grid really is uniform.

   Both boxes are required, and they check each other: the whole grid gives the pitch
   (`gridWidth / 8`), and the single slot is the independent measurement. **If they disagree by more
   than a little, the tab refuses to save** rather than averaging two numbers that cannot both be
   right — that disagreement is a bad drag, not something to split.

### Buy side — the shop list, the focus point and MAX

5. **Draw list region** — drag a box around **exactly the visible rows of the shop list**, from the
   first row's top to the last row's bottom. There is a real boundary to aim at, and every derived
   row is only as good as this drag.
6. **Mark focus point** — click a spot in the capture, then **click that spot** in the capture.
   **It must be somewhere inert** — empty panel space or a window title bar. This is not a marker:
   **both tools left-click it at the start of every run**, because starting a tool means clicking the
   launcher, and that leaves the game unfocused — a state in which it ignores both the wheel and a
   right-click. So it must not be over a shop row or a bag slot, where a left-click selects or buys.
7. **Mark MAX button** — click the **MAX** button in the count dialog.
8. **Save Calibration** — writes the grid, the slot box, the list region, the focus point and MAX to
   `local.yaml`. The **Setup so far** checklist shows what is set and what is still missing; saving
   with a gap is allowed, and the tool says what is missing when you press Start rather than clicking
   into empty screen.

### Verifying

- **Show 64 centres** — the dots must sit on the bag slots, and the green row dots on the shop rows.
  If the magenta dots are off, re-drag the grid area; if the green ones are, re-drag the list region.
- **Test scroll (wheel)** — sends real wheel notches so you can see how far one notch moves the list.
  The game must be **focused** and the cursor over the list, which is why the button clicks the focus
  point first. The number you learn here is what a buy item's **Scroll notches** is set from.
- **Sell** tab → **Dry run** — walks the cursor through the selected slots **in the order the real run
  would sell them**, and clicks nothing.
- **Buy** tab → **Dry run** — scrolls and parks the cursor on the item's row, and clicks nothing.

> **The shop list must be at the top before a buy run**, and that is your setup rather than the tool's
> job. A preset's scroll means "notches down from the top", so a list left part-way down puts the item
> that much further off, and nothing detects it. Scroll it there yourself before a run or a dry run.

## 5. Calibrate Pet and Calibrate Tooltip

Two tabs that belong to the **Pet Feeder** and to nothing else. Skip them unless you run it.

- **Calibrate Pet** measures the boarding window's geometry — the 目錄 button, the pet feed icon, the
  window's **X**, the bag's page tabs, the per-row start/end toggle, the boarding pet slot, the two
  feeder slots, and the **boarding bag's own** 8 × 8 grid (it is *not* the shop's bag grid — the bag
  sits somewhere else on this screen, and sharing the numbers aims every click at the wrong item).
  It also carries a **row geometry** pane that shows each row's crop pixels, and a **Test read** pane
  that prints every slot's raw reading.
- **Calibrate Tooltip** measures the offset and shape of the **hover panel** — the box the feeder reads
  a pet's growth and EXP% from before boarding it. Without it the feeder still runs, but it cannot tell
  a finished pet from a feedable one and will board the finished one.

**Step by step, control by control:** [USER_GUIDE.md](USER_GUIDE.md#calibrate-pet-tab) and
[USER_GUIDE.md](USER_GUIDE.md#calibrate-tooltip-tab). For what the marks *mean* and how the tool uses
them, see [PET-TAB-DESIGN.md](PET-TAB-DESIGN.md). The feeder refuses to start until its own
calibration is complete, and says what is missing when it does.

---

## 6. Prerequisite: fixed Windows pointer precision (Gem Composer)

The game is an OS-cursor title (early Windows, not raw-input), so the on-screen distance a relative
`D dx dy` travels depends on the Windows pointer-precision mapping. With "Enhance pointer precision"
on, that mapping is velocity-dependent and the composer drifts between runs.

1. `Settings → Bluetooth & devices → Mouse → Additional mouse options → Pointer Options` → uncheck
   **"Enhance pointer precision"**, and leave the speed notch where it is.
2. Then **hand-tune** the raw `dx`/`dy` values in the Composer moves grid until **Test Move** lands
   on its target.

There is no scale factor to measure and none is stored — the raw counts *are* the tuning. This is
why the moves are hand-tuned rather than computed from pixel deltas.

## Empty-result detection (what "advance on empty" actually compares)

The composer has to decide whether the result box still holds a gem. It compares the box's live
pixels against **`config/calib_gem_result.png`** — the crop of the box saved by **Save Gem Composer**
— and counts the pixels that differ by more than 30 on any channel:

| | measured (62×59 crop) |
|---|---|
| empty box vs the saved crop | **0.000** — pixel-identical |
| box holding a gem | **0.55** |

**The comparison skips the box's drawn border** (6 px at each edge). The frame moves by a pixel when
the game window moves, and those few rows were enough to make an *empty* box score 7.7 % — above the
gate, so the composer never advanced again. Looking only at the interior fixes it exactly: empty
0.0 %, gem 55 %, and the gem is drawn in the middle so nothing is lost.

`gem.empty_distance` is that fraction (default **0.01**): below it the box is "empty". The margin is
enormous on both sides, and the test is deliberately **colour- and shape-blind** — it asks "is this
still the same picture?", so a red, green or blue gem, or a different grade's shape, all read the
same. (A colour average cannot do this: `dominant hue` measured *identical* for empty and gem,
because the pale slot dominates the average in both.)

Two consequences for calibration:

- **The reference crop must show an EMPTY box.** Save Gem Composer asks you to confirm this; if the
  box has a gem, answer **No** — empty detection stays off rather than capturing a gem as "empty".
- **The empty reference is taken from the launcher-hidden screenshot**, not a live screen grab, so a
  window covering the box at save time can't contaminate it.

**The check only judges while the game is the foreground window.** The crop comes from
`CopyFromScreen`, so it shows whatever is *in front* — measuring the launcher's own dark UI once
produced `RGB(26,26,46)` and a meaningless verdict. When the game isn't in front the check answers
"not empty" instead (the safe direction: it keeps combining rather than advancing a grade) and writes
`refused: not foreground (fg="…")` to the log.

If the crop is missing, the composer falls back to the older colour-signature comparison, which
averages the whole box and is the weaker test. With `gem.save_empty_captures: true` every check
writes the crop plus a line to `<bin>\logs\empty_check.txt` (`diff=0.000 threshold=0.01 empty=True`)
— that is the number to look at if detection ever misbehaves.

## 7. Files written by calibration

| File | Written by |
|------|-----------|
| `config/local.yaml` | Save Tuner / Save Gem Composer / Save Coordinates / **Save tuned counts** / Save Calibration (Buy·Sell) / Save Preset (spammer) / Save Setup |
| `config/calib_tuner.png` | Save Tuner (reference screenshot with the bands drawn on it) |
| `config/calib_gem.png` | Save Gem Composer (reference screenshot with the click points + result box) |
| `config/calib_gem_result.png` | Save Gem Composer (crop of the result-gem area) |

The Buy / Sell tab writes **only** `local.yaml` — it saves no screenshot. Every mark it records is
verifiable on screen instead, by the 64 slot centres and the derived shop rows.

## 8. Removed v1 mechanisms (do not look for them)

The old Python build's calibration aids no longer exist in v2, and older copies of this document
described them:

- **No web panel** — there is no `http://127.0.0.1:5003` UI; calibration is in the WPF launcher.
- **No `--autoanchor` / `--diagnose` flags** — the launcher takes no arguments. Auto-anchor is
  documented as a planned feature but is **not implemented**; `reference_window` in `defaults.yaml`
  is currently unused.
- **No `gem.mouse_scale` and no "Probe scale" button** — the scale-based relative-move computation
  was removed; moves are raw hand-tuned counts.
- **No `display.dpi_scale`** — the process is DPI-unaware and the key was removed.

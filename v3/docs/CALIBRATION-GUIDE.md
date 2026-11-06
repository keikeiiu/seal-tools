# Calibrating v3: a practical walkthrough

This is the click-by-click companion to **USER_GUIDE §6**. It assumes the v3.0.10 **Calibrate** tab and
covers the part that is easiest to get wrong: **capturing, then clicking the capture and drawing on
it**. The reference for what a mark means is `spec/CALIBRATION.md`; the list of all 17 marks is
USER_GUIDE §6.2.

---

## 1. Before you start

- Run the game **windowed** (or borderless windowed) at the display size and Windows scale you
  normally play at.
- **Calibrate once per display setup.** If you change monitor, resolution, Windows scaling or move
  the game to another PC, recalibrate; v3 deliberately does not scale old marks.
- Put the **game window in front** for every capture. The shell hides itself for one frame and
  photographs the game, not the shell.
- Have the game screen that contains the thing you are calibrating visible before you capture.
  Examples: the shop bag open, the pet menu open, the tuner's status window open, the tooltip
  item hovered.

## 2. The Calibrate screen at a glance

| Area | What it is for |
|---|---|
| **Captures** (left, top) | The frames this Calibrate screen took. Capture, select a frame, see the `n / 4` cache count. |
| **Targets** (left, below Captures) | The 17 marks, grouped by tool. A named-instance mark expands to its targets. A tool's sessions are listed inside that tool's group with a `session` label. |
| **Picture/canvas** (middle) | The captured game frame. **This is where you click or drag to make a mark.** Fit / 1:1 / zoom / pan controls sit above it. |
| **Mark editor** (below the canvas) | **Surface**, **Target instance**, the typed `x / y / w / h` fields, the gem empty question, **Save mark**. |
| **Sessions** (bottom) | The two multi-mark calibrations: Tuner OCR 3-box + Spring point, and the Pet tooltip offset. They take their own captures and do not use the Captures shelf. |

## 3. Capture, then click the capture — the step people miss

1. **Bring the game to the front.** Click the game window; Seal Tools must not own the foreground.
2. In **Calibrate → Captures**, press **Capture the window** (after the first one the button says
   **Capture another**).
3. The shell hides for one frame, captures the game's client area, and reopens. A tile labelled
   **Frame 1 · hh:mm** appears under Captures, and the picture appears in the middle.
4. Look at the Captures header: **`0 / 4` → `1 / 4`**. That is *how many frames are currently kept*
   out of the **four-frame cache**, not "four captures required". **One live capture is enough to
   calibrate.** A fifth capture evicts the oldest.
5. **Click the capture tile itself to select it.** The selected tile gets a highlighted background,
   a check icon and a thicker border. The newest capture is selected automatically; click an older
   tile only when you deliberately want to save against that older frame.
6. The tile also says **`id f1`** (the live frame id a save needs), **`next capture evicts this one`**
   when it is the oldest of four, or **`refused by the core — capture again`** if that frame can no
   longer be saved against.
7. **Saving always uses the selected capture's frame id.** If you are about to take a fifth capture
   while using the oldest frame, save first.

> **Clicking a capture tile only selects which frame to draw on.** It does not place a mark itself.
> Marks are made by clicking/dragging the large **picture**, described next.

If the shell reopened but the picture is black/wrong, the game was not in front or was covered:
click the game, then press **Capture another**. A refusal sentence under Captures is the core's
answer, not a silent failure.

## 4. Choose the surface (and target)

1. Pick a **Surface** in the mark editor, or click a kind in the **Targets** tree. Both do the same
   thing: select the mark's kind and load its stored value.
2. If the surface is a **named-instance** kind, choose a **Target instance** as well (for example
   gem grade `DG`, pet row `2`, bag page `ITEM1`). **Save mark stays disabled until a target is
   chosen.**
3. Switching to another surface clears the target on purpose, so a mark cannot be saved against the
   wrong instance by habit.

## 5. Draw the mark on the picture: click vs drag

The shape is listed next to the surface; the picture only responds in that shape.

**Point surfaces** (e.g. Focus/scroll point, MAX button, grade point, pet menu/feed points):

1. Click **once** on the exact spot in the picture.
2. The mark appears and the `x / y` fields fill in. Type different numbers if you want exact values.
3. Move the point by clicking again, or by editing the fields.

**Rectangle surfaces** (e.g. Bag grid, Bag slot, Shop list region, result gem area, pet row rects):

1. Press the left mouse button at one **corner** of the target area.
2. Keep the button down and drag to the **opposite corner**; release.
3. The rectangle is drawn and `x / y / w / h` fill in.
4. A drag shorter than **4 px** is ignored as too small: drag farther, or type the fields.
5. To adjust, drag again to replace the rectangle, or type exact numbers.

Rules that apply to both shapes:

- Click **inside the game picture**. Clicks in the letterbox/black surround are ignored, not clamped
  into a mark.
- If the picture is zoomed, **Fit** shows all of it; **1:1** is one display pixel per frame pixel;
  the mouse wheel zooms and **Pan** moves a zoomed view. Marks are converted through the current
  zoom/pan, so draw where the pixels are on screen.
- The typed **x / y / w / h** fields are authoritative: drawing just fills them, and **Save mark**
  sends the fields. When in doubt, type the values from the game's own pixel coordinates.
- A rectangle's `w`/`h` must be positive; a bag slot must agree with the bag grid's pitch, otherwise
  the core refuses with its own sentence.

## 6. Save and verify a normal mark

1. For **Result gem area** only, answer the empty-result question first: **the result box is EMPTY
   right now**, or **a gem is in the box**. Save stays disabled until you answer.
2. Press **Save mark**.
3. Read the result line: it names the surface, the new revision and the mark. A `bag_grid` also shows
   its derived pitch; the result-gem crop appears under **Artifact**.
4. **Refresh stored** re-reads the stored marks. The **Stored mark** section shows what the core has
   for the selected surface/target, or "Not calibrated yet".
5. A refusal is text from the core, not a lost click. Fix the mark or the target and save again.

## 7. Recommended order for a fresh install

1. **Shop:** Bag grid → Bag slot → Shop list region → MAX button → Focus/scroll point.
2. **Gem:** Result gem area (answer the empty question) → grade point for each grade you use →
   resource slot points for the slots you use.
3. **Pet:** menu point, feed point, Boarding X point, bag page tab points, then boarding bag grid +
   slot, then the row rects for each row you use. Use the mark editor for each one — there is no
   separate Pet rows/crops session any more (it duplicated these marks).
4. **Tuner:** the **Tuner OCR 3-box + Spring point** session (§8.1) — one capture for all four
   tuner targets.
5. **Pet tooltip:** the hover offset + shape session (§8.2).

Calibrate the marks a tool actually reads before starting that tool; a missing mark is reported by the
tool, not guessed.

## 8. The two session-shaped calibrations

These panels sit under **Sessions** at the bottom of Calibrate. They each take their **own** capture
(the Captures shelf is for the normal marks above) and save several related values in one go. They
also draw their frame; the typed fields stay authoritative.

### 8.1 Tuner OCR 3-box + Spring point

Purpose: the tuner's three-place read (grade letter, three attribute rows, remaining count) plus the
optional Spring point — **all from one tuner capture**, like v2.

1. Open the tuner's status window in the game, then in the **Tuner OCR 3-box + Spring point** card
   press **Capture screen**. This one capture is used for the three OCR boxes **and** the Spring
   point; you do not take a second capture for the spring mark.
2. Drag the three boxes **in order**:
   **Grade letter** (green) → **Attribute rows** (blue) → **Remaining count** (amber).
   A drag fills the selected card and advances to the next. Click a card/chip to go back. Each card
   shows its own `x / y / w / h` and pixel size; a drag under 4 px is ignored.
3. Optional: select **Spring point** and click it on the same picture; set **row height** only if you
   want to pin the attribute pitch instead of letting the check measure it.
4. Press **Check boxes**. The result card shows the derived region, row pitch, grade, remaining
   count, filter verdict and how many attribute rows matched. Compare it with the game; fix boxes and
   check again if needed.
5. Press **Save tuner**. This writes `tuner.ocr` + `tuner.spring_point` and the annotated
   `calib_tuner.png`; the status line shows the core's revision.

### 8.2 Pet tooltip offset + shape

Purpose: where the hover panel appears relative to the cursor, and how large it is.

1. Put the game in front and open the item/pet so hovering at your chosen point will produce the
   panel. In the **Tooltip offset + shape** card set **hover x / y** and the delay, then press
   **Capture hover**. The core places the cursor there and captures; the picture returns with a pink
   cursor crosshair.
2. Drag a box around the panel — the core requires at least **20 × 20 px** — or type **offset x / y**
   and **width / height**. The formula line reads `cursor + offset = panel top-left`; the panel box
   is drawn over the cursor.
3. Press **Save tooltip** (writes the machine `tooltip` block: offset, size, delay).
4. Press **Test read** to hover again at the same point using the saved values. The **Read result**
   card lists the OCR lines; the debug crop is under `logs/reads/`.

The nine Pet marks (menu/feed/close/page points, bag grid/slot, row rects) are normal marks in the
mark editor above — there is no separate Pet rows/crops session. Rows and queue pets are edited on the
**Pet** tab.

## 9. Troubleshooting

| Symptom | Likely cause and fix |
|---|---|
| Capture returns a red refusal / shell reopens with no tile | The game was not the foreground window (or was minimised). Click the game, then **Capture another**. |
| Capture is black or shows Seal Tools | Same foreground issue; click the game and recapture. |
| A click on the picture does nothing | The selected surface is a **rectangle** (drag instead), or you clicked the letterbox/black surround. For a point surface, click inside the game image. |
| A drag produces no mark | Drag is under 4 px, or the surface is a **point** (click). Drag farther / switch shape. |
| Save mark is disabled | No target chosen for a named-instance surface, no capture selected, or the gem empty question is unanswered. |
| Save is refused after a drag | The core's rule (e.g. bag slot vs grid pitch, tooltip under 20×20, target not in config). Read the sentence and correct the mark. |
| `next capture evicts this one` is showing | You are on the oldest of four frames. Save before capturing again, or the save will be refused as an unknown frame id. |
| Marks are offset after changing display settings | Recalibrate; do not reuse marks across a different resolution/scaling. |
| The target tree shows `0/4` on a row kind | Zero of that kind's four named instances are saved (not a capture requirement). |

## 10. One-screen summary

1. Game in front → **Capture the window**.
2. **Click the Frame tile** to choose which capture you are editing.
3. Choose **Surface** (+ **Target instance** if needed).
4. **Point = click; rectangle = drag** on the large picture, or type `x / y / w / h`.
5. Answer the gem empty question if asked → **Save mark**.
6. For multi-mark work use the **Sessions** card: Tuner OCR 3-box + Spring point (one capture), or the
   Pet tooltip offset — each with its own Capture button. Everything else is a normal mark above.

# Seal Tools v2 — User Guide

**繁體中文版：[USER_GUIDE.zh-TW.md](USER_GUIDE.zh-TW.md)**

Every tab and button in the launcher, what it does and when to use it. For *why* the coordinates
work the way they do, see [COORDINATES.md](COORDINATES.md); for the step-by-step calibration flow,
see [CALIBRATION.md](CALIBRATION.md).

> Only one tool runs at a time — all three share the single Arduino COM port.

---

## The window

```
┌─ Tool cards ────────────────────────────────────────┐
│  Magic Tuner      [Start] [Stop]                    │
│  ● RUNNING / stopped  + live status                 │
│  Gem Composer     [Start] [Stop]                    │
│  Skill Spammer    [Start] [Stop]                    │
└─────────────────────────────────────────────────────┘
 Tuner | Gem | Spammer | Attributes | Calibrate Tuner | Calibrate Gem | Arduino | Setup | Settings
```

### Tool cards

| Control | What it does |
|---|---|
| **Start** | Opens the Arduino port if needed, then launches that tool. If the Arduino can't be found you get a message box saying why (check the **Arduino** tab). |
| **Stop** | Cancels the tool, waits for its loop to exit, releases the port. |
| Live status | Refreshes ~every 750 ms: `● RUNNING` / `● paused`, then `Grade`, `Remaining`, `Attempt`, `Cycle`, `Current`, the matched attribute lines, `Filter:` and any `⚠` warning (e.g. an unsupported spammer key). |

Starting a tool also makes it roll immediately — there is no separate "go" step, though the in-tool
F12 hotkey can pause/resume a tuner or composer while it runs.

---

## Tuner tab

Filter and stop conditions for the Magic Tuner. Saved to `config/defaults.yaml` (portable).

| Control | Meaning |
|---|---|
| **Target grade** | The grade the tuner rolls toward (`N → G → DG → XG → SG`). It stops when this grade (or better) is reached. |
| **Max retries** | Safety cap on attempts. |
| **Click delay (s)** | Pause after the Arduino click before the Enter. |
| **OCR delay (s)** | Pause after Enter before reading the screen — too short reads the old result. |
| **Filter enabled** | Turns the attribute filter on/off. When off, only the grade matters. |
| **Match mode** | `any` (one rule is enough), `all` (every rule must match), `per_attr` (each rule needs its own count of matching attributes). |
| **Require grade** | Grade floor for a filter match; `None` means "any grade". |
| **Save OCR captures** | Writes the OCR region to `logs/captures/` on every scan (debug only — it fills disk). |
| **Rules** | The goal list: attribute + count + min/max value, `✕` to remove, **+ Add Rule** to append. |
| **Override rules** | If any of these match, the tuner stops immediately regardless of the grade. |
| **Save Tuner Config** | Writes the above to `defaults.yaml`. |
| **Clean up captures** | Deletes `logs/captures/*.png` and reports how many were removed. |

## Gem tab

Behaviour of the Gem Composer. Saved to `config/defaults.yaml`.

| Control | Meaning |
|---|---|
| **Start grade** | Grade the composer begins on (`N` / `G` / `DG`). |
| **On empty result** | What to do when the composed-result box is empty: **Stop**, **Advance to next grade**, or **Clear resources, then advance** (right-clicks the three resource slots first — for a stuck gem). |
| **Save empty-check captures** | Writes the sampled result-box crop and a distance log each cycle (debug only). |
| **Save Gem Config** | Writes the above to `defaults.yaml`. |

## Spammer tab

Key rotation for the Skill Spammer. Saved to `config/defaults.yaml`.

The Arduino supports digits **0–9** and **F1–F10**; prefix a key with `*` for the fast hold. Anything
else is skipped and reported as a `⚠` on the tool card.

| Control | Meaning |
|---|---|
| **Preset** dropdown | Which named key set the spammer presses. Switching keeps unsaved edits in memory. |
| **name** box + **+ New** | Create a new empty preset with the name you type. |
| **Rename** | Move the current preset's keys to the typed name. |
| **Delete** | Remove the current preset (the last one can't be deleted). |
| Key rows | One row per key: the key, its cooldown in seconds, `✕` to remove. |
| **+ Add Key** | Adds an empty row. |
| **Advanced** | Reveals the raw `key:seconds` list for the current preset. Ticking it fills the text from the rows; unticking rebuilds the rows from the text. |
| **Save Spammer Config** | Writes the current preset and marks it active. |

## Attributes tab

A read-only view of the OCR attribute dictionary (`config/attributes.yaml`): **Name** (what a filter
rule matches), **Category**, and the **OCR variants** that are auto-corrected to that name. No
controls — edit the YAML if you need to add an attribute.

## Calibrate Tuner tab

Points the tuner's OCR at the 發條 (Magic Tuning) window.

| Control | What it does |
|---|---|
| **Capture 發條 window** | Grabs the game window in physical pixels. The launcher hides itself for ~0.3 s so it can't cover the game — the blink is expected. The image must show the whole game. |
| Canvas | Drag three boxes in order: **grade letter**, **the 3 attribute lines**, **spring count**. Each is colour-coded (green / blue / orange). A tiny drag is rejected. |
| **Check OCR** | Runs the full read → match → filter pipeline on the current screen. Uses the boxes you just dragged; if you haven't dragged all three, it uses the **saved** calibration and says `(checking the saved calibration from local.yaml)`. Output: grade, spring count, the three attribute lines, `Matched:` (what the dictionary recognised) and the `Filter:` verdict. |
| **Save Tuner** | Writes the region + sub-bands + measured row height to `config/local.yaml`, records the display environment, and saves `config/calib_tuner.png` (the screenshot with your bands drawn on it). |

## Calibrate Gem tab

Points the composer at the gem-combine UI and stores the relative move counts.

| Control | What it does |
|---|---|
| **Capture gem window** | Same physical-pixel grab, launcher hidden during it. |
| Canvas | Click, in order: **N, G, DG, Register, Combine**, then the **3 resource slots**, then drag a box around the **composed result gem**. |
| **Diagnose capture** | Reports the window's frame/client rects and the non-client offset, and saves `logs/captures/diag_capture.png` — use it to confirm the launcher isn't covering the game. |
| **Save Gem Composer** | Writes positions, resource points and the result area to `local.yaml`. It then asks whether the result box is **empty** — answer **Yes** to sample the empty-colour reference used by empty detection, **No** to leave it off. Also saves `calib_gem.png` and the result-box crop. |
| **Coordinates** grid + **Save Coordinates** | Type X/Y (and W/H for the result area) directly instead of re-capturing. |
| **from / to** + **Test Click** | Moves the cursor to the selected point **and clicks it** (Arduino `C`). |
| **Test Move (rel)** | Click `from`, send that route's raw `D dx dy`, click `to`. Confirms a composer move lands. |
| **Check Result Colour** | Samples the result box now and reports its colour plus the distance to the empty reference and the empty/has-gem verdict. |
| **Test Result Gem** | Same sample with extra detail (channel spread, dominant tone) for judging the empty-detection threshold by hand. |
| **Debug Cursor (logical)** | Moves the cursor to the selected point using `SetCursorPos` on the converted coordinates and prints the computed target, whether the API accepted it, and where the cursor ended up. **No click.** Diagnostic only — the tools place the cursor with the Arduino, not this call. |
| **Debug Physical** | Same, but with `SetPhysicalCursorPos`. Kept for comparing the two APIs. |
| **Composer moves** grid | One row per route (N→Register, G→Register, DG→Register, Register→Combine, Combine→Register, Register→Resource1, Resource1→Resource2, Resource2→Resource3, Resource3→N/G/DG) with raw `dx`/`dy` and a **Test** button per row. |
| **Save Composer Moves** | Writes `gem.movements` to `local.yaml`. These are hand-tuned HID counts — not derived from pixels, and specific to your Arduino + pointer speed + in-game display. |
| **Composer move mode** | `tuned` (default) = the composer sends the hand-tuned counts above. `arduino` = it places the cursor on each route's destination point with the Arduino, closed loop — no tuning, and it re-aims every move. Saved to `defaults.yaml` by **Save Gem Composer**. See [MOVE-SETS.md](MOVE-SETS.md). |
| **New Gem Composer Moves** grid | The same routes, each shown with the **point it goes to**, and a **Test** button that clicks the source point, places the cursor on the destination point and clicks. This is what the composer does in `arduino` mode, so a route that lands here lands in the composer. |

## Arduino tab

Connection diagnostics.

| Control | What it does |
|---|---|
| Status light | Green when a serial device matching the configured VID/PID is present. |
| Port list | Every serial port the OS sees, with `>>` marking the match, plus the expected VID/PID. |
| **Refresh** | Re-runs the scan (it is not live). |
| **Test Click (C)** | Opens the port and sends a real Arduino click — the end-to-end check that the device is alive. |

## Setup tab

Records the display environment the calibration was measured in.

| Control | What it does |
|---|---|
| **Detect** | Measures the game window: monitor physical size + DPI + scale, and the window's frame/client rects. Fills the fields below. |
| **Scale** | Physical pixels per logical pixel (e.g. 1.5). Detected automatically; editable if the detection is wrong. |
| **Reference client size** | The game client size (physical) the calibration belongs to. |
| **Save Setup** | Writes the `calibration:` block to `local.yaml`. |
| Stored calibration | Shows the saved scale / client size / timestamp. |
| ⚠ warning | Appears when the live client size differs from the stored one — **recalibrate**, don't trust the old coordinates. |

## Settings tab

Global hotkeys. Type a name: `F1–F24`, `Esc`, `CapsLock`, `Space`, `Tab`, `Enter`, or a single
letter/digit.

| Control | Meaning |
|---|---|
| **Start / stop rolling** | F12 by default — toggles the running tool. |
| **Quit (immediate)** | F11 by default — stops the tool at once. |
| **Advance grade (gem)** | `G` by default — advances the composer to the next grade. |
| **Pause (graceful stop)** | CapsLock by default — finishes the current cycle, then stops. |
| **Save Hotkeys** | Writes them to `defaults.yaml`. |

> **Hotkeys only work when the game is *not* focused.** The game's anti-cheat blocks background key
> reads, so while you are in-game F11/F12/CapsLock do nothing. Click the launcher first (or use the
> card's **Stop**), then the key works.

---

## Typical workflows

**First time on a machine**

1. **Setup** → **Detect** → check the scale/client size → **Save Setup**.
2. **Calibrate Tuner** → **Capture 發條 window** → drag the three bands → **Check OCR** (must read a
   real grade, the count and three attribute lines) → **Save Tuner**.
3. **Calibrate Gem** → **Capture gem window** → click the points + drag the result box →
   **Save Gem Composer** → **Save Composer Moves** → **Test Move** one route.
4. **Arduino** → **Refresh** → **Test Click (C)** to confirm the device.

**Verifying an existing calibration** (no capture needed)

- **Calibrate Tuner** → **Check OCR** straight away: it tests the saved geometry and prints the
  matcher + filter result.
- **Calibrate Gem** → **Test Click** a point; **Check Result Colour** for empty detection.

**Running**

- **Start** on a card. Stop with **Stop**, or F11 / CapsLock after clicking the launcher.

---

## Troubleshooting

| Symptom | Likely cause / fix |
|---|---|
| Start does nothing, message box "Arduino not found" | Wrong port/VID/PID, or the device is unplugged — check the **Arduino** tab. |
| Capture shows the launcher or a partly-black image | The game must be visible; move the launcher off it and use **Diagnose capture** to check. |
| Check OCR reads the wrong text | The tuning window moved, or another in-game window covers it. Re-capture and re-drag. |
| Composer clicks drift | "Enhance pointer precision" is on, or the Arduino/pointer speed changed — re-tune `gem.movements` with **Test Move**. |
| Everything is off after moving to a new monitor/resolution | Open **Setup**: if the scale or client size differs, recalibrate. |
| Spammer never presses a key | The key isn't a digit or F1–F10 — the card shows a `⚠`. |
| Hotkeys dead while playing | Expected: focus the launcher first (see Settings). |

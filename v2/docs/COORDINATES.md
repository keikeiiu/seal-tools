# Seal Tools v2 — Coordinate Space, DPI and Capture

Decision record for how screen coordinates are measured, stored and used. Read this before
changing anything in `WindowFinder`, `ScreenCapture`, `GemPointer` or the calibrators.

---

## The model in one table

| | Decision |
|---|---|
| Canonical coordinate space | **physical pixels, client-area-relative** |
| Process DPI awareness | **unaware** (unchanged — see "Why we stay unaware") |
| How physical values are obtained | a **thread-scoped** awareness switch, used only for measurement and capture |
| Cursor moves | physical → logical conversion (`SetPhysicalCursorPos` if it works, else `SetCursorPos(physical / scale)`) |
| The scale | **measured at runtime**, never hardcoded |
| `gem.movements` | raw HID counts — **not** coordinates, never scaled |
| `gem.empty_signature` | colour composition — **not** coordinates, never scaled |

## The problem this solves (measured 2026-09-09)

The launcher captured only the top-left **2/3** of the game: the minimap, the skill hotbar and the
chat window were missing from every screenshot, and the same capture path feeds OCR and the
composer's empty-box check.

Measured on the game window:

| | physical (DPI-aware) | unaware process (÷1.5) |
|---|---|---|
| screen | 3840 × 2160 | 2560 × 1440 |
| window frame | 2889 × 1848 @ (0,0) | 1926 × 1232 |
| **window client** | **2865 × 1789 @ (12,47)** | **1910 × 1193 @ (8,31)** |

`GetClientRect`/`ClientToScreen` report the divided-down (logical) values, but the screen blit is
1:1 physical. Requesting a 1910 × 1193 region therefore grabbed the top-left 1910 × 1193 *physical*
pixels of a 2865 × 1789 client — a crop, not a scale.

## Why we stay DPI-unaware

Commit `7f9e1c7` (2026-09-08) recorded the reason verbatim:

> SetCursorPos/SetPhysicalCursorPos return ok=False on a mixed-DPI two-monitor setup when the
> process is PerMonitorV2-aware.

That is a hard blocker: with `PerMonitorV2` the cursor API **fails outright** on this machine's
dual-monitor setup, so no amount of coordinate conversion saves it. The process stays unaware.

**Consequence:** every Win32 call in the process is virtualised by the system scale, so
`GetDpiForWindow` returns **96** (confirmed in `logs/ocr_log.jsonl`) and cannot tell us the real
scale. The scale has to be *measured*.

## How the scale is measured

The same window is read twice — once normally, once on a thread temporarily switched to
`DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2` via `SetThreadDpiAwarenessContext`:

```
scale = physical client width ÷ logical client width
      = 2865 ÷ 1910
      = 1.5
```

Cross-check: `3840 / 2560 = 1.5`. Note that `GetDpiForWindow` is **not** usable here — it reports
the *window's* awareness (96 for this DPI-unaware game window, even on an aware thread), not the
monitor's scale. The effective DPI shown in the UI is derived as `96 × scale`.

- **Widths, not heights** — `1193 × 1.5 = 1789.5` truncates to 1789 (0.03% error); the width ratio
  is exact.
- **The ratio, not `GetDpiForWindow`** — the ratio is the actual transform between the two
  coordinate spaces, and it stays correct if the game window sits on a monitor whose scale differs
  from the system scale. The DPI value is shown in the UI as a cross-check.
- **Nothing is hardcoded** — a 125% machine yields 1.25, a 100% machine 1.0.

### Hard rule

`SetThreadDpiAwarenessContext` is **thread-scoped and restored in a `finally`**. No `SetCursorPos`
or `SetPhysicalCursorPos` call may ever run while a thread is aware — that is exactly what broke in
`7f9e1c7`. The switch exists only inside `Dpi.Measure` and the capture.

## Capture

- **`CopyFromScreen` only.** `PrintWindow` was measured to return a **solid black frame** for this
  game (see `ScreenCapture.cs`); do not reintroduce it.
- The blit is 1:1 physical, so the capture must be given **physical** rects and must run on the
  aware thread. Result: the full 2865 × 1789 client.
- `CopyFromScreen` reads what is on screen, so the launcher hides itself for the grab (it would
  otherwise be painted into the image).

## What is and is not scaled

| Scaled (coordinates) | Not scaled |
|---|---|
| `tuner.ocr.region`, `grade_area`, `grade_y` / `attr_y` / `remaining_y`, `attr_x` / `remaining_x`, `row_height` | `gem.movements.*` — raw HID counts sent as `D dx dy` |
| `gem.grade_positions`, `gem.resource_gems`, `gem.result_gem_area` | `gem.empty_signature`, `gem.colored_gap_min`, grade colour masks |

Scaling a HID count or a colour threshold by the display scale would be nonsense — they have no
pixel-space meaning.

## Portability (using this on another PC)

`local.yaml` records the environment the calibration was measured in:

```yaml
calibration:
  dpi_scale: 1.5
  monitor_dpi: 144
  screen: [3840, 2160]
  client_size: [2865, 1789]
```

- **Different scale, same game resolution** → the stored physical coordinates remain valid; the
  only difference is the physical → logical conversion for cursor moves, which uses the *current*
  measured scale.
- **Different game window size** → **recalibrate.** The Setup tab warns when the live client size
  differs from the stored one. We deliberately do **not** auto-anchor by the size ratio: game UI
  does not scale linearly with the window (the HP bar keeps its pixel size, the world view expands),
  so a ratio transform would place things wrongly. This is why `reference_window` stayed unused.

## Migration

Existing `local.yaml` files hold **logical** coordinates (measured before this change), so they are
wrong in the new physical space. The stance is: **recalibrate** — those numbers came out of the
cropped capture, so they are not worth converting. `gem.movements` and `empty_signature` survive
untouched because they are not coordinates.

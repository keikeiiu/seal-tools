# Quick dock size presets — design note

## The issue

`DockSize` currently scales only the dock window (`DockSize.scale()` → placement size). The tile title
(13 px), summary (10.5 px), icon (30 px), row height (44 px), Start button (64 × 28) and its label
(12 px) are hard-coded constants in `quick_dock.dart`.

Large therefore gets a **1.25× window with a 1.0× UI**. The extra space makes the Start label look
small next to the title and leaves the action column with no visual weight.

## Proposal

One `DockMetrics` table per preset; derive it from `model.dockSize` and pass it through the header,
tiles, footer and collapsed form.

| Metric | Compact | Standard | Large |
|---|---|---|---|
| Window (bay) | 306 × 414 | 340 × 460 | 425 × 575 |
| Header label | 9 px | 9.5 px | 11 px |
| Tile title | 12.5 px | 13 px | 15 px |
| Tile summary | 10 px | 10.5 px | 11.5 px |
| Row min height | 40 px | 44 px | 54 px |
| Icon tile / icon | 28 / 15 px | 30 / 16 px | 36 / 19 px |
| Start button | 58 × 26 · 11.5 px | 64 × 28 · 12 px | 76 × 34 · 13.5 px |
| Options button | 26 px | 28 px | 32 px |
| Header control | 24 px | 26 px | 30 px |
| Footer button | 30 · 12 px | 32 · 12 px | 40 · 13.5 px |
| Row padding x / y | 6 / 3 px | 8 / 4 px | 10 / 6 px |

## Five rules

1. **One metrics object per preset** — no fixed `fontSize: 13` / `height: 28` in the tile.
2. **Fixed action column** — a Start/Stop button of `actionWidth` × `actionHeight`, centred in every
   row; Start and Stop never change the column width.
3. **Type ramp, not window factor** — Large uses 15 / 11.5 / 13.5; Compact stays legible at 12.5 / 10 /
   11.5 even though its window is 0.9×.
4. **Status stays out of the action geometry** — the running chip belongs in the title line, never
   between the flexible text and the fixed action column.
5. **Same tokens across placements** — rail, bay and island differ only in window dimensions; the
   metrics table follows the size preset.

## Demo

Open `quick-dock-sizes.html` directly in a browser.

- Large before/after, both at the real 425 × 575 window size.
- Compact / Standard / Large at 1:1.
- Live Setup size selector, Start/Stop toggles, dark/light and alignment guides.
- `?theme=light` and `?selftest=1` (6 interaction checks) are supported.

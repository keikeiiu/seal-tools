# Seal Tools v3 — full UI demo

`ui-demo-full.html` is a self-contained interactive prototype of the shipped v3 shell (design review only;
no app code or backend is involved).

## Open it

Open `ui-demo-full.html` directly in a browser. No server or build step is needed.

## What is included

- All 13 registered screens: Cards/Run, Skill Spammer, Magic Tuner, Gem Composer, Buy, Sell,
  Quest Hand-in, Pet Feeder, Attributes, Arduino, Hotkeys, Setup and Calibrate.
- Dark and light appearance, a quick appearance button in the title bar, and the Setup appearance
  selector.
- The current layouts: Tuner/Gem colour-only grade ladders with the minimum selector under the
  ladder, the two-column Pet Feeder with readiness summary and folded advanced settings, and the
  fixed Calibrate workspace with tool tabs and dialogs.
- Working interactions: rail navigation, collapsibles, switches, page tabs, bag/food cells, queue
  removal, save/dirty states, Sell selection and cap count, attribute search, Calibrate tool tabs
  and target selection, Captures/Details/Stored marks dialogs, logs drawer, zoom controls.

## Review helpers

- `?screen=pet` (or any rail key) opens a screen directly.
- `?theme=light` (or `dark` / `system`) starts in that appearance.
- `?selftest=1` runs the built-in 14-check interaction pass and prints the result in the lower-left
  corner (expected `SELFTEST 14/14`).

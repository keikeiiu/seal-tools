# Finished design studies

Interactive HTML prototypes that were reviewed and then implemented in the shipped shell. They are
kept here as design history and as a visual reference for the current behaviour.

These are **prototypes**, not the Flutter application: their data is fake and their controls simulate
the real UI. Each file is self-contained and opens directly in a browser — no server is needed.

| Demo | Status | Shows |
|---|---|---|
| [Full UI demo](full-ui-demo.html) | Implemented shell screens | All 13 destinations, dark/light, rail, Sets, Calibrate workspace |
| [Quick dock sizes](quick-dock-sizes.html) | Implemented (`f5f561c`, `8e93fec`, `c958127`) | Compact/Standard/Large metrics, fixed action column, Setup and header controls |
| [Mini bar text](mini-bar-text.html) | Implemented (`66fbc0e`) | 460 × 168 running bar: primary action line, labelled fact chips, Filter line, countdown/Stop |
| [Pet Feeder redesign](pet-feeder-redesign.html) | Implemented (`bad394a`) | Two-column Pet setup, readiness, folded advanced fields |
| [Calibration capture once](calibrate-capture-once.html) | Implemented (`012cdc9`, `9211175`) | Fixed Calibrate workspace, tool tabs and crop/check cards |

The implementation is in `v3/shell`; the specs and dated reasoning are in
[`v3/docs/spec`](../../spec) and [`v3/docs/records/PROGRESS.md`](../../records/PROGRESS.md).

Open [index.html](index.html) for the browsable cards.

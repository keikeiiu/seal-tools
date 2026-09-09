# Seal Tools v2 — Open Work

What is still open. Completed fixes and their commits are in [STATUS.md](STATUS.md); the design
guardrails (things not to reverse) are in [REVIEW.md](REVIEW.md).

---

## Needs a live check before any code change

- [ ] **OCR row-bucket pooling.** `BuildLines` buckets detected items by `y / row_height`, so a fixed
  grid can pool two nearby attribute rows into one.
  - Evidence already collected: `logs/ocr_log.jsonl` records every raw item with its `rowKey`. Collect
    a few real unconfirmed frames, then decide whether the grid or the detector is at fault.
  - Do **not** change the bucketing without that evidence.

## Settled (measured — do not reopen)

- **Capture method.** `PrintWindow` returns a **black frame** for this game; `CopyFromScreen` returns
  the real screen. Everything now captures with `CopyFromScreen` (client-area origin). Evidence:
  `logs/captures/diag_printwindow.png` vs `diag_copyfromscreen.png` (2026-09-09). The calibrator's
  **Diagnose capture** button reports the frame/client rects and saves a sample capture if you need to
  re-check.

## Verified working on the live game (2026-09-09)

Both tools are calibrated and tested on the reference machine. What was confirmed:

- **Setup tab → Detect** — scale 1.5, client 2865×1789; **Save Setup** writes the `calibration:` block.
- **Capture** — shows the whole game (minimap + hotbar); the launcher hides itself for the grab.
- **Cursor** — `Debug Cursor (logical)` and `Test Click → N` both land on the N radio; the logical
  `SetCursorPos` path is the one used (see [COORDINATES.md](COORDINATES.md)).
- **Composer moves** — the raw hand-tuned `D dx dy` counts are per-PC (Arduino + pointer speed +
  in-game display) and verified; **Test Move** lands.
- **Check OCR** — tests the in-session boxes after a drag, or the saved `local.yaml` geometry when
  nothing is dragged; prints grade, spring count, the three attribute lines, the matcher output and
  the filter verdict.

Remaining live checks (no code change expected):

- [ ] One full tuner run and one composer cycle end to end.
- [ ] Tuner `remaining_y` band (spring count) may need a manual nudge per machine — it is derived
  proportionally.

## Decisions waiting on you

- [ ] **`publish.bat` ships `local.yaml`.** Deliberate for personal/same-machine use. Revisit when the
  build is release-ready: either document it or exclude the file so a shared build starts uncalibrated.
- [ ] **`ReferenceWindowConfig`** is written to `defaults.yaml` but never read (auto-anchor is *not*
  implemented). Remove it, or leave it as a placeholder for the planned feature.
- [ ] **Debug Cursor / Debug Physical buttons** in the Gem calibrate tab — keep as diagnostics or remove.

## Out of scope

- Check-in stays the standalone Python script (`v1/checkin/checkin.py`).
- v1 (Python) is frozen — do not port further from it.

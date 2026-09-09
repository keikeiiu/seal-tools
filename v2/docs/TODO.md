# Seal Tools v2 — Open Work

What is still open. Completed fixes and their commits are in [STATUS.md](STATUS.md); the design
guardrails (things not to reverse) are in [REVIEW.md](REVIEW.md).

---

## Needs a live check before any code change

- [ ] **Calibration vs runtime capture origin.** The calibrator captures with `PrintWindow` (origin =
  window frame, includes the title bar); OCR and the composer capture with `CopyFromScreen` (origin =
  client area). If the game window has a title bar, the calibration image is offset from the coordinate
  space those coordinates are used in.
  - **Run the diagnostic:** with the game open, **Calibrate Gem → Diagnose capture**. It reports the
    frame and client rects plus the non-client offset, and saves both captures
    (`logs/captures/diag_printwindow.png` / `diag_copyfromscreen.png`) to compare.
  - Offset `(0,0)` → the origins coincide, nothing to fix. Non-zero → the saved calibration is shifted
    by that amount and needs recalibrating, or a one-time shift of the stored coordinates.
  - Do **not** switch the calibration path to `CopyFromScreen` blind — it was chosen for a reason, and
    the runtime path already works.
- [ ] **OCR row-bucket pooling.** `BuildLines` buckets detected items by `y / row_height`, so a fixed
  grid can pool two nearby attribute rows into one.
  - Evidence already collected: `logs/ocr_log.jsonl` records every raw item with its `rowKey`. Collect
    a few real unconfirmed frames, then decide whether the grid or the detector is at fault.
  - Do **not** change the bucketing without that evidence.

## Verify on a live game (no code change expected)

- [ ] End-to-end pass: **Calibrate Tuner → Check OCR**, then one full tuner run.
- [ ] **Calibrate Gem → Save Gem Composer**, then **Save Composer Moves**, then **Test Move** per route
  (with "Enhance pointer precision" off), then one composer cycle.
- [ ] Tuner `remaining_y` band (spring count) may need a manual nudge per machine — it is derived
  proportionally.

## Decisions waiting on you

- [ ] **`publish.bat` ships `local.yaml`.** Deliberate for personal/same-machine use. Revisit when the
  build is release-ready: either document it or exclude the file so a shared build starts uncalibrated.
- [ ] **`ReferenceWindowConfig`** is written to `defaults.yaml` but never read (auto-anchor is *not*
  implemented). Remove it, or leave it as a placeholder for the planned feature.
- [ ] **Debug Cursor / Debug Physical buttons** in the Gem calibrate tab — keep as diagnostics or remove.

## Out of scope

- Check-in stays the standalone Python script (`checkin/checkin.py`).
- v1 (Python) is frozen — do not port further from it.

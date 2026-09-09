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

## Recalibrate (required after the physical-coordinate change)

- [ ] **Setup tab → Detect** — confirm the monitor scale (1.5 here) and client size; **Save Setup**.
- [ ] **Calibrate Tuner → Capture** — the image must now show the whole game (minimap + hotbar).
  Drag the three boxes → **Check OCR** (a real grade + 3 attribute rows) → **Save Tuner**.
- [ ] **Calibrate Gem → Capture** — click N/G/DG/Register/Combine + the 3 resource slots, drag the
  result box → **Save Gem Composer** (with the result box empty when asked).
- [ ] **Save Composer Moves** — `gem.movements` survived the change, but verify one route with
  **Test Move** (pointer acceleration off).
- [ ] One full tuner run and one composer cycle.

## Verify on a live game (no code change expected)

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

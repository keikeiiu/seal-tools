# Seal Tools v2 — Open Work

What is still open. Completed fixes and their commits are in [STATUS.md](STATUS.md); the design
guardrails (things not to reverse) are in [REVIEW.md](REVIEW.md).

---

## Planned (design written, not built)

- [x] **Tuner spring positioning + cursor guard** — built on `v2-tuner-spring` (2026-09-11), not
  merged. `tuner.spring_mode` (manual/hid) + `spring_point`, a 4th Calibrate Tuner step + Test Click,
  and `tuner.mouse_guard` (off/stop/recenter). **Not verified live** — see the warp question below.
  Plan + the two data questions: [PLAN-TUNER-SPRING.md](PLAN-TUNER-SPRING.md).
- [ ] **A full tuner run and a composer cycle in the current build.** The residency work rewrote the
  shared Start/Stop path and only the pet tool has exercised it — see [HANDOVER.md](HANDOVER.md).
- [ ] **UI cleanup (tabs, buttons, text-box layout).** Make the launcher readable: one tab one job,
  controls grouped by intent, advanced/diagnostic bits behind a toggle, one shared row builder.
  **Study first** — the target structure has to be agreed before any code, or the layout churn gets
  redone. Inventory, problems, principles, five open questions and the tab-by-tab method:
  [PLAN-UI-CLEANUP.md](PLAN-UI-CLEANUP.md).

## Ideas for later (captured, not designed)

The wider backlog — reliability, safety, calibration UX, observability, code health, release and
game-side mini-features, each with a rough size and a "decide first" flag — is in
[IDEAS.md](IDEAS.md).

Mini-features worth having once the UI work lands. Each reuses what already exists — calibrated
points, the closed-loop cursor placement, the HID click, and a capture/OCR pass — so none of them
needs new machinery, just a small driver per feature.

- [ ] **Auto-sell items.** Calibrate the vendor's sell slot + the inventory grid; click an item, read
  whether it matches a keep-list, sell the rest. Needs a keep/sell rule set (the attribute matcher is
  the natural home) and a hard cap per run so it can't empty a bag by accident.
- [ ] **Auto-buy items.** The mirror image: calibrate the vendor's list + a quantity control, buy a
  configured amount, stop on gold or stock limits.
- [ ] **Auto-submit missions.** Calibrate the quest-list entry + the submit/complete button (and any
  confirmation dialog); click through the finished missions one by one. Needs a way to know when
  nothing is left to submit — a capture/OCR check on the list, or a fixed cap — so it stops rather
  than clicking empty rows.
- [ ] **Anti-AFK nudge** (toggle) — a tiny periodic movement, using the same placement code.
- [~] **Park the cursor before a capture.** Done for the BAG captures (2026-09-19) and for the feeder
  counts (2026-09-21, where the read also refuses when the game is not in front). **Still open for the
  OCR bands**: the tuner's read regions can have the pointer in them, and that one wants a measurement
  first; see [PLAN-TUNER-SPRING.md](PLAN-TUNER-SPRING.md).

## Needs a live check before any code change

- [x] **OCR row-bucket pooling** — resolved (2026-09-11). The "one line less" symptom was **not** a
  `row_height` pooling after all: the captures show all 3 lines are read, and the drop was in the
  matcher (OCR character misreads breaking the dictionary match). Fixed in `3b8023b` (added `国/盘/地`→`每`,
  `等增加`→`等級增加`, `幸莲`→`幸運`, `必毅技`→`必殺技` to `text_fixes`). No bucketing change was needed.

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

- [x] **`publish.bat` ships `local.yaml`.** Resolved (2026-09-11): `publish.bat` now has two modes —
  `public` (default) ships only the config templates, `local` ships your full `config\` incl.
  `local.yaml` + `calib_*.png`. A shared build starts uncalibrated; a personal build arrives calibrated.
- [ ] **`ReferenceWindowConfig`** is written to `defaults.yaml` but never read (auto-anchor is *not*
  implemented). Remove it, or leave it as a placeholder for the planned feature.
- [ ] **Debug Cursor / Debug Physical buttons** in the Gem calibrate tab — keep as diagnostics or remove.

## Accepted as-is (recorded, not fixed)

Real, but deliberately left alone. Listed so they are not rediscovered as bugs and "fixed" without
the context — three of the four would be *reverted* by a well-meaning cleanup.

- **A null attribute value satisfies a bounded filter rule.** `AttrMatcher.ValueOk` passes when the
  OCR read the attribute NAME but not its number, so a rule with `min`/`max` is satisfied by an
  unreadable value. **Deliberate**: 減少傷害 is rare enough that the name match is the signal, and
  failing a roll over a number that failed to read would mean missing one that should have stopped
  the run. Bounds still apply whenever a value *was* read. Pinned by `AttrMatcherTests`; comment at
  the method.
- **`Arduino.Find` duplicates `Diagnose` on purpose.** Both format the VID/PID hex and run the same
  WMI query, and `Diagnose` already computes the flag `Find` wants — but `Find` carries a name-based
  fallback for when the VID/PID lookup finds nothing, and `Diagnose` does not. The obvious dedup would
  silently delete that fallback, which exists for exactly the case where the device is not reporting
  its IDs properly. Comment at `Arduino.Find`.
- **Tuner mouse guard fails open.** `SealTuner.CheckMouseGuard` returns "no drift, carry on" when the
  game window can't be measured, so a window that is closed, renamed, minimized or alt-tabbed turns
  the guard *off* rather than tripping it, and the loop goes on to click wherever the cursor sits.
  Not fixed: it needs the window to move mid-run, which does not happen in normal use, and failing
  *closed* would change run behaviour in a way that wants a live run to confirm. Comment at the call
  site.
- **`Thread.Sleep` on the UI thread in the calibrator's test buttons.** `GemTestFullCycle` and the
  other HID test handlers sleep on the dispatcher — roughly 18 s of frozen window for a full cycle.
  Left alone because the real fix is not `await Task.Delay`: `HidPointer.To` blocks internally in
  `WaitForCursorToSettle`, so the whole test cycle would have to move off the dispatcher with progress
  marshalled back. That is a refactor of five handlers for a diagnostic button, not a two-line
  change. Revisit if the UI ever needs to stay responsive during a test run.

## Out of scope

- Check-in stays the standalone Python script (`v1/checkin/checkin.py`).
- v1 (Python) is frozen — do not port further from it.

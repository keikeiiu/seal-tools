# Seal Tools v2 — Open Work

What is still open. Completed fixes and their commits are in [PROGRESS.md](PROGRESS.md) — the older
[STATUS.md](STATUS.md) is a historical snapshot and stops at v2.4; **how it works and
why is in [DESIGN.md](DESIGN.md)**; the design guardrails (things not to reverse) are in
[REVIEW.md](REVIEW.md); finished plans, with their reasoning, are in [archive/](archive/README.md).

---

## Planned (design written, not built)

- [x] **Tuner spring positioning + cursor guard** — **shipped in v2.4** (branch `v2-tuner-spring`,
  built 2026-09-11 and merged). `tuner.spring_mode` (manual/hid) + `spring_point`, a 4th Calibrate
  Tuner step + Test Click, and `tuner.mouse_guard` (off/stop/recenter) — `defaults.yaml` ships
  `spring_mode: hid` and `mouse_guard: stop`. Reasoning and the two data questions that shaped it:
  [PLAN-TUNER-SPRING.md](PLAN-TUNER-SPRING.md).
- [ ] **A full tuner run and a composer cycle in the current build.** The residency work rewrote the
  shared Start/Stop path and only the pet tool has exercised it — see [HANDOVER.md](HANDOVER.md).
- [~] **UI cleanup (tabs, buttons, text-box layout).** The **WPF-UI rebuild shipped** — the plan's
  steps 0–9 are done (palette to theme brushes, then `Hotkeys`, `Arduino`, `Setup`, `Gem`, `Spammer`,
  `Attributes`, `Tuner`, `Calibrate Tuner`, `Calibrate Gem` rebuilt on `ui:Card`/`ui:` controls), and
  the `NavigationView` rail was **tried and reverted** — the tab strip read better and clicking a rail
  item did not switch the page. What is left of that plan is the button label/weight convention.
  **For a next pass**, the measured start is [ANALYSIS-UI.md](ANALYSIS-UI.md): extract the calibration
  canvas (copy-pasted 5×), collapse the four button factories into one, and the two cross-cutting
  items the earlier plan never covered — accessibility, and the status poll's dependency on a
  `ToolState` thread-safety claim that does not hold. Plan and its reasoning:
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

## From the 2026-09-22 audit — found, not fixed

A read-only audit of `Core`, the tool projects and the launcher UI. Nothing here was changed in
response, and **the two groups are not the same kind of claim**: the first four were read out of the
source and confirmed; the rest came back from the audit and have **not** been reproduced, so treat
them as leads rather than findings.

**Confirmed by reading the code:**

- **Four `PetConfig` properties cannot be set from either config file.** `ItemsPerMinute` (the 3/min
  burn rate), `PetSlotOccupiedAbove` (the slot-occupied threshold) and `FeederSlotA`/`FeederSlotB` have
  no `LocalPet` counterpart, so `local.yaml` can never carry them; and `SaveDefaults` writes an
  anonymous object with **no `pet` key**, so a `pet:` block hand-added to `defaults.yaml` is read on
  load and then **deleted by the next Save from any tab**. The first two are live settings the tool
  reads (`PetTool` reads both), which is what makes this worth fixing rather than deleting.
  `FeederSlotA`/`FeederSlotB` on `PetConfig` are dead — only the legacy `LocalPet`-era fields are read,
  by the migration. The existing guard test (`EveryLocalPetFieldIsCopiedFromTheConfig`) walks
  **LocalPet → PetConfig only**, which is why the reverse gap went unnoticed; the fix is a second
  test walking the other way.
- **`ConfigLoader.SaveLocal` / `SaveDefaults` have no mutual exclusion and share a temp filename.**
  Both do `File.WriteAllText(path + ".tmp")` then `File.Move(..., overwrite: true)`. Two writers of
  the *same* file — the WPF thread saving a calibration and the pet tool's `PersistPetState` — collide
  on that one `.tmp` name, and the read-modify-write around it is last-writer-wins. The comment on the
  method claims atomicity; it covers concurrent *readers*, which is not the case that exists.
- **`AttrMatcher` uses `int.Parse` on unbounded `(\d+)` captures** (`AttrMatcher.cs:79,88,95,104`).
  Every other parser in the project uses `TryParse`. An OCR line containing an 11+ digit run throws
  `OverflowException` out of `MatchAttributes`, which nothing catches on that path.
- **`PetConfig.FeederSlotA`/`FeederSlotB`** — see the first item; they are unreachable, not merely
  unconfigurable.
- **The projection guard only protects what its fixture remembers.** `EveryLocalPetFieldIsCopiedFromTheConfig`
  walks `LocalPet`'s properties and compares each against the config — so a field the **fixture omits**
  is null on *both* sides and passes whether or not the projection carries it. Verified 2026-09-23 by
  removing a copy and watching it fail *with* the fixture set, then removing the fixture value and
  watching it pass silently. So the guard's reach is exactly the set of fields someone remembered —
  the same by-hand weakness that let this projection drop a field three times. Closing it means the
  fixture covering every field, or the test noticing a property it was never given a value for.

**Reported by the audit, not reproduced (verify before acting):**

- `OcrEngine` filters attribute rows with `a.Y >= 10` where `Y` is the *bucket start* (`rk * rowHeight`),
  not the measured Y — so a large calibrated `row_height` can bucket every row to 0 and drop all of
  them. Suspected line; needs a config with a large `row_height` to trigger.
- `ConfigValidator` dereferences `c.Tuner` unguarded: a bare `tuner:` line (a section key with no
  value) gives an NRE instead of the descriptive `ConfigException` the file's other guards exist to
  produce.
- `PetTool`'s "try the return slot first" path puts the fall-through-to-scan branch **inside**
  `if (_cfg.Tooltip.IsSet)`, so with the tooltip uncalibrated the scan is unreachable and a row with
  no `PetSlotEmptyPng` can report "boarded" after clicking nothing.
- `LauncherService.StopTool` writes releases on the same `SerialPort` a tool thread may be mid-drag on,
  so a Stop during a food drag can release the button mid-move and drop the stack.
- The documented "the read refuses when the game is not in front" holds for the feeder counts and the
  bag scan but **not** for the slot-occupancy checks — with anything covering the game, a slot reads
  "occupied" and the row is scheduled for hours while nothing is boarding.
- `SealTuner` does not reset attempt/repeat state on a Stop→Start (the spammer has a `Reset()`), and
  `GradeIndex()` returns `-1` for a `target_grade` typo, which `gradeOk = idx >= -1` accepts.
- `GemComposer` gates auto-advance on `EmptySignature != null` even though the saved crop is the
  primary test; and `Gem.Grades` has no length validation, so an empty list throws before the loop's
  `try`.
- **No tests cover any of `SealTools.Pet`** — the largest and most recently changed file.

## Settled (measured — do not reopen)

- **Capture method.** `PrintWindow` returns a **black frame** for this game; `CopyFromScreen` returns
  the real screen. Everything now captures with `CopyFromScreen` (client-area origin). Evidence:
  `logs/captures/diag_printwindow.png` vs `diag_copyfromscreen.png` (2026-09-09). The calibrator's
  **Diagnose capture** button reports the frame/client rects and saves a sample capture if you need to
  re-check.

## What was verified, and when

Not tracked here. A "verified working on the live game (2026-09-09)" snapshot used to sit in this file
and had become actively misleading — it still named the logical `SetCursorPos` path as the one in use,
which v2.2 replaced with Arduino placement. Verification history belongs in
[PROGRESS.md](PROGRESS.md), which is dated and keeps the measurements with the reasoning.

The live checks still outstanding are in **Planned** above (a full tuner run and a composer cycle in
the current build, after the residency rewrite of the shared Start/Stop path).

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

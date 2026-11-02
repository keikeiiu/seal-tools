# v2→v3 parity audit (2026-10-28)

**What this is.** A read-only, surface-by-surface comparison of the shipped v2 launcher (C#/WPF) with
the v3 shell/core, commissioned after the player found v3's Sell grid had lost several v2 controls
("for the sell grid, there are still features you missed from the original v2" / "please do the
checking for all the features"). Sources: v2 `USER_GUIDE`/`TOOL-FLOWS`/`CONFIG`/`CALIBRATION` plus the
v2 UI source (`v2/SealTools.Launcher/MainWindow.xaml.cs` and the tool projects), against v3's
`shell/lib`, `core/crates`, and `v3/docs/spec`. The four v2 Calibrate surfaces and Setup Detect were
compared against `PLAN-V3-CALIBRATION.md`, not as gaps.

**Headline.** The tools themselves are ported; what is missing is concentrated in (a) **run-state and
maintenance safety** the v2 launcher performed around the tools, and (b) **card/tab controls and live
lines** the player uses to see and steer them. The Sell-grid misses were the first instance of a
pattern, not a one-off.

## Functional / safety gaps (fix first)

- **A1. The Pet Feeder no longer persists its run state.** v2 wrote `food_slots_used`, per-row
  `boarding_running` and the close-guard belief back to `local.yaml` as they changed, so a days-long
  run could not come back aiming at used cells or toggling the wrong boarding state. v3 reads those
  fields but has no writer (only `config.apply`/`calibration.save` write), and the Pet tab has no
  boarding tick. After a restart the feeder can feed already-used cells again. Needs a protocol ruling
  for core-initiated run-state writes (PROTOCOL §7 anticipates the emitter) and a Pet-tab surface.
- **A2. Buy and Sell dry runs are gone.** v2's dry runs clicked nothing and were the only safe way to
  verify row/scroll/bag targeting before spending gold or selling. v3 has no command or control.
- **A3. The Pet Feeder can no longer be set up or maintained from the UI.** v2's Pet tab could mark
  food cells/return slot, capture a food icon, scan the bag for food and for feedable pets, add queue
  pets by capture, run a panel test read, and showed "Setup so far"/"Ready to run". v3's tab edits the
  stored data but cannot capture, scan or add — the player cannot queue a new pet or refresh food
  cells at all.
- **A4. Quest "Test one loop" is gone.** v2's only pre-run verification of a blind flow; v3 has no
  equivalent, so a wrong step/guard is discovered by a real run.
- **A5. Deletion guards lost.** v2 refused to delete the last spammer preset and the last quest flow;
  v3's Delete buttons can leave a config no run can use. Also v2's sequence "only flow" guard.
- **A6. Sell over-cap refusal** — present only as the in-flight task-83 fix; finish it (v3 compared the
  raw list length, now the deduped valid selection).

## UI-control gaps likely noticed next

- **B1. Spammer card rotation picker** — v2 built the preset rotation on the card; v3's cards only
  have Buy/Quest extras.
- **B3. Arduino connection diagnostics** — v2's Connection card (status light, verdict + remedy, full
  port list with the chosen port marked, VID/PID + PnP id, firmware level/detail) is reduced to a
  probe summary; this is a regression of a surface a past release deliberately fixed.
- **B4. Quest flow/sequence Rename and Duplicate** — v2.16 feature; v3 has Add/Delete only (and v2's
  rename re-pointed sequence entries and the card target).
- **B5. Tuner card live detail** — v2's card printed grade, remaining, attempt, cycle, current,
  attributes and filter verdict; v3's `ToolStatus` carries only message/countdown/rows, so the card is
  much quieter. Needs a protocol status extension.
- **B6. Tuner "Clean up capture images"**; **B7. Spammer raw key editor ("Advanced")**;
  **B9. Buy row labelled picker** — v2 replaced the free numeric row after an off-by-one bought the
  wrong item; v3 reintroduces the free field.
- **B2/B8.** Pet-tab session surface and its "Setup so far" checklist (part of A3); resident marker
  and persistent-crash line are minor diagnostics.
- **Window placement/pin persistence** and **mini mode** remain deliberate gaps (mini is named in
  OPEN); placement is not named — record it.

## Calibration plan gap-check (additions to the plan)

- Calibrate Pet's **"Setup so far" checklist** is in v2 but missing from the plan's §3.4 inventory.
- **Reopening a calibrate tab must redraw the saved overlay**: v2 loaded `calib_tuner.png`,
  `calib_gem.png`, `calib_pet.png` at startup and redrew the marks; the plan covers writing artifacts
  but not load/render-on-reopen acceptance.
- **Gem move mode** has two candidate homes (the plan's Calibrate Gem vs v3's existing Gem-tab "Move
  mode"); decide one home.

## Deliberate differences (not gaps)

Five v2 Calibrate tabs → one Calibrate tab + the planned sessions; no global hotkey poller; one tool
at a time + resident pet; dry/calibration pointer tests take the input lease; v2's pure diagnostics
(Debug Cursor, Diagnose capture, Sample result gem detail) dropped with reasons in the calibration
plan; Saves consolidated into one per tab.

## Triage (batches the Lead will run)

1. **v3.0.3 — quick parity + the in-flight work**: task-83 over-cap refusal, calibration increment 0
   (pointer/artifact foundation), always-on-top setting, A5 deletion guards, B1 spammer card picker,
   B6 cleanup captures, B9 buy row picker.
2. **v3.0.4 — pet safety/maintenance (A1+A3) as a small plan**: core-initiated run-state writes +
   a Pet-tab session surface (food/queue/feedable scans, capture-add, test read, checklists), built on
   the calibration pointer primitives.
3. **v3.1 — the calibration increments** (Setup Detect → Tuner → Gem → Buy/Sell → Tooltip → Pet →
   parity pass) with A2 dry runs / A4 Quest test loop folded into the pointer/command batch they need;
   B5 tuner card status and B3 Arduino diagnostics ride whichever release they fit.

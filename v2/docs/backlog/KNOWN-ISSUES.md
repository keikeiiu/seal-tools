# Known issues — found, not scheduled

Defects that are **real and unfixed**, and that nobody has decided to work on. They are recorded here so
they are neither lost nor re-discovered as news — and they are deliberately NOT in [TODO.md](TODO.md),
which is the work we intend to execute. An open list that keeps things nobody intends to do stops being
a plan.

Each was re-checked against the code on **2026-10-16** and is still present. When one is fixed, remove
it: what it was belongs in [../records/PROGRESS.md](../records/PROGRESS.md) with the fix, and the entry
here has done its job. When one gets scheduled, move it to TODO.md.

- **`OcrEngine` filters attribute rows by the bucket start rather than the measured Y.**
  `attrLines` stores `(rk * rowHeight, …)` and the filter is `a.Y >= 10`, so a large calibrated
  `row_height` can bucket every row to 0 and drop all of them. Suspected rather than reproduced: it
  needs a config with a large `row_height` to trigger. `OcrEngine.cs:305`, `:325`, `:328`.

- **`PetTool`'s "try the return slot first" path hides the fall-through to the scan.**
  The branch that scans for a queued pet when the return slot holds nothing sits **inside**
  `if (_cfg.Tooltip.IsSet)`, so with the tooltip uncalibrated the scan is unreachable and a row with no
  `PetSlotEmptyPng` can report "boarded" after clicking nothing. `PetTool.cs:2521`, the fall-through at
  `:2542-2547`.

- **`SealTuner` carries attempt/repeat state across a Stop→Start, and takes an unresolvable grade on
  trust.** `attempt` and `consecutiveRepeats` are only reset inside a run, so stopping and starting
  continues the previous run's counts — the spammer has a `Reset()` for exactly this. And
  `target_grade` is compared by POSITION (`GradeIndex(grade) >= GradeIndex(effectiveGrade)`), so a grade
  the tuner cannot resolve is silently accepted rather than refused. `SealTuner.cs:48`, `:53`, `:63-86`.

- **`GemComposer` gates auto-advance on the signature even though the saved crop is the primary test.**
  `EmptyMode != "stop" && EmptySignature != null` means a machine that saved the empty-result crop but
  never the colour signature never auto-advances. And `Gem.Grades` has no length validation, so an empty
  list throws before the loop's `try`. `GemComposer.cs:494`, `:255-270`.

- **No test constructs `PetTool`** — the largest and most recently changed file. The test project does
  reference `SealTools.Pet`, but its one use is the `PetTool.MatchLimit` constant in `IconMatchTests`;
  nothing exercises the tool's own logic, so every branch in it is reasoned rather than verified.

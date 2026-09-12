# Progress log

A dated, append-only record of what was actually done and why — the reasoning that is not visible in
the code or the commit titles. One entry per session or per landed victory, newest first.

**How to use it:** append an entry when a change lands. Keep it short: what was the goal, what was
decided and why, what was measured, what is still open. Link to the commit and to the doc that owns
the detail (`CURSOR-INVESTIGATION.md`, `MOVE-SETS.md`, …). Do not restate what the code already says.

---

## 2026-09-13 (3) — review sweep: 19 fixes, 4 recorded as deliberate

A full review of v2 in three passes (Core, launcher, tools), then the fixes. Every candidate was
verified by reading the code before anything was written, and that rule earned its place immediately:
one item handed over as a bug — the composer's empty-check gate — turned out to be a load-bearing
arming flag, and "fixing" it would have re-enabled auto-advance for a user who had explicitly
declined it. It was retracted rather than changed.

What mattered most:

- **`gem.move_mode` was deleted by every launcher save.** `SaveDefaults` serialises an explicit field
  list and the gem projection omitted it, so choosing `tuned` could not be persisted and the next
  launch fell back to `arduino`. The round-trip test that exists to catch exactly this compared the
  property default to itself, so it passed either way. It now sets a non-default value and is
  mutation-tested. Same shape as the `ocr_retries` regression that test was written for.
- **Two Start clicks could orphan a tool loop** on the shared serial port: unreachable by Stop, still
  writing, still driving the game. Guarded — plus the matching hole where a Stop during a cold start
  silently did nothing and the tool started anyway.
- **The tuner could stop below the target grade** (a stop branch missing `filter.Enabled`) and
  **treat a failed OCR read as a repeat**, ending a run after two bad reads behind a message claiming
  "x3". A read that recognises nothing now stops immediately and says so.
- **`CaptureScreen` threw instead of returning the null its callers were already written against**, so
  a locked session or a zero-sized region took down the tool rather than being handled.

Four items were deliberately **not** changed, and are recorded in [TODO.md](TODO.md) with their
reasoning because three would be reverted by a well-meaning cleanup: the null attribute value that
intentionally satisfies a bounded filter rule (`減少傷害` is rare enough that the name match is the
signal), `Arduino.Find`'s duplicated WMI query — which carries a name-based fallback `Diagnose` does
not have, the tuner's fail-open mouse guard, and the dispatcher sleeps in the calibrator's test
buttons, where the real fix is a five-handler refactor rather than `await Task.Delay`.

Also landed: spammer presets moved to `local.yaml` so they stop shipping, with adoption for any
already stranded in `defaults.yaml`; the test project can finally reach `AttrMatcher` and the other
tool logic (23 → 30 tests); and a CI workflow.

**Needs a live run** — in-loop behaviour the test project cannot reach: the tuner's failed-read stop,
the `filter.Enabled` guard, and the spammer's disconnect message. **Needs a flashed board**: the
firmware's host-gone key release (hold space, kill the launcher, confirm release). **Not yet run**:
the CI workflow's build step, which needs a push — the running launcher held a file lock locally.

## 2026-09-13 (2) — presets left in defaults.yaml are adopted into local.yaml, not deleted

Follow-up to the entry below, which fixed the leak but opened a data-loss path. On a machine that
only ever ran the older build the presets lived in `defaults.yaml` alone. The first Save on *any*
tab rewrites that file without a spammer block (tuner, gem and hotkeys all call `SaveDefaults`), so
that Save deleted the player's only copy, and the next launch found no presets and quietly created a
blank `default`. The entry below called these presets "stranded" — they were deleted on first save.

`Load()` now adopts them before anything can rewrite the file. `AdoptSpammerPresetsIntoLocal` fires
when `local.Spammer` is null and `defaults.yaml` still carries presets, writes them through
`SaveLocal`, and never fires again because `local.Spammer` is set afterwards. A first run seeded from
`local.yaml.example` carries no spammer block, so it does not fire there either.

**`MigrateSpammerPresets` had to move above `ApplyOverrides`, and that is a fix in its own right.**
It converts a pre-presets flat `spammer.keys` list into a preset named `default`. Running after the
merge, it saw `Presets.Count != 0` whenever `local.yaml` supplied any preset at all, skipped the
conversion, and the legacy keys were dropped without a word. Before the merge they become a preset,
are adopted, and survive in `local.yaml`. Both `Presets` reads now use `is { Count: > 0 }`: an
explicitly empty `presets:` key deserialises to null and `.Count` threw.

Tests: `LoadAdoptsSpammerPresetsLeftInDefaultsIntoLocalYaml` checks the presets reach `local.yaml`
and survive a `SaveDefaults` + reload — the assertion that actually catches the loss —
and `LegacyFlatKeysReachLocalYamlThroughAdoption` covers the ordering. Both were verified to fail
against a deliberately inverted adoption guard, so neither passes vacuously.

**Left open.** `defaults.yaml` keeps its stale block until the next Save, so on an affected machine
a preset deleted in the UI would resurrect once from it before the block goes. Self-healing after
one Save; stripping it during `Load()` would mean rewriting `defaults.yaml` (reformatting, losing
its comments) on every affected machine. Also unguarded: `publish.bat` public mode does not check
that `defaults.yaml` is free of a `spammer:` block before shipping it.

## 2026-09-13 — spammer presets move to local.yaml; Hold Space releases the key on stop

**Spammer presets were being published.** The "Save Spammer Config" button wrote every preset and
`active` into `defaults.yaml` via `SaveDefaults` — the one file `publish.bat` copies into the public
zip. So a player's personal rotations shipped with every release; the `Knight0-9` rename sitting in
the working tree was the symptom, not the cause. Spammer was the last personal setting with no
`local.yaml` home: calibration, tuner geometry, spring point, gem moves and UI placement all had one,
but `LocalOverrides` had no `Spammer` field at all.

Fix: `LocalOverrides` gains `LocalSpammer { Active, Presets }`, merged per preset *name* in
`ApplyOverrides`. The spammer tab now saves through `SaveLocal` (same shape as the tuner tab) and
says so in its InfoBar; the button is "Save Preset", not "Save Spammer Config".

**The seed had to go, and that decided the design.** `SaveDefaults` rewrites `defaults.yaml` from an
explicit field list, so removing spammer from that list would silently delete the block the first
time any *other* tab saved (tuner, gem and hotkeys all call it) — a seed that vanishes on first use
is worse than none. So `defaults.yaml` now carries no spammer block at all and `local.yaml` is the
single source of truth; `BuildSpammerTab` already creates an empty `default` preset when none exist,
so a fresh install still opens on a usable editor.

Guarded by three tests: presets load from `local.yaml`, `SaveDefaults` leaves no preset names in the
written `defaults.yaml`, and `SaveLocal` round-trips them. The first version of the no-leak test
asserted on a *reloaded* config and failed — reloading merges `local.yaml` back on top, so `Active`
is correctly the personal one again. It now reads the file on disk, which is where the leak would be.

**Hold Space could leave the spacebar down** (`d28a63a`). Stop clicked mid-loop skipped the release,
so the key stayed held. `LauncherService.ReleaseSpace()` writes `U` directly, independent of the
tool's loop, and the stop path calls it first. The top-right card also gained an idle/holding dot.

**Left open:** the log has a two-day gap before this entry (`1b85c59`, the Hold Space commits, and
the composer move set all landed unlogged). The preset migration this entry flagged turned out to be
a data-loss bug rather than a tidiness one — see the entry above.

## 2026-09-11 — two live-testing bugs fixed: grade parsed as G, and the matcher dropping lines

Both found while the user ran the tuner against a real DG item.

**Grade parsed as "G" when the item was "DG".** The OCR log proved the text was `GRADE：DG` and
the colour score was `DG=287` (yellow) — so OCR was right and the *parser* was wrong. `DetectGradeFromLine`
stripped the label, then a "rightmost match wins" scan let the bare `"G"` at index 1 override the longer
`"DG"` at index 0. It stayed latent for N/G items (no nested grade letter). Now longest-first.

**"One line less" — a matcher drop, not a read failure.** The user saw ~2 of 3 attribute lines.
The captures (`logs/captures/*.png`) are decisive: every DG frame shows all 3 lines, so the OCR reads
3 and `AttrMatcher` throws the third away when a misread character breaks the dictionary match. Four
recurring confusions, all in `text_fixes.substring`:
`每`→`国/盘/地`, `等級`→`等` (drops `級`), `幸運`→`幸莲`, `必殺技`→`必毅技` (`毅`≠`殺`).

This **closes the open "OCR row-bucket pooling" question** ([TODO.md](TODO.md), [IDEAS.md](IDEAS.md)):
the evidence shows no `row_height` pooling — the lines are all read; the drop was in the matcher. The
fix is `b42247f` (grade) and `3b8023b` (attributes.yaml cleanup rules). Note the launcher caches
`attributes.yaml` at startup, so the cleanup rules need a restart to take effect.

A **third drop shape** followed: a dropped *trailing* stat character (`幸運` → `幸`). Because the
truncated form is a prefix of the correct one, a plain substring replace would corrupt correct reads,
so `71f9672` adds a `regex` fix type (negative lookahead) for all six per-level stats.

A **fourth** (`必殺技` → `必技`, middle char dropped) was the only remaining shape after a 3,189-attempt
capture run (2 drops, ~0.06%) — fixed in `54ad743`. Merged to `main` and tagged `v2.4` (2026-09-12).

---

## 2026-09-11 — the tuner places the cursor on 發條 and guards it (branch `v2-tuner-spring`)

**Goal.** Give the Magic Tuner the composer's closed-loop cursor: put the mouse on the 發條 button
automatically, opt-in, and stop (or re-centre) when the mouse moves away mid-run — the
human-takes-over safety. Full design: [PLAN-TUNER-SPRING.md](PLAN-TUNER-SPRING.md).

**What was decided, and why**

- **Opt-in, mirroring `gem.move_mode`.** `tuner.spring_mode: manual | hid` (default `manual` — today's
  behaviour unchanged); `tuner.spring_point: [x,y]` in `local.yaml`, calibrated by a 4th Calibrate Tuner
  step (click the 發條 button) with a Test Click to verify. In `hid` mode the cursor is placed at the
  end of the 5-second countdown; a missing point / window / failed placement stops the run and reports
  on the card — never click blind.
- **The guard is a three-way toggle** `tuner.mouse_guard: off | stop | recenter` (default `off`), because
  the user wants all three: "refocus" (recenter), "take over" (stop), "leave home and let it run" (off).
  `stop` halts on the first drift past `guard_px`; `recenter` re-places the cursor and keeps rolling
  toward the target grade (which outranks a stray mouse), stopping only after `recenter_max` runaway
  drifts. Checked once per attempt, before the click.
- **`GemPointer` → `HidPointer`** (mechanical), so the tuner reusing it reads honestly.

**What's left open (the data questions, still unanswered)**

- **Does the game warp the cursor during a run?** Until measured, `stop` can false-trigger if the game
  re-centres the pointer on its own — that is why the guard defaults to `off`, and why the plan's extra
  mitigations (two-consecutive-polls, foreground-only judging) are the first thing to add if a run shows
  one. **Verified live (2026-09-11):** `hid` + `off` (auto-place, no guard) and `hid` + `stop` (halt on a
  drifted mouse) both behaved as intended, with no false-trigger seen in `stop` — weak evidence the game
  does not warp the cursor, not yet a rigorous per-attempt measurement.
- The guard's "once per attempt" cadence is a first cut; a live run should show whether the poll needs
  to be more frequent (e.g. on the `SleepCheck` loop).

**Commits** (branch `v2-tuner-spring`, not merged): `63ff486` rename, `d5ff0f3` config, `61bf8b9`
4th calibrate step + Test Click, `f0de93d` placement, `8b9e2e2` guard + UI.

---

## 2026-09-11 — the v2.3 zip is built; publish.bat gains a public/personal split

**Goal.** Build the v2.3 distributable and close the packaging gap: `publish.bat` produced a
`publish\` folder but never the zip the README points at, and it copied `config\` wholesale — shipping
the machine's real `local.yaml` and 14 MB of `calib_*.png` screenshots into a public zip.

**What was decided, and why**

- **Two publish modes.** `publish.bat` (public, the default) ships only the config templates
  (`attributes.yaml`, `defaults.yaml`, `local.yaml.example`); `publish.bat local` ships the full
  `config\` for a same-machine reinstall that is already calibrated. The safe one is the default, so
  an unthinking `publish.bat` cannot leak a calibration.
- **The publish folder is deleted before `dotnet publish`** — not just the mode's config. A stale
  folder carries whatever a previous mode left, so the clean delete is what guarantees a public build
  can never ship a `local.yaml` a local build left behind.

**Three traps found while writing the script**

- **An incremental `dotnet publish` silently drops the native OCR DLLs.** A second publish without a
  source change produced only `SealTools.Launcher.exe` + models + config — no `onnxruntime.dll`,
  `OpenCvSharpExtern.dll`, `libSkiaSharp.dll`, … (11 DLLs, ~170 MB). The exe is 182 MB either way, so
  nothing looked wrong until the zip was listed against the known-good v2.1 release, which ships all 11
  alongside the exe. The fix is the clean delete above, which forces a full publish every time. This is
  the measured, don't-assume lesson: the "obvious" build worked, the second one didn't, and only
  diffing the zip's file list against v2.1 caught it.
- **NuGet content ships an 89 MB `libSkiaSharp.pdb` and `*.lib` import libraries** that
  `-p:DebugType=None` does not touch (it only stops *our* symbols). The script strips `*.pdb` / `*.lib`
  after publish; v2.1's zip never had them.
- **`set VERSION=v2.3` leaked into `dotnet publish`.** MSBuild reads env vars as properties
  (case-insensitive), so `VERSION` overrode the `Version` property and `'v2.3'` failed semver. The
  variable is now `RELTAG`.
- **LF-only line endings broke cmd's batch parser** (`goto`/`if` blocks → spurious
  "not recognized" errors). The file is CRLF, and the CRLF fix must be the *last* edit — GNU
  `sed -i` re-strips `\r`.

**Measured.** `dist\SealTools-v2.3.zip` (public) = 144 MB — 11 native DLLs + exe + 3 models + 3
templates, no `.pdb`/`.lib`, no `local.yaml`. `SealTools-v2.3-local.zip` = 159 MB — the same plus the
full `config\` (`local.yaml` + three `calib_*.png`). The public zip built *after* the local one still
holds only the templates.

**Left open.** The zip is built but **not uploaded**: `gh` is unauthenticated and the GitHub release
`v2.3` does not exist yet. Nor does the `v2.2` release the README advertised — its features shipped
inside v2.3, and that row is now folded into v2.3's. The old v2.0/v2.1 folders and zips in `dist\`
are still there, to delete once the release lands.

---

## 2026-09-11 — the launcher adopts WPF-UI; the window learns where it belongs

**Goal.** Make the launcher readable and keep the tool status visible while playing. The project had
WPF-UI loaded but only ever used `FluentWindow`, `ui:TitleBar` and `ui:Button` — every tab was plain
WPF with a second, hard-coded palette beside the theme.

**What was decided, and why**

- **Adopt WPF-UI properly, one tab per commit.** Card-based sections, `ui:` controls, and the
  theme's semantic brushes instead of our hex ones. The information architecture carried over
  untouched: grouping, renames, the `Hotkeys` rename. Full checklist in
  [PLAN-UI-CLEANUP.md](PLAN-UI-CLEANUP.md).
- **The shell change was tried and reverted.** A `ui:NavigationView` rail rendered correctly but
  clicking an item never switched the page, and the tab strip read better anyway — so the tabs
  stayed and the experiment is recorded so nobody retries it blind.
- **The window now belongs to the user.** It opens as just the tool cards, the configuration tabs
  hide behind a chevron, it can be pinned above the game, and while a tool runs it shrinks to that
  tool's card. Placement, size and expanded height are remembered in `local.yaml` and written
  ~0.7 s after a move or resize settles — not only on a clean close, which is what used to lose a
  resize when the process was killed.

**Two real bugs found in review (both fixed, both were live-facing)**

- **The move-set selector disabled itself.** It sat inside the arduino card, which is disabled
  whenever the other set is active — so choosing `tuned` disabled the only control that could switch
  back. It has its own card now.
- **The empty check was comparing the box's border.** The frame shifts by a pixel when the game
  window moves, which made an *empty* box score 7.7 % against a 0.01 gate and stalled the composer.
  Comparing the interior only: empty 0.0 %, gem 0.68–0.82 on a live run.

**Verified live.** Pin and placement survive a relaunch (moved to logical `(1927,3)`, reopened there
at `619×430`, pinned). Mini mode: `920×430` idle → `920×320` running → back on stop. A full composer
run advanced correctly with the inset fix.

**Left open.** The v2.3 zip is not built or published. The robustness list, the tuner spring plan and
the spammer key pad are all designed and waiting in [IDEAS.md](IDEAS.md).

---

## 2026-09-10 (6) — the empty check was comparing the box's border, and a moved window broke it

**Symptom (reported live).** The composer kept combining and never advanced, with the result box
visibly empty. The card said "stopped" only because the run had been stopped by hand.

**First check: not the guard.** The foreground guard from entry (5) was the obvious suspect, but the
log showed it working — `diff=0.077 empty=False` for six cycles, and exactly one
`refused: not foreground` line, at the moment focus moved to VS Code. So the check was running and
judging; it was judging wrongly.

**Diagnosis.** `save_empty_captures` was turned back on, the run repeated, and the saved crop showed
an *empty* box. Comparing that crop against the reference per-row showed the differing pixels were
not in the middle but in horizontal bands at the very top and bottom — y=0,1,4 and y=55–58: the box's
drawn frame. A one-pixel shift in where the crop lands moves those rows while the flat interior stays
identical. That alone was 7.7 % of the box — six times the 0.01 gate.

**Fix.** The comparison now skips a 6 px border on each edge (`EmptyCompareInset`, passed through to
`GemColorAnalyzer.DiffFraction`). Measured on the real crops:

| inset | empty | gem |
|---|---|---|
| 0 (before) | 7.7 % | 35.6 % |
| 6 (now) | **0.0 %** | **54.6 %** |

The gem is drawn in the interior, so the separation gets *better*, not worse. A new test pins the
inset, including the fallback when the inset would swallow the whole image.

**Verified live afterwards**, same run, from `empty_check.txt`:

```
23:38:47  diff=0.818  empty=False     ← gem in the box (0.68–0.82 across the run)
23:39:13  diff=0.000  empty=True      ← emptied, so the composer cleared and advanced
```

The gem signal on a real run is even wider than the offline measurement (0.82 vs 0.55), and the empty
state is exactly 0.000 — the two states are now further apart than at any point before, with the gate
untouched at 0.01.

**Lesson worth keeping:** a 0.01 gate is only safe when the empty state really is pixel-identical. It
was — until the window moved. The inset is what makes the tight gate honest.

---

## 2026-09-10 (5) — the empty check refuses to judge a screen grab that isn't the game

**Why.** The check crops the result box from a screen grab (`CopyFromScreen`), so it measures
whatever is *in front*. During the empty-detection investigation a check ran with the launcher in
front and returned `RGB(26,26,46)` — the launcher's own dark UI — which produced a verdict about a
window that had nothing to do with the game. Nothing warned about it; the log line just looked odd.

**Fix.** `IsResultBoxEmpty` now requires the game window to be the foreground window before it
judges. When it isn't, it answers **"not empty"** — the safe direction (the composer keeps combining
instead of advancing a grade on a bad read) — and writes `refused: not foreground (fg="…")` to
`empty_check.txt` so the reason is visible rather than silent. The refusal shares the same log path
as a normal check, via a small `LogEmptyCheck` helper.

This is item 1 of the "Small robustness wins" list in [IDEAS.md](IDEAS.md), which is now ticked off.
Not yet verified live: the refusal only fires when something steals focus from the game mid-run, so
the next composer run with a stray click on the launcher is the observation to look for.

---

## 2026-09-10 (4) — a run ends after the last grade

**Symptom.** The first live `arduino`-mode run worked — combines until empty, N → G → DG, empty check
correct throughout — but after DG's material ran out it went back to N and started over.

**Why.** `AdvanceGrade` advanced with `gidx = (gidx + 1) % grades.Count`, an intentional endless
loop from v1. A run is meant to be N → G → DG once.

**Fix.** `AdvanceGrade` returns false when there is no next grade; the composer reports "All grades
done (last was DG) — composer stopped." on the card and breaks out of the loop. The manual F9
advance still wraps. Commit `6a28ecf`.

**Evidence from the run** (`<bin>\logs\empty_check.txt`) — the new empty check behaved exactly as
designed: gem frames `diff=0.44–0.54`, empty frames `diff=0.000`, gate `0.01`.

---

## 2026-09-10 (3) — empty-result detection rebuilt on a pixel difference

**Goal.** The composer advanced the grade while the result box plainly held a gem, so `empty_mode:
advance_grade_clear` ran away. Chased it to the empty check, not the moves.

**What was measured** (62×59 crop, the real empty reference vs a real gem frame)

- The old metric — mean of six absolute colour differences — scored the pair **0.100**, under the
  0.18 threshold: a box full of gem read as "empty". The one strong signal, the coloured fraction
  (0.556 → 0.930), was being divided by six.
- Euclidean norm over the same six: **0.294** (empty vs gem) and **0.001** (empty vs itself).
- Per-pixel difference vs the saved empty crop: **0.000** for an empty box at every channel
  threshold 10–60, **0.357** with a gem. The empty slot is static UI and renders pixel-identical.
- A metric sweep showed *every* pure-colour feature is the wrong family: `dominant hue` is identical
  (60°) for both, and mean R/G/B invert depending on gem colour. Structure metrics (edges 6×,
  distinct colours 8×, laplacian variance 10×) all separate and are colour/shape-blind.

**What was decided, and why**

- **Primary test = fraction of pixels differing from the saved empty crop** (>30 on any channel),
  threshold `gem.empty_distance` lowered 0.18 → **0.01**: ~35× below the gem signal, ~10× above the
  floor. Colour- and shape-blind, so any gem colour or grade shape reads the same.
- Euclidean colour signature kept only as a fallback for when the crop is missing.
- **The empty reference is now taken from the launcher-hidden screenshot**, not a live screen grab:
  the launcher covering the box at save time is what had poisoned the stored signature (0.17 away
  from its own reference crop).
- Diagnosis trap worth remembering: the sampler reads *screen* pixels, so a covering window makes
  every reading garbage — a check run with the launcher in front returned `RGB(26,26,46)`.

**Commit** — `f4b5f02` (branch `v2-arduino-moves`). 5 new tests, 17/17 pass.

**Left open** — the composer has not yet run a full session in `arduino` mode with the new empty
check; the next live run should show `diff=0.000` on empty boxes and `diff≈0.36` on gems in
`<bin>\logs\empty_check.txt`.

---

## 2026-09-10 (2) — Test Full Cycle (Arduino)

**Goal.** Let the new move set be judged on a real run before the composer is switched to it: one
button in Calibrate Gem that plays a whole cycle with arduino moves only.

**What was decided, and why**

- The sequence mirrors the **composer's own loop body**, not a guess: select → Register → Combine,
  then the composer's normal-path *deregister + register* (two Register clicks), then Combine again.
  Without that pair a second combine does nothing. The user asked for the combine loop twice at N and
  G; DG combines once and the cycle stops (no trailing resource clear — the user's choice).
- It always uses the **arduino** set regardless of `gem.move_mode`; that is the point of the button.
- All points are resolved *before* the first click, so a missing calibration can't half-run a cycle.

**Measured (live)** — `test-cycle-arduino done steps=21`, game focused the whole way, gold down ~2.16M
(the combines really ran), slots and result box empty afterwards.

**Commit** — `ac0f8ec` (branch `v2-arduino-moves`).

**Follow-up, same session:** the composer was switched over — `gem.move_mode: arduino` in
`defaults.yaml` (this commit). The composer now runs the arduino set on every route; the tuned counts
stay in `local.yaml` untouched, so switching back is one line.

**Left open** — same as the entry below: no full composer run with `gem.move_mode: arduino` yet, and
the reason `SetCursorPos` is refused in our process is still unknown.

---

## 2026-09-10 — cursor placement rebuilt on the Arduino; a second move set

**Goal.** Finish the `SetCursorPos` bug: test the last untested hypothesis (the game's anti-cheat
reacting to the process holding the Arduino port), then stop depending on the refused API at all.

**What was decided, and why**

- **The port hypothesis is refuted.** A throwaway process opened COM5, drove it, and called
  `SetCursorPos` 30 times while holding it: 30/30 accepted. A second process managed 29/30 while the
  port stayed held. So the refusal is not about the Arduino at all — it stays specific to the
  launcher's process, and stays unexplained. Details and every earlier probe:
  [CURSOR-INVESTIGATION.md](CURSOR-INVESTIGATION.md).
- **The cursor is now positioned with the Arduino** — a closed loop against `GetCursorPos`, which
  always worked in our process. The HID path cannot be refused, and a placement that fails now stops
  the tool instead of clicking somewhere arbitrary.
- **The move set was added, not replaced.** The user asked for the inter-point movement to use the
  same mechanism as Test Click, explicitly *without* discarding the hand-tuned `gem.movements`
  counts. So both sets are live, selected by `gem.move_mode` (default `tuned` — nothing changes for
  an untouched install). [MOVE-SETS.md](MOVE-SETS.md) explains both and how to test each.

**Measured (live, on the reference PC)**

- `D n 0` moves the cursor exactly `n` px in the process's cursor space at 100 / 250 / 500 / 600 px —
  gain 1.0, no acceleration. A placement therefore converges in one move; from a far start, three.
- The firmware walks a `D` move out in 10-px chunks with a 1 ms gap, so a fixed 20 ms settle read a
  stale position and stacked corrections. Polling until the cursor stops moving fixed it.
- Test Click lands exactly on the N radio and the radio selects; the `Register → Combine` arduino
  route places on (829,725) → (785,871) against calibrated targets (830,726) / (786,872).

**Commits** (branch `v2-arduino-moves`, not merged)

| Commit | What |
|---|---|
| `624fddd` | `feat(v2)`: the arduino move set — `GemRoutes`, `gem.move_mode`, `MOVE-SETS.md` |
| `2e0b043` | `fix(v2)`: closed-loop Arduino placement, failure stops the tool, mode wired to the UI |

**Left open**

- Why `SetCursorPos` is refused in the launcher's process — intermittently, while another process
  under the same user/session/integrity succeeds. Not load-bearing any more; the shortest next step
  if it ever matters is a minimal WPF app that only calls `SetCursorPos`.
- The `arduino` move set has been tested per route through the calibrator buttons, not yet through a
  full composer run with `gem.move_mode: arduino`.
- The composer's runtime path (both modes) is the same call the tests make, but a live composer run
  was not started during this session — it clicks in the game.

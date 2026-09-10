# Progress log

A dated, append-only record of what was actually done and why — the reasoning that is not visible in
the code or the commit titles. One entry per session or per landed victory, newest first.

**How to use it:** append an entry when a change lands. Keep it short: what was the goal, what was
decided and why, what was measured, what is still open. Link to the commit and to the doc that owns
the detail (`CURSOR-INVESTIGATION.md`, `MOVE-SETS.md`, …). Do not restate what the code already says.

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

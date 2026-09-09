# Progress log

A dated, append-only record of what was actually done and why — the reasoning that is not visible in
the code or the commit titles. One entry per session or per landed victory, newest first.

**How to use it:** append an entry when a change lands. Keep it short: what was the goal, what was
decided and why, what was measured, what is still open. Link to the commit and to the doc that owns
the detail (`CURSOR-INVESTIGATION.md`, `MOVE-SETS.md`, …). Do not restate what the code already says.

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

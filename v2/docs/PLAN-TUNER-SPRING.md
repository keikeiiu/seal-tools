# Plan — Tuner spring positioning + cursor guard (NOT implemented)

Planning note, 2026-09-10. Nothing here is built. It exists so the idea survives the session; the
work items are mirrored in [TODO.md](TODO.md).

## Why this is possible now

The tuner has **no positioning at all** today: `SealTuner.Run` sends `C` (click) + `E` (Enter) per
attempt and assumes you already put the mouse on the 發條 (spring) button. v2.2 gave us a cursor
placement that cannot be refused and verifies itself (`GemPointer.To` — closed loop against
`GetCursorPos`, see [MOVE-SETS.md](MOVE-SETS.md)), so the tuner can put the cursor on the spring
itself and keep it there.

## Feature 1 — place the cursor on the spring at start

**Opt-in, like the gem's move sets.** `tuner.spring_mode`:

| mode | behaviour |
|---|---|
| `manual` (default) | **today's behaviour, unchanged** — you put the mouse on the 發條 button yourself; the tuner only sends `C` + `E`. No placement, no cursor polling. |
| `hid` (advanced) | the tuner places the cursor on the spring with the Arduino closed loop at run start, and (if the guard is on) keeps it there. |

This mirrors `gem.move_mode: tuned \| arduino` (see [MOVE-SETS.md](MOVE-SETS.md)): the old way stays
the default and nothing changes for an untouched install.

| piece | detail |
|---|---|
| config | `tuner.spring_point: [x, y]` — client-relative **physical** pixels, in `local.yaml` (machine-specific, like the gem points). Only read in `hid` mode. |
| calibration | Calibrate Tuner grows a 4th step: after the three OCR drags, "click the 發條 button". The tab already has a step flow (`_tunerStep` 0→3); this adds 3→4. |
| test | a **Test Click** on that point, exactly like the gem tab's — place, click, log the landing |
| runtime | in `hid` mode, after the 5-second countdown (so you can still cancel), place the cursor on the spring. On failure **stop and report on the card**, same policy as the composer — never click blind |
| reuse | `GemPointer` is really "the Arduino pointer"; rename to `HidPointer` in Core and update callers (mechanical, no behaviour change) so the tuner reusing it reads honestly |

Note the tuner's stop conditions are unchanged by this: **the target grade outranks everything else**
— that is the run's purpose, and everything below (filter, springs, retries) only bounds how long we
keep trying to reach it. `remaining <= 0`, same result ×3 and max retries stay as they are.

## Feature 2 — the cursor guard (the toggle)

Only meaningful in `spring_mode: hid` — in `manual` mode the tuner never touches the cursor, so there
is nothing to guard.

Config: `tuner.mouse_guard: off | stop | recenter`, plus `tuner.guard_px` (default ~8) and
`tuner.recenter_max` (default ~20).

| mode | behaviour |
|---|---|
| `off` | today's behaviour — the tool never looks at the cursor |
| `stop` | if the cursor leaves the spring point by more than `guard_px`, **stop** the run with *"mouse moved — stopped"* on the card. This is the human-takes-over case. |
| `recenter` | if it drifts, place it back on the spring and carry on. **Meeting the target grade outranks everything else** — that is what the run is for — so re-centring continues until the target is met (or another stop fires: out of springs, same result ×3, max retries). `recenter_max` is only a runaway guard, not a normal ending. |

**How the drift is detected.** Poll `WindowFinder.LogicalCursorPosition()` on the tuner's existing
`SleepCheck` loops (every ~50 ms is plenty). The tool's own actions do not move the cursor — clicks
are HID button events, and every move it makes is one it placed itself — so any movement is external:
you, or the game.

**The one real risk: the game warping the cursor.** If the game re-centres or clips the pointer (on
focus changes, or when it leaves the window), `stop` would false-trigger. Mitigations before choosing
a default:

- a tolerance (`guard_px`) rather than exact equality,
- require the cursor to be away for two consecutive polls,
- only judge while the game window is the foreground window,
- and **measure first** — see the open questions.

If the game does warp the cursor, `recenter` is the natural default and `stop` becomes the opt-in.

## Feature 3 — other things the reliable mouse action unlocks (ideas, not committed)

1. **Park the cursor away from the OCR regions.** The tuner reads its grade/attribute bands from a
   screen capture; if the pointer sits over one, its pixels are in the crop. Sequence: place → click →
   park at a neutral point → capture/OCR → back to the spring. Worth measuring before building.
2. **A global human override.** Feature 2's detection, applied to every tool: grab the mouse and the
   running tool stops. Cheap to add once the polling exists.
3. **Self-healing moves in the composer.** It already re-aims on every move; it could also verify the
   click landed (grade/result changed) and retry once.
4. **More of the UI automated** — confirm dialogs, "keep/take" buttons — as additional calibrated
   points, reusing the same place-and-click primitive.
5. **Anti-AFK nudge** (toggle): a tiny periodic movement, using the same placement code.
6. **A Test Click for every calibrated point**, tuner included, so any coordinate can be checked
   without running the tool.

## Suggested order (each step = one commit, each buildable)

1. Rename `GemPointer` → `HidPointer` (mechanical; no behaviour change).
2. `tuner.spring_mode` (`manual` default) + `tuner.spring_point`: config fields + the 4th calibration
   step + Test Click. `manual` still runs exactly as today.
3. In `hid` mode, place the cursor on the spring when a run starts; stop with a card message on
   failure.
4. `tuner.mouse_guard` (`off`/`stop`/`recenter`) + the poll loop + the UI control — `hid` mode only.
5. Answer the two data questions below, then decide on the cursor-parking idea.

## Open questions to answer with data before choosing defaults

- [ ] **Does the game warp the cursor during a tuning run?** Log `GetCursorPos` once per attempt for
      one full run; if it never moves by more than a pixel, `stop` is safe.
- [ ] **Does the cursor over the tuning UI affect OCR?** `logs/ocr_log.jsonl` records every raw item
      with its `rowKey`; compare attempts where the pointer sat over the read bands against ones where
      it didn't. This also feeds the open `BuildLines` row-bucket question in [TODO.md](TODO.md).

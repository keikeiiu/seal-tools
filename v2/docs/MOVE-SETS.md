# Gem Composer move sets — `tuned` and `arduino`

The composer has to move the mouse cursor between a handful of fixed game buttons (the three grade
radios, **Register**, **Combine**, the three resource slots). There are two ways it can do that, and
they are both live: **nothing was replaced.** Which one runs is `gem.move_mode`.

| | `tuned` (default) | `arduino` |
|---|---|---|
| What is sent | one `D dx dy` per route, using the hand-tuned counts in `gem.movements` | `D dx dy` corrections in a loop until the cursor sits on the route's destination **point** |
| Where the numbers come from | tuned by hand on this PC (Calibrate Gem → "Composer moves") | none — it reuses the points you already calibrated |
| Corrects itself | no — a missed move stays missed | yes, every move re-measures and re-aims |
| Sensitive to pointer speed / acceleration | yes | no (the loop absorbs it) |
| Extra cost per move | none | a settle-poll, ~30–80 ms |
| Defined by | `gem.movements` in `config/local.yaml` | `GemRoutes.All` + `gem.grade_positions` / `gem.resource_gems` |

**Choosing one.** Calibrate Gem → **New Gem Composer Moves** → *Composer move mode*, then **Save Gem
Composer** (the choice is written to `config/defaults.yaml` as `gem.move_mode`). `tuned` is the
default, so an untouched install behaves exactly as before.

## How the `arduino` move works

It is the same mechanism as the **Test Click** button. A route is defined by the *point it ends on*
(`GemRoutes`), not by a delta:

```
N → Register        ends on "Register"
Register → Combine  ends on "Combine"
Combine → Register  ends on "Register"
Register → Resource1, Resource1 → Resource2, Resource2 → Resource3   end on that slot
Resource3 → N/G/DG  ends on that grade radio
```

`GemPointer.To(ser, target)` then drives the cursor there:

1. **Read** the current position with `GetCursorPos` (this always works in our process — it is the
   *move* API that is refused, see `CURSOR-INVESTIGATION.md`).
2. **Compute** the remaining error `target − current` in the process's own (logical) cursor space.
3. **Stop** if the error is within **2 px** — a game button is tens of pixels wide, so this is a hit.
4. **Send** the error as a `D dx dy` HID move, clamped to **600 px per move** so a wrong target can't
   fling the pointer across the desktop.
5. **Wait for the move to finish** (see below), then go back to step 1.

The measured gain is exactly **1:1** — `D 100 0` moves the cursor +100 px in that space, and 250/500/
600 px all land exactly — so a normal move converges in **one** iteration. The loop is bounded at 6
moves, and if it still hasn't arrived it returns `ok=false` with the position it stopped at. Callers
**never click through a failed placement**: the composer stops and puts the reason on the card, and
the test buttons say so in the hint.

### The one non-obvious part: the firmware walks long moves out in chunks

`arduino/seal_mouse.ino` implements `D dx dy` as a loop of **10-px `Mouse.move` calls with `delay(1)`
between them**. A 600 px move therefore keeps arriving for tens of milliseconds *after* the serial
write returns. Reading the position too early made the loop chase a stale value and stack a second
move on top of a first that was still running — which is how a far target once ended up 400 px short:

```
step 1 : at (2400,1400) err=1671 sent=(-600,-600) -> (2000,1001) actualDelta=(-400,-399)
step 2 : at (1990,991)  err=1261 sent=(-600,-296) -> (1560,560)  actualDelta=(-430,-431)  <- y overshoots
```

So after every move `To` polls `GetCursorPos` until two consecutive reads (15 ms apart, bounded at 24
polls) agree — i.e. the cursor has stopped moving. That is the whole fix for the chunking.

### Failure modes and what they mean

| Message | Cause |
|---|---|
| `'{point}' isn't calibrated yet` | that grade/Register/Combine/slot has no saved coordinate |
| `couldn't move the cursor to {point} — the cursor wouldn't move…` | the cursor never reached the point within 6 moves |
| `the cursor position can't be read (GetCursorPos failed)` | `GetCursorPos` itself failed (rare) |

## Testing a move without running the composer

Calibrate Gem has two test sections, and they test *different sets*:

- **Composer moves** (dx/dy rows) — **Test** sends that row's raw tuned counts.
- **New Gem Composer Moves** — **Test** places the cursor on the source point, clicks, places it on
  the destination point, clicks. That is exactly what the composer does in `arduino` mode, so a route
  that lands here lands in the composer.

Each New-Gem-Composer-Moves test appends one line to `logs/arduino_debug.txt`:

```
01:32:15 test-route-arduino Register → Combine from ok=True at (829,725), to ok=True at (785,871) fg="-TW_LIVE"
```

## Reverting / coexistence

`gem.movements` is never modified by the `arduino` set — the tuned values stay in `local.yaml` and the
tuned editor stays on screen. Switch the mode back to `tuned` (or delete the `move_mode` line, which
defaults to `tuned`) and the old path runs unchanged.

## Rules for anyone touching this later

- **Do not scale `gem.movements` by DPI.** They are raw HID counts, not pixels, and they are
  hand-tuned per PC. The `arduino` set is the alternative to tuning — it does not re-interpret them.
- **Do not replace the settle-poll with a fixed sleep.** The firmware's chunk pacing is what makes a
  fixed sleep read a stale position; the poll is the fix, not a workaround.
- **Never click after a failed placement.** The whole point is that a refused move used to be silent.
- A negative cursor coordinate is **legitimate** on a monitor left of the primary one — it must never
  be used as a failure signal (`LogicalCursorPosition` returns `null` for failure instead).

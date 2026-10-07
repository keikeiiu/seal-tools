# Tool flows — every tool, in one place

Written 2026-10-08, from the source as it stands on `feature/input-arbiter`. **Documentation, not a
change.** Every action each tool sends to the game, every branch it takes, and where it can be stopped,
so the flows can be read rather than reconstructed file by file.

Line numbers are given so any claim here can be checked against the code. Where a number has drifted,
the method name is the durable reference.

**How to read it.** §1 is the machinery every tool shares — read it once and the per-tool sections get
much shorter. §2 is the whole suite at a glance. Then one section per tool, each with a flow diagram, an
inventory of every action sent, and a decision table. **The pet feeder is the exception**: its details
already have their own document, so its section here is the outer loop plus a pointer.

**No line numbers.** The diagrams describe what a tool DOES, not where that code lives — a reader
following a flow wants the shape of it, and a citation on every branch is noise that hides the shape.
The code is the source of truth for line numbers; this document is for reading.

### Notation

```
  ●            the entry point of the flow
  ◆            a decision — every branch out of it is drawn beneath
  ▓            a write to the board, i.e. the game sees this
  ✋            the run ends here (by itself, or because it was stopped)
  ⟲            the end of a repeating body; control returns to the loop head
  ──►          a branch that leaves the spine
  ══ LOOP ══   a bar above a loop head
  ┏ ┃ ┗        a box around a loop body
```

Left-aligned where possible, one idea per line, and a loop drawn so its body is visibly one block.

**The loop is always marked by its `⟲`.** The two fence marks are emphasis, drawn where a body is long
enough to be hard to follow without one. The eight sections were written separately and did not agree on
which fence to use, so all three presentations appear; they mean the same thing, and unifying them is a
tidy-up rather than a correction.

---

## 1. The machinery every tool shares

All eight tools are the same shape underneath. Reading this once is why no tool section below has to
explain how a stop works.

| Piece | What it does | Where |
|---|---|---|
| **`ToolBase`** | The shared skeleton: a hotkey-aware `SleepCheck` and the cancellation rule. | [ToolBase.cs](../SealTools.Core/ToolBase.cs) |
| **`PortGate`** | One holder at a time. `TryAcquire` / `Release` / `Owner` / `AnnounceWaiting` / `WithdrawWaiting`, plus an optional **deadline** per claim. | [PortGate.cs](../SealTools.Core/PortGate.cs) |
| **`HeldKeys`** | Which tools can leave something **held down** when they stop — only `holdspace` and `pet` today. Every stop path asks it. | [HeldKeys.cs](../SealTools.Core/HeldKeys.cs) |
| **`PortChoice`** | Which serial port to open, and when to refuse instead. | [PortChoice.cs](../SealTools.Core/PortChoice.cs) |
| **`ToolState`** | The card's live text. The UI polls it every 750 ms. | [ToolState.cs](../SealTools.Core/ToolState.cs) |

### 1.1 How a run starts

```
  ●  Start("gem")

  1  displace        stop whatever this start replaces
  │                  — everything EXCEPT the pet, which keeps its own schedule (§2)
  │
  2  take the game   ◆ is it free?
  │                     yes ──► claim it for this run
  │                     no  ──► wait, bounded, and SAY who it waits for
  │
  3  open the port   ◆ did a port resolve?
  │                     no  ──► ✋ refuse, and put the reason on the card
  │                     yes ──► open it (a cold start waits ~2 s for the board to boot)
  │
  4  run             hand the tool the port, the card state, and ONE cancellation token
  │
  └──► the tool's own flow — one section per tool, below
```

### 1.2 How a run stops — the part worth reading twice

```
  ●  Stop pressed
  │
  1  deregister      the registry entry goes first, so nothing else still reads it as running
  │
  2  let go          ◆ can this tool hold something DOWN?
  │                     yes ──► ▓ release it now — the spacebar, or the mouse button
  │                            (only Hold Space and the pet; the rest hold nothing)
  │
  3  cancel          the token is cancelled, and the tool's next sleep THROWS
  │                  └─ this is what abandons a step IN PROGRESS. A pet visit runs for
  │                     minutes, and a stop honoured only at a visit boundary is a button
  │                     that looks broken.
  │
  4  cleanup         the tool's own finally blocks run — let go of the game, release a
  │                  held key, close the window. Cleanup is NEVER abandoned by the throw.
  │
  5  report          the launcher catches the throw and reports NOTHING: a stop is not a
  │                  crash, so it writes no card message and no error log.
  │
  └──► registry cleared · token disposed · the game handed back
```

**The rule that holds it together: a stop abandons the WORK, not the CLEANUP.** Throwing from an
arbitrary point is only safe because two things are true, and both are enforced:

- **Anything that presses is inside the same `try` whose `finally` releases.** There are exactly two
  press/release pairs in the suite — the pet's left mouse button and Hold Space's spacebar — and both
  are structured this way.
- **Cleanup that sleeps runs with cancellation muted**, via `ToolBase.WithoutCancellation`. Otherwise a
  stop arriving inside a teardown aborted the teardown itself: the pet left its boarding window open
  over the game, and a `finally` that throws *replaces* the exception it was unwinding — which would
  file a real crash as a clean stop.

### 1.3 How a run ends by itself

Only one tool does, and until it existed none did: the **quest** tool. Every other tool loops until it
is stopped. A tool that returns on its own is torn down by the worker's `finally`, which has to tell a
self-end from a stop — it compares the **CTS** before removing the registry entry, because a stop may
have already replaced it.

---

## 2. The suite at a glance

| id | Name | Ends on its own | Holds anything down | Takes the gate | Card |
|---|---|---|---|---|---|
| `tuner` | Magic Tuner | no | no | yes (foreground) | yes |
| `gem` | Gem Composer | no | no | yes (foreground) | yes |
| `spammer` | Skill Spammer | no | no | yes (foreground) — and it **yields** | yes |
| `buy` | Buy Items | no | no | yes (foreground) | yes |
| `sell` | Sell Items | no | no | yes (foreground) | yes |
| `quest` | Quest Hand-in | **yes** | no | yes (foreground) | yes |
| `pet` | Pet Feeder | no (loops for days) | **yes** — left mouse button | **resident** — takes it per visit | yes |
| `holdspace` | Hold Space | no | **yes** — spacebar | yes (foreground) | **no card** — a toggle top-right |

Two terms that are easy to confuse:

- **Foreground** — a normal tool. Starting one stops the others, and it owns the game for its whole run.
- **Resident** (`pet`) — the one tool whose `Start` does **not** stop the others, and which another
  tool's `Start` does **not** stop. It is background furniture that keeps its own schedule.

---

## 3. The tools

Sections follow, one per tool, each with a flow diagram and an inventory of every action sent.



## Magic Tuner — `tuner`

*Clicks the 發條 (spring) button through the Arduino, OCRs the result panel, and repeats until the
target grade (and filter) is met — stopping on success, on an empty/never-changing/over-budget read, on
a cursor drift guard, on a write fault, or on the user's Stop.*

### What it is

| Term | What it is |
|---|---|
| **Attempt** | One C-then-E cycle: one click on 發條, one OCR scan of the result panel. |
| **Countdown** | Five one-second ticks after a start, before the first attempt. |
| **C / E** | The Arduino commands sent per attempt — `"C\n"` is a left-click, `"E\n"` is Enter. |
| **Signature / `prevSig`** | `(grade, remaining, joined attributes)` — the key compared to spot a repeating result. |
| **`consecutiveRepeats`** | How many times the last signature has repeated; `>= 2` means three identical reads in a row. |
| **Grade order** | `tuner.grade_order`; a grade's rank is its `IndexOf` there. |
| **Filter** | An attribute filter (`tuner.filter`); its `Passed` result is `filterPass`. |
| **`requireGrade`** | The filter's own grade requirement; with the target it forms `effectiveGrade`. |
| **`spring_mode`** | `"hid"` makes the tool place the cursor on 發條 and arm the guard; anything else skips both. |
| **`spring_point`** | The calibrated 發條 position, `[x, y]`. |
| **`mouse_guard`** | `"off"`, `"stop"` or `"recenter"` — how a drifting cursor is treated. |
| **Recenter** | Guard mode `"recenter"` re-places the cursor; after `recenter_max` put-backs it stops. |
| **The gate** | `PortGate`; claimed under the id `tuner` by the launcher, not by this loop. |

### The flow

```
  ●  Run(ser, state, ct)

  1  arm the stop    WatchCancellation — any later sleep can throw the stop
  2  open the run    logs/run_<ts>.jsonl + .txt; build the OCR engine
  3  seed the state  running=false · countdown=0 · attempt=0 · f12Was

  4  LOOP  ←── the body below returns here, from the ⟲ marks
  ┌───────────────────────────────────────────────────────────────────────
  │ a  the wait      SleepCheck(0.05)   ← a Stop THROWS here
  │    ◆ F11 quit, or the token cancelled? ──► left the loop (stage 5)
  │
  │ b  the panel     ◆ panel says START? ──► running=true, countdown=5
  │                  ◆ panel says STOP?  ──► running=false, Beep(1000)
  │
  │ c  the toggle    ◆ F12 rising edge? ──► flip running
  │                       on → countdown=5, Beep(523)
  │                       off → countdown=0, Beep(1000)
  │
  │ d  the countdown ◆ countdown > 0? ──► 20 ticks of 50 ms
  │                  │    F12 down → abort it (running=false)
  │                  │    quit/cancel → leave the ticks
  │                  │  countdown--; at 0: "[>] RUNNING", Beep(1500)
  │                  │  ◆ spring_mode == "hid"? ──► ▓ place the cursor on
  │                  │       發條 (HID); a failure ──► ✋ end the run — reason on the card
  │                  └─ ⟲ continue
  │
  │ e  the idle      ◆ not running? ──► ⟲ continue, nothing sent
  │
  │ f  the attempt   attempt++
  │                  ◆ hid mode, and the cursor drifted? ──► ✋ end the run — reason on the card
  │                     (recenter mode may ▓ re-place the cursor first)
  │                  ▓ write "C\n" — the click
  │                  ▓ write "E\n" — Enter
  │                  ◆ a write threw (and is not a cancellation)? ──► ✋ stop
  │                  scan the panel; match attributes; run the filter
  │                  append this attempt to the run log
  │                  ◆ grade, remaining, attributes ALL empty? ──► ✋ end the run — reason on the card
  │                  ◆ the signature equals the last? ──► repeats++, else 0
  │                  refresh the card
  │
  │ g  the outcome   in this order:
  │        grade ok + filter ok ──► ✋ SUCCESS (beeps)
  │        filter on, no grade requirement, pass ──► ✋ MATCH (beeps)
  │        grade ok but the filter missed ──► keep going
  │        filter pass but grade below the requirement ──► keep going
  │        no springs left ──► ✋ OUT OF SPRINGS
  │        three identical reads ──► ✋ STUCK
  │        attempts >= max ──► ✋ MAX RETRIES
  │        pause asked ──► ✋ PAUSE
  │        (none of the above) ──► ⟲ back to the wait (4a)
  └───────────────────────────────────────────────────────────────────────

  5  the exit — every ✋ and every break lands here
     │  Stop cancels the token ──► the next sleep THROWS out of the loop
     │  F11, or any ✋ branch  ──► the loop breaks cleanly
     │  finally: state.Running = false   ("Done. N attempts.")
     └──► ✋ the run ends · the game is handed back
```

**Nothing is left held down and no window is left open:** the tuner uses only self-contained
primitives — a left-click, an Enter sent directly as `"E\n"`, and HID relative moves — it never
presses-and-holds (`LeftDown`/`LeftUp`), so there is no drag to release and no window it opens. Its only
teardown is the `finally`. What persists after a stop is the cursor's physical position, wherever the
last HID move left it.

### Every action sent to the game

The tuner sends exactly three kinds of input: HID moves, a click, and an Enter. **It sends no keyboard
input other than the Enter, and no right-click.**

| # | Action | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | HID move `D dx dy` onto 發條 (one or more, closed-loop) | `PlaceOnSpring`, via `HidPointer.To` | once per start, when `spring_mode=="hid"` and the countdown reaches 0 | no — the precondition for the click landing on the button |
| 2 | HID move `D dx dy` re-placing the cursor on 發條 | `CheckMouseGuard`, via `HidPointer.To` | only with `mouse_guard=="recenter"`, when the cursor has drifted | **yes** — a corrective re-placement |
| 3 | Click `"C\n"` | the attempt's click step | every attempt, before the scan | no |
| 4 | Enter `"E\n"` | the attempt's Enter step | every attempt, after the click and `ClickEnterDelay` | no |

**Side effects that are not game input:** the run log is written every attempt — `run_<ts>.jsonl`
(serialised) and `run_<ts>.txt` (one line) — and `state` is refreshed each attempt for the card.

### Decision points

| Where | Question | True → | False → |
|---|---|---|---|
| the wait | Quit hotkey (F11) or token cancelled? | leave the loop | tick on |
| the panel | Panel asked START and we are not running? | running, countdown=5, "[Panel] START" | fall through |
| the panel | Panel asked STOP and we are running? | running=false, Beep(1000) | fall through |
| the toggle | F12 rising edge (and not quitting)? | on → "[GO]", countdown=5, Beep(523); off → "[STOP]", countdown=0 | `f12Was=f12Now` |
| the countdown | countdown > 0? | print a tick and run the 20×50 ms inner loop | go to the attempt |
| the countdown | F12 down inside the countdown? | abort the countdown, running=false | keep counting |
| the countdown | quit or cancel inside the countdown? | break the inner loop | keep counting |
| the countdown | the countdown just reached 0? | "[>] RUNNING", Beep(1500), hid placement | loop back |
| the countdown | `PlaceOnSpring` returned an error? | message on card, running=false, stop | carry on |
| the idle check | not running? | continue, no attempt | run an attempt |
| the attempt | `spring_mode=="hid"` and the guard says stop? | running=false, break | attempt proceeds |
| `CheckMouseGuard` | `mouse_guard == "off"`? | never stop — return false | judge drift |
| `CheckMouseGuard` | `spring_point` not calibrated? | the guard can't judge — false | read the window |
| `CheckMouseGuard` | game window missing or minimised? | **fail open** — false ("no drift") | read the cursor |
| `CheckMouseGuard` | cursor position unreadable? | **fail open** — false | compute the drift |
| `CheckMouseGuard` | drift > `guard_px`? | stop (stop mode), or re-place and count (recenter mode) | `recenterCount=0`, false |
| `CheckMouseGuard` | recenter failed or `recenterCount > recenter_max`? | stop with a message | log "[recenter #n]", false |
| the C/E write | a C/E write threw (and is not a cancellation)? | "[!] Arduino disconnected", stop | continue |
| the read | grade, remaining and attributes all empty? | "READ FAILED" message, stop | keep the result |
| the repeat check | signature equals the previous one? | `consecutiveRepeats++` | `consecutiveRepeats=0` |
| the outcome | `gradeOk` AND `filterOk`? | SUCCESS — BeepMany, stop | next check |
| the outcome | filter enabled, no `requireGrade`, `filterPass`? | filter matched — BeepMany, stop | next check |
| the outcome | `gradeOk` but the filter did not pass? | log "continuing" | next check |
| the outcome | filter passed but the grade is below `requireGrade`? | log "continuing" | next check |
| the outcome | `remaining <= 0`? | OUT OF SPRINGS, stop | next check |
| the outcome | `consecutiveRepeats >= 2`? | STUCK, stop | next check |
| the outcome | `attempt >= max_retries`? | "[!] Max N", stop | next check |
| the outcome | `PauseRequested`? | "[PAUSE] graceful stop" | loop again |

**Worth a verdict:**

1. **A non-`hid` `spring_mode` clicks blind.** Both the placement (`PlaceOnSpring`) and the guard
   (`CheckMouseGuard`) are gated on `SpringMode == "hid"`, so with any other value the tool sends C/E
   wherever the cursor happens to sit, with no placement and no drift check. If the C command still
   lands only on 發條 when the cursor is somewhere else, that is the one thing to confirm.

2. **The mouse guard fails open.** A missing/minimised game window, or an unreadable cursor, makes
   `CheckMouseGuard` return false — "no drift" — so the loop keeps clicking instead of tripping. The
   code calls this an acknowledged fail-open rather than a bug; it is worth a verdict because the guard
   is the only thing protecting the click target in `hid` mode.

3. **A panel STOP does not abort a countdown that is already running.** The panel-STOP branch clears
   `running` but not `countdown`, and only F12 aborts the countdown; the countdown therefore runs to 0
   and may still call `PlaceOnSpring` and cross the cursor.

4. **The attempt counter survives a stop/start.** `attempt` is not reset when F12 re-starts a run, so
   toggling start/stop within one `Run` keeps driving it toward `max_retries`.

5. **The "Arduino disconnected" catch covers only C and E.** `PlaceOnSpring`, and the guard's
   re-placement, write to the port *outside* the catch, so a write fault there throws out of `Run` with
   the friendly message never shown.

6. **Stop reasons are uneven on the card.** Only the all-empty read and the guard stops set
   `state.Message`; SUCCESS, OUT OF SPRINGS, STUCK and MAX stop silently — and their `Console.WriteLine`
   is invisible in the WinExe.

7. **The all-empty read is special-cased before the repeat counter** precisely so a total OCR failure
   cannot count as the first of three identical reads; that ordering is load-bearing, not incidental.

---

## Gem Composer — `gem`

*Selects a gem grade, moves to Register, clicks to combine and re-registers, and on an empty result box
(or a cleared resource slot) advances N → G → DG once; it stops on request, on a failed
placement/route, when the last grade is done, or when something is not calibrated.*

### What it is

| Term | What it is |
|---|---|
| **Cycle** | One combine: Move Register→Combine, click, empty check, Move Combine→Register, then either the deregister+register clicks or an advance. |
| **Grade** | One of `gem.grades` (e.g. `N`/`G`/`DG`); the current one is `grades[gidx]`, starting at `gem.start_grade`. |
| **Register** | The register/combine button; routes named `register_combine`, `combine_register`, `radio_<grade>`. |
| **Route** | One composer movement. `gem.move_mode=="arduino"` places the cursor on the route's destination point; otherwise it sends the hand-tuned counts in `gem.movements`. |
| **Point** | A calibrated named position resolved by `GemRoutes.Resolve`. |
| **Empty result box** | The composed gem slot showing no gem — `IsResultBoxEmpty`. |
| **Diff fraction / `empty_distance`** | Fraction of pixels differing from the saved empty crop; `<= gem.empty_distance` reads as empty. |
| **Empty signature** | The fallback colour test when the crop is missing. |
| **`empty_streak` / `empty_count`** | Consecutive empty results needed before advancing. |
| **`empty_mode`** | `"stop"` (no auto-advance) or `"advance_grade_clear"` (clear the resource slots first). |
| **The gate** | `PortGate`; claimed under the id `gem` by the launcher, not by this loop. |

### The flow

```
  ●  Run(ser, state, ct)

  1  arm the stop     WatchCancellation — any later sleep can throw the stop
  2  read the grades  grades = gem.grades; gidx = index of start_grade
  3  seed the state   running=false · cycle=0 · emptyCount=0 · f12Was · f9Was

  4  LOOP  ←── the body below returns here, from the ⟲ marks
  ┌───────────────────────────────────────────────────────────────────────
  │ a  the wait      SleepCheck(0.05)   ← a Stop THROWS here
  │    ◆ F11 quit, or the token cancelled? ──► left the loop (stage 5)
  │
  │ b  the panel     ◆ panel says START? ──► running=true, Beep(523),
  │                                            select the grade — see stage 9
  │                  ◆ panel says STOP?  ──► running=false, Beep(1000)
  │
  │ c  F12 / F9      ◆ F12 rising edge? ──► flip running; on: Beep +
  │                  │                         select the grade — see stage 9
  │                  ◆ F9 rising edge?  ──► next grade (WRAPS round the list),
  │                                            select the grade — see stage 9
  │
  │ d  the idle      ◆ not running? ──► ⟲ continue, nothing sent
  │
  │ e  the cycle     cycle++
  │                  ◆ F12 held at the top of the cycle? ──► running=false,
  │                       "[STOP]" ──► ⟲ continue
  │                  ▓ route Register → Combine   ✗ failed ──► ✋ break
  │                  ▓ click "C" — combine
  │                  ◆ auto-advance armed (not "stop", signature set)?
  │                       ▼ the empty check (stage 8) ──► sets advanceNow
  │                  ▓ route Combine → Register   ✗ failed ──► ✋ break
  │                  ◆ advanceNow? ──► advance to the next grade (stage 6)
  │                       a false from stage 6 ──► ✋ break
  │                       a true ──► ⟲ continue
  │                  ▓ click "C" — deregister
  │                  ▓ click "C" — register (same spot, no move between)
  │                  ◆ pause asked? ──► ✋ PAUSE
  │                  (otherwise) ──► ⟲ back to the wait (4a)
  └───────────────────────────────────────────────────────────────────────

  5  the exit — every ✋ and every break lands here
     │  Stop cancels the token ──► the next sleep THROWS out of the loop
     │  F11, or any ✋ branch  ──► the loop breaks cleanly
     │  finally: state.Running = false   ("Done. N cycles.")
     └──► ✋ the run ends · the game is handed back

  6  ADVANCE to the next grade  (N → G → DG once — it never wraps)
     │  ◆ advance_grade_clear? ──► clear the resource slots first (stage 7)
     │  ◆ this is the last grade? ──► ✋ "[DONE] last grade", Beep(880)
     │  gidx++; refresh the card
     │  ◆ clear mode? ──► ▓ route Resource3 → the next grade; ▓ click "C"
     │      otherwise  ──► ▓ place on the grade button (HID); ▓ click "C"
     │  ▓ route the next grade → Register; ▓ click "C"
     └──► back to the cycle that called it

  7  CLEAR the resource slots  (only in advance_grade_clear)
     │  ▓ route Register → Resource1;  ▓ right-click "R"
     │  ▓ route Resource1 → Resource2; ▓ right-click "R"
     │  ▓ route Resource2 → Resource3; ▓ right-click "R"
     └──► three slots cleared, unconditionally, whether or not anything is stuck

  8  THE EMPTY CHECK  (judges the composed result box)
     │  ◆ the game window is the FOREGROUND window?
     │        no ──► "refused: not foreground", answer NOT empty
     │  capture the result region (CopyFromScreen); a null crop → NOT empty
     │  ◆ the saved reference crop is there? ──► pixel-diff <= empty_distance
     │      else a colour signature is set? ──► the colour test
     │      else ──► NOT empty  (never advance on a missing reference)
     │  ◆ empty this cycle? ──► emptyCount++; at empty_streak ──► advanceNow
     │      not empty     ──► emptyCount = 0
     └──► (with save_empty_captures: the crop and a log line are written)

  9  SELECT THE GRADE + REGISTER  (on start, on F12, on F9)
     │  ◆ the game window is there? ──► a failure: reason on the card, stop
     │  ◆ the grade button is calibrated? ──► ▓ place the cursor on it (HID)
     │        a failed placement ──► reason on the card — never click blind
     │  ▓ click "C" — select the grade (the click also focuses the game)
     │  ▓ route the grade → Register; ▓ click "C" — select Register
     └──► back to the step that called it
```

`★` marks a click sent while selecting a grade — these fire on start, on F12 and on F9 even before any
combine has run. **Nothing is left held down and no window is left open:** the tool uses only
self-contained primitives — a click, a right-click, and HID moves (and the closed-loop placement in
`HidPointer.To`) — it never uses `LeftDown`/`LeftUp`, so there is no drag to release and no window it
opens. Its only teardown is the `finally`. What persists after a stop is the cursor's physical position.

### Every action sent to the game

The composer sends exactly three kinds of input: HID moves, clicks, and right-clicks. **It sends no
keyboard input at all** (no Enter, unlike the tuner).

| # | Action | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | HID placement onto a point (one or more `D dx dy`) | `PlaceAt` and friends, via `HidPointer.To` | on start, on F9, and on every `arduino` route | no |
| 2 | Click `"C"` on the grade button | `SelectGradeAndRegister` | on start / F12 / F9, and on each advance | no |
| 3 | Move grade→Register (`D` tuned, or a placement) | `Route`, from `SelectGradeAndRegister` | after the grade click, every (re)select | no |
| 4 | Click `"C"` (select Register) | `SelectGradeAndRegister` | after the grade→Register move | no |
| 5 | Move Register→Combine | `Route`, in the cycle | every cycle | no |
| 6 | Click `"C"` (combine) | the cycle | every cycle | no |
| 7 | Move Combine→Register | `Route`, in the cycle | every cycle | no |
| 8 | Click `"C"` ×2 (deregister, register) | the cycle's normal path | every non-advancing cycle | no — the v1 re-select |
| 9 | Move Register→Resource1 / →Resource2 / →Resource3 | `ClearResources` | `advance_grade_clear` only, before advancing | no |
| 10 | Right-click `"R"` ×3 (clear stuck resource gems) | `ClearResources` | `advance_grade_clear` only — **unconditional**, fires even when nothing is stuck | **yes** |
| 11 | Move Resource3→next grade | `AdvanceGrade` | `advance_grade_clear` only | no |
| 12 | Click `"C"` (select next grade, slot3 path) | `AdvanceGrade` | `advance_grade_clear` only | no |
| 13 | Click `"C"` (select next grade, absolute path) | `AdvanceGrade` | non-clear mode, on advance | no |
| 14 | Move next grade→Register + Click `"C"` | `AdvanceGrade` | every advance | no |

**Read-only, not game input:** the result-box crop is a `CopyFromScreen` grab of `gem.result_gem_area`;
when `gem.save_empty_captures` is on it is written to `logs/captures/` and one line is appended to
`logs/empty_check.txt`. No game input clears or opens anything.

### Decision points

| Where | Question | True → | False → |
|---|---|---|---|
| the wait | Quit hotkey (F11) or token cancelled? | leave the loop | tick on |
| the panel | Panel asked START and we are not running? | running, Beep(523), select the grade | fall through |
| the panel | Panel asked STOP and we are running? | running=false, Beep(1000) | fall through |
| F12 | F12 rising edge? | toggle; on → Beep + select the grade, off → "[STOP]" | fall through |
| F9 | F9 rising edge? | `gidx=(gidx+1)%count` (**wraps**), select the grade | fall through |
| the idle check | not running? | continue, no cycle | run a cycle |
| the cycle top | F12 held at the top of a cycle? | stop, continue | combine |
| the route to Combine | Register→Combine route failed? | break, reason on card (`Fail`) | click to combine |
| the advance gate | `empty_mode != "stop"` AND `empty_signature` set? | run the empty check | skip it — `advanceNow=false` |
| the empty check | the game window is the foreground window? | judge the crop | refuse → "not empty", logged |
| the empty check | the result crop captured? | compare | "not empty" |
| the empty check | the empty-box reference crop loads? | pixel-diff vs reference | colour signature if set, else not empty |
| the empty check | diff fraction `<= empty_distance`? | box empty | box not empty |
| the empty check | empty this cycle? | `emptyCount++`; `>= empty_streak` → `advanceNow`, reset | `emptyCount=0` |
| the route to Register | Combine→Register route failed? | break | continue |
| the advanceNow branch | `advanceNow`? | `AdvanceGrade()`; false → break, true → continue | deregister+register clicks |
| `AdvanceGrade` | `empty_mode == "advance_grade_clear"`? | `ClearResources()` first | skip the clear |
| `AdvanceGrade` | `gidx+1 >= grades.Count`? | "[DONE] last grade", Beep(880), stop | `gidx++`, set grade |
| `AdvanceGrade` | slot3→next-grade route known? | place and click | `Fail("no route")` |
| `AdvanceGrade` | next grade point calibrated? | place it | `Fail` |
| `AdvanceGrade` | the placement landed? | click to select | `Fail` "stopped instead of clicking blind" |
| `AdvanceGrade` | next grade→Register route failed? | `Fail`, stop | click to select |
| the pause check | `PauseRequested`? | "[PAUSE] graceful stop", stop | loop again |
| `Fail` | any helper found a point/move uncalibrated or a placement to have failed? | message on card, running=false | carry on |

**Worth a verdict:**

1. **Auto-advance is gated on the fallback, not the primary signal.** The empty check runs only when
   `empty_signature != null`, even though the primary test — the pixel-diff against
   `calib_gem_result.png` — does not need it. A user with the crop calibrated but no colour signature
   would never auto-advance. Worth confirming whether `empty_signature` is always set.

2. **F9 wraps the grade list; `AdvanceGrade` does not.** F9 does `(gidx+1) % grades.Count`, while the
   automatic advance stops after the last grade. A manual cycle round the grades is therefore possible,
   but the run never wraps on its own.

3. **A non-foreground game silently disables advancing.** `IsResultBoxEmpty` refuses to judge unless the
   game is the foreground window and answers "not empty" — the safe direction, since a `CopyFromScreen`
   crop of the launcher is meaningless. The consequence is that a run with the launcher in front never
   advances, and the refusal is only recorded when `save_empty_captures` is on.

4. **`ClearResources` right-clicks three slots unconditionally**, on every `advance_grade_clear`
   advance, whether or not anything is stuck. Defensive; harmless if a right-click over an empty slot is
   a no-op.

5. **No click is verified after it lands.** Unlike the pet feeder's post-click slot check, the composer
   never re-reads to confirm a click did anything; a mis-landed click surfaces only as a later wrong or
   empty result. The only verification anywhere is the empty-box check, which judges the *outcome* of a
   combine, not the clicks.

6. **The two deregister/register clicks land on the same spot with no move between them** (the cursor is
   still on Register after the Combine→Register route), so they are a deregister then a register on the
   same button. That matches the v1 sequence the comment names, but it is worth knowing they are not two
   different buttons.

7. **A failure inside `SelectGradeAndRegister` does not stop the run directly.** It calls `Fail`, which
   sets `running=false` and `state.Running=false` but returns `void`, so the panel-START and F12 callers
   do not branch on it — the run ends by falling into the `!running` check on the next pass. Same end,
   one indirection.

8. **A serial write fault has no handler at all.** The tuner at least has a catch around its C/E writes;
   the composer's Click/RightClick/`D` writes are outside any try, so a fault throws out of `Run` with
   only the `finally` running.

## Skill Spammer — `spammer`

*Presses a cooldown-governed rotation of keys — every viable combo first, then the singles — and is the
one foreground tool that stands down for another; it has no natural end, and leaves its loop only on a
stop, a quit, a cancel, or a dead port.*

### What it is

| Term | What it is |
|---|---|
| **Preset** | A named key set (`Config.Spammer.Presets`, chosen by `Active`). `ActiveKeys` is the active preset's `key → cooldown-seconds` map, and it is **empty when the named preset is missing**. |
| **Key** | One entry of the active preset. A leading `*` spells the **fast tap** — the same press, sent lowercase. |
| **Cooldown** | Seconds a key waits after it was last cast before it is due again. |
| **Combo** | An ordered group that must be cast together, one step per `Gap`, with nothing else pressed in between. A key named in a combo is pressed **only** as part of it. |
| **Single** | A key the loop may press on its own: the preset minus everything a combo owns. |
| **Priority / preempt** | An ordered subset of keys. When non-empty, at most **one** press goes out per tick and the rest are **delayed**, not dropped. |
| **Lead-in / Step** | The opening: a wait before the first press (`LeadIn`), then the spacing between the opening presses (`Step`). |
| **Yield** | The spammer standing down from the game so a **waiting** tool can have it: it releases the gate, presses nothing, and re-acquires when the gate comes free. |
| **The gate** | `PortGate`. The launcher claims it for `spammer` at Start; the spammer is the only tool that gives it up mid-run and takes it back. |
| **Dead port** | A serial write that throws. The tool stops with a reason on the card rather than letting the exception unwind to the launcher. |

### The flow

```
  ●  Run — handed the port, the card state, and one cancellation token

  1  arm the stop    give the run's token to the base, so a later sleep THROWS
  2  read the preset ◆ any keys in the active preset?
                       no  ──► message on the card ──► ✋ nothing runs
                       yes ──► go on
  3  plan the run    combos first, then the singles that stand alone;
                     the preempt flag; the opening's lead-in and step

  ══ LOOP — repeats until the run is stopped ═════════════════════════════════

    4  tick          sleep ~20 ms; a QUIT press sets QuitPressed here
    5  ◆ stopped?    QUIT hotkey or run cancelled ───────────► ✋ end the run

    6  ◆ the panel   START ──► run · Reset (every key due at once)
                     STOP  ──► idle, but the loop keeps ticking
    7  ◆ F12 edge?   THE TOGGLE — never a terminate:
                       on  ──► run · Reset                       [GO]
                       off ──► idle, presses nothing             [STOP]
    8  ◆ pause?      graceful stop ─────────────────────────► ✋ end the run
    9  ◆ running?    no ──► press nothing ──────────────────► ⟲ tick again

   10  lead-in over? clear the countdown moment shown on the card

   11  THE YIELD — only when a game claim is wired
         holding  ◆ someone waits? ─► release the game; name them on the card
         yielded  ◆ claim free?    ─► re-acquire · Reset (no new lead-in)
         waiting  ◆ still held?    ───────────────► ⟲ press nothing

   12  ◆ a combo free? none running, every key off cooldown
                        ──► enqueue its keys; the combo owns the tick
   13  ◆ a step due?   one combo step ──► ▓ write that key ▓
                         (no filler goes out until the combo drains)
   14  ◆ a single due? otherwise walk the singles, each due key
                        ──► ▓ write it ▓  (preempt: one per tick)
   15  ◆ port dead?    a write threw ──────────────────────► ✋ end the run

  ⟲ back to stage 4 — the loop head. Nothing is left held between ticks.

  STOP PATH — however the loop leaves
   16  a Stop cancels the token ──► the loop's next sleep THROWS
       (a QUIT hotkey, or a dead port, leaves by BREAK instead)
   17  the tool's finally runs ──► clears "running"
   18  ─────────────────────────────────────────────► ✋ the run ends;
       the launcher hands the game back
```

**Every ▓ in that diagram is a tap, not a hold.** `SendKey` writes one command and returns; the firmware
does the press-and-release. So the spammer keeps **nothing** down between ticks, and it is deliberately
*not* one of the tools `HeldKeys` names. That is why there is no held-key moment anywhere in this flow:
there is no point at which this tool can leave something pressed.

**The lead-in presses nothing at all.** From `Reset` until the opening moment the loop ticks and the
F12/stop branches still run, but no key is sent — which is why the countdown is pushed to the card's
SCHEDULE line as a *moment* (`NextActionAt`) rather than a number.

### Every action sent to the board

This is the complete set of bytes the tool writes. It is four commands, and nothing else — no mouse, no
gate command, no held key. All four are written by `SendKey`.

| # | Command written | When | Defensive? |
|---|---|---|---|
| 1 | `F <n>\n` — press function key n (1–12) | a key or combo step is due | no |
| 2 | `f <n>\n` — the fast tap of the same | the key was spelled `*F…` | no |
| 3 | `K <c>\n` — press one printable ASCII char | a key or combo step is due | no |
| 4 | `k <c>\n` — the fast tap of the same | the key was spelled `*…` | no |

**Side effects that are not board input, but are actions:**

- **`state.*` writes** — `Message` (the yield line and every failure), `Schedule` / `NextActionAt` (the
  lead-in), `Current` and `Cycle` (the last key and the count), and `Running`.
- **The gate is released and re-acquired** by the tool itself on the yield path — the only place any tool
  does this.
- **`Console.WriteLine` and `Beep`** throughout — both invisible in the published WinExe, which is why
  the card state above carries the same information.

### Every decision point

| # | Decision | Step | One way | The other |
|---|---|---|---|---|
| 1 | Active preset has keys? | read the preset | run | refuse on the card, return 0 without a run |
| 2 | Quit hotkey / run cancelled? | quit check | **end the run** | keep ticking |
| 3 | Panel said Start? | panel sync | `run`, clear `QuitPressed`, `Reset()` | — |
| 4 | Panel said Stop? | panel sync | `idle` (the loop lives on) | — |
| 5 | `F12` pressed (an edge)? | F12 toggle | **toggle** run/idle; if starting, clear `QuitPressed` + `Reset()` | — |
| 6 | Pause hotkey? | pause check | graceful stop, **break** | — |
| 7 | Running? | running gate | press | tick again, pressing nothing |
| 8 | Somebody waits for the gate? | the yield | release it and yield | keep the game |
| 9 | Yielded, and the gate is now free? | the yield | re-acquire, `Reset` without a fresh lead-in | stay paused |
| 10 | A combo is fully off cooldown? | combo start | enqueue and cast it | fall through to the singles |
| 11 | A combo step is due? | combo step | send ONE step | the combo keeps the tick |
| 12 | A single is due? | single walk | send it; with `preempt`, one per tick | skip to the next tick |
| 13 | The key can't be sent? | `SendKey` | warn once, do **not** count it | — |
| 14 | The write threw (dead port)? | dead-port check | stop with a reason on the card | — |

### Worth a verdict

Not description — the things I would want a decision on.

1. **The hotkey the toggle actually is.** This page's brief described the *quit* hotkey as a toggle that
   "clears `QuitPressed` and carries on". The code disagrees, and the labels are the kind that get
   swapped: the **toggle** is the **Start / stop** hotkey, `F12`, and the **quit** hotkey, `F11`, sets
   `QuitPressed` and the loop **breaks** on it. The clearing of `QuitPressed` is real but sits in the
   *start* paths.

2. **The `QuitPressed = false` writes are unreachable.** Both clears sit *after* the top-of-loop
   `if (QuitPressed) break`, and nothing between can have set the flag except the tick that just ran —
   which would have broken first. So the "pause, not quit" meaning the base class attributes to these
   writes, and which PROGRESS §62 relies on, is in this loop dead code. Either the break is the intent
   (F11 ends the run, and the clears should go), or the clears are the intent (F11 should pause) — the
   two cannot both be true.

3. **The yield rule has not moved onto `ShouldStandDown`.** The spammer yields with a hand-written test —
   the gate is ours and `Waiting` names a waiter — while `PortGate`'s own doc says `ShouldStandDown(owner)`
   "replaces every hand-written pair-wise rule" and that "the spammer yields to anyone waiting". Behaviour
   matches today only because the sole waiter (the pet) declares a deadline — `ShouldStandDown` returns
   **false** for a waiter with no deadline — so swapping the call in now would quietly stop the yield for
   any future deadline-less waiter.

4. **The yield has no timeout.** If the waiter never releases the gate, the spammer stays paused with the
   card reading "paused — … has the game" for as long as that lasts; there is no attempt limit or expiry.
   The comment says the pause "lasts exactly as long as the waiter's action does", which is honest — but
   it also means a wedged waiter silently starves the rotation.

5. **`WarnedKeys` is a `static` set**, so a bad key warned during one run is never warned again for the
   life of the process, across later runs. The warning is one-per-process, not one-per-run.

6. **The F-range is narrowed to 1–12 here** even though the comment notes `ParseVk` accepts `F1`–`F24`
   elsewhere. A preset naming `F20` is refused with a one-time warning and is simply never pressed —
   deliberate, but it means such a key looks configured and does nothing.

---

## Hold Space — `holdspace`

*Holds the spacebar down on the board (one `P`) so the game picks items up, and always lifts it (`U`) on
the way out — including in the loop's `finally` — so the key can never be left stuck down.*

### What it is

| Term | What it is |
|---|---|
| **Hold / Release** | The two board commands: `P\n` presses the spacebar down, `U\n` lifts it. |
| **The toggle** | The launcher's top-right button, `HoldSpaceToggle` — this tool has no card, so the button is its only control. |
| **Engaged** | The question the toggle asks — loaded, *starting*, or *stopping* — so a press during a cold start means stop, not a second start. |
| **Held down** | `HeldKeys` names `holdspace` as one of the two tools that can leave something held, so **every** stop path must release it. |
| **ReleaseHeld** | The launcher's own release — `U\n` **and** `l\n`, sent on every stop path independent of this loop. |
| **The gate** | The launcher claims `PortGate` for `holdspace` at Start; unlike the spammer, this tool **never** reads it and never yields. |

### The flow

```
  ●  Run — handed the port, the card state, and one cancellation token

  1  arm the stop    give the run's token to the base, so a later sleep THROWS

  ══ LOOP — repeats until the run is stopped ═════════════════════════════════

    2  tick          sleep ~50 ms; a QUIT press sets QuitPressed here
    3  ◆ stopped?    QUIT hotkey or cancelled ───────────────► ✋ end the run

    4  ◆ the panel   START ──► holding:
                               ▓ write "P" — the spacebar goes DOWN
                     STOP  ──► idle:
                               ▓ write "U" — the spacebar lifts
    5  ◆ F12 edge?   THE TOGGLE — never a terminate:
                       on  ──► ▓ write "P"  (down again)
                       off ──► ▓ write "U"  (lifted)

  ⟲ back to stage 2 — the loop head.

  STOP PATH — however the loop leaves
    6  the tool's finally ──► ▓ write "U" — never leave the spacebar stuck
    7  ──► clear "running"
    8  ──► ✋ the run ends; the launcher sends its own release on every stop
```

**▓ marks every board write** — a `P` that presses the spacebar down, or a `U` that lifts it. Every DOWN
is matched by a `U`: the stop and toggle paths send one immediately, and the `finally` sends one however
the loop leaves — a `break`, or an exception thrown out of the loop body. The `finally` is the
load-bearing one: it is the release that still goes out when the loop's own write is what failed. The
launcher duplicates it deliberately — `ReleaseHeld` sends `U\n` (and `l\n`) on every stop path, including
when the port is closed, so the tool's own `finally` is not the only thing standing between the player
and a stuck key.

### Every action sent to the board

Two commands. That is the whole inventory.

| # | Command written | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | `P\n` — press the spacebar down | `Hold()` | panel Start, or an `F12` *start* edge — once per start | no |
| 2 | `U\n` — lift the spacebar | `Release()` | panel Stop, an `F12` *stop* edge, **and the loop's `finally`** | **yes** — the `finally` guarantee |

**Side effects that are not board input, but are actions:**

- **`state.Message`** is written when a `P` or `U` write throws — but see the verdict list: this tool has
  no card, so nothing renders it.
- **`state.Running`** is set true on each start and false in the `finally`.
- **The launcher's `ReleaseHeld`** sends `U\n` **and** `l\n` on every stop of this tool, not just the
  spacebar — see the verdict list.
- **`Console.WriteLine` and `Beep`** — invisible in the published WinExe.

### Every decision point

| # | Decision | Step | One way | The other |
|---|---|---|---|---|
| 1 | Quit hotkey / run cancelled? | quit check | **break**; the `finally` releases | keep ticking |
| 2 | Panel said Start (or an `F12` start edge)? | panel / toggle | `Hold()` — write `P` | — |
| 3 | Panel said Stop (or an `F12` stop edge)? | panel / toggle | `Release()` — write `U` | — |
| 4 | The loop leaves (any reason)? | the `finally` | the `finally` writes `U` | — |
| 5 | A `P`/`U` write throws? | `Hold` / `Release` | `state.Message` is set (see below) | — |

### Worth a verdict

Not description — the things I would want a decision on.

1. **This tool's failure report has nowhere to go.** `Hold()` and `Release()` write their failure to
   `state.Message`, but Hold Space has no card and the launcher says so in as many words — "*its
   `state.Message` has nowhere to be drawn*" — and surfaces only **its own** `ReleaseHeld` failure on the
   status line beside the toggle. So a failed **`Hold`** (the `P` write throwing) is recorded on a state
   nobody displays: the button would read "holding" over a spacebar that was never pressed. A failed
   **release** is covered, but by the launcher's copy, not this one.

2. **`QuitPressed = false` is unreachable here too.** Both clears sit after the top-of-loop
   `if (QuitPressed) break`, so the same contradiction as the spammer applies: the start paths say "pause,
   not quit", the loop-top break says "quit ends the tool", and the break wins.

3. **A `Hold` needs no yield, and this tool has none.** The launcher claims the gate for `holdspace` at
   Start, and this tool never reads it or gives it up (the launcher passes no `PortGate` when it builds
   `HoldSpace`). Unlike the spammer — the one tool that stands down for a waiter — Hold Space will block
   the resident pet feeder for as long as it is left on, and it "never ends on its own" either. Worth
   confirming that the pickup toggle is expected to be short.

4. **A stop of Hold Space releases the mouse button as well.** Every stop path runs `ReleaseHeld`, which
   sends `U\n` **and** `l\n` unconditionally. The two are idempotent, so this is safe *today* — the pet
   cannot be mid-drag while holdspace holds the gate — but it is a cross-tool write from a stop of an
   unrelated tool, and it is only safe because of the gate, not because of anything in this tool.

---

## What the spammer and Hold Space audit did not look at

The board side of the protocol (what `P`, `U`, `K/k`, `F/f` do in the firmware) is not traced here. The
spammer's planning half — `SpammerOrder.Runnable` / `Singles` / `For` and the preset migration — is
audited by its own tests, not here. And the config editor's half of the same settings (`Spammer` tab,
the card's rotation picker) is out of scope; only the run-time reading of those settings is covered.

## The shop tool — what it is

| Term | What it is |
|---|---|
| **The tool** | `ShopTool`, one class in two modes. `ShopMode.Buy` is the "Buy Items" card; `ShopMode.Sell` is "Sell Items". |
| **A run** | One `Run` call, which does exactly **one pass**, then stops — these are counted jobs, not loops. |
| **A pass** | `BuyPass` or `SellPass` — the list of transactions for this run. |
| **A transaction** | The shared gesture: RIGHT-click the target, then MAX, then Enter, then Enter. Identical for both modes. |
| **Buy preset** | `BuyPreset` — a name carrying (scroll notches, row index) and a count the run-time ignores. |
| **Shop region** | The calibrated rectangle round the visible shop list. Every row centre is *derived* from it, not marked. |
| **Row** | A derived row centre inside the shop region; row 0 is the top one. |
| **Bag grid** | The calibrated 8×8 region. The 64 slot centres derive from it. |
| **Slot index** | 0 = top-left, 63 = bottom-right, row-major. |
| **Focus point / scroll point** | The *same* config field, `BuySellConfig.ScrollPoint`. It is left-clicked to focus the game; Buy also wheels from it. Buy's `Ready` calls it the "scroll point", Sell's calls it the "focus point". |
| **MAX** | The count dialog's max-quantity button — the only click in a transaction with no keyboard equivalent. |
| **SellCap** | A hard ceiling on one sell run, enforced in the loop, not advice. |
| **The board** | The Arduino HID mouse, over serial. Placement is a verified closed loop; click, key and wheel are one open-loop write each. |

---

## Buy Items — `buy`

*One line: right-clicks one derived shop row up to `count` times, each time pressing MAX, Enter, Enter to buy the maximum quantity; it stops when the count is reached, or on a placement that fails, a quit hotkey, or the launcher's Stop.*

### The flow

```
  ●  Run() — ONE PASS, then stop: a counted job, never a loop of its own
  │
  1  arm the stop    hand the run's token to the base, so a sleep can throw
  │
  2  ready?          ◆ preset chosen · preset exists · rows usable · points marked?
  │                     no  ──► ✋ message on the card — nothing moves
  │                     yes ──► note whether the Start hotkey is already held
  │
  3  LOOP — the wait-for-start poll: a tick that sends nothing returns here (⟲);
  │          a tick that runs a pass breaks out at the foot
  │
  4  wait · watch    sleep 0.05, then
  │                  ◆ launcher Stop (the token cancels)? ──► ✋ THROW out of Run
  │                  ◆ quit hotkey held? ──► ✋ leave the loop
  │
  5  the edges       ◆ panel switched on?  ──► running on, "[Panel] BUY", beep
  │                  ◆ panel switched off? ──► running off, "[Panel] STOP", beep
  │                  ◆ Start hotkey rising edge? ──► flip running, "[GO]"/"[STOP]"
  │
  6  running?        ◆ no ──► nothing is sent ──────────► ⟲
  │                  ◆ yes ──► the pass, below
  │
  7  ONE PASS (BuyPass)
  │     a  focus     ▓ place on the focus point, ▓ LEFT-CLICK — give the game focus
  │     b  scroll    ◆ the preset asks for notches? ──► ▓ SCROLL DOWN "Z n", settle
  │     c  the row   ◆ the row is unusable? ──► ✋ message, the pass ends — no click
  │     d  buy ×count:                                    ← inner loop
  │           ◆ quit or cancel? ──► ✋ the pass ends
  │           ▓ place on the row, ▓ RIGHT-CLICK it
  │               ◆ the placement failed ──► ✋ message, the run ends
  │           ▓ place on MAX, ▓ LEFT-CLICK it, ▓ ENTER, ▓ ENTER
  │               ◆ a placement failed ──► ✋ message, the run ends
  │        ⟲ next transaction — the pass ends when the count is reached
  │
  8  finish          five beeps ("done"), then running off
  │
  └──► ✋ break the loop — the run ends by itself
  finally   running off · "Done." · return
```

**The stop path, in full.** A Stop is not one of the branches above — it arrives as a throw,
and the throw is what ends the run:

```
  launcher Stop ──► the token cancels ──► the next sleep THROWS
                                            │
                                            └─► the finally runs — the game is let go
                                                — and the throw ends the run
```

**Nothing is left held down and no window is left open:** the shop tool only ever *taps* — a HID
placement, a click, a right-click, an Enter, a wheel notch — so there is no press-and-hold to
release and no window to close. Its only teardown is the `finally`.

The board receives, in order per run: any number of HID `D dx dy` placements (each click, and the
focus click, drives one), then `C` (focus), optionally `Z n` (scroll), then per purchase `R`
(right-click), `C` (MAX) and `E`, `E` (the two Enters). There is no upward scroll: `Scroll`'s
`down:false` branch would send `Q n`, but the one call site passes `down:true`, so `Q` is never
sent by this class.

### Every action sent to the game

| # | Action | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | **HID move** `D dx dy` (closed-loop, 1..16 steps) | `PlaceOn` | before **every** click (focus, row, MAX) | no — it is the precondition for any click to be aimed |
| 2 | **Left-click** `C` | `FocusGame` | once at the start of every buy run | **yes** — the wheel and the right-click are both ignored while the game is unfocused, and starting the tool just unfocused it |
| 3 | **Scroll down** `Z n` | `Scroll`, from `ScrollFromTop` | once at the start, **only** when `preset.Scroll > 0` | no — positions the list so `preset.Row` means the right row |
| 4 | **Right-click** `R` | `ClickAt`, from `BuyPass` | once per purchase, on the derived row | no |
| 5 | **Left-click** `C` on MAX | `MaxEnterEnter` | after every right-click | no |
| 6 | **Enter** `E` (count dialog) | `MaxEnterEnter` | after MAX, **unconditionally** | **yes** — the dialog is expected; Enter on no dialog does nothing |
| 7 | **Enter** `E` (confirmation) | `MaxEnterEnter` | after the first Enter, **unconditionally** | **yes** — same reasoning |

**Side effect that is not game input:** `state.Cycle`, `state.Current` and `state.Message` on the
shared `ToolState` are written so the launcher card can show them. Nothing is written to disk.

### Decision points

| Where | Question | True → | False → |
|---|---|---|---|
| `Run` — before the loop | A precondition missing (`Ready`)? | message on the card, `return` — no pass | enter the wait-for-start loop |
| `Run` — loop top | Quit hotkey pressed, or token cancelled? | `break` (the `finally` clears `Running`) | fall through to the start edges |
| `Run` — panel on | Has the panel switched `state.Running` on? | `running=true`, "[Panel] BUY", beep 523 | test the switch-off edge |
| `Run` — panel off | Has the panel switched `state.Running` off? | `running=false`, "[Panel] STOP", beep 1000 | test the hotkey edge |
| `Run` — Start edge | Start hotkey a rising edge? | flip `running` **and** `state.Running`, "[GO]"/"[STOP]" | leave `running` alone |
| `Run` — not running | Not running? | `continue` — nothing is bought | run the one pass |
| `FocusGame` | Did the focus point resolve and the placement land? | click it and continue | `Stop`, `return` (the message was set inside the `Stop`) |
| `BuyPass` — the row | `RowCentre` returned null (region or row unusable)? | message, `return` — no click, **no `Stop`** | proceed to the purchase loop |
| `BuyPass` — purchase loop | Quit/cancel inside the purchase loop? | `return` — remaining purchases skipped, **no message** | do the next purchase |
| `ClickAt` — the row | Placement for the row right-click succeeded? | send the right-click | `Stop`, `return` |
| `MaxEnterEnter` — MAX | Placement for the MAX click succeeded? | click MAX, then the two Enters | `false` → `Stop`, `return` |

**Worth a verdict:**

1. **`_count` is transactions, and each transaction presses MAX — not "buy N units".** The gesture half
   (`MaxEnterEnter`) always clicks MAX before the Enters, so one transaction buys the dialog's
   *maximum* quantity, and the loop runs that transaction `_count` times. `BuyPreset.Count` —
   documented as "How many to buy per run" — is never read by `ShopTool` at all; the number comes from
   the run. So the config carries a count that does nothing, and the live count means something
   different from what its name suggests.

2. **A quit hotkey aborts the pass silently, then plays the same finish beep.** `BuyPass` returns on
   `QuitPressed` with no message, and `Run` still calls `BeepMany()` — the pattern that otherwise means
   "done". So an abandoned run and a completed run sound identical, and the card keeps whatever
   `Message` it last held.

3. **`ScrollFromTop` does not scroll to the top — and nothing checks the list is there.** Despite the
   name, it only applies the preset's scroll from wherever the list already is; the docstring is
   explicit that scroll-to-top was deliberately removed. The preset's row is measured from the top,
   so a list left scrolled by the player silently buys the wrong row. `Ready` cannot catch this,
   because nothing reads the list.

4. **The `RowCentre == null` branch is the odd one out.** It sets `state.Message` but does not call
   `Stop` and prints no `[!]` line, unlike every other failure in the file. It is harmless only
   because `Run`'s `finally` clears `Running` anyway, which makes the two failure paths behave the
   same for a reason the branch itself does not state.

5. **The Start hotkey is not honoured during a pass.** It is polled only in the outer loop, and the
   passes' `SleepCheck` watches only Quit and Pause. A pass is short, so the window is small — but a
   second Start press mid-pass is ignored, and the loop then breaks after that one pass regardless.

---

## Sell Items — `sell`

*One line: right-clicks each selected bag slot highest-index-first, each time pressing MAX, Enter, Enter to sell the whole stack; it stops when the slots are done, or on a failed placement, a quit hotkey, or the launcher's Stop.*

### The flow

**As Buy, except** the pass, the target, and the slot selection. The outer `Run` —
`WatchCancellation`, the wait-for-start loop, the panel and hotkey edges, the one-pass-then-stop,
the `BeepMany`, the `finally` — is **identical to the Buy diagram above**. Only what follows
differs:

```
  ●  Run() — as Buy: the same ready check, the same loop, the same stop path.
     Only the pass differs, and it is drawn in full below.

  1  ready?          ◆ bag grid valid · MAX marked · focus point marked ·
  │                     at least one slot selected · selection within the cap?
  │                     no ──► ✋ message naming what is missing — no pass
  │
  2  LOOP — the same body as Buy's: sleep 0.05, the quit/Stop checks, the panel
  │         and Start edges, then "not running ⟲ to the head / running → the pass".
  │         A launcher Stop still THROWS to the finally, exactly as at Buy.
  │
  3  ONE PASS (SellPass)
  │     a  the slots   keep indices inside the grid, drop duplicates, order
  │                   HIGHEST FIRST, then take at most SellCap
  │     b  focus       ▓ place on the focus point, ▓ LEFT-CLICK — give the game focus
  │                   (the same click as Buy's — but NO scroll ever follows)
  │     c  sell each slot:                                 ← inner loop
  │           ◆ quit or cancel? ──► ✋ the pass ends
  │           ▓ place in the slot, ▓ RIGHT-CLICK it
  │               ◆ the placement failed ──► ✋ message, the run ends
  │           ▓ place on MAX, ▓ LEFT-CLICK it, ▓ ENTER, ▓ ENTER
  │               ◆ a placement failed ──► ✋ message, the run ends
  │        ⟲ next slot — the pass ends when the slots run out
  │
  4  finish          five beeps ("done"), then running off
  │
  └──► ✋ break the loop — the run ends by itself
  finally   running off · "Done." · return
```

The stop path is the same as Buy's: a Stop cancels the token, the next sleep throws, the `finally`
lets the game go, and the throw ends the run. **Nothing is ever held down and no window is left
open** — this pass only taps, exactly as Buy does.

The board receives: HID `D dx dy` placements, one `C` focus, then per slot `R`, `C`, `E`, `E`.
**No scroll is ever sent** — Sell has no `Scroll`/`ScrollFromTop` call at all, the one structural
difference from Buy.

### Every action sent to the game

| # | Action | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | **HID move** `D dx dy` | `PlaceOn` | before every click | no |
| 2 | **Left-click** `C` | `FocusGame` | once at the start of the run | **yes** — without it the first right-click is ignored, and the game reports nothing, so it looks exactly like a wrong coordinate |
| 3 | **Right-click** `R` | `ClickAt`, from `SellPass` | once per slot, highest index first | no |
| 4 | **Left-click** `C` on MAX | `MaxEnterEnter` | after every right-click | no |
| 5 | **Enter** `E` (count dialog) | `MaxEnterEnter` | after MAX, unconditionally | **yes** |
| 6 | **Enter** `E` (confirmation) | `MaxEnterEnter` | after the first Enter, unconditionally | **yes** |

### Decision points

**As Buy, except** the preconditions and the slot selection replace the preset/row ones:

| Where | Question | True → | False → |
|---|---|---|---|
| `Ready` — sell branch | bag grid valid? MAX set? focus point set? | check the next | message naming what is missing, `return` |
| `Ready` — selection | At least one slot selected? | continue | "No slots are selected to sell." |
| `Ready` — the cap | `SellSlots.Count <= SellCap`? | continue | refuse, telling the player to raise the cap or narrow the selection |
| `SellPass` — the filter | Is a selected index inside `0 .. 63`? | keep it | **drop it silently** |
| `SellPass` — the filter | Duplicate indices? | collapsed by `Distinct` | — |
| `SellPass` — the cap | More than `SellCap` slots survive the filter? | `Take(SellCap)` — the highest indices win | sell them all |
| `SellPass` — focus | `FocusGame` succeeded? | enter the slot loop | `return` |
| `SellPass` — slot loop | Quit/cancel inside the slot loop? | `return`, no message | sell the next slot |
| `ClickAt` — the slot | Placement for the slot right-click succeeded? | send the right-click | `Stop`, `return` |
| `MaxEnterEnter` — MAX | Placement for the MAX click succeeded? | click MAX, then the two Enters | `false` → `Stop`, `return` |

**Worth a verdict:**

1. **`Ready` and `SellPass` disagree about which slots exist.** `Ready` compares the *raw* list length
   to the cap, while `SellPass` first drops out-of-range indices and duplicates and only then takes
   the cap. So a selection of 16 valid slots plus 1 stale out-of-range index is refused by `Ready`
   ("17 slots selected but the per-run cap is 16") even though the pass would sell exactly 16 — and
   duplicates can inflate `Ready`'s count the same way. The two checks are about different sets and
   can disagree in both directions.

2. **Highest-index-first is deliberate, and the reason is worth keeping written down.** Selling 0
   upward would skip items if the bag compacts after a sale (removing slot 0 shifts slot 1 into it);
   going highest-first is correct whether or not it compacts. This is the defensive choice, not a
   surprise — but it is invisible in a message, so it is only safe as long as the comment stays.

3. **The focus point is left-clicked, so it must be inert.** Sell sends a real left-click at
   `ScrollPoint` for focus only; the same point Buy wheels from. A point chosen over a list row would
   select or act on that row on every run. Because Sell never scrolls, the "inert" requirement is
   *only* about this click — which is easy to forget when the field is named and configured as a
   scroll point.

4. **The cap is enforced twice on purpose.** Once in `Ready` and again in the loop that destroys
   things. That is stated as intentional and not a defect, but it is the reason the two checks
   existing is acceptable rather than a redundancy to remove.

5. **The selection is not persisted, on purpose.** `BuySellConfig.SellSlots` is deliberately not
   saved — selling is the one irreversible thing in the suite, so it starts from an empty grid every
   launch. A selection carried over from a previous session is a selection nobody re-checked, so the
   tool reads only what is ticked now.

---

## What the shop audit did not look at

The two `Ready` branches share a shape but not their checks, and each is traced here only to its
`return` value. The placement primitive (`PlaceOn` → `HidPointer.To` → `D` / `WaitForCursorToSettle`)
is read only as far as the shop tool calls it, and is audited where it lives (`HidPointer.cs`). The
serial protocol letters (`C`, `R`, `E`, `Z`/`Q`, `D`) are named as the tool writes them; their
firmware behaviour — including whether `E` on no dialog is truly a no-op — is the board's to
guarantee, not this file's. The launcher wiring that supplies the preset, count and mode is out of
scope here.

## Quest Hand-in — `quest`

*Replays a fixed, player-built flow of clicks and keypresses at the quest NPC, a set number of times,
and ends by itself when the loops run out — or when the mouse moves past the guard, or when it is
stopped.*

### What it is

| Term | What it is |
|---|---|
| **Flow** | One named list of steps, with its own loop count and its own initial wait. `QuestPreset`, run by `RunFlow`. |
| **Sequence** | A named list of flows, each with a repeat count, wrapped in a **master loop**. Run by `RunSequence`. |
| **Step** | One action the board sends: click, right-click, Enter, key, or wait. Sent by `Send`. |
| **The positioning window** | The opening countdown; the player puts the mouse where the clicks must land, and the tool never moves it again. `Position`. |
| **The guard** | The cursor read once at the start; if it moves more than `GuardPx`, the run stops. `TryReadGuardReference`, `GuardProblem`. |
| **Refuse** | A pre-flight validation failure — nothing has been clicked yet. `Validate` → `Refuse`. |
| **`Finish`** | Clears the standing card lines on every exit; leaves `Message` alone. |

The tool **reads nothing** — no capture, no OCR, no window lookup — which is why it has no calibration
at all. A wrong flow does `Loops` wrong actions and the game accepts every one of them; the only thing
that can notice is the player, watching one test loop.

### The flow

```
  ●  Quest Hand-in — a bounded replay of a flow the player built
  │
  1  choose         what to run: the card's choice outranks the tab's
  │                   ◆ did the card name a sequence or a flow?
  │                       a missing name ──► ✋ refuse, reason left on the card
  │                   otherwise the tab's — its sequence if one is chosen,
  │                   else the single active flow
  │
  2  validate       ◆ a flow with no steps, or a step the board can't send?
  │                   yes ──► ✋ refuse — NOTHING has been clicked yet
  │
  3  announce       the plan goes to the card; every step goes to logs/quest.log
  │
  4  position       count the initial wait down in whole seconds, out loud
  │                   stopped or quit during it ──► ✋ cancelled
  │
  5  read the guard ◆ is the cursor readable?
  │                   no  ──► ✋ stop — it fails CLOSED, never clicks unwatched
  │                   yes ──► remember the start point; drift past GuardPx stops
  │
  6  THE LOOP       bounded — the one thing that lets a run end BY ITSELF
  ┏ 6a  ◆ stopped or quit before the step? ────► ✋ cancelled
  ┃ 6b  ◆ cursor drifted past GuardPx? ───────► ✋ guard stop
  ┃ 6c  send the step    ▓  click / right-click / Enter / one key
  ┃ 6d  wait out the step's delay, watching for a stop
  ┃       └ 6c–6d repeat for EVERY step in the pass
  ┃ 6e  ◆ the pass came back Done, and any loops remain?
  ┃        yes ──► run the next pass
  ┃        no  ──► ✋ DONE — the run ends by itself
  ┗──⟲  the body ends; control returns to the head for the next pass
  │
  │    a SEQUENCE is a master loop round its flows, each flow run its own
  │    count of times; a single FLOW is just its own passes. Either way the
  │    loop is counted, so exhaustion is a normal ending, not an error.
  │
  └──► teardown — the plan clears off the card, then the run is torn down.
       Quest is the ONLY tool that ends on its own, so this is the one exit the
       launcher has to tell from a stop: it compares the run's own token (§1.3).
```

**The stop path is explicit in three places, and all three end the same way.** A stop arrives either
as a cancelled token or as the `QuitPressed` flag, and both are checked in `RunSteps` *before every
send* and inside `Wait` *before every tick*; the positioning countdown is the third, so a stop there
does not have to wait out the seconds. Each returns `Cancelled`, which logs a line and calls `Finish`.
A stop also cancels the token that the base class's `SleepCheck` watches, so a stop that lands mid-sleep
**throws**, unwinding the run — cleanup is the launcher worker's `finally`, and §1.2's rule holds: a stop
abandons the WORK, never the CLEANUP.

**Quest is the suite's only tool that ends on its own.** There is no endless loop: the two are counted —
one over a flow's passes, one over a sequence's master rounds. When the last pass is `Done`, `Finish`
clears the card and `Run` returns 0. A guard stop and a refusal tear the run down the same way. The tool
never sets `ToolState.Running`; the launcher does, and tears it down when `Run` returns.

**Where it writes to the board:** only in `Send`, and only in the loop. There is no cursor move anywhere
in the tool — the clicks land wherever the player left the mouse.

### Every action sent to the game

This is the complete inventory. There are **no defensive actions**: every write is a step the player
built, and nothing here fires to clear a dialog or guard a state.

| # | Action | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | Left **click** — board `C` | `Send` | once per click step, every pass | no |
| 2 | **Right-click** — board `R` | `Send` | once per right-click step, every pass | no |
| 3 | **Enter** — board `E` | `Send` | once per enter step, every pass | no |
| 4 | **Key** — board `K <char>` | `Send` | once per key step; exactly one printable character | no |

A **wait** step sends nothing at all — it falls through `Send`'s default branch.

**Side effects that are not game input:** `Log` appends to `logs/quest.log` beside the launcher, and to
the console. The file is the durable record, because the published WinExe has no console and a
self-ended run otherwise looks exactly like a stopped one. The tool also writes the card's standing
lines (`Schedule`, `Current`, `Message`).

### Decision points

| Where | Question | True → | False → |
|---|---|---|---|
| `Run` | Card named a sequence? | look it up; a missing name → Refuse | check for a card flow |
| `Run` | Card named a flow? | look it up; a missing name → Refuse | fall through to the tab |
| `Run` | A tab sequence is chosen? | `RunSequence` | `RunFlow` on the tab's single flow |
| `Validate` | Any step complaint — none, or unsendable? | Refuse, nothing clicked | run |
| `Position` | Cancelled or quit during the countdown? | Cancelled → Finish | read the guard |
| `TryReadGuardReference` | Cursor readable? | remember the start point, run | Finish → stop (fails closed) |
| `RunSteps` | Cancelled or quit, before a step? | Cancelled | check the guard |
| `GuardProblem` | Cursor moved more than `GuardPx`? | StoppedByGuard → Finish | `Send` the step |
| `Wait` | Cancelled or quit inside a step's delay? | Cancelled | next step |

**Worth a verdict:**

1. **There are no defensive actions, and the tool reads nothing.** Every write is a configured step, so
   a wrong flow does `Loops` wrong actions and the game accepts every one of them — the trade stated in
   the class comment. The cursor guard is the only safety net.

2. **The guard reads the cursor once and fails CLOSED** — an unreadable cursor ends the run. That is the
   deliberate opposite of the tuner's guard, which fails *open* because it needs a game window it may
   not be able to measure.

3. **Three ways to end all return 0 and all leave the card reading "stopped".** Done, StoppedByGuard and
   Cancelled are told apart only by `Message` and by `logs/quest.log`. The log exists precisely because
   of that.

4. **`Finish` clears `Schedule` and `Current` but leaves `Message`** on purpose — the guard stop's
   sentence and a refusal are the one thing left to read afterwards.

5. **Validation and the guard cover different windows and nothing else does.** `Refuse` checks every
   step before the mouse moves; the guard can only notice a hand that moved after the wait. A step that
   is valid but pointed at the wrong pixel is caught by neither.

6. **"Test one loop" runs the identical code** — the guard, the validation and the sending are not
   re-implemented beside it. Worth confirming that stays true as the tool grows.

---

## Pet Feeder — `pet`

*Keeps each ticked breeding row fed on its own per-row clock by reloading the feeder before the pet
runs out; it loops for days and stops only on its own Stop — it is the one tool that ignores the quit
hotkey.*

### The flow, at the top level

```
  ●  Pet Feeder — RESIDENT. Another tool's Start does NOT stop it, and its own
     Start stops only a previous pet. It needs the game for about half a minute
     at a time, five times a day, so it steps aside rather than being stopped.
  │
  1  ready          ◆ any precondition missing?
  │                   yes ──► ✋ stop, the missing thing named on the card
  │
  2  load the table  pet-data.csv. A missing or unreadable table costs the
  │                  COMPUTED schedule and nothing else — the pets still feed.
  │
  3  CLAIM the game  for the look. ◆ is it free?
  │                   no  ──► ANNOUNCE that it is waiting and SAY who for, then
  │                          poll until free — a holder that yields (the
  │                          spammer) stands down and hands the game over
  │                   cancelled while waiting ──► ✋ "Stopped before it could
  │                                               read the rows."
  │
  4  THE LOOK       open the breeder ONCE, read every row, close ONCE   ▓
  │                   └ the game is given back in a finally, however it goes
  │
  5  schedule       failures = 0 per row · publish the standing card line
  │
  6  THE LOOP       runs for DAYS — nothing but its own Stop ends it, because
  │                 the quit hotkey is ignored for this tool
  ┏ 6a  ◆ any live rows left? ── no ──► ✋ "Every row has failed repeatedly"
  ┃ 6b  sleep to the soonest row's next reload (a stop here just breaks out)
  ┃ 6c  take EVERY row due now, in ROW ORDER (none due → just the soonest)
  ┃ 6d  CLAIM the game for the visit — waiting again if another tool holds it.
  ┃       The row's next is left in the PAST, so the same row is picked again
  ┃       after the wait, rather than skipped.
  ┃ 6e  THE VISIT — open ONCE · read every row · reload each due row ·   ▓
  ┃       close ONCE. The whole reload is board writes (detail: AUDIT-PET-FLOW).
  ┃       └ cut by a stop ──► note "it may not have been fed — check it in
  ┃          game", give the game back, then rethrow
  ┃ 6f  fold the results back, per row:
  ┃        Ok              ──► next load when the pet or the food runs out,
  ┃                            whichever comes first, plus the grace period
  ┃        NothingToBoard  ──► look again in 30 min — a WAIT, not a failure
  ┃        Failed          ──► retry in 5 min; after 3 tries, DROP that row
  ┃ 6g  publish the schedule · beep (523 if every row was Ok, else 200)
  ┗──⟲  back to the head — and it stays here for days
  │
  └──► ✋ teardown — the game is given back and Running falls. Its own Stop is
       the only ending; the quit hotkey is ignored on purpose.
```

**A stop cancels the token, and the tool's next sleep THROWS.** If that lands inside a visit, the throw
is caught only long enough to leave the player a note — *"it may not have been fed, check it in game"* —
and is then rethrown; the game goes back in a `finally` on that path too, and the loop's own `finally`
clears `Running` before the run ends. That is §1.2's rule, and here it matters most: a drag cut between
its press and its release leaves the stack where the cursor was, and this process cannot put that back.

**Why it can be resident.** The gate is taken **per look and per visit, and never held in between** —
while it waits it holds nothing, which is exactly why another tool's Start does not have to stop it. Its
own Start stops only a previous pet. And it is never a silent wait: it announces itself as waiting and
names the holder, because a stalled feeder and a broken one look identical otherwise.

**Where it writes to the board.** Only inside the two touch-points above — the look (the 目錄 menu, the
feed icon, one Enter, the X) and the visit (the whole reload) — and nothing at all in between, where it
only sleeps. The complete action inventory is in `AUDIT-PET-FLOW.md` §2.

### Decision points (the outer loop only)

| Where | Question | True → | False → |
|---|---|---|---|
| `Ready` | Any precondition missing? | stop with a message naming it | run |
| `ClaimGame` | Game free, for the look? | `InspectRows` | wait, saying who; cancelled → end the run |
| loop | Any live rows left? | sleep to the soonest | "Every row has failed repeatedly" → break |
| `SleepUntil` | Cancelled during the wait? | break | compute the due batch |
| batch | Does the batch hold any row? | visit them | `batch = [soonest]` |
| `ClaimGame` | Game free, for the visit? | `Visit` | wait; `next[row]` stays in the past; cancelled → break |
| `Visit` | Was the visit cut by a stop? | throw — "may not have been fed. Check it in game." | fold the results back |
| fold | The row's reload was **Ok**? | `next = now + min(pet, load) + WaitAfterEmpty`; failures = 0 | check next |
| fold | **NothingToBoard**? | `next = now + 30 min`; not a failure | Failed |
| fold | Failures reached `MaxFailures` (3)? | **drop that row**; the others carry on | `next = now + 5 min` |

### In-visit detail lives elsewhere

**[AUDIT-PET-FLOW.md](../AUDIT-PET-FLOW.md)** is this section's in-visit companion and covers everything
between `OpenBoarding` and `CloseBoarding`, which the diagram above deliberately collapses into one box:

- **§1 the cast**, **§2 the complete action inventory** — every click, Enter, hover, right-click, drag,
  MAX and cursor-park, and which of them are defensive.
- **§3 run start**, **§4 the main loop pass**, **§5 one visit**, **§6 one row's reload** — the
  end → place → load → start order and the three unconditional Enters it sends.
- **§7 placing the pet** — the return-slot-first order and the +9/100% guard.
- **§8 loading the food** — drag vs right-click and the bag-cell check.
- **§9 the one scheduling rule** (`ScheduleFor`), **§10 every decision point**, and **§11 its own
  Worth-a-verdict list**.

**Worth a verdict:**

1. **A stop mid-visit cannot repair the game** — a drag cut between its press and its release leaves the
   stack where the cursor was, so the tool hands the player a note rather than fixing it. The schedule is
   never left wrong (a restart re-inspects), but the game may be.

2. **The gate is taken per look and per visit, never held in between** — this is what makes the feeder
   resident. But the deferral leaves `next[row]` in the past, so a foreground tool that holds the game
   for a long time defers the same row over and over.

3. **A row is dropped after 3 failures**, so one bad calibration costs that row only. Worth confirming 3
   is the count you want for something that runs unattended for days.

4. **`NothingToBoard` is a 30-minute wait, not a failure** — a bag holding no feedable pet keeps the
   row's place and is never counted against it.

5. **The quit hotkey is disabled for this tool**, so its own Stop is the only way to end it. That is
   deliberate (one press must not kill a schedule that is feeding four pets) but it means the global
   hotkey is not a panic button for the feeder.

6. **`Ready` checks every ticked row, not just the first**, so a switched-off half-set-up row no longer
   blocks a run that never touches it.

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
  launcher Start(id)
        │
        ├─ stop whatever this start displaces          StartToolCoreAsync  LauncherService.cs
        │     (starting anything but the pet stops the rest, EXCEPT ResidentId = "pet")
        │
        ├─ foreground only: WaitForGameAsync(id) ──► Gate.TryAcquire(id)
        │                                            ├─ free  → claim it, continue
        │                                            └─ held  → wait, bounded, and SAY who it waits for
        │
        ├─ ArduinoPortAsync()  ──► PortChoice.Choose → open (2 s boot wait on a cold start)
        │
        └─ Task.Run(() => RunTool(id, ser, state, ct))
                              │
                              └─ new <Tool>(…).Run(ser, state, ct)
                                       └─ WatchCancellation(ct)     ← the tool hands its token to the base
```

### 1.2 How a run stops — the part worth reading twice

```
  Stop pressed
        │
        ├─ StopTool: deregister from the registry FIRST, then
        │     if (HeldKeys.NeedsReleaseOnStop(id)) ReleaseHeld()   ← writes "U\n" and "l\n"
        │     cts.Cancel()
        │
        ▼
  the tool's next SleepCheck
        │
        ├─ token cancelled ──► THROW OperationCanceledException
        │                        │
        │                        └─ unwinds the step IN PROGRESS — the whole point: a pet visit is
        │                           minutes long, and a stop honoured only at a visit boundary looks
        │                           exactly like a broken button
        │
        └─ otherwise ──► sleep, and check the quit/pause hotkeys
        │
        ▼
  the tool's own finally blocks run (release the game, release a held key, close a window)
        │
        ▼
  the worker catches it:   catch (OperationCanceledException) when (ct.IsCancellationRequested)
        │                   — reported and logged as NOTHING. A stop is not a crash.
        │
        └─ finally: deregister, ReleaseHeld() if HeldKeys, dispose the CTS, release the gate
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
| **Attempt** | One C-then-E cycle: one click on 發條, one OCR scan of the result panel (`SealTuner.cs:121`, `:134`). |
| **Countdown** | Five one-second ticks after a start, before the first attempt (`:66`, `:88-116`). |
| **C / E** | The Arduino commands sent per attempt — `"C\n"` is a left-click, `"E\n"` is Enter (`:137`, `:140`). |
| **Signature / `prevSig`** | `(grade, remaining, joined attributes)` — the key compared to spot a repeating result (`:51`, `:190`). |
| **`consecutiveRepeats`** | How many times the last signature has repeated; `>= 2` means three identical reads in a row (`:53`, `:247`). |
| **Grade order** | `tuner.grade_order`; a grade's rank is its `IndexOf` there (`:216`, `:277-278`). |
| **Filter** | An attribute filter (`tuner.filter`); its `Passed` result is `filterPass` (`:164-165`). |
| **`requireGrade`** | The filter's own grade requirement; with the target it forms `effectiveGrade` (`:212-214`). |
| **`spring_mode`** | `"hid"` makes the tool place the cursor on 發條 and arm the guard; anything else skips both (`:104`, `:127`). |
| **`spring_point`** | The calibrated 發條 position, `[x, y]` (`:284`). |
| **`mouse_guard`** | `"off"`, `"stop"` or `"recenter"` — how a drifting cursor is treated (`:301`, `:324`, `:331`). |
| **Recenter** | Guard mode `"recenter"` re-places the cursor; after `recenter_max` put-backs it stops (`:331-339`). |
| **The gate** | `PortGate`; claimed under the id `tuner` by the launcher, not by this loop (`LauncherService.cs:600`). |

### The flow

```
Run(ser, state, ct)                                                          SealTuner.cs:32
 │ WatchCancellation(ct)  — makes SleepCheck able to throw the stop            :34
 │ open logs/run_<ts>.jsonl + .txt, build OcrEngine                           :35-41
 │ running=false; countdown=0; attempt=0; f12Was=IsDown(Start)                 :46-53
 │
 └─ while (true)                                                              :57
     │ SleepCheck(0.05); quit(F11) or token cancelled? ─────────────────────► BREAK :59-60
     │
     ├─ panel said START (state.Running && !running)?                          :63
     │      running=true; countdown=5; "[Panel] START"  (no beep — panel does it) :65-70
     ├─ panel said STOP  (!state.Running && running)?                          :71
     │      running=false; "[Panel] STOP"; Beep(1000)                          :73-75
     │
     ├─ F12 rising edge (and not quitting)?                                    :79
     │      toggle running; state.Running=running                              :81-82
     │      on  → "[GO]"; countdown=5; Beep(523)                               :83
     │      off → "[STOP]"; countdown=0; Beep(1000)                            :84
     │
     ├─ countdown > 0? ───────────────────────────────────────────────┐        :88
     │      print n...; inner loop 20 x 50 ms:                        │        :90-96
     │        F12 down        → running=false, state.Running=false ───┤ ABORT   :94
     │        quit / cancel   → break the inner loop ─────────────────┤        :95
     │      countdown--;                                               │        :97
     │      at 0: "[>] RUNNING"; Beep(1500)                           │        :100-101
     │        spring_mode=="hid"? → PlaceOnSpring(ser)  (HID)         │        :104-106
     │            returned an error → message on card, STOP ──────────┼──►★    :107-112
     │      continue                                                   │        :116
     │                                                                 │
     ├─ !running? ────────────────────────────────────────────────────►┘ continue :119
     │
     ├─ attempt++; state.Attempt=attempt                                      :121-122
     ├─ spring_mode=="hid" && CheckMouseGuard(...) == stop? ──────────► STOP  :127-132
     │      (recenter mode may send a HID re-placement here — see §guard)
     │
     ├─ try:  ser.Write("C\n")   ← CLICK                                      :137
     │        SleepCheck(ClickEnterDelay); quit/cancel? break                 :138-139
     │        ser.Write("E\n")   ← ENTER                                      :140
     │        SleepCheck(OcrDelay)                                            :141
     │   catch (any exception EXCEPT cancellation):                           :149
     │        "[!] Arduino disconnected — stopping"; STOP  ──────────────────►★ :151-154
     │   (a real cancellation is deliberately NOT caught — see verdicts)
     │
     ├─ result = ocr.Scan(); match attributes; CheckFilter                    :159-165
     ├─ write the attempt to run_<ts>.jsonl / .txt                            :168-176
     ├─ grade, remaining and attributes ALL empty? ──────────────────────────►★ STOP :182-187
     ├─ sig == prevSig ? consecutiveRepeats++ : consecutiveRepeats=0; prevSig=sig :190-193
     ├─ print result; state.Grade/Remaining/Attributes/FilterStatus           :195-205
     ├─ SleepCheck(0.3); quit/cancel? break                                   :207-208
     │
     └─ STOP CONDITIONS (in this order):                                     :210-265
        gradeOk && filterOk        → SUCCESS; BeepMany ─────────────────────► STOP :218-224
        filter on, no requireGrade, filterPass → MATCH; BeepMany ───────────► STOP :229-235
        gradeOk && !filterPass     → log "continuing"                             :236-237
        filterPass && !gradeOk && requireGrade != null → "continuing"             :238-239
        remaining <= 0             → OUT OF SPRINGS ────────────────────────► STOP :241-246
        consecutiveRepeats >= 2    → STUCK (same result x3) ────────────────► STOP :247-252
        attempt >= max_retries     → "[!] Max N" ───────────────────────────► STOP :253-258
        PauseRequested             → "[PAUSE] graceful stop" ───────────────► STOP :259-265
        (else → loop again)                                                       :266

 finally → state.Running = false                                             :268-271
 print "Done. N attempts."; return 0                                         :273-274
```

`★` marks a stop whose reason is written to `state.Message` for the card. **Nothing is left held down
and no window is left open:** the tuner uses only self-contained primitives — a left-click
(`HidPointer.cs:185`), an Enter sent directly as `"E\n"` (`SealTuner.cs:140`), and HID relative moves
(`HidPointer.cs:189`) — it never presses-and-holds (`LeftDown`/`LeftUp`, `HidPointer.cs:208`), so
there is no drag to release and no window it opens. Its only teardown is the `finally` at `:268-271`.
What persists after a stop is the cursor's physical position, wherever the last HID move left it.

### Every action sent to the game

The tuner sends exactly three kinds of input: HID moves, a click, and an Enter. **It sends no keyboard
input other than the Enter, and no right-click.**

| # | Action | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | HID move `D dx dy` onto 發條 (one or more, closed-loop) | `PlaceOnSpring` `:292-293` via `HidPointer.To` `:74` | once per start, when `spring_mode=="hid"` and the countdown reaches 0 (`:104-106`) | no — the precondition for the click landing on the button |
| 2 | HID move `D dx dy` re-placing the cursor on 發條 | `CheckMouseGuard` `:332` via `HidPointer.To` | only with `mouse_guard=="recenter"`, when the cursor has drifted (`:320`, `:331`) | **yes** — a corrective re-placement |
| 3 | Click `"C\n"` | `:137` | every attempt, before the scan | no |
| 4 | Enter `"E\n"` | `:140` | every attempt, after the click and `ClickEnterDelay` | no |

**Side effects that are not game input:** the run log is written every attempt — `run_<ts>.jsonl`
(serialised) and `run_<ts>.txt` (one line) (`:168-176`) — and `state` is refreshed each attempt for the
card (`:202-205`).

### Decision points

| Where | Question | True → | False → |
|---|---|---|---|
| `:60` | Quit hotkey (F11) or token cancelled? | leave the loop | tick on |
| `:63` | Panel asked START and we are not running? | running, countdown=5, "[Panel] START" (`:65-67`) | fall through |
| `:71` | Panel asked STOP and we are running? | running=false, Beep(1000) (`:73-75`) | fall through |
| `:79` | F12 rising edge (and not quitting)? | on → "[GO]", countdown=5, Beep(523) (`:83`); off → "[STOP]", countdown=0 (`:84`) | `f12Was=f12Now` (`:86`) |
| `:88` | countdown > 0? | print a tick and run the 20×50 ms inner loop (`:90-96`) | go to the attempt |
| `:94` | F12 down inside the countdown? | abort the countdown, running=false (`:94`) | keep counting |
| `:95` | quit or cancel inside the countdown? | break the inner loop (`:95`) | keep counting |
| `:98` | the countdown just reached 0? | "[>] RUNNING", Beep(1500), hid placement (`:100-106`) | loop back |
| `:107` | `PlaceOnSpring` returned an error? | message on card, running=false, stop (`:109-112`) | carry on |
| `:119` | not running? | continue, no attempt (`:119`) | run an attempt |
| `:127` | `spring_mode=="hid"` and the guard says stop? | running=false, break (`:129-131`) | attempt proceeds |
| `CheckMouseGuard:302` | `mouse_guard == "off"`? | never stop — return false | judge drift |
| `:305` | `spring_point` not calibrated? | the guard can't judge — false | read the window |
| `:313` | game window missing or minimised? | **fail open** — false ("no drift") | read the cursor |
| `:317` | cursor position unreadable? | **fail open** — false | compute the drift |
| `:320` | drift > `guard_px`? | stop (`:324-329`) or re-place (`:331-340`) | `recenterCount=0`, false (`:322`) |
| `:334` | recenter failed or `recenterCount > recenter_max`? | stop with a message (`:336-338`) | log "[recenter #n]", false (`:340`) |
| `:149` | a C/E write threw (and is not a cancellation)? | "[!] Arduino disconnected", stop (`:151-154`) | continue |
| `:182` | grade, remaining and attributes all empty? | "READ FAILED" message, stop (`:184-187`) | keep the result |
| `:191` | signature equals the previous one? | `consecutiveRepeats++` (`:191`) | `consecutiveRepeats=0` (`:192`) |
| `:218` | `gradeOk` AND `filterOk`? | SUCCESS — BeepMany, stop (`:220-224`) | next check |
| `:229` | filter enabled, no `requireGrade`, `filterPass`? | filter matched — BeepMany, stop (`:231-235`) | next check |
| `:236` | `gradeOk` but the filter did not pass? | log "continuing" (`:237`) | next check |
| `:238` | filter passed but the grade is below `requireGrade`? | log "continuing" (`:239`) | next check |
| `:241` | `remaining <= 0`? | OUT OF SPRINGS, stop (`:243-245`) | next check |
| `:247` | `consecutiveRepeats >= 2`? | STUCK, stop (`:249-251`) | next check |
| `:253` | `attempt >= max_retries`? | "[!] Max N", stop (`:255-257`) | next check |
| `:259` | `PauseRequested`? | "[PAUSE] graceful stop" (`:261-264`) | loop again |

**Worth a verdict:**

1. **A non-`hid` `spring_mode` clicks blind.** Both the placement (`:104`) and the guard (`:127`) are
   gated on `SpringMode == "hid"`, so with any other value the tool sends C/E wherever the cursor
   happens to sit, with no placement and no drift check. If the C command still lands only on 發條 when
   the cursor is somewhere else, that is the one thing to confirm.

2. **The mouse guard fails open.** A missing/minimised game window (`:313`) or an unreadable cursor
   (`:317`) returns false — "no drift" — so the loop keeps clicking instead of tripping. The code calls
   this an acknowledged fail-open rather than a bug (`:306-312`); it is worth a verdict because the
   guard is the only thing protecting the click target in `hid` mode.

3. **A panel STOP does not abort a countdown that is already running.** The panel-STOP branch clears
   `running` but not `countdown` (`:71-75`), and only F12 aborts the countdown (`:94`); the countdown
   therefore runs to 0 and may still call `PlaceOnSpring` and cross the cursor (`:98-106`).

4. **The attempt counter survives a stop/start.** `attempt` is not reset when F12 re-starts a run
   (`:79-85`, `:121`), so toggling start/stop within one `Run` keeps driving it toward `max_retries`
   (`:253`).

5. **The "Arduino disconnected" catch covers only C and E.** `PlaceOnSpring` (`:106`) and the guard's
   re-placement (`:332`) write to the port *outside* the try at `:135`, so a write fault there throws
   out of `Run` with the friendly message never shown (`:151`).

6. **Stop reasons are uneven on the card.** Only the all-empty read (`:185`) and the guard stops
   (`:328`, `:336-337`) set `state.Message`; SUCCESS, OUT OF SPRINGS, STUCK and MAX stop silently
   (`:220`, `:243`, `:249`, `:255`) — and their `Console.WriteLine` is invisible in the WinExe.

7. **The all-empty read is special-cased before the repeat counter** (`:178-188`) precisely so a total
   OCR failure cannot count as the first of three identical reads; that ordering is load-bearing, not
   incidental.

---

## Gem Composer — `gem`

*Selects a gem grade, moves to Register, clicks to combine and re-registers, and on an empty result box
(or a cleared resource slot) advances N → G → DG once; it stops on request, on a failed
placement/route, when the last grade is done, or when something is not calibrated.*

### What it is

| Term | What it is |
|---|---|
| **Cycle** | One combine: Move Register→Combine, click, empty check, Move Combine→Register, then either the deregister+register clicks or an advance (`GemComposer.cs:416`, `:431-467`). |
| **Grade** | One of `gem.grades` (e.g. `N`/`G`/`DG`); the current one is `grades[gidx]`, starting at `gem.start_grade` (`:58-59`). |
| **Register** | The register/combine button; routes named `register_combine`, `combine_register`, `radio_<grade>`. |
| **Route** | One composer movement. `gem.move_mode=="arduino"` places the cursor on the route's destination point; otherwise it sends the hand-tuned counts in `gem.movements` (`:129-144`). |
| **Point** | A calibrated named position resolved by `GemRoutes.Resolve` (`:106`). |
| **Empty result box** | The composed gem slot showing no gem — `IsResultBoxEmpty` (`:191`). |
| **Diff fraction / `empty_distance`** | Fraction of pixels differing from the saved empty crop; `<= gem.empty_distance` reads as empty (`:213-219`). |
| **Empty signature** | The fallback colour test when the crop is missing (`:221-223`). |
| **`empty_streak` / `empty_count`** | Consecutive empty results needed before advancing (`:62`, `:443`). |
| **`empty_mode`** | `"stop"` (no auto-advance) or `"advance_grade_clear"` (clear the resource slots first) (`:304`, `:438`). |
| **The gate** | `PortGate`; claimed under the id `gem` by the launcher, not by this loop (`LauncherService.cs:600`). |

### The flow

```
Run(ser, state, ct)                                                          GemComposer.cs:55
 │ WatchCancellation(ct)                                                       :57
 │ grades=gem.grades; gidx=index of start_grade                                :58-59
 │ helpers: Fail / TryPoint / TryMove / PlaceAt / Route /                      :76-144
 │          SelectGradeAndRegister / IsResultBoxEmpty / ClearResources /
 │          Slot3ToGrade / Slot3RouteKey / AdvanceGrade
 │ state.Cycle=0; state.Grade=grades[gidx]                                     :69-70
 │
 └─ while (true)                                                              :366
     │ SleepCheck(0.05); quit(F11) or token cancelled? ─────────────────────► BREAK :368-369
     │
     ├─ panel said START (state.Running && !running)?                          :372
     │      running=true; f12Was=true; Beep(523);                              :374-377
     │      ★ SelectGradeAndRegister()  ← the game is clicked here            :378
     ├─ panel said STOP  → running=false; Beep(1000)                          :380-385
     │
     ├─ F12 rising edge?                                                       :390
     │      toggle running; if on: Beep(523) + ★ SelectGradeAndRegister()      :393-399
     ├─ F9 rising edge?                                                        :403
     │      gidx=(gidx+1) % count  (WRAPS, unlike AdvanceGrade);               :405-407
     │      ★ SelectGradeAndRegister()                                         :408
     │
     ├─ !running? ───────────────────────────────────────────────────► continue :414
     │
     ├─ cycle++; state.Cycle=cycle                                             :416-417
     ├─ quit / cancel? break                                                   :419
     ├─ F12 held at the top of the cycle? → f12Was=true; running=false; continue :420-428
     │
     ├─ Route Register→Combine ── failed? ───────────────────────────► BREAK   :431
     ├─ Click "C"  ← combine                                                   :433
     │
     ├─ empty_mode != "stop" AND empty_signature set?                          :438
     │      IsResultBoxEmpty():
     │        game window is the FOREGROUND window?                           :202
     │           no → "refused: not foreground", answer NOT empty ────────────► :204
     │        capture result region (CopyFromScreen) ── null → NOT empty       :208-209
     │        empty crop reference present? → pixel-diff <= empty_distance     :213-219
     │           else empty_signature present? → colour-signature test         :221-223
     │           else → NOT empty  (never advance on a missing reference)      :226
     │      empty → emptyCount++; >= empty_streak → advanceNow=true, count=0   :441-447
     │      not empty → emptyCount=0                                           :449
     │
     ├─ quit / cancel? break                                                   :453
     ├─ Route Combine→Register ── failed? ──────────────────────────► BREAK    :454
     │
     ├─ advanceNow? ── yes ─► AdvanceGrade() ── false ──────────────► BREAK    :457-460
     │                            │  (false = last grade done, or a step failed)
     │                            └─ true → continue                          :460
     │
     └─ normal path:
        Click "C"   ← deregister                                              :464
        Click "C"   ← register                                                :466
        cycle % 10 == 0 → print; PauseRequested? → "[PAUSE] graceful stop" ──► BREAK :469-476
        (loop)

 AdvanceGrade()                                                               :302
   ├─ empty_mode=="advance_grade_clear" → ClearResources():                   :304
   │      Route Register→Resource1; RightClick "R"                            :264-266
   │      Route Resource1→Resource2; RightClick "R"                           :268-270
   │      Route Resource2→Resource3; RightClick "R"                           :272-274
   ├─ gidx+1 >= grades.Count? → "[DONE] last grade"; Beep(880); STOP ────────► :307-314
   ├─ gidx++; state.Grade=grades[gidx]                                        :317-318
   ├─ clear mode → Route Resource3→next grade; Click "C"                      :324-332
   │     else      → place the grade button (HID); Click "C"                  :337-352
   └─ Route next grade→Register; Click "C"                                    :356-360

 finally → state.Running = false                                              :479-482
 print "Done. N cycles."; return 0                                            :484-485
```

`★` marks a click sent while selecting a grade — these fire on start, on F12 and on F9 even before any
combine has run. **Nothing is left held down and no window is left open:** the tool uses only
self-contained primitives — a click (`HidPointer.cs:185`), a right-click (`HidPointer.cs:192`), and HID
moves (`HidPointer.cs:189`, and the closed-loop placement in `HidPointer.To`) — it never uses
`LeftDown`/`LeftUp` (`HidPointer.cs:208`), so there is no drag to release and no window it opens. Its
only teardown is the `finally` at `:479-482`. What persists after a stop is the cursor's physical
position.

### Every action sent to the game

The composer sends exactly three kinds of input: HID moves, clicks, and right-clicks. **It sends no
keyboard input at all** (no Enter, unlike the tuner).

| # | Action | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | HID placement onto a point (one or more `D dx dy`) | `HidPointer.To` via `PlaceAt:117`, `SelectGradeAndRegister:165`, `AdvanceGrade:344` | on start, on F9, and on every `arduino` route | no |
| 2 | Click `"C"` on the grade button | `SelectGradeAndRegister:172` | on start / F12 / F9, and on each advance | no |
| 3 | Move grade→Register (`D` tuned `:142`, or placement `:138`) | `Route` from `:174` | after the grade click, every (re)select | no |
| 4 | Click `"C"` (select Register) | `:177` | after the grade→Register move | no |
| 5 | Move Register→Combine | `Route` `:431` | every cycle | no |
| 6 | Click `"C"` (combine) | `:433` | every cycle | no |
| 7 | Move Combine→Register | `Route` `:454` | every cycle | no |
| 8 | Click `"C"` ×2 (deregister, register) | `:464`, `:466` | every non-advancing cycle | no — the v1 re-select |
| 9 | Move Register→Resource1 / →Resource2 / →Resource3 | `ClearResources:264`, `:268`, `:272` | `advance_grade_clear` only, before advancing | no |
| 10 | Right-click `"R"` ×3 (clear stuck resource gems) | `:266`, `:270`, `:274` | `advance_grade_clear` only — **unconditional**, fires even when nothing is stuck | **yes** |
| 11 | Move Resource3→next grade | `AdvanceGrade:329` | `advance_grade_clear` only | no |
| 12 | Click `"C"` (select next grade, slot3 path) | `AdvanceGrade:331` | `advance_grade_clear` only | no |
| 13 | Click `"C"` (select next grade, absolute path) | `AdvanceGrade:351` | non-clear mode, on advance | no |
| 14 | Move next grade→Register + Click `"C"` | `AdvanceGrade:356`, `:359` | every advance | no |

**Read-only, not game input:** the result-box crop is a `CopyFromScreen` grab of `gem.result_gem_area`
(`:208`); when `gem.save_empty_captures` is on it is written to `logs/captures/` and one line is
appended to `logs/empty_check.txt` (`:230-240`, `:246-257`). No game input clears or opens anything.

### Decision points

| Where | Question | True → | False → |
|---|---|---|---|
| `:369` | Quit hotkey (F11) or token cancelled? | leave the loop | tick on |
| `:372` | Panel asked START and we are not running? | running, Beep(523), select grade (`:374-378`) | fall through |
| `:380` | Panel asked STOP and we are running? | running=false, Beep(1000) (`:382-384`) | fall through |
| `:390` | F12 rising edge? | toggle; on → Beep + select grade (`:393-399`), off → "[STOP]" | fall through |
| `:403` | F9 rising edge? | `gidx=(gidx+1)%count` (**wraps**), select grade (`:405-408`) | fall through |
| `:414` | not running? | continue, no cycle (`:414`) | run a cycle |
| `:420` | F12 held at the top of a cycle? | stop, continue (`:422-427`) | combine |
| `:431` | Register→Combine route failed? | break, reason on card (`Fail`) | click to combine (`:433`) |
| `:438` | `empty_mode != "stop"` AND `empty_signature` set? | run the empty check | skip it — `advanceNow=false` |
| `:202` | the game window is the foreground window? | judge the crop | refuse → "not empty", logged (`:204`) |
| `:209` | the result crop captured? | compare | "not empty" |
| `:213` | the empty-box reference crop loads? | pixel-diff vs reference (`:214`) | colour signature if set (`:221`), else not empty (`:226`) |
| `:219` | diff fraction `<= empty_distance`? | box empty | box not empty |
| `:441` | empty this cycle? | `emptyCount++`; `>= empty_streak` → `advanceNow`, reset (`:443-447`) | `emptyCount=0` (`:449`) |
| `:454` | Combine→Register route failed? | break (`:454`) | continue |
| `:457` | `advanceNow`? | `AdvanceGrade()`; false → break, true → continue (`:459-460`) | deregister+register clicks (`:464-466`) |
| `:304` | `empty_mode == "advance_grade_clear"`? | `ClearResources()` first (`:304`) | skip the clear |
| `AdvanceGrade:307` | `gidx+1 >= grades.Count`? | "[DONE] last grade", Beep(880), stop (`:309-314`) | `gidx++`, set grade (`:317-318`) |
| `:324` | slot3→next-grade route known? | place and click | `Fail("no route")` (`:326`) |
| `:343` | next grade point calibrated? | place it (`:344`) | `Fail` (`:343`) |
| `:345` | the placement landed? | click to select (`:351`) | `Fail` "stopped instead of clicking blind" (`:347`) |
| `:356` | next grade→Register route failed? | `Fail`, stop (`:356`) | click to select (`:359`) |
| `:470` | `PauseRequested`? | "[PAUSE] graceful stop", stop (`:472-475`) | loop again |
| `Fail:76` | any helper found a point/move uncalibrated or a placement to have failed? | message on card, running=false (`:78-81`) | carry on |

**Worth a verdict:**

1. **Auto-advance is gated on the fallback, not the primary signal.** The empty check runs only when
   `empty_signature != null` (`:438`), even though the primary test — the pixel-diff against
   `calib_gem_result.png` — does not need it (`:213-219`). A user with the crop calibrated but no colour
   signature would never auto-advance. Worth confirming whether `empty_signature` is always set.

2. **F9 wraps the grade list; `AdvanceGrade` does not.** F9 does `(gidx+1) % grades.Count` (`:405`),
   while the automatic advance stops after the last grade (`:307`). A manual cycle round the grades is
   therefore possible, but the run never wraps on its own.

3. **A non-foreground game silently disables advancing.** `IsResultBoxEmpty` refuses to judge unless the
   game is the foreground window (`:202`) and answers "not empty" — the safe direction, since a
   `CopyFromScreen` crop of the launcher is meaningless (`:197-206`). The consequence is that a run with
   the launcher in front never advances, and the refusal is only recorded when
   `save_empty_captures` is on (`:204`, `:246-257`).

4. **`ClearResources` right-clicks three slots unconditionally** (`:261-277`), on every
   `advance_grade_clear` advance, whether or not anything is stuck. Defensive; harmless if a right-click
   over an empty slot is a no-op.

5. **No click is verified after it lands.** Unlike the pet feeder's post-click slot check, the composer
   never re-reads to confirm a click did anything; a mis-landed click surfaces only as a later wrong or
   empty result. The only verification anywhere is the empty-box check (`:191`), which judges the
   *outcome* of a combine, not the clicks.

6. **The two clicks at `:464`/`:466` land on the same spot with no move between them** (the cursor is
   still on Register after the Combine→Register route), so they are a deregister then a register on the
   same button. That matches the v1 sequence the comment names (`:157-160`), but it is worth knowing they
   are not two different buttons.

7. **A failure inside `SelectGradeAndRegister` does not stop the run directly.** It calls `Fail`, which
   sets `running=false` and `state.Running=false` (`:78-81`) but returns `void`, so the panel-START
   (`:378`) and F12 (`:398`) callers do not branch on it — the run ends by falling into the `!running`
   check on the next pass (`:414`). Same end, one indirection.

8. **A serial write fault has no handler at all.** The tuner at least has a catch around its C/E writes;
   the composer's Click/RightClick/`D` writes (`HidPointer.cs:185`, `:189`, `:192`) are outside any
   try, so a fault throws out of `Run` with only the `finally` running (`:479-482`).


## Skill Spammer — `spammer`

*Presses a cooldown-governed rotation of keys — every viable combo first, then the singles — and is the
one foreground tool that stands down for another; it has no natural end, and leaves its loop only on a
stop, a quit, a cancel, or a dead port.*

### What it is

| Term | What it is |
|---|---|
| **Preset** | A named key set (`Config.Spammer.Presets`, chosen by `Active`). `ActiveKeys` is the active preset's `key → cooldown-seconds` map, and it is **empty when the named preset is missing** (AppConfig.cs:906-923). |
| **Key** | One entry of the active preset. A leading `*` spells the **fast tap** — the same press, sent lowercase (SkillSpammer.cs:373-374). |
| **Cooldown** | Seconds a key waits after it was last cast before it is due again (SkillSpammer.cs:320-321). |
| **Combo** | An ordered group that must be cast together, one step per `Gap`, with nothing else pressed in between (SkillSpammer.cs:52-58). A key named in a combo is pressed **only** as part of it (SpammerOrder.cs:66-73). |
| **Single** | A key the loop may press on its own: the preset minus everything a combo owns (SpammerOrder.cs:66-73). |
| **Priority / preempt** | An ordered subset of keys. When non-empty, at most **one** press goes out per tick and the rest are **delayed**, not dropped (SkillSpammer.cs:59-62, :340-350). |
| **Lead-in / Step** | The opening: a wait before the first press (`LeadIn`), then the spacing between the opening presses (`Step`) (SkillSpammer.cs:63-69, :112-141). |
| **Yield** | The spammer standing down from the game so a **waiting** tool can have it: it releases the gate, presses nothing, and re-acquires when the gate comes free (SkillSpammer.cs:233-268). |
| **The gate** | `PortGate`. The launcher claims it for `spammer` at Start (LauncherService.cs:451, :595-608); the spammer is the only tool that gives it up mid-run and takes it back. |
| **Dead port** | A serial write that throws. The tool stops with a reason on the card rather than letting the exception unwind to the launcher (SkillSpammer.cs:157-175). |

### The flow

```
Run(ser, state, ct)                                                  SkillSpammer.cs:34
 │
 ├─ WatchCancellation(ct)                                                    :36
 ├─ cooldowns = ActiveKeys
 │    └─ EMPTY (active preset missing) ─────────────────────► message on the card,
 │                                                            return 0 — no run            :37-50
 ├─ plan:  combos  = SpammerOrder.Runnable(...)                              :60
 │         singles = SpammerOrder.Singles(keys, priority, combos)            :61
 │         preempt = priority.Count > 0                                      :62
 │         startup = ActiveStartup                                           :66
 │         _gate == null  ⟹ this tool can never yield (tests, no board)   :21, :243
 └─ try ▸ while (true)                                                    :179-355
      │
      ├─ SleepCheck(0.02) ── the QUIT hotkey sets QuitPressed       :183 / ToolBase:94
      ├─ QuitPressed || ct cancelled? ─────────────────────────► BREAK          :184
      │
      ├─ panel said START?  (state.Running && !running)                        :187
      │     running = true · QuitPressed = false · Reset()  → "[Panel] START"
      ├─ panel said STOP?   (!state.Running && running)                        :195
      │     running = false                                 → "[Panel] STOP"
      │
      ├─ F12 edge? ── THE TOGGLE, not a terminate ──────────────────────── :202-210
      │     running = !running · state.Running = running
      │       starting → QuitPressed = false · Reset()      → "[GO]"
      │       stopping → (nothing pressed; the loop keeps ticking)  → "[STOP]"
      │
      ├─ PauseRequested? ──────────────────────────────────────► BREAK   :215-221
      │
      ├─ !running ─────────────────────────────► continue (tick again; press nothing)  :223
      │
      ├─ the lead-in moment has passed? → clear Schedule / NextActionAt        :227-231
      │
      ├─ ── THE YIELD  (only when a PortGate is wired) ───────────────────── :243-268
      │    we are yielded?
      │      ├─ TryAcquire("spammer") == true ─► yielded = false
      │      │      · Message = null · Reset(payLeadIn: false)   → "[RESUME]"  :245-257
      │      └─ still held ────────────────────► continue (press nothing)      :268
      │    else if we HOLD the gate AND someone WAITS  ───────────────────     :259-266
      │      └─ Release("spammer") · yielded = true
      │             · Message = "paused — <waiter> has the game" → "[YIELD]"
      │             └──────────────────────────► continue (press nothing)      :268
      │
      ├─ no combo running, and one is fully off cooldown? → enqueue its keys   :275-286
      │
      ├─ a combo step is due?  → ▓ SendKey(one key) ▓ ← WRITE to the board      :292-313
      │        (the combo OWNS the tick; no filler goes out until it drains)
      │
      ├─ otherwise walk the singles; each due key → ▓ SendKey ▓ ← WRITE         :315-352
      │        (preempt: ONE press per tick, the rest delayed)
      │
      └─ any send threw (dead port)? ──────────────────────────► BREAK        :169-174, :354
   finally ▸ state.Running = false                                             :357-360
```

**Every write in that diagram is a tap, not a hold.** `SendKey` writes one command and returns
(SkillSpammer.cs:400); the firmware does the press-and-release. So the spammer keeps **nothing** down
between ticks, and it is deliberately *not* one of the tools `HeldKeys` names (HeldKeys.cs:21). There
is no `★` in this diagram for that reason: there is no moment at which this tool can leave something
held.

**The lead-in presses nothing at all.** From `Reset` until `openingAt` the loop ticks, the F12/stop
branches still run, but no key is sent — which is why the countdown is pushed to the card's SCHEDULE
line as a *moment* (`NextActionAt`) rather than a number (SkillSpammer.cs:112-128, :227-231).

### Every action sent to the board

This is the complete set of bytes the tool writes. It is four commands, and nothing else — no mouse, no
gate command, no held key.

| # | Command written | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | `F <n>\n` — press function key n (1–12) | `SendKey` 386 | a key or combo step is due | no |
| 2 | `f <n>\n` — the fast tap of the same | `SendKey` 386 | the key was spelled `*F…` | no |
| 3 | `K <c>\n` — press one printable ASCII char | `SendKey` 393 | a key or combo step is due | no |
| 4 | `k <c>\n` — the fast tap of the same | `SendKey` 393 | the key was spelled `*…` | no |

**Side effects that are not board input, but are actions:**

- **`state.*` writes** — `Message` (the yield line and every failure), `Schedule` / `NextActionAt` (the
  lead-in), `Current` and `Cycle` (the last key and the count), and `Running` (SkillSpammer.cs:47, :121,
  :150-151, :169, :250, :264, :308-309, :338-339, :359).
- **The gate is released and re-acquired** by the tool itself on the yield path
  (SkillSpammer.cs:246, :261) — the only place any tool does this.
- **`Console.WriteLine` and `Beep`** throughout — both invisible in the published WinExe, which is why
  the card state above carries the same information (ToolState.cs:37-40).

### Every decision point

| # | Decision | Where | One way | The other |
|---|---|---|---|---|
| 1 | Active preset has keys? | 37 | run | refuse on the card, return 0 without a run |
| 2 | Quit hotkey / run cancelled? | 184 | **end the run** | keep ticking |
| 3 | Panel said Start? | 187 | `running=true`, clear `QuitPressed`, `Reset()` | — |
| 4 | Panel said Stop? | 195 | `running=false` (the loop lives on) | — |
| 5 | `F12` pressed (an edge)? | 202 | **toggle** `running`; if starting, clear `QuitPressed` + `Reset()` | — |
| 6 | Pause hotkey? | 215 | graceful stop, **break** | — |
| 7 | Running? | 223 | press | tick again, pressing nothing |
| 8 | Somebody waits for the gate? | 259 | release it and yield | keep the game |
| 9 | Yielded, and the gate is now free? | 246 | re-acquire, `Reset(payLeadIn: false)` | stay paused |
| 10 | A combo is fully off cooldown? | 280 | enqueue and cast it | fall through to the singles |
| 11 | A combo step is due? | 296 | send ONE step | the combo keeps the tick |
| 12 | A single is due? | 321 | send it; with `preempt`, one per tick | skip to the next tick |
| 13 | The key can't be sent? | 395 | warn once, do **not** count it | — |
| 14 | The write threw (dead port)? | 169 | stop with a reason on the card | — |

### Worth a verdict

Not description — the things I would want a decision on.

1. **The hotkey the toggle actually is.** This page's brief described the *quit* hotkey as a toggle
   that "clears `QuitPressed` and carries on". The code disagrees, and the labels are the kind that get
   swapped: the **toggle** is the **Start / stop** hotkey, `F12` (SkillSpammer.cs:202-210;
   USER_GUIDE.md:906), and the **quit** hotkey, `F11`, sets `QuitPressed` and the loop **breaks** on it
   (SkillSpammer.cs:184, ToolBase.cs:94; USER_GUIDE.md:907). The clearing of `QuitPressed` is real but
   sits in the *start* paths.

2. **The `QuitPressed = false` writes are unreachable.** Both clears (SkillSpammer.cs:190, :207) sit
   *after* the top-of-loop `if (QuitPressed) break` (:184), and nothing between can have set the flag
   except the `SleepCheck` that just ran — which would have broken first. So the "pause, not quit"
   meaning the base class attributes to these writes (ToolBase.cs:80) and PROGRESS §62 relies on is, in
   this loop, dead code. Either the break is the intent (F11 ends the run, and the clears should go), or
   the clears are the intent (F11 should pause) — the two cannot both be true.

3. **The yield rule has not moved onto `ShouldStandDown`.** The spammer yields with a hand-written test,
   `held.Owner == GateOwner && held.Waiting is { } waiter` (SkillSpammer.cs:259), while `PortGate`'s own
   doc says `ShouldStandDown(owner)` "replaces every hand-written pair-wise rule" and that "the spammer
   yields to anyone waiting" (PortGate.cs:108-116). Behaviour matches today only because the sole waiter
   (the pet) declares a deadline — `ShouldStandDown` returns **false** for a waiter with no deadline
   (PortGate.cs:114-115), so swapping the call in now would quietly stop the yield for any future
   deadline-less waiter.

4. **The yield has no timeout.** If the waiter never releases the gate, the spammer stays paused with
   the card reading "paused — … has the game" for as long as that lasts; there is no attempt limit or
   expiry (SkillSpammer.cs:243-268). The comment at :239-242 says the pause "lasts exactly as long as
   the waiter's action does", which is honest — but it also means a wedged waiter silently starves the
   rotation.

5. **`WarnedKeys` is a `static` set** (SkillSpammer.cs:366), so a bad key warned during one run is never
   warned again for the life of the process, across later runs. The warning is one-per-process, not
   one-per-run.

6. **The F-range is narrowed to 1–12 here** (SkillSpammer.cs:381) even though the comment notes
   `ParseVk` accepts `F1`–`F24` elsewhere. A preset naming `F20` is refused with a one-time warning and
   is simply never pressed — deliberate, but it means such a key looks configured and does nothing.

---

## Hold Space — `holdspace`

*Holds the spacebar down on the board (one `P`) so the game picks items up, and always lifts it (`U`)
on the way out — including in the loop's `finally` — so the key can never be left stuck down.*

### What it is

| Term | What it is |
|---|---|
| **Hold / Release** | The two board commands: `P\n` presses the spacebar down, `U\n` lifts it (HoldSpace.cs:32-33). |
| **The toggle** | The launcher's top-right button, `HoldSpaceToggle` — this tool has no card, so the button is its only control (MainWindow.xaml.cs:342-384). |
| **Engaged** | The question the toggle asks — loaded, *starting*, or *stopping* — so a press during a cold start means stop, not a second start (ToolToggle.cs:26-30; MainWindow.xaml.cs:362-365). |
| **Held down** | `HeldKeys` names `holdspace` as one of the two tools that can leave something held, so **every** stop path must release it (HeldKeys.cs:17-22). |
| **ReleaseHeld** | The launcher's own release — `U\n` **and** `l\n`, sent on every stop path independent of this loop (LauncherService.cs:674, :795-827). |
| **The gate** | The launcher claims `PortGate` for `holdspace` at Start; unlike the spammer, this tool **never** reads it and never yields (LauncherService.cs:451; HeldKeys.cs:17-22). |

### The flow

```
Run(ser, state, ct)                                                     HoldSpace.cs:23
 │
 ├─ WatchCancellation(ct)                                                      :25
 └─ try ▸ while (true)                                                      :35-69
      │
      ├─ SleepCheck(0.05) ── the QUIT hotkey sets QuitPressed       :39 / ToolBase:94
      ├─ QuitPressed || ct cancelled? ─────────────────────────► BREAK         :40
      │
      ├─ panel said START?  (state.Running && !running)                       :43
      │     running = true · QuitPressed = false
      │     └─ ★ Hold()  ── WRITE "P" ── the spacebar is now DOWN    :47 → :32
      │
      ├─ panel said STOP?   (!state.Running && running)                       :51
      │     running = false
      │     └─ Release() ── WRITE "U" ── the spacebar is UP again    :54 → :33
      │
      └─ F12 edge? ── THE TOGGLE, not a terminate ───────────────────── :59-67
            running = !running · state.Running = running
              starting → QuitPressed = false
                   └─ ★ Hold()  ── WRITE "P"  (down again)          :64 → :32
              stopping → Release() ── WRITE "U"  (lifted)           :65 → :33
            (the loop keeps ticking either way)
   finally ▸                                                                :70-74
      ├─ Release() ── ★ WRITE "U"  ← the spacebar can never stay stuck down :72 → :33
      └─ state.Running = false                                              :73
```

**★ marks every moment the spacebar is DOWN.** `HeldKeys` names this tool precisely because of them
(HeldKeys.cs:17-22). Each ★ is matched by a `U`: the stop/branch paths send one immediately, and the
`finally` (:72) sends one however the loop leaves — a `break`, or an exception thrown out of the loop
body. The `finally` is the load-bearing one: it is the release that still goes out when the loop's own
write is what failed. The launcher duplicates it deliberately — `ReleaseHeld` sends `U\n` (and `l\n`)
on every stop path, including when the port is closed, so the tool's own `finally` is not the only
thing standing between the player and a stuck key (LauncherService.cs:674, :795-827).

### Every action sent to the board

Two commands. That is the whole inventory.

| # | Command written | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | `P\n` — press the spacebar down | `Hold()` 32 | panel Start, or an `F12` *start* edge — once per start | no |
| 2 | `U\n` — lift the spacebar | `Release()` 33 | panel Stop, an `F12` *stop* edge, **and the loop's `finally`** | **yes** — the `finally` guarantee |

**Side effects that are not board input, but are actions:**

- **`state.Message`** is written when a `P` or `U` write throws (HoldSpace.cs:32-33) — but see the
  verdict list: this tool has no card, so nothing renders it.
- **`state.Running`** is set true on each start and false in the `finally` (HoldSpace.cs:63, :73).
- **The launcher's `ReleaseHeld`** sends `U\n` **and** `l\n` on every stop of this tool, not just the
  spacebar (LauncherService.cs:674, :815-816) — see the verdict list.
- **`Console.WriteLine` and `Beep`** — invisible in the published WinExe (ToolBase.cs / ToolState.cs:
  37-40).

### Every decision point

| # | Decision | Where | One way | The other |
|---|---|---|---|---|
| 1 | Quit hotkey / run cancelled? | 40 | **break**; the `finally` releases | keep ticking |
| 2 | Panel said Start (or an `F12` start edge)? | 43, 60 | `Hold()` — write `P` | — |
| 3 | Panel said Stop (or an `F12` stop edge)? | 51, 65 | `Release()` — write `U` | — |
| 4 | The loop leaves (any reason)? | 70-74 | the `finally` writes `U` | — |
| 5 | A `P`/`U` write throws? | 32-33 | `state.Message` is set (see below) | — |

### Worth a verdict

Not description — the things I would want a decision on.

1. **This tool's failure report has nowhere to go.** `Hold()` and `Release()` write their failure to
   `state.Message` (HoldSpace.cs:32-33), but Hold Space has no card and the launcher says so in as many
   words — "*its `state.Message` has nowhere to be drawn*" — and surfaces only **its own**
   `ReleaseHeld` failure on the status line beside the toggle (LauncherService.cs:719-723). So a failed
   **`Hold`** (the `P` write throwing) is recorded on a state nobody displays: the button would read
   "holding" over a spacebar that was never pressed. A failed **release** is covered, but by the
   launcher's copy, not this one.

2. **`QuitPressed = false` is unreachable here too.** Both clears (HoldSpace.cs:46, :64) sit after the
   top-of-loop `if (QuitPressed) break` (:40), so the same contradiction as the spammer applies: the
   start paths say "pause, not quit", the loop-top break says "quit ends the tool", and the break wins.

3. **A `Hold` needs no yield, and this tool has none.** The launcher claims the gate for `holdspace` at
   Start, and this tool never reads it or gives it up (LauncherService.cs:451; the launcher passes no
   `PortGate` when it builds `HoldSpace`). Unlike the spammer — the one tool that stands down for a waiter — Hold
   Space will block the resident pet feeder for as long as it is left on, and it "never ends on its
   own" either. Worth confirming that the pickup toggle is expected to be short.

4. **A stop of Hold Space releases the mouse button as well.** Every stop path runs `ReleaseHeld`,
   which sends `U\n` **and** `l\n` unconditionally (LauncherService.cs:674, :815-816). The two are
   idempotent, so this is safe *today* — the pet cannot be mid-drag while holdspace holds the gate —
   but it is a cross-tool write from a stop of an unrelated tool, and it is only safe because of the
   gate, not because of anything in this tool.

---

## What the spammer and Hold Space audit did not look at

The board side of the protocol (what `P`, `U`, `K/k`, `F/f` do in the firmware) is not traced here. The
spammer's planning half — `SpammerOrder.Runnable` / `Singles` / `For` and the preset migration — is
audited by its own tests, not here. And the config editor's half of the same settings (`Spammer` tab,
the card's rotation picker) is out of scope; only the run-time reading of those settings is covered.


## The shop tool — what it is

| Term | What it is |
|---|---|
| **The tool** | `ShopTool`, one class in two modes. `ShopMode.Buy` is the "Buy Items" card; `ShopMode.Sell` is "Sell Items" (`ShopTool.cs:12`, `MainWindow.xaml.cs:44-45`). |
| **A run** | One `Run` call, which does exactly **one pass**, then stops — these are counted jobs, not loops (`ShopTool.cs:93-95`). |
| **A pass** | `BuyPass` or `SellPass` — the list of transactions for this run (`ShopTool.cs:148`, `:190`). |
| **A transaction** | The shared gesture: RIGHT-click the target, then MAX, then Enter, then Enter (`ShopTool.cs:14-16`, `:232-248`). Identical for both modes. |
| **Buy preset** | `BuyPreset` — a name carrying (scroll notches, row index) and a count the run-time ignores (`AppConfig.cs:656-668`). |
| **Shop region** | The calibrated rectangle round the visible shop list. Every row centre is *derived* from it, not marked (`ShopGeometry.cs:20-58`). |
| **Row** | A derived row centre inside the shop region; row 0 is the top one (`ShopGeometry.cs:51-58`). |
| **Bag grid** | The calibrated 8×8 region. The 64 slot centres derive from it (`BagGrid.cs:37-49`). |
| **Slot index** | 0 = top-left, 63 = bottom-right, row-major (`BagGrid.cs:34-36`). |
| **Focus point / scroll point** | The *same* config field, `BuySellConfig.ScrollPoint`. It is left-clicked to focus the game; Buy also wheels from it (`AppConfig.cs:630-632`, `ShopTool.cs:275-280`). Buy's `Ready` calls it the "scroll point", Sell's calls it the "focus point" (`ShopTool.cs:123`, `:139`). |
| **MAX** | The count dialog's max-quantity button — the only click in a transaction with no keyboard equivalent (`AppConfig.cs:634-636`). |
| **SellCap** | A hard ceiling on one sell run, enforced in the loop, not advice (`AppConfig.cs:649-651`, `ShopTool.cs:195-201`). |
| **The board** | The Arduino HID mouse, over serial. Placement is a verified closed loop; click, key and wheel are one open-loop write each (`HidPointer.cs:74`, `:185`, `:189`, `:192`). |

---

## Buy Items — `buy`

*One line: right-clicks one derived shop row up to `count` times, each time pressing MAX, Enter, Enter to
buy the maximum quantity; it stops when the count is reached, or on a placement that fails, a quit
hotkey, or the launcher's Stop.*

### The flow

```
Run()                                                                     ShopTool.cs:46
 │
 ├─ WatchCancellation(ct)  ← hand the run's token to the base so a       :48
 │                            SleepCheck can see the launcher's Stop
 ├─ Ready(state) is a problem? ────────────────────────────────► card message, return 0
 │      buy needs: a preset chosen, the preset still exists,          :49-55, :115-142
 │      shop rows usable, focus/scroll point, MAX
 ├─ f12Was = Start hotkey already down   ← an EDGE, not a level         :58
 │
 └─ while (true)                                                         :62
     │
     ├─ SleepCheck(0.05)                                               :64, ToolBase.cs:85
     │     ├─ launcher Stop cancels the token ───────────────────► THROW
     │     └─ Quit hotkey down ─► QuitPressed = true                  ToolBase.cs:94
     ├─ QuitPressed || ct cancelled ─────────────────────────────► break  :65
     │
     ├─ panel edge:  off→on  → running=true;  "[Panel] BUY";  beep 523  :67-73
     │               on→off  → running=false; "[Panel] STOP"; beep 1000 :74-79
     ├─ Start hotkey RISING edge?  running = !running;                 :81-89
     │     state.Running = running;  "[GO]" / "[STOP]";  beep
     ├─ not running? ────────────────────────────────────────────► continue :91
     │
     └─ ONE PASS, then stop (a counted job — it never repeats)         :93-101
        │
        └─ BuyPass()                                                   :148
           │
           ├─ ScrollFromTop(preset.Scroll)                             :155, :300
           │    └─ FocusGame()                                         :281
           │         ├─ focus point not calibrated ──────────► Stop, return :283-288
           │         ├─ PlaceOn(point): window missing /            :289, :321-339
           │         │    cursor wouldn't arrive ────────────► Stop, return
           │         ├─ SleepCheck(ClickWait)                       :294
           │         ├─ LEFT-CLICK "C"     ← gives the GAME focus    :295
           │         └─ SleepCheck(DialogWait)                      :296
           │    └─ preset.Scroll > 0? ─ yes ─► SCROLL DOWN "Z n"     :304-307
           │                                   wait n*0.025 + 0.4 s  :256
           │
           ├─ RowCentre(ShopRegion, ShopRows, preset.Row)          :157
           │    └─ null ─► message "Couldn't work out the shop       :158-162
           │               row"; return   (no Stop, no [!] line)
           │
           └─ for i in 0 .. _count-1:                                :164
              ├─ QuitPressed || ct cancelled ───────────────► return  :166
              ├─ state.Cycle = i+1;  state.Current = "name i/n"      :168-169
              ├─ ClickAt(row, RIGHT-CLICK "R")                       :173, :312
              │    └─ PlaceOn failed ───────────────────────► Stop, return :333-336
              └─ MaxEnterEnter()                                     :178, :234
                 ├─ SleepCheck(DialogWait)                           :236
                 ├─ ClickAt(MAX, LEFT-CLICK "C")                     :239
                 │    └─ PlaceOn failed ──► return false ──► Stop, return :333-336
                 ├─ SleepCheck(ClickWait)                            :240
                 ├─ ENTER "E"   ← the count dialog                   :242, :250
                 ├─ SleepCheck(DialogWait)                           :244
                 ├─ ENTER "E"   ← the confirmation                   :245
                 └─ SleepCheck(DialogWait)                           :246
              │   (next i)
           └─ state.Message = "Bought {_count}x '{preset}'."         :185
        │
        ├─ BeepMany()   ← five beeps, the "done" pattern              :98
        ├─ running = false;  state.Running = false                    :99-100
        └─ break                                                       :101
     │
     finally: state.Running = false                                   :106
     "Done.";  return 0                                               :109-110
```

The board receives, in order per run: any number of HID `D dx dy` placements (each click and the focus
click drives one), then `C` (focus), optionally `Z n` (scroll), then per purchase `R` (right-click),
`C` (MAX) and `E`, `E` (the two Enters). There is no upward scroll: `Scroll`'s `down:false` branch would
send `Q n` (`ShopTool.cs:261-262`) but the one call site passes `down:true` (`ShopTool.cs:306`), so `Q`
is never sent by this class.

### Every action sent to the game

| # | Action | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | **HID move** `D dx dy` (closed-loop, 1..16 steps) | `PlaceOn` :321 → `HidPointer.To` `HidPointer.cs:74` | before **every** click (focus, row, MAX) | no — it is the precondition for any click to be aimed |
| 2 | **Left-click** `C` | `FocusGame` :295 | once at the start of every buy run | **yes** — the wheel and the right-click are both ignored while the game is unfocused, and starting the tool just unfocused it (`ShopTool.cs:274-280`) |
| 3 | **Scroll down** `Z n` | `Scroll` :306 | once at the start, **only** when `preset.Scroll > 0` | no — positions the list so `preset.Row` means the right row |
| 4 | **Right-click** `R` | `ClickAt` :316 (from `BuyPass` :173) | once per purchase, on the derived row | no |
| 5 | **Left-click** `C` on MAX | `MaxEnterEnter` :239 | after every right-click | no |
| 6 | **Enter** `E` (count dialog) | `MaxEnterEnter` :242 | after MAX, **unconditionally** | **yes** — the dialog is expected; Enter on no dialog does nothing |
| 7 | **Enter** `E` (confirmation) | `MaxEnterEnter` :245 | after the first Enter, **unconditionally** | **yes** — same reasoning (`ShopTool.cs:232-233`) |

**Side effect that is not game input:** `state.Cycle`, `state.Current` and `state.Message` on the shared
`ToolState` are written so the launcher card can show them (`ShopTool.cs:168-169`, `:185`;
`ToolState.cs:26-40`). Nothing is written to disk.

### Decision points

| Where | Question | True → | False → |
|---|---|---|---|
| `Run` :49 | A precondition missing (`Ready`)? | message on the card, `return 0` — no pass | enter the wait-for-start loop |
| `Run` :65 | Quit hotkey pressed, or token cancelled? | `break` (the `finally` clears `Running`) | fall through to the start edges |
| `Run` :67 | Has the panel switched `state.Running` on? | `running=true`, "[Panel] BUY", beep 523 | test the switch-off edge |
| `Run` :74 | Has the panel switched `state.Running` off? | `running=false`, "[Panel] STOP", beep 1000 | test the hotkey edge |
| `Run` :82 | Start hotkey a rising edge? | flip `running` **and** `state.Running`, "[GO]"/"[STOP]" | leave `running` alone |
| `Run` :91 | Not running? | `continue` — nothing is bought | run the one pass |
| `BuyPass` :155 | `ScrollFromTop` succeeded (focus placed)? | continue to the row | `return` (the message was set inside `FocusGame`'s `Stop`) |
| `BuyPass` :158 | `RowCentre` returned null (region or row unusable)? | message, `return` — no click, **no `Stop`** | proceed to the purchase loop |
| `BuyPass` :166 | Quit/cancel inside the purchase loop? | `return` — remaining purchases skipped, **no message** | do the next purchase |
| `BuyPass` :173 | Placement for the row right-click succeeded? | send the right-click | `Stop`, `return` |
| `MaxEnterEnter` :239 | Placement for the MAX click succeeded? | click MAX, then the two Enters | `false` → `Stop`, `return` |

**Worth a verdict:**

1. **`_count` is transactions, and each transaction presses MAX — not "buy N units".** The gesture half
   (`MaxEnterEnter`, `ShopTool.cs:234-248`) always clicks MAX (`:239`) before the Enters, so one
   transaction buys the dialog's *maximum* quantity, and the loop runs that transaction `_count` times
   (`:164`). `BuyPreset.Count` — documented as "How many to buy per run" (`AppConfig.cs:666-667`) — is
   never read by `ShopTool` at all; the number comes from the run (`ShopTool.cs:35-44`,
   `LauncherService.cs:98`). So the config carries a count that does nothing, and the live count means
   something different from what its name suggests.

2. **A quit hotkey aborts the pass silently, then plays the same finish beep.** `BuyPass` returns on
   `QuitPressed` with no message (`ShopTool.cs:166`), and `Run` still calls `BeepMany()` (`:98`) — the
   pattern that otherwise means "done". So an abandoned run and a completed run sound identical, and the
   card keeps whatever `Message` it last held.

3. **`ScrollFromTop` does not scroll to the top — and nothing checks the list is there.** Despite the
   name (`ShopTool.cs:300`), it only applies the preset's scroll from wherever the list already is; the
   docstring is explicit that scroll-to-top was deliberately removed (`:273`). The preset's row is
   measured from the top (`:153-154`), so a list left scrolled by the player silently buys the wrong row.
   `Ready` cannot catch this, because nothing reads the list.

4. **The `RowCentre == null` branch is the odd one out.** It sets `state.Message` but does not call
   `Stop` and prints no `[!]` line (`ShopTool.cs:158-162`), unlike every other failure in the file
   (`:341-346`). It is harmless only because `Run`'s `finally` clears `Running` anyway (`:106`), which
   makes the two failure paths behave the same for a reason the branch itself does not state.

5. **The Start hotkey is not honoured during a pass.** It is polled only in the outer loop
   (`ShopTool.cs:81`), and the passes' `SleepCheck` watches only Quit and Pause (`ToolBase.cs:94-95`). A
   pass is short, so the window is small — but a second Start press mid-pass is ignored, and the loop
   then breaks after that one pass regardless (`:101`).

---

## Sell Items — `sell`

*One line: right-clicks each selected bag slot highest-index-first, each time pressing MAX, Enter, Enter to
sell the whole stack; it stops when the slots are done, or on a failed placement, a quit hotkey, or the
launcher's Stop.*

### The flow

**As Buy, except** the pass, the target, and where the run's start edges lead. The outer `Run` —
`WatchCancellation`, the wait-for-start loop, the panel and hotkey edges, the one-pass-then-stop, the
`BeepMany`, the `finally` — is **identical to the Buy diagram above** (`ShopTool.cs:46-111` shares every
line). Only what follows differs:

```
SellPass()   — reached instead of BuyPass                              ShopTool.cs:190
 │
 ├─ centres = BagGrid.Centres(BagGrid)   ← the 64 slot centres          :193, BagGrid.cs:37
 │
 ├─ slots = SellSlots
 │      .Where(i => 0 <= i < centres.Count)   ← out-of-range dropped    :197
 │      .Distinct()                                                     :198
 │      .OrderByDescending(i => i)            ← HIGHEST INDEX FIRST     :199
 │      .Take(SellCap)                        ← the hard ceiling        :200
 │
 ├─ FocusGame()  ── same left-click "C" as Buy's ScrollFromTop, but     :205, :281
 │    │              with NO scroll afterwards
 │    └─ failed ────────────────────────────────────► return
 │
 └─ for n in 0 .. slots.Count-1:                                        :207
    ├─ QuitPressed || ct cancelled ───────────────► return  :209
    ├─ state.Cycle = n+1;                            :212
    │  state.Current = "slot {index} ({n}/{total})"  :213
    ├─ ClickAt(centres[slot], RIGHT-CLICK "R")       :215, :312
    │    └─ PlaceOn failed ──────────────► Stop, return :333-336
    └─ MaxEnterEnter()   ← IDENTICAL to Buy's          :220, :234
       (SleepCheck → MAX "C" → Enter "E" → Enter "E")
    │   (next n)
    └─ state.Message = "Sold {slots.Count} slot(s)."   :227
```

The board receives: HID `D dx dy` placements, one `C` focus, then per slot `R`, `C`, `E`, `E`. **No
scroll is ever sent** — Sell has no `Scroll`/`ScrollFromTop` call at all (`ShopTool.cs:190-228`), the one
structural difference from Buy.

### Every action sent to the game

| # | Action | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | **HID move** `D dx dy` | `PlaceOn` :321 | before every click | no |
| 2 | **Left-click** `C` | `FocusGame` :295 | once at the start of the run | **yes** — without it the first right-click is ignored, and the game reports nothing, so it looks exactly like a wrong coordinate (`ShopTool.cs:203-204`) |
| 3 | **Right-click** `R` | `ClickAt` :316 (from `SellPass` :215) | once per slot, highest index first | no |
| 4 | **Left-click** `C` on MAX | `MaxEnterEnter` :239 | after every right-click | no |
| 5 | **Enter** `E` (count dialog) | `MaxEnterEnter` :242 | after MAX, unconditionally | **yes** |
| 6 | **Enter** `E` (confirmation) | `MaxEnterEnter` :245 | after the first Enter, unconditionally | **yes** |

### Decision points

**As Buy, except** the preconditions and the slot selection replace the preset/row ones:

| Where | Question | True → | False → |
|---|---|---|---|
| `Ready` :119 | (sell branch) bag grid valid? MAX set? focus point set? | check the next | message naming what is missing, `return 0` |
| `Ready` :126 | At least one slot selected? | continue | "No slots are selected to sell." |
| `Ready` :127 | `SellSlots.Count <= SellCap`? | continue | refuse, telling the player to raise the cap or narrow the selection |
| `SellPass` :197 | Is a selected index inside `0 .. 63`? | keep it | **drop it silently** |
| `SellPass` :198 | Duplicate indices? | collapsed by `Distinct` | — |
| `SellPass` :200 | More than `SellCap` slots survive the filter? | `Take(SellCap)` — the highest indices win | sell them all |
| `SellPass` :205 | `FocusGame` succeeded? | enter the slot loop | `return` |
| `SellPass` :209 | Quit/cancel inside the slot loop? | `return`, no message | sell the next slot |
| `SellPass` :215 | Placement for the slot right-click succeeded? | send the right-click | `Stop`, `return` |
| `MaxEnterEnter` :239 | Placement for the MAX click succeeded? | click MAX, then the two Enters | `false` → `Stop`, `return` |

**Worth a verdict:**

1. **`Ready` and `SellPass` disagree about which slots exist.** `Ready` compares the *raw* list length to
   the cap (`ShopTool.cs:127`), while `SellPass` first drops out-of-range indices and duplicates
   (`:197-198`) and only then takes the cap (`:200`). So a selection of 16 valid slots plus 1 stale
   out-of-range index is refused by `Ready` ("17 slots selected but the per-run cap is 16") even though
   the pass would sell exactly 16 — and duplicates can inflate `Ready`'s count the same way. The two
   checks are about different sets and can disagree in both directions.

2. **Highest-index-first is deliberate, and the reason is worth keeping written down.** Selling 0 upward
   would skip items if the bag compacts after a sale (removing slot 0 shifts slot 1 into it); going
   highest-first is correct whether or not it compacts (`ShopTool.cs:19-22`, `:199`). This is the
   defensive choice, not a surprise — but it is invisible in a message, so it is only safe as long as the
   comment stays.

3. **The focus point is left-clicked, so it must be inert.** Sell sends a real left-click at
   `ScrollPoint` for focus only (`ShopTool.cs:205`, `:281-298`); the same point Buy wheels from. A point
   chosen over a list row would select or act on that row on every run (`:275-277`). Because Sell never
   scrolls, the "inert" requirement is *only* about this click — which is easy to forget when the field
   is named and configured as a scroll point.

4. **The cap is enforced twice on purpose.** Once in `Ready` (`ShopTool.cs:127`) and again in the loop
   that destroys things (`:195-200`). That is stated as intentional (`:195-196`) and not a defect, but it
   is the reason the two checks existing is acceptable rather than a redundancy to remove.

5. **The selection is not persisted, on purpose.** `BuySellConfig.SellSlots` is deliberately not saved —
   selling is the one irreversible thing in the suite, so it starts from an empty grid every launch
   (`AppConfig.cs:643-647`). A selection carried over from a previous session is a selection nobody
   re-checked, so the tool reads only what is ticked now.

---

## What the shop audit did not look at

The two `Ready` branches share a shape but not their checks, and each is traced here only to its
`return` value. The placement primitive (`PlaceOn` → `HidPointer.To` → `D` / `WaitForCursorToSettle`) is
read only as far as the shop tool calls it, and is audited where it lives (`HidPointer.cs`). The serial
protocol letters (`C`, `R`, `E`, `Z`/`Q`, `D`) are named as the tool writes them; their firmware
behaviour — including whether `E` on no dialog is truly a no-op — is the board's to guarantee, not this
file's. The launcher wiring (`LauncherService.cs:133-134`) is cited only to show where the preset, count
and mode come from.


## Quest Hand-in — `quest`

*Replays a fixed, player-built flow of clicks and keypresses at the quest NPC, a set number of times,
and ends by itself when the loops run out — or when the mouse moves past the guard, or when it is
stopped.*

### What it is

| Term | What it is |
|---|---|
| **Flow** | One named list of steps, with its own loop count and its own initial wait. `QuestPreset`, run by `RunFlow` [QuestTool.cs:83](../SealTools.Quest/QuestTool.cs#L83). |
| **Sequence** | A named list of flows, each with a repeat count, wrapped in a **master loop**. `RunSequence` [QuestTool.cs:134](../SealTools.Quest/QuestTool.cs#L134). |
| **Step** | One action the board sends: click, right-click, Enter, key, or wait. `Send` [QuestTool.cs:324](../SealTools.Quest/QuestTool.cs#L324). |
| **The positioning window** | The opening countdown; the player puts the mouse where the clicks must land, and the tool never moves it again [QuestTool.cs:247](../SealTools.Quest/QuestTool.cs#L247), [QuestTool.cs:250](../SealTools.Quest/QuestTool.cs#L250). |
| **The guard** | The cursor read once at the start; if it moves more than `GuardPx`, the run stops [QuestTool.cs:266](../SealTools.Quest/QuestTool.cs#L266), [QuestTool.cs:310](../SealTools.Quest/QuestTool.cs#L310). |
| **Refuse** | A pre-flight validation failure — nothing has been clicked yet [QuestTool.cs:285](../SealTools.Quest/QuestTool.cs#L285), [QuestTool.cs:408](../SealTools.Quest/QuestTool.cs#L408). |
| **`Finish`** | Clears the standing card lines on every exit; leaves `Message` alone [QuestTool.cs:365](../SealTools.Quest/QuestTool.cs#L365). |

The tool **reads nothing** — no capture, no OCR, no window lookup — which is why it has no calibration
at all [QuestTool.cs:11](../SealTools.Quest/QuestTool.cs#L11). A wrong flow does `Loops` wrong actions
and the game accepts every one of them; the only thing that can notice is the player, watching one test
loop [QuestTool.cs:15](../SealTools.Quest/QuestTool.cs#L15).

### The flow

```
Run()                                                                     QuestTool.cs:46
 │
 ├─ WatchCancellation(ct)                                                  :48
 │
 ├─ WHAT THE CARD CHOSE  (it outranks the tab)
 │    ├─ card names a SEQUENCE  ── missing? ──► Refuse → Finish → return 0             :53-59
 │    ├─ card names a FLOW      ── missing? ──► Refuse → Finish → return 0             :62-70
 │    └─ neither ─► the TAB's choice: its SEQUENCE if one is chosen, else the one flow :75-79
 │
 ├─ Validate(preset)  ── no steps, or a step the board can't send? ──► Refuse → Finish → 0
 │        (NOTHING IS CLICKED)                                  :86, :285-295
 │
 ├─ state.Schedule = the plan · Reset() · Log the plan                                  :88-94
 │
 ├─ Position(InitialWait)   count down in whole seconds, out loud                       :96, :250
 │    └─ cancelled or quit hotkey ──► false ──► Cancelled → Finish → return 0           :97, :256
 │
 ├─ TryReadGuardReference()  the cursor, read ONCE and STRICTLY                         :99, :266
 │    └─ unreadable ──► message ──► Finish → return 0        ← fails CLOSED             :275-279
 │
 └─ LOOP   (master loop over a sequence :175, or the flow's own count :101)
      │
      └─ RunSteps(one pass)                                                             :224
           │
           └─ per STEP, in order:
                │   ├─ cancelled? / QuitPressed? ──► Cancelled                           :229
                │   ├─ cursor moved more than GuardPx? ──► StoppedByGuard                :233-237
                │   │      message: where it is now and where it started
                │   └─ otherwise ──► Send(step) ───────────────────────► TO THE BOARD    :240, :324
                │                     · click  →  "C\n"                                  :329
                │                     · right  →  "R\n"                                  :332
                │                     · enter  →  "E\n"                                  :335
                │                     · key    →  "K <char>\n"                           :338
                │                     · wait   →  nothing at all                         :340
                └─ Wait(step delay) ── cancelled / quit ──► Cancelled                    :241, :350
           │
           └─ the pass ends as ONE of three:
                Done           ──► next loop, or Finish() + "DONE" ──► return 0          :108, :122-124
                Cancelled      ──► Cancelled → Finish → return 0                         :111-112
                StoppedByGuard ──► message on the card → Finish → return 0               :114-118
```

**The stop path is explicit in three places, and all three end the same way.** A stop arrives as a
cancelled token or a `QuitPressed` flag, and both are checked in `RunSteps` *before every send* and
inside `Wait` *before every tick* [QuestTool.cs:229](../SealTools.Quest/QuestTool.cs#L229),
[QuestTool.cs:355](../SealTools.Quest/QuestTool.cs#L355); the positioning countdown is a third, so a
stop there does not wait out the seconds [QuestTool.cs:256](../SealTools.Quest/QuestTool.cs#L256). Each
returns `Cancelled`, which logs a line and calls `Finish`
[QuestTool.cs:402](../SealTools.Quest/QuestTool.cs#L402).

**Quest is the suite's only tool that ends on its own.** There is no `while(true)`: the two loops are
bounded — `for (loop = 1; loop <= loops; loop++)` and the sequence's `for (master = 1; master <= loops;
master++)` [QuestTool.cs:101](../SealTools.Quest/QuestTool.cs#L101),
[QuestTool.cs:175](../SealTools.Quest/QuestTool.cs#L175). When the last pass is `Done`, `Finish()`
clears the card and `Run` returns 0 [QuestTool.cs:122](../SealTools.Quest/QuestTool.cs#L122). A guard
stop [QuestTool.cs:114](../SealTools.Quest/QuestTool.cs#L114) and a refusal
[QuestTool.cs:408](../SealTools.Quest/QuestTool.cs#L408) tear the run down the same way. The tool never
sets `ToolState.Running`; the launcher does, and tears it down when `Run` returns
[QuestTool.cs:19](../SealTools.Quest/QuestTool.cs#L19).

**Where it writes to the board:** only in `Send`, and only in the loop
[QuestTool.cs:324](../SealTools.Quest/QuestTool.cs#L324). There is no cursor move anywhere in the tool —
the clicks land wherever the player left the mouse [QuestTool.cs:247](../SealTools.Quest/QuestTool.cs#L247).

### Every action sent to the game

This is the complete inventory. There are **no defensive actions**: every write is a step the player
built, and nothing here fires to clear a dialog or guard a state.

| # | Action | Where | When | Defensive? |
|---|---|---|---|---|
| 1 | Left **click** — board `C` [HidPointer.cs:185](../SealTools.Core/HidPointer.cs#L185) | `Send` [QuestTool.cs:329](../SealTools.Quest/QuestTool.cs#L329) | once per click step, every loop | no |
| 2 | **Right-click** — board `R` [HidPointer.cs:192](../SealTools.Core/HidPointer.cs#L192) | `Send` [QuestTool.cs:332](../SealTools.Quest/QuestTool.cs#L332) | once per right-click step, every loop | no |
| 3 | **Enter** — board `E` | `Send` [QuestTool.cs:335](../SealTools.Quest/QuestTool.cs#L335) | once per enter step, every loop | no |
| 4 | **Key** — board `K <char>` | `Send` [QuestTool.cs:338](../SealTools.Quest/QuestTool.cs#L338) | once per key step; exactly one printable character | no |

A **wait** step sends nothing at all — it falls through `Send`'s default branch
[QuestTool.cs:340](../SealTools.Quest/QuestTool.cs#L340).

**Side effects that are not game input:** `Log` appends to `logs/quest.log` beside the launcher, and to
the console [QuestTool.cs:382](../SealTools.Quest/QuestTool.cs#L382) — the file is the durable record,
because the published WinExe has no console and a self-ended run otherwise looks exactly like a stopped
one [QuestTool.cs:374](../SealTools.Quest/QuestTool.cs#L374). The tool also writes the card's standing
lines (`Schedule`, `Current`, `Message`).

### Decision points

| Where | Question | True → | False → |
|---|---|---|---|
| `Run` [53](../SealTools.Quest/QuestTool.cs#L53) | Card named a sequence? | look it up; a missing name → Refuse | check for a card flow |
| `Run` [62](../SealTools.Quest/QuestTool.cs#L62) | Card named a flow? | look it up; a missing name → Refuse | fall through to the tab |
| `Run` [75](../SealTools.Quest/QuestTool.cs#L75) | A tab sequence is chosen? | `RunSequence` | `RunFlow` on the tab's single flow |
| `Validate` [285](../SealTools.Quest/QuestTool.cs#L285) | Any step complaint — none, or unsendable? | Refuse, nothing clicked | run |
| `Position` [256](../SealTools.Quest/QuestTool.cs#L256) | Cancelled or quit during the countdown? | Cancelled → Finish | read the guard |
| `TryReadGuardReference` [268](../SealTools.Quest/QuestTool.cs#L268) | Cursor readable? | remember the start point, run | Finish → stop (fails closed) |
| `RunSteps` [229](../SealTools.Quest/QuestTool.cs#L229) | Cancelled or quit, before a step? | Cancelled | check the guard |
| `GuardProblem` [310](../SealTools.Quest/QuestTool.cs#L310) | Cursor moved more than `GuardPx`? | StoppedByGuard → Finish | `Send` the step |
| `Wait` [355](../SealTools.Quest/QuestTool.cs#L355) | Cancelled or quit inside a step's delay? | Cancelled | next step |

**Worth a verdict:**

1. **There are no defensive actions, and the tool reads nothing.** Every write is a configured step
   [QuestTool.cs:324](../SealTools.Quest/QuestTool.cs#L324), so a wrong flow does `Loops` wrong actions
   and the game accepts every one of them — the trade stated in the class comment
   [QuestTool.cs:15](../SealTools.Quest/QuestTool.cs#L15). The cursor guard is the only safety net.

2. **The guard reads the cursor once and fails CLOSED** — an unreadable cursor ends the run
   [QuestTool.cs:266](../SealTools.Quest/QuestTool.cs#L266). That is the deliberate opposite of the
   tuner's guard, which fails *open* because it needs a game window it may not be able to measure
   [QuestTool.cs:262](../SealTools.Quest/QuestTool.cs#L262).

3. **Three ways to end all return 0 and all leave the card reading "stopped".** Done
   [QuestTool.cs:124](../SealTools.Quest/QuestTool.cs#L124), StoppedByGuard
   [QuestTool.cs:118](../SealTools.Quest/QuestTool.cs#L118) and Cancelled
   [QuestTool.cs:112](../SealTools.Quest/QuestTool.cs#L112) are told apart only by `Message` and by
   `logs/quest.log` [QuestTool.cs:402](../SealTools.Quest/QuestTool.cs#L402). The log exists precisely
   because of that [QuestTool.cs:376](../SealTools.Quest/QuestTool.cs#L376).

4. **`Finish` clears `Schedule` and `Current` but leaves `Message`** on purpose — the guard stop's
   sentence and a refusal are the one thing left to read afterwards
   [QuestTool.cs:365](../SealTools.Quest/QuestTool.cs#L365).

5. **Validation and the guard cover different windows and nothing else does.** `Refuse` checks every
   step before the mouse moves [QuestTool.cs:285](../SealTools.Quest/QuestTool.cs#L285); the guard can
   only notice a hand that moved after the wait. A step that is valid but pointed at the wrong pixel is
   caught by neither.

6. **"Test one loop" runs the identical code** — the guard, the validation and the sending are not
   re-implemented beside it [QuestTool.cs:28](../SealTools.Quest/QuestTool.cs#L28). Worth confirming
   that stays true as the tool grows.

---

## Pet Feeder — `pet`

*Keeps each ticked breeding row fed on its own per-row clock by reloading the feeder before the pet
runs out; it loops for days and stops only on its own Stop — it is the one tool that ignores the quit
hotkey.*

### The flow, at the top level

```
Run()                                                                    PetTool.cs:283
 │
 ├─ Ready()  ── a precondition missing? ────────────────────────────► STOP, message on the card   :286-292
 │
 ├─ _feeding = PetFeeding.Load(pet-data.csv)   ← a failure costs the COMPUTED schedule only      :296
 │
 ├─ CLAIM THE GAME (for the look) ── held? wait, saying who ── cancelled? ──► STOP,              :322-344
 │                                                                             "Stopped before it
 │                                                                              could read the rows."
 │
 ├─ InspectRows() = THE LOOK   open the breeder once, read every row, close     :701, :744
 │
 ├─ failures[row] = 0  for every active row · PublishSchedule()                  :352-379
 │
 └─► MAIN LOOP   while !QuitPressed && !ct                                      :387
      │
      ├─ live rows empty? ────────────────────────────────────────► STOP, "Every row has failed"  :389-394
      ├─ soonest = the row whose next is earliest · SleepUntil(next[soonest])                     :396-397
      │     └─ cancelled during the wait ──► break
      ├─ batch = every row due NOW, in ROW ORDER  (none → batch = [soonest])                      :402-407
      ├─ CLAIM THE GAME (for the visit) ── held? wait, saying who · cancelled ──► break           :413
      │     · the row's next is left IN THE PAST, so the same row is picked again after the wait
      ├─ Visit(batch)   open ONCE → READ every row → reload each due row → close ONCE   :418, :1115
      │     └─ cut by a stop ──► "may not have been fed. Check it in game." ──► rethrow   :420-436
      │                          (finally: ReleaseGame)                                  :439
      ├─ fold the result back, per row:
      │     Ok              ──► next = now + min(pet needs, load) + WaitAfterEmpty ; failures=0   :453-476
      │     NothingToBoard  ──► next = now + 30 min          ← a WAIT, NOT a failure               :477-488
      │     Failed          ──► failures++ ; ≥3 → DROP THE ROW, others carry on ; else +5 min      :490-506
      ├─ PublishSchedule() · Beep 523 if every row was Ok, else 200                                :509-511
      └─ repeat
```

**Where it writes to the board.** Only inside the two touch-points above — the look (目錄, the feed
icon, one Enter, the X) and the visit (the whole reload) — and nothing at all in between, where it only
sleeps [PetTool.cs:397](../SealTools.Pet/PetTool.cs#L397). The complete action inventory is in
[AUDIT-PET-FLOW.md](../AUDIT-PET-FLOW.md) §2.

### Decision points (the outer loop only)

| Where | Question | True → | False → |
|---|---|---|---|
| `Ready` [598](../SealTools.Pet/PetTool.cs#L598) | Any precondition missing? | stop with a message naming it [286-292](../SealTools.Pet/PetTool.cs#L286) | run |
| `ClaimGame` [538](../SealTools.Pet/PetTool.cs#L538) | Game free, for the look? | `InspectRows` [327](../SealTools.Pet/PetTool.cs#L327) | wait, saying who; cancelled → end the run [340-345](../SealTools.Pet/PetTool.cs#L340) |
| loop [389](../SealTools.Pet/PetTool.cs#L389) | Any live rows left? | sleep to the soonest | "Every row has failed repeatedly" → break [389-394](../SealTools.Pet/PetTool.cs#L389) |
| `SleepUntil` [397](../SealTools.Pet/PetTool.cs#L397) | Cancelled during the wait? | break | compute the due batch |
| [407](../SealTools.Pet/PetTool.cs#L407) | Does the batch hold any row? | visit them | `batch = [soonest]` |
| `ClaimGame` [413](../SealTools.Pet/PetTool.cs#L413) | Game free, for the visit? | `Visit` [418](../SealTools.Pet/PetTool.cs#L418) | wait; `next[row]` stays in the past; cancelled → break |
| `Visit` [420](../SealTools.Pet/PetTool.cs#L420) | Was the visit cut by a stop? | throw — "may not have been fed. Check it in game." [431-435](../SealTools.Pet/PetTool.cs#L431) | fold the results back |
| [453](../SealTools.Pet/PetTool.cs#L453) | The row's reload was **Ok**? | `next = now + min(pet, load) + WaitAfterEmpty`; failures = 0 | check next |
| [477](../SealTools.Pet/PetTool.cs#L477) | **NothingToBoard**? | `next = now + 30 min`; not a failure | Failed |
| [498](../SealTools.Pet/PetTool.cs#L498) | Failures reached `MaxFailures` (3)? | **drop that row**; the others carry on [502](../SealTools.Pet/PetTool.cs#L502) | `next = now + 5 min` [505](../SealTools.Pet/PetTool.cs#L505) |

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

Its line numbers are from 2026-10-06 and have drifted since (the visit was added after it was written);
where a number does not line up, read the named method.

**Worth a verdict:**

1. **A stop mid-visit cannot repair the game** — a drag cut between its press and its release leaves the
   stack where the cursor was, so the tool hands the player a note rather than fixing it
   [PetTool.cs:424](../SealTools.Pet/PetTool.cs#L424). The schedule is never left wrong (a restart
   re-inspects [PetTool.cs:314](../SealTools.Pet/PetTool.cs#L314)); it is the game that may be.

2. **The gate is taken per look and per visit, never held in between** [PetTool.cs:266](../SealTools.Pet/PetTool.cs#L266)
   — this is what makes the feeder resident. But the deferral leaves `next[row]` in the past
   [PetTool.cs:409](../SealTools.Pet/PetTool.cs#L409), so a foreground tool that holds the game for a
   long time defers the same row over and over.

3. **A row is dropped after 3 failures** [PetTool.cs:498](../SealTools.Pet/PetTool.cs#L498), so one bad
   calibration costs that row only. Worth confirming 3 is the count you want for something that runs
   unattended for days.

4. **`NothingToBoard` is a 30-minute wait, not a failure** [PetTool.cs:477](../SealTools.Pet/PetTool.cs#L477)
   — a bag holding no feedable pet keeps the row's place and is never counted against it.

5. **The quit hotkey is disabled for this tool** [PetTool.cs:271](../SealTools.Pet/PetTool.cs#L271), so
   its own Stop is the only way to end it. That is deliberate (one press must not kill a schedule that
   is feeding four pets) but it means the global hotkey is not a panic button for the feeder.

6. **`Ready` checks every ticked row, not just the first** [PetTool.cs:615](../SealTools.Pet/PetTool.cs#L615),
   so a switched-off half-set-up row no longer blocks a run that never touches it.

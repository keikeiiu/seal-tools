# Cursor investigation — `SetCursorPos` refused inside the launcher (2026-09-10)

Debug log for the "Gem Composer / Test Click stopped landing" bug. Written so the next session can
pick it up without re-deriving it. **Fixed** — the tool no longer positions the cursor with
`SetCursorPos`; the refusal itself is still unexplained but no longer load-bearing.

---

## Symptom

Pressing **Test Click** (Calibrate Gem) or running the **Gem Composer** did nothing useful: the N
radio never got selected. Intermittent — the same build worked at 00:48, failed at 01:00.

## The bug's own defect

`GemPointer.To` / `SetLogicalCursorPosition` **discarded the return value**, so a refused move was
indistinguishable from a successful one and the tool clicked wherever the mouse happened to be. That
is why the symptom was "clicked the wrong place" rather than "stopped and complained".

## The probes, in order

Each of these was its own commit and wrote one line per Test Click to `logs/arduino_debug.txt`:
`2b12475` target/port/read-back/foreground · `8e0a892` accepted · `a25c753` immediate + after 300 ms ·
`9cc1d47` DPI awareness context · `83387f7` Win32 error + clip rect · `3f154a4` immediate retry ·
`f1ca65c` `SetPhysicalCursorPos` fallback · `87323e1` background thread.

The launcher also gained `WindowFinder.IsMinimized` (`8c3d4aa`): a minimized window reports
`-48000,-48000 0×0`, so every derived coordinate is garbage. Several of the early "failures" were
exactly this — the log proved it (`client=(-48000,-48000,0x0)`).

| # | Probe | Result | Ruled out |
|---|---|---|---|
| 1 | Compute the target from the live window | `(729,695)` logical = `(1094,1043)` physical; the N radio is at ~`(1095,1040)` | wrong coordinates |
| 2 | Screenshot the N point | tuning/gem radio exactly where predicted | calibration drift |
| 3 | `SetCursorPos(729,695)` from PowerShell + read-back | `ok=True`, cursor lands and stays | the API itself |
| 4 | Same, from a copy of `powershell.exe` renamed `SealTools.Launcher.exe` | `ok=True` | process *name* blacklist |
| 5 | Send Arduino `D 100 0` and measure | cursor moved exactly **+100 px** | serial + HID path; scale is 1:1 |
| 6 | App log after Test Click | `target=(729,695) accepted=False immediate=(1931,801)` | "cursor was right, click missed" — the move was refused |
| 7 | `GetThreadDpiAwarenessContext()` in the app | `24592`; an unaware PowerShell also reports `24592` and `AreDpiAwarenessContextsEqual(ctx, UNAWARE)=True` | thread left DPI-aware (`7f9e1c7` theory) |
| 8 | Process owning the foreground window calls `SetCursorPos` | `ok=True` | foreground-ownership restriction |
| 9 | Thread holding mouse capture (`SetCapture`) | `ok=True` | mouse-capture restriction |
| 10 | Compare integrity levels | launcher `S-1-16-8192` (medium), powershell medium, **game `S-1-16-12288` (high/elevated)** | — (see below) |
| 11 | Compare session + owner | both session 1, both `KEI-AM5-PC\keiwa` | session / user mismatch |
| 12 | Immediate retry | `retry=False` | transient refusal |
| 13 | `SetPhysicalCursorPos` fallback | `phys=False` | "wrong API" |
| 14 | Same call on a thread-pool thread | `bg=False` | UI-thread-specific cause |
| 15 | Cursor clip rect | `-1067,0,3627x1707` (whole virtual desktop) | clip rect excluding the target |
| 16 | Win32 error | `err=0` | no error reported (SetCursorPos is not documented to set one) |
| **17** | **Hold + drive COM5 in a throwaway process, then `SetCursorPos`** | **`ok=30/30`; a second process 29/30 while the port was held** | **the anti-cheat reacting to the Arduino port — see below** |
| 18 | Park the cursor exactly where it sat during the failures (`2049,741`, over an Explorer window) and call `SetCursorPos(729,695)` | `ok=12/12` (and `12/12` parked over the game) | cursor start position / window under the cursor |
| 19 | Decode `dpiCtx=24592` | `AreDpiAwarenessContextsEqual(ctx, UNAWARE)=True`; PerMonitorV2 is a **different** handle (`34`) | DPI awareness, independently re-confirmed |

Evidence line, verbatim:

```
01:10:42 test-click name=N port=COM5 open=True baud=115200 target=(729,695) accepted=False
         immediate=(2049,741) after300ms=(2049,741) dpiCtx=24592 err=0 retry=False phys=False
         bg=False clip=-1067,0,3627x1707 fg="Seal Tools V2"
```

### Probe 17 — the last untested difference, and it is refuted

Every successful external test up to then had the COM port closed, so the standing hypothesis was
that the game's anti-cheat reacts to the process driving the Arduino. It does not:

- A throwaway PowerShell process **opened COM5, waited out the boot delay, sent `D 0 0`, then called
  `SetCursorPos` 30 times while still holding the port: 30/30 accepted.**
- While that process kept the port open and kept writing to it, a **second** process (no port) called
  `SetCursorPos` 30 times: **29/30 accepted** (the one miss was `accepted=True` landing 2 px short).
- Control with the port free: 30/30.

So neither holding the port nor driving the device from the calling process changes anything. The
refusal remains specific to the **launcher's process**, and it is not the port.

## Where it stands

- **Our own defect: fixed** — the placement is verified now, and a cursor that can't be placed stops
  the tool instead of clicking blind (below).
- **The refusal is still specific to our process and still unexplained.** Ruled out: coordinates,
  calibration, the API, the process name, foreground ownership, mouse capture, session/user,
  integrity level, DPI awareness, the clip rect, retries, a different API, a different thread, the
  Arduino port, and the cursor's starting position. The one lead left unexplored is the difference
  between a WPF app and a console app; it is not worth chasing, because the tool no longer calls the
  API.

### Hypotheses that were wrong (don't repeat them)

- "The game must be in front / focus is the issue" — the game is not the foreground window in the
  failing runs (`fg="Seal Tools V2"`), and a process owning the foreground can still move the cursor.
- "The physical mouse moved the cursor between the move and the click" — the app's own read-back
  shows the move never took effect (`accepted=False`), and the user confirmed they weren't touching
  the mouse.
- "The anti-cheat is reacting to the process driving the Arduino" — probe 17.
- "v1 is started in-game so it never faced this" — v1's `launcher.py` spawns the tool with
  `subprocess.Popen` and is driven from a browser panel, so the game was unfocused for v1 too. That
  claim lived in an older README and is not in any current doc.

## The fix (implemented)

**The cursor is positioned with the Arduino, not `SetCursorPos`.** The Arduino is a genuine HID
mouse; its input cannot be refused, and the project already depends on it for every click and
relative move. `GemPointer.To(ser, target)` now:

1. reads the current position with `GetCursorPos` (this always worked in our process);
2. computes `delta = target − current` in the process's (logical) cursor space;
3. sends `D dx dy`, clamped to 600 px per move, and repeats until within 2 px.

It returns a `CursorPlacement` (ok, steps, final position, reason). **Every caller checks it**:

- `GemComposer.SelectGradeAndRegister` / `AdvanceGrade` call `Fail(...)`, which stops the tool and
  puts the reason on the card, instead of clicking somewhere arbitrary.
- The calibrate test buttons report it in the hint and skip the click.

Measured gain is exactly 1:1 (`D 100 0` → +100 px, and 250/500/600 px all land exactly), so a normal
placement converges in **one** move.

### The one non-obvious detail: the firmware walks long moves out in chunks

`arduino/seal_mouse.ino` implements `D dx dy` as a loop of **10-px `Mouse.move` calls with
`delay(1)` between them**. A 600 px move therefore keeps arriving for tens of milliseconds after
`SerialPort.Write` returns. With a fixed 20 ms settle the loop read a **stale** position, computed a
correction from it, and stacked a second move on top of a first that was still running — which is how
a far target ended up at `(340,445)` after six moves:

```
step 1 : at (2400,1400) err=1671 sent=(-600,-600) -> (2000,1001) actualDelta=(-400,-399)
step 2 : at (1990,991)  err=1261 sent=(-600,-296) -> (1560,560)  actualDelta=(-430,-431)  <- y overshoots
```

`To` now polls `GetCursorPos` after each move until two consecutive polls agree (15 ms apart, bounded
at 24 polls) instead of sleeping a fixed amount. Verified through the launcher's real UI: from
`(2400,1400)` it reaches `(729,695)` in 3 moves, and from a nearby point in 1.

### Instrumentation

The per-probe logging (`accepted`/`retry`/`phys`/`bg`/`dpiCtx`/`clip`) is gone, along with the now
dead `WindowFinder.ThreadDpiAwarenessContext`, `CursorClip` and the error-reporting
`SetLogicalCursorPosition` overload. One line per Test Click remains:

```
01:20:40 test-click name=N target=(729,695) ok=True steps=3 final=(729,695) fg="Seal Tools v2"
```

`SetCursorPos`/`SetPhysicalCursorPos` survive only behind the calibrator's **Debug Cursor (logical)**
and **Debug Physical** buttons, which exist to compare the two APIs by hand.

## Reproducing

```
logs/arduino_debug.txt        one line per Test Click (target / ok / steps / final position / fg)
logs/captures/                screen captures used to verify what is on screen
```

Throwaway probes used for this round (not part of the repo): a PowerShell `SetCursorPos` loop with an
optional `-Hold` on COM5, a window-under-point reporter, a `D`-move gain measurement, and a UI
Automation driver for the launcher's Test Click button.

---

## Still open

Why the OS refuses `SetCursorPos` for the launcher's process — intermittently, while another process
under the same user/session/integrity succeeds — is **unanswered**. It no longer matters to the tool,
but if it ever resurfaces the shortest next step is a minimal WPF app that only calls `SetCursorPos`,
to see whether the refusal follows the framework rather than this code.

One late clue (user-reported 2026-09-10, not yet reproduced in the logs): after the launcher was
restarted for the fix, **both Debug Cursor buttons succeeded** — `ok=True`, cursor landing on the
target. The failures at 01:00–01:10 came from an instance that had been running a while. That points
at a *runtime-state* degradation inside the process rather than a property of the process, and would
be the first thing to look at if the refusal ever matters again. It does not change the fix: the
placement path never calls the API.

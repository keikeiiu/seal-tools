# Cursor investigation — `SetCursorPos` refused inside the launcher (2026-09-10)

Debug log for the "Gem Composer / Test Click stopped landing" bug. Written so the next session can
pick it up without re-deriving it. **Open** — root cause not yet identified; the fix is proposed but
not implemented.

---

## Symptom

Pressing **Test Click** (Calibrate Gem) or running the **Gem Composer** did nothing useful: the N
radio never got selected. Intermittent — the same build worked at 00:48, failed at 01:00.

## Instrumentation added along the way

Each of these is its own commit and writes one line per Test Click to `logs/arduino_debug.txt`:

- `2b12475` — log target, port, cursor read-back, foreground title.
- `8e0a892` — log whether `SetCursorPos` was accepted.
- `a25c753` — log the cursor immediately after the move **and** after the 300 ms wait.
- `9cc1d47` — log the thread's DPI awareness context.
- `83387f7` — log the Win32 error and the cursor clip rect.
- `3f154a4` — log whether an immediate retry succeeds.
- `f1ca65c` — log `SetPhysicalCursorPos` as a fallback.
- `87323e1` — log the same call from a background thread.

The launcher also gained `WindowFinder.IsMinimized` (`8c3d4aa`): a minimized window reports
`-48000,-48000 0×0`, so every derived coordinate is garbage. Several of the early "failures" were
exactly this — the log proved it (`client=(-48000,-48000,0x0)`).

## The probes, in order

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

Evidence line, verbatim:

```
01:10:42 test-click name=N port=COM5 open=True baud=115200 target=(729,695) accepted=False
         immediate=(2049,741) after300ms=(2049,741) dpiCtx=24592 err=0 retry=False phys=False
         bg=False clip=-1067,0,3627x1707 fg="Seal Tools v2"
```

## Where it stands

- **Our own defect:** `GemPointer.To` / `SetLogicalCursorPosition` discard the return value, so a
  refused move is indistinguishable from a successful one and the tool clicks the wrong place.
- **The refusal is specific to our process.** The identical call succeeds from PowerShell under the
  same user, session and integrity level, with the game running.
- **Not identified:** why the OS refuses our process. The one difference never tested is that the
  launcher holds the **Arduino COM port** open; every successful external test had it closed. A
  plausible (unverified) explanation is the game's anti-cheat reacting to the process driving the
  Arduino.

### Hypotheses that were wrong (don't repeat them)

- "The game must be in front / focus is the issue" — the game is not the foreground window in the
  failing runs (`fg="Seal Tools V2"`), and a process owning the foreground can still move the cursor.
- "The physical mouse moved the cursor between the move and the click" — the app's own read-back
  shows the move never took effect (`accepted=False`), and the user confirmed they weren't touching
  the mouse.
- "v1 is started in-game so it never faced this" — v1's `launcher.py` spawns the tool with
  `subprocess.Popen` and is driven from a browser panel, so the game was unfocused for v1 too. That
  claim lived in an older README and is not in any current doc.

## Proposed fix (not implemented)

**Move the cursor with the Arduino instead of `SetCursorPos`.** The Arduino is a genuine HID mouse;
its input cannot be refused, and the project already depends on it for every click and relative
move. Concretely, in the one place that positions the cursor:

1. read the current position with `GetCursorPos` (this works in our process — every read-back was
   correct);
2. compute `delta = target − current` in logical screen coordinates;
3. send `D delta.x delta.y` (clamped per step), repeat until within ~2 px.

Measured scale is exactly 1:1 (`D 100 0` → +100 px), so the loop converges in one or two iterations;
iterating also absorbs any pointer-acceleration non-linearity.

Independently of that, **never ignore the move's result**: if the cursor cannot be placed, stop the
tool and say so on the card rather than clicking somewhere arbitrary.

## Reproducing

```
logs/arduino_debug.txt        one line per Test Click, with all the fields above
logs/test_click_debug.txt     target / client rect / foreground titles per Test Click
logs/captures/                screen captures used to verify what is on screen
```

---

## Prompt for the next session

> Continue the Seal Tools v2 cursor bug in `v2/docs/CURSOR-INVESTIGATION.md`. Read that file first —
> it lists every probe and what each one ruled out, so don't redo them.
>
> Summary: `SetCursorPos` (and `SetPhysicalCursorPos`) return **false** from the launcher's process,
> intermittently, while the identical call succeeds from PowerShell under the same user, session and
> integrity level. The move is silently ignored, so the Arduino click lands wherever the mouse was.
>
> Two things to do, in this order:
> 1. **Test the last untested difference:** hold the Arduino COM5 port open in a throwaway process
>    and then call `SetCursorPos` — every successful external test so far had the port closed. If it
>    fails while the port is held, the anti-cheat is reacting to the process driving the Arduino.
> 2. **Implement the fix regardless of the answer:** position the cursor with the Arduino (relative
>    `D dx dy` in a closed loop against `GetCursorPos`, which works fine in our process) instead of
>    `SetCursorPos`, and stop ignoring the result — if the cursor can't be placed, stop the tool and
>    report it on the card instead of clicking somewhere arbitrary. The Arduino scale is 1:1
>    (`D 100 0` → +100 px), so the loop converges in one or two steps.
>
> Constraints: do not re-add a focus-click or a Win32 focus API; keep `gem.movements` untouched; the
> test instrumentation in `MainWindow.xaml.cs` (the `accepted`/`retry`/`phys`/`bg` probes) should be
> folded into a clean single check once the fix lands.

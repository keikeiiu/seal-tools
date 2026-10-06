# Plan — lifecycle state, and who may write to the board

Written 2026-10-06. **NOT BUILT — plan only.**

Written after an external read-only audit of `cc6551b` (2026-10-06) whose findings were then checked
against the source. Four of them are real and are itemised below; the rest it found were already open in
[TODO.md](TODO.md) and are listed at the end so they are not counted twice.

**The four are independent.** Each is a separate commit, smallest first, and each says how it is
verified — because "it compiles" is not verification for any of them.

---

## Item 1 (S) — Hold Space: a click during STOPPING restarts it

**The one with a symptom the player has actually hit**, and a hole in the 2026-10-02 fix. That fix
covered *"a click during **Starting** cancels"*. The inverse was left open.

**Evidence.** `StopTool` clears the current id **synchronously**, before the worker has left its loop
([LauncherService.cs:509](../SealTools.Launcher/LauncherService.cs#L509)):

```csharp
_running.Remove(target);
if (_currentId == target) _currentId = null;   // ← the button now reads "Hold Space"
...
running.Cts.Cancel();                          // ← but the loop has not stopped yet
```

The card's button asks exactly that question — `CurrentId == "holdspace"` — so between those two lines a
second click takes the **start** branch and presses Space again. **The 2026-10-02 fix made this window
easier to hit**: `RefreshStatus()` is now called directly from the click handler, so the label flips to
"Hold Space" at once instead of up to 750 ms later.

**The fix.** An explicit stop-in-flight state, which is what the player asked for in the first place:

- `LauncherService` exposes whether a stop for an id is in flight (a flag set before the clear, cleared
  in the continuation that already exists).
- The toggle treats *starting*, *running* and *stopping* as one engaged state: a click in any of them
  stops or cancels, never starts.
- The button reads **"Stopping…"** while it lasts, the same way it reads "Starting…" — a label that
  says what is true (see §4 of the audit, the same lesson).

**Verified by:** a test on `PortGate`-style state is not possible (this lives in the launcher), so the
check is a **unit test on the decision**, extracted the way `SpammerOrder` was — a pure function
`(isCurrent, isStarting, isStopping) → start | stop`. Plus a live click-test: stop, then click again
immediately, and confirm the run does **not** come back.

---

## Item 2 (S) — `arduino.port` is loaded and never read

**Evidence.** `ArduinoConfig.Port` is documented *"empty = auto-detect"* and is merged from `local.yaml`
at [ConfigLoader.cs:312](../SealTools.Core/Config/ConfigLoader.cs#L312) — and **read nowhere else in the
project**. `ArduinoPortAsync` always calls `Arduino.Find(Config.Arduino.Vid, Config.Arduino.Pid)`. So a
player who names a port gets it silently ignored.

This is the same class as the four `PetConfig` properties fixed in entry 55: a setting that loads,
looks honoured, and does nothing.

**The fix — honour it, rather than delete it.** A named port is genuinely useful (two boards, a COM
number the OS reassigned, a board whose ID is not reported properly — which is exactly why `Arduino.Find`
already carries a name-based fallback). So:

- `ArduinoPortAsync` uses `Config.Arduino.Port` when it is non-empty, and falls back to VID/PID
  discovery when it is not — or when the named port is absent.
- **A named port that does not exist is REPORTED, not silently fallen back from.** Quietly using a
  different board than the one named is the failure this whole item is about.

**Verified by:** a test on the resolution helper (`named port → that port; empty → discovery; named but
missing → the reported error`), extracted as a pure function so it needs no board.

---

## Item 3 (S) — `_running` is a `Dictionary` with no lock

**Evidence, and the code says it itself.** `_running` is a plain `Dictionary`, and
[LauncherService.cs:138](../SealTools.Launcher/LauncherService.cs#L138) describes the neighbouring
`PortGate` as *"Thread-safe, **unlike `_running`**"*. It is:

- **read** by the UI timer every 750 ms, per card (`StateFor`);
- **written** by `StartToolAsync` on the UI thread;
- **written** by the self-end continuation ([line 428](../SealTools.Launcher/LauncherService.cs#L428)),
  which runs on a **worker thread** when a tool finishes on its own.

Concurrent read and write on `Dictionary` is undefined behaviour — a corrupted lookup or a spin, not a
clean exception. It has not been seen because the window is small and the pet feeder is the only tool
that ends by itself.

**The fix.** One private lock object, taken at all seven access sites. **A lock rather than
`ConcurrentDictionary`**, deliberately: two of the sites are check-then-act — the self-end continuation
does `TryGetValue` then `Remove` and compares the CTS — and `ConcurrentDictionary` makes each call
atomic without making the *pair* atomic. It would look like a fix and leave the same race.

**Verified by:** a stress test that starts and ends a fake tool from two threads while reading
`StateFor` in a loop. That test is only meaningful with a **fake runner**, which needs Item 4's seam —
so either this lands after Item 4, or the test is written against `_running` alone by extracting the
registry.

---

## Item 4 (M) — calibration input bypasses the gate entirely

**Evidence.** `MainWindow` never references `.Gate` — **zero** occurrences — while **19** call sites in
it obtain the port and write to it directly ([MainWindow.xaml.cs](../SealTools.Launcher/MainWindow.xaml.cs),
`ArduinoPortAsync` at 1188, 2894, 2954, 3041, 3096, 3184, 3707, 4980, 5022, 5437, 5604, 6661, 8304,
8427, 8833, 9107, 9180, 9748 …).

So a Test Click, a "hold space" test, or a calibrate-step button can interleave with a running tool's
gesture — a pet drag being the one that loses something real if it is interrupted (it drops a stack).

**The fix, and the size is why this is last.** Not the arbiter from
[PLAN-SCHEDULING.md](PLAN-SCHEDULING.md) §4 — that is the L version and it is not needed to close this.
The smallest correct thing:

- **One accessor on `LauncherService` that the UI uses for test/calibration input**: it refuses while **a
  tool holds the gate**, with a message saying which, and otherwise takes the gate for the duration of
  the test.
- **19 mechanical call-site edits** to use it. Mechanical, but 19 of them, which is what makes this an M
  rather than an S — and each one is a place a mistake is silent.

**Verified by:** the guard is testable on the service (refuse-while-held), and the 19 sites are checked
by grepping that no UI call site still reads the port directly — the same check this item was found by.

**Risk, stated plainly:** a calibration test that now *refuses* is a behaviour change the player will
meet. The refusal has to say what is running and how to stop it, or it reads as a broken button — which
is the failure this repo has paid for repeatedly.

---

## What is deliberately NOT here

Both were found by the same audit and both were **already open** — listed so they are not counted twice:

- **The config read-modify-write race.** Entry 52 fixed the temp-file collision and says in as many words
  that the lost-update half is *"a design question, not a bug fixed here"*. The audit independently
  proposes the same fix (`lock → read latest → mutate → write`), and that remains a separate piece of
  work with its own design.
- **Stop releasing the mouse during a pet drag.** Open in TODO.md from the 2026-09-22 audit. Closing it
  properly means **making a complete gesture the unit of ownership** — which is
  [PLAN-SCHEDULING.md](PLAN-SCHEDULING.md) §4, and it needs the arbiter rather than a guard.

And one where the audit **overstates**: *"screen capture can read a window covering the game."* The pet
path already guards this — [PetTool.cs:886](../SealTools.Pet/PetTool.cs#L886) and
[:1661](../SealTools.Pet/PetTool.cs#L1661) both check `ForegroundWindow() != hwnd` and refuse. The claim
holds only for launcher-side scans (the food scan checks that the window exists, not that it is in
front), so if it is taken up at all it is a fifth, smaller item.

---

## Order

| | Item | Size | Why here |
|---|---|---|---|
| 1 | Hold Space Stopping state | S | The only one with a symptom the player has hit |
| 2 | `arduino.port` honoured | S | A setting that does nothing, and the fix is contained |
| 3 | `_running` under a lock | S | Real, undefined behaviour, small window |
| 4 | Calibration takes the gate | M | 19 call sites, and it changes behaviour the player meets |

One commit each. Item 4 is the only one that can surprise the player, so it lands last and with the
refusal message written first.

# Plan — a second input backend: AutoHotInterception, alongside the board

Written 2026-10-10. **DESIGN ONLY — nothing installed, no driver added, no code changed.**

The ask: an **alternative to the Arduino board** for driving the game, with the user choosing between
them — *"give user option to choose from AHI / board"*.

Sources read for this plan: [evilC/AutoHotInterception](https://github.com/evilC/AutoHotInterception)
and [oblitum/Interception](https://github.com/oblitum/Interception), both READMEs, fetched 2026-10-10.
Everything quoted from them is their claim, not a measurement on this machine — §8 keeps that line clear.

---

## 1. The two mechanisms

| | the board (today) | AutoHotInterception |
|---|---|---|
| what it is | an Arduino Pro Micro acting as a **real USB HID** mouse/keyboard | a **kernel-mode filter driver** (Interception) that injects into the keyboard/mouse class stack |
| the host talks to it over | a COM port, one command per line | a .NET assembly (`AutoHotInterception.dll`) over the driver, in-process |
| input looks like | genuine hardware | genuine input *below* most software hooks — Windows and apps cannot tell it is synthetic |
| needs | the board, a flash, a free COM port | **admin install of a driver**, admin rights to use |
| nothing extra to buy | — | **yes, if the driver installs** ← the whole question |

The mechanism that matters: AHI's own README says injected output *"will be totally invisible to windows
(And all apps running on it)"*, and AHK `$`-prefixed hotkeys **cannot** tell it is synthetic. That is the
same property the board buys — which is why this is a credible alternative and not a toy.

## 2. What AHI actually offers (from its README)

**Synthesis — the half we need:**

| call | what it does |
|---|---|
| `SendKeyEvent(keyboardId, scanCode, 1\|0)` | key down / key up — so a held key is possible |
| `SendMouseButtonEvent(mouseId, button, 1\|0)` | button 0 left, 1 right, 2 middle, 5/6 wheel; state `1`/`-1` for wheel up/down |
| `SendMouseMove(mouseId, x, y)` | **relative, in "Mickeys"** — *not* pixels |
| `SendMouseMoveAbsolute(mouseId, x, y)` | absolute, in a `0..65535` space that maps to **all** monitors |
| `MoveCursor(x, y [, coordMode, mouseId])` | move the cursor to a screen or window coordinate |

**Detection** (subscribe/block per device) is the other half, and **we do not want it** — see §6.

**Device identity:** Interception IDs are `1..10` for keyboards, `11..20` for mice, resolved from a
VID/PID (`GetKeyboardId`, `GetMouseId`) or a device handle. Its README is explicit about a limitation
worth reading twice:

> If you unplug / replug a device, or go into hibernate and resume, the Interception ID of a device will
> increase by 1. If the ID of a device goes above 10 (For keyboards) or 20 (For Mice), **The device will
> completely cease to function until the next reboot** — not only in AutoHotInterception, but in Windows
> also. There is nothing I can do to fix this issue, it is a limitation of the Interception driver.

For a tool whose pet feeder **runs for days and survives sleep**, that is a first-class design constraint,
not a footnote.

**Install:** `install-interception.exe` must be run **from an admin command prompt** (not double-clicked),
then `install-interception.exe /install`. The Interception README says it is built on WDK 7.1 and was
*"tested from Windows XP to Windows 10"*.

---

## 3. Why it fits this project

1. **Same effect as the board.** The board exists because the process is refused its own cursor placement
   (`SetCursorPos` returns `ok=False`, `COORDINATES.md:106-114`) and the game must see genuine input.
   Interception injects at the driver, so the OS cursor moves as if from a mouse — the same thing the
   board achieves, without hardware.
2. **It may remove two of the hardest calibrations.** `MoveCursor` places the cursor at an absolute
   coordinate, which could delete the closed loop in `HidPointer.To` and the **hand-tuned per-machine raw
   counts** (`gem.movements`). `DESIGN.md:46-49` records the cost of those counts today: *"valid until the
   board, the pointer speed or the in-game display changes"*.
   **But note the units:** relative moves are in Mickeys, which is the same class of problem as the
   board's raw counts. Only the *absolute* path is a genuine simplification.
3. **No board to buy, flash, or keep plugged in** — the biggest barrier to a new user disappears, and the
   firmware-version gating (`FirmwareVersion`, protocol level 2, the pet's drag) becomes board-only.

---

## 4. The seam it forces — and the repo has none

This plan cannot be built before the codebase gets its **first interface**. Measured earlier today:
**zero `interface` declarations across 22,859 lines**, and the input layer is:

- `HidPointer` — static, five methods, board-specific;
- **18 raw `SerialPort.Write` sites**; `E`, `K`/`F`, `Q`/`Z`, `P`/`U` and `V` bypass `HidPointer`;
- `InputLease` carries a **`SerialPort`** (`InputArbiter.cs:93`) — the arbiter's unit of ownership *is the
  port*;
- `PortGate`/`InputArbiter` decide who may write to that port;
- `FirmwareVersion` gates the pet feeder's drag (`LauncherService.cs:475-479`);
- `HeldKeys` + the firmware's host-gone release provide the failsafe for held keys (§5).

So the work is: **`IInputBackend`** (key down/up, button down/up, move relative, move absolute, place at
a point, wheel, liveness) with `BoardBackend` and `AhiBackend` behind it. `PortGate` becomes a gate on
*the input device* rather than on a serial port — which is precisely the Reading-A/Reading-B tension
`PLAN-ARBITER.md` §6 left open, arriving on a schedule nobody chose.

This is the same seam `PLAN-AGENT.md` §5 asks for. **It is worth building once, for both reasons.**

---

## 5. The safety regression, and it is the serious one

The board's failsafe is **in firmware**, and it is the one thing this repo can rely on when the host dies:

> A held key must not survive the host. If the launcher dies while the spacebar or the left button is
> down, the board releases both when the serial port closes. — `DESIGN.md:56-57`

**AHI has no equivalent.** It injects from the host process; if that process is killed, nothing sends the
key-up. A stuck left button follows the real cursor and drops whatever it is over on the next press — the
exact hazard `HeldKeys` exists for, and the pet's food drag is the thing most at risk.

Mitigations, none as strong as firmware:
- a `Drop` guard type that releases on every unwinding path (this is the Rust-invariant from
  `PLAN-V3-RUST.md` §3, arriving early);
- a shutdown handler for a graceful exit;
- **nothing for a hard kill or a crash** — which is exactly the case the firmware covers.

**Therefore: the board keeps the failsafe and AHI must be documented as not having it.** That belongs in
`USER_GUIDE.md` and its zh-TW twin, in the same words, if this is built.

---

## 6. What we should not take from AHI

AhI's README carries a blunt warning: because Interception sits below Windows, **blocking** goes so deep
it can block `CTRL+ALT+DEL`; you can lock up all input and need the reset button; *"the best insurance
policy is to have another keyboard or mouse handy"*.

**That entire risk belongs to the detection half (subscribe + block), which this project does not need.**
We only synthesise. Written down because the warning is the first thing anyone reading AHI will find, and
the answer is: we use half the library, and the dangerous half stays unused. If a future feature wants
per-device blocking, this plan does not cover it and should not be stretched to.

---

## 7. Shape of the change

```
config/defaults.yaml
    input:
      backend: board            # board | ahi
      ahi:
        mouse_vid / mouse_pid / mouse_instance     # resolved to an Interception ID at start
        keyboard_vid / keyboard_pid / keyboard_instance
```

- **Backend resolved once at load**, like `game.variant` is (`PLAN-US-CLIENT.md` §2.1) — not per call site.
- **The Setup tab** reports which backend is active and, for AHI, whether the driver and the device were
  found — the same "one resolution, two readers" rule entry 63 established for the Arduino tab.
- **`Ready()` refuses with a pointed message** when the chosen backend is unavailable, rather than failing
  mid-run.
- **No firmware change**, so no reflash and no `FirmwareSketchTests` bump — but also no protocol level to
  report, since there is no board to ask.

---

## 8. Risks, in the order I would want them answered

1. **Does the driver install at all on this machine?** It is built on **WDK 7.1** and tested XP→Win10.
   Windows 11 with Secure Boot and Memory Integrity enabled is a different world; that combination
   famously refuses older drivers. **This is the single question that decides whether the plan is worth
   anything**, and it is answered by one attempt, not by reading.
2. **Do injected events reach this game?** The docs disagree and the disagreement is exactly this
   question: `v2/README.md:141-143` says *"This title is an OS-cursor game (early Windows, **not
   raw-input**)"*, while `DESIGN.md:42-44` says *"a game that reads raw input sees `SetCursorPos` moves as
   teleports"*. If the game is OS-cursor, driver-injected movement should behave exactly like the board's.
   If it reads raw input, whether it accepts synthetic driver input is unknown. **One run in the game
   settles it.**
3. **Anti-cheat.** A kernel filter driver is a known artefact — it is named in Interception's own README
   under use cases *"in game applications like BOTs"*. A physical Arduino Pro Micro is indistinguishable
   from a real keyboard. **So on the axis that matters most to the user — account safety — the board is
   probably the safer of the two**, and this plan must say so rather than sell the convenience.
4. **Device-ID churn** (§2) versus a resident tool that runs for days through sleep.
5. **Licensing and distribution.** Interception is dual-licensed: LGPL for non-commercial use, *with
   rights to distribute the driver binaries once communication happens solely through the library and its
   API*. The conservative answer is **do not bundle it** — `INSTALL.md` tells the user to install it, the
   same way it tells them to flash the board.
6. **Which library to consume.** `AutoHotInterception.dll` is a .NET assembly written to be driven from
   AutoHotkey (loaded via `CLR.ahk`), so consuming it from .NET 8 is an assumption, not a given. The
   alternative is P/Invoking `interception.dll` directly — a small C API, but our own structs and context
   handling. **⚑ decision; the spike tests the first because it is the least code.**

---

## 9. Order of work

0. **The spike, before any product code — and it answers the plan's fate.**
   A throwaway console app that: installs nothing itself, checks whether the driver is present and
   whether an Interception device is visible; then moves the cursor to a known point, clicks, and sends
   one key — **in the game**. It reports: did the driver install, did the device resolve, did the cursor
   land where aimed, did the game register the click, did the game's own pointer follow.
   Everything above §9 is speculation until that runs.
1. **`IInputBackend` + `BoardBackend`** — the refactor with no behaviour change: extract the seam, move
   the 18 write sites and `HidPointer` behind it, keep the board as the only implementation. This is the
   prerequisite for *both* this plan and `PLAN-AGENT.md` §5, and it is worth doing even if AHI is
   abandoned tomorrow.
2. **`AhiBackend`**, synthesis only, with the extra care §5 demands.
3. **Config + the Setup tab**, and `Ready()` refusing clearly.
4. **Docs**: `CONFIG.md`, `INSTALL.md` (driver install, and *do not bundle it*), `USER_GUIDE.md` **and
   `USER_GUIDE.zh-TW.md`** together — including the failsafe difference, because it is a user-visible
   safety property, not an implementation detail.

---

## 10. Open questions ⚑

1. **Is AHI/Interception worth offering at all if the board is safer for the account?** The honest
   answer may be *"only for people who cannot or will not buy a board"* — which is a real audience, and a
   different pitch from *"an equivalent alternative"*.
2. **Absolute placement: use it, or keep the closed loop?** Absolute would delete the hand-tuned counts;
   it also changes what "calibrated point" means, and every existing calibration would need re-testing.
3. **The seam's shape** — a C# interface (available today) or the Rust trait from `PLAN-V3-RUST.md`? The
   seam is wanted by three plans now; it should be cut once, in one language, on purpose.
4. **Default backend.** The board, unchanged — a new backend must never become the default by accident.
5. **What happens to `FirmwareVersion`'s meaning** when there is no board? The pet feeder's drag is gated
   on protocol level 2; on AHI there is no level, so the gate must become backend-aware rather than
   deleted.

---

## 11. What was measured, and what was only reasoned about

**Measured on this machine, today:** the input layer's shape — `HidPointer`'s five methods, the 18 raw
serial writes, `InputLease` carrying a `SerialPort`, the `FirmwareVersion` gate on the pet's drag, and
**zero interfaces** in `v2/`. The failsafe quoted in §5 is the repo's own documented behaviour.

**Read from the projects' READMEs (external, unverified here):** the whole API list and constraint set in
§2, the install procedure, the device-ID limitation, the blocking warning in §6, and the dual licence —
each cited to its source, and none of it tested on this machine.

**Reasoned, not measured:** that driver-injected input reaches this game (§8.2) — the two repo documents
disagree and neither is a test; that `MoveCursor` would remove the closed loop; that the board is safer
against anti-cheat (§8.3), which is an argument from what the two mechanisms *are*, not from anything
observed; and every effort estimate.

**Not attempted:** no driver downloaded or installed, no AHI release downloaded, no console app written,
no config or code touched.

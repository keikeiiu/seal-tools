# PLAN-NATIVE-INPUT — native Interception as the board alternative

**Decision (2026-11-07): AutoHotkey is excluded; the C# AHI assembly is not.** nGuard does not
accept AutoHotkey, so no AHK runtime, AHK script, or compiled AHK executable may be shipped, launched,
or used for testing. AutoHotInterception (AHI) itself ships **two** implementations in its repository:
the AHK v1/v2 library and a **C# .NET assembly** (`C#/AutoHotInterception`). The C# assembly does not
need AutoHotkey and is the only AHI form still allowed here. It still requires the Interception driver,
so nGuard acceptance of the driver remains the Phase 0 gate. If only an AHK/AHI executable was built,
that artifact is rejected with the rest of AHK.

**Proposal.** This plan evaluates a native **Interception** backend (or the prebuilt C# AHI assembly as
a proof of concept) as a replacement for the Arduino Pro Micro board for tools that move the cursor, click, hold keys, or type. It does not change how
tools decide what to do; it changes only where the effects are executed.

> **Read this as a proposal, not as current behaviour.** The shipped input backend is still the
> Arduino board over serial (`v3/core/crates/seal-board`). Interception is not a dependency today.

## 1. Why this is worth measuring

The board is reliable but adds hardware and setup friction:

- a Pro Micro/CH340 board and a COM port;
- a firmware level that must match the host grammar;
- a physical watchdog (3 s host silence) that releases a stuck key/button;
- one board per machine, shared with the game's anti-cheat over the COM port during a run.

Interception is a Windows kernel-mode input filter/injection driver; its mouse and keyboard reports
can be driven from a native process, without AutoHotkey. If nGuard tolerates the driver, it could
provide the same key/mouse effects with no board, no COM port and no firmware version.

The question is not "can it inject?" — it can. The question is whether it can meet this repository's
existing safety, determinism and **nGuard** constraints without making the suite less safe than the
board.

## 2. What the board seam already isolates

The alternative should fit the seam the board already uses, not fork the tools:

| Layer | Current file | What an Interception backend must provide |
|---|---|---|
| Effect interface | `v3/core/crates/seal-tools/src/runner.rs` (`Effects`) | `click`, `key`, `key_fast`, `hold_space`, `release_space`, `enter`, `place`, `click_here`, wheel, waits |
| Board execution | `v3/core/crates/seal-core/src/effects.rs` (`BoardEffects`) | one implementation per backend, behind the same trait |
| Command grammar | `v3/core/crates/seal-board/src/command.rs` | key/button/wheel/move semantics; Interception's primitive is a driver report, not a serial line |
| Cursor placement | `v3/core/crates/seal-board/src/placement.rs` | closed loop: move → settle → read cursor → correct → tolerance/budget |
| Board lifecycle | `v3/core/crates/seal-core/src/board.rs` | probe, open, firmware level, failure lines |
| Config/UI | `v3/core/crates/seal-config/src/registry.rs`, Setup/Arduino tabs | backend choice, device choice, admin/driver readiness |
| Safety | firmware `HOST_SILENCE_MS` watchdog and `Drop` release | equivalent held-input release when the host dies |

Decisive constraints already encoded there:

- **Raw HID counts, never scaled** for movement; placement is closed-loop against the real cursor.
- **Seconds, not milliseconds** in tool waits.
- A held key/button must be released on normal stop, Stop, process exit, and host loss.
- A start must fail closed; a missing backend never silently becomes another backend.

## 3. Integration decision

### Rejected: AHI on AutoHotkey

The AHK v1/v2 library and any compiled AHK executable are rejected: nGuard blocks AutoHotkey. No AHK
runtime, script, or AHI/AHK helper may be part of the product, the test plan, or the release folder.

### Allowed AHI form: the C# assembly

`C#/AutoHotInterception` is a .NET Framework 4.6.1 class library with `Manager`, `MouseHandler` and
`KeyboardHandler`; it calls `interception.dll` directly and exposes `SendKeyEvent`,
`SendMouseButtonEvent`, `SendMouseMove`, `SendMouseMoveAbsolute` and `MoveCursor`. It does not need
AutoHotkey. If this assembly is already built, it is the fastest way to prove the Interception path.

Constraints of that route:

- it is a **library**, not a standalone service; v3's Rust core needs a small .NET helper process
  (or a C# bridge) to host it and a pipe/IPC contract;
- it targets **.NET Framework 4.6.1**, so the helper is a separate runtime/deployment concern;
- it still needs the **Interception driver**, `interception.dll` and admin installation;
- the helper, not the core, owns the Interception context and must implement the same held-input
  release/watchdog as any other backend;
- AHI's MIT license covers the assembly; the Interception driver's license must be reviewed
  separately before redistribution.

### Selected production direction: native Interception in Rust

For a shipped v3 backend, prefer a native Rust adapter (`seal-input-interception`) over a .NET helper:
one process model, no .NET Framework dependency, Rust tests around report encoding, and the same
release binary. The C# AHI assembly is the **fast proof-of-concept path**; it is not a reason to add a
permanent .NET sidecar unless it proves clearly simpler and passes all safety gates.

Both routes use the same `Effects` seam and the same backend name, `interception`.

## 4. Safety design (the part the board currently wins)

This is the reason the plan is not a one-line backend swap.

### 4.1 Held-input release

The board firmware releases a held spacebar/left button on:
- normal `Drop`;
- `tool.stop`;
- serial port close;
- **3 seconds of host silence** (hung host).

Interception has no equivalent physical watchdog. The native adapter must provide:

1. an RAII release on every normal path;
2. release on Stop and on process exit;
3. a **separate watchdog process** that monitors the core/helper heartbeat and releases if the
   process is killed;
4. a documented recovery step if the watchdog itself dies;
5. no auto-start at Windows login; manual launch only.

If (3) cannot be made reliable, the Interception backend must be labelled "experimental; a physical
keyboard/board must remain available" and must not replace the board for normal use.

### 4.2 Input lockout

Interception can sit below Windows input. The adapter must:

- **never** install blocking filters on the user's normal keyboard/mouse;
- never subscribe/block `Ctrl`, `Alt`, `Del`, or the emergency release key;
- use injection only, not global blocking;
- expose an emergency physical escape path in the docs;
- require an explicit user confirmation before the first Interception run.

### 4.3 Fail-closed backend selection

- Settings: `input.backend: board | interception` (new field; default stays `board`).
- Selecting `interception` must not disable the board probe/Setup; switching back is one action.
- Interception start checks: driver installed, device discovered, admin rights if required, target IDs
  stable; failure is a named refusal, never a fallback to the board.
- Existing board users see no change.

## 5. Proposed architecture

```text
tool decision half (seal-domain)
        ↓ Effects trait
seal-tools runner
        ↓
   input backend selected in config
     ├── BoardEffects          → seal-board Transport (`D/P/U/...` serial) → firmware → HID
     └── InterceptionEffects   → seal-input-interception → Interception driver → Windows input
```

New pieces:

- `v3/core/crates/seal-input-interception/` (or `seal-input` with an `interception` module)
  - `open(device)` / `close()`
  - `send_key(scan_code, down)`
  - `send_mouse_button(button, down)`
  - `send_mouse_move(dx, dy)` and `send_mouse_move_absolute(x, y)` behind one measured interface
  - `send_wheel(notches)`
  - `device_ids()` / `find_device(vid, pid)` and a stable refusal when absent
- `v3/core/crates/seal-core/src/input.rs`
  - `InputBackend` selection from merged config
  - `InterceptionEffects` implementing the same `Effects`
  - a backend-neutral `place` closed loop using the existing cursor `Cursor` trait
- Config:
  - `input.backend` in the portable setup block, not `machine/`? The choice is per PC but travels;
    decide in Phase 1; no silent migration.
  - `input.interception.mouse_id`, `input.interception.keyboard_id`, or VID/PID-based discovery.
- UI:
  - Setup/Arduino gets an **Input backend** section: Board / Interception.
  - Device picker with the driver monitor's identity, VID/PID and current ID.
  - A driver status/refusal panel; no manual DLL path fields.
- Diagnostics:
  - `logs/input.log`: backend, device ID, open/close, every start/stop, release reason;
  - one `hello`/status field so a support log identifies the backend.

The driver DLL is **not committed** to this repository unless its license allows redistribution.
Interception licensing must be reviewed before packaging. The plan assumes the player installs the
driver, or the release provides a separately licensed optional installer.

## 6. Phases and gates

### Phase 0 — nGuard, license and driver decision (no code)

- Determine whether nGuard accepts the **native Interception driver itself**, not just AHK. This is a
  stop/go gate; if the driver is treated as a cheat, stop.
- Read Interception's license; determine whether redistribution with the zip is allowed or whether a
  separate installer is required. Record the answer.
- Confirm Windows version/signature requirements; test install/uninstall on a spare machine/VM.
- Obtain written risk acceptance for the test account/machine.

**Gate:** written decision + risk acceptance. No code if the driver cannot be installed safely, or
nGuard blocks it.

### Phase 1 — Backend seam and settings

- Introduce `input.backend` and an `Effects` factory; board remains the default.
- Move backend-neutral cursor placement to a shared helper if needed.
- Setup UI gains the backend choice and a read-only readiness status.
- Tests: config default remains `board`; unknown backend refuses; boot/hello includes the backend;
  board tests unchanged.

**Gate:** full Rust + shell gates; board behaviour byte-identical.

### Phase 2 — Interception proof of concept outside the game

- Minimal Rust binary that opens a context, finds one mouse and one keyboard, and sends:
  left/right click, key down/up, relative move, absolute move, wheel.
- A local observer (cursor position, a text editor, pointer trail) proves the reports.
- Measure relative vs absolute movement: raw HID counts are not comparable to Interception screen
  coordinates; decide which primitive the adapter exposes.
- Test device hot-plug / ID changes and permission failures.

**Gate:** a written measurement of movement/click latency and a deterministic way to address the
device. No game or nGuard run yet.

### Phase 3 — `InterceptionEffects` behind the trait

- Implement `Effects` with the same semantics as `BoardEffects`.
- Closed-loop `place` using `GetCursorPos`: initial candidate = chosen Interception move primitive;
  measure how many corrections the game needs.
- Key mapping: share the existing `Key`/scan-code table; ensure hold/release, Enter, wheel.
- Tests with a fake Interception transport: command ordering, release-on-drop, refusal lines.
- No fallback to board; `backend=interception` with no driver is a named failure.

**Gate:** all unit gates; a live cursor loop on a non-game window within the same tolerance the board
meets.

### Phase 4 — Held-input safety and crash tests

- Add the native watchdog/release helper and heartbeat.
- Kill the core/helper with a key held; verify release.
- Kill the watchdog; verify the documented emergency path.
- Stop, error, panic, window close, `tool.stop`, hotkey and backend switch all release.
- Interception only sends while the game is foreground; covered-window/read tests keep their gate.

**Gate:** no stuck key/button in the test matrix. If this cannot be shown, Interception stays a lab
tool and never replaces the board.

### Phase 5 — nGuard-safe live test, then one tool

- First run a non-automating injection probe with nGuard active and confirm no detection/block.
- Run one low-risk tool (e.g. Buy) end-to-end.
- Verify the game foreground interaction, the game's raw-input cursor behaviour, and the cursor
  guard.
- Then Tuner/Gem/Pet/Spammer/sessions, one at a time; each has its own live acceptance.
- Compare against the board: same plans, same counts, same stop reasons.

**Gate:** each tool signs off live; no tool is "ported" before it has run under nGuard.

### Phase 6 — Packaging and support boundary

- Decide whether the native backend is shipped, optional, or developer-only.
- Installer/driver instructions, uninstall/recovery, admin requirements.
- Setup defaults remain board; Interception must be explicitly selected.
- Docs: one page for "Interception backend (experimental)" with the safety warning and recovery steps.
- Add a release note and a support matrix.

## 7. Test matrix

| Area | Test |
|---|---|
| Config | default board; Interception opt-in; unknown backend refused; no fallback |
| Device | find by VID/PID, missing device, ID change, unplug mid-run |
| Keys | down/up, hold/release, Enter, fast key, stuck-key recovery |
| Mouse | left/right click, wheel, relative/absolute move, click-here |
| Cursor | closed-loop tolerance, move budget, foreground loss, wrong window |
| Safety | Stop, hotkey, panic, core kill, helper kill, watchdog kill, app close |
| nGuard | non-automating probe, one tool, full set; no detection/block |
| Regression | every board test and board live path unchanged |

## 8. Risks and stop conditions

| Risk | Mitigation / stop condition |
|---|---|
| nGuard blocks AutoHotkey | **already true**: no AHK anywhere; native only |
| nGuard blocks the Interception driver | stop at Phase 0; keep the board |
| Driver signing/licensing blocks redistribution | separate optional install, not in the zip; stop if not permitted |
| Stuck key/button on process death | watchdog + crash matrix; stop if not demonstrable |
| Input lockout | no blocking filters; emergency keyboard; manual launch only |
| Cursor movement differs from board | measurement and closed-loop; not a drop-in for raw HID counts |
| Device IDs change | discover by VID/PID each run; named refusal, no guessing |
| Security/support burden | opt-in, logged, clearly labelled experimental |
| User expects "no board" but has no admin rights | keep the board path; Interception is not the default |

The board remains the reference implementation and the default. The native backend is successful only
if it matches the same decisions, safety and nGuard behaviour — not merely if it clicks.

## 9. Open questions for the player

1. What exactly is already built — the C# `AutoHotInterception.dll`, a C# helper, or an AHK/AHI
   executable? Please give the path/version; an AHK executable is rejected outright.
2. Is the **Interception kernel driver itself** acceptable to nGuard, or is any kernel filter treated
   as a cheat?
3. Is installing a third-party kernel driver and running it with admin rights acceptable on the
   target PC?
4. Is the account/game risk accepted for testing, and on which account/machine?
5. Should the backend ever be player-facing, or is this developer-only?
6. Which device should be injected — the real mouse/keyboard, or a dedicated second device?
7. Does it need to match the board's physical watchdog, or is "experimental with a recovery step"
   acceptable?

## 10. References

- [AutoHotInterception README](https://github.com/evilC/AutoHotInterception) — behavioural reference
  only; **not shipped or launched** because nGuard rejects AutoHotkey.
- [Interception driver](https://github.com/oblitum/Interception) — kernel driver, API and license;
  review before packaging.
- Current board seam: [seal-board](../../core/crates/seal-board/src/lib.rs),
  [BoardEffects](../../core/crates/seal-core/src/effects.rs),
  [placement](../../core/crates/seal-board/src/placement.rs).
- Current tool effects contract: [runner.rs](../../core/crates/seal-tools/src/runner.rs).

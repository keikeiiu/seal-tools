# PLAN-AHI-INPUT — AutoHotInterception as a board alternative

**Proposal.** This plan evaluates AutoHotInterception (AHI) / the Interception kernel driver as an
input backend that can replace the Arduino Pro Micro board for tools that need to move the cursor,
click, hold keys, or type. It does **not** change how tools decide what to do; it only changes where
the effects are executed.

> **Read this as a proposal, not as current behaviour.** The current shipped input backend is the
> Arduino board over serial (`v3/core/crates/seal-board`). AHI is not a dependency today.

## 1. Why this is worth measuring

The board is reliable but adds hardware and setup friction:

- a Pro Micro/CH340 board and a COM port;
- a firmware level that must match the host grammar;
- a physical watchdog (3 s host silence) that releases a stuck key/button;
- one board per machine, shared with the game's anti-cheat over the COM port during a run.

AHI promises the same key/mouse effects without a board: an AHK library over Francisco Lopez's
Interception driver, which can inject relative/absolute mouse movement, mouse buttons, wheel and
keyboard input, and can target a specific device. AHI's own documentation warns that Interception sits
below Windows and can block input at the OS level, including Ctrl+Alt+Del.

The question is not "can it inject?" — it can. The question is whether it can meet this repository's
existing safety, determinism and anti-cheat constraints without making the suite less safe than the
board.

## 2. What the board seam already isolates

The alternative should fit the seam the board already uses, not fork the tools:

| Layer | Current file | What an AHI backend must provide |
|---|---|---|
| Effect interface | `v3/core/crates/seal-tools/src/runner.rs` (`Effects`) | `click`, `key`, `key_fast`, `hold_space`, `release_space`, `enter`, `place`, `click_here`, wheel, waits |
| Board execution | `v3/core/crates/seal-core/src/effects.rs` (`BoardEffects`) | one implementation per backend, behind the same trait |
| Command grammar | `v3/core/crates/seal-board/src/command.rs` | key/button/wheel/move semantics, but AHI's primitive is a driver report, not a serial line |
| Cursor placement | `v3/core/crates/seal-board/src/placement.rs` | closed loop: move → settle → read cursor → correct → tolerance/budget |
| Board lifecycle | `v3/core/crates/seal-core/src/board.rs` | probe, open, firmware level, failure lines |
| Config/UI | `v3/core/crates/seal-config/src/registry.rs`, Setup/Arduino tabs | backend choice, device choice, admin/driver readiness |
| Safety | firmware `HOST_SILENCE_MS` watchdog and `Drop` release | equivalent held-input release when the host dies |

The decisive constraints are already encoded there:

- **Raw HID counts, never scaled** for movement; placement is closed-loop against the real cursor.
- **Seconds, not milliseconds** in tool waits.
- A held key/button must be released on normal stop, Stop, process exit, and host loss.
- A start must fail closed; a missing backend never silently becomes another backend.

## 3. The two integration options

### Option A — native Interception adapter in Rust (recommended)

Add a new crate, e.g. `seal-input-interception`:

- load/own the Interception context from Rust, using the driver's native API;
- implement `Effects` directly (or a narrow `InputDevice` trait that `Effects` delegates to);
- use the existing `Effects`/runner seam;
- use AHI only as a **behavioral reference** for mouse relative/absolute modes, device-ID
  conventions and `MoveCursor` semantics — not as a runtime dependency.

Pros: no AutoHotkey dependency, one process model, Rust tests around encoding, same release binary.
Cons: Interception is a kernel driver with installation/signing/licensing implications; the API is
C, so unsafe FFI must follow the repository's existing Win32 rules.

### Option B — AHI AutoHotkey helper process

Ship a small AHK/AHI helper (`seal-ahi.exe`) that owns the Interception context; the core talks to it
over a named pipe with one-line commands and gets an acknowledgement/state back.

Pros: uses AHI as intended, quickest prototype, AHI can be compiled to an EXE with its DLLs packed.
Cons: requires an AHK runtime/compiled helper, a second process, pipe protocol, a separate watchdog,
and duplicate configuration/error surfaces. AHI is built for user scripts, not a long-running
machine-controlled input service; process death can leave a held key with no release.

**Recommendation:** Option A for anything shipped. If a short proof-of-concept is wanted first,
Option B may prove the driver/injection path, but not the final safety or packaging.

## 4. Safety design (the part the board currently wins)

This is the reason the plan is not a one-line backend swap.

### 4.1 Held-input release

The board firmware releases a held spacebar/left button on:
- normal `Drop`;
- `tool.stop`;
- serial port close;
- **3 seconds of host silence** (hung host).

AHI has no equivalent physical watchdog. The adapter/helper must provide:

1. an RAII release on every normal path;
2. release on Stop and on process exit;
3. a **separate watchdog process** that monitors the core/helper heartbeat and releases if the
   process is killed;
4. a documented recovery step if the watchdog itself dies;
5. no auto-start at Windows login; manual launch only.

If (3) cannot be made reliable, AHI must be labelled "experimental; a physical keyboard/board must
remain available" and must not replace the board for normal use.

### 4.2 Input lockout

Interception can block input below Windows. The adapter must:

- **never** install blocking filters on the user's normal keyboard/mouse;
- never subscribe/block `Ctrl`, `Alt`, `Del`, or the emergency release key;
- use context mode / injection only, not global blocking;
- expose an emergency physical escape path in the docs;
- require an explicit user confirmation before the first AHI run.

### 4.3 Fail-closed backend selection

- Settings: `input.backend: board | ahi` (new field; default stays `board`).
- Selecting `ahi` must not disable probe/Setup for the board; switching back remains one action.
- AHI start checks: driver installed, device discovered, admin rights if required, target IDs
  stable; failure is a named refusal, never a fallback to the board.
- Existing board users see no change.

## 5. Proposed architecture

```text
tool decision half (seal-domain)
        ↓ Effects trait
seal-tools runner
        ↓
   input backend selected in config
     ├── BoardEffects → seal-board Transport (`D/P/U/...` serial) → firmware → HID
     └── AhiEffects   → seal-input-interception → Interception driver → Windows input
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
  - `AhiEffects` implementing the same `Effects`
  - a backend-neutral `place` closed loop using the existing cursor `Cursor` trait
- Config:
  - `input.backend` in the portable setup block, not `machine/`? The choice is per PC but travels;
    decide in Phase 1; no silent migration.
  - `input.ahi.mouse_id`, `input.ahi.keyboard_id`, or VID/PID-based discovery.
- UI:
  - Setup/Arduino gets an **Input backend** section: Board / AHI.
  - Device picker with the Interception monitor's identity, VID/PID and current ID.
  - A driver status/refusal panel; no manual DLL path fields.
- Diagnostics:
  - `logs/input.log`: backend, device ID, open/close, every start/stop, release reason;
  - one `hello`/status field so a support log identifies the backend.

The driver DLL and any AHI artifacts are **not committed** to this repository unless their licenses
allow redistribution. Interception/AHI licensing must be reviewed before packaging. The plan assumes
the player installs the driver, or the release provides a separately licensed optional installer.

## 6. Phases and gates

### Phase 0 — License, driver and anti-cheat decision (no code)

- Read Interception's license; determine whether redistribution with the zip is allowed or whether a
  separate installer is required. Record the answer.
- Confirm Windows version/signature requirements for the driver; test install/uninstall on a spare
  machine/VM.
- Read the game's anti-cheat behaviour with a kernel input filter present; this is a **stop/go** gate.
- Decide whether AHI is literal (AHK helper) or whether native Interception is acceptable.

**Gate:** written decision + a risk acceptance from the player. No code if the driver cannot be
installed safely or the game is likely to treat it as a cheat.

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
- Measure relative vs absolute movement: raw HID counts are not comparable to AHI
  `MoveCursor` screen coordinates; decide which primitive the adapter exposes.
- Test device hot-plug / ID changes and permission failures.

**Gate:** a written measurement of movement/click latency and a deterministic way to address the
device. No game run yet.

### Phase 3 — `AhiEffects` behind the trait

- Implement `Effects` with the same semantics as `BoardEffects`.
- Closed-loop `place` using `GetCursorPos`: initial candidate = current AHI move primitive; measure
  how many corrections the game needs.
- Key mapping: share the existing `Key`/scan-code table; ensure hold/release, Enter, wheel.
- Tests with a fake Interception transport: command ordering, release-on-drop, refusal lines.
- No fallback to board; `backend=ahi` with no driver is a named failure.

**Gate:** all unit gates; a live cursor loop on a non-game window within the same tolerance the board
meets.

### Phase 4 — Held-input safety and crash tests

- Add the watchdog/release helper and heartbeat.
- Kill the core/helper with a key held; verify release.
- Kill the watchdog; verify the documented emergency path.
- Stop, error, panic, window close, `tool.stop`, hotkey and backend switch all release.
- AHI only sends while the game is foreground; covered-window/read tests keep their existing gate.

**Gate:** no stuck key/button in the test matrix. If this cannot be shown, AHI stays a lab tool and
never replaces the board.

### Phase 5 — One tool live, then all tools

- Add an experimental backend switch; run one low-risk tool (e.g. Buy) end-to-end.
- Verify the game foreground/anti-cheat interaction, the game's raw-input cursor behaviour, and the
  cursor guard.
- Then Tuner/Gem/Pet/Spammer/sessions, one at a time; each has its own live acceptance.
- Compare against the board: same plans, same counts, same stop reasons.

**Gate:** each tool signs off live; no tool is "ported" before it has run.

### Phase 6 — Packaging and support boundary

- Decide whether AHI is shipped, optional, or developer-only.
- Installer/driver instructions, uninstall/recovery, admin requirements.
- Setup defaults remain board; AHI must be explicitly selected.
- Docs: one page for "AHI backend (experimental)" with the safety warning and recovery steps.
- Add a release note and a support matrix.

## 7. Test matrix

| Area | Test |
|---|---|
| Config | default board; AHI opt-in; unknown backend refused; no fallback |
| Device | find by VID/PID, missing device, ID change, unplug mid-run |
| Keys | down/up, hold/release, Enter, fast key, stuck-key recovery |
| Mouse | left/right click, wheel, relative/absolute move, click-here |
| Cursor | closed-loop tolerance, move budget, foreground loss, wrong window |
| Safety | Stop, hotkey, panic, core kill, helper kill, watchdog kill, app close |
| Game | foreground requirement, anti-cheat, raw-input cursor, one tool at a time |
| Regression | every board test and board live path unchanged |

## 8. Risks and stop conditions

| Risk | Mitigation / stop condition |
|---|---|
| Anti-cheat detects/blocks the driver | stop at Phase 0; do not ship |
| Driver signing/licensing blocks redistribution | separate optional install, not in the zip; stop if not permitted |
| Stuck key/button on process death | watchdog + crash matrix; stop if not demonstrable |
| Input lockout | no blocking filters; emergency keyboard; manual launch only |
| Cursor movement differs from board | measurement and closed-loop; AHI is not a drop-in for raw HID counts |
| Device IDs change | discover by VID/PID each run; named refusal, no guessing |
| Security/support burden | opt-in, logged, clearly labelled experimental |
| User expects "no board" but has no admin rights | keep the board path; AHI is not the default |

The board remains the reference implementation and the default. AHI is successful only if it matches
the same decisions and safety, not merely if it clicks.

## 9. Open questions for the player

1. Does "AHI" mean using AutoHotInterception/AutoHotkey as a helper, or is a native Interception
   adapter acceptable?
2. Is installing a third-party kernel driver and running it with admin rights acceptable on the
   target PC?
3. Is the account/game anti-cheat risk accepted for testing, and on which account/machine?
4. Should AHI ever be player-facing, or is this a developer-only alternative?
5. Which device should be injected — the real mouse/keyboard, or a dedicated second device? AHI
   targets a device ID; the game may distinguish injected from physical input.
6. Does AHI need to match the board's physical watchdog, or is "experimental with a recovery step"
   acceptable?

## 10. References

- [AutoHotInterception README](https://github.com/evilC/AutoHotInterception) — device IDs, relative
  and absolute mouse movement, blocking warning.
- [Interception driver](https://github.com/oblitum/Interception) — kernel driver, API and license;
  review before packaging.
- Current board seam: [seal-board](../../core/crates/seal-board/src/lib.rs),
  [BoardEffects](../../core/crates/seal-core/src/effects.rs),
  [placement](../../core/crates/seal-board/src/placement.rs).
- Current tool effects contract: [runner.rs](../../core/crates/seal-tools/src/runner.rs).

# Plan — v3: a headless Rust core, and a Flutter client

Written 2026-10-25. **Its build state lives on the board in [README.md](README.md) — never in
this file.** Supersedes [PLAN-V3-RUST.md](PLAN-V3-RUST.md): that document's diagnosis is still
the reason for this one, but its shape — an in-process C ABI behind WPF, or Tauri — is replaced
by a **process boundary and a typed protocol**. Its open question §8.1 (Rust core, or Rust app)
is answered: neither; **a Rust core and a Flutter client**, as two processes.

The ask behind it: *"this project is also a learn project for me — suggest a solution that really
outperforms my current setup, better core and better frontend."* So "better" is not "newer
tools". It is the three things v2's own audits measured and failed on: **boundedness** (0
interfaces across 26,650 lines), **testability** (~12,600 lines the test project cannot see), and
**messages that lie** (`PROGRESS.md` entry 63 — nine defects, every one a decision made somewhere
no test could reach). The frontend half is measured separately in §9.

---

## 1. What v2 measures today (`main` @ `c8d96bc`, 2026-10-25)

`v2/`, excluding `obj/` and `bin/`:

```
  77  .cs files, 26,650 lines
9415  SealTools.Launcher\MainWindow.xaml.cs
3153  SealTools.Pet\PetTool.cs
1448  SealTools.Tests\ConfigLoaderTests.cs
 889  SealTools.Core\Config\AppConfig.cs
 614  SealTools.Core\OcrEngine.cs
  16  DllImport declarations in Core (14 WindowFinder, 1 Dpi, 1 Hotkeys)
   0  interfaces declared (re-checked; still zero)
 324  test cases (DESIGN.md), across 9 projects
   7  runnable tools, not six: the six tool projects plus the `holdspace` toggle
```

Published payload, measured in the publish folder: **317.6 MB** — a 174.5 MB self-contained exe
plus `OpenCvSharpExtern` 65.4, `opencv_videoio_ffmpeg` 27.3, `onnxruntime` 15.4, `libSkiaSharp`
11.7 and the three PP-OCRv4 `.onnx` models 15.4. `publish.bat` ships this as a **zip of the
folder**; "one self-contained `.exe`" in `AGENTS.md` is shorthand for "one exe at the top of the
zip", which removes a distribution objection the first draft of this plan had.

**The diagnosis is PLAN-V3-RUST §1 and it has not improved in fifteen days — it has drifted
worse:** that document measured 71 files / 22,859 lines / a 9,058-line launcher; today it is 77 /
26,650 / 9,415. The god object grows roughly 3,800 lines a fortnight. That is the strongest
argument for cutting the seam now rather than after "one more feature".

---

## 2. The decision: two processes, one seam

```
─ UI process ─────────────────────────────────────────────────────────────
  Flutter / Dart 3.47            DPI-AWARE. Renders. Holds a copy of config and
  v3/shell                       live state. Owns no Win32, no game, no board,
                                 no file writes. Talks one protocol.
        │  newline-delimited JSON over the core's stdin/stdout
        │  (UI spawns the core; EOF on stdin = "stop safely")
─ core process ───────────────────────────────────────────────────────────
  Rust workspace                 DPI-UNAWARE (app.manifest, on purpose).
  v3/core                        Owns: config load/save (single writer), Win32
                                 window/DPI/capture/hotkeys, Arduino serial,
                                 OCR, the six tools plus Hold Space, logging.
```

Five reasons the boundary is a process and not an interface or an FFI bridge:

1. **The DPI contract forces it.** The core must stay DPI-unaware — the calibrated coordinates,
   the `SetCursorPos` refusal on aware threads (`7f9e1c7`) and the capture blit are all built on
   it ([COORDINATES.md](../spec/COORDINATES.md) §"Why we stay DPI-unaware"). Flutter's Windows
   runner ships PerMonitorV2 awareness and the engine does its own DPI handling, so an in-process
   Flutter UI either renders bitmap-scaled or forces the coordinate contract to be re-derived.
   Two processes each get the awareness they need: **core unaware, UI aware and sharp.**
2. **A process boundary cannot be violated by an import.** The repo's problem was never a missing
   interface — it is that the launcher can reach the domain because they share a process. The
   only way a widget can decide a pet is unfed is if it owns the logic. It won't.
3. **Crash and stall isolation.** A frozen or crashed Flutter UI must not kill a pet feeder that
   has been running for days. The core keeps running, stops safely on stdin EOF (releasing held
   keys and the left button through the board's guard), and is restarted by the next UI launch.
   This also neutralises Flutter's worst Windows failure (`EGL_CONTEXT_LOST` on sleep/resume,
   §9) — restart the client, the tools never noticed.
4. **Each half is testable without the other.** The core runs from a CLI and a test harness; the
   shell runs against a replay core. Neither needs the game, the board, or a screen.
5. **No Dart FFI, no Win32, no plugins in the shell.** That removes the three specific Flutter
   costs the research found: Flutter exposes no screen coordinates (the core sends the frame and
   its mapping instead), desktop window/state/single-instance plugins are third-party churn (the
   runner's C++ is ours to edit), and text input is the platform's own control, not a re-learned
   IMM32 path.

**Why Flutter despite §9.** The declarative UI is the boundary WPF never imposed; hot reload is
what an app with five copy-pasted calibration canvases wants; Dart 3's `sealed` classes,
exhaustive `switch` and sound null safety mirror the core's `Decision` enums better than any
other UI language; and one `CalibrationCanvas` widget replaces the duplication ANALYSIS-UI §3.1
measured. The user-facing costs are real and listed with their mitigations in §9. And the
protocol means the client is **replaceable**: if Flutter disappoints in a year, the same core
drives a Tauri or Avalonia client. That is the insurance, and it is why the protocol has to be
designed properly before either half gets big.

---

## 3. The shape

```
v3/core/                        cargo workspace, Rust
  crates/seal-domain            pure model: config types and authoring rules, geometry,
                                arbiter, scheduling, matchers, and decide(read, cfg) -> Decision
                                #![forbid(unsafe_code)], no I/O, no Win32
  crates/seal-win               window find/measure, DPI double-read, GDI capture,
                                foreground guard, hotkey polling
  crates/seal-board             serial: level-2 commands, V query, RAII held-key guard,
                                closed-loop placement
  crates/seal-ocr               ort + the three PP-OCRv4 models + the existing
                                grouping/score/text-fix pipeline
  crates/seal-tools             the six tools plus Hold Space; trait Tool; cancellation;
                                state events
  crates/seal-core              the binary: protocol server, lifecycle, single writer,
                                logging, single-instance lock
v3/shell/                       Flutter app, Dart
  lib/protocol/                 typed client: framing, handshake, requests, event stream
  lib/model/                    Dart mirror of the config and state types
  lib/screens/                  cards and the fifteen tabs
  lib/widgets/                  CalibrationCanvas (one), Section/Hint/LabeledField, ...
  lib/theme/                    dark theme; window chrome in the runner's C++
  test/                         widget + golden tests against a replay core
```

A rule as short as v2's: **the shell never decides, the core never draws.**

| | core | shell |
|---|---|---|
| config | reads and writes, one writer, `local.yaml`/`defaults.yaml.example`/`attributes.yaml`; owns every rule | a copy for the forms; sends validated merge intents |
| game | finds, measures, captures, reads, decides | displays frames and numbers |
| input | Arduino only; cursor via raw HID counts | none |
| state | owns it; publishes snapshots as events | renders them; owns nothing |
| logs | writes them | shows the lines it is sent |

---

## 4. The protocol

**Transport.** The shell spawns `seal-core.exe` and speaks **UTF-8 newline-delimited JSON** on the
child's stdin/stdout. stderr is the core's panic/log channel. No HTTP, no named pipe, no FFI: stdio
is binary-safe on Windows, needs no plugin, ties the core's life to the client's by construction,
and lets a test harness (and a future CLI) drive the same core. If a detached long-running core is
ever wanted (⚑ §13.3), the framing moves to a named pipe without changing the messages.

**Envelope.** Every message is one JSON object with `v` (protocol version), and requests carry
`id`; responses echo `id` and are `{ok: true, ...}` or `{ok: false, error: {code, message,
detail}}`. Events carry `event` and no `id`. A `hello`/`hello_ok` handshake exchanges protocol
version, core version, resolved root directory, and the full config; a version mismatch refuses
with a message the UI can show.

**Commands** (UI → core), the minimum set:

| command | returns | notes |
|---|---|---|
| `hello` | version, root, config, rev | first message; mismatch refuses |
| `config.validate` | errors, warnings | dry-run of what a tab is about to save |
| `config.apply` | rev, merged sections | `{rev, sections}` — only the blocks the tab owns (§4.1) |
| `config.reload` | config, rev | re-read from disk (hand edits) |
| `window.find` / `window.measure` | title/hwnd, `DisplayInfo` | `DisplayInfo` = physical frame/client, logical client, scale, monitor DPI |
| `capture` | frame + origin + `DisplayInfo` | region defaults to the game client; PNG or raw BGRA, base64 in JSON |
| `calibration.save` | rev, derived values | the dragged region + reference frame (§4.2) |
| `calibration.fetch` | saved frame(s) | the reference screenshot the canvas shows on launch |
| `board.probe` / `board.test` | level, port, result | level from `V`; test = click/move/key |
| `ui.window` | ok | window geometry and pin state — the UI is the only thing that has them, the core is the writer |
| `tool.start` / `tool.stop` | ok | one tool at a time (`holdspace` included), enforced in the core |
| `ocr.scan` | lines, attributes, grade, remaining, debug path | calibration/test reads |
| `shutdown` | — | clean exit |

**Events** (core → UI): `state` (per tool: running, phase, countdown, rows, message, error — the
whole 750 ms-poll surface, now pushed), `config.changed` (§4.1), `log` (level, line),
`core.stopping` (reason), and `core.ready` after the handshake. **Images cross the protocol** — a
calibration frame and the `DisplayInfo` that says how to map image pixels to each coordinate
space. The UI never asks Win32 anything.

### 4.1 Config is a merge, not a document

The first draft of this plan had the UI send the **whole** config on save. That is wrong, and the
reason is `local.yaml`'s history: it is not only settings. The pet feeder persists **run state**
mid-run — learned close-guard data, the boarding belief, food-cell marks — through
`PetTool` → `PersistPetState`, and the launcher persists the `ui:` block on every move/resize. In
v2 both writers share one in-memory `AppConfig` and one `UpdateLocal` lock, so nothing goes stale.
Across a process boundary, a UI holding a copy from startup would write the whole file back and
delete everything the core wrote since — the lost-block class
[CONFIG.md](../spec/CONFIG.md) says this file paid for four times. `ConfigLoader.LocalPet` already
knows the split (`ApplySession` writes the session half; the Pet tab writes the static half); v3
keeps that split, but the **core is the only merger**:

- The UI edits a **working copy** of the sections it is allowed to write, validates types only,
  and asks `config.validate` for the real rules before it shows a Save as possible.
- Save sends `config.apply {rev, sections}` — **only the blocks that tab owns**, never the file.
  The core re-runs the authoritative rules, merges field-wise into the current block (so a session
  half it wrote a second ago survives), writes atomically, bumps `rev`, and returns the merged
  sections.
- A stale `rev` is refused with `config.conflict` plus the current values; the UI reconciles and
  tells the player what changed underneath.
- Core-initiated writes broadcast `config.changed {rev, block, values}` — pet session state, the
  `ui:` block, calibration saves. A UI that is not editing that block applies it silently; an
  editing UI shows a conflict rather than silently overwriting.
- `ui:` is fully core-owned: the UI sends `ui.window`; nobody edits it in a tab.
- The authoring rules the launcher currently carries — Quest Save refusing a flow/sequence entry
  that names a missing flow and listing it, Duplicate/rename/re-point, preset-name and hotkey-name
  validation, game-variant switching, Setup's blank-client-size refusal, the Arduino tab's two
  readers of one resolution — move into `seal-domain` and are exercised through
  `config.validate`/`config.apply`. **No widget keeps a rule**, which is the failure class this
  plan exists to remove.
- Hand edits are read at startup and on `config.reload`; two UI instances are refused by a
  single-instance lock (a lock file under the root, held by the core) rather than by hoping.

### 4.2 Calibration artifacts stay in the core

Calibration Save is not just YAML: v2 writes `config/calib_*.png` reference screenshots, writes
`config/calib_gem_result.png`, and derives `empty_signature` from the screenshot taken **with the
launcher hidden** — deliberately not a live grab, because a live one read the launcher and the
empty check stopped judging correctly (the comment in `MainWindow.xaml.cs` records it). In v3 the
core keeps that whole job: `calibration.save` receives the dragged region and the frame captured
while the UI was hidden, writes the artifacts and the derived values, and returns them;
`calibration.fetch` gives the canvas the saved reference on the next launch. The shell draws and
sends; it never derives.

---

## 5. The coordinate contract — the part that must not drift

This is where a port usually loses a month, so it is written down before anything is built.

- **Core process: DPI-unaware**, as today. Win32 geometry calls therefore return *logical*
  pixels; the screen blit is 1:1 *physical* — that asymmetry is measured, load-bearing and
  documented in [COORDINATES.md](../spec/COORDINATES.md) §Capture. The port copies the behaviour;
  it does not "fix" it into a scale.
- **UI process: DPI-aware.** Its own coordinates are irrelevant to tooling. It renders a captured
  frame at its natural pixel size (or scaled to fit with the scale shown), and converts a drag
  rectangle back to **physical client-relative offsets** using only the `DisplayInfo` the core
  sent — no Win32, no `GetDpiForWindow`, no assumptions.
- **Stored calibration keeps its current meaning** (physical client-relative offsets; raw HID
  counts for `gem.movements`, never scaled). `local.yaml` from v2 loads unchanged — that is a hard
  requirement inherited from PLAN-V3-RUST §8.2.
- **Capture gating:** the tools' "the game must be IN FRONT" guard becomes simpler in the split:
  the core asks `GetForegroundWindow` and compares it with the game's HWND. The UI window, being
  another process's window, can never be mistaken for the game. When the UI *is* in front
  (calibration), the UI hides itself, then sends `capture`; the core waits for the foreground to
  settle before blitting. The three existing guards are ported, not reinvented
  ([DESIGN.md](../spec/DESIGN.md) §5).
- **DPI change:** the core re-measures per call, never caches ([WindowFinder.cs](../../SealTools.Core/WindowFinder.cs)
  comment "Re-query per call — never cache across a mixed-DPI multi-monitor move"). The UI handles
  `WM_DPICHANGED` in the engine and re-lays out; the canvas re-derives from the next `DisplayInfo`.

---

## 6. What the core must preserve

The port is through behaviour, not through files — but four subsystems encode measured wisdom
that must survive verbatim.

**6.1 Config** (`ConfigLoader.cs`, 806 lines; `AppConfig.cs`, 889). Port the *semantics*, not the
field list: three layers (`defaults.yaml.example` base, the one-time fold into `local.yaml`,
`attributes.yaml`), the `OwnedLocalBlocks` list, **foreign top-level blocks preserved verbatim
including comments** (`ForeignBlocks`), the constant `LocalHeader`, atomic write through a temp
file, and read-modify-write inside one lock so two writers can never delete each other's block —
the bug that file's doc comment says it paid for four times. `serde` derive replaces the
hand-written projection (the other four-time bug, the silently dropped field); a test walks the
type and asserts every field reaches the projection. **The `pet:` block's two halves are
preserved** — `LocalPet.ApplySession` for run state, the tab's static half — and the core merges
both (§4.1). `ConfigLoaderTests.cs` (1,448 lines) is the oracle for all of it.

**6.2 Board** (`HidPointer`, `HidController`, the pipeline). Level-2 protocol exactly as
[DESIGN.md](../spec/DESIGN.md) §2.1 documents it (`C`/`R`, `D`, `E`, `K`/`F`, `Q`/`Z`, `L`/`l`, `V`),
one command per line; `V` is the only thing the sketch writes back and is how level 1 is told
from level 2. The held-key release becomes a `Drop` guard, not two hand-written `finally` blocks.
Placement stays a closed loop over `GetCursorPos` with a step budget, reporting the goal and where
it stopped. `SleepCheck` takes **seconds** — the eleven-minute sleep incident is a doc-comment and
a type, not a hope.

**6.3 OCR** (`OcrEngine.cs`, 614 lines). The three `.onnx` files are reused unchanged; `ort` runs
them; the det/rec/cls post-processing, line grouping, score gates, `TextCleaner`/`text_fixes` and
`AttrMatcher` are ported from the C# rather than re-derived. The measured quirks are kept and
cited: the crop that must be the region's full height, the feeder-count band only a few pixels
wide, the confidence gate because a cramped crop lies at 0.99. **The recognizer's character
dictionary is not in `models/`** — RapidOCRSharpOnnx embeds it inside its 18.5 MB DLL; extracting
it is a step-5 precondition, not an afterthought. Parity is measured against the recorded corpus
(§8), not argued.

**6.4 Tools** (`ToolBase`, six tools plus Hold Space). `trait Tool` with an explicit cancellation
type — "a stop abandons the WORK, not the CLEANUP" becomes a signature instead of a comment. Each
tool's decision half becomes a pure function returning a `Decision` (`Click`, `Key`, `Wait`,
`Stop(Reason)`) that carries the message to show; the loop only performs effects. The resident pet
feeder keeps its deferral rules ([DESIGN.md](../spec/DESIGN.md) §1, §4). **Hold Space is a
runnable tool and stays in scope**: `holdspace` in `LauncherService._toolRunners`, no card, a
board-level held spacebar (`P`/`U`) toggled by its own row, with its release failures logged to
`logs/holdspace.log` and surfaced on the toggle's status line. It is a toggle, not a card — the
protocol's `tool.start`/`tool.stop` covers it, and its step is next to the spammer.

**6.5 Hotkeys need a measurement, not a port.** [USER_GUIDE.md](../spec/USER_GUIDE.md) line 587 is
explicit: *"The quit hotkey will not stop this while you are in-game — the anti-cheat blocks
background key reads, which is why the launcher has to be focused for any hotkey to register."*
The core is a process that never owns the foreground window, so two rules fit the same evidence:
(a) the anti-cheat blocks reads only while the **game** is foreground, in which case core-side
`GetAsyncKeyState` behaves exactly like today's launcher-side polling; or (b) it blocks reads from
any non-foreground process, in which case core-side polling never works and the hotkey path must
move to the UI process (a documented exception to "shell owns no Win32") with `hotkey` commands
sent to the core. **Step 1 measures which.** Either outcome preserves today's behaviour — the Stop
button is already the control that always works — but the plan must not call this a mechanical
port until the spike says so.

Everything else — 16 P/Invoke equivalents, foreground checks, log paths (`logs/`, `logs/reads/`,
`logs/captures/`, `logs/ocr_log.jsonl`) — is a mechanical port with the spec as the checklist.
Hotkey polling itself (`GetAsyncKeyState`, not `RegisterHotKey`) is the one item §6.5 holds back.

---

## 7. What the shell is

- **A protocol client, then state, then widgets.** One state object per concern (connection, config,
  tools, status), fed only by decoded events; the widget tree renders it. No business rule lives
  in a widget — the rule that failed in v2.
- **One `CalibrationCanvas`.** CustomPainter for the frame + overlay rectangles, `GestureDetector`
  for drag, and the image-pixel ↔ physical-offset conversion from `DisplayInfo`. The five
  copy-pasted WPF canvases become one widget with one behaviour, tested once.
- **The form vocabulary is ported, the code is not.** `Section`/`Hint`/`LabeledField`/`MakeButton`
  become widgets and theme tokens; the 62 inline layout constants become one density token set.
- **Theme and chrome.** Dark, Mica-like. Window chrome, minimum size and Mica are done in the
  Flutter Windows runner's C++ (ours to edit) rather than through `window_manager`/`bitsdojo`
  plugins — the plugin churn the research found is avoidable because we own the runner.
- **Chinese name fields keep their IME mitigation.** An app-lifecycle listener unfocuses the text
  client when the window loses activation, which is the condition Flutter's composition bug needs.
  A spike with this machine's Microsoft Bopomofo precedes the tab work; fallbacks are in §9.
- **No tray, no file dialogs, no single instance to port** — v2 has none of them (checked), and
  single instance is now the core's lock.

---

## 8. Testing and parity

- **The 324 tests are the oracle.** Translate the ones that assert decisions and messages; drop
  the ones that test C# plumbing. `cargo test` runs them without Windows, the game or the board.
  The v2 suite stays green until cutover as the reference implementation.
- **`proptest` for the invariants** the language now enforces: arbiter never starts two tools;
  "unknown does not skip" (a guard may only remove a boarding); decision purity (same read + same
  config → same `Decision`); board command round-trip; config projection completeness.
- **`insta` snapshots** for decision outputs and the user-visible strings, so the "says the wrong
  thing" class fails a test instead of reaching the player.
- **OCR parity corpus — measured scope, not assumed.** `v2/logs/captures/` holds **3,366 PNGs**
  and `v2/logs/ocr_log.jsonl` **9,326 scan records**, and they line up almost one-to-one — but
  they are the **tuner's full-region scans**. The feeder-count and pet-panel reads (the
  "few-pixels-wide band", the "0.99 on a cramped crop" quirks) save only occasional `logs/reads`
  crops, so a corpus-only parity claim would cover the least knife-edge path and none of the
  most. Two consequences: the **v2 maintenance window records a live corpus for the missing read
  paths** (needs a game session or two), and step 5's acceptance is **per read path**, not one
  number. `logs/` is gitignored — the corpus is archived locally before it can be cleaned.
- **Protocol fixtures:** Rust tests write golden NDJSON files and Dart tests decode them; a Rust
  test replays recorded command fixtures. The two sides cannot silently drift.
- **A replay core** (`seal-core --replay script.json`) drives the shell with no game, no board,
  no OCR — the basis of widget tests, golden tests, and UI development.
- **Live verification per stage**, as the repo already does — a tool is done when it has run in the
  game, not when it compiles ([PROGRESS.md](../records/PROGRESS.md) is full of the difference).

---

## 9. What Flutter costs, and how the shape pays for it

Research on 2026-10-25; every claim has a link. This is the price list, not an argument against the
choice — the core/client split is what makes most of it survivable.

| Flutter Windows behaviour | effect on this app | mitigation / why it is tolerable |
|---|---|---|
| CJK composition dies after a long focus loss — [#190042](https://github.com/flutter/flutter/issues/190042), fix [PR #190043](https://github.com/flutter/flutter/pull/190043) still draft, issue auto-closed | name fields (preset/flow names) stop accepting Chinese until restart | unfocus on window deactivate (§7); spike **with this machine's Bopomofo**, not the reporters' Pinyin/Sogou; fallback: native EDIT overlay or pick/auto names (§13.2) |
| Sogou composition leaks/duplicates across a field change — [#191196](https://github.com/flutter/flutter/issues/191196), [#192624](https://github.com/flutter/flutter/pull/192624) open | corrupted text in two adjacent fields | IME-specific (Microsoft Bopomofo not implicated); test; same fallbacks |
| Grayscale AA only — no ClearType/DirectWrite — [#63043](https://github.com/flutter/flutter/issues/63043), 3.47 unchanged [#194931](https://github.com/flutter/flutter/issues/194931) | 12–14 px Chinese text softer than native at 100% | at this machine's 150% the current build is bitmap-stretched, so Flutter is likely *sharper* than today; verify side-by-side in the skeleton |
| No screen-coordinate API in Flutter | a canvas that must map to game/screen coords | the core sends the frame + `DisplayInfo`; no FFI needed |
| Can lose its GL context on sleep/resume/TDR (`EGL_CONTEXT_LOST`) | frozen/black UI after waking | restart the client: tools live in the core and keep running; the UI is a stateless-ish client by design |
| Impeller shader warm-up can freeze the window for seconds at startup — [#193048](https://github.com/flutter/flutter/issues/193048) | slow first paint | measure in the skeleton; Skia fallback is still available if it bites |
| UIA drops `Semantics.hint` — [#192782](https://github.com/flutter/flutter/issues/192782) | screen-reader descriptions incomplete | v2 has two `AutomationProperties` calls total; accessibility is a §13.8 question, not a regression to fear |
| Bundle ~30–50 MB, no installer of its own | delivery | the release is already a zip of a folder (§1), so the shape is unchanged; Inno Setup or the zip both work |

The architectural point: **none of these can corrupt a running tool**, because none of them live in
the process that runs the tools.

---

## 10. The port sequence

Every step leaves v2 shippable, and every step except 0 is a working artifact, not a milestone.

**0. v2 maintenance window (before anything else).** The `.NET 8` EOL is **2026-11-10**
([TODO.md](../backlog/TODO.md) §1) — the TFM bump to .NET 10 is forced regardless of v3, and it
carries an IME regression check: a cancelled composition on **.NET 10.0.12** is retained and
routes later input to the stale element ([dotnet/wpf#12013](https://github.com/dotnet/wpf/issues/12013),
open 2026-10-10). Test it in the bump; pin a serviced patch. **Player decision 2026-10-25: the
bump is skipped** — v2 keeps shipping on .NET 8 past its 2026-11-10 end of support until v3 cuts
over, accepting that the release is self-contained so a servicing fix would mean republishing
anyway, and that the IME check above is deferred with it. From here v2 is **maintenance only** —
no refactors, no new features — because every hour spent extracting C# interfaces that v3 deletes
is wasted (that is why PLAN-V3-RUST step 1 is dropped). Two things happen in this window: the
commit that freezes v2 is **tagged as the parity baseline** (later v2 fixes are ported
deliberately or deferred, never silently), and a **live recording session or two** captures the
read paths the existing corpus misses (§8).

**1. Protocol spec + walking skeleton.** The command/event list (§4) written into
`spec/PROTOCOL.md`; a Rust binary that discovers the root, loads `local.yaml`, opens the board,
finds and measures the game window and captures one frame; a Flutter window that spawns it, shows
the version/scale/board level, and draws the frame with a drag rectangle. Acceptance: on this
machine at 150%, a drag in the canvas comes back as the same physical offset the WPF calibrator
records; the UI is visibly sharper than today's; the IME spike and the hotkey spike (§6.5) are
scripted and recorded. **This is the go/no-go for the whole stack — and it is days, not weeks.**

**2. Config port.** Loader, writer, fold, foreign-block preservation, atomic write, lock; the
`pet:` two-halves merge; the authoring rules that currently live in the launcher (§4.1); the
`config.validate`/`config.apply`/`config.reload`/`config.changed` surface; `calibration.save` and
`calibration.fetch`. Acceptance: a v2-written `local.yaml` loads and round-trips;
`ConfigLoaderTests` translated green; a stale-`rev` save is refused and reconciles; the v2.16
Quest rules (missing flow, duplicate name) reject the same inputs; a core pet-state write during a
UI save is not lost.

**3. Domain port.** Geometry, arbiter, scheduling, matchers, the `Decision` types and every tool's
pure decision half; the translated tests + proptest. No UI work. Acceptance: the decision half of
every runnable tool (seven) tested headless.

**4. Spammer + Hold Space end-to-end (the pilot).** One card tool and one toggle tool through
every layer — decision, loop, board, state events, UI, hotkey, stop and release paths — in the
game. Acceptance: live runs with the same observable behaviour as v2, including Hold Space's
release-failure reporting.

**5. OCR parity.** `ort` + the pipeline + the extracted dictionary, measured against the archived
corpus; then the tuner's read in the game, then the feeder-count and pet-panel paths against their
recorded corpora. Acceptance: per-path parity, then one live tuner run.

**6. The remaining tools**, in order: shop, gem composer, tuner, quest, pet. Each one is decision
tests → loop → events → tab → live run. Pet last because it is the resident, most failure-prone
tool and it wants the arbiter and scheduling to be settled.

**7. Cut-over.** Publish pipeline (core exe + Flutter bundle, zipped and installed the way
`publish.bat` works today), the v3 spec tree (`DESIGN.md`, `CONFIG.md`, `COORDINATES.md`,
`TOOL-FLOWS.md`, `USER_GUIDE.md` + `zh-TW` twin), release notes. Firmware unchanged — no reflash,
no `FirmwareSketchTests`/`FirmwareVersionTests` bump; that hard requirement is inherited.

**What each step teaches.** 1 — process protocols, `windows` crate, GDI, Flutter spawning and
canvas painting; 2 — serde, file I/O discipline, merge/round-trip testing; 3 — ownership, traits,
property-based testing, ADT design; 4 — RAII/`Drop` guards, cancellation, supervision; 5 — ONNX
Runtime from Rust, image pipelines, parity engineering; 6 — the same patterns at scale; 7 —
packaging and docs.

---

## 11. Risks, in the order they would hurt

1. **OCR parity is the wall.** The PP-OCRv4 post-processing is the least-known code and the most
   value-bearing. The corpus makes it measurable; if Rust parity stalls, the fallback is to keep
   OCR in a small native helper behind the same protocol rather than block the rest of v3. The
   corpus covers the tuner path only, and the dictionary must be extracted from the C# package
   — both are step-5 preconditions, not surprises (§8).
2. **Config writes from the core mid-run.** The whole-file save the first draft described would
   have deleted pet session state on the next tab save — the four-times-bug again. The merge
   protocol (§4.1) is the mitigation, and "a core write during a UI save survives" is an explicit
   step-2 acceptance test.
3. **The duplicate config model** (Rust types and Dart mirrors). Mitigation: protocol fixtures, a
   round-trip test ("save untouched → identical JSON"), and a Rust test that the projection covers
   every field. Codegen is a later option, not a first one.
4. **Hotkeys in a background process** (§6.5). Measurement in step 1; fallback is UI-side polling
   and a `hotkey` command — a documented exception, not an architecture change.
5. **IME with the player's actual Bopomofo** (§9). Mitigation: unfocus-on-deactivate, spike first,
   fallbacks named.
6. **YAML in Rust.** `serde_yaml` is deprecated; pick a maintained fork/`saphyr` early. The
   writer's requirements are modest — a typed graph, a constant header, foreign blocks passed
   through as raw text — but the parser must accept everything v2 writes and everything the player
   has hand-edited.
7. **Flutter text rendering** (§9). Measured side-by-side before the tab work commits.
8. **The port's temptation: a line-by-line translation.** Every god method copied into Rust is a
   failure. The unit of port is the *decision*, and a function that reads, decides and acts is not
   ported until it is three things.
9. **Timeline.** This is months of evenings, not weeks. The sequence is staged so stopping after
   step 4 leaves a working spammer rather than a half-ported app.
10. **v2 starves during the port.** Deliberate: v2 only gets the .NET 10 bump, bug fixes and docs
    until v3 earns its keep. If v3 stalls, v2 is still the shipped app and its suite still guards
    it.

---

## 12. What this does to the other live plans

- **PLAN-V3-RUST.md** — superseded in shape, kept for §1's diagnosis and §3's invariant table
  (the checklist the Rust types are designed against). Its step 1 (draw boundaries in C#) is
  dropped; its step 3 (WPF over the Rust core) is replaced by the Flutter client; its §8.1 is
  answered here.
- **PLAN-INPUT-BACKEND.md §10.3** — "the seam is wanted by three plans now; it should be cut once,
  in one language, on purpose." This plan cuts it: the seam is the protocol plus `trait
  InputBackend` inside the core. The AHI backend, if ever built, is a core implementation behind
  that trait; the UI never sees it.
- **PLAN-AGENT.md** — same seam, same answer: agent commands and tool commands ride the protocol;
  the agent is a core client like the UI.
- **PLAN-DEPLOY.md** — its config-split reasoning is already in `ConfigLoader` and moves to the
  core as §6.1 describes; the plan itself is not rewritten.

---

## 13. Questions the player answers before step 1 ⚑

1. **Distribution.** The release stays a zip of a folder (as today), with a Flutter bundle + core
   exe + `models/` + `config/`, optionally wrapped in an Inno installer. Confirm, and confirm the
   version is **v3.0** (a user-visible contract change: new frontend, same tool behaviour, same
   firmware, same `local.yaml`).
2. **Chinese names.** Keep free-text Chinese preset/flow names (with the §9 mitigations and a
   Bopomofo spike), or switch those few fields to pick/auto-generated names that cannot hit the
   Flutter IME bugs at all? *(Recommend: keep, because typed names are how the player thinks; the
   spike decides whether the mitigation holds.)*
3. **Tools outlive the UI?** Recommended: closing the UI stops the tools and the core, exactly as
   today; a crashed UI is restartable and the core stops cleanly. A detached core (feeder running
   with no window) would be a new feature, not part of v3.0.
4. **Config ownership.** Core is the single writer of `local.yaml`; the UI sends merge intents for
   the blocks it owns; hand edits are picked up at startup or on `config.reload`; two instances
   refused by a lock. Confirm.
5. **Repo layout.** Add `v3/` beside `v1/` and `v2/`, keep `v2/` as the shipped app until cut-over,
   and keep `v3/docs/` as a fresh spec tree at step 7 (rather than migrating `v2/docs/`).
6. **Dependency policy.** Minimal and explicit: `serde`/`serde_json`, a maintained YAML crate,
   `windows`, `serialport`, `ort`, `image`, `tracing`, `proptest`/`insta` (dev); no async runtime
   unless a thread-per-tool design fails (std threads are enough here). On the Dart side: Material
   + one state solution (Riverpod preferred) and **no desktop plugins** in v3.0. Confirm — it is a
   learning project, so the "no magic" is a feature.
7. **Tool order** as in §10 (spammer + Hold Space pilot, pet last) and **v3.0 = all seven runnable
   tools + all fifteen tabs** (full parity) rather than a subset.
8. **Accessibility / system text scaling** — out of scope for v3.0, recorded in the backlog?
   *(v2 has no keyboard path and two automation names; matching that is not a regression, and it
   should be a deliberate deferral rather than an omission.)*
9. **Windows runner C++.** Allowed to edit the Flutter runner for chrome/Mica/minimum size instead
   of adopting `window_manager`-class plugins? *(Recommended: yes — the runner is ours and it
   avoids the plugin churn the research found.)*
10. **Recording the missing OCR corpora** needs one or two live sessions of v2 in the maintenance
    window, with logging on, exercising feeder counts, pet panel and attribute reads. When can
    those happen? *(Step 5's acceptance depends on them; without them, only the tuner path has a
    parity oracle.)*

---

## 14. What was measured, what was reasoned, what was not attempted

**Measured on 2026-10-25:** the file and line counts in §1 (re-counted on `main`); the 0 interfaces;
the publish payload and its breakdown; the 324 test cases and 9 projects; the seventh runnable
tool (`holdspace` in `LauncherService._toolRunners`); the OCR corpus (`v2/logs/captures` 3,366
PNGs, `v2/logs/ocr_log.jsonl` 9,326 records) and that the recognizer dictionary is embedded in
`RapidOCRSharpOnnx.dll` (18.5 MB, no dictionary file in `models/`); this machine's Windows build
(Windows 11 25H2, 26200) and its zh-Hant-HK Microsoft Bopomofo IME; `Hotkeys` is `GetAsyncKeyState`
polling, `ScreenCapture` is GDI, no tray/file-dialog/single-instance code exists; `ConfigLoader`'s
owned-block list, foreign-block preservation and atomic write as described in §6.1; and
`USER_GUIDE.md:587-589`'s hotkey/focus caveat.

**Read out of the research, dated and linked in §9:** every Flutter Windows behaviour claim; the
Avalonia/WebView2/WinUI comparison that chose Flutter's risks over the alternatives'; the .NET 10
WPF IME bug; the Flutter issue statuses (as of 2026-10-25).

**Reasoned, not measured:** that two processes will preserve the coordinate contract; that the
protocol's overhead is negligible at this app's rates (status at a few hertz, calibration frames at
human speed); that the UI is cheap enough to restart that GL-context loss becomes a non-event; the
stage costs; the claim that the shell can reach full parity with fifteen tabs.

**Not attempted:** no Rust or Flutter project exists, no crate chosen, no branch cut, no `.csproj`
or config file touched, no v3 directory created. This document changes nothing.

---

## 15. What is left to the builder (not the player)

- Transport framing details (NDJSON over stdio; base64 for images) and the exact envelope.
- The crate split inside `v3/core` and the Dart package structure inside `v3/shell`.
- The `DisplayInfo`/frame message shape and the drag-to-physical conversion.
- The `config.apply` delta granularity (per-block with a revision, field-wise merge in the core).
- The state management choice on the Dart side within §13.6's constraint.
- The replay-core script format and the golden-fixture layout.
- The order of the config port's internal steps (loader before writer, tests first).
- Naming: `SealTools.exe` for the client, `seal-core.exe` for the core, `v3/` for the tree.

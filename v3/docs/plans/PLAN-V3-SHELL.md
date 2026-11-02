# Plan — the v3 shell (Flutter)

**A proposal, not a description.** The core half of v3 has its own plan
([PLAN-V3-CORE.md](../../../v2/docs/plans/PLAN-V3-CORE.md)); this document is the plan for the SECOND
process — the Flutter shell that the player actually sees and clicks. It says what to build and why,
not whether it exists: a plan's state lives only on [the v3 plans board](README.md).

The shell is one of two processes. What is already true and must not be re-litigated by this plan:
the process split and its five reasons, the config ownership, the coordinate contract, the board and
OCR sides — all in [DESIGN.md](../spec/DESIGN.md), [PROTOCOL.md](../spec/PROTOCOL.md),
[CONFIG.md](../spec/CONFIG.md) and [CALIBRATION.md](../spec/CALIBRATION.md). This document links them
rather than restating them.

---

## 1. Decisions this plan builds to

Fixed before the design, not reopened here:

1. **The Dart protocol client is HAND-WRITTEN and fixture-tested, not generated.** The golden
   fixtures (`v3/protocol/fixtures/`; 49 files at write time, and the set grows with the protocol) are
   the contract. Codegen is rejected for v3.0 because the shapes are small and few, and a hand-written
   client can carry the request/response/event distinction in types a generator would flatten into one
   bag of maps.
2. **State management is built-in only.** One app-level `ChangeNotifier` model and its sub-models;
   no Riverpod/bloc/provider. Smaller supply chain, and the shell's state is one connection, one
   config copy and eight tool snapshots — not a state-management problem that needs a library.
3. **Wave 1 is five tool surfaces**, in this order: Tuner, Gem, Spammer, Buy/Sell (one card, two tab
   surfaces), Quest. **Wave 2 is everything else:** Pet, the five calibration surfaces (Calibrate
   Tuner, Calibrate Gem, Buy / Sell, Calibrate Pet, Calibrate Tooltip), Attributes, Arduino, Hotkeys,
   Setup.
4. **The shell never decides, the core never draws** ([DESIGN.md](../spec/DESIGN.md) §1/§2). No rule
   is reimplemented in Dart; the shell sends merge intents and renders snapshots.

Scope boundary: v3.0 is **feature parity with v2** — the same seven runnable tools plus Hold Space,
the same sixteen tabs, the same messages. It is a new frontend, not a new product.

## 2. Shape and lifecycle

```
SealTools.exe (Flutter, DPI-aware)
  └── spawns seal-core.exe from its own directory, no arguments
        spoke: UTF-8 NDJSON on the child's stdin/stdout; stderr = logs only
        close: EOF on the child's stdin  ⇒ core stops tools safely and exits 0
```

- **First frame before handshake.** The window shows a connection surface immediately, then starts
  the core and sends `hello {"protocol":1,"client":"<shell version>"}`. A core that cannot start
  (crash before `hello`) is shown as a crash with the captured stderr tail, not as a spinner forever.
- **Handshake outcomes** ([PROTOCOL.md](../spec/PROTOCOL.md) §1/§4): success gives `protocol`,
  `core_version`, `root`, `rev`, `config`; the shell stores all of it and renders. `bad_version`
  shows both versions and does not retry — a version mismatch is a packaging bug, not a transient.
  Any other pre-handshake failure is the crash path.
- **Boot refusal** is not a protocol message: the core exits non-zero with its reason on stderr
  (§1 "Boot refusal"). The shell captures that tail and shows it in the crash view. It must never
  parse stderr as protocol.
- **Crash and restart.** The process handle is watched; on any exit the shell keeps the last rendered
  state, marks the connection dead, shows the stderr tail, and offers **Restart core**. Restart is one
  click, one new process, one new handshake — no automatic loop, because an auto-restart against a
  broken config would flicker forever. The tools' real state is the game and the board, so a restart
  loses nothing already lost ([DESIGN.md](../spec/DESIGN.md) §1).
- **Closing the window** closes the shell, which closes stdin: the core cancels the running tool
  through its normal stop path and exits. A detached core (a feeder continuing with no window) is a
  new feature, not v3.0.
- **Single instance** is NOT the shell's to implement. The protocol specifies a core-held lock
  ([PROTOCOL.md](../spec/PROTOCOL.md) §1) and that lock is not built yet ([OPEN.md](../backlog/OPEN.md)
  §3); until it is, the shell does not add a second mechanism, and a second shell against one install
  is a known-unsupported state. If it is wanted sooner, it belongs in the Windows runner's C++ (a
  named mutex), not in Dart.
- **Shutdown surface:** a closing window shows nothing extra; the tools stop. A tool that must not be
  interrupted is the feeder's problem in the core (it is resident), not the UI's.

## 3. Package layout and tests

```
v3/shell/
  pubspec.yaml                 flutter, material; NO plugins, NO state library, NO codegen
  analysis_options.yaml        the repo's lint set, strict-casts on
  lib/
    main.dart                  runApp(ShellApp); no I/O before the first frame
    protocol/
      envelope.dart            Request, Response, Event, CoreError, protocol version
      types.dart               DisplayInfo, Frame, ToolSnapshot, ToolRow, config escapes
      codec.dart               encode/decode + the typed event-vs-response distinction
      client.dart              CoreClient: process, NDJSON transport, ids, pending map, event stream
    model/
      shell_model.dart         the ONE ChangeNotifier: connection, config, tools, logs
      connection.dart          state machine: starting → handshaking → ready → failed
      config_store.dart        merged JSON + rev + per-tab drafts (owned blocks only)
      tool_state.dart          Map<ToolId, ToolSnapshot> + finished reason + card lines
      log_buffer.dart          ring buffer from `log` events and core stderr
    ui/
      cards/                   card grid, mini mode, hold-space toggle
      tabs/                    one file per tab (wave 1 first)
      widgets/                 Section, Hint, LabeledField, Stepper, RowList,
                               CalibrationCanvas (ONE, wave 2)
      theme.dart               density tokens: spacing, font scale, colour
    dev/
      fake_core.dart           a Dart stand-in for the core, driving the same client
  test/
    protocol/fixtures_test.dart   decode EVERY fixture; field-set drift test
    protocol/codec_test.dart      request encoding, event/response discrimination
    model/*_test.dart             connection, config rev/conflict, tool-state transitions
    ui/*_test.dart                two or three widget tests (card fields, mini mode)
    support/                      fixture loading, fake core, golden root
  windows/runner/              chrome work lives in C++ here (Mica, min size, DPI manifest)
```

**Test strategy, in order of value:** (1) every fixture decoded into its Dart type, and a test that
the fixture's field set equals the Dart type's field set — that last one is the drift guard the
generator would have given us; (2) state-machine tests with no widgets — the connection, the
config/rev flow and the event→snapshot mapping are all plain Dart; (3) a small number of widget
tests for the card grid and mini mode. Golden-image tests are deliberately sparse: font AA differs by
machine (see §9) and a goldens-heavy suite would be brittle for the value it adds.

**Gates** (same three shapes as the Rust side): `dart format --output=none --set-exit-if-changed .`,
`flutter analyze`, `flutter test`. A warning is a failure.

## 4. The protocol client (hand-written, fixture-tested)

One file owns transport, one owns encoding, one owns the conversation.

- **Transport.** `CoreClient` starts the process (path = own directory + `seal-core.exe`; dev builds
  take an override), writes one compact JSON line per request, splits stdout on `\n` and decodes one
  line at a time. A half line is never decoded — stdout is buffered until a newline. stderr is
  drained into the log buffer and the crash tail, never parsed.
- **Types, not maps.** `Request`, `Response` (ok/error), `Event` are distinct Dart types: an event can
  never be mistaken for a response to a pending call, and a `stop` style bug cannot hide in a map.
  `Event` decodes to a sealed family: state, tool.finished, log, config.changed, core.stopping.
- **Request ids** are generated by the shell (`<seq>`), unique per connection, opaque to the core. A
  `Map<String, Completer<Response>>` resolves the reply; a timeout (10 s, a shell constant, not a
  protocol value) fails the request with a typed error rather than hanging the UI. Unknown ids are
  logged and dropped.
- **Forward compatibility.** Unknown fields are ignored on decode; unknown event names and error
  codes render as "unrecognised event/code" rather than throwing. This is what lets the shell run
  against a newer core during the port.
- **Fixture tests are the contract.** `test/protocol/fixtures_test.dart` walks
  `v3/protocol/fixtures/*.json` (shape: `{case, kind: request|response|event, message}`), decodes each
  into its Dart type, and asserts the type's field set matches the message's keys. It covers
  spec-only shapes too — `event.config.changed.json` decodes even though the core does not emit that
  event yet.

## 5. State: one model, no rules

`ShellModel extends ChangeNotifier` is created in `main.dart` and rendered through one `ListenableBuilder`
at the app root (an `InheritedNotifier` keeps it reachable by widgets; no third-party scoping). It owns
four plain sub-models and forwards their changes:

- **connection** — `starting | handshaking | ready | failed(reason)`: the lifecycle of §2. Every core
  failure lands here and the root builder switches to the crash/connection view.
- **config** — the merged JSON and `rev` from `hello`/`config.reload`, plus a `Draft` per tab holding
  ONLY the owned blocks that tab edits. A draft is plain data; the tab renders it, the model sends it.
- **tools** — `Map<ToolId, ToolSnapshot>` fed only by `state` events and cleared/settled by
  `tool.finished`. The card reads this map; a card never queries the core.
- **logs** — the ring buffer from `log` events + core stderr, for the status surface and the crash tail.

Rules the model enforces, all core-side policy (no widget re-implements them):

- a save is `config.validate` first, then `config.apply {rev, sections}`; `conflict` opens the
  reconcile sheet with the current `rev`/sections the core returned, and the player re-saves after
  seeing what changed ([PROTOCOL.md](../spec/PROTOCOL.md) §7);
- the core is the only writer; the shell never composes `local.yaml` text;
- a tool command is `tool.start {tool}` / `tool.stop {tool}`; `tool_busy` is shown as the core's own
  line, never rewritten in Dart;
- a button that starts a tool does not also start it optimistically: the card state comes from the
  core's `state` events.

## 6. UI surfaces

**The card grid** (the window's default view) has one card per tool id — `spammer`, `tuner`, `buy`,
`sell`, `gem`, `quest`, `pet` — plus the **Hold Space** toggle in the chrome (no card, per v2). A card
shows name, Start/Stop, and the `ToolSnapshot` fields: `phase` (as the engaged word), `message`,
`countdown_s` when present, `error`, and `rows` when the tool publishes them. **The `rows` inner shape
is being pinned by config-author (`task-24`) from `seal-tools::ToolRow` and PROTOCOL §5 — the card
renders that type (name/status/detail/next_action_in_seconds); this plan does not define a second
shape.** Pet's card is the resident exception: it keeps its standing line while another tool holds the
game; its row lines appear only if the row shape lands in time (⚑4).

Card ids and behaviour follow v2: Buy's card additionally carries the item dropdown and count stepper
(one choice shown in two places, seeded from the preset's usual count); Quest's card carries the
target picker and loop count; a running foreground tool collapses the window to that card ("mini
mode"), Pet never does.

**Wave 1 tabs** — port the v2 control semantics ([USER_GUIDE.md](../../../v2/docs/spec/USER_GUIDE.md)
is the behaviour reference; the widgets are new):

| Tab | Surface |
|---|---|
| Tuner | Goal (target grade, require grade, max retries), Spring (`manual`/`hid`, guard mode), Timing, Filter rules + overrides editors, Advanced (save captures, clean captures), Save |
| Gem | Start grade, On empty result, save-captures toggle, Save; the card shows the per-grade cycle tally |
| Spammer | Preset picker with rename/duplicate/delete, key tiles editor, advanced raw `key:seconds` text, Save preset |
| Buy | Item picker + Reload, Position (name, row, scroll notches, usual count), Save item / Delete / Dry run, Status line |
| Sell | 64-slot grid with row-select buttons, select all/clear, Dry run, Max slots per run + Save, Status line |
| Quest | Flow picker + rename/duplicate/delete, sequence picker + entries + master loops, steps editor, timing (initial wait, loops, guard px), Test one loop, Save quest |

**Wave 2** adds Pet, the five calibration surfaces (Calibrate Tuner, Calibrate Gem, Buy / Sell,
Calibrate Pet, Calibrate Tooltip — five canvases, one widget), Attributes (read-only dictionary
view), Arduino (connection + input test), Hotkeys, Setup (variant switch, window, reference window).

**The one CalibrationCanvas.** CustomPainter for the captured frame and the overlay; `GestureDetector`
for drag. It renders the frame at natural pixel size by default (fit-to-window with an explicit zoom
is allowed if shown), and turns a drawn rectangle into **client-relative physical offsets using only
the `DisplayInfo` the core sent** — the formula is [CALIBRATION.md](../spec/CALIBRATION.md) §4 and is
NOT restated here. The trap that section names is the one to code around: canvas zoom is not
`DisplayInfo.scale`. Save → `calibration.save {frame_id, region, kind}`; opening a calibrate tab, or
Discovering an existing install, fetches the saved reference (`calibration.fetch`) instead of
re-deriving it. The calibrator's capture flow is: hide the shell window, wait one frame, send
`capture`; the core refuses if the game is not foreground (`game_not_foreground`).

**Status surface:** a bottom strip showing the last log lines and the `root` path from `hello`;
errors appear as a dismissible banner with the core's message; the crash view is the same area in
full-window form with the stderr tail and Restart.

---

## 7. Windows specifics

Flutter on Windows is not the v2 surface; these are the known gaps and the plan's answer for each.
The research is PLAN-V3-CORE §9; the two that touch this document directly are DPI and IMEs.

| Behaviour | Effect | Plan |
|---|---|---|
| **DPI awareness** — the Flutter Windows runner ships a PerMonitorV2 manifest by default | the shell is DPI-aware while the core is unaware, which is the point | **confirm the template's manifest** (it is one of the generated files) and keep the core unaware; if the runner's manifest needs an edit, it is a runner-C++ change. The canvas/`DisplayInfo` split is CALIBRATION §2/§4 and does not depend on this beyond the shell's own sharpness |
| **CJK IME bugs** — flutter#190042 (composition dies after a long focus loss), #191196 (a third-party IME leaks text across fields) | preset/flow name fields could corrupt or stop accepting Chinese | the mitigation is an app-lifecycle listener that unfocuses the text client on window deactivate (PLAN §7); **spike with this machine's Microsoft Bopomofo before the first tab with a name field** — this is a go/no-go for wave 1, not a detail. Fallback: native EDIT overlay, or generated names (PLAN-V3-CORE §13.2's answer) |
| **Grayscale AA only** — no ClearType/DirectWrite (#63043, still so in #194931) | 12–14 px Chinese text softer than native at 100 % | at this machine's 150 % the v2 UI is bitmap-stretched, so Flutter is likely sharper; **verify side-by-side in the first skeleton**, and if it bites, raise the minimum font size rather than fight the renderer |
| **GL context loss** (`EGL_CONTEXT_LOST` on sleep/resume/TDR) | frozen/black window after waking | restart the shell (tools are in the core and keep running); the crash/restart path of §2 covers it |
| **Impeller warm-up** (#193048) | slow first paint on some drivers | measure in the skeleton; Skia fallback if it bites |
| **No screen-coordinate API** | a canvas mapping to game pixels | not a limitation here: the core sends frame + `DisplayInfo`, the shell maps with `DisplayInfo` only |
| **Hotkeys** — PROTOCOL §11.1: whether a background core can read keys while the launcher is focused is unmeasured | determines whether hotkeys stay core-side or move to the shell | step-1 spike in the core; **if it fails, the shell polls `GetAsyncKeyState` in the runner/FFI and sends hotkey intent to the core** — that is a documented exception to "no Win32 in the shell" (PLAN §6.5), decided only after the spike |
| **UIA/semantics** (#192782) | screen-reader hints incomplete | v2 has two automation names; accessibility is out of scope for v3.0 and recorded as a deliberate deferral, not an omission |

## 8. Development and build pipeline

- **SDK location.** The Flutter SDK is being installed at `C:\src\flutter` (seen at write time; not on
  `PATH` yet, and `flutter doctor` has not run). This document's build commands assume
  `C:\src\flutter\bin` on `PATH`; the shell's `README.md` will record the exact `flutter --version`
  it was built with once `flutter doctor` is clean — **verify after install**.
- **Project creation.** `flutter create --platforms=windows --org seal.tools --project-name seal_shell
  v3/shell` then delete the generated counter app and the `flutter_test` example; **no plugins added**
  (the runner's C++ does chrome/Mica; file dialogs do not exist; the core does I/O).
- **The three Dart gates** are `dart format --output=none --set-exit-if-changed .`,
  `flutter analyze`, `flutter test`, run from `v3/shell`; `flutter build windows` is the packaging
  step, not a gate.
- **Running during development without the game or the board.** Two modes, both first-class:
  1. **`lib/dev/fake_core.dart`** — a Dart executable speaking the same NDJSON protocol and serving
     the golden fixtures, so `flutter run -d windows` shows realistic cards/events with no core
     binary; widget tests use it in-process.
  2. **the real core with `--root <test-fixture-dir>`** — `seal-core` already accepts `--root`
     ([DESIGN.md](../spec/DESIGN.md) §3); the shell's dev override points at it. A `--replay
     script.json` core mode is PLAN-V3-CORE §8's idea and does not exist yet; the fake core is the
     stand-in, and the real replay flag is a core-side task when it is wanted.
- **Windows runner edits** are allowed and expected: window chrome, minimum size, Mica, and the DPI
  manifest confirmation. The runner is ours; `window_manager`-class plugins are not adopted.

## 9. Packaging and cut-over

The release is a ZIP OF A FOLDER, unchanged in shape
([PLAN-DEPLOY.md](../../../v2/docs/plans/PLAN-DEPLOY.md) §4 is the layout and the update rules; the
shell must honor them by never writing `config/`):

```
SealTools/                     the folder is the unit that moves
  SealTools.exe                the Flutter shell (renamed from the runner's build output)
  *.dll, data/                 the Flutter Windows bundle
  seal-core.exe                the core, beside the shell
  config/                      program templates + user state (PLAN-DEPLOY §4)
  docs/pet-data.csv            the feeding table the core loads
  logs/                        created by the core, never by the shell
```

- **The shell finds the core beside itself**, not through `PATH` ([PROTOCOL.md](../spec/PROTOCOL.md)
  §1); the core finds the root by walking up for `config/defaults.yaml(.example)`.
- **Version handshake:** `hello`'s `protocol`/`client` and the core's `core_version` are shown on the
  crash/`bad_version` path. The shell and core ship together, so a mismatch means a mixed install —
  the shell says so instead of retrying.
- **v2 → v3 cut-over gate** (all must hold before the v3 zip replaces the v2 one):
  all eight tool ids runnable end-to-end in the game; the sixteen tabs present; **OCR parity per read
  path** (tuner, feeder counts, pet panel, attributes — the corpus record is PLAN-V3-CORE §8, not one
  number); protocol fixtures green on both sides; **a v2-written `local.yaml` loads unchanged**
  (inherited hard requirement); the firmware is unchanged — no reflash; and a live pass of every tool
  with the same observable behaviour as v2.
- The v2 tag frozen as the parity baseline stays the reference until cut-over; later v2 fixes are
  ported deliberately or deferred, never silently.

## 10. Risks, in the order they would hurt

1. **The IME spike fails.** Name fields are the only Chinese text input in the shell; if Bopomofo
   mitigations fail, the fallbacks are an overlay edit control or generated names — decided after the
   spike, before wave 1's Spammer/Quest tabs (which have name fields) are built.
2. **The hotkey spike moves input into the shell.** That adds a Win32/FFI surface the design avoided;
   it is contained (poll → intent → core) but it changes the runner/one plugin decision.
3. **The `rows` shape lands late.** The pet card and the per-row detail are the only surfaces that
   need it; wave 1 can ship without it (⚑4).
4. **The Dart config mirror drifts from the Rust model.** Mitigation: fixtures for protocol shapes,
   and the config blocks are edited as opaque JSON through per-tab drafts — the shell copies only the
   field names a tab renders, and `config.validate` is the authority.
5. **Flutter Windows surprises** (GL context, Impeller paint) despite the mitigations; the crash/restart
   path and "tools live in the core" are the structural answer.
6. **Schedule.** Two toolchains installing in parallel means the first flutter build is not the
   critical path; the fixture/fake-core work can start the day `flutter test` runs.

## 11. ⚑ Open questions for the player

⚑ 1. **Startup view.** Cards only, as v2 opens — or reopen on the last tab you were using?
   *(Recommend: cards only; the configuration is a deliberate extra step and the window is small.)*
⚑ 2. **Theme.** Dark only, matching v2, or follow the Windows light/dark setting? *(Recommend: dark only
   for v3.0; a second theme is a token-set change, not a rewrite, and can wait.)*
⚑ 3. **Hotkeys, if the core-side spike fails.** Move the keyboard poll into the shell (documented
   exception to "no Win32 in the shell") — or accept Stop-button-only for v3.0 and keep hotkeys out
   entirely? *(Recommend: shell poll, because the quit hotkey is muscle memory; but it is a
   behaviour decision, not ours.)*
⚑ 4. **The pet card's row lines.** If the `rows` shape is pinned in time, show the per-row lines in
   v3.0; if not, hold them and show only the standing schedule line? *(Recommend: hold; the card is
   still honest without rows, and the type churn is not worth rushing.)*
⚑ 5. **Upgrade path.** Install v3 as a separate folder beside v2 (both launchable, `config/` copied
   once by the player — PLAN-DEPLOY's migration rules) — or replace the v2 folder in place, with the
   fold handling `local.yaml`? *(Recommend: separate folder for v3.0 so a fallback exists; the fold
   is proven but the player is the one who has to trust it.)*

# v3 shell — the Flutter UI

The DPI-aware half of v3. It owns **no rules**: every decision comes from the Rust core over the
protocol ([`v3/docs/spec/PROTOCOL.md`](../docs/spec/PROTOCOL.md)) and this app only renders, collects
edits and forwards them. The two-process reason and the boundaries are
[`DESIGN.md`](../docs/spec/DESIGN.md) §1; the build plan is
[`PLAN-V3-SHELL.md`](../docs/plans/PLAN-V3-SHELL.md) (status on its board, not here).

## Toolchain (installed 2026-10-25)

| What | Where | Version |
|---|---|---|
| Flutter SDK | `C:\src\flutter` (**not** on `PATH` yet — call `C:\src\flutter\bin\flutter.bat`) | stable 3.47.8 |
| Dart | ships with the SDK (`C:\src\flutter\bin\dart.bat`) | 3.13.7 |
| Windows toolchain | VS Build Tools 2022 (VCTools + SDK 10.0.26100), MSVC 14.44 | `flutter doctor` clean for Windows |

`flutter doctor` still reports the Android toolchain missing and the SDK not on `PATH`; both are
irrelevant/warnings for this Windows-only project.

## Gates (the same three, every change)

```powershell
$env:FLUTTER_SUPPRESS_ANALYTICS = "true"
C:\src\flutter\bin\flutter.bat analyze
C:\src\flutter\bin\flutter.bat test
C:\src\flutter\bin\dart.bat format --output=none --set-exit-if-changed lib test
```

All three must be clean, matching the Rust side's fmt/clippy/test rule.

## Layout

```
lib/
  main.dart          the entry point: resolves the core/root, spawns, runs ShellApp
  app/               the one ChangeNotifier, its sub-models, drafts, core actions, path rules
    README.md        the model/draft/action contracts (read this before adding a tab)
  protocol/          the hand-written typed protocol client + the wire types
  ui/                the card grid, the eight tabs, the shared tab frame and fields
test/                fixture-decode tests, model tests, widget tests
windows/             the generated runner (built by `flutter build windows`)
```

The protocol client is **hand-written and fixture-tested**, not generated — decision and reasoning in
PLAN-V3-SHELL §4. Its tests decode every golden fixture in `v3/protocol/fixtures/` (the set grows with
the protocol; decoder coverage is the guard).

## Import rules — the three type namespaces

The shell sits between three different type vocabularies, and mixing them up costs a checkpoint each
time. Import only what a file means:

| namespace | imported from | what it holds | who imports it |
|---|---|---|---|
| **Flutter** | `package:flutter/material.dart`, `dart:ui as ui` | widgets, `ThemeData`, Flutter's `Size`/`Point`/`Rect`/`Offset` | `lib/ui/**` always; `lib/main.dart` |
| **app barrel** | `lib/app/app.dart` | `ShellModel`, `ConfigDraft`/`ConfigDraftGroup`/`ConfigScope`, `ConfigSaveOutcome`, `CoreActionResult` + action result types, sub-models | `lib/ui/**`, `lib/main.dart` |
| **protocol library** | `lib/protocol/protocol.dart` | wire types: `DisplayInfo`, `Rect`, `Frame`, `ToolSnapshot`/`ToolRow`, `ToolId`, `Response`/`Event`, `Transport` | `lib/app/**` (it speaks the protocol); tabs only when they touch wire types (e.g. Setup's `DisplayInfo`) |

Rules of thumb, earned by three real failures:

- A tab imports Flutter + the app barrel; it adds the protocol library **only for wire types**. Setup
  needed `DisplayInfo` from `lib/protocol/protocol.dart`, not from the barrel.
- `lib/app` imports the protocol library; it never imports `lib/ui`.
- `lib/protocol` imports neither Flutter nor the app layer — no exceptions.
- Widget tests import the app barrel (or a specific tab), Flutter/`flutter_test`, and support files;
  they may import the protocol library for payload shapes.
- Helper builders that live in test support (`test/support/fake_transport.dart`) are **not** re-exported
  by accident: `test/support/app_harness.dart` re-exports only the raw response builders it promises
  (`okResponse`, `errorResponse`, `helloResult`). A missing name there is an import error, not a
  protocol problem — the fix is the import, never a copy of the helper.
- Flutter's `Size`/`Point`/`Rect` collide with the protocol's §3 types. In UI/test code write
  `import 'dart:ui' as ui;` and qualify (`ui.Size`), or `hide Size` on the protocol import; keep the
  protocol names inside protocol code and the app layer.
- **The named example:** `lib/ui/tabs/calibration_widgets.dart` decodes the wire `Frame` to plain
  ints and `ui.Rect` at the boundary and does all painting/gesture geometry in `ui.*`. When that file
  once imported the protocol for `Frame`, its `Rect`/`Size` shadowed Flutter's and the analyzer
  reported nine errors at once, the fifth checkpoint lost to this class. A mechanical `ui.` sweep
  also over-reaches: it once turned `SizedBox` into `ui.SizedBox`, which is not a type. After such a
  sweep, list every `ui.<Name>` identifier used and check each exists in `dart:ui`.

When a new type is added, ask which namespace owns it before writing the import: **wire shape →
protocol; shell state/decision → app; rendering/geometry → Flutter**.

## Running during development

The release binary is **`SealTools.exe`** (`windows/CMakeLists.txt`'s `BINARY_NAME`, CUTOVER.md
decision 2); development is unchanged. `flutter run -d windows` starts the app; `main.dart` spawns
`seal-core.exe` (release: beside the shell; development: `v3/core/target/debug`, root `<repo>/v3`) with
`--root`. `SEAL_CORE` / `SEAL_CORE_ROOT` dart-defines override the path and root (PLAN-V3-SHELL §8).

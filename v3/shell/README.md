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
  main.dart          the app entry (generated; replaced by the shell model)
  protocol/          the hand-written typed protocol client + the wire types
  model/             the one app-wide ChangeNotifier and its sub-models
  ui/                the card grid and the tab surfaces
test/                fixture-decode tests, state tests, a few widget tests
windows/             the generated runner (built by `flutter build windows`)
```

The protocol client is **hand-written and fixture-tested**, not generated — decision and reasoning in
PLAN-V3-SHELL §4. Its tests decode the 50 golden fixtures in `v3/protocol/fixtures/`.

## Running during development

`flutter run -d windows` starts the app; the app spawns `seal-core.exe` from its own directory. For UI
work without a core, PLAN-V3-SHELL §8 describes the FakeCore replay loop (`--root` points the core at
a scratch config tree).

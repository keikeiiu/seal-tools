# The shell's protocol client

The hand-written Dart side of [PROTOCOL.md](../../../docs/spec/PROTOCOL.md). It is **not generated**:
the golden fixtures in `v3/protocol/fixtures/` are the contract, and
`test/protocol/fixtures_test.dart` decodes every one of them (see
[PLAN-V3-SHELL.md](../../../docs/plans/PLAN-V3-SHELL.md) §4 for why codegen was rejected).

## Files

| file | what it is |
|---|---|
| `json.dart` | `JsonMap`, typed decode helpers and `ProtocolDecodeException` |
| `envelope.dart` | `protocolVersion`, `Request`, sealed `Response`, `ErrorObject`/`ErrorCode` |
| `types.dart` | the payloads the shell reads now: `HelloResult`, `DisplayInfo`, `Frame`, `ToolSnapshot`/`ToolRow`, `ToolFinished`, `LogLine`, `CoreStopping`, `ConfigChanged`, the config command results |
| `events.dart` | sealed `ProtocolEvent` family (state / tool.finished / log / config.changed / core.stopping / unknown) |
| `args.dart` | typed arguments for the commands the shell sends today |
| `framing.dart` | NDJSON encode/decode; a response and an event are different TYPES |
| `client.dart` | `Transport` seam, `ProtocolClient`: ids, pending map, timeouts, handshake, event stream |
| `process_transport.dart` | the real transport over a spawned `seal-core.exe` |

## The contract

- **One object per line**, UTF-8, `\n` terminated. The encoder emits compact JSON; the decoder accepts
  one optional trailing `\r` and rejects blank lines, invalid JSON, a wrong `v`, and envelopes that are
  neither a response nor an event — each with a typed `ProtocolDecodeException`.
- **Responses are correlated by `id`; events are not.** The distinction is in the type system:
  `IncomingResponse` vs `IncomingEvent`, `OkResponse`/`ErrorResponse` vs the sealed `ProtocolEvent`
  family. Nothing "returns null on error".
- **Timeouts and closure are typed.** `ProtocolTimeoutException`, `ProtocolClosedException`,
  `ProtocolStateException`, `HandshakeException`, `CoreCommandException`.
- **Config is opaque.** Blocks are `JsonMap`; the shell sends merge intents and never models a block it
  does not own.
- **Unknown event names decode to `UnknownEvent`** (forward compatibility); unknown fields are ignored
  everywhere. A KNOWN shape that is malformed fails loudly.
- **Closing:** `close()` sends `shutdown` best-effort, then EOF on the core's stdin — the EOF is the
  safe stop. `terminate()` is the restart path after a wedged core.

## Adding a protocol shape

1. **Fixture first.** The Rust side writes the golden fixture (serialising the real type) and the file
   lands in `v3/protocol/fixtures/`. The fixture is the cross-language contract; do not hand-write it.
2. **Type next.** Add the Dart model in `types.dart` / `events.dart` with a `fromJson` that uses the
   strict helpers, and a `toJson` whose keys are exactly the fixture's keys.
3. **Decoder map.** Add the case name to the explicit map in `test/protocol/fixture_decoders.dart`.
4. **Tests.** `fixtures_test.dart` fails until the map covers the new case and the field-set drift test
   compares the decoded keys with the fixture's; a request fixture is round-tripped byte-for-byte at the
   JSON object level.
5. **Command helpers.** If the shell sends it, add typed args in `args.dart` and a typed method on
   `ProtocolClient`; if not, the fixture still decodes through `Request`.

## Name collisions with Flutter

The §3 geometry types are deliberately named like Flutter's: `Size` (fields `w`/`h`), `Point`, `Rect`.
A file that imports both `package:flutter/material.dart` and `package:seal_shell/protocol/protocol.dart`
therefore has an ambiguous `Size`. In UI/tests write one of:

```dart
import 'dart:ui' as ui;
// ... const ui.Size(1600, 2400) for Flutter geometry

import 'package:seal_shell/protocol/protocol.dart' hide Size;
// ... or hide the protocol names you do not mean
```

Keep the protocol names in protocol code (the fixture decoder map needs them); qualify at the Flutter
boundary instead. This hit `test/ui/shell_page_test.dart` first — the test viewport is Flutter `ui.Size`,
not the protocol's `Size`.

Run the three gates from `v3/shell`:

```powershell
C:\src\flutter\bin\flutter.bat analyze
C:\src\flutter\bin\flutter.bat test
C:\src\flutter\bin\dart.bat format --output=none --set-exit-if-changed lib test
```

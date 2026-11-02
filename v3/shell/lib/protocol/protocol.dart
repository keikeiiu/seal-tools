/// The v3 shell's protocol client — the hand-written, fixture-tested Dart side of `PROTOCOL.md`.
///
/// Start here: `ProtocolClient` (`client.dart`) is the conversation, `framing.dart` is the line
/// format, `types.dart`/`events.dart` are the payloads, `args.dart` is what the shell sends. Nothing
/// in this library models a config BLOCK: config travels as `Map<String, dynamic>`, because the shell
/// copies what it does not own.
///
/// The contract of this library, and how to add a protocol shape, are in `README.md` here.
library;

export 'args.dart';
export 'client.dart';
export 'envelope.dart';
export 'events.dart';
export 'framing.dart';
export 'json.dart';
export 'process_transport.dart';
export 'types.dart';

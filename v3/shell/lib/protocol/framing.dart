/// NDJSON framing: one UTF-8 JSON object per line, exactly as [PROTOCOL.md] §1/§2 defines it.
///
/// The encoder emits COMPACT JSON plus one `\n` — the core reads by line and a pretty-printed object
/// would span lines. The decoder accepts a single line without its terminator (that is what the line
/// splitter hands it) and tolerates one trailing `\r`, because Windows pipes are not guaranteed to be
/// polished at the boundary even though the protocol says `\n`; it still rejects blank lines, invalid
/// JSON and envelopes that are neither a response nor an event with a typed
/// [ProtocolDecodeException] rather than a raw throw.
///
/// The stream helpers never buffer beyond one line: the line splitter is what bounds memory (a frame
/// is legitimately megabytes on one line, so there is no arbitrary frame-size cap here — the transport
/// and the core are ours).
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'dart:convert';

import 'envelope.dart';
import 'events.dart';
import 'json.dart';

/// Encodes one envelope object as one protocol line (compact JSON + `\n`).
///
/// `jsonEncode` inserts no whitespace, so the result is exactly one line; the caller writes it as-is.
String encodeMessage(JsonMap message) => '${jsonEncode(message)}\n';

/// Encodes a request ([PROTOCOL.md] §2).
String encodeRequest(Request request) => encodeMessage(request.toJson());

/// One decoded incoming line: a response to correlate, or an event to broadcast.
sealed class IncomingMessage {
  const IncomingMessage();
}

/// A response — it answers a request by [OkResponse.id]/[ErrorResponse.id].
final class IncomingResponse extends IncomingMessage {
  const IncomingResponse(this.response);

  final Response response;
}

/// An event — it has no id and is never awaited.
final class IncomingEvent extends IncomingMessage {
  const IncomingEvent(this.event);

  final ProtocolEvent event;
}

/// An event line whose payload could not be decoded, kept as a NON-FATAL marker (SHELL-F1).
///
/// A newer core may add a phase, stop reason or field this shell predates; one such event must not
/// end the connection and leave the core running unreferenced. The client logs this and forwards it
/// as an [UnknownEvent]; only a RESPONSE that cannot be correlated stays fatal, because nothing else
/// will ever settle its request.
final class IncomingMalformedEvent extends IncomingMessage {
  const IncomingMalformedEvent({
    required this.error,
    this.eventName,
    this.eventData,
  });

  /// The decode error, for the status log.
  final ProtocolDecodeException error;

  /// The event's wire name when the envelope itself was readable.
  final String? eventName;

  /// The event's `data` object when it was an object.
  final JsonMap? eventData;
}

/// Decodes one line of the protocol.
///
/// Throws [ProtocolDecodeException] for a blank line, invalid JSON, a non-object, a wrong `v`, or an
/// envelope that is neither `ok` nor `event`. Those are protocol violations, not command errors.
IncomingMessage decodeLine(String line) {
  // One optional `\r`: tolerate CRLF while keeping the rest byte-exact (a line with trailing spaces
  // is still accepted by the JSON parser, which is the right kind of tolerance for a wire format).
  final trimmed = line.endsWith('\r')
      ? line.substring(0, line.length - 1)
      : line;
  if (trimmed.trim().isEmpty) {
    throw const ProtocolDecodeException('blank protocol line');
  }

  final Object? decoded;
  try {
    decoded = jsonDecode(trimmed);
  } on FormatException catch (error) {
    throw ProtocolDecodeException(
      'line is not valid JSON',
      cause: error,
      found: trimmed.length > 80 ? '${trimmed.substring(0, 77)}...' : trimmed,
    );
  }

  final message = decodeMap(decoded, 'line');
  final version = decodeInt(message['v'], 'v');
  if (version != protocolVersion) {
    throw ProtocolDecodeException(
      'unsupported protocol version',
      path: 'v',
      expected: '$protocolVersion',
      found: version,
    );
  }

  if (message.containsKey('event')) {
    return IncomingEvent(decodeEvent(message));
  }
  if (message.containsKey('ok')) {
    final ok = message['ok'];
    if (ok is! bool) {
      throw ProtocolDecodeException(
        'expected a boolean',
        path: 'ok',
        expected: 'boolean',
        found: ok,
      );
    }
    return IncomingResponse(
      ok ? OkResponse.fromJson(message) : ErrorResponse.fromJson(message),
    );
  }

  throw const ProtocolDecodeException(
    'envelope is neither a response nor an event',
    expected: 'ok or event',
  );
}

/// When a decoded line fails and the line is identifiably an EVENT, returns a non-fatal marker for it.
///
/// Only an `event` envelope qualifies: its payload is informational, so the shell can log it and keep
/// reading. Everything else — invalid JSON, a non-object, a wrong version, a malformed RESPONSE —
/// returns null and the caller rethrows, because a response that cannot be correlated leaves a pending
/// request hanging and is the one case that must end the connection (SHELL-F1).
IncomingMalformedEvent? classifyMalformedEvent(
  String line,
  ProtocolDecodeException error,
) {
  final trimmed = line.endsWith('\r')
      ? line.substring(0, line.length - 1)
      : line;
  if (trimmed.trim().isEmpty) {
    return null;
  }
  final Object? decoded;
  try {
    decoded = jsonDecode(trimmed);
  } on FormatException {
    return null;
  }
  if (decoded is! Map<String, dynamic>) {
    return null;
  }
  if (!decoded.containsKey('event')) {
    return null;
  }
  final name = decoded['event'];
  final data = decoded['data'];
  return IncomingMalformedEvent(
    error: error,
    eventName: name is String ? name : null,
    eventData: data is Map<String, dynamic> ? data : null,
  );
}

/// Decodes a stream of lines into messages.
///
/// A malformed line that is identifiably an EVENT is delivered as [IncomingMalformedEvent] and the
/// stream continues; any other protocol violation ends the stream (see [classifyMalformedEvent]), so a
/// response that cannot be correlated never leaves its request pending forever.
Stream<IncomingMessage> decodeLines(Stream<String> lines) async* {
  await for (final line in lines) {
    try {
      yield decodeLine(line);
    } on ProtocolDecodeException catch (error) {
      final malformed = classifyMalformedEvent(line, error);
      if (malformed == null) {
        rethrow;
      }
      yield malformed;
    }
  }
}

/// Decodes a stream of raw bytes into messages: UTF-8, split on `\n`.
///
/// A UTF-8 sequence split across two chunks is fine — `utf8.decoder` is a chunked converter, and the
/// line splitter runs after decoding, which is the order that keeps multi-byte Chinese text intact.
Stream<IncomingMessage> decodeByteStream(Stream<List<int>> bytes) =>
    decodeLines(bytes.transform(utf8.decoder).transform(const LineSplitter()));

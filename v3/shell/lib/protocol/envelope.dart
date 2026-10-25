/// The protocol envelope: the three message kinds of [PROTOCOL.md] §2 and §6.
///
/// The distinction between a RESPONSE and an EVENT is in the type system here, not in a map: a
/// response can be correlated with a request by id, an event cannot, and a caller that mixes them up
/// is a compile error rather than a silent drop. Payloads the shell does not own (config blocks, the
/// core's opaque `detail` maps) stay [JsonMap] on purpose — the shell must not model config it does
/// not write.
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'json.dart';

/// The protocol version carried in every message's `v` ([PROTOCOL.md] §2).
const int protocolVersion = 1;

/// One request to the core.
class Request {
  const Request({required this.id, required this.cmd, this.args = const {}});

  /// Chosen by the shell, unique on the connection, opaque to the core.
  final String id;

  /// One of [PROTOCOL.md] §4's command names.
  final String cmd;

  /// The command's arguments, opaque at this layer.
  final JsonMap args;

  JsonMap toJson() => {
    'v': protocolVersion,
    'id': id,
    'cmd': cmd,
    'args': args,
  };

  /// Decodes a request envelope, checking `v` and the required string fields.
  factory Request.fromJson(JsonMap message) {
    final v = decodeInt(message['v'], 'v');
    if (v != protocolVersion) {
      throw ProtocolDecodeException(
        'unsupported protocol version',
        path: 'v',
        expected: '$protocolVersion',
        found: v,
      );
    }
    return Request(
      id: decodeString(message['id'], 'id'),
      cmd: decodeString(message['cmd'], 'cmd'),
      args: decodeNullableMap(message['args'], 'args') ?? const {},
    );
  }
}

/// A payload decoded from the wire, encodable back for the drift test.
abstract interface class WirePayload {
  JsonMap toJson();
}

/// A response to a request — success or failure ([PROTOCOL.md] §2).
///
/// It is sealed so a caller must handle both halves; a convenience accessor like [result] returning
/// null on an error response is exactly the swallowed-error shape this type exists to prevent.
sealed class Response {
  const Response({required this.id});

  /// The id of the request this answers, echoed by the core.
  final String id;
}

/// A successful response. `result` is the command's payload, decoded by the caller that knows the
/// command (the client's typed helpers do).
final class OkResponse extends Response {
  const OkResponse({required super.id, required this.result});

  final JsonMap result;

  factory OkResponse.fromJson(JsonMap message) => OkResponse(
    id: decodeString(message['id'], 'id'),
    result: decodeMap(message['result'], 'result'),
  );
}

/// A failed response; [error] carries the code, message and optional detail ([PROTOCOL.md] §6).
final class ErrorResponse extends Response {
  const ErrorResponse({required super.id, required this.error});

  final ErrorObject error;

  factory ErrorResponse.fromJson(JsonMap message) => ErrorResponse(
    id: decodeString(message['id'], 'id'),
    error: ErrorObject.fromJson(
      decodeMap(message['error'], 'error'),
      path: 'error',
    ),
  );
}

/// The error codes the protocol defines ([PROTOCOL.md] §6).
///
/// [unknown] exists deliberately: a newer core adding a code must not crash an older shell's
/// decoder, and the raw code is kept on [ErrorObject.rawCode] so the UI can still show something
/// honest. Every code the spec defines is a named value, and the fixtures cover each one.
enum ErrorCode {
  badRequest('bad_request'),
  badVersion('bad_version'),
  notReady('not_ready'),
  conflict('conflict'),
  windowNotFound('window_not_found'),
  gameNotForeground('game_not_foreground'),
  boardUnavailable('board_unavailable'),
  ocrUnavailable('ocr_unavailable'),
  toolBusy('tool_busy'),
  notRunning('not_running'),
  io('io'),
  configInvalid('config_invalid'),
  internal('internal'),
  unknown('');

  const ErrorCode(this.wire);

  /// The code as the protocol spells it.
  final String wire;

  /// A code not in the spec; the raw string is kept on the error object.
  static ErrorCode parse(String wire) => values.firstWhere(
    (code) => code.wire == wire,
    orElse: () => ErrorCode.unknown,
  );
}

/// One error from the core. `detail` is deliberately opaque ([PROTOCOL.md] §6 fixes each code's
/// detail only loosely), with typed accessors for the details the shell acts on.
class ErrorObject {
  const ErrorObject({
    required this.code,
    required this.rawCode,
    required this.message,
    required this.detail,
  });

  final ErrorCode code;

  /// The code as received; equals [code.wire] for every known code.
  final String rawCode;

  /// A message safe to show the player.
  final String message;

  /// Code-specific detail; never null, `{}` when the core sent none.
  final JsonMap detail;

  factory ErrorObject.fromJson(JsonMap message, {String path = 'error'}) {
    final rawCode = decodeString(message['code'], '$path.code');
    return ErrorObject(
      code: ErrorCode.parse(rawCode),
      rawCode: rawCode,
      message: decodeString(message['message'], '$path.message'),
      detail: decodeNullableMap(message['detail'], '$path.detail') ?? const {},
    );
  }

  JsonMap toJson() => {'code': rawCode, 'message': message, 'detail': detail};

  /// The player-facing validator lines on `config_invalid` ([PROTOCOL.md] §6).
  List<String> get validationErrors {
    final errors = detail['errors'];
    if (errors is List) {
      return [
        for (final line in errors)
          if (line is String) line,
      ];
    }
    return const [];
  }

  /// The current revision on `conflict` — what the shell reconciles against.
  int? get conflictRev => detail['rev'] is int ? detail['rev'] as int : null;

  /// The current values of the conflicting sections on `conflict`.
  JsonMap? get conflictSections {
    final sections = detail['sections'];
    return sections is Map<String, dynamic> ? sections : null;
  }

  /// The tool that holds the game on `tool_busy`.
  String? get busyTool =>
      detail['tool'] is String ? detail['tool'] as String : null;

  /// The file a failed write was about, on `io`.
  String? get ioPath =>
      detail['path'] is String ? detail['path'] as String : null;

  /// The protocol version the core expected, on `bad_version`.
  int? get expectedProtocol =>
      detail['expected'] is int ? detail['expected'] as int : null;

  /// The window title the core looked for, on `window_not_found`.
  String? get windowTitle =>
      detail['title'] is String ? detail['title'] as String : null;

  /// The port, on `board_unavailable`.
  String? get boardPort =>
      detail['port'] is String ? detail['port'] as String : null;

  /// Why the OCR models cannot serve a command, on `ocr_unavailable`
  /// (`models_env_unset`, `models_dir_missing` or `model_load_failed:<file>`).
  String? get ocrReason =>
      detail['reason'] is String ? detail['reason'] as String : null;
}

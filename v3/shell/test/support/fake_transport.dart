/// A [Transport] with no process: the client's state machine under the test's control.
library;

import 'dart:async';
import 'dart:convert';

import 'package:seal_shell/protocol/protocol.dart';

class FakeTransport implements Transport {
  FakeTransport();

  // Buffered single-subscription streams, like the real stdout/stderr: an event emitted before the
  // client's decoder has subscribed is not lost, which a broadcast stream would drop.
  final _incoming = StreamController<String>();
  final _diagnostics = StreamController<String>();

  /// Every line the client wrote, as sent.
  final sent = <String>[];

  /// Called after a line is recorded; the test answers here.
  void Function(String line)? onWrite;

  bool inputClosed = false;
  bool terminated = false;

  @override
  Stream<String> get lines => _incoming.stream;

  @override
  Stream<String> get diagnostics => _diagnostics.stream;

  @override
  void writeLine(String line) {
    sent.add(line);
    onWrite?.call(line);
  }

  @override
  Future<void> closeInput() async {
    inputClosed = true;
  }

  @override
  Future<void> terminate() async {
    terminated = true;
  }

  /// Sends one complete line to the client.
  void emit(String line) => _incoming.add(line);

  /// Sends one JSON object as the core would: compact JSON, no trailing newline (the real transport's
  /// line splitter removes it; the decoder also tolerates one, which is tested in framing).
  void emitJson(Map<String, dynamic> message) => emit(jsonEncode(message));

  /// Sends a stderr line.
  void emitDiagnostic(String line) => _diagnostics.add(line);

  /// Closes the incoming stream: the core's output ended.
  void closeIncoming() => _incoming.close();

  /// Closes both streams, for a full teardown.
  Future<void> dispose() async {
    if (!_incoming.isClosed) {
      await _incoming.close();
    }
    if (!_diagnostics.isClosed) {
      await _diagnostics.close();
    }
  }

  /// Parses a line the client wrote and returns its JSON object.
  Map<String, dynamic> sentJson(int index) =>
      jsonDecode(sent[index]) as Map<String, dynamic>;

  /// The id of the last request written.
  String lastId() => sentJson(sent.length - 1)['id'] as String;

  /// The command of the last request written.
  String lastCmd() => sentJson(sent.length - 1)['cmd'] as String;
}

/// A success response object.
Map<String, dynamic> okResponse(String id, Map<String, dynamic> result) => {
  'v': 1,
  'id': id,
  'ok': true,
  'result': result,
};

/// An error response object.
Map<String, dynamic> errorResponse(
  String id,
  String code, {
  String message = 'failed',
  Map<String, dynamic> detail = const {},
}) => {
  'v': 1,
  'id': id,
  'ok': false,
  'error': {'code': code, 'message': message, 'detail': detail},
};

/// The minimal `hello` result the client accepts.
Map<String, dynamic> helloResult({int protocol = 1}) => {
  'protocol': protocol,
  'core_version': '0.1.0',
  'root': r'C:\SealTools',
  'rev': 7,
  'config': {
    'window': {'title': 'TW_LIVE'},
  },
};

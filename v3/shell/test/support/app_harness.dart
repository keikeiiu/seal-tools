/// Test support: a [ShellModel] wired to a fake transport, plus JSON event/response builders that
/// go through the REAL protocol decoder (the client), so the model tests exercise the seam, not a
/// mocked client.
library;

import 'package:seal_shell/app/app.dart';
import 'package:seal_shell/protocol/protocol.dart';

import 'fake_transport.dart';

/// The raw response/hello builders the harness and its tests share, so a test that hand-writes a
/// `hello` answer (e.g. the `bad_version` case) does not need a second support import.
export 'fake_transport.dart' show errorResponse, helloResult, okResponse;

class AppHarness {
  AppHarness({
    this.shellVersion = '0.1.0',
    Duration requestTimeout = const Duration(seconds: 10),
  }) {
    model = ShellModel(
      transportFactory: _spawn,
      shellVersion: shellVersion,
      requestTimeout: requestTimeout,
    );
  }

  final String shellVersion;
  late final ShellModel model;

  /// Every transport the model created, oldest first (restart appends a new one).
  final List<FakeTransport> transports = [];

  /// Set before [connect]; receives every non-`hello` line the model writes. If it throws at spawn
  /// time (before creating a transport), the harness simulates a missing core.
  void Function(String line, FakeTransport transport)? onWrite;

  /// When set, the transport factory throws it — the spawn-failure path.
  Object? spawnError;

  int spawnAttempts = 0;

  FakeTransport get transport => transports.last;

  Map<String, dynamic> get lastRequest =>
      transport.sentJson(transport.sent.length - 1);

  String get lastCommandId => lastRequest['id'] as String;

  Future<Transport> _spawn() async {
    spawnAttempts++;
    final error = spawnError;
    if (error != null) {
      throw error;
    }
    final fake = FakeTransport();
    fake.onWrite = (line) => onWrite?.call(line, fake);
    transports.add(fake);
    return fake;
  }

  /// Connects, answering `hello` with [hello] (or the standard test payload). Any other command is
  /// routed to [onWrite] as it was when this was called.
  Future<void> connect({Map<String, dynamic>? hello}) async {
    final extra = onWrite;
    onWrite = (line, fake) {
      final message = fake.sentJson(fake.sent.length - 1);
      if (message['cmd'] == 'hello') {
        fake.emitJson(
          okResponse(message['id'] as String, hello ?? helloResult()),
        );
        return;
      }
      extra?.call(line, fake);
    };
    await model.connect();
  }

  /// Emits a protocol event through the transport.
  void emitEvent(String name, Map<String, dynamic> data) {
    transport.emitJson({'v': 1, 'event': name, 'data': data});
  }

  void emitState({
    required ToolId tool,
    bool running = true,
    ToolPhase phase = ToolPhase.running,
    String message = 'running',
    double? countdown,
    List<Map<String, dynamic>> rows = const [],
    String? error,
  }) {
    emitEvent('state', {
      'tool': tool.wire,
      'running': running,
      'phase': phase.wire,
      'message': message,
      'countdown_s': countdown,
      'rows': rows,
      'error': error,
    });
  }

  void emitToolFinished({
    required ToolId tool,
    ToolStopKind reason = ToolStopKind.done,
    String message = 'run finished',
  }) {
    emitEvent('tool.finished', {
      'tool': tool.wire,
      'reason': reason.wire,
      'message': message,
    });
  }

  void emitLog(String level, String line) {
    emitEvent('log', {'level': level, 'line': line});
  }

  void emitCoreStopping([CoreStoppingReason reason = CoreStoppingReason.eof]) {
    emitEvent('core.stopping', {'reason': reason.wire});
  }

  void emitConfigChanged({
    required int rev,
    required String block,
    Map<String, dynamic> values = const {},
  }) {
    emitEvent('config.changed', {'rev': rev, 'block': block, 'values': values});
  }

  /// Answers the request the model last wrote.
  void respondOk([Map<String, dynamic> result = const {}]) =>
      transport.emitJson(okResponse(lastCommandId, result));

  void respondError(
    String code, {
    String message = 'failed',
    Map<String, dynamic> detail = const {},
  }) => transport.emitJson(
    errorResponse(lastCommandId, code, message: message, detail: detail),
  );

  /// The core's output ends (crash / boot refusal / kill).
  void crash() => transport.closeIncoming();

  void emitStderr(String line) => transport.emitDiagnostic(line);

  /// A row object for a `state` event, matching the pinned shape ([PROTOCOL.md] §5).
  static Map<String, dynamic> row({
    required String name,
    required String status,
    String? detail,
    double? nextActionInSeconds,
  }) => {
    'name': name,
    'status': status,
    'detail': detail,
    'next_action_in_seconds': nextActionInSeconds,
  };

  Future<void> dispose() async {
    model.dispose();
    for (final fake in transports) {
      await fake.dispose();
    }
  }
}

/// Waits until the model's async stream plumbing has run (event delivery, command completion).
Future<void> settle() async {
  for (var i = 0; i < 4; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

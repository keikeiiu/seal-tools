/// The protocol client: spawn-side transport seam, request/response correlation, the event stream
/// and the `hello` handshake.
///
/// It is deliberately transport-agnostic. A [Transport] is "lines in, lines out, closed" — the real
/// one wraps a spawned `seal-core.exe` ([process_transport.dart]); the tests use a fake. The client
/// owns:
///
/// - one id per request, opaque and unique on the connection ([PROTOCOL.md] §2),
/// - a pending map so out-of-order responses still resolve the right future,
/// - per-request timeouts and a typed error for each failure mode,
/// - the handshake, including `bad_version`/`not_ready` surfaced as typed data ([PROTOCOL.md] §4),
/// - a broadcast event stream ([PROTOCOL.md] §5) and a log/stderr channel for the status surface.
///
/// It never parses config and never models a tool: the caller decodes the payloads it asked for.
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'dart:async';

import 'args.dart';
import 'envelope.dart';
import 'events.dart';
import 'framing.dart';
import 'json.dart';
import 'types.dart';

/// The seam over the core: one line out, a stream of lines in, and a way to close or kill.
///
/// `lines` completing (onDone) means the transport is CLOSED; the client turns that into a typed
/// [ProtocolClosedException] for every pending request. `diagnostics` carries stderr only — the core's
/// logs — and is never protocol data. `writeFailures` carries the async write side: a `stdin.done`
/// error (a dead core) must fail pending requests at once, not after the request timeout (SHELL-F1).
abstract interface class Transport {
  /// Complete lines from the core, terminator stripped.
  Stream<String> get lines;

  /// stderr lines, for the status surface and the crash tail. Not protocol messages.
  Stream<String> get diagnostics;

  /// Errors from the write side (the core's stdin): the request path is broken, not the output path.
  /// Completes the client's pending requests immediately on the first error.
  Stream<Object> get writeFailures;

  /// Writes one line; the transport adds the newline.
  void writeLine(String line);

  /// Closes the core's stdin. EOF makes the core stop its tools safely and exit ([PROTOCOL.md] §1).
  Future<void> closeInput();

  /// Kills the process, for a restart. Only a fallback for a wedged core.
  Future<void> terminate();
}

/// A request did not get its response before the deadline.
class ProtocolTimeoutException implements Exception {
  const ProtocolTimeoutException({
    required this.cmd,
    required this.id,
    required this.timeout,
  });

  final String cmd;
  final String id;
  final Duration timeout;

  @override
  String toString() =>
      'ProtocolTimeoutException: "$cmd" ($id) timed out after '
      '${timeout.inMilliseconds} ms';
}

/// The connection is closed (the core exited, the transport failed, or the client was closed).
class ProtocolClosedException implements Exception {
  const ProtocolClosedException(this.message);

  final String message;

  @override
  String toString() => 'ProtocolClosedException: $message';
}

/// A call was made in the wrong state — a request before [ProtocolClient.handshake], a second
/// handshake, a request after close.
class ProtocolStateException implements Exception {
  const ProtocolStateException(this.message);

  final String message;

  @override
  String toString() => 'ProtocolStateException: $message';
}

/// Why a handshake failed ([PROTOCOL.md] §1/§4).
enum HandshakeFailure {
  /// The core speaks a different protocol version; the shell shows both and does not retry.
  badVersion,

  /// A request arrived before `hello` — the client's own guard should have prevented this.
  notReady,

  /// The `hello` result's `protocol` is not the version this client speaks.
  protocolMismatch,
}

/// A `hello` exchange that did not produce a usable connection.
class HandshakeException implements Exception {
  const HandshakeException({
    required this.failure,
    required this.shellProtocol,
    this.coreExpected,
    this.coreReported,
    this.coreMessage,
  });

  final HandshakeFailure failure;

  /// The protocol version the shell spoke in `hello`.
  final int shellProtocol;

  /// On [HandshakeFailure.badVersion], the version the core said it expected (`detail.expected`).
  final int? coreExpected;

  /// On [HandshakeFailure.protocolMismatch], the version the core's result reported.
  final int? coreReported;

  /// The core's own message, for the crash view.
  final String? coreMessage;

  @override
  String toString() =>
      'HandshakeException: ${failure.name} (shell speaks $shellProtocol'
      '${coreExpected == null ? '' : ', core expected $coreExpected'}'
      '${coreReported == null ? '' : ', core reported $coreReported'})'
      '${coreMessage == null ? '' : ' — $coreMessage'}';
}

/// A command answered with a protocol error ([PROTOCOL.md] §6).
class CoreCommandException implements Exception {
  const CoreCommandException(this.error);

  final ErrorObject error;

  @override
  String toString() =>
      'CoreCommandException: ${error.rawCode} — ${error.message}';
}

class _Pending {
  _Pending(this.completer, this.timer, this.cmd);

  final Completer<Response> completer;
  final Timer timer;
  final String cmd;
}

/// The hand-written client ([PLAN-V3-SHELL.md] §4). One per core process.
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
class ProtocolClient {
  ProtocolClient(
    this._transport, {
    this.clientVersion = '0.0.0',
    this.requestTimeout = const Duration(seconds: 10),
  }) {
    _linesSubscription = decodeLines(_transport.lines)
        .listen(_onMessage, onError: _onStreamError, onDone: _onStreamDone);
    _diagnosticsSubscription = _transport.diagnostics.listen(
      _onDiagnostic,
      onError: (Object _) {},
    );
    _writeFailuresSubscription = _transport.writeFailures.listen(
      _onWriteFailure,
      onError: (Object _) {},
    );
  }

  final Transport _transport;

  /// The shell's own version, sent in `hello` ([PROTOCOL.md] §4).
  final String clientVersion;

  /// Default per-request deadline. Overridable per call where a command is known to be slower.
  final Duration requestTimeout;

  final _events = StreamController<ProtocolEvent>.broadcast();
  final _logs = StreamController<String>.broadcast();
  final _pending = <String, _Pending>{};
  final _stderrTail = <String>[];
  late final StreamSubscription<IncomingMessage> _linesSubscription;
  late final StreamSubscription<String> _diagnosticsSubscription;
  late final StreamSubscription<Object> _writeFailuresSubscription;

  int _nextId = 0;
  bool _closed = false;
  bool _protocolFailure = false;
  String? _closeReason;

  /// The single termination of the transport; every close path shares it (SHELL-F1).
  Future<void>? _termination;
  bool _handshaken = false;
  HelloResult? _hello;

  /// Events from the core, as a broadcast stream ([PROTOCOL.md] §5). A late subscriber misses
  /// earlier events; the caller that needs the whole history keeps its own model.
  Stream<ProtocolEvent> get events => _events.stream;

  /// Core logs and stderr: the status surface's source, and the crash view's tail.
  Stream<String> get logs => _logs.stream;

  /// The last stderr lines, for the crash view ([PLAN-V3-SHELL.md] §2).
  List<String> get stderrTail => List.unmodifiable(_stderrTail);

  /// The handshake payload, or null before it succeeds.
  HelloResult? get hello => _hello;

  /// Whether [handshake] has succeeded.
  bool get isHandshaken => _handshaken;

  /// Whether the connection is closed (by the core, the transport, or [close]).
  bool get isClosed => _closed;

  /// Whether the connection ended because the core's output could not be understood (an undecodable
  /// RESPONSE, a broken write path) rather than because the core exited. The shell reports this as
  /// `ConnectionFailure.protocol` instead of a fake "the core exited" (SHELL-F1).
  bool get protocolFailure => _protocolFailure;

  /// Why the connection closed, or null while it is open.
  String? get closeReason => _closeReason;

  /// Performs `hello` and validates the result.
  ///
  /// Sends `{"protocol":1,"client":<clientVersion>}`; a `bad_version` error or a result whose
  /// `protocol` differs becomes a typed [HandshakeException] — the caller shows the versions and does
  /// not retry ([PROTOCOL.md] §1). Must be called exactly once, before any command.
  Future<HelloResult> handshake({Duration? timeout}) async {
    if (_closed) {
      throw const ProtocolClosedException(
        'cannot handshake: the client is closed',
      );
    }
    if (_handshaken) {
      throw const ProtocolStateException('handshake has already completed');
    }

    final response = await _send(
      Request(
        id: _nextRequestId(),
        cmd: 'hello',
        args: HelloArgs(
          protocol: protocolVersion,
          client: clientVersion,
        ).toJson(),
      ),
      timeout: timeout,
      bypassHandshakeGuard: true,
    );

    switch (response) {
      case ErrorResponse(:final error):
        if (error.code == ErrorCode.badVersion) {
          throw HandshakeException(
            failure: HandshakeFailure.badVersion,
            shellProtocol: protocolVersion,
            coreExpected: error.expectedProtocol,
            coreMessage: error.message,
          );
        }
        if (error.code == ErrorCode.notReady) {
          throw HandshakeException(
            failure: HandshakeFailure.notReady,
            shellProtocol: protocolVersion,
            coreMessage: error.message,
          );
        }
        throw CoreCommandException(error);
      case OkResponse(:final result):
        final hello = HelloResult.fromJson(result);
        if (hello.protocol != protocolVersion) {
          throw HandshakeException(
            failure: HandshakeFailure.protocolMismatch,
            shellProtocol: protocolVersion,
            coreReported: hello.protocol,
          );
        }
        _hello = hello;
        _handshaken = true;
        return hello;
    }
  }

  /// Sends one command and returns its raw response ([PROTOCOL.md] §4).
  ///
  /// Throws [ProtocolStateException] before the handshake, [ProtocolTimeoutException] on the deadline,
  /// and [ProtocolClosedException] once the connection is gone. An [ErrorResponse] is returned, not
  /// thrown: the typed helpers below decide which codes are their concern.
  Future<Response> request(String cmd, [JsonMap args = const {}]) {
    if (!_handshaken) {
      return Future<Response>.error(
        const ProtocolStateException(
          'handshake() must complete before sending commands',
        ),
      );
    }
    return _send(Request(id: _nextRequestId(), cmd: cmd, args: args));
  }

  /// `config.validate`: the authoring rules on the blocks a Save would send
  /// ([PROTOCOL.md] §4/§7).
  Future<ConfigValidateResult> configValidate(ConfigSections sections) =>
      _requestTyped(
        'config.validate',
        ConfigValidateArgs(sections: sections).toJson(),
        ConfigValidateResult.fromJson,
      );

  /// `config.apply`: merge the shell's blocks under the session revision ([PROTOCOL.md] §4/§7).
  Future<ConfigApplyResult> configApply({
    required int rev,
    required ConfigSections sections,
  }) => _requestTyped(
    'config.apply',
    ConfigApplyArgs(rev: rev, sections: sections).toJson(),
    ConfigApplyResult.fromJson,
  );

  /// `config.reload`: re-read the files ([PROTOCOL.md] §4).
  Future<ConfigReloadResult> configReload() => _requestTyped(
    'config.reload',
    const ConfigReloadArgs().toJson(),
    ConfigReloadResult.fromJson,
  );

  /// `tool.start` ([PROTOCOL.md] §4). A refusal is a [CoreCommandException] carrying the core's line.
  Future<void> toolStart(ToolId tool) =>
      _requestVoid('tool.start', ToolStartArgs(tool: tool).toJson());

  /// `tool.stop`: a tool, or none to stop whatever runs ([PROTOCOL.md] §4).
  Future<void> toolStop([ToolId? tool]) =>
      _requestVoid('tool.stop', ToolStopArgs(tool: tool).toJson());

  /// `ui.window`: the shell's own window geometry into `local.yaml` ([PROTOCOL.md] §4).
  Future<void> uiWindow(UiWindowArgs args) =>
      _requestVoid('ui.window', args.toJson());

  /// Sends `shutdown` and returns once the core acknowledges it. The process exits after that; use
  /// [close] to also close the transport.
  Future<void> shutdown({Duration? timeout}) async {
    final response = await _send(
      Request(id: _nextRequestId(), cmd: 'shutdown'),
      timeout: timeout,
      bypassHandshakeGuard: true,
    );
    if (response is ErrorResponse) {
      throw CoreCommandException(response.error);
    }
  }

  /// Closes the connection: a best-effort `shutdown`, then EOF on the core's stdin
  /// ([PROTOCOL.md] §1/§2).
  ///
  /// Every pending request fails with [ProtocolClosedException], the event/log streams close, and a
  /// second call still TERMINATES the transport — a close after a crash is the caller saying "make
  /// sure it is gone", and a live process must never be left unreferenced (SHELL-F1). A core that
  /// does not acknowledge `shutdown` inside the timeout is still closed by EOF — the safe stop is the
  /// EOF, not the acknowledgement.
  Future<void> close({Duration timeout = const Duration(seconds: 2)}) async {
    if (_closed) {
      await _terminateTransport();
      return;
    }
    if (_handshaken) {
      try {
        await shutdown(timeout: timeout);
      } catch (_) {
        // The EOF below is the load-bearing part; a missed acknowledgement is not a failure to close.
      }
    }
    await _transport.closeInput();
    _markClosed('the shell closed the connection', clean: true);
  }

  /// Kills the transport — the restart path after a wedged core ([PLAN-V3-SHELL.md] §2).
  Future<void> terminate({
    String reason = 'the shell terminated the core',
  }) async {
    if (!_closed) {
      _markClosed(reason);
    }
    // Already closed (a crash/refusal path): still make sure the process is gone.
    await _terminateTransport();
  }

  Future<T> _requestTyped<T>(
    String cmd,
    JsonMap args,
    T Function(JsonMap result) decode,
  ) async {
    final response = await request(cmd, args);
    return switch (response) {
      ErrorResponse(:final error) => throw CoreCommandException(error),
      OkResponse(:final result) => decode(result),
    };
  }

  Future<void> _requestVoid(String cmd, JsonMap args) async {
    final response = await request(cmd, args);
    if (response is ErrorResponse) {
      throw CoreCommandException(response.error);
    }
  }

  Future<Response> _send(
    Request request, {
    Duration? timeout,
    bool bypassHandshakeGuard = false,
  }) {
    if (_closed) {
      return Future<Response>.error(
        const ProtocolClosedException('the connection is closed'),
      );
    }
    if (!bypassHandshakeGuard && !_handshaken) {
      return Future<Response>.error(
        const ProtocolStateException(
          'handshake() must complete before sending commands',
        ),
      );
    }

    final deadline = timeout ?? requestTimeout;
    final completer = Completer<Response>();
    final timer = Timer(deadline, () {
      final pending = _pending.remove(request.id);
      if (pending == null) {
        return;
      }
      _log(
        'request ${request.id} ("${pending.cmd}") timed out after '
        '${deadline.inMilliseconds} ms',
      );
      pending.completer.completeError(
        ProtocolTimeoutException(
          cmd: pending.cmd,
          id: request.id,
          timeout: deadline,
        ),
      );
    });
    _pending[request.id] = _Pending(completer, timer, request.cmd);

    try {
      _transport.writeLine(encodeRequest(request));
    } catch (error) {
      timer.cancel();
      _pending.remove(request.id);
      _markClosed('could not write to the core: $error', protocol: true);
      return Future<Response>.error(
        ProtocolClosedException('could not write to the core: $error'),
      );
    }

    return completer.future;
  }

  String _nextRequestId() => '${++_nextId}';

  void _onMessage(IncomingMessage message) {
    switch (message) {
      case IncomingResponse(:final response):
        final pending = _pending.remove(response.id);
        if (pending == null) {
          // A late reply to a timed-out request, or an id this client never sent. Both are worth a
          // line in the status surface and neither is a reason to kill the connection.
          _log('response for unknown request id "${response.id}"');
          return;
        }
        pending.timer.cancel();
        pending.completer.complete(response);
      case IncomingEvent(:final event):
        if (!_events.isClosed) {
          _events.add(event);
        }
      case IncomingMalformedEvent(
        :final error,
        :final eventName,
        :final eventData,
      ):
        // One undecodable EVENT is informational: log it, forward it as an unknown event and keep
        // reading — a newer core's addition must not end the connection and leave the core running
        // unreferenced (SHELL-F1). A response that cannot be correlated stays fatal: decodeLines
        // rethrows it into [_onStreamError].
        _log(
          'ignoring undecodable event'
          '${eventName == null ? '' : ' "$eventName"'}: ${error.message}',
        );
        if (!_events.isClosed) {
          _events.add(
            UnknownEvent(
              eventName ?? 'malformed',
              eventData ?? <String, dynamic>{'error': error.message},
            ),
          );
        }
    }
  }

  void _onStreamError(Object error, StackTrace _) {
    _markClosed('the core output could not be decoded: $error', protocol: true);
  }

  /// The write side failed (`stdin.done`): the connection is dead, so fail pending requests NOW rather
  /// than letting them wait out the request timeout (SHELL-F1).
  void _onWriteFailure(Object error) {
    _markClosed('could not write to the core: $error', protocol: true);
  }

  void _onStreamDone() {
    _markClosed('the core closed its output');
  }

  void _onDiagnostic(String line) {
    _stderrTail.add(line);
    if (_stderrTail.length > 100) {
      _stderrTail.removeAt(0);
    }
    _log(line);
  }

  void _log(String line) {
    if (!_logs.isClosed) {
      _logs.add(line);
    }
  }

  void _markClosed(String reason, {bool clean = false, bool protocol = false}) {
    if (_closed) {
      return;
    }
    _closed = true;
    _closeReason = reason;
    _protocolFailure = protocol;
    _failPending(reason);
    unawaited(_linesSubscription.cancel());
    unawaited(_diagnosticsSubscription.cancel());
    unawaited(_writeFailuresSubscription.cancel());
    if (!clean) {
      // A non-clean close means nobody else will reap the process: terminate it, so a malformed line
      // or a broken pipe can never leave a live core running with an open stdin (SHELL-F1).
      unawaited(_terminateTransport());
    }
    if (!_events.isClosed) {
      unawaited(_events.close());
    }
    if (!_logs.isClosed) {
      unawaited(_logs.close());
    }
  }

  /// Terminates the transport exactly once; later callers join the same future. Failures are
  /// swallowed — the connection is already gone either way.
  Future<void> _terminateTransport() {
    return _termination ??= _transport.terminate().catchError((Object _) {});
  }

  void _failPending(String reason) {
    final pending = _pending.values.toList();
    _pending.clear();
    for (final entry in pending) {
      entry.timer.cancel();
      if (!entry.completer.isCompleted) {
        entry.completer.completeError(ProtocolClosedException(reason));
      }
    }
  }
}

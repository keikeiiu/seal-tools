/// The shell's ONE `ChangeNotifier` ([PLAN-V3-SHELL.md] §5): it owns the [ProtocolClient], the four
/// sub-models the UI renders, and the commands the cards call.
///
/// What lives where, and why:
///
/// - **Protocol bookkeeping** (the model's decisions): the revision echoed on `config.apply`, the
///   `validate`-then-`apply` order a save is ([PROTOCOL.md] §7), clearing cards on a new process,
///   the connection lifecycle, and capturing the stderr tail at a crash. These are rules, and no
///   widget re-implements them.
/// - **UI convenience** (marked where it happens): the "command in flight" flag that disables a
///   card's buttons, and the refusal line stored verbatim per card. Neither is state the core owns.
///
/// It never decides game policy: a Start sends `tool.start`; the card changes only when the core
/// publishes `state` ([PLAN-V3-SHELL.md] §5).
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'dart:async';

import 'package:flutter/foundation.dart';

import '../protocol/protocol.dart';
import 'attributes_result.dart';
import 'calibration_result.dart';
import 'config.dart';
import 'connection.dart';
import 'core_actions.dart';
import 'key_names_result.dart';
import 'logs.dart';
import 'tools.dart';

/// Creates the transport for a connection attempt. The real one spawns a core process; tests return
/// a fake. One call per spawn, so a restart gets a fresh transport from the same factory.
typedef CoreTransportFactory = Future<Transport> Function();

class ShellModel extends ChangeNotifier {
  ShellModel({
    required this.transportFactory,
    this.shellVersion = '0.1.0',
    this.requestTimeout = const Duration(seconds: 10),
    this.handshakeTimeout = const Duration(seconds: 5),
  }) {
    connection.addListener(_onSubmodelChanged);
    tools.addListener(_onSubmodelChanged);
    config.addListener(_onSubmodelChanged);
    logs.addListener(_onSubmodelChanged);
  }

  /// How a connection is created: the shell's `main.dart` supplies the process spawner, tests a
  /// fake. One call per spawn, so a restart gets a fresh transport from the same factory.
  final CoreTransportFactory transportFactory;

  /// The shell's own version, sent in `hello`; supplied by `main.dart` (the protocol library's
  /// default deliberately stays `0.0.0` — the version choice belongs to the shell).
  final String shellVersion;

  /// Per-request deadline for commands ([PLAN-V3-SHELL.md] §4: a shell constant, not protocol).
  final Duration requestTimeout;

  /// `hello` gets its own shorter deadline so a wedged core shows the crash view quickly.
  final Duration handshakeTimeout;

  /// Connection lifecycle and the crash facts.
  final ConnectionModel connection = ConnectionModel();

  /// The eight cards' core-published facts.
  final ToolsModel tools = ToolsModel();

  /// The mirrored config + revision and command outcomes.
  final ConfigModel config = ConfigModel();

  /// The status surface's ring buffer.
  final LogBuffer logs = LogBuffer();

  ProtocolClient? _client;
  StreamSubscription<ProtocolEvent>? _eventSubscription;
  StreamSubscription<String>? _logSubscription;
  Future<void>? _connectInFlight;
  bool _closing = false;
  bool _disposed = false;

  /// The live client, for the wiring and for tests; widgets use the commands below.
  ProtocolClient? get client => _client;

  bool get isReady => connection.isReady;

  /// True once [shutdown] started; the app is going away and no reconnect is attempted.
  bool get isClosing => _closing;

  /// Spawns (or reuses) the core, performs `hello`, and fills the config mirror.
  ///
  /// Safe to call more than once: a call while a connection attempt is in flight joins it. A failure
  /// lands in [connection] as a failed state — this future completes normally either way, because a
  /// failed connection is a RENDERED state, not an exception for `main` to swallow.
  Future<void> connect() {
    if (_closing || _disposed) {
      return Future<void>.value();
    }
    final inFlight = _connectInFlight;
    if (inFlight != null) {
      return inFlight;
    }
    final future = _connect().whenComplete(() => _connectInFlight = null);
    _connectInFlight = future;
    return future;
  }

  Future<void> _connect() async {
    if (_closing || _disposed) {
      return;
    }
    connection.markStarting();
    final Transport transport;
    try {
      transport = await transportFactory();
    } catch (error) {
      _fail(ConnectionFailure.spawn, 'the core did not start: $error');
      return;
    }

    final client = ProtocolClient(
      transport,
      clientVersion: shellVersion,
      requestTimeout: requestTimeout,
    );
    _client = client;
    _eventSubscription = client.events.listen(
      _onEvent,
      onDone: () => _onCoreOutputClosed(client),
    );
    _logSubscription = client.logs.listen(logs.addStderr);
    connection.markHandshaking();

    final HelloResult hello;
    try {
      hello = await client.handshake(timeout: handshakeTimeout);
    } on HandshakeException catch (error) {
      final failure = switch (error.failure) {
        HandshakeFailure.badVersion => ConnectionFailure.badVersion,
        HandshakeFailure.protocolMismatch => ConnectionFailure.badVersion,
        HandshakeFailure.notReady => ConnectionFailure.handshake,
      };
      _fail(failure, _describeHandshake(error));
      await _detachClient(client);
      return;
    } catch (error) {
      _fail(ConnectionFailure.handshake, 'the handshake failed: $error');
      await _detachClient(client);
      return;
    }

    if (_closing || _disposed) {
      await _detachClient(client);
      return;
    }
    config.applyHello(hello);
    connection.markReady(coreVersion: hello.coreVersion, root: hello.root);
    logs.addShellNote(
      'core ${hello.coreVersion} connected '
      '(rev ${hello.rev}, root ${hello.root})',
    );
  }

  /// One click, one new process, one new handshake ([PLAN-V3-SHELL.md] §2) — never an auto loop.
  Future<void> restart() async {
    if (_closing || _disposed) {
      return;
    }
    final previous = _client;
    _client = null;
    await _cancelSubscriptions();
    if (previous != null) {
      try {
        await previous.close();
      } catch (_) {
        // The old connection is dead or dying either way; the new process is the point.
      }
    }
    // Cards keep the last rendered state (PLAN-V3-SHELL §2) — nothing is cleared here.
    await connect();
  }

  /// Sends `shutdown`, then EOF on the core's stdin — the safe stop ([PROTOCOL.md] §1/§2).
  Future<void> shutdown() async {
    if (_closing) {
      return;
    }
    _closing = true;
    final client = _client;
    _client = null;
    await _cancelSubscriptions();
    if (client != null) {
      try {
        await client.close();
      } catch (_) {
        // EOF is the load-bearing stop; a missed acknowledgement is not a failed shutdown.
      }
    }
    connection.markStopped();
  }

  // ── tool commands ─────────────────────────────────────────────────────────────────────────

  /// `tool.start {tool}` ([PROTOCOL.md] §4). Returns false when the command was refused; the core's
  /// own line is then on the card ([PROTOCOL.md] §6). The card's phase still changes only when the
  /// core publishes `state`.
  /// `tool.start {tool}`. [runArgs] are the optional per-run arguments PROTOCOL §12.6 defines; today
  /// only `buy` accepts them (`preset`, `count`). When omitted the typed client call is used unchanged.
  Future<bool> startTool(ToolId tool, {JsonMap? runArgs}) {
    if (runArgs == null) {
      return _toolCommand(tool, (client) => client.toolStart(tool));
    }
    return _toolCommand(tool, (client) async {
      final response = await client.request('tool.start', {
        'tool': tool.wire,
        ...runArgs,
      });
      if (response is ErrorResponse) {
        throw CoreCommandException(response.error);
      }
    });
  }

  /// `tool.stop {tool}`, or none to stop whatever runs ([PROTOCOL.md] §4).
  Future<bool> stopTool([ToolId? tool]) =>
      _toolCommand(tool, (client) => client.toolStop(tool));

  /// `ui.window` — the shell's own window geometry, written by the core ([PROTOCOL.md] §4).
  Future<bool> saveWindow(UiWindowArgs args) =>
      _voidCommand('ui.window', (client) => client.uiWindow(args));

  /// `window.find`: locate the configured game window. "Not found" is a RESULT, not an error; a
  /// config/Win32 refusal comes back typed and is rendered verbatim ([PROTOCOL.md] §4).
  Future<CoreActionResult<WindowFindResult>> findWindow() =>
      _coreAction('window.find', WindowFindResult.fromJson);

  /// `window.measure`: the `DisplayInfo` the shell maps frames with ([PROTOCOL.md] §3/§4).
  Future<CoreActionResult<DisplayInfo>> measureWindow() => _coreAction(
    'window.measure',
    (result) => DisplayInfo.fromJson(result['display'], 'display'),
  );

  /// `board.probe`: port and firmware facts. No board is `board_unavailable`, shown verbatim — the
  /// no-hardware path ([PROTOCOL.md] §4/§6).
  Future<CoreActionResult<BoardProbeResult>> probeBoard() =>
      _coreAction('board.probe', BoardProbeResult.fromJson);

  /// `attributes.get`: the dictionary the core already loaded for the active client. The payload is
  /// decoded app-locally (the Attributes surface consumes an untyped result; no protocol type exists).
  Future<CoreActionResult<AttributesResult>> attributesGet() =>
      _coreAction('attributes.get', AttributesResult.fromJson);

  /// `board.test` — the calibrator's input tests. A command reaches the board only after the port
  /// opens; every refusal line is the core's ([PROTOCOL.md] §4/§6).
  Future<CoreActionResult<JsonMap>> boardTestClick() => _coreAction(
    'board.test',
    (result) => result,
    args: const {'kind': 'click'},
  );

  Future<CoreActionResult<JsonMap>> boardTestMove({
    required int dx,
    required int dy,
  }) => _coreAction(
    'board.test',
    (result) => result,
    args: {'kind': 'move', 'dx': dx, 'dy': dy},
  );

  Future<CoreActionResult<JsonMap>> boardTestKey(String key) => _coreAction(
    'board.test',
    (result) => result,
    args: {'kind': 'key', 'key': key},
  );

  /// `capture`: the game client as PNG plus the `DisplayInfo` measurement. The full client is the
  /// calibration frame; a region is only for the capture command's general use.
  Future<CoreActionResult<CaptureResult>> captureFrame({Rect? region}) =>
      _coreAction(
        'capture',
        CaptureResult.fromJson,
        args: region == null ? const {} : {'region': region.toJson()},
      );

  /// `calibration.save`: the mark drawn on the captured frame, plus the capture's measured display so
  /// the core records the environment. Refusals (bag-slot pitch disagreement, unknown frame_id, …)
  /// come back as the core's typed error line.
  Future<CoreActionResult<CalibrationSaveResult>> calibrationSave({
    required String frameId,
    required String kind,
    Rect? region,
    Point? point,
    bool? empty,
    DisplayInfo? display,
  }) => _coreAction(
    'calibration.save',
    CalibrationSaveResult.fromJson,
    args: {
      'frame_id': frameId,
      'kind': kind,
      if (region != null) 'region': region.toJson(),
      if (point != null) 'point': point.toJson(),
      // v2's empty verdict for `result_gem_area` (PROTOCOL §12.7): true saves the crop and samples
      // the signature, false samples the signature only. Omitted for every other kind.
      'empty': ?empty,
      if (display != null) 'display': calibrationDisplayJson(display),
    },
  );

  /// `calibration.fetch`: the stored mark (null = not calibrated yet) and, for artifact kinds, the
  /// stored crop as base64 PNG.
  Future<CoreActionResult<CalibrationFetchResult>> calibrationFetch(
    String kind,
  ) => _coreAction(
    'calibration.fetch',
    CalibrationFetchResult.fromJson,
    args: {'kind': kind},
  );

  /// `keys.names`: display names for the stored VK codes (the domain's inverse rule, hex fallback
  /// included). The Hotkeys tab shows these; it invents no name of its own.
  Future<CoreActionResult<KeyNamesResult>> keyNames({
    required Map<String, int> codes,
  }) => _coreAction(
    'keys.names',
    KeyNamesResult.fromJson,
    args: {'codes': codes},
  );

  /// `keys.parse`: typed names → VK codes. The domain answers every parseable field and one line per
  /// bad box; the tab renders those lines verbatim and never parses a name itself.
  Future<CoreActionResult<KeyParseResult>> parseKeyNames({
    required Map<String, String> names,
  }) => _coreAction(
    'keys.parse',
    KeyParseResult.fromJson,
    args: {'names': names},
  );

  // ── config commands: validate, then apply ([PLAN-V3-SHELL.md] §5 / PROTOCOL §7) ───────────

  /// `config.validate`: the player-facing lines a save would refuse.
  Future<ConfigValidateResult?> validateConfig(ConfigSections sections) async {
    final client = _client;
    if (!isReady || client == null) {
      logs.addShellNote('cannot validate config: the core is not ready');
      return null;
    }
    try {
      final result = await client.configValidate(sections);
      config.recordValidation(result);
      return result;
    } on CoreCommandException catch (error) {
      config.recordError(error.error);
      logs.addShellNote(error.error.message);
      return null;
    } on ProtocolTimeoutException catch (error) {
      _noteTimeout(error);
      return null;
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('cannot validate config: ${error.message}');
      return null;
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return null;
    }
  }

  /// `config.apply {rev, sections}`. The revision echoed is the one the shell last saw
  /// ([PROTOCOL.md] §7); a `conflict` is recorded on [config] with the core's rev/sections and shown
  /// verbatim — the shell does not retry by itself.
  Future<ConfigApplyResult?> applyConfig(ConfigSections sections) async {
    final client = _client;
    if (!isReady || client == null) {
      logs.addShellNote('cannot save config: the core is not ready');
      return null;
    }
    final rev = config.rev;
    try {
      final result = await client.configApply(rev: rev, sections: sections);
      config.recordApply(result);
      return result;
    } on CoreCommandException catch (error) {
      config.recordError(error.error);
      logs.addShellNote(error.error.message);
      return null;
    } on ProtocolTimeoutException catch (error) {
      _noteTimeout(error);
      return null;
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('cannot save config: ${error.message}');
      return null;
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return null;
    }
  }

  /// `config.reload`: re-read the files; the whole merged config replaces the mirror.
  Future<ConfigReloadResult?> reloadConfig() async {
    final client = _client;
    if (!isReady || client == null) {
      logs.addShellNote('cannot reload config: the core is not ready');
      return null;
    }
    try {
      final result = await client.configReload();
      config.recordReload(result);
      return result;
    } on CoreCommandException catch (error) {
      config.recordError(error.error);
      logs.addShellNote(error.error.message);
      return null;
    } on ProtocolTimeoutException catch (error) {
      _noteTimeout(error);
      return null;
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('cannot reload config: ${error.message}');
      return null;
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return null;
    }
  }

  /// A tab's Save: the protocol order is `config.validate` FIRST, then `config.apply`
  /// ([PLAN-V3-SHELL.md] §5 / PROTOCOL §7). The tab never sends apply directly, and the core's
  /// validation lines are returned verbatim for the tab to render.
  ///
  /// The outcome is data, not an exception: a refused save is a rendered state on the tab.
  Future<ConfigSaveOutcome> saveConfig(ConfigSections sections) async {
    if (!isReady || _client == null) {
      const note = 'the core is not ready';
      logs.addShellNote('cannot save config: $note');
      return const ConfigSaveOutcome(note: note);
    }
    final validation = await validateConfig(sections);
    if (validation == null) {
      return ConfigSaveOutcome(
        error: config.lastError,
        note: config.lastError == null ? 'validation failed' : null,
      );
    }
    if (validation.errors.isNotEmpty) {
      return ConfigSaveOutcome(validation: validation);
    }
    final applied = await applyConfig(sections);
    if (applied == null) {
      return ConfigSaveOutcome(
        validation: validation,
        error: config.lastError,
        note: config.lastError == null ? 'the save failed' : null,
      );
    }
    return ConfigSaveOutcome(validation: validation, result: applied);
  }

  // ── plumbing ─────────────────────────────────────────────────────────────────────────────

  void _onEvent(ProtocolEvent event) {
    if (_disposed) {
      return;
    }
    switch (event) {
      case StateEvent():
      case ToolFinishedEvent():
        tools.applyEvent(event);
      case LogEvent(:final log):
        logs.addCoreLog(log);
      case ConfigChangedEvent(:final changed):
        config.applyChanged(changed);
      case CoreStoppingEvent(:final stopping):
        tools.applyEvent(event);
        logs.addShellNote('core stopping: ${stopping.reason.wire}');
      case UnknownEvent(:final name):
        logs.addShellNote('unrecognised event "$name"');
    }
  }

  void _onCoreOutputClosed(ProtocolClient client) {
    if (_disposed || !identical(_client, client)) {
      return;
    }
    final tail = List<String>.of(client.stderrTail);
    logs.captureStderrTail(tail);
    _fail(
      ConnectionFailure.coreExit,
      tail.isEmpty
          ? 'the core exited'
          : 'the core exited — last line on stderr: ${tail.last}',
      stderrTail: tail,
    );
    // The last rendered state is deliberately kept (PLAN §2): the crash view replaces the grid but a
    // restart starts from the same cards.
  }

  Future<bool> _toolCommand(
    ToolId? tool,
    Future<void> Function(ProtocolClient client) send,
  ) async {
    final client = _client;
    if (!isReady || client == null) {
      logs.addShellNote('the core is not ready');
      return false;
    }
    if (tool != null) {
      tools.beginCommand(tool);
      tools.clearRefusal(tool);
    }
    try {
      await send(client);
      return true;
    } on CoreCommandException catch (error) {
      if (tool != null) {
        tools.refuse(tool, error.error.message);
      }
      logs.addShellNote(error.error.message);
      return false;
    } on ProtocolTimeoutException catch (error) {
      final line =
          'the core did not answer ${error.cmd} within '
          '${error.timeout.inSeconds}s';
      if (tool != null) {
        tools.refuse(tool, line);
      }
      logs.addShellNote(line);
      return false;
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('the core is gone: ${error.message}');
      return false;
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return false;
    } finally {
      if (tool != null) {
        tools.endCommand(tool);
      }
    }
  }

  /// One read-only command with a typed result: the core's answer (or its refusal) is data for the
  /// Setup surface; nothing is thrown at the widget.
  Future<CoreActionResult<T>> _coreAction<T>(
    String cmd,
    T Function(JsonMap result) decode, {
    JsonMap args = const {},
  }) async {
    final client = _client;
    if (!isReady || client == null) {
      const note = 'the core is not ready';
      logs.addShellNote('cannot send $cmd: $note');
      return CoreActionResult<T>.failure(shellNote: note);
    }
    try {
      final response = await client.request(cmd, args);
      return switch (response) {
        OkResponse(:final result) => CoreActionResult<T>.success(
          decode(result),
        ),
        ErrorResponse(:final error) => CoreActionResult<T>.failure(
          coreError: error,
        ),
      };
    } on ProtocolTimeoutException catch (error) {
      _noteTimeout(error);
      return CoreActionResult<T>.failure(
        shellNote:
            'the core did not answer $cmd within ${error.timeout.inSeconds}s',
      );
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('cannot send $cmd: ${error.message}');
      return CoreActionResult<T>.failure(
        shellNote: 'the core is gone: ${error.message}',
      );
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return CoreActionResult<T>.failure(shellNote: error.message);
    } on ProtocolDecodeException catch (error) {
      logs.addShellNote('cannot read the $cmd result: ${error.message}');
      return CoreActionResult<T>.failure(
        shellNote: 'unexpected $cmd result: ${error.message}',
      );
    }
  }

  Future<bool> _voidCommand(
    String what,
    Future<void> Function(ProtocolClient client) send,
  ) async {
    final client = _client;
    if (!isReady || client == null) {
      logs.addShellNote('cannot send $what: the core is not ready');
      return false;
    }
    try {
      await send(client);
      return true;
    } on CoreCommandException catch (error) {
      logs.addShellNote(error.error.message);
      return false;
    } on ProtocolTimeoutException catch (error) {
      _noteTimeout(error);
      return false;
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('cannot send $what: ${error.message}');
      return false;
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return false;
    }
  }

  void _fail(
    ConnectionFailure failure,
    String message, {
    List<String> stderrTail = const [],
  }) {
    final tail = stderrTail.isEmpty
        ? List<String>.of(_client?.stderrTail ?? const [])
        : stderrTail;
    logs.captureStderrTail(tail);
    connection.markFailed(failure: failure, message: message, stderrTail: tail);
    logs.addShellNote(message);
  }

  String _describeHandshake(HandshakeException error) {
    final message = switch (error.failure) {
      HandshakeFailure.badVersion =>
        'protocol mismatch: the shell speaks ${error.shellProtocol}, '
            'the core expects ${error.coreExpected ?? '?'}',
      HandshakeFailure.protocolMismatch =>
        'protocol mismatch: the shell speaks ${error.shellProtocol}, '
            'the core reports ${error.coreReported ?? '?'}',
      HandshakeFailure.notReady =>
        'the core refused the handshake: '
            '${error.coreMessage ?? 'not ready'}',
    };
    final coreMessage = error.coreMessage;
    if (coreMessage == null || message.contains(coreMessage)) {
      return message;
    }
    return '$message — $coreMessage';
  }

  void _noteTimeout(ProtocolTimeoutException error) {
    logs.addShellNote(
      'the core did not answer ${error.cmd} within '
      '${error.timeout.inSeconds}s',
    );
  }

  Future<void> _detachClient(ProtocolClient client) async {
    if (identical(_client, client)) {
      _client = null;
    }
    await _cancelSubscriptions();
    try {
      await client.close();
    } catch (_) {
      // Already closed or the process is gone; nothing to recover.
    }
  }

  Future<void> _cancelSubscriptions() async {
    final events = _eventSubscription;
    final logLines = _logSubscription;
    _eventSubscription = null;
    _logSubscription = null;
    await events?.cancel();
    await logLines?.cancel();
  }

  void _onSubmodelChanged() {
    if (_disposed) {
      return;
    }
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    connection.removeListener(_onSubmodelChanged);
    tools.removeListener(_onSubmodelChanged);
    config.removeListener(_onSubmodelChanged);
    logs.removeListener(_onSubmodelChanged);
    _eventSubscription?.cancel();
    _logSubscription?.cancel();
    super.dispose();
  }
}

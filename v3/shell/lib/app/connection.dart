/// The connection sub-model: the core's lifecycle, its identity after `hello`, and the crash facts.
///
/// Lifecycle follows PLAN-V3-SHELL §2/§5: `starting → handshaking → ready`, or `failed(reason)`. The
/// failure kind is data, not a string to parse, because the crash view behaves differently per kind
/// (a `bad_version` is a packaging bug and is NOT offered a restart; a core exit is).
library;

import 'package:flutter/foundation.dart';

/// Where the connection is in its lifecycle.
enum ConnectionStatus { idle, starting, handshaking, ready, failed }

/// What ended the connection, so the crash view can say the right thing.
enum ConnectionFailure {
  /// Spawning the process failed (missing executable, permission, …).
  spawn,

  /// `hello` was answered with an error other than `bad_version`.
  handshake,

  /// The core speaks another protocol version; a packaging bug, not a transient.
  badVersion,

  /// The process's output ended (crash, boot refusal, or kill).
  coreExit,

  /// A protocol-level failure in the shell (decode error, closed transport mid-handshake, …).
  protocol,
}

class ConnectionModel extends ChangeNotifier {
  ConnectionStatus _status = ConnectionStatus.idle;
  ConnectionFailure? _failure;
  String? _failureMessage;
  String? _coreVersion;
  String? _root;
  List<String> _stderrTail = const [];
  int _crashCount = 0;
  int? _exitCode;

  /// Where the connection is.
  ConnectionStatus get status => _status;

  /// Why it is [ConnectionStatus.failed]; null otherwise.
  ConnectionFailure? get failure => _failure;

  /// The crash view's headline — the spawn error, the handshake's own words, or "the core exited".
  String? get failureMessage => _failureMessage;

  /// The core's version from `hello`, once known. Kept after a crash so the view can still say what
  /// was connected.
  String? get coreVersion => _coreVersion;

  /// The application root the core bound to (`hello.root`). Kept across a crash for the same reason.
  String? get root => _root;

  /// The stderr tail captured at the moment of the crash ([PROTOCOL.md] §1 — boot refusals are stderr,
  /// never protocol). Empty while healthy.
  List<String> get stderrTail => _stderrTail;

  /// The core process's exit code at the crash, when the transport reported one (SHELL-F4).
  int? get exitCode => _exitCode;

  /// How many failed connections this app has seen. A count is a UI convenience: it lets the crash
  /// view say "the core failed twice" without the shell inventing a reason.
  int get crashCount => _crashCount;

  bool get isReady => _status == ConnectionStatus.ready;

  /// Whether the failure is one a restart can plausibly fix. A `bad_version` is not: the binary must
  /// be replaced, so the crash view hides the restart button for it.
  bool get canRestart =>
      _status == ConnectionStatus.failed &&
      _failure != ConnectionFailure.badVersion;

  // ── mutations: ShellModel only ────────────────────────────────────────────────────────────

  void markStarting() {
    _status = ConnectionStatus.starting;
    _failure = null;
    _failureMessage = null;
    _stderrTail = const [];
    _exitCode = null;
    notifyListeners();
  }

  void markHandshaking() {
    _status = ConnectionStatus.handshaking;
    notifyListeners();
  }

  void markReady({required String coreVersion, required String root}) {
    _status = ConnectionStatus.ready;
    _failure = null;
    _failureMessage = null;
    _coreVersion = coreVersion;
    _root = root;
    notifyListeners();
  }

  void markFailed({
    required ConnectionFailure failure,
    required String message,
    List<String> stderrTail = const [],
    int? exitCode,
  }) {
    _status = ConnectionStatus.failed;
    _failure = failure;
    _failureMessage = message;
    _stderrTail = List.unmodifiable(stderrTail);
    _exitCode = exitCode;
    _crashCount++;
    notifyListeners();
  }

  /// An intentional shutdown (`shutdown()` or the window closing): no crash count, no restart offer.
  void markStopped() {
    _status = ConnectionStatus.idle;
    _failure = null;
    _failureMessage = null;
    notifyListeners();
  }
}

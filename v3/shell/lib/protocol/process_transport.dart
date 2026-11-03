/// The real [Transport]: a spawned `seal-core.exe`, its stdin/stdout as NDJSON, its stderr as logs.
///
/// The shell spawns the core from its own directory with no arguments ([PROTOCOL.md] §1); this
/// transport only does what the protocol says — it does not hide the window, find the root or retry.
/// The shell passes the executable path (resolved beside itself) and, for development, a `--root`
/// value; the core's root discovery is its own business ([DESIGN.md] §3).
///
/// A closed or broken stdin is the failure the client must see at once (SHELL-F1): `stdin.done` is
/// observed and reported through [writeFailures], so a write to a dead core fails its pending request
/// immediately instead of waiting out the request timeout. [terminate] is graceful-then-forceful —
/// stdin EOF first (the safe stop), then `kill()` when the core does not exit within ~2 s — so no
/// live process is ever left behind with an open pipe.
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
/// [DESIGN.md]: ../../../docs/spec/DESIGN.md
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'client.dart';

/// A core process over its stdio.
class CoreProcessTransport implements Transport {
  CoreProcessTransport._(this._process);

  final Process _process;

  final _writeFailures = StreamController<Object>.broadcast();

  /// The single in-flight termination; every caller (a close after a crash, a restart) awaits this.
  Future<void>? _termination;

  /// Whether `stdin.done` has been observed (one listener per process is enough).
  bool _stdinObserved = false;

  /// Spawns the core. Throws whatever `Process.start` throws (a missing executable is a crash the
  /// shell shows, not a silent fallback).
  static Future<CoreProcessTransport> start({
    required String executable,
    List<String> arguments = const [],
    String? workingDirectory,
  }) async {
    final process = await Process.start(
      executable,
      arguments,
      workingDirectory: workingDirectory,
      runInShell: false,
    );
    return CoreProcessTransport._(process);
  }

  /// The process's exit code, completing when the core exits.
  Future<int> get exitCode => _process.exitCode;

  /// The OS process, for a caller that needs the pid.
  int get pid => _process.pid;

  @override
  Stream<String> get lines => _decode(_process.stdout);

  @override
  Stream<String> get diagnostics =>
      _decode(_process.stderr, allowMalformed: true);

  @override
  Stream<Object> get writeFailures => _writeFailures.stream;

  @override
  void writeLine(String line) {
    // `writeln` appends the platform line terminator; the protocol reads by line and the core strips
    // `\r`, so this is safe on Windows and is the one place a newline is chosen.
    try {
      _process.stdin.writeln(line);
    } catch (error) {
      _reportWriteFailure(error);
      rethrow;
    }
    _observeStdin();
  }

  /// Observes `stdin.done` ONCE: a write to a dead core reports "broken pipe" there, never from
  /// `writeln`, so without this the request would wait out its full timeout (SHELL-F1).
  void _observeStdin() {
    if (_stdinObserved) {
      return;
    }
    _stdinObserved = true;
    unawaited(
      _process.stdin.done.then<void>(
        (_) {},
        onError: (Object error, StackTrace _) {
          _reportWriteFailure(error);
        },
      ),
    );
  }

  void _reportWriteFailure(Object error) {
    if (!_writeFailures.isClosed) {
      _writeFailures.add(error);
    }
  }

  @override
  Future<void> closeInput() => _process.stdin.close();

  /// Stops the process: close stdin (the safe EOF stop), wait up to ~2 s for the core to exit, then
  /// kill. Idempotent — every caller shares the same termination future, so a restart that killed a
  /// wedged core does not kill a second time or hang on an already-dead process.
  @override
  Future<void> terminate() => _termination ??= _terminate();

  Future<void> _terminate() async {
    try {
      await _process.stdin.close();
    } catch (_) {
      // The pipe is already gone; the process is what matters below.
    }
    try {
      await _process.exitCode.timeout(const Duration(seconds: 2));
    } on TimeoutException {
      _process.kill();
      try {
        await _process.exitCode.timeout(const Duration(seconds: 2));
      } catch (_) {
        // A process that survives kill() cannot be waited for; the caller must not hang either way.
      }
    }
    if (!_writeFailures.isClosed) {
      unawaited(_writeFailures.close());
    }
  }

  Stream<String> _decode(
    Stream<List<int>> bytes, {
    bool allowMalformed = false,
  }) => bytes
      .transform(
        allowMalformed ? const Utf8Decoder(allowMalformed: true) : utf8.decoder,
      )
      .transform(const LineSplitter());
}

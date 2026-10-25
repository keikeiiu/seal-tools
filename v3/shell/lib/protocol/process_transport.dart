/// The real [Transport]: a spawned `seal-core.exe`, its stdin/stdout as NDJSON, its stderr as logs.
///
/// The shell spawns the core from its own directory with no arguments ([PROTOCOL.md] §1); this
/// transport only does what the protocol says — it does not hide the window, find the root or retry.
/// The shell passes the executable path (resolved beside itself) and, for development, a `--root`
/// value; the core's root discovery is its own business ([DESIGN.md] §3).
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
/// [DESIGN.md]: ../../../docs/spec/DESIGN.md
library;

import 'dart:convert';
import 'dart:io';

import 'client.dart';

/// A core process over its stdio.
class CoreProcessTransport implements Transport {
  CoreProcessTransport._(this._process);

  final Process _process;

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
  Stream<String> get diagnostics => _decode(_process.stderr);

  @override
  void writeLine(String line) {
    // `writeln` appends the platform line terminator; the protocol reads by line and the core strips
    // `\r`, so this is safe on Windows and is the one place a newline is chosen.
    _process.stdin.writeln(line);
  }

  @override
  Future<void> closeInput() => _process.stdin.close();

  @override
  Future<void> terminate() async {
    _process.kill();
    await _process.exitCode;
  }

  Stream<String> _decode(Stream<List<int>> bytes) =>
      bytes.transform(utf8.decoder).transform(const LineSplitter());
}

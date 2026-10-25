/// The log sub-model: a bounded ring of what the status surface shows.
///
/// Two sources feed it and they stay distinguishable: `log` EVENTS are protocol messages
/// ([PROTOCOL.md] §5) and core stderr is not protocol at all — it is where a boot refusal goes
/// ([PROTOCOL.md] §1), and the shell must never parse it. Shell notes (a command that could not be
/// sent, an unrecognised event) are a third origin so the surface never pretends they came from the
/// core.
library;

import 'package:flutter/foundation.dart';

import '../protocol/protocol.dart';

/// Where a displayed line came from.
enum LogOrigin {
  /// A `log` event — the core's own line.
  core,

  /// The core process's stderr — never parsed, shown as-is.
  stderr,

  /// The shell talking to itself (command not sent, unknown event, …).
  shell,
}

class LogEntry {
  const LogEntry({required this.text, required this.origin, this.level});

  final String text;
  final LogOrigin origin;

  /// The `log` event's level (`info`/`warn`/`error`); null for stderr/shell lines.
  final String? level;
}

class LogBuffer extends ChangeNotifier {
  /// The status surface keeps the most recent lines only; the crash tail is separate (bounded by the
  /// connection model) and is what the crash view shows in full.
  static const int maxLines = 200;

  final List<LogEntry> _entries = [];
  List<String> _stderrTail = const [];

  /// Oldest first. Unmodifiable.
  List<LogEntry> get entries => List.unmodifiable(_entries);

  /// The last stderr lines captured before a crash; empty while healthy.
  List<String> get stderrTail => _stderrTail;

  /// The most recent [count] entries, oldest first.
  List<LogEntry> tail([int count = 20]) {
    if (_entries.length <= count) {
      return entries;
    }
    return List.unmodifiable(_entries.sublist(_entries.length - count));
  }

  // ── mutations: ShellModel only ────────────────────────────────────────────────────────────

  void addEvent(LogEvent event) => addCoreLog(event.log);

  /// A `log` event's payload ([PROTOCOL.md] §5) — the core's own line, level preserved for styling.
  void addCoreLog(LogLine log) {
    _add(
      LogEntry(text: log.line, origin: LogOrigin.core, level: log.level.wire),
    );
  }

  void addStderr(String line) {
    _add(LogEntry(text: line, origin: LogOrigin.stderr));
  }

  void addShellNote(String text) {
    _add(LogEntry(text: text, origin: LogOrigin.shell));
  }

  /// Records the stderr tail at a crash so the crash view can show it without re-reading a dead
  /// process's stream.
  void captureStderrTail(List<String> tail) {
    _stderrTail = List.unmodifiable(tail);
    notifyListeners();
  }

  void _add(LogEntry entry) {
    _entries.add(entry);
    if (_entries.length > maxLines) {
      _entries.removeRange(0, _entries.length - maxLines);
    }
    notifyListeners();
  }
}

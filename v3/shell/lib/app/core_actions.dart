/// Typed results for the read-only Setup actions (`window.find`, `window.measure`, `board.probe`).
///
/// These are DISPLAY shapes: the core's answer, decoded by the model and rendered by the tab. A
/// refusal keeps the [ErrorObject] verbatim; no widget invents a line.
///
/// The result shapes are the core's: `window.find -> {found, title?}`, `window.measure ->
/// {display: DisplayInfo}` (the domain type), `board.probe -> {port, baud, vid, pid, level}` with the
/// VID/PID already formatted as four hex digits by the core.
library;

import '../protocol/protocol.dart';
import 'config_json.dart';

/// A command that returns data or a typed refusal.
class CoreActionResult<T> {
  const CoreActionResult({this.value, this.error, this.note});

  const CoreActionResult.success(T result)
    : value = result,
      error = null,
      note = null;

  const CoreActionResult.failure({ErrorObject? coreError, String? shellNote})
    : value = null,
      error = coreError,
      note = shellNote;

  final T? value;
  final ErrorObject? error;

  /// A shell-side reason the command was not sent (not ready, connection gone).
  final String? note;

  bool get ok => value != null;

  /// The core's own line, or the shell's reason when there is no core answer to show.
  String? get errorLine => error?.message ?? note;
}

/// `window.find`'s result. "Not found" is a result, not an error ([PROTOCOL.md] §4).
class WindowFindResult {
  const WindowFindResult({required this.found, this.title});

  final bool found;
  final String? title;

  factory WindowFindResult.fromJson(JsonMap map) => WindowFindResult(
    found: asBool(map['found']) ?? false,
    title: asString(map['title']),
  );
}

/// `board.probe`'s result ([PROTOCOL.md] §4).
class BoardProbeResult {
  const BoardProbeResult({
    required this.port,
    required this.baud,
    this.vid,
    this.pid,
    required this.level,
  });

  final String port;
  final int baud;

  /// Four lowercase hex digits, or null when the OS did not report an id.
  final String? vid;
  final String? pid;

  /// Firmware protocol level: 1 = silent (legacy), 2 = replies to `V`.
  final int level;

  factory BoardProbeResult.fromJson(JsonMap map) => BoardProbeResult(
    port: asString(map['port']) ?? '?',
    baud: asNumber(map['baud'])?.round() ?? 0,
    vid: asString(map['vid']),
    pid: asString(map['pid']),
    level: asNumber(map['level'])?.round() ?? 1,
  );
}

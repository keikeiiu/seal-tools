/// A durable shell-side log: widget exceptions and the shell's own notes are invisible in a release
/// GUI (no console), so they are appended to `<root>/logs/shell-YYYYMMDD.log` (SHELL-F7).
///
/// It rotates at 1 MiB and keeps five older generations. Writes are synchronous and best-effort: a
/// logging failure must never become the next failure.
library;

import 'dart:io';

class ShellFileLog {
  ShellFileLog(this._root);

  final Directory _root;

  static const int _maxBytes = 1024 * 1024;
  static const int _keep = 5;

  /// Appends one line; `origin` is `shell`/`stderr`/`core` so a later reader can tell them apart.
  void write(String origin, String text) {
    try {
      final now = DateTime.now();
      final file = File('${_root.path}/logs/shell-${_stamp(now)}.log');
      if (!file.parent.existsSync()) {
        file.parent.createSync(recursive: true);
      }
      if (file.existsSync() && file.lengthSync() > _maxBytes) {
        _rotate(file);
      }
      final line =
          '${_timestamp(now)} [$origin] ${text.replaceAll('\n', '\\n')}\n';
      file.writeAsStringSync(line, mode: FileMode.append, flush: true);
    } catch (_) {
      // Best effort; a broken log directory must not break the GUI.
    }
  }

  void _rotate(File file) {
    final oldest = File('${file.path}.$_keep');
    if (oldest.existsSync()) {
      oldest.deleteSync();
    }
    for (var generation = _keep - 1; generation >= 1; generation--) {
      final from = File('${file.path}.$generation');
      if (from.existsSync()) {
        from.renameSync('${file.path}.${generation + 1}');
      }
    }
    if (file.existsSync()) {
      file.renameSync('${file.path}.1');
    }
  }

  static String _stamp(DateTime when) =>
      '${when.year.toString().padLeft(4, '0')}'
      '${when.month.toString().padLeft(2, '0')}'
      '${when.day.toString().padLeft(2, '0')}';

  static String _timestamp(DateTime when) =>
      '${when.year.toString().padLeft(4, '0')}-'
      '${when.month.toString().padLeft(2, '0')}-'
      '${when.day.toString().padLeft(2, '0')} '
      '${when.hour.toString().padLeft(2, '0')}:'
      '${when.minute.toString().padLeft(2, '0')}:'
      '${when.second.toString().padLeft(2, '0')}';
}

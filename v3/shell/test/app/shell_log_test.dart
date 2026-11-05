/// The shell-side durable log (SHELL-F7).
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/logs.dart';
import 'package:seal_shell/app/shell_log.dart';

void main() {
  test('writes a timestamped origin line under logs/', () {
    final root = Directory.systemTemp.createTempSync('seal-shell-log-');
    addTearDown(() => root.deleteSync(recursive: true));
    final log = ShellFileLog(root);

    log.write('shell', 'a widget threw');
    log.write('stderr', 'seal-core: cannot load config');

    final files = Directory('${root.path}/logs')
        .listSync()
        .whereType<File>()
        .toList();
    expect(files, hasLength(1));
    final lines = files.single.readAsLinesSync();
    expect(lines, hasLength(2));
    expect(lines.first, contains('[shell] a widget threw'));
    expect(lines.last, contains('[stderr] seal-core: cannot load config'));
    expect(
      RegExp(r'^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} ').hasMatch(lines.first),
      isTrue,
    );
  });

  test('rotates a full file and keeps the older generation', () {
    final root = Directory.systemTemp.createTempSync('seal-shell-log-');
    addTearDown(() => root.deleteSync(recursive: true));
    final log = ShellFileLog(root);
    log.write('shell', 'first');
    final file = Directory('${root.path}/logs')
        .listSync()
        .whereType<File>()
        .single;
    // Fill past the 1 MiB threshold; the next write rotates.
    file.writeAsStringSync('x' * (1024 * 1024 + 1), mode: FileMode.append);

    log.write('shell', 'after rotation');

    final rotated = File('${file.path}.1');
    expect(rotated.existsSync(), isTrue);
    expect(file.readAsLinesSync().last, contains('after rotation'));
    expect(rotated.lengthSync(), greaterThan(1024 * 1024));
  });

  test('a log sink receives the origin and text of every entry', () {
    final logs = LogBuffer();
    final seen = <String>[];
    logs.sink = (origin, text) => seen.add('${origin.name}:$text');

    logs.addShellNote('note');
    logs.addStderr('stderr line');

    expect(seen, ['shell:note', 'stderr:stderr line']);
  });
}

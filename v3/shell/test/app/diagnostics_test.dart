import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:seal_shell/app/diagnostics.dart';
import 'package:seal_shell/app/zip_store.dart';

void main() {
  test(
    'stored zip has the local, central and end signatures and its names',
    () {
      final zip = ZipStoreBuilder()
        ..addFile('logs/shell.log', utf8.encode('shell'))
        ..addFile('manifest.json', utf8.encode('{}'))
        ..addFile('README.txt', utf8.encode('read me'));
      final bytes = zip.build();
      final data = ByteData.sublistView(Uint8List.fromList(bytes));
      expect(data.getUint32(0, Endian.little), 0x04034b50);
      expect(data.getUint32(bytes.length - 22, Endian.little), 0x06054b50);
      final text = utf8.decode(bytes, allowMalformed: true);
      expect(text, contains('logs/shell.log'));
      expect(text, contains('manifest.json'));
      expect(text, contains('README.txt'));
    },
  );

  test('diagnostics bundle carries logs and excludes calibration/captures', () async {
    final root = await Directory.systemTemp.createTemp('seal-diag-');
    addTearDown(() => root.delete(recursive: true));
    final logs = Directory('${root.path}${Platform.pathSeparator}logs');
    await logs.create(recursive: true);
    await File('${logs.path}${Platform.pathSeparator}shell-20260101.log')
        .writeAsString('shell note');
    final captures = Directory('${logs.path}${Platform.pathSeparator}captures');
    await captures.create();
    await File('${captures.path}${Platform.pathSeparator}capture.png')
        .writeAsBytes([1, 2, 3]);
    final machine = Directory(
      '${root.path}${Platform.pathSeparator}config'
      '${Platform.pathSeparator}machine',
    );
    await machine.create(recursive: true);
    await File('${machine.path}${Platform.pathSeparator}tuner.yaml')
        .writeAsString('spring_point: 1,2');

    final export = await DiagnosticsExporter(
      root: root.path,
      appVersion: '9.9.9',
      coreVersion: '9.9.8',
    ).export();
    expect(File(export.zipPath).existsSync(), isTrue);
    expect(export.logFiles, 1);
    final bytes = await File(export.zipPath).readAsBytes();
    final text = utf8.decode(bytes, allowMalformed: true);
    expect(text, contains('logs/shell-20260101.log'));
    expect(text, contains('manifest.json'));
    expect(text, contains('README.txt'));
    expect(text, isNot(contains('capture.png')));
    // The manifest may name a config file, but it must never carry its values.
    expect(text, isNot(contains('spring_point')));
  });
}

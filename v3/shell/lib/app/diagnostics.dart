import 'dart:convert';
import 'dart:io';

import 'zip_store.dart';

/// One successfully written support bundle.
class DiagnosticsExport {
  const DiagnosticsExport({
    required this.zipPath,
    required this.logFiles,
    required this.bytes,
  });

  final String zipPath;
  final int logFiles;
  final int bytes;
}

/// Collects the support facts a bug report needs without personal calibration or captures.
///
/// The bundle carries: a manifest (app/core version, root, OS, timestamp), the shell's current log
/// tail, the core's text logs and OCR diagnostics, and a list of config file names/sizes. It does
/// **not** copy `config/machine/`, `config/artifacts/`, `captures/` or any configuration values: those
/// are personal and often larger than a useful support bundle.
class DiagnosticsExporter {
  DiagnosticsExporter({
    required this.root,
    required this.appVersion,
    required this.coreVersion,
  });

  final String root;
  final String appVersion;
  final String? coreVersion;

  static const int _maxLogBytes = 2 * 1024 * 1024;

  Future<DiagnosticsExport> export() async {
    final now = DateTime.now();
    final stamp =
        '${now.year.toString().padLeft(4, '0')}'
        '${now.month.toString().padLeft(2, '0')}'
        '${now.day.toString().padLeft(2, '0')}-'
        '${now.hour.toString().padLeft(2, '0')}'
        '${now.minute.toString().padLeft(2, '0')}'
        '${now.second.toString().padLeft(2, '0')}';
    final logsDir = Directory('$root${Platform.pathSeparator}logs');
    await logsDir.create(recursive: true);
    final zipFile = File(
      '${logsDir.path}${Platform.pathSeparator}'
      'SealTools-diagnostics-$stamp.zip',
    );
    final zip = ZipStoreBuilder();

    final logFiles = <String>[];
    final skipped = <String>[];
    if (await logsDir.exists()) {
      final entries = await logsDir.list(followLinks: false).toList();
      entries.sort((a, b) => a.path.compareTo(b.path));
      for (final entity in entries) {
        if (entity is! File) {
          continue;
        }
        final name = entity.uri.pathSegments.last;
        if (!name.endsWith('.log') && !name.endsWith('.jsonl')) {
          continue;
        }
        final length = await entity.length();
        if (length > _maxLogBytes) {
          skipped.add('$name (too large: $length bytes)');
          continue;
        }
        zip.addFile('logs/$name', await entity.readAsBytes());
        logFiles.add(name);
      }
    }

    final configFiles = <String>[];
    final configDir = Directory('$root${Platform.pathSeparator}config');
    if (await configDir.exists()) {
      await for (final entity in configDir.list(
        recursive: true,
        followLinks: false,
      )) {
        if (entity is! File) {
          continue;
        }
        final relative = entity.path
            .substring(configDir.path.length + 1)
            .replaceAll(Platform.pathSeparator, '/');
        if (relative.contains('artifacts/') || relative.endsWith('.png')) {
          continue;
        }
        configFiles.add(relative);
        if (configFiles.length >= 200) {
          break;
        }
      }
    }

    final manifest = const JsonEncoder.withIndent('  ').convert({
      'app': 'Seal Tools',
      'app_version': appVersion,
      'core_version': coreVersion,
      'root': root,
      'exported_at': now.toUtc().toIso8601String(),
      'os': Platform.operatingSystemVersion,
      'log_files': logFiles,
      'skipped_log_files': skipped,
      'config_files': configFiles,
      'excluded': [
        'config/machine (personal calibration)',
        'config/artifacts (reference images)',
        'logs/captures (screenshots)',
      ],
    });
    zip
      ..addFile('manifest.json', utf8.encode(manifest))
      ..addFile(
        'README.txt',
        utf8.encode(
          'Seal Tools diagnostics bundle\n\n'
          'This bundle contains the shell/core logs and a manifest only. It intentionally excludes '
          'personal calibration (config/machine), reference images (config/artifacts), screenshots '
          '(logs/captures) and every configuration value. Attach it to a bug report with the steps '
          'that reproduce the problem.\n',
        ),
      );

    final bytes = zip.build();
    await zipFile.writeAsBytes(bytes, flush: true);
    return DiagnosticsExport(
      zipPath: zipFile.path,
      logFiles: logFiles.length,
      bytes: bytes.length,
    );
  }
}

/// Loads the golden fixture set from the repository.
///
/// The fixtures live at `v3/protocol/fixtures/`, which is beside the shell package's parent — the
/// loader walks up from the test's working directory so `flutter test` works from `v3/shell` and from
/// an IDE whose cwd differs.
library;

import 'dart:convert';
import 'dart:io';

import 'package:seal_shell/protocol/protocol.dart';

/// One fixture file: its case name, its kind, and the message object.
class FixtureFile {
  const FixtureFile({
    required this.caseName,
    required this.kind,
    required this.message,
  });

  final String caseName;
  final String kind;
  final JsonMap message;

  String get fileName => '$caseName.json';
}

/// Every fixture, sorted by case name for stable test output.
List<FixtureFile> loadFixtures() {
  final directory = _fixturesDirectory();
  final files =
      directory
          .listSync()
          .whereType<File>()
          .where((file) => file.path.endsWith('.json'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));

  return [
    for (final file in files)
      _parseFixture(file, utf8.decode(file.readAsBytesSync())),
  ];
}

FixtureFile _parseFixture(File file, String text) {
  final Object? decoded = jsonDecode(text);
  final map = decodeMap(decoded, file.path);
  return FixtureFile(
    caseName: decodeString(map['case'], '${file.path}: case'),
    kind: decodeString(map['kind'], '${file.path}: kind'),
    message: decodeMap(map['message'], '${file.path}: message'),
  );
}

Directory _fixturesDirectory() {
  final separator = Platform.pathSeparator;
  var directory = Directory.current.absolute;
  for (var depth = 0; depth < 8; depth++) {
    final candidate = Directory(
      '${directory.path}${separator}protocol${separator}fixtures',
    );
    if (candidate.existsSync()) {
      return candidate;
    }
    final parent = directory.parent;
    if (parent.path == directory.path) {
      break;
    }
    directory = parent;
  }
  throw StateError(
    'could not find protocol/fixtures above ${Directory.current.path}',
  );
}

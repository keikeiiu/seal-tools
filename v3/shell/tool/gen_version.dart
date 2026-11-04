/// Generates `lib/app/version.dart` from `pubspec.yaml`, the single source of truth (SHELL-F10).
///
/// `pubspec.yaml`'s `version:` is what Flutter stamps into the Windows executable's resource and what
/// a release zip is named after. The app bar needs the same value at compile time, so this script
/// writes the Dart constant instead of letting a human keep two copies in step. Run it from
/// `v3/shell` after changing the version:
///
///   C:\src\flutter\bin\dart.bat run tool/gen_version.dart
///
/// `test/app/version_test.dart` fails when the generated file disagrees with `pubspec.yaml`, and
/// `v3/publish.ps1` refuses to build a release unless pubspec, `Cargo.toml` and this file agree.
library;

import 'dart:io';

/// The shell's PROTOCOL CLIENT identity, sent in `hello`'s `client` field (CUTOVER decision 2). It
/// versions the shell↔core conversation, not the product; do not move it with the product version.
const String _protocolClientVersion = '0.1.0';

final RegExp _pubspecVersion = RegExp(
  r'^version:\s*(\d+\.\d+\.\d+)\+\d+\s*$',
  multiLine: true,
);

void main(List<String> arguments) {
  final shellDir = File.fromUri(Platform.script).parent.parent;
  final pubspec = File('${shellDir.path}/pubspec.yaml');
  final versionFile = File('${shellDir.path}/lib/app/version.dart');
  final match = _pubspecVersion.firstMatch(pubspec.readAsStringSync());
  if (match == null) {
    stderr.writeln(
      'pubspec.yaml has no `version: X.Y.Z+N` line; nothing was written.',
    );
    exitCode = 1;
    return;
  }
  final product = match.group(1)!;
  final generated = _render(product);

  if (arguments.contains('--check')) {
    final current = versionFile.existsSync()
        ? versionFile.readAsStringSync()
        : '';
    if (current != generated) {
      stderr.writeln(
        'lib/app/version.dart is stale; run tool/gen_version.dart.',
      );
      exitCode = 1;
      return;
    }
    stdout.writeln('version.dart matches pubspec.yaml ($product).');
    return;
  }

  versionFile.writeAsStringSync(generated);
  stdout.writeln('Wrote lib/app/version.dart for $product.');
}

String _render(String product) =>
    '''
// GENERATED FILE - do not edit by hand.
//
// Source of truth: `pubspec.yaml`'s `version:` line. Regenerate from `v3/shell` with:
//   C:\\src\\flutter\\bin\\dart.bat run tool/gen_version.dart
//
// The product version and the protocol client version are different numbers with different
// lifetimes (see `lib/app/version.dart`'s original rationale; CUTOVER.md decision 2). Only the
// product version is generated.
library;

/// The product version shown in the app bar and named by a release. Keep it in step with
/// `pubspec.yaml`'s `version:` (which also feeds `SealTools.exe`'s Windows resource version).
const String productVersion = '$product';

/// The shell's PROTOCOL CLIENT identity, sent in `hello`'s `client` field. This is an engineering
/// value - do not change it when the product version changes.
const String protocolClientVersion = '$_protocolClientVersion';
''';

/// SHELL-F10: the product version has ONE source of truth — `pubspec.yaml`.
///
/// `lib/app/version.dart` is generated from it by `tool/gen_version.dart`; this test fails when a
/// hand edit (or a version bump that ran the release script without regenerating) makes the app bar
/// disagree with the executable's resource version and the release zip's name.
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:seal_shell/app/version.dart';

void main() {
  test('productVersion matches pubspec.yaml', () {
    final pubspec = File('pubspec.yaml').readAsStringSync();
    final match = RegExp(
      r'^version:\s*(\d+\.\d+\.\d+)\+\d+\s*$',
      multiLine: true,
    ).firstMatch(pubspec);
    expect(
      match,
      isNotNull,
      reason: 'pubspec.yaml needs a `version: X.Y.Z+N` line',
    );
    expect(productVersion, match!.group(1));
  });

  test('version.dart is the generated file', () {
    final source = File('lib/app/version.dart').readAsStringSync();
    expect(source, contains('GENERATED FILE'));
    expect(
      source,
      contains("const String productVersion = '$productVersion';"),
    );
  });
}

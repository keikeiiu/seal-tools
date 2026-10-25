/// JSON comparison helpers for the fixture tests.
///
/// [expectSameShape] compares SHAPE, not values: a model's encoded keys must match the fixture's
/// recursively and each leaf must be JSON kind-compatible. That is the drift guard — a field added to
/// the fixture but not the Dart type (or the other way round) fails here first.
///
/// The comparison itself is pure Dart ([json_shape.dart]) so the local verification harness can use
/// the same logic without the Flutter test runner.
library;

import 'package:flutter_test/flutter_test.dart';

import 'json_shape.dart';

export 'json_shape.dart' show deepEquals, shapeDifferences;

/// Asserts that [actual]'s JSON shape matches [expected]'s, recursively.
void expectSameShape(Object? actual, Object? expected, [String path = r'$']) {
  final differences = shapeDifferences(actual, expected, path);
  if (differences.isEmpty) {
    return;
  }
  fail(differences.join('\n'));
}

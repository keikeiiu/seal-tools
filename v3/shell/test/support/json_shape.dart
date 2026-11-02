/// Pure-Dart JSON shape comparison, shared by the fixture tests and the local verification harness.
///
/// [shapeDifferences] compares SHAPE, not values: decoded keys must match the fixture's recursively
/// and each leaf must be JSON-kind compatible. That is the drift guard — a field added to one side
/// fails here first. It returns human-readable differences instead of throwing so a caller can decide
/// how to report them (the flutter tests use `expect`, the harness prints).
library;

/// Deep JSON equality; maps compare by key set and per-key value, lists element-wise.
bool deepEquals(Object? a, Object? b) {
  if (a is Map<String, dynamic> && b is Map<String, dynamic>) {
    if (a.length != b.length) {
      return false;
    }
    for (final key in a.keys) {
      if (!b.containsKey(key) || !deepEquals(a[key], b[key])) {
        return false;
      }
    }
    return true;
  }
  if (a is List && b is List) {
    if (a.length != b.length) {
      return false;
    }
    for (var index = 0; index < a.length; index++) {
      if (!deepEquals(a[index], b[index])) {
        return false;
      }
    }
    return true;
  }
  return a == b;
}

/// Every shape difference between [actual] and [expected], recursively; empty when they match.
List<String> shapeDifferences(
  Object? actual,
  Object? expected, [
  String path = r'$',
]) {
  final differences = <String>[];

  switch (expected) {
    case final Map<String, dynamic> expectedMap:
      if (actual is! Map<String, dynamic>) {
        differences.add('$path should be an object, was ${actual.runtimeType}');
        return differences;
      }
      final missing = expectedMap.keys.where((key) => !actual.containsKey(key));
      final extra = actual.keys.where((key) => !expectedMap.containsKey(key));
      for (final key in missing) {
        differences.add('$path.$key missing from the decoded model');
      }
      for (final key in extra) {
        differences.add('$path.$key is not in the fixture');
      }
      for (final key in expectedMap.keys) {
        if (actual.containsKey(key)) {
          differences.addAll(
            shapeDifferences(actual[key], expectedMap[key], '$path.$key'),
          );
        }
      }
    case final List<Object?> expectedList:
      if (actual is! List<Object?>) {
        differences.add('$path should be an array, was ${actual.runtimeType}');
        return differences;
      }
      if (actual.length != expectedList.length) {
        differences.add(
          '$path has ${actual.length} entries, the fixture has '
          '${expectedList.length}',
        );
        return differences;
      }
      for (var index = 0; index < expectedList.length; index++) {
        differences.addAll(
          shapeDifferences(actual[index], expectedList[index], '$path[$index]'),
        );
      }
    case null:
      if (actual != null) {
        differences.add('$path should be null, was ${actual.runtimeType}');
      }
    case String _:
      if (actual is! String) {
        differences.add('$path should be a string, was ${actual.runtimeType}');
      }
    case num _:
      if (actual is! num) {
        differences.add('$path should be a number, was ${actual.runtimeType}');
      }
    case bool _:
      if (actual is! bool) {
        differences.add('$path should be a boolean, was ${actual.runtimeType}');
      }
    default:
      differences.add(
        '$path has an unexpected fixture kind ${expected.runtimeType}',
      );
  }

  return differences;
}

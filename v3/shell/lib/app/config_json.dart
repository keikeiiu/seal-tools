/// Pure JSON helpers for the tab drafts: copying, equality and shape coercion.
///
/// No Flutter import, so these rules are locally testable headless. The tabs edit opaque config
/// blocks with them; the core remains the only writer and the only rule authoring.
library;

/// A deep copy of JSON data (maps and lists are copied, scalars shared). Null-safe.
Object? cloneJson(Object? value) {
  if (value is Map<String, dynamic>) {
    return {
      for (final entry in value.entries) entry.key: cloneJson(entry.value),
    };
  }
  if (value is List) {
    return [for (final entry in value) cloneJson(entry)];
  }
  return value;
}

/// Order-insensitive deep equality: map key order is not data, list order is.
bool jsonEquals(Object? a, Object? b) {
  if (identical(a, b)) {
    return true;
  }
  if (a is Map<String, dynamic> && b is Map<String, dynamic>) {
    if (a.length != b.length) {
      return false;
    }
    for (final key in a.keys) {
      if (!b.containsKey(key) || !jsonEquals(a[key], b[key])) {
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
      if (!jsonEquals(a[index], b[index])) {
        return false;
      }
    }
    return true;
  }
  return a == b;
}

/// [value] as a JSON object, or `{}` when it is absent/another type (hand-edited files are loose).
Map<String, dynamic> asJsonMap(Object? value) =>
    value is Map<String, dynamic> ? value : const {};

/// [value] as a JSON array, or `[]`.
List<Object?> asJsonList(Object? value) => value is List ? value : const [];

/// [value] as a list of strings (non-strings dropped rather than crashing a card).
List<String> asStringList(Object? value) => [
  for (final entry in asJsonList(value))
    if (entry is String) entry,
];

/// [value] as a number, or null.
num? asNumber(Object? value) => value is num ? value : null;

/// [value] as a string, or null.
String? asString(Object? value) => value is String ? value : null;

/// [value] as a bool, or null.
bool? asBool(Object? value) => value is bool ? value : null;

/// A copy of [source] with only [keys] kept; an empty [keys] means "whole object".
Map<String, dynamic> pickKeys(Map<String, dynamic> source, Set<String> keys) {
  if (keys.isEmpty) {
    return Map<String, dynamic>.of(source);
  }
  return {
    for (final key in keys)
      if (source.containsKey(key)) key: cloneJson(source[key]),
  };
}

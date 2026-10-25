/// JSON helpers shared by every wire type.
///
/// The protocol is one JSON object per line ([PROTOCOL.md] §2). Decoding is STRICT about the shape
/// the spec fixes (a missing `id` is a protocol error, not a null) and TOLERANT about unknown extra
/// fields, so a newer core can add a field without breaking an older shell.
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

/// A decoded JSON object. `dynamic` values are what `jsonDecode` produces; nothing outside this
/// library is expected to trust them.
typedef JsonMap = Map<String, dynamic>;

/// A protocol shape did not match the spec.
///
/// Typed rather than a raw `FormatException` so the client can tell a BROKEN LINE (a protocol
/// violation worth logging and failing the connection over) from an ordinary command error.
class ProtocolDecodeException implements Exception {
  const ProtocolDecodeException(
    this.message, {
    this.path,
    this.expected,
    this.found,
    this.cause,
  });

  /// What went wrong, in words.
  final String message;

  /// Where in the object it went wrong, e.g. `result.rows[1].status`.
  final String? path;

  /// The shape the spec requires, e.g. `string`.
  final String? expected;

  /// What was actually there.
  final Object? found;

  /// The underlying error, when this wraps one (a JSON parse error).
  final Object? cause;

  @override
  String toString() {
    final location = path == null ? '' : ' at $path';
    final wanted = expected == null ? '' : ' (expected $expected)';
    final value = found;
    final got = value == null ? '' : ' (found: ${_brief(value)})';
    return 'ProtocolDecodeException: $message$location$wanted$got';
  }

  static String _brief(Object value) {
    final text = value.toString();
    return text.length <= 40 ? text : '${text.substring(0, 37)}...';
  }
}

/// Requires a JSON object at [path].
JsonMap decodeMap(Object? value, String path) {
  if (value is Map<String, dynamic>) {
    return value;
  }
  throw ProtocolDecodeException(
    'expected a JSON object',
    path: path,
    expected: 'object',
    found: value,
  );
}

/// Requires a JSON object at [path], or null.
JsonMap? decodeNullableMap(Object? value, String path) {
  if (value == null) {
    return null;
  }
  return decodeMap(value, path);
}

/// Requires a JSON string at [path].
String decodeString(Object? value, String path) {
  if (value is String) {
    return value;
  }
  throw ProtocolDecodeException(
    'expected a JSON string',
    path: path,
    expected: 'string',
    found: value,
  );
}

/// Requires a JSON string at [path], or null.
String? decodeNullableString(Object? value, String path) {
  if (value == null) {
    return null;
  }
  return decodeString(value, path);
}

/// Requires a JSON integer at [path]. A double is rejected: the spec's integers are ids, counts and
/// pixel geometry, and accepting `1.0` would let a shape drift silently.
int decodeInt(Object? value, String path) {
  if (value is int) {
    return value;
  }
  throw ProtocolDecodeException(
    'expected a JSON integer',
    path: path,
    expected: 'integer',
    found: value,
  );
}

/// Requires a JSON number at [path].
num decodeNum(Object? value, String path) {
  if (value is num) {
    return value;
  }
  throw ProtocolDecodeException(
    'expected a JSON number',
    path: path,
    expected: 'number',
    found: value,
  );
}

/// Requires a JSON number at [path], as a double.
double decodeDouble(Object? value, String path) {
  final number = decodeNum(value, path);
  return number.toDouble();
}

/// Requires a JSON number at [path], or null.
double? decodeNullableDouble(Object? value, String path) {
  if (value == null) {
    return null;
  }
  return decodeDouble(value, path);
}

/// Requires a JSON boolean at [path].
bool decodeBool(Object? value, String path) {
  if (value is bool) {
    return value;
  }
  throw ProtocolDecodeException(
    'expected a JSON boolean',
    path: path,
    expected: 'boolean',
    found: value,
  );
}

/// Decodes a JSON array at [path] with [element] for every entry.
List<T> decodeList<T>(
  Object? value,
  String path,
  T Function(Object? value, String path) element,
) {
  if (value is List) {
    return [
      for (var index = 0; index < value.length; index++)
        element(value[index], '$path[$index]'),
    ];
  }
  throw ProtocolDecodeException(
    'expected a JSON array',
    path: path,
    expected: 'array',
    found: value,
  );
}

/// Decodes a JSON array of strings.
List<String> decodeStringList(Object? value, String path) =>
    decodeList(value, path, decodeString);

/// Decodes a JSON array of arrays of strings — the OCR scan's `attributes` pairs.
List<List<String>> decodeStringPairs(Object? value, String path) => decodeList(
  value,
  path,
  (entry, entryPath) => decodeStringList(entry, entryPath),
);

/// True when [value] is a map and has [key].
bool hasKey(Object? value, String key) =>
    value is Map<String, dynamic> && value.containsKey(key);

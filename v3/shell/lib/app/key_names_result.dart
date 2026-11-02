/// The `keys.parse` / `keys.names` results, decoded APP-LOCALLY (the shell alone consumes these two
/// untyped answers, like `attributes.get`; their golden fixtures pin the envelope and exact result
/// keys, while the typed models stay app-local).
///
/// The rule is the domain's, not the tab's:
/// - `keys.parse {"names": {field: typedName}}` → `{codes: {field: vk}, errors: [{field, input, line}]}`;
///   every parseable field is answered even when another box is wrong, and the line is the domain's
///   verbatim text for that box.
/// - `keys.names {"codes": {field: vk}}` → `{names: {field: displayName}}`, including v2's hex
///   fallback (`"0x5"`) for a code no name covers — a hand-edited config must display, not be refused.
library;

import '../protocol/protocol.dart';
import 'config_json.dart';

/// One box `keys.parse` could not turn into a VK code.
class KeyParseError {
  const KeyParseError({
    required this.field,
    required this.input,
    required this.line,
  });

  final String field;

  /// What the player typed.
  final String input;

  /// The domain's player-facing line, rendered verbatim.
  final String line;

  factory KeyParseError.fromJson(JsonMap map) => KeyParseError(
    field: asString(map['field']) ?? '?',
    input: asString(map['input']) ?? '',
    line: asString(map['line']) ?? '',
  );
}

/// `keys.parse`'s answer: the codes it could compute plus one line per bad box.
class KeyParseResult {
  const KeyParseResult({required this.codes, required this.errors});

  /// Fields that parsed, as VK codes.
  final Map<String, int> codes;
  final List<KeyParseError> errors;

  bool get ok => errors.isEmpty;

  factory KeyParseResult.fromJson(JsonMap map) {
    final codes = <String, int>{};
    for (final entry in asJsonMap(map['codes']).entries) {
      final number = asNumber(entry.value);
      if (number != null) {
        codes[entry.key] = number.round();
      }
    }
    return KeyParseResult(
      codes: codes,
      errors: [
        for (final entry in asJsonList(map['errors']))
          KeyParseError.fromJson(asJsonMap(entry)),
      ],
    );
  }
}

/// `keys.names`'s answer: the display name for every code asked about.
class KeyNamesResult {
  const KeyNamesResult({required this.names});

  final Map<String, String> names;

  factory KeyNamesResult.fromJson(JsonMap map) {
    final names = <String, String>{};
    for (final entry in asJsonMap(map['names']).entries) {
      final name = asString(entry.value);
      if (name != null) {
        names[entry.key] = name;
      }
    }
    return KeyNamesResult(names: names);
  }
}

/// The four editable hotkey fields, in the tab's display order (the config stores VK codes per name).
const List<String> hotkeyFields = ['start', 'quit', 'advance_grade', 'pause'];

/// Friendly labels for [hotkeyFields]; the config's field names stay the payload keys.
const Map<String, String> hotkeyLabels = {
  'start': 'Start',
  'quit': 'Quit',
  'advance_grade': 'Advance grade',
  'pause': 'Pause',
};

/// The `attributes.get` result, decoded APP-LOCALLY (the shell alone consumes it; it is not a wire
/// type in `lib/protocol`, and the golden fixture pins the untyped envelope + key shape —
/// PROTOCOL §4 documents the untyped result and the shell decodes what it renders).
///
/// Shapes are the core's serde names: `{variant, dictionary, attributes: {attributes: [{name,
/// category, variants}], per_level_stats, negative_attr_markers, text_fixes: {whole, substring,
/// simplified_traditional, final, regex}}}`. `variant`/`dictionary` are null when no client resolved.
library;

import '../protocol/protocol.dart';
import 'config_json.dart';

/// One attribute definition.
class AttributeDef {
  const AttributeDef({
    required this.name,
    required this.category,
    required this.variants,
  });

  final String name;
  final String category;

  /// The OCR-garbled forms; the first is normally the exact name.
  final List<String> variants;

  factory AttributeDef.fromJson(JsonMap map) => AttributeDef(
    name: asString(map['name']) ?? '',
    category: asString(map['category']) ?? '',
    variants: asStringList(map['variants']),
  );

  /// True when the free-text filter matches this entry (name or any OCR form).
  bool matches(String query) {
    final needle = query.trim().toLowerCase();
    if (needle.isEmpty) {
      return true;
    }
    return name.toLowerCase().contains(needle) ||
        variants.any((variant) => variant.toLowerCase().contains(needle));
  }
}

/// One `{from, to}` cleanup-table entry.
class TextFixEntry {
  const TextFixEntry({required this.from, required this.to});

  final String from;
  final String to;

  factory TextFixEntry.fromJson(JsonMap map) => TextFixEntry(
    from: asString(map['from']) ?? '',
    to: asString(map['to']) ?? '',
  );
}

/// The dictionary the core has loaded for the active client.
class AttributesDictionary {
  const AttributesDictionary({
    required this.attributes,
    required this.perLevelStats,
    required this.negativeMarkers,
    required this.textFixes,
  });

  final List<AttributeDef> attributes;
  final List<String> perLevelStats;
  final List<String> negativeMarkers;

  /// Table name (`whole`, `substring`, `simplified_traditional`, `final`, `regex`) → entries, in the
  /// order the cleaner applies them.
  final Map<String, List<TextFixEntry>> textFixes;

  factory AttributesDictionary.fromJson(JsonMap map) {
    final rawFixes = asJsonMap(map['text_fixes']);
    return AttributesDictionary(
      attributes: [
        for (final entry in asJsonList(map['attributes']))
          AttributeDef.fromJson(asJsonMap(entry)),
      ],
      perLevelStats: asStringList(map['per_level_stats']),
      negativeMarkers: asStringList(map['negative_attr_markers']),
      textFixes: {
        for (final entry in rawFixes.entries)
          entry.key: [
            for (final fix in asJsonList(entry.value))
              TextFixEntry.fromJson(asJsonMap(fix)),
          ],
      },
    );
  }

  /// Attribute groups in the dictionary's own order (the order the tab lists them).
  List<({String category, List<AttributeDef> entries})> get byCategory {
    final order = <String>[];
    final grouped = <String, List<AttributeDef>>{};
    for (final attribute in attributes) {
      final category = attribute.category.isEmpty
          ? '(uncategorised)'
          : attribute.category;
      if (!grouped.containsKey(category)) {
        order.add(category);
        grouped[category] = [];
      }
      grouped[category]!.add(attribute);
    }
    return [
      for (final category in order)
        (category: category, entries: grouped[category]!),
    ];
  }
}

/// `attributes.get`'s answer.
class AttributesResult {
  const AttributesResult({
    required this.variant,
    required this.dictionary,
    required this.attributes,
  });

  /// The active client; null when the core could not resolve one.
  final String? variant;

  /// The dictionary FILE name the variant resolved to.
  final String? dictionary;

  final AttributesDictionary attributes;

  factory AttributesResult.fromJson(JsonMap map) => AttributesResult(
    variant: asString(map['variant']),
    dictionary: asString(map['dictionary']),
    attributes: AttributesDictionary.fromJson(asJsonMap(map['attributes'])),
  );

  bool get hasVariant => variant != null && variant!.isNotEmpty;
}

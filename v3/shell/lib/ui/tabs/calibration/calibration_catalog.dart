/// The served calibration kind table and the derived views the Calibrate workspace needs.
///
/// Presentation only, and the copy is deliberate: the CORE owns the kind table, target validity and
/// every refusal line (`PROTOCOL §12.7`/`§12.8`); this file only knows the 18 kinds = 35 marks well
/// enough to draw the target tree, the stored-coordinate table and the reference gallery. The
/// targets are the presentation-side allowed values (1-based/player-facing); the picker offers them
/// and the core validates them.
///
/// [CALIBRATION.md]: ../../../docs/spec/CALIBRATION.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import '../../../app/app.dart';
import '../../../protocol/protocol.dart' show DisplayInfo;

/// One served calibration kind: protocol name, picker group, player-facing label, shape, whether it
/// owns a crop artifact, whether its save needs v2's empty-result verdict, and the spec's allowed
/// `target` values when the kind marks a NAMED INSTANCE (PROTOCOL §12.8).
class CalibrationKind {
  const CalibrationKind(
    this.name,
    this.group,
    this.tool,
    this.label, {
    required this.isRect,
    this.hasArtifact = false,
    this.requiresEmptyChoice = false,
    this.targets,
  });

  /// The wire name (`bag_grid`, `pet_toggle`, …).
  final String name;

  /// The picker group the player labels the kind with (`Shop`, `Pet — rows`, …). Kept in the kind
  /// dropdown's item text so the existing vocabulary does not move.
  final String group;

  /// The tool that CONSUMES the mark (`Shop`, `Gem`, `Pet`, `Tuner`) — the target tree's grouping
  /// and the stored-coordinate table's filter.
  final String tool;

  final String label;
  final bool isRect;
  final bool hasArtifact;
  final bool requiresEmptyChoice;

  /// Null for a single mark; otherwise the presentation-side allowed values (1-based/player-facing).
  final List<String>? targets;

  bool get isTargeted => targets != null;

  String get shapeLabel => isRect ? 'rectangle (x, y, w, h)' : 'point (x, y)';

  /// The number of actual marks this kind owns (a targeted kind owns one per instance).
  int get markCount => targets?.length ?? 1;
}

/// Named-instance targets (PROTOCOL §12.8), kept here as presentation constants only.
const List<String> petPageTargets = ['ITEM1', 'ITEM2', 'ITEM3'];
const List<String> petRowTargets = ['1', '2', '3', '4'];
const List<String> gemGradeTargets = ['N', 'G', 'DG', 'Register', 'Combine'];
const List<String> gemResourceTargets = ['1', '2', '3'];

/// The consuming tools, in the table filter's order.
const List<String> calibrationTools = ['Shop', 'Gem', 'Pet', 'Tuner'];

/// The repaired + named-instance kind table (PROTOCOL §4/§12.7/§12.8). Groups only order the
/// picker; `focus_point`/`tuner_region` stay withdrawn.
const List<CalibrationKind> calibrationKinds = [
  // Shop
  CalibrationKind('bag_grid', 'Shop', 'Shop', 'Bag grid', isRect: true),
  CalibrationKind('bag_slot', 'Shop', 'Shop', 'Bag slot', isRect: true),
  CalibrationKind(
    'shop_list',
    'Shop',
    'Shop',
    'Shop list region',
    isRect: true,
  ),
  CalibrationKind(
    'scroll_point',
    'Shop',
    'Shop',
    'Focus / scroll point',
    isRect: false,
  ),
  CalibrationKind('max_button', 'Shop', 'Shop', 'MAX button', isRect: false),
  // Gem
  CalibrationKind(
    'result_gem_area',
    'Gem',
    'Gem',
    'Result gem area',
    isRect: true,
    hasArtifact: true,
    requiresEmptyChoice: true,
  ),
  CalibrationKind(
    'gem_grade',
    'Gem',
    'Gem',
    'Grade point',
    isRect: false,
    targets: gemGradeTargets,
  ),
  CalibrationKind(
    'gem_resource',
    'Gem',
    'Gem',
    'Resource slot point',
    isRect: false,
    targets: gemResourceTargets,
  ),
  // Pet — points
  CalibrationKind(
    'pet_menu',
    'Pet — points',
    'Pet',
    '目錄 (menu) point',
    isRect: false,
  ),
  CalibrationKind(
    'pet_feed',
    'Pet — points',
    'Pet',
    'Pet feed icon point',
    isRect: false,
  ),
  CalibrationKind(
    'pet_close',
    'Pet — points',
    'Pet',
    'Boarding X point',
    isRect: false,
  ),
  CalibrationKind(
    'pet_page_tab',
    'Pet — points',
    'Pet',
    'Bag page tab point',
    isRect: false,
    targets: petPageTargets,
  ),
  // Pet — bag
  CalibrationKind(
    'pet_bag_grid',
    'Pet — bag',
    'Pet',
    'Boarding bag grid',
    isRect: true,
  ),
  CalibrationKind(
    'pet_bag_slot',
    'Pet — bag',
    'Pet',
    'Boarding bag slot',
    isRect: true,
  ),
  // Pet — rows
  CalibrationKind(
    'pet_toggle',
    'Pet — rows',
    'Pet',
    'Row start/end button',
    isRect: true,
    targets: petRowTargets,
  ),
  CalibrationKind(
    'pet_boarding_slot',
    'Pet — rows',
    'Pet',
    'Row boarding pet slot',
    isRect: true,
    targets: petRowTargets,
  ),
  CalibrationKind(
    'pet_feeder_strip',
    'Pet — rows',
    'Pet',
    'Row food strip',
    isRect: true,
    targets: petRowTargets,
  ),
  // Tuner
  CalibrationKind(
    'spring_point',
    'Tuner',
    'Tuner',
    'Spring point',
    isRect: false,
  ),
];

/// One served kind grouped under the tool that consumes its marks (the target tree's unit).
class CalibrationToolGroup {
  const CalibrationToolGroup(this.tool, this.kinds);

  final String tool;
  final List<CalibrationKind> kinds;

  int get kindCount => kinds.length;

  int get markCount => kinds.fold(0, (total, kind) => total + kind.markCount);
}

/// The target tree: all 18 kinds grouped by consuming tool (Shop 5/5, Gem 3/9, Pet 9/20, Tuner 1/1).
final List<CalibrationToolGroup> calibrationToolGroups = [
  for (final tool in calibrationTools)
    CalibrationToolGroup(tool, [
      for (final kind in calibrationKinds)
        if (kind.tool == tool) kind,
    ]),
];

/// One actual mark: a kind, plus the named instance for a targeted kind.
class CalibrationMarkSlot {
  const CalibrationMarkSlot(this.kind, [this.target]);

  final CalibrationKind kind;
  final String? target;

  /// The stored-map key: kind alone, or `kind@target`.
  String get key => calibrationStoredKey(kind.name, target);

  /// Player-facing row label (`Row start/end button · 2`).
  String get label => target == null ? kind.label : '${kind.label} · $target';

  String get tool => kind.tool;

  String get shape => kind.isRect ? 'rect' : 'point';
}

/// The stored-map key for a mark. Stored marks are keyed by kind AND instance: a targeted kind has
/// one mark per target.
String calibrationStoredKey(String kind, String? target) =>
    target == null ? kind : '$kind@$target';

/// The kinds' marks in table order: 12 untargeted kinds + 23 instance marks = 35.
List<CalibrationMarkSlot> calibrationMarkSlots() => [
  for (final kind in calibrationKinds)
    if (kind.isTargeted)
      for (final target in kind.targets!) CalibrationMarkSlot(kind, target)
    else
      CalibrationMarkSlot(kind),
];

/// The marks a single kind owns, in instance order.
List<CalibrationMarkSlot> calibrationSlotsOf(CalibrationKind kind) => [
  for (final target in kind.targets ?? const <String?>[null])
    CalibrationMarkSlot(kind, target),
];

CalibrationKind calibrationKindByName(String name) =>
    calibrationKinds.firstWhere(
      (kind) => kind.name == name,
      orElse: () => calibrationKinds.first,
    );

/// How filled one kind's marks are — the target tree's semantic dot (colour is never the only
/// channel: the row also carries the word/count).
enum CalibrationFill { saved, partial, notSet }

CalibrationFill calibrationFill(
  CalibrationKind kind,
  Map<String, CalibrationFetchResult> stored,
) {
  final slots = calibrationSlotsOf(kind);
  final saved = slots.where((slot) => stored[slot.key]?.mark != null).length;
  if (saved == 0) {
    return CalibrationFill.notSet;
  }
  return saved == slots.length
      ? CalibrationFill.saved
      : CalibrationFill.partial;
}

int calibrationSavedCount(
  CalibrationKind kind,
  Map<String, CalibrationFetchResult> stored,
) =>
    calibrationSlotsOf(kind)
        .where((slot) => stored[slot.key]?.mark != null)
        .length;

int calibrationSavedMarks(Map<String, CalibrationFetchResult> stored) =>
    calibrationMarkSlots()
        .where((slot) => stored[slot.key]?.mark != null)
        .length;

/// The mark's value in the table's `x · y` / `x · y · w×h` vocabulary, or an em dash when unset.
String calibrationMarkValue(CalibrationMark? mark) {
  if (mark == null) {
    return '—';
  }
  if (!mark.isRect) {
    return '${mark.x}, ${mark.y}';
  }
  return '${mark.x}, ${mark.y} · ${mark.w}×${mark.h}';
}

// ── display environment ────────────────────────────────────────────────────────────────────

/// The environment a save records, as the shell can see it: the `calibration` block of the config
/// mirror (written by `calibration.save`, and refreshed only by `hello`/`config.reload`), or the
/// environment this screen just sent with a successful save.
///
/// Fields mirror the core's write (`calibration.rs`): `dpi_scale`, `monitor_dpi`, `screen`,
/// `client_size`. Comparison is presentation: order scale → client → screen, a scale difference
/// alone does NOT invalidate the marks, a client-size or screen difference does (CALIBRATION §6/§8).
/// No transform and no auto-anchor is ever applied.
class StoredCalibrationEnvironment {
  const StoredCalibrationEnvironment({
    this.scale,
    this.monitorDpi,
    this.clientWidth,
    this.clientHeight,
    this.screenWidth,
    this.screenHeight,
  });

  /// Parses the config mirror's `calibration` block. Fields the block does not carry are null and
  /// are NOT a difference ("nothing stored yet" is a separate state).
  factory StoredCalibrationEnvironment.fromConfig(Object? block) {
    final map = asJsonMap(block);
    final client = asJsonList(map['client_size']);
    final screen = asJsonMap(map['screen']);
    return StoredCalibrationEnvironment(
      scale: asNumber(map['dpi_scale'])?.toDouble(),
      monitorDpi: asNumber(map['monitor_dpi'])?.round(),
      clientWidth: client.isNotEmpty ? asNumber(client[0])?.round() : null,
      clientHeight: client.length > 1 ? asNumber(client[1])?.round() : null,
      screenWidth: asNumber(screen['w'])?.round(),
      screenHeight: asNumber(screen['h'])?.round(),
    );
  }

  /// The environment this screen just sent with a save: the capture's own `DisplayInfo` and the
  /// frame's dimensions as `client_size` (the core writes the FRAME's dimensions, `calibration.rs`).
  factory StoredCalibrationEnvironment.fromSave(
    DisplayInfo display,
    int frameWidth,
    int frameHeight,
  ) => StoredCalibrationEnvironment(
    scale: display.scale,
    monitorDpi: display.monitorDpi,
    clientWidth: frameWidth,
    clientHeight: frameHeight,
    screenWidth: display.screen.w,
    screenHeight: display.screen.h,
  );

  final double? scale;
  final int? monitorDpi;
  final int? clientWidth;
  final int? clientHeight;
  final int? screenWidth;
  final int? screenHeight;

  /// True when the shell has no stored environment to compare (never calibrated, or the mirror does
  /// not carry the block).
  bool get isEmpty =>
      scale == null && clientWidth == null && screenWidth == null;

  /// Human-readable differences between this stored environment and a live capture, in the spec's
  /// order (scale, client, screen). Empty = matches.
  List<String> differencesFrom(DisplayInfo display) {
    final lines = <String>[];
    final storedScale = scale;
    if (storedScale != null && (display.scale - storedScale).abs() > 0.001) {
      lines.add(
        'scale ${formatCalibrationScale(storedScale)} → '
        '${formatCalibrationScale(display.scale)} (a scale change alone does not invalidate '
        'stored marks; the cursor-move conversion uses the current scale)',
      );
    }
    if (clientWidth != null &&
        (clientWidth != display.physicalClient.w ||
            clientHeight != display.physicalClient.h)) {
      lines.add(
        'client $clientWidth×$clientHeight → '
        '${display.physicalClient.w}×${display.physicalClient.h} '
        '(every stored point lands elsewhere)',
      );
    }
    if (screenWidth != null &&
        (screenWidth != display.screen.w || screenHeight != display.screen.h)) {
      lines.add(
        'screen $screenWidth×$screenHeight → '
        '${display.screen.w}×${display.screen.h} '
        '(the calibration was measured on a different display)',
      );
    }
    return lines;
  }

  /// The one-line description the display chip and the banner share.
  String get summary {
    final parts = <String>[
      if (scale != null) '${formatCalibrationScale(scale!)}× scale',
      if (clientWidth != null) '$clientWidth×$clientHeight client',
      if (screenWidth != null) 'screen $screenWidth×$screenHeight',
    ];
    return parts.isEmpty ? 'nothing stored yet' : parts.join(' · ');
  }
}

/// `{0.###}`-style scale: at most three decimals, trailing zeros trimmed (the player reads it, and
/// it is how the CalibrationCheck lines format it, CALIBRATION §6).
String formatCalibrationScale(double value) {
  final fixed = value.toStringAsFixed(3);
  final trimmed = fixed.contains('.')
      ? fixed.replaceFirst(RegExp(r'0+$'), '').replaceFirst(RegExp(r'\.$'), '')
      : fixed;
  return trimmed.isEmpty ? '0' : trimmed;
}

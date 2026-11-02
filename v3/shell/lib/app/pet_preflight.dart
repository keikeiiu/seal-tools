/// `pet.preflight`'s answer, decoded APP-LOCALLY (the shell alone consumes it; the wire type is the
/// untyped envelope, PROTOCOL §4).
///
/// Shape (the core's serde names, `seal-core/src/pet.rs`): `{ready, calibration: [{ok,label,detail?}],
/// session: [{ok,label,detail?}], lines: [...]}`. The CORE owns the rules and v2's ready lines; this
/// file only decodes them so the tab can render the two Setup-so-far lists, the core's lines and the
/// ready verdict without re-deriving either. `detail` is tolerated absent (a list item may be just a
/// fact and its ok flag).
library;

import '../protocol/protocol.dart';
import 'config_json.dart';

/// One checklist row of `pet.preflight`: an ok/failed fact with the core's label and detail.
class PetChecklistItem {
  const PetChecklistItem({required this.ok, required this.label, this.detail});

  final bool ok;

  /// The short player-facing label ("return slot", "food cells"), from the core.
  final String label;

  /// The detail the player needs to judge the item (`"page 1, cell 6"`), or null.
  final String? detail;

  factory PetChecklistItem.fromJson(JsonMap map) => PetChecklistItem(
    ok: asBool(map['ok']) ?? false,
    label: asString(map['label']) ?? '',
    detail: asString(map['detail']),
  );
}

/// `pet.preflight`'s answer: the two checklists, v2's ready lines and the verdict.
class PetPreflightResult {
  const PetPreflightResult({
    required this.ready,
    required this.calibration,
    required this.session,
    required this.lines,
  });

  /// True only when no readiness line fires AND the calibration is complete (the core's rule).
  final bool ready;

  /// The calibration marks the session needs (Calibrate Pet's checklist).
  final List<PetChecklistItem> calibration;

  /// The session setup facts (v2's "Setup so far" for this tab).
  final List<PetChecklistItem> session;

  /// v2's exact Ready-to-run lines: a ready sentence, or the reasons Start would refuse/fall back.
  final List<String> lines;

  factory PetPreflightResult.fromJson(JsonMap map) => PetPreflightResult(
    ready: asBool(map['ready']) ?? false,
    calibration: [
      for (final entry in asJsonList(map['calibration']))
        PetChecklistItem.fromJson(asJsonMap(entry)),
    ],
    session: [
      for (final entry in asJsonList(map['session']))
        PetChecklistItem.fromJson(asJsonMap(entry)),
    ],
    lines: asStringList(map['lines']),
  );
}

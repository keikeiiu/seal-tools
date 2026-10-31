/// The `calibration.save`/`calibration.fetch` results, decoded app-locally (the shell alone consumes
/// the mark/derived/artifact shapes; the wire envelopes have golden fixtures —
/// `response.ok.calibration_save*` / `response.ok.calibration_fetch*` — and these decoders exist
/// because the payloads are presentation-only rather than typed app state).
///
/// The units are the calibration contract's ([CALIBRATION.md] §1): every mark is CLIENT-RELATIVE
/// PHYSICAL pixels, drawn on the captured full-client frame. The core owns the derived pitch and the
/// artifact crop; this file only names their shapes.
///
/// [CALIBRATION.md]: ../../../docs/spec/CALIBRATION.md
library;

import '../protocol/protocol.dart';
import 'config_json.dart';

/// A stored or saved mark: `[x, y, w, h]` for a rect kind, `[x, y]` for a point kind.
class CalibrationMark {
  const CalibrationMark.rect(this.x, this.y, this.w, this.h) : isRect = true;

  const CalibrationMark.point(this.x, this.y) : isRect = false, w = 0, h = 0;

  final bool isRect;
  final int x;
  final int y;
  final int w;
  final int h;

  /// The wire form: a rect array or a point array.
  List<int> get values => isRect ? [x, y, w, h] : [x, y];

  /// Decodes a mark, or null when the kind is not calibrated yet.
  static CalibrationMark? fromJson(Object? value) {
    if (value is! List) {
      return null;
    }
    final numbers = [
      for (final entry in value)
        if (entry is num) entry.round(),
    ];
    if (numbers.length == 4) {
      return CalibrationMark.rect(
        numbers[0],
        numbers[1],
        numbers[2],
        numbers[3],
      );
    }
    if (numbers.length == 2) {
      return CalibrationMark.point(numbers[0], numbers[1]);
    }
    return null;
  }

  /// `[x, y]` or `[x, y, w, h]` for the read-only lines.
  String get label => values.join(', ');
}

/// The crop a save wrote (`calib_gem_result.png` and any future artifact kinds).
class CalibrationArtifact {
  const CalibrationArtifact({
    required this.file,
    this.path,
    required this.width,
    required this.height,
  });

  final String file;

  /// Where the core wrote it, for the save result's diagnostic line.
  final String? path;
  final int width;
  final int height;

  factory CalibrationArtifact.fromJson(JsonMap map) => CalibrationArtifact(
    file: asString(map['file']) ?? '?',
    path: asString(map['path']),
    width: asNumber(map['width'])?.round() ?? 0,
    height: asNumber(map['height'])?.round() ?? 0,
  );
}

/// `calibration.save`'s answer.
class CalibrationSaveResult {
  const CalibrationSaveResult({
    required this.rev,
    required this.kind,
    this.target,
    required this.mark,
    this.pitchW,
    this.pitchH,
    this.artifact,
    this.calibration,
  });

  final int rev;
  final String kind;

  /// The named instance a targeted save echoed (e.g. `2`), null for a non-targeted kind.
  final String? target;
  final CalibrationMark mark;

  /// `derived.pitch` — present for `bag_grid` only.
  final double? pitchW;
  final double? pitchH;

  final CalibrationArtifact? artifact;

  /// The core's `calibration` environment echo (dpi_scale/monitor_dpi/screen/client_size/measured_at)
  /// written with this save. G07: the shell renders THIS instead of re-deriving it from the capture.
  final JsonMap? calibration;

  factory CalibrationSaveResult.fromJson(JsonMap map) {
    final derived = asJsonMap(map['derived']);
    final pitch = asJsonMap(derived['pitch']);
    final artifact = map['artifact'];
    return CalibrationSaveResult(
      rev: asNumber(map['rev'])?.round() ?? 0,
      kind: asString(map['kind']) ?? '?',
      target: asString(map['target']),
      mark:
          CalibrationMark.fromJson(map['mark']) ??
          const CalibrationMark.rect(0, 0, 0, 0),
      pitchW: asNumber(pitch['w'])?.toDouble(),
      pitchH: asNumber(pitch['h'])?.toDouble(),
      artifact: artifact is Map<String, dynamic>
          ? CalibrationArtifact.fromJson(artifact)
          : null,
      calibration: map['calibration'] is Map
          ? asJsonMap(map['calibration'])
          : null,
    );
  }

  String? get pitchLine => pitchW == null || pitchH == null
      ? null
      : 'derived pitch ${pitchW!.toStringAsFixed(2)} × ${pitchH!.toStringAsFixed(2)} px '
            '(per bag cell, 8×8 grid)';
}

/// The stored crop `calibration.fetch` returns as base64 PNG.
class CalibrationCrop {
  const CalibrationCrop({required this.file, required this.pngBase64});

  final String file;
  final String pngBase64;

  factory CalibrationCrop.fromJson(JsonMap map) => CalibrationCrop(
    file: asString(map['file']) ?? '?',
    pngBase64: asString(map['png_base64']) ?? '',
  );
}

/// `calibration.fetch`'s answer. `mark` is null until the kind is calibrated.
class CalibrationFetchResult {
  const CalibrationFetchResult({
    required this.kind,
    this.target,
    required this.mark,
    this.artifact,
    required this.rev,
  });

  final String kind;

  /// The named instance the fetch echoed, when the kind is targeted.
  final String? target;
  final CalibrationMark? mark;
  final CalibrationCrop? artifact;
  final int rev;

  factory CalibrationFetchResult.fromJson(JsonMap map) {
    final artifact = map['artifact'];
    return CalibrationFetchResult(
      kind: asString(map['kind']) ?? '?',
      target: asString(map['target']),
      mark: CalibrationMark.fromJson(map['mark']),
      artifact: artifact is Map<String, dynamic>
          ? CalibrationCrop.fromJson(artifact)
          : null,
      rev: asNumber(map['rev'])?.round() ?? 0,
    );
  }

  String get storedLine => mark == null
      ? 'Not calibrated yet'
      : 'Stored mark [${mark!.label}] (rev $rev)';
}

/// The `display` subset a save carries so the core records the measured environment.
///
/// The protocol's calibration environment uses v2's key names (`dpi_scale`, `monitor_dpi`, `screen`),
/// which are NOT `DisplayInfo`'s shell-side names — the mapping lives here, once.
JsonMap calibrationDisplayJson(DisplayInfo display) => {
  'dpi_scale': display.scale,
  'monitor_dpi': display.monitorDpi,
  'screen': display.screen.toJson(),
};

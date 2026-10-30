/// The typed payloads the shell needs now, decoded from [PROTOCOL.md] §3–§5.
///
/// These are only the shapes the shell actually reads. Every one keeps its JSON key names verbatim
/// (the drift test in `test/protocol/fixtures_test.dart` compares the key sets recursively against
/// the golden fixtures), and none of them models a config BLOCK: config sections travel as
/// [JsonMap], because the shell copies what it does not own.
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'envelope.dart';
import 'json.dart';

// ── §14? no: §3 — geometry and images ─────────────────────────────────────────────────────────

/// An integer pixel rectangle ([PROTOCOL.md] §3). Physical screen pixels unless its field name says
/// otherwise; which space a rect is in is its producer's contract.
class Rect implements WirePayload {
  const Rect({
    required this.x,
    required this.y,
    required this.w,
    required this.h,
  });

  final int x;
  final int y;
  final int w;
  final int h;

  factory Rect.fromJson(Object? value, String path) {
    final map = decodeMap(value, path);
    return Rect(
      x: decodeInt(map['x'], '$path.x'),
      y: decodeInt(map['y'], '$path.y'),
      w: decodeInt(map['w'], '$path.w'),
      h: decodeInt(map['h'], '$path.h'),
    );
  }

  @override
  JsonMap toJson() => {'x': x, 'y': y, 'w': w, 'h': h};
}

/// A width and height with no origin.
class Size implements WirePayload {
  const Size({required this.w, required this.h});

  final int w;
  final int h;

  factory Size.fromJson(Object? value, String path) {
    final map = decodeMap(value, path);
    return Size(
      w: decodeInt(map['w'], '$path.w'),
      h: decodeInt(map['h'], '$path.h'),
    );
  }

  @override
  JsonMap toJson() => {'w': w, 'h': h};
}

/// A point in one of the two spaces ([PROTOCOL.md] §3; the field that carries it says which).
class Point implements WirePayload {
  const Point({required this.x, required this.y});

  final int x;
  final int y;

  factory Point.fromJson(Object? value, String path) {
    final map = decodeMap(value, path);
    return Point(
      x: decodeInt(map['x'], '$path.x'),
      y: decodeInt(map['y'], '$path.y'),
    );
  }

  @override
  JsonMap toJson() => {'x': x, 'y': y};
}

/// What the core measured; the shell uses ONLY this to map frames to coordinates
/// ([PROTOCOL.md] §3, [CALIBRATION.md] §4). It never asks Win32 for a mapping.
///
/// [CALIBRATION.md]: ../../../docs/spec/CALIBRATION.md
class DisplayInfo implements WirePayload {
  const DisplayInfo({
    required this.scale,
    required this.monitorDpi,
    required this.screen,
    required this.frame,
    required this.physicalClient,
    required this.logicalClient,
  });

  /// Physical px per LOGICAL px, measured on this machine, never cached.
  final double scale;

  /// Effective monitor DPI, `96 * scale`, shown as a cross-check only.
  final int monitorDpi;

  /// The primary screen, physical pixels.
  final Size screen;

  /// The window frame, physical; diagnostic only, never a coordinate origin.
  final Rect frame;

  /// The client area, physical; the canonical origin of every calibrated coordinate.
  final Rect physicalClient;

  /// The client area as the DPI-unaware core process sees it.
  final Rect logicalClient;

  factory DisplayInfo.fromJson(Object? value, String path) {
    final map = decodeMap(value, path);
    return DisplayInfo(
      scale: decodeDouble(map['scale'], '$path.scale'),
      monitorDpi: decodeInt(map['monitor_dpi'], '$path.monitor_dpi'),
      screen: Size.fromJson(map['screen'], '$path.screen'),
      frame: Rect.fromJson(map['frame'], '$path.frame'),
      physicalClient: Rect.fromJson(
        map['physical_client'],
        '$path.physical_client',
      ),
      logicalClient: Rect.fromJson(
        map['logical_client'],
        '$path.logical_client',
      ),
    );
  }

  @override
  JsonMap toJson() => {
    'scale': scale,
    'monitor_dpi': monitorDpi,
    'screen': screen.toJson(),
    'frame': frame.toJson(),
    'physical_client': physicalClient.toJson(),
    'logical_client': logicalClient.toJson(),
  };
}

/// One captured frame ([PROTOCOL.md] §3/§8). `origin` is the frame's top-left in physical screen
/// pixels, so the shell never has to guess where the image came from.
class Frame implements WirePayload {
  const Frame({
    required this.format,
    required this.width,
    required this.height,
    required this.origin,
    required this.data,
    required this.frameId,
  });

  /// `png` for now; `bgra` is a later, measured option.
  final String format;
  final int width;
  final int height;
  final Point origin;

  /// Base64 image data.
  final String data;

  /// Retained by the core's cache; `calibration.save` references it.
  final String frameId;

  factory Frame.fromJson(Object? value, String path) {
    final map = decodeMap(value, path);
    return Frame(
      format: decodeString(map['format'], '$path.format'),
      width: decodeInt(map['width'], '$path.width'),
      height: decodeInt(map['height'], '$path.height'),
      origin: Point.fromJson(map['origin'], '$path.origin'),
      data: decodeString(map['data'], '$path.data'),
      frameId: decodeString(map['frame_id'], '$path.frame_id'),
    );
  }

  @override
  JsonMap toJson() => {
    'format': format,
    'width': width,
    'height': height,
    'origin': origin.toJson(),
    'data': data,
    'frame_id': frameId,
  };
}

/// The `capture` result: the image and the measurement that maps it ([PROTOCOL.md] §4).
class CaptureResult implements WirePayload {
  const CaptureResult({required this.frame, required this.display});

  final Frame frame;
  final DisplayInfo display;

  factory CaptureResult.fromJson(JsonMap map) => CaptureResult(
    frame: Frame.fromJson(map['frame'], 'result.frame'),
    display: DisplayInfo.fromJson(map['display'], 'result.display'),
  );

  @override
  JsonMap toJson() => {'frame': frame.toJson(), 'display': display.toJson()};
}

// ── tool ids, phases and stop reasons ([PROTOCOL.md] §9, §5) ───────────────────────────────

/// The eight tool ids ([PROTOCOL.md] §9). Exact wire values, untrimmed: this is not player input.
enum ToolId {
  spammer('spammer'),
  holdspace('holdspace'),
  buy('buy'),
  sell('sell'),
  tuner('tuner'),
  gem('gem'),
  quest('quest'),
  pet('pet');

  const ToolId(this.wire);

  final String wire;

  /// Parses a wire id; an unknown id is a protocol violation (the core accepted it, so the two
  /// sides disagree about §9 — worth failing loudly during the port).
  static ToolId parse(String wire) {
    for (final tool in values) {
      if (tool.wire == wire) {
        return tool;
      }
    }
    throw ProtocolDecodeException(
      'unknown tool id',
      expected: 'one of §9',
      found: wire,
    );
  }

  /// The pet feeder is the one resident tool.
  bool get isResident => this == ToolId.pet;
}

/// The lifecycle phase ([PROTOCOL.md] §5's `phase`).
enum ToolPhase {
  idle('idle'),
  starting('starting'),
  running('running'),
  stopping('stopping');

  const ToolPhase(this.wire);

  final String wire;

  static ToolPhase parse(String wire) {
    for (final phase in values) {
      if (phase.wire == wire) {
        return phase;
      }
    }
    throw ProtocolDecodeException(
      'unknown tool phase',
      expected: 'idle|starting|running|stopping',
      found: wire,
    );
  }
}

/// Why a run ended ([PROTOCOL.md] §5 `tool.finished.reason`).
enum ToolStopKind {
  done('done'),
  cancelled('cancelled'),
  error('error'),
  guard('guard');

  const ToolStopKind(this.wire);

  final String wire;

  static ToolStopKind parse(String wire) {
    for (final kind in values) {
      if (kind.wire == wire) {
        return kind;
      }
    }
    throw ProtocolDecodeException(
      'unknown stop reason',
      expected: 'done|cancelled|error|guard',
      found: wire,
    );
  }
}

/// One row of a tool's standing plan — the pinned shape of [PROTOCOL.md] §5's `state.rows`.
class ToolRow implements WirePayload {
  const ToolRow({
    required this.name,
    required this.status,
    this.detail,
    this.nextActionInSeconds,
  });

  /// The row's label on the card.
  final String name;

  /// The short state word the card shows (`boarding`, `waiting for a reload`, `given up`).
  final String status;

  /// What the row is doing, when there is more to say.
  final String? detail;

  /// Seconds from the publish until this row next needs attention; null when never again.
  final double? nextActionInSeconds;

  factory ToolRow.fromJson(Object? value, String path) {
    final map = decodeMap(value, path);
    return ToolRow(
      name: decodeString(map['name'], '$path.name'),
      status: decodeString(map['status'], '$path.status'),
      detail: decodeNullableString(map['detail'], '$path.detail'),
      nextActionInSeconds: decodeNullableDouble(
        map['next_action_in_seconds'],
        '$path.next_action_in_seconds',
      ),
    );
  }

  @override
  JsonMap toJson() => {
    'name': name,
    'status': status,
    'detail': detail,
    'next_action_in_seconds': nextActionInSeconds,
  };
}

/// One labelled fact on a running card ([PROTOCOL.md] §5 `state.detail`). The core owns both the
/// label and the rendered value; a null label is v2's label-less `name=value` attribute line.
class ToolDetail implements WirePayload {
  const ToolDetail({this.label, required this.value});

  final String? label;
  final String value;

  factory ToolDetail.fromJson(Object? value, String path) {
    final map = decodeMap(value, path);
    return ToolDetail(
      label: decodeNullableString(map['label'], '$path.label'),
      value: decodeString(map['value'], '$path.value'),
    );
  }

  @override
  JsonMap toJson() => {'label': label, 'value': value};
}

/// Everything the `state` event carries ([PROTOCOL.md] §5) — exactly the fields of the core's
/// immutable snapshot, so the card renders one value rather than assembling one.
class ToolSnapshot implements WirePayload {
  const ToolSnapshot({
    required this.tool,
    required this.running,
    required this.phase,
    required this.message,
    this.countdownSeconds,
    this.rows = const [],
    this.detail = const [],
    this.error,
  });

  final ToolId tool;
  final bool running;
  final ToolPhase phase;

  /// The last thing that happened, in the tool's own words.
  final String message;

  /// Seconds until the next action, counted down by the shell from the publish.
  final double? countdownSeconds;

  /// The row lines; `[]` means "no rows to show", never "not known yet".
  final List<ToolRow> rows;

  /// The running card's labelled facts; `[]` means "no detail to show".
  final List<ToolDetail> detail;

  /// A failure worth keeping on the card.
  final String? error;

  factory ToolSnapshot.fromJson(JsonMap map) => ToolSnapshot(
    tool: ToolId.parse(decodeString(map['tool'], 'tool')),
    running: decodeBool(map['running'], 'running'),
    phase: ToolPhase.parse(decodeString(map['phase'], 'phase')),
    message: decodeString(map['message'], 'message'),
    countdownSeconds: decodeNullableDouble(map['countdown_s'], 'countdown_s'),
    rows: decodeList(map['rows'], 'rows', ToolRow.fromJson),
    detail: map['detail'] == null
        ? const []
        : decodeList(map['detail'], 'detail', ToolDetail.fromJson),
    error: decodeNullableString(map['error'], 'error'),
  );

  @override
  JsonMap toJson() => {
    'tool': tool.wire,
    'running': running,
    'phase': phase.wire,
    'message': message,
    'countdown_s': countdownSeconds,
    'rows': [for (final row in rows) row.toJson()],
    'detail': [for (final line in detail) line.toJson()],
    'error': error,
  };
}

/// The `tool.finished` payload ([PROTOCOL.md] §5); `message` is the exact line the card shows.
class ToolFinished implements WirePayload {
  const ToolFinished({
    required this.tool,
    required this.reason,
    required this.message,
  });

  final ToolId tool;
  final ToolStopKind reason;
  final String message;

  factory ToolFinished.fromJson(JsonMap map) => ToolFinished(
    tool: ToolId.parse(decodeString(map['tool'], 'tool')),
    reason: ToolStopKind.parse(decodeString(map['reason'], 'reason')),
    message: decodeString(map['message'], 'message'),
  );

  @override
  JsonMap toJson() => {
    'tool': tool.wire,
    'reason': reason.wire,
    'message': message,
  };
}

/// A `log` event ([PROTOCOL.md] §5).
enum LogLevel {
  info('info'),
  warn('warn'),
  error('error');

  const LogLevel(this.wire);

  final String wire;

  static LogLevel parse(String wire) {
    for (final level in values) {
      if (level.wire == wire) {
        return level;
      }
    }
    throw ProtocolDecodeException(
      'unknown log level',
      expected: 'info|warn|error',
      found: wire,
    );
  }
}

/// One log line the shell may show.
class LogLine implements WirePayload {
  const LogLine({required this.level, required this.line});

  final LogLevel level;
  final String line;

  factory LogLine.fromJson(JsonMap map) => LogLine(
    level: LogLevel.parse(decodeString(map['level'], 'level')),
    line: decodeString(map['line'], 'line'),
  );

  @override
  JsonMap toJson() => {'level': level.wire, 'line': line};
}

/// Why the core is stopping ([PROTOCOL.md] §5; `fatal` has no producer yet and is kept because the
/// spec lists it).
enum CoreStoppingReason {
  eof('eof'),
  shutdown('shutdown'),
  fatal('fatal');

  const CoreStoppingReason(this.wire);

  final String wire;

  static CoreStoppingReason parse(String wire) {
    for (final reason in values) {
      if (reason.wire == wire) {
        return reason;
      }
    }
    throw ProtocolDecodeException(
      'unknown core stopping reason',
      expected: 'eof|shutdown|fatal',
      found: wire,
    );
  }
}

/// The `core.stopping` payload ([PROTOCOL.md] §5).
class CoreStopping implements WirePayload {
  const CoreStopping({required this.reason});

  final CoreStoppingReason reason;

  factory CoreStopping.fromJson(JsonMap map) => CoreStopping(
    reason: CoreStoppingReason.parse(decodeString(map['reason'], 'reason')),
  );

  @override
  JsonMap toJson() => {'reason': reason.wire};
}

/// The `config.changed` payload ([PROTOCOL.md] §5) — specified, but the core has no emitter yet;
/// the type exists so the fixture decodes and the shell can wire it the day it is emitted.
class ConfigChanged implements WirePayload {
  const ConfigChanged({
    required this.rev,
    required this.block,
    required this.values,
  });

  final int rev;
  final String block;

  /// The block's new values; opaque, like all config in the shell.
  final JsonMap values;

  factory ConfigChanged.fromJson(JsonMap map) => ConfigChanged(
    rev: decodeInt(map['rev'], 'rev'),
    block: decodeString(map['block'], 'block'),
    values: decodeMap(map['values'], 'values'),
  );

  @override
  JsonMap toJson() => {'rev': rev, 'block': block, 'values': values};
}

// ── config command results ([PROTOCOL.md] §4/§7) ──────────────────────────────────────────

/// The `hello` result ([PROTOCOL.md] §4): the handshake's whole payload.
class HelloResult implements WirePayload {
  const HelloResult({
    required this.protocol,
    required this.coreVersion,
    required this.root,
    required this.rev,
    required this.config,
  });

  final int protocol;
  final String coreVersion;

  /// The application root the core bound to — where `config/` and `logs/` live.
  final String root;

  /// The session revision the shell echoes back on every `config.apply`.
  final int rev;

  /// The merged config, opaque. The shell mirrors only the blocks it edits.
  final JsonMap config;

  factory HelloResult.fromJson(JsonMap map) => HelloResult(
    protocol: decodeInt(map['protocol'], 'protocol'),
    coreVersion: decodeString(map['core_version'], 'core_version'),
    root: decodeString(map['root'], 'root'),
    rev: decodeInt(map['rev'], 'rev'),
    config: decodeMap(map['config'], 'config'),
  );

  @override
  JsonMap toJson() => {
    'protocol': protocol,
    'core_version': coreVersion,
    'root': root,
    'rev': rev,
    'config': config,
  };
}

/// The `config.apply` result: the new revision and the effective sections it echoed back
/// ([PROTOCOL.md] §4).
class ConfigApplyResult implements WirePayload {
  const ConfigApplyResult({required this.rev, required this.sections});

  final int rev;

  /// The effective sections for the names the request asked about; opaque.
  final JsonMap sections;

  factory ConfigApplyResult.fromJson(JsonMap map) => ConfigApplyResult(
    rev: decodeInt(map['rev'], 'rev'),
    sections: decodeMap(map['sections'], 'sections'),
  );

  @override
  JsonMap toJson() => {'rev': rev, 'sections': sections};
}

/// The `config.reload` result: the new revision and the whole merged config ([PROTOCOL.md] §4).
class ConfigReloadResult implements WirePayload {
  const ConfigReloadResult({required this.rev, required this.config});

  final int rev;
  final JsonMap config;

  factory ConfigReloadResult.fromJson(JsonMap map) => ConfigReloadResult(
    rev: decodeInt(map['rev'], 'rev'),
    config: decodeMap(map['config'], 'config'),
  );

  @override
  JsonMap toJson() => {'rev': rev, 'config': config};
}

/// The `config.validate` result ([PROTOCOL.md] §4): the player-facing lines a save would refuse.
class ConfigValidateResult implements WirePayload {
  const ConfigValidateResult({required this.errors, required this.warnings});

  final List<String> errors;
  final List<String> warnings;

  factory ConfigValidateResult.fromJson(JsonMap map) => ConfigValidateResult(
    errors: decodeStringList(map['errors'], 'errors'),
    warnings: decodeStringList(map['warnings'], 'warnings'),
  );

  @override
  JsonMap toJson() => {'errors': errors, 'warnings': warnings};

  /// True when nothing would refuse the save.
  bool get isClean => errors.isEmpty;
}

// ── other command results the fixtures pin ([PROTOCOL.md] §4) ─────────────────────────────

/// The `ocr.scan` result ([PROTOCOL.md] §4). Spec-only for now (the reader crate does not exist in
/// the shell's reach yet), decoded because the fixture set is the protocol's guard.
class OcrScanResult implements WirePayload {
  const OcrScanResult({
    required this.lines,
    required this.attributes,
    this.grade,
    this.remaining,
    this.capturePath,
  });

  final List<String> lines;

  /// Name/attribute pairs, e.g. `[["攻擊力", "atk"]]`.
  final List<List<String>> attributes;

  final String? grade;
  final String? remaining;
  final String? capturePath;

  factory OcrScanResult.fromJson(JsonMap map) => OcrScanResult(
    lines: decodeStringList(map['lines'], 'lines'),
    attributes: decodeStringPairs(map['attributes'], 'attributes'),
    grade: decodeNullableString(map['grade'], 'grade'),
    remaining: decodeNullableString(map['remaining'], 'remaining'),
    capturePath: decodeNullableString(map['capture_path'], 'capture_path'),
  );

  @override
  JsonMap toJson() => {
    'lines': lines,
    'attributes': attributes,
    'grade': grade,
    'remaining': remaining,
    'capture_path': capturePath,
  };
}

/// A successful result with no payload — `shutdown`, and `config.validate` before any rules ran
/// ([PROTOCOL.md] §4's `{}` shapes).
class EmptyResult implements WirePayload {
  const EmptyResult();

  factory EmptyResult.fromJson(JsonMap map) => const EmptyResult();

  @override
  JsonMap toJson() => const {};
}

/// `board.details`'s answer, decoded APP-LOCALLY (the shell alone consumes it; the wire type is the
/// untyped envelope, PROTOCOL §4/§10 — the golden fixture pins the untyped shape).
///
/// The shape is the core's serde names (`seal-core/src/board.rs::details`):
/// `{baud, expected: {vid, pids, port}, ports: [{port, vid, pid, product, manufacturer,
/// matches_ids, chosen}], choice: {state, port, refusal}, firmware: null | {level}}`.
///
/// The CORE owns the rule and the refusal sentence (`choice.refusal`); the shell owns the three
/// verdict/advice lines it renders from `choice.state` (v2's wording, see the Arduino tab). Nothing
/// here opens the port: `board.details` is enumeration + the cached firmware level only.
library;

import '../protocol/protocol.dart';
import 'config_json.dart';

/// The IDs the launcher looks for and its port override — DATA only; the core decides what wins.
class BoardExpected {
  const BoardExpected({required this.vid, required this.pids, this.port});

  /// Lower-case hex, e.g. `2341`.
  final String vid;

  /// Lower-case hex PIDs, e.g. `['8036', '8037']`.
  final List<String> pids;

  /// `arduino.port`, or null when the launcher finds the board by IDs.
  final String? port;

  factory BoardExpected.fromJson(JsonMap map) => BoardExpected(
    vid: asString(map['vid']) ?? '',
    pids: asStringList(map['pids']),
    port: asString(map['port']),
  );

  /// True when a named port overrides discovery.
  bool get hasPortOverride => port != null && port!.trim().isNotEmpty;
}

/// One serial port the machine currently enumerates.
class BoardPort {
  const BoardPort({
    required this.port,
    this.vid,
    this.pid,
    this.product,
    this.manufacturer,
    required this.matchesIds,
    required this.chosen,
  });

  final String port;
  final String? vid;
  final String? pid;
  final String? product;
  final String? manufacturer;

  /// The port's IDs are one of the launcher's expected ones.
  final bool matchesIds;

  /// The port a Start would open right now (the CHOICE, not merely a matching ID).
  final bool chosen;

  factory BoardPort.fromJson(JsonMap map) => BoardPort(
    port: asString(map['port']) ?? '',
    vid: asString(map['vid']),
    pid: asString(map['pid']),
    product: asString(map['product']),
    manufacturer: asString(map['manufacturer']),
    matchesIds: asBool(map['matches_ids']) ?? false,
    chosen: asBool(map['chosen']) ?? false,
  );

  /// `product` when the driver reported one, else the manufacturer, else null.
  String? get description {
    final product = this.product;
    if (product != null && product.trim().isNotEmpty) {
      return product;
    }
    final manufacturer = this.manufacturer;
    if (manufacturer != null && manufacturer.trim().isNotEmpty) {
      return manufacturer;
    }
    return null;
  }
}

/// What a Start would do, as the core decided it.
enum BoardChoiceState {
  /// A port is chosen: Start will open it.
  ready('ready'),

  /// `arduino.port` names a port this machine does not have: a refusal, not an absence.
  namedMissing('named_missing'),

  /// No port matches the expected IDs.
  notFound('not_found');

  const BoardChoiceState(this.wire);

  final String wire;

  /// An ADDITIVE state a newer core adds is treated as `notFound` until the shell learns it: the
  /// verdict stays honest (Start would not have a board this build can name) instead of throwing on
  /// a protocol addition.
  static BoardChoiceState parse(String? wire) {
    for (final state in values) {
      if (state.wire == wire) {
        return state;
      }
    }
    return BoardChoiceState.notFound;
  }
}

/// `choice`: what Start would do, why not, and the core's own sentence when it refuses.
class BoardChoice {
  const BoardChoice({required this.state, this.port, this.refusal});

  final BoardChoiceState state;

  /// The CHOSEN port when ready; the NAMED port when it is missing; null when none was found.
  final String? port;

  /// The core's refusal sentence, verbatim (null when ready).
  final String? refusal;

  factory BoardChoice.fromJson(JsonMap map) => BoardChoice(
    state: BoardChoiceState.parse(asString(map['state'])),
    port: asString(map['port']),
    refusal: asString(map['refusal']),
  );
}

/// The CACHED firmware level: null in [BoardDetailsResult.firmware] means "not checked yet".
class BoardFirmware {
  const BoardFirmware({required this.level});

  /// The protocol level the board reported (`V`), never a build counter.
  final int level;

  factory BoardFirmware.fromJson(JsonMap map) =>
      BoardFirmware(level: asNumber(map['level'])?.round() ?? 0);
}

/// `board.details`'s answer.
class BoardDetailsResult {
  const BoardDetailsResult({
    required this.baud,
    required this.expected,
    required this.ports,
    required this.choice,
    this.firmware,
  });

  final int baud;
  final BoardExpected expected;

  /// The enumerated ports, CHOSEN FIRST then enumeration order (the core's order, rendered as-is).
  final List<BoardPort> ports;

  final BoardChoice choice;

  /// The core's cached level, or null when the board has not been read this session.
  final BoardFirmware? firmware;

  factory BoardDetailsResult.fromJson(JsonMap map) {
    final rawFirmware = map['firmware'];
    return BoardDetailsResult(
      baud: asNumber(map['baud'])?.round() ?? 0,
      expected: BoardExpected.fromJson(asJsonMap(map['expected'])),
      ports: [
        for (final entry in asJsonList(map['ports']))
          BoardPort.fromJson(asJsonMap(entry)),
      ],
      choice: BoardChoice.fromJson(asJsonMap(map['choice'])),
      firmware: rawFirmware is Map
          ? BoardFirmware.fromJson(asJsonMap(rawFirmware))
          : null,
    );
  }
}

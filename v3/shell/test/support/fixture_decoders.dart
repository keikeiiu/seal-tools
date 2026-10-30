/// The explicit fixture-case → decoder map.
///
/// There is one entry per fixture file, and `fixtures_test.dart` asserts both directions: a fixture
/// with no entry fails, and an entry with no fixture fails. That is what makes the fixture set a
/// CROSS-LANGUAGE guard rather than a sample: the Rust side cannot add a shape without the Dart side
/// failing here until it is decoded on purpose.
///
/// Requests decode to [Request] plus, for the commands the shell sends, their typed args so the args
/// shape is checked too. Responses decode to their typed payload. Events decode to the sealed
/// [ProtocolEvent] family.
library;

import 'package:seal_shell/protocol/protocol.dart';

/// A fixture decoded into its typed model, tagged with its case name.
sealed class DecodedFixture {
  const DecodedFixture(this.caseName);

  final String caseName;
}

/// A request fixture: the envelope, and the typed args when the shell has a type for that command.
final class DecodedRequestFixture extends DecodedFixture {
  const DecodedRequestFixture(super.caseName, this.request, {this.typedArgs});

  final Request request;
  final WirePayload? typedArgs;
}

/// A successful response fixture: its id and its typed result payload.
final class DecodedOkFixture extends DecodedFixture {
  const DecodedOkFixture(super.caseName, this.id, this.result);

  final String id;
  final WirePayload result;
}

/// An error response fixture.
final class DecodedErrorFixture extends DecodedFixture {
  const DecodedErrorFixture(super.caseName, this.id, this.error);

  final String id;

  final ErrorObject error;
}

/// An event fixture: the typed event.
final class DecodedEventFixture extends DecodedFixture {
  const DecodedEventFixture(super.caseName, this.event);

  final ProtocolEvent event;
}

/// Decodes one fixture message.
typedef FixtureDecoder = DecodedFixture Function(JsonMap message);

JsonMap _result(JsonMap message) => decodeMap(message['result'], 'result');
String _id(JsonMap message) => decodeString(message['id'], 'id');
JsonMap _args(JsonMap message) => decodeMap(message['args'], 'args');

DecodedRequestFixture _request(
  String caseName,
  JsonMap message, {
  WirePayload? typedArgs,
}) => DecodedRequestFixture(
  caseName,
  Request.fromJson(message),
  typedArgs: typedArgs,
);

DecodedOkFixture _ok(String caseName, JsonMap message, WirePayload result) =>
    DecodedOkFixture(caseName, _id(message), result);

DecodedErrorFixture _error(String caseName, JsonMap message) =>
    DecodedErrorFixture(
      caseName,
      _id(message),
      ErrorObject.fromJson(decodeMap(message['error'], 'error')),
    );

DecodedEventFixture _event(
  String caseName,
  JsonMap message,
  ProtocolEvent event,
) => DecodedEventFixture(caseName, event);

/// A result whose typed model lives app-locally, not in `lib/protocol`: the test still checks that
/// the envelope and the exact key shape decode from the fixture, without inventing a wire type.
final class EnvelopePayload implements WirePayload {
  const EnvelopePayload(this.value);

  final JsonMap value;

  @override
  JsonMap toJson() => value;
}

/// The 45 request fixtures.
final Map<String, FixtureDecoder> requestDecoders = {
  'request.board.probe': (message) => _request('request.board.probe', message),
  'request.board.details': (message) =>
      _request('request.board.details', message),
  'request.pet.preflight': (message) =>
      _request('request.pet.preflight', message),
  'request.pet.set_boarding': (message) =>
      _request('request.pet.set_boarding', message),
  'request.pet.capture_icon.queue': (message) =>
      _request('request.pet.capture_icon.queue', message),
  'request.pet.capture_icon.food': (message) =>
      _request('request.pet.capture_icon.food', message),
  'request.pet.scan_food_page': (message) =>
      _request('request.pet.scan_food_page', message),
  'request.pet.scan_queue_page': (message) =>
      _request('request.pet.scan_queue_page', message),
  'request.pet.test_read_panel': (message) =>
      _request('request.pet.test_read_panel', message),
  'request.pet.apply_food_scan': (message) =>
      _request('request.pet.apply_food_scan', message),
  'request.board.test.click': (message) =>
      _request('request.board.test.click', message),
  'request.board.test.key': (message) =>
      _request('request.board.test.key', message),
  'request.board.test.move': (message) =>
      _request('request.board.test.move', message),
  'request.calibration.fetch': (message) =>
      _request('request.calibration.fetch', message),
  'request.calibration.save': (message) =>
      _request('request.calibration.save', message),
  'request.calibration.save.with_rev': (message) =>
      _request('request.calibration.save.with_rev', message),
  'request.calibration.fetch_all': (message) =>
      _request('request.calibration.fetch_all', message),
  'request.calibration.kinds': (message) =>
      _request('request.calibration.kinds', message),
  'request.calibration.check': (message) =>
      _request('request.calibration.check', message),
  'request.calibration.tooltip.hover_capture': (message) =>
      _request('request.calibration.tooltip.hover_capture', message),
  'request.calibration.tooltip.save': (message) =>
      _request('request.calibration.tooltip.save', message),
  'request.calibration.tooltip.test': (message) =>
      _request('request.calibration.tooltip.test', message),
  'request.calibration.fetch.targeted': (message) =>
      _request('request.calibration.fetch.targeted', message),
  'request.calibration.save.empty': (message) =>
      _request('request.calibration.save.empty', message),
  'request.calibration.save.targeted': (message) =>
      _request('request.calibration.save.targeted', message),
  'request.attributes.get': (message) =>
      _request('request.attributes.get', message),
  'request.keys.names': (message) => _request('request.keys.names', message),
  'request.keys.parse': (message) => _request('request.keys.parse', message),
  'request.capture': (message) {
    final args = _args(message);
    return _request(
      'request.capture',
      message,
      typedArgs: CaptureArgs(
        region: Rect.fromJson(args['region'], 'args.region'),
      ),
    );
  },
  'request.config.apply': (message) => _request(
    'request.config.apply',
    message,
    typedArgs: ConfigApplyArgs(
      rev: decodeInt(_args(message)['rev'], 'args.rev'),
      sections: decodeMap(_args(message)['sections'], 'args.sections'),
    ),
  ),
  'request.config.apply.mixed': (message) => _request(
    'request.config.apply.mixed',
    message,
    typedArgs: ConfigApplyArgs(
      rev: decodeInt(_args(message)['rev'], 'args.rev'),
      sections: decodeMap(_args(message)['sections'], 'args.sections'),
    ),
  ),
  'request.config.apply.settings': (message) => _request(
    'request.config.apply.settings',
    message,
    typedArgs: ConfigApplyArgs(
      rev: decodeInt(_args(message)['rev'], 'args.rev'),
      sections: decodeMap(_args(message)['sections'], 'args.sections'),
    ),
  ),
  'request.config.apply.variant': (message) => _request(
    'request.config.apply.variant',
    message,
    typedArgs: ConfigApplyArgs(
      rev: decodeInt(_args(message)['rev'], 'args.rev'),
      sections: decodeMap(_args(message)['sections'], 'args.sections'),
    ),
  ),
  'request.config.reload': (message) => _request(
    'request.config.reload',
    message,
    typedArgs: const ConfigReloadArgs(),
  ),
  'request.config.validate': (message) => _request(
    'request.config.validate',
    message,
    typedArgs: ConfigValidateArgs(
      sections: decodeMap(_args(message)['sections'], 'args.sections'),
    ),
  ),
  'request.hello': (message) => _request(
    'request.hello',
    message,
    typedArgs: HelloArgs(
      protocol: decodeInt(_args(message)['protocol'], 'args.protocol'),
      client: decodeString(_args(message)['client'], 'args.client'),
    ),
  ),
  'request.ocr.scan.attributes': (message) =>
      _request('request.ocr.scan.attributes', message),
  'request.ocr.scan.count': (message) =>
      _request('request.ocr.scan.count', message),
  'request.shutdown': (message) => _request('request.shutdown', message),
  'request.tool.start': (message) => _request(
    'request.tool.start',
    message,
    typedArgs: ToolStartArgs(
      tool: ToolId.parse(decodeString(_args(message)['tool'], 'args.tool')),
    ),
  ),
  'request.tool.stop.any': (message) => _request(
    'request.tool.stop.any',
    message,
    typedArgs: const ToolStopArgs(),
  ),
  'request.tool.stop.tool': (message) => _request(
    'request.tool.stop.tool',
    message,
    typedArgs: ToolStopArgs(
      tool: ToolId.parse(decodeString(_args(message)['tool'], 'args.tool')),
    ),
  ),
  'request.ui.window': (message) => _request(
    'request.ui.window',
    message,
    typedArgs: UiWindowArgs(
      left: decodeNum(_args(message)['left'], 'args.left'),
      top: decodeNum(_args(message)['top'], 'args.top'),
      width: decodeNum(_args(message)['width'], 'args.width'),
      height: decodeNum(_args(message)['height'], 'args.height'),
      pinned: decodeBool(_args(message)['pinned'], 'args.pinned'),
    ),
  ),
  'request.window.find': (message) => _request('request.window.find', message),
  'request.window.measure': (message) =>
      _request('request.window.measure', message),
};

/// The 38 response fixtures: 25 successes (9 typed, 16 envelope-only) and 13 errors.
final Map<String, FixtureDecoder> responseDecoders = {
  'response.ok.capture': (message) => _ok(
    'response.ok.capture',
    message,
    CaptureResult.fromJson(_result(message)),
  ),
  'response.ok.config_apply': (message) => _ok(
    'response.ok.config_apply',
    message,
    ConfigApplyResult.fromJson(_result(message)),
  ),
  'response.ok.config_apply.mixed': (message) => _ok(
    'response.ok.config_apply.mixed',
    message,
    ConfigApplyResult.fromJson(_result(message)),
  ),
  'response.ok.config_apply.settings': (message) => _ok(
    'response.ok.config_apply.settings',
    message,
    ConfigApplyResult.fromJson(_result(message)),
  ),
  'response.ok.config_apply.variant': (message) => _ok(
    'response.ok.config_apply.variant',
    message,
    ConfigApplyResult.fromJson(_result(message)),
  ),
  'response.ok.config_reload': (message) => _ok(
    'response.ok.config_reload',
    message,
    ConfigReloadResult.fromJson(_result(message)),
  ),
  'response.ok.empty': (message) =>
      _ok('response.ok.empty', message, EmptyResult.fromJson(_result(message))),
  'response.ok.board_details': (message) => _ok(
    'response.ok.board_details',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.pet_preflight': (message) => _ok(
    'response.ok.pet_preflight',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.pet_set_boarding': (message) => _ok(
    'response.ok.pet_set_boarding',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.pet_capture_icon.queue': (message) => _ok(
    'response.ok.pet_capture_icon.queue',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.pet_capture_icon.food': (message) => _ok(
    'response.ok.pet_capture_icon.food',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.pet_scan_food_page': (message) => _ok(
    'response.ok.pet_scan_food_page',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.pet_scan_queue_page': (message) => _ok(
    'response.ok.pet_scan_queue_page',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.pet_test_read_panel': (message) => _ok(
    'response.ok.pet_test_read_panel',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.pet_apply_food_scan': (message) => _ok(
    'response.ok.pet_apply_food_scan',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.calibration_save': (message) => _ok(
    'response.ok.calibration_save',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.calibration_save.targeted': (message) => _ok(
    'response.ok.calibration_save.targeted',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.calibration_save.empty': (message) => _ok(
    'response.ok.calibration_save.empty',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.calibration_fetch': (message) => _ok(
    'response.ok.calibration_fetch',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.calibration_fetch_all': (message) => _ok(
    'response.ok.calibration_fetch_all',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.calibration_kinds': (message) => _ok(
    'response.ok.calibration_kinds',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.calibration_check': (message) => _ok(
    'response.ok.calibration_check',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.tooltip_save': (message) => _ok(
    'response.ok.tooltip_save',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.tooltip_hover_capture': (message) => _ok(
    'response.ok.tooltip_hover_capture',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.tooltip_test': (message) => _ok(
    'response.ok.tooltip_test',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.calibration_fetch.targeted': (message) => _ok(
    'response.ok.calibration_fetch.targeted',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.keys_parse': (message) =>
      _ok('response.ok.keys_parse', message, EnvelopePayload(_result(message))),
  'response.ok.keys_names': (message) =>
      _ok('response.ok.keys_names', message, EnvelopePayload(_result(message))),
  'response.ok.attributes_get': (message) => _ok(
    'response.ok.attributes_get',
    message,
    EnvelopePayload(_result(message)),
  ),
  'response.ok.hello': (message) =>
      _ok('response.ok.hello', message, HelloResult.fromJson(_result(message))),
  'response.ok.ocr_scan': (message) => _ok(
    'response.ok.ocr_scan',
    message,
    OcrScanResult.fromJson(_result(message)),
  ),
  'response.error.bad_request': (message) =>
      _error('response.error.bad_request', message),
  'response.error.bad_version': (message) =>
      _error('response.error.bad_version', message),
  'response.error.board_unavailable': (message) =>
      _error('response.error.board_unavailable', message),
  'response.error.config_invalid': (message) =>
      _error('response.error.config_invalid', message),
  'response.error.conflict': (message) =>
      _error('response.error.conflict', message),
  'response.error.game_not_foreground': (message) =>
      _error('response.error.game_not_foreground', message),
  'response.error.internal': (message) =>
      _error('response.error.internal', message),
  'response.error.io': (message) => _error('response.error.io', message),
  'response.error.not_ready': (message) =>
      _error('response.error.not_ready', message),
  'response.error.not_running': (message) =>
      _error('response.error.not_running', message),
  'response.error.ocr_unavailable': (message) =>
      _error('response.error.ocr_unavailable', message),
  'response.error.tool_busy': (message) =>
      _error('response.error.tool_busy', message),
  'response.error.window_not_found': (message) =>
      _error('response.error.window_not_found', message),
};

/// The 6 event fixtures.
final Map<String, FixtureDecoder> eventDecoders = {
  'event.config.changed': (message) => _event(
    'event.config.changed',
    message,
    ConfigChangedEvent(
      ConfigChanged.fromJson(decodeMap(message['data'], 'data')),
    ),
  ),
  'event.core.stopping': (message) => _event(
    'event.core.stopping',
    message,
    CoreStoppingEvent(
      CoreStopping.fromJson(decodeMap(message['data'], 'data')),
    ),
  ),
  'event.log': (message) => _event(
    'event.log',
    message,
    LogEvent(LogLine.fromJson(decodeMap(message['data'], 'data'))),
  ),
  'event.state': (message) => _event(
    'event.state',
    message,
    StateEvent(ToolSnapshot.fromJson(decodeMap(message['data'], 'data'))),
  ),
  'event.state.empty': (message) => _event(
    'event.state.empty',
    message,
    StateEvent(ToolSnapshot.fromJson(decodeMap(message['data'], 'data'))),
  ),
  // The detail list is additive `v:1`; the typed ToolSnapshot gains it when the shell renders it
  // (frontend session), but the fixture must decode through the existing state event already.
  'event.state.detail': (message) => _event(
    'event.state.detail',
    message,
    StateEvent(ToolSnapshot.fromJson(decodeMap(message['data'], 'data'))),
  ),
  'event.tool.finished': (message) => _event(
    'event.tool.finished',
    message,
    ToolFinishedEvent(
      ToolFinished.fromJson(decodeMap(message['data'], 'data')),
    ),
  ),
  'event.tool.finished.summary': (message) => _event(
    'event.tool.finished.summary',
    message,
    ToolFinishedEvent(
      ToolFinished.fromJson(decodeMap(message['data'], 'data')),
    ),
  ),
};

/// The decoders for one fixture kind.
Map<String, FixtureDecoder> decodersFor(String kind) => switch (kind) {
  'request' => requestDecoders,
  'response' => responseDecoders,
  'event' => eventDecoders,
  _ => throw ArgumentError.value(kind, 'kind', 'unknown fixture kind'),
};

/// Decodes [fixture] through its case's decoder.
DecodedFixture decodeFixture(String caseName, String kind, JsonMap message) {
  final decoder = decodersFor(kind)[caseName];
  if (decoder == null) {
    throw StateError(
      'no decoder for fixture case "$caseName" ($kind) — add it to the '
      'fixture_decoders.dart map',
    );
  }
  return decoder(message);
}

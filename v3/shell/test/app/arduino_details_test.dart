/// The `board.details` decoder: the wire contract shape, the absent-firmware "not checked yet"
/// case, and the documented tolerance for a newer core's additive `choice.state`.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/arduino_details.dart';

/// A minimal well-formed `board.details` payload (PROTOCOL §4 / the golden fixture's shape).
Map<String, dynamic> _details({
  String state = 'ready',
  String? refusal,
  Map<String, dynamic>? firmware,
  String? chosenPort = 'COM5',
}) => {
  'baud': 115200,
  'expected': {
    'vid': '2341',
    'pids': ['8036', '8037'],
    'port': null,
  },
  'ports': [
    {
      'port': 'COM5',
      'vid': '2341',
      'pid': '8036',
      'product': 'Arduino Leonardo',
      'manufacturer': 'Arduino LLC',
      'matches_ids': true,
      'chosen': chosenPort == 'COM5',
    },
  ],
  'choice': {'state': state, 'port': chosenPort, 'refusal': refusal},
  'firmware': firmware,
};

void main() {
  test('the details payload decodes its contract shape', () {
    final details = BoardDetailsResult.fromJson(
      _details(firmware: {'level': 2, 'checked': true}),
    );

    expect(details.baud, 115200);
    expect(details.expected.vid, '2341');
    expect(details.expected.pids, ['8036', '8037']);
    expect(details.expected.port, isNull);
    expect(details.expected.hasPortOverride, isFalse);
    expect(details.choice.state, BoardChoiceState.ready);
    expect(details.choice.port, 'COM5');
    expect(details.choice.refusal, isNull);
    expect(details.firmware?.level, 2);

    final port = details.ports.single;
    expect(port.port, 'COM5');
    expect(port.vid, '2341');
    expect(port.pid, '8036');
    expect(port.description, 'Arduino Leonardo');
    expect(port.matchesIds, isTrue);
    expect(port.chosen, isTrue);
  });

  test('absent firmware decodes to null: not checked yet is normal', () {
    final details = BoardDetailsResult.fromJson(_details());
    expect(
      details.firmware,
      isNull,
      reason: '"not checked yet" is a normal state, not an error',
    );
  });

  test('an unknown choice state decodes as notFound instead of throwing', () {
    // A newer core may add an additive `choice.state`; the shell keeps an honest, conservative
    // verdict (Start would have no board this build can name) rather than failing to decode.
    final details = BoardDetailsResult.fromJson(
      _details(state: 'some_future_state', chosenPort: null),
    );
    expect(details.choice.state, BoardChoiceState.notFound);
    expect(details.choice.port, isNull);
    expect(details.choice.refusal, isNull);

    expect(BoardChoiceState.parse('ready'), BoardChoiceState.ready);
    expect(
      BoardChoiceState.parse('named_missing'),
      BoardChoiceState.namedMissing,
    );
    expect(BoardChoiceState.parse('not_found'), BoardChoiceState.notFound);
    expect(
      BoardChoiceState.parse('some_future_state'),
      BoardChoiceState.notFound,
    );
    expect(
      BoardChoiceState.parse(null),
      BoardChoiceState.notFound,
      reason: 'a missing state is not a ready board',
    );
  });

  test('a port override and an explicit refusal decode as data', () {
    final details = BoardDetailsResult.fromJson({
      ..._details(state: 'named_missing', chosenPort: 'COM9'),
      'expected': {
        'vid': '2341',
        'pids': ['8036'],
        'port': 'COM9',
      },
      'choice': {
        'state': 'named_missing',
        'port': 'COM9',
        'refusal': 'arduino.port names COM9, which this machine does not have',
      },
    });

    expect(details.expected.hasPortOverride, isTrue);
    expect(details.expected.port, 'COM9');
    expect(details.choice.state, BoardChoiceState.namedMissing);
    expect(
      details.choice.refusal,
      'arduino.port names COM9, which this machine does not have',
    );
    expect(details.ports.single.chosen, isFalse);
  });
}

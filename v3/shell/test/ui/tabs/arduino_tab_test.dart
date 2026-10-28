/// The Arduino tab: v2's Connection card on the read-only `board.details` (three-state light, port
/// list, what the launcher looks for, cached firmware), plus the explicit `board.probe` firmware
/// query and the three `board.test` input tests. Every refusal is the core's line, verbatim.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/protocol/protocol.dart';
import 'package:seal_shell/ui/tabs/arduino_tab.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

Map<String, dynamic> _probeAnswer() => {
  'port': 'COM3',
  'baud': 115200,
  'vid': '2341',
  'pid': '8036',
  'level': 2,
};

/// One port row for a fake `board.details` answer.
Map<String, dynamic> _port({
  required String port,
  bool chosen = false,
  bool matchesIds = false,
  String? product,
}) => {
  'port': port,
  'vid': matchesIds ? '2341' : '10c4',
  'pid': matchesIds ? '8036' : 'ea60',
  'product': product,
  'manufacturer': product == null ? null : 'Arduino LLC',
  'matches_ids': matchesIds,
  'chosen': chosen,
};

/// A fake `board.details` answer. `choice.port` follows the protocol: the chosen port when ready, the
/// NAMED missing port for `named_missing`, null otherwise.
Map<String, dynamic> _detailsAnswer({
  String state = 'ready',
  String? port = 'COM5',
  String? refusal,
  List<Map<String, dynamic>>? ports,
  Map<String, dynamic>? firmware,
  Map<String, dynamic>? expected,
}) => {
  'baud': 115200,
  'expected':
      expected ??
      {
        'vid': '2341',
        'pids': ['8036', '8037'],
        'port': null,
      },
  'ports':
      ports ??
      [
        _port(
          port: 'COM5',
          chosen: state == 'ready',
          matchesIds: true,
          product: 'Arduino Leonardo',
        ),
      ],
  'choice': {
    'state': state,
    'port': state == 'ready'
        ? port
        : (state == 'named_missing' ? 'COM9' : null),
    'refusal': refusal,
  },
  'firmware': firmware,
};

/// Pumps the tab with a fake core. [answer] drives every command; a test that does not answer
/// `board.details` explicitly gets the ready card below.
Future<AppHarness> _pumpArduino(
  WidgetTester tester, {
  required Map<String, dynamic> Function(Map<String, dynamic> request) answer,
}) async {
  tester.view.physicalSize = const ui.Size(1600, 2400);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  final harness = AppHarness();
  addTearDown(harness.model.dispose);
  await tester.pumpWidget(
    const MaterialApp(
      home: Scaffold(body: Center(child: CircularProgressIndicator())),
    ),
  );
  final connecting = harness.connect(
    config: {
      'window': {'title': 'TW_LIVE'},
    },
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  harness.onWrite = (line, fake) {
    final message = fake.sentJson(fake.sent.length - 1);
    final id = message['id'] as String;
    final reply = answer(message);
    if (reply.containsKey('error')) {
      final error = reply['error'] as Map<String, dynamic>;
      fake.emitJson(
        errorResponse(
          id,
          error['code'] as String,
          message: error['message'] as String,
        ),
      );
    } else {
      fake.emitJson(okResponse(id, reply));
    }
  };
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(body: ArduinoTab(model: harness.model)),
    ),
  );
  await tester.pump();
  await tester.pump();
  return harness;
}

/// The `data` of one keyed Text.
String _text(WidgetTester tester, String key) =>
    tester.widget<Text>(find.byKey(ValueKey(key))).data!;

/// The first Text inside one keyed action-result line.
String _resultText(WidgetTester tester, String key) => tester
    .widget<Text>(
      find.descendant(
        of: find.byKey(ValueKey(key)),
        matching: find.byType(Text),
      ),
    )
    .data!;

/// The connection light's fill.
Color? _statusColor(WidgetTester tester) {
  final container = tester.widget<Container>(
    find.byKey(const ValueKey('arduino-connection-status')),
  );
  final decoration = container.decoration;
  return decoration is BoxDecoration ? decoration.color : null;
}

Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets('opening the tab asks board.details once, never board.probe', (
    tester,
  ) async {
    var detailsCalls = 0;
    final harness = await _pumpArduino(
      tester,
      answer: (request) {
        if (request['cmd'] == 'board.details') {
          detailsCalls++;
          return _detailsAnswer();
        }
        return <String, dynamic>{};
      },
    );

    expect(detailsCalls, 1);
    expect(
      harness.lastSentWithCmd('board.probe'),
      isNull,
      reason: 'the tab is read-only until Query firmware is pressed',
    );
    // A rebuild is not a refetch.
    await tester.pump();
    expect(detailsCalls, 1);
    // The explicit Refresh re-reads.
    await tester.tap(find.byKey(const ValueKey('arduino-refresh-details')));
    await _flush(tester);
    expect(detailsCalls, 2);

    expect(
      _text(tester, 'arduino-connection-verdict'),
      'Ready — Start will open COM5',
    );
    expect(_statusColor(tester), Colors.green);
    expect(
      find.byKey(const ValueKey('arduino-advice')),
      findsNothing,
      reason: 'a ready Start has nothing to advise',
    );
    expect(
      _text(tester, 'arduino-looking-for'),
      contains('VID 0x2341    PID 0x8036 / 0x8037'),
    );
    expect(_text(tester, 'arduino-looking-for'), contains('baud 115200'));
    expect(
      _text(tester, 'arduino-looking-for'),
      contains('arduino.port: not set — the board is found by the ID above'),
    );
  });

  testWidgets('a named missing port renders the core refusal verbatim', (
    tester,
  ) async {
    const refusal =
        'arduino.port names COM9, which this machine does not have — so nothing was opened, '
        'rather than opening a different board. Fix it in local.yaml, or clear it to go back to '
        'finding the board by its IDs.';
    await _pumpArduino(
      tester,
      answer: (request) => request['cmd'] == 'board.details'
          ? _detailsAnswer(state: 'named_missing', refusal: refusal)
          : <String, dynamic>{},
    );

    final theme = Theme.of(tester.element(find.byType(ArduinoTab)));
    expect(
      _text(tester, 'arduino-connection-verdict'),
      'Start will refuse — the named port is missing',
    );
    expect(_text(tester, 'arduino-advice'), refusal);
    expect(_statusColor(tester), theme.colorScheme.error);
    // The present board is NOT marked as chosen: Start would refuse it, because COM9 was named.
    final row = _text(tester, 'arduino-port-row-COM5');
    expect(row, startsWith('○'));
    expect(row, contains('matches the expected ID'));
    expect(row, isNot(contains('Start will open this one')));
  });

  testWidgets('no board found renders the charge-only-cable advice', (
    tester,
  ) async {
    await _pumpArduino(
      tester,
      answer: (request) => request['cmd'] == 'board.details'
          ? _detailsAnswer(
              state: 'not_found',
              refusal: 'no board found: the serial port list is empty',
              ports: [_port(port: 'COM3')],
            )
          : <String, dynamic>{},
    );

    final theme = Theme.of(tester.element(find.byType(ArduinoTab)));
    expect(
      _text(tester, 'arduino-connection-verdict'),
      'Start will refuse — no Arduino found',
    );
    expect(
      _text(tester, 'arduino-advice'),
      'Plug the board in, then press Refresh. A charge-only cable is the usual cause — the '
      'board lights up and never appears as a port.',
    );
    expect(_statusColor(tester), theme.colorScheme.error);
  });

  testWidgets('no serial ports at all gets the honest line', (tester) async {
    await _pumpArduino(
      tester,
      answer: (request) => request['cmd'] == 'board.details'
          ? _detailsAnswer(
              state: 'not_found',
              refusal: 'no board found: the serial port list is empty',
              ports: <Map<String, dynamic>>[],
            )
          : <String, dynamic>{},
    );

    expect(
      _text(tester, 'arduino-ports-empty'),
      'This machine reports no serial ports at all.',
    );
  });

  testWidgets('the chosen port is first and marked; other matches say so', (
    tester,
  ) async {
    await _pumpArduino(
      tester,
      answer: (request) => request['cmd'] == 'board.details'
          ? _detailsAnswer(
              ports: [
                _port(
                  port: 'COM5',
                  chosen: true,
                  matchesIds: true,
                  product: 'Arduino Leonardo',
                ),
                _port(port: 'COM3', matchesIds: true),
              ],
            )
          : <String, dynamic>{},
    );

    final chosen = _text(tester, 'arduino-port-row-COM5');
    expect(chosen, startsWith('●'));
    expect(chosen, contains('Arduino Leonardo'));
    expect(chosen, contains('Start will open this one'));

    final other = _text(tester, 'arduino-port-row-COM3');
    expect(other, startsWith('○'));
    expect(other, contains('matches the expected ID'));

    // First row in the list is the chosen one.
    expect(
      tester.getTopLeft(find.byKey(const ValueKey('arduino-port-row-COM5'))).dy,
      lessThan(
        tester
            .getTopLeft(find.byKey(const ValueKey('arduino-port-row-COM3')))
            .dy,
      ),
    );
  });

  testWidgets('firmware is not checked yet, then Query firmware reads it', (
    tester,
  ) async {
    final harness = await _pumpArduino(
      tester,
      answer: (request) => switch (request['cmd']) {
        'board.details' => _detailsAnswer(),
        'board.probe' => _probeAnswer(),
        _ => <String, dynamic>{},
      },
    );

    expect(
      _text(tester, 'arduino-firmware'),
      'Not checked yet — this is normal.',
    );
    expect(
      _text(tester, 'arduino-firmware-detail'),
      contains('read the first time a tool opens it'),
    );

    await tester.tap(find.byKey(const ValueKey('arduino-query-firmware')));
    await _flush(tester);

    expect(
      harness.lastSentWithCmd('board.probe')!['args'],
      <String, dynamic>{},
    );
    expect(
      _resultText(tester, 'arduino-probe-result'),
      contains(
        'port COM3 · 115200 baud · VID 2341 · PID 8036 · protocol level 2',
      ),
    );
    expect(
      _text(tester, 'arduino-firmware'),
      'Protocol level 2 (queried now).',
    );
  });

  testWidgets('a cached firmware level reads as checked', (tester) async {
    await _pumpArduino(
      tester,
      answer: (request) => request['cmd'] == 'board.details'
          ? _detailsAnswer(firmware: {'level': 2, 'checked': true})
          : <String, dynamic>{},
    );

    expect(_text(tester, 'arduino-firmware'), 'Protocol level 2 (checked).');
    expect(
      _text(tester, 'arduino-firmware-detail'),
      "Reported by the board's V reply.",
    );
  });

  testWidgets('Query firmware is disabled with its reason while a tool runs', (
    tester,
  ) async {
    final harness = await _pumpArduino(
      tester,
      answer: (request) => switch (request['cmd']) {
        'board.details' => _detailsAnswer(),
        _ => <String, dynamic>{},
      },
    );

    harness.emitState(tool: ToolId.tuner, message: 'scanning the grade');
    await _flush(tester);

    final button = tester.widget<OutlinedButton>(
      find.byKey(const ValueKey('arduino-query-firmware')),
    );
    expect(
      button.onPressed,
      isNull,
      reason: 'the probe opens the port the running tool is using',
    );
    expect(
      _text(tester, 'arduino-query-busy'),
      contains('Magic Tuner'),
      reason: 'the disabled reason names the tool holding the board',
    );
    // The read-only card is NOT gated by the run.
    expect(
      _text(tester, 'arduino-connection-verdict'),
      'Ready — Start will open COM5',
    );
  });

  testWidgets('a board.details refusal renders verbatim', (tester) async {
    await _pumpArduino(
      tester,
      answer: (request) {
        if (request['cmd'] == 'board.details') {
          return {
            'error': {
              'code': 'board_unavailable',
              'message': 'no board found: the serial port list is empty',
            },
          };
        }
        return <String, dynamic>{};
      },
    );

    expect(
      _resultText(tester, 'arduino-details-error'),
      contains('no board found: the serial port list is empty'),
    );
  });

  testWidgets('click/move/key send the exact board.test payloads', (
    tester,
  ) async {
    final harness = await _pumpArduino(
      tester,
      answer: (request) => switch (request['cmd']) {
        'board.details' => _detailsAnswer(),
        _ => <String, dynamic>{},
      },
    );

    await tester.tap(find.byKey(const ValueKey('arduino-click')));
    await _flush(tester);
    expect(
      (harness.lastSentWithCmd('board.test')!['args'] as Map<String, dynamic>)
          .cast<String, dynamic>(),
      {'kind': 'click'},
    );
    expect(_resultText(tester, 'arduino-click-result'), 'Click: sent');

    await tester.enterText(find.byKey(const ValueKey('arduino-move-dx')), '50');
    await tester.enterText(
      find.byKey(const ValueKey('arduino-move-dy')),
      '-20',
    );
    await tester.tap(find.byKey(const ValueKey('arduino-move')));
    await _flush(tester);
    expect(
      (harness.lastSentWithCmd('board.test')!['args'] as Map<String, dynamic>)
          .cast<String, dynamic>(),
      {'kind': 'move', 'dx': 50, 'dy': -20},
    );
    expect(_resultText(tester, 'arduino-move-result'), 'Move: sent');

    await tester.enterText(
      find.byKey(const ValueKey('arduino-key-input')),
      'Q',
    );
    await tester.tap(find.byKey(const ValueKey('arduino-send-key')));
    await _flush(tester);
    expect(
      (harness.lastSentWithCmd('board.test')!['args'] as Map<String, dynamic>)
          .cast<String, dynamic>(),
      {'kind': 'key', 'key': 'Q'},
    );
    expect(_resultText(tester, 'arduino-key-result'), 'Key: sent');
  });

  testWidgets('a refusal line is shown verbatim; empty input is not sent', (
    tester,
  ) async {
    final harness = await _pumpArduino(
      tester,
      answer: (request) {
        if (request['cmd'] == 'board.details') {
          return _detailsAnswer();
        }
        return {
          'error': {
            'code': 'bad_request',
            'message':
                'board.test key needs one printable character, Enter or F1–F12',
          },
        };
      },
    );

    await tester.enterText(
      find.byKey(const ValueKey('arduino-key-input')),
      'QQ',
    );
    await tester.tap(find.byKey(const ValueKey('arduino-send-key')));
    await _flush(tester);
    expect(
      _resultText(tester, 'arduino-key-result'),
      contains('board.test key needs one printable character, Enter or F1–F12'),
    );

    // Empty input is a shell-side note; nothing is sent.
    await tester.enterText(find.byKey(const ValueKey('arduino-key-input')), '');
    await tester.pump();
    final sentBefore = harness.sentMessages
        .where((message) => message['cmd'] == 'board.test')
        .length;
    await tester.tap(find.byKey(const ValueKey('arduino-send-key')));
    await _flush(tester);
    expect(
      _resultText(tester, 'arduino-key-result'),
      contains('type the key to send'),
    );
    expect(
      harness.sentMessages
          .where((message) => message['cmd'] == 'board.test')
          .length,
      sentBefore,
    );
  });
}

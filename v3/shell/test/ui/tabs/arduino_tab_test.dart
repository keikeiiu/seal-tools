/// The Arduino tab: the probe readout and the three `board.test` input tests, each rendering the
/// core's success or refusal line.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

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

/// Pumps the tab with a fake core. [answer] drives `board.probe`/`board.test`; the record of sent
/// `board.test` args is kept so a test can assert the exact payload.
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
    final cmd = message['cmd'] as String;
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
    if (cmd == 'board.probe' || cmd == 'board.test') {
      // nothing to record: the test reads lastSentWithCmd
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

String _resultText(WidgetTester tester, String key) => tester
    .widget<Text>(
      find.descendant(
        of: find.byKey(ValueKey(key)),
        matching: find.byType(Text),
      ),
    )
    .data!;

Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets('probe readout, and the no-board line verbatim', (tester) async {
    var noBoard = false;
    await _pumpArduino(
      tester,
      answer: (request) {
        if (request['cmd'] == 'board.probe') {
          return noBoard
              ? {
                  'error': {
                    'code': 'board_unavailable',
                    'message': 'no board found: the serial port list is empty',
                  },
                }
              : _probeAnswer();
        }
        return <String, dynamic>{};
      },
    );

    expect(
      _resultText(tester, 'arduino-probe-result'),
      contains(
        'port COM3 · 115200 baud · VID 2341 · PID 8036 · protocol level 2',
      ),
    );

    noBoard = true;
    await tester.tap(find.byKey(const ValueKey('arduino-refresh-probe')));
    await _flush(tester);
    expect(
      _resultText(tester, 'arduino-probe-result'),
      contains('no board found: the serial port list is empty'),
    );
  });

  testWidgets('click/move/key send the exact board.test payloads', (
    tester,
  ) async {
    final harness = await _pumpArduino(
      tester,
      answer: (request) => request['cmd'] == 'board.probe'
          ? _probeAnswer()
          : <String, dynamic>{},
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
        if (request['cmd'] == 'board.probe') {
          return _probeAnswer();
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

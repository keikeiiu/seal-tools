/// The Setup tab: one save across window/game/arduino (portable under `settings:`, port as the owned
/// `arduino` block) and the three read-only core actions including the no-board refusal.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/ui/tabs/setup_tab.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

Map<String, dynamic> _setupConfig() => {
  'window': {'title': 'TW_LIVE'},
  'game': {
    'variant': 'tw',
    'variants': {
      'tw': {'title': 'TW_LIVE', 'attributes': 'attributes.yaml'},
      'us': {'title': 'US_LIVE', 'attributes': 'attributes.us.yaml'},
    },
  },
  'arduino': {
    'vid': 9025,
    'pid': [32822, 32823],
    'baud': 115200,
    'port': '',
  },
};

Map<String, dynamic> _displayJson() => {
  'scale': 1.5,
  'monitor_dpi': 144,
  'screen': {'w': 2560, 'h': 1440},
  'frame': {'x': 100, 'y': 100, 'w': 1940, 'h': 1120},
  'physical_client': {'x': 110, 'y': 130, 'w': 1920, 'h': 1080},
  'logical_client': {'x': 73, 'y': 87, 'w': 1280, 'h': 720},
};

Finder _field(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(TextField),
);

Future<AppHarness> _pumpSetup(WidgetTester tester) async {
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
  final connecting = harness.connect(config: _setupConfig());
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(body: SetupTab(model: harness.model)),
    ),
  );
  await tester.pump();
  return harness;
}

Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets(
    'renders the blocks and saves them as settings + the owned port',
    (tester) async {
      final harness = await _pumpSetup(tester);

      // Read-only readouts: the configured title and the active variant's dictionary.
      expect(find.textContaining('TW_LIVE'), findsWidgets);
      expect(
        find.textContaining('tw · dictionary attributes.yaml'),
        findsOneWidget,
      );

      await tester.enterText(_field('setup-window-title'), 'US_LIVE');
      await tester.enterText(_field('setup-arduino-baud'), '9600');
      await tester.pump();
      expect(find.byKey(const ValueKey('dirty-setup')), findsOneWidget);

      harness.answerConfigCommands();
      await tester.tap(find.byKey(const ValueKey('save-setup')));
      await _flush(tester);

      final apply = harness.lastSentWithCmd('config.apply');
      expect(apply, isNotNull);
      final sections =
          (apply!['args'] as Map<String, dynamic>)['sections']
              as Map<String, dynamic>;
      expect(
        sections.keys.toSet(),
        {'settings', 'arduino'},
        reason:
            'portable half under settings, the machine port as its own block',
      );
      final settings = sections['settings'] as Map<String, dynamic>;
      expect(settings.keys.toSet(), {'window', 'game', 'arduino'});
      expect((settings['window'] as Map<String, dynamic>)['title'], 'US_LIVE');
      expect((settings['game'] as Map<String, dynamic>)['variant'], 'tw');
      final arduino = settings['arduino'] as Map<String, dynamic>;
      expect(arduino['baud'], 9600);
      expect(arduino['vid'], 9025);
      expect(arduino['pid'], [32822, 32823]);
      expect(
        (sections['arduino'] as Map<String, dynamic>)['port'],
        '',
        reason: 'the machine port rides the owned arduino block',
      );

      expect(find.byKey(const ValueKey('dirty-setup')), findsNothing);
      expect(find.byKey(const ValueKey('save-ok-setup')), findsOneWidget);
    },
  );

  testWidgets(
    'the three actions render the core\'s data and its no-board line',
    (tester) async {
      final harness = await _pumpSetup(tester);
      var noBoard = false;
      harness.onWrite = (line, fake) {
        final message = fake.sentJson(fake.sent.length - 1);
        final id = message['id'] as String;
        switch (message['cmd']) {
          case 'window.find':
            fake.emitJson(okResponse(id, {'found': true, 'title': 'TW_LIVE'}));
          case 'window.measure':
            fake.emitJson(okResponse(id, {'display': _displayJson()}));
          case 'board.probe':
            if (noBoard) {
              fake.emitJson(
                errorResponse(
                  id,
                  'board_unavailable',
                  message: 'no board found: the serial port list is empty',
                ),
              );
            } else {
              fake.emitJson(
                okResponse(id, {
                  'port': 'COM3',
                  'baud': 115200,
                  'vid': '2341',
                  'pid': '8036',
                  'level': 2,
                }),
              );
            }
        }
      };

      await tester.tap(find.byKey(const ValueKey('setup-find-window')));
      await tester.tap(find.byKey(const ValueKey('setup-measure-window')));
      await tester.tap(find.byKey(const ValueKey('setup-probe-board')));
      await _flush(tester);

      expect(
        tester
            .widget<Text>(find.byKey(const ValueKey('setup-find-result')))
            .data,
        contains('Window found: TW_LIVE'),
      );
      final measure = tester
          .widget<Text>(find.byKey(const ValueKey('setup-measure-result')))
          .data!;
      expect(measure, contains('scale 1.5'));
      expect(measure, contains('monitor DPI 144'));
      expect(measure, contains('client 1920x1080 physical'));
      expect(measure, contains('1280x720 logical'));
      final probe = tester
          .widget<Text>(find.byKey(const ValueKey('setup-probe-result')))
          .data!;
      expect(probe, contains('port COM3'));
      expect(probe, contains('VID 2341'));
      expect(probe, contains('protocol level 2'));

      // The no-hardware path: the core's own board_unavailable line, verbatim.
      noBoard = true;
      await tester.tap(find.byKey(const ValueKey('setup-probe-board')));
      await _flush(tester);
      expect(
        tester
            .widget<Text>(find.byKey(const ValueKey('setup-probe-result')))
            .data,
        contains('no board found: the serial port list is empty'),
      );
    },
  );

  testWidgets('renders the core validation lines verbatim; Cancel restores', (
    tester,
  ) async {
    final harness = await _pumpSetup(tester);
    await tester.enterText(_field('setup-window-title'), 'US_LIVE');
    await tester.pump();

    harness.answerConfigCommands(
      errors: ['the game block is malformed: variant must be a string'],
    );
    await tester.tap(find.byKey(const ValueKey('save-setup')));
    await _flush(tester);

    expect(
      find.text('the game block is malformed: variant must be a string'),
      findsOneWidget,
    );
    expect(harness.lastSentWithCmd('config.apply'), isNull);

    await tester.tap(find.byKey(const ValueKey('cancel-setup')));
    await tester.pump();
    expect(
      tester.widget<TextField>(_field('setup-window-title')).controller?.text,
      'TW_LIVE',
    );
  });
}

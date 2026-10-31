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
  testWidgets(
    'renders the board match read-only and saves only the editable blocks',
    (tester) async {
      final harness = await _pumpSetup(tester);

      // The game window title is read-only (the player's live-pass decision).
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('setup-window-title')),
          matching: find.byType(TextField),
        ),
        findsNothing,
      );
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('setup-window-title')),
          matching: find.text('TW_LIVE'),
        ),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('setup-title-caption')), findsOneWidget);

      // The four Arduino fields are shown but not editable: shipped board-matching values.
      for (final key in [
        'setup-arduino-vid',
        'setup-arduino-baud',
        'setup-arduino-pid',
        'setup-arduino-port',
      ]) {
        expect(
          find.descendant(
            of: find.byKey(ValueKey(key)),
            matching: find.byType(TextField),
          ),
          findsNothing,
          reason: '$key is read-only',
        );
      }
      expect(find.text('9025'), findsOneWidget);
      expect(find.text('115200'), findsOneWidget);
      expect(find.text('32822, 32823'), findsOneWidget);
      expect(find.text('automatic (by board IDs)'), findsOneWidget);

      // The variant is the editable field on this screen now.
      await tester.tap(find.byKey(const ValueKey('setup-variant')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('us').last);
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('dirty-setup')), findsOneWidget);

      harness.answerConfigCommands();
      await tester.tap(find.byKey(const ValueKey('save-setup')));
      await _flush(tester);

      final apply = harness.lastSentWithCmd('config.apply');
      expect(apply, isNotNull);
      final sections =
          (apply!['args'] as Map<String, dynamic>)['sections']
              as Map<String, dynamic>;
      expect(sections.keys.toSet(), {
        'settings',
      }, reason: 'the read-only Arduino values are never sent back');
      final settings = sections['settings'] as Map<String, dynamic>;
      expect(settings.keys.toSet(), {'window', 'game'});
      expect(
        (settings['window'] as Map<String, dynamic>)['title'],
        'TW_LIVE',
        reason: 'the title is displayed read-only, so the save keeps it',
      );
      expect((settings['game'] as Map<String, dynamic>)['variant'], 'us');
      expect(
        sections.containsKey('arduino'),
        isFalse,
        reason: 'the machine port is not editable here',
      );

      expect(find.byKey(const ValueKey('dirty-setup')), findsNothing);
      expect(find.byKey(const ValueKey('save-ok-setup')), findsOneWidget);
    },
  );

  testWidgets(
    'the core actions render the data and the no-board refusal line',
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
        _resultText(tester, 'setup-find-result'),
        contains('Window found: TW_LIVE'),
      );
      final measure = _resultText(tester, 'setup-measure-result');
      expect(measure, contains('scale 1.5'));
      expect(measure, contains('monitor DPI 144'));
      expect(measure, contains('client 1920x1080 physical'));
      expect(measure, contains('1280x720 logical'));
      final probe = _resultText(tester, 'setup-probe-result');
      expect(probe, contains('port COM3'));
      expect(probe, contains('VID 2341'));
      expect(probe, contains('protocol level 2'));

      // The no-hardware path: the core's own board_unavailable line, verbatim.
      noBoard = true;
      await tester.tap(find.byKey(const ValueKey('setup-probe-board')));
      await _flush(tester);
      expect(
        _resultText(tester, 'setup-probe-result'),
        contains('no board found: the serial port list is empty'),
      );
    },
  );

  testWidgets('renders the core validation lines verbatim; Cancel restores', (
    tester,
  ) async {
    final harness = await _pumpSetup(tester);
    await tester.tap(find.byKey(const ValueKey('setup-variant')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('us').last);
    await tester.pumpAndSettle();

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
      find.text('tw'),
      findsWidgets,
      reason: 'Cancel re-seeds the editable draft from the mirror',
    );
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('setup-window-title')),
        matching: find.text('TW_LIVE'),
      ),
      findsOneWidget,
    );
  });

  testWidgets(
    'Use detected title is gated on a successful Find and saves the detected title',
    (tester) async {
      final harness = await _pumpSetup(tester);
      var detected = '';

      harness.onWrite = (line, fake) {
        final message = fake.sentJson(fake.sent.length - 1);
        final id = message['id'] as String;
        if (message['cmd'] == 'window.find') {
          fake.emitJson(
            okResponse(id, {'found': detected.isNotEmpty, 'title': detected}),
          );
        }
      };

      final use = find.byKey(const ValueKey('setup-use-detected-title'));
      // The only action that writes config is the screen's single filled action now (§2.8).
      expect(tester.widget<FilledButton>(use).onPressed, isNull);

      // A find that detected no title must not enable the save.
      await tester.tap(find.byKey(const ValueKey('setup-find-window')));
      await _flush(tester);
      expect(tester.widget<FilledButton>(use).onPressed, isNull);

      detected = 'DETECTED_TITLE';
      await tester.tap(find.byKey(const ValueKey('setup-find-window')));
      await _flush(tester);
      expect(tester.widget<FilledButton>(use).onPressed, isNotNull);

      harness.answerConfigCommands();
      await tester.tap(use);
      await _flush(tester);

      final apply = harness.lastSentWithCmd('config.apply');
      expect(apply, isNotNull);
      final sections =
          (apply!['args'] as Map<String, dynamic>)['sections']
              as Map<String, dynamic>;
      final window =
          (sections['settings'] as Map<String, dynamic>)['window']
              as Map<String, dynamic>;
      expect(window['title'], 'DETECTED_TITLE');
    },
  );
}

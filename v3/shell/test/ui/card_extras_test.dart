/// Card extras: the Buy card's display-only preset picker/count (the protocol has no run argument, and
/// `buy_sell` has no active field) and the Quest card's persisted flow/sequence pickers.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/ui/card_grid.dart';
import 'package:seal_shell/ui/theme.dart';

import '../support/app_harness.dart';

Map<String, dynamic> _config({
  Map<String, dynamic>? buySell,
  Map<String, dynamic>? quest,
}) => {
  'window': {'title': 'TW_LIVE'},
  'buy_sell': ?buySell,
  'quest': ?quest,
};

Future<AppHarness> _pumpCards(
  WidgetTester tester,
  Map<String, dynamic> config,
) async {
  tester.view.physicalSize = const ui.Size(1600, 2400);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  final harness = AppHarness();
  addTearDown(harness.model.dispose);
  final connecting = harness.connect(config: config);
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(
        body: ListenableBuilder(
          listenable: harness.model,
          builder: (context, _) => CardGrid(model: harness.model),
        ),
      ),
    ),
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  await tester.pump();
  return harness;
}

Future<void> _unmount(WidgetTester tester) async {
  await tester.pumpWidget(const SizedBox());
  await tester.pump();
}

Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets(
    'the Buy card defaults to the first config preset and its count',
    (tester) async {
      final harness = await _pumpCards(
        tester,
        _config(
          buySell: {
            'presets': {
              'Potion': {'scroll': 0, 'row': 2, 'count': 2},
              'Herb': {'scroll': 0, 'row': 1, 'count': 3},
            },
            'sell_cap': 10,
          },
        ),
      );

      // CONFIG order, not sorted: Potion is first.
      expect(find.text('Potion'), findsOneWidget);
      expect(
        tester
            .widget<Text>(find.byKey(const ValueKey('buy-card-count-value')))
            .data,
        '2',
      );
      expect(find.byKey(const ValueKey('buy-card-note')), findsOneWidget);

      // Nothing is sent on mount, and the choice was never a config write.
      expect(harness.lastSentWithCmd('tool.start'), isNull);
      expect(harness.lastSentWithCmd('config.validate'), isNull);
      expect(harness.lastSentWithCmd('config.apply'), isNull);
      await _unmount(tester);
    },
  );

  testWidgets('Start sends the selected buy preset and count exactly', (
    tester,
  ) async {
    final harness = await _pumpCards(
      tester,
      _config(
        buySell: {
          'presets': {
            'Potion': {'scroll': 0, 'row': 2, 'count': 2},
            'Herb': {'scroll': 0, 'row': 1, 'count': 3},
          },
          'sell_cap': 10,
        },
      ),
    );

    await tester.tap(find.byKey(const ValueKey('start-buy')));
    await _flush(tester);
    expect(harness.lastSentWithCmd('tool.start')!['args'], {
      'tool': 'buy',
      'preset': 'Potion',
      'count': 2,
    });
    harness.respondOk(<String, dynamic>{});
    await _flush(tester);
    expect(harness.lastSentWithCmd('config.validate'), isNull);
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    await _unmount(tester);
  });

  testWidgets('changing the preset resets the count to its configured value', (
    tester,
  ) async {
    final harness = await _pumpCards(
      tester,
      _config(
        buySell: {
          'presets': {
            'Potion': {'scroll': 0, 'row': 2, 'count': 2},
            'Herb': {'scroll': 0, 'row': 1, 'count': 3},
          },
          'sell_cap': 10,
        },
      ),
    );

    await tester.tap(find.byKey(const ValueKey('buy-card-preset')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('buy-card-item-Herb')));
    await tester.pumpAndSettle();
    expect(
      tester
          .widget<Text>(find.byKey(const ValueKey('buy-card-count-value')))
          .data,
      '3',
    );

    await tester.tap(find.byKey(const ValueKey('start-buy')));
    await _flush(tester);
    expect(harness.lastSentWithCmd('tool.start')!['args'], {
      'tool': 'buy',
      'preset': 'Herb',
      'count': 3,
    });
    harness.respondOk(<String, dynamic>{});
    await _flush(tester);
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    await _unmount(tester);
  });

  testWidgets('the stepper changes only the count, never the preset', (
    tester,
  ) async {
    final harness = await _pumpCards(
      tester,
      _config(
        buySell: {
          'presets': {
            'Potion': {'scroll': 0, 'row': 2, 'count': 2},
          },
        },
      ),
    );

    // A pump between taps: each tap rebuilds the card from the new count, so two taps in one frame
    // would both read the pre-tap value (a test artefact, not a UI defect).
    await tester.tap(find.byKey(const ValueKey('buy-card-count-plus')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('buy-card-count-plus')));
    await tester.pump();
    expect(
      tester
          .widget<Text>(find.byKey(const ValueKey('buy-card-count-value')))
          .data,
      '4',
    );

    await tester.tap(find.byKey(const ValueKey('start-buy')));
    await _flush(tester);
    expect(harness.lastSentWithCmd('tool.start')!['args'], {
      'tool': 'buy',
      'preset': 'Potion',
      'count': 4,
    });
    harness.respondOk(<String, dynamic>{});
    await _flush(tester);
    await _unmount(tester);
  });

  testWidgets(
    'no presets: plain Start and the core refusal line renders verbatim',
    (tester) async {
      final harness = await _pumpCards(
        tester,
        _config(buySell: {'presets': <String, dynamic>{}}),
      );

      expect(find.text('No buy presets configured'), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('start-buy')));
      await _flush(tester);
      expect(harness.lastSentWithCmd('tool.start')!['args'], {'tool': 'buy'});
      harness.respondError('bad_request', message: 'No buy preset selected.');
      await _flush(tester);

      expect(find.text('No buy preset selected.'), findsOneWidget);
      expect(find.byKey(const ValueKey('refusal-buy')), findsOneWidget);
      expect(harness.lastSentWithCmd('config.apply'), isNull);
      await _unmount(tester);
    },
  );
  testWidgets(
    'Quest flow and sequence pickers save exactly the changed field',
    (tester) async {
      final harness = await _pumpCards(
        tester,
        _config(
          quest: {
            'active': 'flowA',
            'active_sequence': '',
            'presets': {
              'flowA': {'steps': <Object?>[]},
              'flowB': {'steps': <Object?>[]},
            },
            'sequences': {
              'seq1': {'entries': <Object?>[]},
            },
          },
        ),
      );

      harness.answerConfigCommands();
      await tester.tap(find.byKey(const ValueKey('quest-card-flow')));
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(const ValueKey('quest-card-flow-item-flowB')),
      );
      await _flush(tester);

      final flowApply = harness.lastSentWithCmd('config.apply');
      expect(flowApply, isNotNull);
      expect((flowApply!['args'] as Map<String, dynamic>)['sections'], {
        'quest': {'active': 'flowB'},
      });
      expect(find.byKey(const ValueKey('quest-card-error')), findsNothing);
      await _unmount(tester);
    },
  );

  testWidgets('the empty sequence choice saves the single-flow meaning', (
    tester,
  ) async {
    final harness = await _pumpCards(
      tester,
      _config(
        quest: {
          'active': 'flowA',
          'active_sequence': 'seq1',
          'presets': {
            'flowA': {'steps': <Object?>[]},
          },
          'sequences': {
            'seq1': {'entries': <Object?>[]},
          },
        },
      ),
    );

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('quest-card-sequence')));
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const ValueKey('quest-card-sequence-item-single-flow')),
    );
    await _flush(tester);

    expect(
      (harness.lastSentWithCmd('config.apply')!['args']
          as Map<String, dynamic>)['sections'],
      {
        'quest': {'active_sequence': ''},
      },
    );
    await _unmount(tester);
  });

  testWidgets('a refused picker save renders the core line verbatim', (
    tester,
  ) async {
    final harness = await _pumpCards(
      tester,
      _config(
        quest: {
          'active': 'flowA',
          'active_sequence': '',
          'presets': {
            'flowA': {'steps': <Object?>[]},
            'flowB': {'steps': <Object?>[]},
          },
          'sequences': <String, dynamic>{},
        },
      ),
    );

    harness.answerConfigCommands(
      errors: ['the quest flow "flowB" does not exist'],
    );
    await tester.tap(find.byKey(const ValueKey('quest-card-flow')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('quest-card-flow-item-flowB')));
    await _flush(tester);

    expect(find.byKey(const ValueKey('quest-card-error')), findsOneWidget);
    expect(find.text('the quest flow "flowB" does not exist'), findsOneWidget);
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    await _unmount(tester);
  });

  testWidgets(
    'a long Quest-card error keeps the full line in text and tooltip',
    (tester) async {
      final harness = await _pumpCards(
        tester,
        _config(
          quest: {
            'active': 'flowA',
            'active_sequence': '',
            'presets': {
              'flowA': {'steps': <Object?>[]},
              'flowB': {'steps': <Object?>[]},
            },
            'sequences': <String, dynamic>{},
          },
        ),
      );
      const longLine =
          'the quest flow "flowB" names a sequence entry that no longer exists; '
          'fix it in the Quest tab before starting the run';

      harness.answerConfigCommands(errors: [longLine]);
      await tester.tap(find.byKey(const ValueKey('quest-card-flow')));
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(const ValueKey('quest-card-flow-item-flowB')),
      );
      await _flush(tester);

      final finder = find.byKey(const ValueKey('quest-card-error'));
      final text = tester.widget<Text>(finder);
      expect(text.data, longLine);
      expect(text.maxLines, 3);
      final tooltip = tester.widget<Tooltip>(
        find.ancestor(of: finder, matching: find.byType(Tooltip)),
      );
      expect(tooltip.message, longLine);
      expect(harness.lastSentWithCmd('config.apply'), isNull);
      await _unmount(tester);
    },
  );
}

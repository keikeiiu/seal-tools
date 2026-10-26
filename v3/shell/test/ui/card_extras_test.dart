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
    'the Buy card shows config presets and count, and writes nothing',
    (tester) async {
      final harness = await _pumpCards(
        tester,
        _config(
          buySell: {
            'presets': {
              'Herb': {'scroll': 0, 'row': 1, 'count': 3},
              'Potion': {'scroll': 0, 'row': 2, 'count': 5},
            },
            'sell_cap': 10,
          },
        ),
      );

      // Sorted preset names: Herb is the display default.
      expect(find.byKey(const ValueKey('buy-card-preset')), findsOneWidget);
      expect(find.text('count 3'), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('buy-card-preset')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('buy-card-item-Potion')));
      await tester.pumpAndSettle();
      expect(find.text('count 5'), findsOneWidget);

      // Display-only: changing the picker is not a config save and not a run argument.
      expect(harness.lastSentWithCmd('config.validate'), isNull);
      expect(harness.lastSentWithCmd('config.apply'), isNull);
      expect(
        harness.lastSentWithCmd('tool.start'),
        isNull,
        reason: 'the card never starts a tool by itself',
      );

      await tester.tap(find.byKey(const ValueKey('start-buy')));
      await _flush(tester);
      expect(harness.lastSentWithCmd('tool.start')!['args'], {'tool': 'buy'});

      // Answer the request: an unanswered command leaves the client's 10 s timeout timer pending.
      harness.respondOk(<String, dynamic>{});
      await _flush(tester);
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
}

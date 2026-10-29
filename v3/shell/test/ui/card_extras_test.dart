/// Card extras: the Buy card's display-only preset picker/count and the Sell card's per-run slot
/// selection (both PROTOCOL §12.6 run arguments — nothing persisted) and the Quest card's persisted
/// flow/sequence pickers.
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
  Map<String, dynamic>? spammer,
}) => {
  'window': {'title': 'TW_LIVE'},
  'buy_sell': ?buySell,
  'quest': ?quest,
  'spammer': ?spammer,
};

/// A minimal spammer block: two rotations, `buffs` active (v2's card falls back to the first when
/// `active` names nothing).
Map<String, dynamic> _spammerBlock() => {
  'active': 'buffs',
  'presets': {
    'buffs': {'1': 1.2},
    'core': {'2': 3.0},
  },
  'priority': {
    'buffs': ['1'],
    'core': ['2'],
  },
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
  // PLAN-V3-UI-UX demo parity: the tile face is one line and its options live behind the tune button.
  // These tests are about the controls themselves, so reveal them once here.
  for (final tool in const ['buy', 'spammer', 'quest']) {
    final options = find.byKey(ValueKey('options-$tool'));
    if (options.evaluate().isEmpty) {
      continue;
    }
    await tester.ensureVisible(options);
    await tester.tap(options);
    await tester.pump();
  }
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
    'a Sell start carries the per-run slot selection exactly, sorted',
    (tester) async {
      final harness = await _pumpCards(
        tester,
        _config(buySell: {'presets': <String, dynamic>{}, 'sell_cap': 10}),
      );

      // The Buy/Sell tab's grid writes these into the model; the card reads them when Start is pressed.
      harness.model.toggleSellSlot(7);
      harness.model.toggleSellSlot(3);

      await tester.tap(find.byKey(const ValueKey('start-sell')));
      await _flush(tester);
      expect(harness.lastSentWithCmd('tool.start')!['args'], {
        'tool': 'sell',
        'slots': [3, 7],
      });
      harness.respondOk(<String, dynamic>{});
      await _flush(tester);

      // A start does not clear the selection: the player may sell the same slots again.
      expect(harness.model.sellSlots, [3, 7]);
      await tester.tap(find.byKey(const ValueKey('start-sell')));
      await _flush(tester);
      expect(harness.lastSentWithCmd('tool.start')!['args'], {
        'tool': 'sell',
        'slots': [3, 7],
      });
      harness.respondOk(<String, dynamic>{});
      await _flush(tester);

      // A run argument, never a config write.
      expect(harness.lastSentWithCmd('config.validate'), isNull);
      expect(harness.lastSentWithCmd('config.apply'), isNull);
      await _unmount(tester);
    },
  );

  testWidgets(
    'an empty Sell selection omits the slots key and the core refusal renders',
    (tester) async {
      final harness = await _pumpCards(
        tester,
        _config(buySell: {'presets': <String, dynamic>{}, 'sell_cap': 10}),
      );

      await tester.tap(find.byKey(const ValueKey('start-sell')));
      await _flush(tester);
      final start = harness.lastSentWithCmd('tool.start')!;
      final args = start['args'] as Map<String, dynamic>;
      expect(args, {'tool': 'sell'});
      expect(
        args.containsKey('slots'),
        isFalse,
        reason: 'an empty selection is omitted, never sent as [] — §12.6',
      );

      harness.respondError(
        'bad_request',
        message: 'No slots are selected to sell.',
      );
      await _flush(tester);
      expect(find.text('No slots are selected to sell.'), findsOneWidget);
      expect(find.byKey(const ValueKey('refusal-sell')), findsOneWidget);
      expect(harness.lastSentWithCmd('config.apply'), isNull);
      await _unmount(tester);
    },
  );

  testWidgets(
    'the Spammer card picker shows the active rotation and saves exactly it',
    (tester) async {
      final harness = await _pumpCards(
        tester,
        _config(spammer: _spammerBlock()),
      );
      final picker = find.byKey(const ValueKey('spammer-card-preset'));
      expect(
        tester.widget<DropdownButton<String>>(picker).value,
        'buffs',
        reason: 'spammer.active is the selected rotation',
      );

      // A real core echoes the EFFECTIVE block (PROTOCOL §7), so the picker can follow the round
      // trip instead of guessing from what it sent.
      harness.onWrite = (line, fake) {
        final message = fake.sentJson(fake.sent.length - 1);
        final id = message['id'] as String;
        switch (message['cmd']) {
          case 'config.validate':
            fake.emitJson(
              okResponse(id, {'errors': <String>[], 'warnings': <String>[]}),
            );
          case 'config.apply':
            fake.emitJson(
              okResponse(id, {
                'rev': 8,
                'sections': {
                  'spammer': {..._spammerBlock(), 'active': 'core'},
                },
              }),
            );
        }
      };

      await tester.tap(picker);
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(const ValueKey('spammer-card-preset-item-core')),
      );
      await _flush(tester);

      final apply = harness.lastSentWithCmd('config.apply');
      expect(apply, isNotNull);
      expect((apply!['args'] as Map<String, dynamic>)['sections'], {
        'spammer': {'active': 'core'},
      }, reason: 'a picker change saves ONLY the active field');
      expect(
        tester.widget<DropdownButton<String>>(picker).value,
        'core',
        reason: 'the picker follows the saved mirror, not the click',
      );

      // There is no per-run spammer argument (§12.6): Start stays the plain command.
      await tester.tap(find.byKey(const ValueKey('start-spammer')));
      await _flush(tester);
      expect(harness.lastSentWithCmd('tool.start')!['args'], {
        'tool': 'spammer',
      });
      harness.respondOk(<String, dynamic>{});
      await _flush(tester);
      await _unmount(tester);
    },
  );

  testWidgets(
    'a refused Spammer rotation save renders the core line verbatim',
    (tester) async {
      final harness = await _pumpCards(
        tester,
        _config(spammer: _spammerBlock()),
      );
      final picker = find.byKey(const ValueKey('spammer-card-preset'));

      harness.answerConfigCommands(
        errors: ['the spammer preset "core" does not exist'],
      );
      await tester.tap(picker);
      await tester.pumpAndSettle();
      await tester.tap(
        find.byKey(const ValueKey('spammer-card-preset-item-core')),
      );
      await _flush(tester);

      expect(
        find.text('the spammer preset "core" does not exist'),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('spammer-card-error')), findsOneWidget);
      expect(
        harness.lastSentWithCmd('config.apply'),
        isNull,
        reason: 'a refused validate never applies',
      );
      expect(
        tester.widget<DropdownButton<String>>(picker).value,
        'buffs',
        reason: 'the refused attempt leaves the actual active rotation showing',
      );
      await _unmount(tester);
    },
  );

  testWidgets(
    'a v2-shaped quest config: the flow picker saves exactly the active field',
    (tester) async {
      // v2 quest steps carry a NUMERIC `value` (e.g. 1) while the wire type is a string; the core
      // fix accepts that config, and the card must list/save the presets without touching steps.
      final harness = await _pumpCards(
        tester,
        _config(
          quest: {
            'active': 'flowA',
            'active_sequence': '',
            'presets': {
              'flowA': {
                'steps': [
                  {'action': 'click', 'value': 1, 'delay_seconds': 0.5},
                ],
              },
              'flowB': {
                'steps': [
                  {'action': 'click', 'value': 1, 'delay_seconds': 0.5},
                ],
              },
            },
            'sequences': {
              'seq1': {'entries': <Object?>[]},
            },
          },
        ),
      );

      expect(
        tester
            .widget<DropdownButton<String>>(
              find.byKey(const ValueKey('quest-card-flow')),
            )
            .value,
        'flowA',
        reason: 'quest.active is the selected preset',
      );

      harness.answerConfigCommands();
      await tester.tap(find.byKey(const ValueKey('quest-card-flow')));
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey('quest-card-flow-item-flowA')),
        findsWidgets,
      );
      expect(
        find.byKey(const ValueKey('quest-card-flow-item-flowB')),
        findsWidgets,
      );
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
            'flowA': {
              'steps': [
                {'action': 'click', 'value': 1, 'delay_seconds': 0.5},
              ],
            },
            'flowB': {
              'steps': [
                {'action': 'click', 'value': 1, 'delay_seconds': 0.5},
              ],
            },
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

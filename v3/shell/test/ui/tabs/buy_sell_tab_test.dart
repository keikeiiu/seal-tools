/// The Buy/Sell tab: renders the `buy_sell` block, edits its presets and sell cap through the
/// form's Save, and edits the 64-slot sell selection through its own `Save selection` — which sends
/// EXACTLY `{buy_sell: {sell_slots: [...]}}` and never the calibrated geometry.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/ui/tabs/buy_sell_tab.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

Map<String, dynamic> _buySellBlock() => {
  'bag_grid': [100, 200, 300, 400],
  'bag_slot': [40, 40, 40, 40],
  'shop_rows': 8,
  'shop_region': [500, 300, 320, 480],
  'scroll_point': [660, 540],
  'max_button': [400, 500],
  'presets': {
    'potion': {'scroll': 1, 'row': 2, 'count': 3},
    'scroll': {'scroll': 0, 'row': 5, 'count': 1},
  },
  'sell_slots': [0, 1, 2],
  'sell_cap': 20,
};

Finder _field(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(TextField),
);

Future<AppHarness> _pumpBuySell(
  WidgetTester tester, {
  Map<String, dynamic>? block,
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
      'buy_sell': block ?? _buySellBlock(),
    },
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(body: BuySellTab(model: harness.model)),
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

/// Answers the selection save the way the real core does: a clean validate, then an apply echo of
/// the EFFECTIVE `buy_sell` block (PROTOCOL §7) — presets and geometry included, not just the
/// `sell_slots` the request carried.
void _answerSelectionSave(
  AppHarness harness, {
  required List<int> slots,
  int rev = 8,
}) {
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
            'rev': rev,
            'sections': {
              'buy_sell': {..._buySellBlock(), 'sell_slots': slots},
            },
          }),
        );
      default:
        break;
    }
  };
}

/// Taps one bag cell and pumps twice: the toggle notifies the selection draft, whose
/// `ListenableBuilder` rebuilds on the first frame and settles on the second.
Future<void> _tapCell(WidgetTester tester, int slot) async {
  await tester.tap(find.byKey(ValueKey('sell-cell-$slot')));
  await tester.pump();
  await tester.pump();
}

/// The selection's Save button, read straight from the tree: disabled while the selection is
/// clean, enabled once it is dirty.
FilledButton _saveSelectionButton(WidgetTester tester) {
  return tester.widget<FilledButton>(
    find.byKey(const ValueKey('save-sell-selection')),
  );
}

/// The fill colour of one bag cell: the visible difference between selected and unselected.
Color? _cellColor(WidgetTester tester, int slot) {
  final container = tester.widget<Container>(
    find.descendant(
      of: find.byKey(ValueKey('sell-cell-$slot')),
      matching: find.byType(Container),
    ),
  );
  final decoration = container.decoration;
  return decoration is BoxDecoration ? decoration.color : null;
}

/// The colours the tab actually renders with (the app theme, not a fresh copy of it).
ColorScheme _scheme(WidgetTester tester) {
  return Theme.of(tester.element(find.byType(BuySellTab))).colorScheme;
}

void main() {
  testWidgets('renders the block and saves ONLY the owned buy_sell fields', (
    tester,
  ) async {
    final harness = await _pumpBuySell(tester);

    // The calibrated geometry stays read-only; the sell slot selection is the Sell section's.
    expect(
      find.textContaining('Calibrated geometry — read-only'),
      findsOneWidget,
    );
    expect(find.text('[660, 540]'), findsOneWidget);
    expect(find.textContaining('potion'), findsWidgets);

    await tester.enterText(_field('buy-count'), '9');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-buy_sell')), findsOneWidget);

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-buy_sell')));
    await _flush(tester);

    final apply = harness.lastSentWithCmd('config.apply');
    expect(apply, isNotNull);
    final args = apply!['args'] as Map<String, dynamic>;
    final sections = args['sections'] as Map<String, dynamic>;
    expect(sections.keys, ['buy_sell'], reason: 'an owned top-level block');
    final block = sections['buy_sell'] as Map<String, dynamic>;
    expect(
      (block['presets'] as Map<String, dynamic>)['potion'],
      containsPair('count', 9),
    );
    expect(block.containsKey('bag_grid'), isFalse);
    expect(block.containsKey('scroll_point'), isFalse);
    expect(
      block.containsKey('sell_slots'),
      isFalse,
      reason: 'the selection has its own draft and its own save',
    );
    expect(
      block.containsKey('sell_cap'),
      isTrue,
      reason: 'the cap is an owned field and rides the same block',
    );

    expect(find.byKey(const ValueKey('dirty-buy_sell')), findsNothing);
    expect(find.byKey(const ValueKey('save-ok-buy_sell')), findsOneWidget);
  });

  testWidgets('renders the core validation lines verbatim and does not apply', (
    tester,
  ) async {
    final harness = await _pumpBuySell(tester);
    await tester.enterText(_field('buy-count'), '9');
    await tester.pump();

    harness.answerConfigCommands(
      errors: ['the sell cap must be at least one slot'],
    );
    await tester.tap(find.byKey(const ValueKey('save-buy_sell')));
    await _flush(tester);

    expect(find.text('the sell cap must be at least one slot'), findsOneWidget);
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    expect(find.byKey(const ValueKey('dirty-buy_sell')), findsOneWidget);
  });

  testWidgets('Cancel restores the mirror value', (tester) async {
    await _pumpBuySell(tester);
    await tester.enterText(_field('buy-count'), '9');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-buy_sell')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('cancel-buy_sell')));
    await tester.pump();

    expect(find.byKey(const ValueKey('dirty-buy_sell')), findsNothing);
    expect(tester.widget<TextField>(_field('buy-count')).controller?.text, '3');
  });

  testWidgets(
    'the sell grid renders the configured selection, count and pitch',
    (tester) async {
      await _pumpBuySell(tester);

      expect(find.text('3 of 64 slots selected'), findsOneWidget);
      expect(find.byKey(const ValueKey('sell-cell-0')), findsOneWidget);
      expect(find.byKey(const ValueKey('sell-cell-1')), findsOneWidget);
      expect(find.byKey(const ValueKey('sell-cell-63')), findsOneWidget);
      // The pitch comes from the calibrated bag grid (300/8 × 400/8).
      expect(find.textContaining('bag pitch 37.5×50.0 px'), findsOneWidget);
      // Selected cells are visibly distinct from the unselected ones.
      expect(_cellColor(tester, 0), _scheme(tester).primary);
      expect(_cellColor(tester, 63), _scheme(tester).surfaceContainerHighest);
      expect(
        find.byKey(const ValueKey('sell-grid-not-calibrated')),
        findsNothing,
      );
      // Nothing is dirty yet, so there is nothing to save.
      expect(_saveSelectionButton(tester).onPressed, isNull);
      expect(find.byKey(const ValueKey('sell-selection-dirty')), findsNothing);
    },
  );

  testWidgets('tapping a cell toggles it and marks the selection dirty', (
    tester,
  ) async {
    await _pumpBuySell(tester);

    // Slot 0 was configured: tapping it deselects it.
    await _tapCell(tester, 0);
    expect(find.text('2 of 64 slots selected'), findsOneWidget);

    // Slot 63 was not: tapping it selects it.
    await _tapCell(tester, 63);
    expect(find.text('3 of 64 slots selected'), findsOneWidget);
    expect(find.byKey(const ValueKey('sell-selection-dirty')), findsOneWidget);
    expect(_saveSelectionButton(tester).onPressed, isNotNull);

    // Discard puts the core's selection back without touching the form's draft.
    await tester.tap(find.byKey(const ValueKey('discard-sell-selection')));
    await tester.pump();
    await tester.pump();
    expect(find.text('3 of 64 slots selected'), findsOneWidget);
    expect(find.byKey(const ValueKey('sell-selection-dirty')), findsNothing);
    expect(find.byKey(const ValueKey('dirty-buy_sell')), findsNothing);
    expect(_cellColor(tester, 0), _scheme(tester).primary);
    expect(_cellColor(tester, 63), _scheme(tester).surfaceContainerHighest);
  });

  testWidgets(
    'Save selection sends exactly buy_sell.sell_slots and re-seeds the draft',
    (tester) async {
      final harness = await _pumpBuySell(tester);
      await _tapCell(tester, 0);
      await _tapCell(tester, 63);
      _answerSelectionSave(harness, slots: [1, 2, 63]);

      await tester.tap(find.byKey(const ValueKey('save-sell-selection')));
      await _flush(tester);

      // The exact payload: no preset, no cap, no geometry — only the clicked indexes.
      final apply = harness.lastSentWithCmd('config.apply');
      expect(apply, isNotNull);
      final args = apply!['args'] as Map<String, dynamic>;
      final sections = args['sections'] as Map<String, dynamic>;
      expect(sections, {
        'buy_sell': {
          'sell_slots': [1, 2, 63],
        },
      });
      expect(sections.keys, ['buy_sell']);
      expect((sections['buy_sell'] as Map<String, dynamic>).keys, [
        'sell_slots',
      ]);
      // The selection save is a normal config save: validate first, then apply (PROTOCOL §7).
      expect(harness.lastSentWithCmd('config.validate'), isNotNull);

      // The draft re-seeded from the effective block, so the dirty marker is gone and the grid
      // still shows the configured cells.
      expect(find.byKey(const ValueKey('sell-selection-dirty')), findsNothing);
      expect(find.byKey(const ValueKey('sell-selection-ok')), findsOneWidget);
      expect(find.text('Selection saved — rev 8'), findsOneWidget);
      expect(find.text('3 of 64 slots selected'), findsOneWidget);
      expect(find.byKey(const ValueKey('dirty-buy_sell')), findsNothing);
    },
  );

  testWidgets(
    'saving the selection keeps an unsaved form edit and raises no conflict',
    (tester) async {
      final harness = await _pumpBuySell(tester);
      await tester.enterText(_field('buy-count'), '9');
      await tester.pump();
      expect(find.byKey(const ValueKey('dirty-buy_sell')), findsOneWidget);

      await _tapCell(tester, 63);
      _answerSelectionSave(harness, slots: [0, 1, 2, 63]);
      await tester.tap(find.byKey(const ValueKey('save-sell-selection')));
      await _flush(tester);

      // The form edit survives the selection's rev bump...
      expect(find.byKey(const ValueKey('dirty-buy_sell')), findsOneWidget);
      expect(
        tester.widget<TextField>(_field('buy-count')).controller?.text,
        '9',
      );
      // ...and the rev bump is not mistaken for an external writer.
      expect(
        find.byKey(const ValueKey('external-change-buy_sell')),
        findsNothing,
      );
      expect(find.byKey(const ValueKey('conflict-buy_sell')), findsNothing);
      expect(find.byKey(const ValueKey('save-ok-buy_sell')), findsNothing);
      expect(find.byKey(const ValueKey('sell-selection-dirty')), findsNothing);
    },
  );

  testWidgets('a refused selection save renders the core line verbatim', (
    tester,
  ) async {
    final harness = await _pumpBuySell(tester);
    await _tapCell(tester, 63);

    harness.answerConfigCommands(conflict: true);
    await tester.tap(find.byKey(const ValueKey('save-sell-selection')));
    await _flush(tester);

    expect(find.text('config changed while you were editing'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('sell-selection-error-0')),
      findsOneWidget,
    );
    expect(harness.lastSentWithCmd('config.apply'), isNotNull);
    // A refusal is not a write: the selection stays dirty, keeps its value, and can be retried.
    expect(find.byKey(const ValueKey('sell-selection-dirty')), findsOneWidget);
    expect(find.text('4 of 64 slots selected'), findsOneWidget);
    expect(_saveSelectionButton(tester).onPressed, isNotNull);
    expect(find.byKey(const ValueKey('sell-selection-ok')), findsNothing);
  });

  testWidgets(
    'the sell grid is disabled with the core line when the bag grid is missing',
    (tester) async {
      await _pumpBuySell(
        tester,
        block: {'presets': <String, dynamic>{}, 'sell_cap': 5},
      );

      expect(
        find.text(
          "The bag grid isn't calibrated — save the Bag grid on the Calibrate "
          'tab.',
        ),
        findsOneWidget,
      );
      expect(
        find.byKey(const ValueKey('sell-grid-not-calibrated')),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('sell-grid-count')), findsNothing);
      expect(find.byKey(const ValueKey('sell-cell-0')), findsNothing);
      expect(find.byKey(const ValueKey('save-sell-selection')), findsNothing);
    },
  );

  testWidgets('a zero-size bag grid is uncalibrated too, like the core rule', (
    tester,
  ) async {
    await _pumpBuySell(
      tester,
      block: {
        'bag_grid': [100, 200, 0, 400],
        'presets': <String, dynamic>{},
        'sell_cap': 5,
      },
    );

    expect(
      find.byKey(const ValueKey('sell-grid-not-calibrated')),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('sell-cell-0')), findsNothing);
  });
}

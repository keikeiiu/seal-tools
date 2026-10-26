/// The Buy/Sell tab: renders the `buy_sell` block, edits its presets and sell cap, and saves ONLY
/// that owned block (never the calibrated geometry).
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

void main() {
  testWidgets('renders the block and saves ONLY the owned buy_sell fields', (
    tester,
  ) async {
    final harness = await _pumpBuySell(tester);

    // The calibrated geometry is labelled read-only (focus point = the scroll parking point).
    expect(find.textContaining('read-only this wave'), findsOneWidget);
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
    final sections =
        (apply!['args'] as Map<String, dynamic>)['sections']
            as Map<String, dynamic>;
    expect(sections.keys, ['buy_sell'], reason: 'an owned top-level block');
    final block = sections['buy_sell'] as Map<String, dynamic>;
    expect(
      (block['presets'] as Map<String, dynamic>)['potion'],
      containsPair('count', 9),
    );
    expect(block.containsKey('bag_grid'), isFalse);
    expect(block.containsKey('scroll_point'), isFalse);
    expect(block.containsKey('sell_slots'), isFalse);
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

  testWidgets('the sell grid renders the configured cells and count', (
    tester,
  ) async {
    await _pumpBuySell(tester);

    expect(find.text('3 of 64 slots configured'), findsOneWidget);
    expect(find.byKey(const ValueKey('sell-cell-0')), findsOneWidget);
    expect(find.byKey(const ValueKey('sell-cell-1')), findsOneWidget);
    expect(find.byKey(const ValueKey('sell-cell-63')), findsOneWidget);
    // The pitch comes from the calibrated bag grid (300/8 × 400/8).
    expect(find.textContaining('bag pitch 37.5×50.0 px'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('sell-grid-not-calibrated')),
      findsNothing,
    );
  });

  testWidgets(
    'the sell grid says not calibrated yet when the config lacks it',
    (tester) async {
      await _pumpBuySell(
        tester,
        block: {'presets': <String, dynamic>{}, 'sell_cap': 5},
      );

      expect(
        find.byKey(const ValueKey('sell-grid-not-calibrated')),
        findsOneWidget,
      );
      expect(find.text('Sell grid: not calibrated yet'), findsOneWidget);
      expect(find.byKey(const ValueKey('sell-grid-count')), findsNothing);
    },
  );
}

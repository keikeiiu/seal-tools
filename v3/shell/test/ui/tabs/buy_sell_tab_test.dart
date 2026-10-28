/// The Buy/Sell tab: the form's `buy_sell` half (presets + sell cap) saves only those fields, and the
/// 8×8 sell grid is PER-RUN state — toggling updates [ShellModel]'s in-memory selection, never config
/// (PROTOCOL §12.6: the selection can differ every sell).
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
  // A v2-era/v3.0.1 leftover: the tab must NOT treat this as the run selection any more.
  'sell_slots': [0, 1, 2],
  'sell_cap': 20,
};

Finder _field(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(TextField),
);

Widget _tab(AppHarness harness) => MaterialApp(
  theme: buildShellTheme(),
  home: Scaffold(body: BuySellTab(model: harness.model)),
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
  await tester.pumpWidget(_tab(harness));
  await tester.pump();
  return harness;
}

Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

/// Taps one bag cell and pumps twice: the toggle mutates the model, then `setState` redraws the grid.
Future<void> _tapCell(WidgetTester tester, int slot) async {
  await tester.tap(find.byKey(ValueKey('sell-cell-$slot')));
  await tester.pump();
  await tester.pump();
}

/// True when the shell has sent no config command at all — the toggle is memory-only.
bool _noConfigCommands(AppHarness harness) => harness.sentMessages.every(
  (message) => !(message['cmd'] as String? ?? '').startsWith('config.'),
);

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

    // The calibrated geometry stays read-only; the sell selection is per-run state, not config.
    expect(
      find.textContaining('Calibrated geometry — read-only'),
      findsOneWidget,
    );
    expect(find.text('[660, 540]'), findsOneWidget);
    expect(find.textContaining('potion'), findsWidgets);
    expect(find.text('0 of 64 slots selected'), findsOneWidget);

    await tester.enterText(_field('buy-count'), '9');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-buy_sell')), findsOneWidget);

    // A per-run slot selection does not ride the form's Save either.
    await _tapCell(tester, 63);
    expect(find.text('1 of 64 slots selected'), findsOneWidget);

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
      reason: 'the selection is a per-run tool.start argument, never config',
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
    'the sell grid is per-run: toggling changes memory, never config',
    (tester) async {
      final harness = await _pumpBuySell(tester);

      // A migrated/persisted `sell_slots` leftover is NOT the run selection any more.
      expect(find.text('0 of 64 slots selected'), findsOneWidget);
      expect(
        find.text('This selection is for the next run only — it is not saved.'),
        findsOneWidget,
      );
      // The grid and its header are still there; the save machinery is gone.
      expect(find.byKey(const ValueKey('sell-grid-pitch')), findsOneWidget);
      expect(find.textContaining('bag pitch 37.5×50.0 px'), findsOneWidget);
      expect(find.byKey(const ValueKey('save-sell-selection')), findsNothing);
      expect(find.byKey(const ValueKey('sell-selection-dirty')), findsNothing);

      await _tapCell(tester, 0);
      expect(find.text('1 of 64 slots selected'), findsOneWidget);
      expect(harness.model.sellSlots, [0]);
      await _tapCell(tester, 63);
      expect(find.text('2 of 64 slots selected'), findsOneWidget);
      expect(harness.model.sellSlots, [0, 63]);

      // Selected cells are visibly distinct, and none of this is a config write.
      expect(_cellColor(tester, 0), _scheme(tester).primary);
      expect(_cellColor(tester, 63), _scheme(tester).primary);
      expect(_cellColor(tester, 1), _scheme(tester).surfaceContainerHighest);
      expect(_noConfigCommands(harness), isTrue);

      // Toggling off removes it again, still without any config command.
      await _tapCell(tester, 0);
      expect(find.text('1 of 64 slots selected'), findsOneWidget);
      expect(harness.model.sellSlots, [63]);
      expect(_noConfigCommands(harness), isTrue);
    },
  );

  testWidgets('the sell selection survives leaving and re-entering the tab', (
    tester,
  ) async {
    final harness = await _pumpBuySell(tester);
    await _tapCell(tester, 5);
    expect(harness.model.sellSlots, [5]);

    // Switching tabs destroys the tab's State; the selection lives in the model for the process.
    await tester.pumpWidget(const SizedBox());
    await tester.pump();
    await tester.pumpWidget(_tab(harness));
    await tester.pump();

    expect(find.text('1 of 64 slots selected'), findsOneWidget);
    expect(harness.model.sellSlots, [5]);
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

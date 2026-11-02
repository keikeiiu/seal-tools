/// The Buy/Sell tab: the form's `buy_sell` half (presets + sell cap) saves only those fields, and the
/// 8×8 sell grid is PER-RUN state — toggling updates [ShellModel]'s in-memory selection, never config
/// (PROTOCOL §12.6: the selection can differ every sell). The grid carries v2's row shortcuts, the
/// Select all / Clear bulk actions and the cap status line.
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

/// Taps one keyed control and pumps twice: a selection change mutates the model, then `setState`
/// redraws the grid, the cap status line and the count header.
Future<void> _tap(WidgetTester tester, String key) async {
  await tester.tap(find.byKey(ValueKey(key)));
  await tester.pump();
  await tester.pump();
}

Future<void> _tapCell(WidgetTester tester, int slot) =>
    _tap(tester, 'sell-cell-$slot');

/// The status line below the grid (v2 `PaintSellSlots`): its text and its colour.
Text _statusWidget(WidgetTester tester) {
  return tester.widget<Text>(
    find.byKey(const ValueKey('sell-selection-status')),
  );
}

/// The current value of a [DraftDropdownField] (its inner form field, not the wrapper).
String? _dropdownValue(WidgetTester tester, String key) {
  final field = tester.widget<DropdownButtonFormField<String>>(
    find.descendant(
      of: find.byKey(ValueKey(key)),
      matching: find.byType(DropdownButtonFormField<String>),
    ),
  );
  return field.initialValue;
}

/// Picks [option] from a [DraftDropdownField]'s open menu.
Future<void> _pick(WidgetTester tester, String key, String option) async {
  await tester.tap(find.byKey(ValueKey(key)));
  await tester.pumpAndSettle();
  await tester.tap(find.text(option).last);
  await tester.pumpAndSettle();
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

  testWidgets('a row shortcut selects and clears exactly that bag row', (
    tester,
  ) async {
    final harness = await _pumpBuySell(tester);

    await _tap(tester, 'sell-row-0');
    expect(find.text('8 of 64 slots selected'), findsOneWidget);
    expect(harness.model.sellSlots, [0, 1, 2, 3, 4, 5, 6, 7]);
    expect(_cellColor(tester, 0), _scheme(tester).primary);
    expect(_cellColor(tester, 7), _scheme(tester).primary);
    expect(_cellColor(tester, 8), _scheme(tester).surfaceContainerHighest);

    // Pressing it again clears the same row (v2's ToggleSellRow).
    await _tap(tester, 'sell-row-0');
    expect(find.text('0 of 64 slots selected'), findsOneWidget);
    expect(harness.model.sellSlots, isEmpty);

    // A partially selected row is COMPLETED, not cleared.
    await _tapCell(tester, 9);
    await _tap(tester, 'sell-row-1');
    expect(find.text('8 of 64 slots selected'), findsOneWidget);
    expect(harness.model.sellSlots, [8, 9, 10, 11, 12, 13, 14, 15]);

    // v2's tooltips: the row button names the row, each cell names its 1-based slot.
    final rowTooltip = tester.widget<Tooltip>(
      find.byKey(const ValueKey('sell-row-0')),
    );
    expect(rowTooltip.message, 'Select or clear all 8 slots of bag row 1');
    final cellTooltip = tester.widget<Tooltip>(
      find.descendant(
        of: find.byKey(const ValueKey('sell-cell-0')),
        matching: find.byType(Tooltip),
      ),
    );
    expect(cellTooltip.message, 'Bag slot 1');
    expect(_noConfigCommands(harness), isTrue);
  });

  testWidgets('Select all selects the 64 cells and Clear empties them', (
    tester,
  ) async {
    final harness = await _pumpBuySell(tester);

    await _tap(tester, 'sell-select-all');
    expect(find.text('64 of 64 slots selected'), findsOneWidget);
    expect(harness.model.sellSlots, hasLength(64));
    expect(harness.model.sellSlots.first, 0);
    expect(harness.model.sellSlots.last, 63);

    await _tap(tester, 'sell-clear');
    expect(find.text('0 of 64 slots selected'), findsOneWidget);
    expect(harness.model.sellSlots, isEmpty);
    expect(_noConfigCommands(harness), isTrue);
  });

  testWidgets(
    'the status line has v2\'s three states and follows the cap live',
    (tester) async {
      final harness = await _pumpBuySell(tester);

      expect(
        _statusWidget(tester).data,
        'No slots selected — click the grid to choose what to sell.',
      );
      expect(_statusWidget(tester).style?.color, isNot(_scheme(tester).error));

      await _tapCell(tester, 0);
      expect(_statusWidget(tester).data, '1 slot(s) selected (cap 20).');

      // The cap is the form's own field; the warning follows it live, including going over.
      await tester.enterText(_field('sell-cap'), '1');
      await tester.pump();
      await _tapCell(tester, 1);
      expect(
        _statusWidget(tester).data,
        '2 slots selected, which is over the cap of 1. Raise the cap on '
        'purpose, or narrow the selection — the run refuses either way.',
      );
      expect(_statusWidget(tester).style?.color, _scheme(tester).error);

      // Raising the cap clears the warning.
      await tester.enterText(_field('sell-cap'), '5');
      await tester.pump();
      expect(_statusWidget(tester).data, '2 slot(s) selected (cap 5).');
      expect(_statusWidget(tester).style?.color, isNot(_scheme(tester).error));
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

  testWidgets('the buy row picker labels 1-based and stores the 0-based row', (
    tester,
  ) async {
    final harness = await _pumpBuySell(tester);

    // The fixture preset stores row 2 (0-based) → the picker shows "Row 3", and there is no free
    // number field left to type an index into.
    expect(_field('buy-row'), findsNothing);
    expect(_dropdownValue(tester, 'buy-row'), 'Row 3');

    await _pick(tester, 'buy-row', 'Row 1');
    expect(_dropdownValue(tester, 'buy-row'), 'Row 1');

    await tester.enterText(_field('buy-count'), '5');
    await tester.pump();
    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-buy_sell')));
    await _flush(tester);

    final apply = harness.lastSentWithCmd('config.apply')!;
    final sections =
        (apply['args'] as Map<String, dynamic>)['sections']
            as Map<String, dynamic>;
    final savedBlock = sections['buy_sell'] as Map<String, dynamic>;
    final presets = savedBlock['presets'] as Map<String, dynamic>;
    final potion = presets['potion'] as Map<String, dynamic>;
    expect(potion['row'], 0, reason: 'Row 1 must store the 0-based index 0');
    expect(potion['scroll'], 1, reason: 'the picker only touches row');
  });

  testWidgets(
    'a stored row past the calibrated list is marked and cannot be picked',
    (tester) async {
      final block = _buySellBlock();
      final presets = block['presets'] as Map<String, dynamic>;
      final potion = presets['potion'] as Map<String, dynamic>;
      potion['row'] = 12;
      final harness = await _pumpBuySell(tester, block: block);

      expect(
        _dropdownValue(tester, 'buy-row'),
        'Row 13 — past the bottom of the list',
        reason: 'a legacy out-of-range row is shown, never blanked',
      );
      expect(
        find.textContaining('Row 13 is past the bottom of the list — only 8'),
        findsOneWidget,
      );

      // Picking a valid row stores its 0-based index and clears the warning.
      await _pick(tester, 'buy-row', 'Row 8');
      expect(find.byKey(const ValueKey('buy-row-warning')), findsNothing);
      expect(_dropdownValue(tester, 'buy-row'), 'Row 8');

      harness.answerConfigCommands();
      await tester.tap(find.byKey(const ValueKey('save-buy_sell')));
      await _flush(tester);
      final apply = harness.lastSentWithCmd('config.apply')!;
      final sections =
          (apply['args'] as Map<String, dynamic>)['sections']
              as Map<String, dynamic>;
      final blockSaved = sections['buy_sell'] as Map<String, dynamic>;
      final savedPresets = blockSaved['presets'] as Map<String, dynamic>;
      final savedPotion = savedPresets['potion'] as Map<String, dynamic>;
      expect(
        savedPotion['row'],
        7,
        reason: 'Row 8 must store the 0-based index 7',
      );
    },
  );
}

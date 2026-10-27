/// The Pet tab: the feeder's setup editors (rows, queue, food cells, return slot, mode and timing)
/// plus the portable `settings.pet` fallbacks, saved together as ONE `config.apply` — with the
/// calibrated marks and reference crops displayed read-only and never sent.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/ui/tabs/pet_tab.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

Map<String, dynamic> _portablePet() => {
  'items_per_minute': 3,
  'pet_slot_occupied_above': 0.05,
};

Map<String, dynamic> _machinePet() => {
  ..._portablePet(),
  'menu_button': [1, 2],
  'feed_icon': [3, 4],
  'close_button': [5, 6],
  'close_button_png': 'iVBORw0KGgo=',
  'food_icon_png': 'iVBORw0KGgo=',
  'page_tabs': [
    [10, 10],
    [20, 10],
    [30, 10],
  ],
  'bag_grid': [100, 200, 300, 400],
  'bag_slot': [40, 40, 40, 40],
  'food_slots': [
    [1, 2],
    [1, 3],
  ],
  'food_slots_used': 0,
  'action_wait_ms': 1200,
  'wait_after_empty_minutes': 5,
  'feeder_count_left_fraction': 0.5,
  'feeder_count_min_score': 0.4,
  'reload_on_start': false,
  'food_load_mode': 'right_click',
  'return_slot': [1, 4],
  'slots': [
    {
      'enabled': true,
      'toggle_label': [10, 20, 100, 30],
      'boarding_pet_slot': [10, 60, 100, 80],
      'feeder_strip': [110, 20, 200, 80],
      'stacks': 5,
      'pet_slot_empty_png': 'iVBORw0KGgo=',
      'boarding_running': false,
    },
    {
      'enabled': false,
      'toggle_label': [10, 200, 100, 30],
      'boarding_pet_slot': [10, 240, 100, 80],
      'feeder_strip': [110, 200, 200, 80],
      'stacks': 2,
      'boarding_running': true,
    },
  ],
  'queue': [
    {
      'label': 'pet A',
      'species': 'Knight',
      'rect': [0, 0, 32, 32],
    },
    {
      'label': 'pet B',
      'species': 'Mage',
      'rect': [32, 0, 32, 32],
    },
  ],
};

Finder _field(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(TextField),
);

Future<AppHarness> _pumpPet(
  WidgetTester tester, {
  Map<String, dynamic>? pet,
}) async {
  tester.view.physicalSize = const ui.Size(1600, 3400);
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
      'pet': pet ?? _portablePet(),
    },
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(body: PetTab(model: harness.model)),
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

/// Saves and returns the `sections` the tab sent.
Future<Map<String, dynamic>> _save(
  WidgetTester tester,
  AppHarness harness,
) async {
  harness.answerConfigCommands();
  await tester.tap(find.byKey(const ValueKey('save-pet')));
  await _flush(tester);
  final apply = harness.lastSentWithCmd('config.apply');
  expect(apply, isNotNull);
  final args = apply!['args'] as Map<String, dynamic>;
  return args['sections'] as Map<String, dynamic>;
}

Future<void> _tap(WidgetTester tester, String key) async {
  final finder = find.byKey(ValueKey(key));
  // The Pet tab's editors run past one screen at the default test surface: scroll the control into
  // view so the tap lands on it rather than on whatever is painted at that offset.
  await tester.ensureVisible(finder);
  await tester.pump();
  await tester.tap(finder);
  await tester.pump();
  await tester.pump();
}

Future<void> _tapSwitch(WidgetTester tester, String key) async {
  final finder = find.descendant(
    of: find.byKey(ValueKey(key)),
    matching: find.byType(Switch),
  );
  await tester.ensureVisible(finder);
  await tester.pump();
  await tester.tap(finder);
  await tester.pump();
  await tester.pump();
}

/// The food-cell count line as rendered, so a missed toggle reports the actual text.
String? _countText(WidgetTester tester) {
  final line = tester.widget<Text>(
    find.byKey(const ValueKey('pet-food-count')),
  );
  return line.data;
}

void main() {
  testWidgets(
    'the dev shape has no rows, disables food grid, saves settings.pet',
    (tester) async {
      final harness = await _pumpPet(tester);

      // No machine rows/crops: the committed dev config is an explicit state, not a blank.
      expect(find.byKey(const ValueKey('pet-no-rows')), findsOneWidget);
      expect(find.byKey(const ValueKey('pet-queue-empty')), findsOneWidget);
      expect(
        find.text(
          "The boarding bag's grid isn't calibrated — save the Boarding bag grid "
          'and Boarding bag slot on the Calibrate tab. (Its own, not the shop\'s.)',
        ),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('pet-food-page-0')), findsNothing);
      final rateField = tester.widget<TextField>(
        _field('pet-items-per-minute'),
      );
      expect(rateField.controller?.text, '3');

      await tester.enterText(_field('pet-items-per-minute'), '5');
      await tester.pump();
      expect(find.byKey(const ValueKey('dirty-pet')), findsOneWidget);

      final sections = await _save(tester, harness);
      expect(sections.keys, [
        'settings',
      ], reason: 'there is no machine pet block to send');
      final settings = sections['settings'] as Map<String, dynamic>;
      final pet = settings['pet'] as Map<String, dynamic>;
      expect(pet['items_per_minute'], 5);
      expect(pet['pet_slot_occupied_above'], 0.05);
      expect(pet.containsKey('slots'), isFalse);
      expect(find.byKey(const ValueKey('save-ok-pet')), findsOneWidget);
    },
  );

  testWidgets(
    'rows: the tick and stacks save, and the read-only marks/crops ride along',
    (tester) async {
      final harness = await _pumpPet(tester, pet: _machinePet());

      expect(find.textContaining('start/end [10, 20, 100, 30]'), findsWidgets);
      expect(find.textContaining('empty-slot crop saved'), findsOneWidget);
      expect(find.textContaining('boarding running'), findsOneWidget);
      expect(find.byKey(const ValueKey('pet-rows-caption')), findsOneWidget);

      await _tapSwitch(tester, 'pet-row-0-enabled');
      await tester.enterText(_field('pet-row-0-stacks'), '3');
      await tester.pump();
      expect(find.byKey(const ValueKey('dirty-pet')), findsOneWidget);

      final sections = await _save(tester, harness);
      expect(sections.keys, ['pet', 'settings']);
      final pet = sections['pet'] as Map<String, dynamic>;
      final slots = (pet['slots'] as List).cast<Map<String, dynamic>>();
      expect(slots, hasLength(2));
      expect(slots[0]['enabled'], false);
      expect(slots[0]['stacks'], 3);
      // The fields this tab does not own are echoed back exactly, not dropped.
      expect(slots[0]['toggle_label'], [10, 20, 100, 30]);
      expect(slots[0]['boarding_pet_slot'], [10, 60, 100, 80]);
      expect(slots[0]['feeder_strip'], [110, 20, 200, 80]);
      expect(slots[0]['pet_slot_empty_png'], 'iVBORw0KGgo=');
      expect(slots[0]['boarding_running'], false);
      expect(slots[1]['enabled'], false);
      expect(slots[1]['stacks'], 2);
      for (final key in [
        'menu_button',
        'bag_grid',
        'bag_slot',
        'page_tabs',
        'close_button_png',
        'food_slots_used',
        'max_button',
      ]) {
        expect(
          pet.containsKey(key),
          isFalse,
          reason: '$key is calibration/session state, not setup',
        );
      }
      final settings = sections['settings'] as Map<String, dynamic>;
      final portable = settings['pet'] as Map<String, dynamic>;
      expect(
        portable['items_per_minute'],
        3,
        reason: 'the portable half rides the same save',
      );
    },
  );

  testWidgets('removing a queue entry saves the shortened queue', (
    tester,
  ) async {
    final harness = await _pumpPet(tester, pet: _machinePet());
    expect(find.byKey(const ValueKey('pet-queue-entry-1')), findsOneWidget);
    expect(find.byKey(const ValueKey('pet-queue-caption')), findsOneWidget);

    await _tap(tester, 'pet-queue-remove-0');

    final sections = await _save(tester, harness);
    final pet = sections['pet'] as Map<String, dynamic>;
    expect(pet['queue'], [
      {
        'label': 'pet B',
        'species': 'Mage',
        'rect': [32, 0, 32, 32],
      },
    ]);
  });

  testWidgets(
    'food cells save as [page, cell] pairs and never write food_slots_used',
    (tester) async {
      final harness = await _pumpPet(tester, pet: _machinePet());

      expect(
        _countText(tester),
        '0 of 64 cells marked on this page · 2 marked overall',
      );
      await _tap(tester, 'pet-food-cell-0-0');
      expect(
        _countText(tester),
        '1 of 64 cells marked on this page · 3 marked overall',
      );

      await _tap(tester, 'pet-food-page-1');
      expect(find.byKey(const ValueKey('pet-food-cell-1-5')), findsOneWidget);
      await _tap(tester, 'pet-food-cell-1-5');

      final sections = await _save(tester, harness);
      final pet = sections['pet'] as Map<String, dynamic>;
      expect(pet['food_slots'], [
        [0, 0],
        [1, 2],
        [1, 3],
        [1, 5],
      ]);
      expect(
        pet.containsKey('food_slots_used'),
        isFalse,
        reason:
            'the run reads the starting used-count; it is not a count we own',
      );
    },
  );

  testWidgets('the scalars and the load mode ride the same save', (
    tester,
  ) async {
    final harness = await _pumpPet(tester, pet: _machinePet());

    // The config stores the page 0-based; the field shows it 1-based.
    expect(
      tester.widget<TextField>(_field('pet-return-page')).controller?.text,
      '2',
    );
    expect(
      tester.widget<TextField>(_field('pet-return-cell')).controller?.text,
      '4',
    );

    await tester.enterText(_field('pet-return-page'), '3');
    await tester.pump();
    await tester.enterText(_field('pet-return-cell'), '7');
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('pet-food-load-mode')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('drag').last);
    await tester.pumpAndSettle();

    await _tapSwitch(tester, 'pet-reload-on-start');
    await tester.enterText(_field('pet-action-wait'), '800');
    await tester.enterText(_field('pet-wait-after-empty'), '10');
    await tester.enterText(_field('pet-feeder-left-fraction'), '0.6');
    await tester.enterText(_field('pet-feeder-min-score'), '0.8');
    await tester.pump();

    final sections = await _save(tester, harness);
    final pet = sections['pet'] as Map<String, dynamic>;
    expect(pet['return_slot'], [2, 7]);
    expect(pet['food_load_mode'], 'drag');
    expect(pet['reload_on_start'], true);
    expect(pet['action_wait_ms'], 800);
    expect(pet['wait_after_empty_minutes'], 10);
    expect(pet['feeder_count_left_fraction'], 0.6);
    expect(pet['feeder_count_min_score'], 0.8);
  });

  testWidgets(
    'the food-cell grid is disabled without a calibrated boarding bag grid',
    (tester) async {
      final machine = Map<String, dynamic>.of(_machinePet())
        ..remove('bag_grid');
      await _pumpPet(tester, pet: machine);

      expect(
        find.byKey(const ValueKey('pet-food-not-calibrated')),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('pet-food-page-0')), findsNothing);
      expect(find.byKey(const ValueKey('pet-food-cell-0-0')), findsNothing);
      // Only the grid needs the bag mark; the setup fields stay editable.
      expect(find.byKey(const ValueKey('pet-row-0-enabled')), findsOneWidget);
      expect(_field('pet-row-0-stacks'), findsOneWidget);
    },
  );

  testWidgets(
    'a refused save renders the core line verbatim and keeps the edits',
    (tester) async {
      final harness = await _pumpPet(tester, pet: _machinePet());
      await _tapSwitch(tester, 'pet-row-0-enabled');

      harness.answerConfigCommands(
        errors: ['a breeding row needs a stack count'],
      );
      await tester.tap(find.byKey(const ValueKey('save-pet')));
      await _flush(tester);

      expect(find.text('a breeding row needs a stack count'), findsOneWidget);
      expect(harness.lastSentWithCmd('config.apply'), isNull);
      expect(find.byKey(const ValueKey('dirty-pet')), findsOneWidget);
    },
  );

  testWidgets('Cancel restores the machine and portable drafts', (
    tester,
  ) async {
    await _pumpPet(tester, pet: _machinePet());

    await _tapSwitch(tester, 'pet-row-0-enabled');
    await tester.enterText(_field('pet-row-0-stacks'), '3');
    await tester.enterText(_field('pet-items-per-minute'), '5');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-pet')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('cancel-pet')));
    await tester.pump();
    await tester.pump();

    expect(find.byKey(const ValueKey('dirty-pet')), findsNothing);
    expect(
      tester.widget<TextField>(_field('pet-row-0-stacks')).controller?.text,
      '5',
    );
    final rateField = tester.widget<TextField>(_field('pet-items-per-minute'));
    expect(rateField.controller?.text, '3');
  });
}

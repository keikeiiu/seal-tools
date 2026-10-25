/// The Pet tab: portable `pet` settings as a `settings:` save, machine rows/crops read-only, and the
/// explicit "no rows configured yet" state the committed dev config produces.
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
  'close_button_png': 'iVBORw0KGgo=',
  'food_icon_png': 'iVBORw0KGgo=',
  'bag_grid': [100, 200, 300, 400],
  'food_slots': [
    [1, 2],
    [1, 3],
  ],
  'return_slot': [1, 4],
  'food_load_mode': 'right_click',
  'queue': [
    {'label': 'pet A'},
    {'label': 'pet B'},
  ],
  'slots': [
    {
      'enabled': true,
      'stacks': 5,
      'feeder_strip': [1, 2, 3, 4],
      'label': 'row 1',
    },
    {
      'enabled': false,
      'stacks': 2,
      'feeder_strip': [5, 6, 7, 8],
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

void main() {
  testWidgets(
    'the committed dev shape shows "no rows", and saves only settings.pet',
    (tester) async {
      final harness = await _pumpPet(tester);

      // The committed dev config has no machine pet rows: that is an explicit state, not a blank.
      expect(find.byKey(const ValueKey('pet-no-rows')), findsOneWidget);
      expect(find.textContaining('No rows configured yet'), findsOneWidget);
      expect(find.textContaining('not configured'), findsWidgets);
      expect(
        tester
            .widget<TextField>(_field('pet-items-per-minute'))
            .controller
            ?.text,
        '3',
      );
      expect(
        tester.widget<TextField>(_field('pet-occupied-above')).controller?.text,
        '0.05',
      );

      await tester.enterText(_field('pet-items-per-minute'), '5');
      await tester.pump();
      expect(find.byKey(const ValueKey('dirty-pet')), findsOneWidget);

      harness.answerConfigCommands();
      await tester.tap(find.byKey(const ValueKey('save-pet')));
      await _flush(tester);

      final apply = harness.lastSentWithCmd('config.apply');
      expect(apply, isNotNull);
      final sections =
          (apply!['args'] as Map<String, dynamic>)['sections']
              as Map<String, dynamic>;
      expect(sections.keys, ['settings']);
      final pet =
          (sections['settings'] as Map<String, dynamic>)['pet']
              as Map<String, dynamic>;
      expect(pet['items_per_minute'], 5);
      expect(pet.containsKey('slots'), isFalse);
      expect(pet.containsKey('bag_grid'), isFalse);
      expect(pet.containsKey('queue'), isFalse);

      expect(find.byKey(const ValueKey('dirty-pet')), findsNothing);
      expect(find.byKey(const ValueKey('save-ok-pet')), findsOneWidget);
    },
  );

  testWidgets(
    'machine rows and calibration crops render read-only; Cancel restores',
    (tester) async {
      await _pumpPet(tester, pet: _machinePet());

      expect(find.textContaining('row 1 · enabled · stacks 5'), findsOneWidget);
      expect(find.textContaining('disabled · stacks 2'), findsOneWidget);
      expect(find.textContaining('2 queued entries'), findsOneWidget);
      expect(find.textContaining('2 slots'), findsOneWidget);
      expect(find.textContaining('configured'), findsWidgets);
      expect(find.byKey(const ValueKey('pet-no-rows')), findsNothing);

      await tester.enterText(_field('pet-occupied-above'), '0.2');
      await tester.pump();
      expect(find.byKey(const ValueKey('dirty-pet')), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('cancel-pet')));
      await tester.pump();

      expect(find.byKey(const ValueKey('dirty-pet')), findsNothing);
      expect(
        tester.widget<TextField>(_field('pet-occupied-above')).controller?.text,
        '0.05',
      );
    },
  );

  testWidgets('renders the core validation lines verbatim and does not apply', (
    tester,
  ) async {
    final harness = await _pumpPet(tester);
    await tester.enterText(_field('pet-items-per-minute'), '5');
    await tester.pump();

    harness.answerConfigCommands(
      errors: ['the fallback feeding rate must be positive'],
    );
    await tester.tap(find.byKey(const ValueKey('save-pet')));
    await _flush(tester);

    expect(
      find.text('the fallback feeding rate must be positive'),
      findsOneWidget,
    );
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    expect(find.byKey(const ValueKey('dirty-pet')), findsOneWidget);
  });
}

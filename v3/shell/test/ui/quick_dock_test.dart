/// The quick dock v1: the idle launcher (seven quick starts, secondary menus per tool, five
/// placements) and its transitions to/from the existing run bar.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/dock_placement.dart';
import 'package:seal_shell/protocol/protocol.dart';
import 'package:seal_shell/ui/quick_dock.dart';
import 'package:seal_shell/ui/shell_app.dart';
import 'package:seal_shell/ui/theme.dart';

import '../support/app_harness.dart';

Map<String, dynamic> _dockConfig() => {
  'window': {'title': 'TW_LIVE'},
  'buy_sell': {
    'presets': {
      'HP potions': {'count': 20},
      'MP potions': {'count': 10},
    },
  },
  'spammer': {
    'active': 'Fire rotation',
    'presets': {
      'Fire rotation': <String, dynamic>{},
      'Ice rotation': <String, dynamic>{},
    },
  },
  'quest': {
    'active': 'Cave run',
    'presets': {
      'Cave run': <String, dynamic>{},
      'Tower run': <String, dynamic>{},
    },
  },
  'tuner': {
    'target_grade': 'N',
    'grade_order': ['N', 'G', 'DG', 'XG', 'SG'],
    'max_retries': 3,
  },
  'gem': {
    'start_grade': 'N',
    'grades': ['N', 'G', 'DG'],
    'empty_mode': 'stop',
  },
};

/// Pumps the dock widget alone, with the model already in dock mode.
Future<AppHarness> _pumpDock(
  WidgetTester tester, {
  DockSize size = DockSize.standard,
}) async {
  tester.view.physicalSize = const ui.Size(460, 620);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  final harness = AppHarness();
  addTearDown(harness.model.dispose);
  final connecting = harness.connect(config: _dockConfig());
  await tester.pumpWidget(
    const MaterialApp(
      home: Scaffold(body: Center(child: Text('connecting'))),
    ),
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  harness.onWrite = (line, fake) {
    final message = fake.sentJson(fake.sent.length - 1);
    final id = message['id'] as String;
    switch (message['cmd']) {
      case 'tool.start' || 'tool.stop':
        fake.emitJson(okResponse(id, {}));
      case 'config.validate':
        fake.emitJson(
          okResponse(id, {'errors': <String>[], 'warnings': <String>[]}),
        );
      case 'config.apply':
        final args = message['args'];
        final sections = args is Map<String, dynamic>
            ? args['sections']
            : <String, dynamic>{};
        fake.emitJson(okResponse(id, {'rev': 8, 'sections': sections}));
    }
  };
  if (size != DockSize.standard) {
    await harness.model.setDockSize(size);
    await tester.pump();
  }
  await harness.model.enterDock();
  final (dockWidth, dockHeight) = DockPlacement.rightRail.sizeFor(size);
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(
        backgroundColor: const Color(0xFF111318),
        body: Center(
          child: SizedBox(
            width: dockWidth.toDouble(),
            height: dockHeight.toDouble(),
            child: QuickDock(model: harness.model),
          ),
        ),
      ),
    ),
  );
  for (var i = 0; i < 4; i++) {
    await tester.pump();
  }
  return harness;
}

void main() {
  testWidgets('the dock shows five quick tiles plus the setup-only rows', (
    tester,
  ) async {
    await _pumpDock(tester);

    expect(find.byKey(const ValueKey('quick-dock')), findsOneWidget);
    for (final key in const [
      'dock-tile-spammer',
      'dock-tile-buy',
      'dock-tile-sell',
      'dock-tile-quest',
      'dock-tile-gem',
      'dock-tile-tuner',
      'dock-tile-holdspace',
      'dock-long-pet',
    ]) {
      expect(find.byKey(ValueKey(key)), findsOneWidget, reason: key);
    }
    expect(find.text('Quick start'), findsOneWidget);
  });

  testWidgets('a tile body starts with the current options', (tester) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-start-spammer')));
    await tester.pump();
    expect(harness.lastSentWithCmd('tool.start')!['args'], {'tool': 'spammer'});

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('start and options are separate, unambiguous targets', (
    tester,
  ) async {
    await _pumpDock(tester);

    expect(find.byKey(const ValueKey('dock-start-buy')), findsOneWidget);
    expect(find.byKey(const ValueKey('dock-menu-buy')), findsOneWidget);
    final options = tester.widget<IconButton>(
      find.byKey(const ValueKey('dock-menu-buy')),
    );
    expect(
      (options.icon as Icon).icon,
      Icons.more_vert,
      reason: 'a ??reads as options; the old chevron read as navigation',
    );

    // Hold Space is just a Start/Stop action now: no separate "Toggle" label, no options menu.
    expect(find.byKey(const ValueKey('dock-menu-holdspace')), findsNothing);
    expect(find.text('Toggle'), findsNothing);
  });

  testWidgets('Hold Space starts from its own action', (tester) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-start-holdspace')));
    await tester.pump();
    expect(harness.lastSentWithCmd('tool.start')!['args'], {
      'tool': 'holdspace',
    });

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets(
    'Gem Composer and Tuner are short-run tiles; only Pet is setup-only',
    (tester) async {
      final harness = await _pumpDock(tester);

      // Their ??options edit the saved run config; the tiles still start directly.
      expect(find.byKey(const ValueKey('dock-menu-gem')), findsOneWidget);
      expect(find.byKey(const ValueKey('dock-menu-tuner')), findsOneWidget);
      expect(find.byKey(const ValueKey('dock-long-pet')), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('dock-start-tuner')));
      await tester.pump();
      expect(harness.lastSentWithCmd('tool.start')!['args'], {'tool': 'tuner'});

      await tester.tap(find.byKey(const ValueKey('dock-start-gem')));
      await tester.pump();
      expect(harness.lastSentWithCmd('tool.start')!['args'], {'tool': 'gem'});

      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 2));
    },
  );

  testWidgets('Tuner and Gem options save the config before starting', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);

    Map<String, dynamic> savedSettings(String block) {
      final args =
          harness.lastSentWithCmd('config.apply')!['args']
              as Map<String, dynamic>;
      final sections = (args['sections'] as Map<String, dynamic>);
      final settings = sections['settings'] as Map<String, dynamic>;
      return settings[block] as Map<String, dynamic>;
    }

    await tester.tap(find.byKey(const ValueKey('dock-menu-tuner')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-tuner-grade-DG')));
    await tester.pump();
    await tester.pump();
    final tuner = savedSettings('tuner');
    expect(tuner['target_grade'], 'DG');
    expect(
      tuner['max_retries'],
      3,
      reason: 'a partial save keeps the other keys',
    );

    await tester.tap(find.byKey(const ValueKey('dock-tuner-retries-plus')));
    await tester.pump();
    await tester.pump();
    expect(savedSettings('tuner')['max_retries'], 4);

    await tester.tap(find.byKey(const ValueKey('dock-menu-start')));
    await tester.pump();
    expect(harness.lastSentWithCmd('tool.start')!['args'], {'tool': 'tuner'});

    await tester.tap(find.byKey(const ValueKey('dock-menu-gem')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-gem-start-G')));
    await tester.pump();
    await tester.pump();
    expect(savedSettings('gem')['start_grade'], 'G');

    await tester.tap(
      find.byKey(const ValueKey('dock-gem-empty-advance_grade')),
    );
    await tester.pump();
    await tester.pump();
    expect(savedSettings('gem')['empty_mode'], 'advance_grade');

    await tester.tap(find.byKey(const ValueKey('dock-menu-start')));
    await tester.pump();
    expect(harness.lastSentWithCmd('tool.start')!['args'], {'tool': 'gem'});

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('Large scales the type and the fixed action column', (
    tester,
  ) async {
    await _pumpDock(tester, size: DockSize.large);

    final start = find.byKey(const ValueKey('dock-start-spammer'));
    expect(tester.getSize(start), const ui.Size(76, 34));
    final title = tester.widget<Text>(
      find.descendant(
        of: find.byKey(const ValueKey('dock-tile-spammer')),
        matching: find.text('Skill Spammer'),
      ),
    );
    expect(title.style?.fontSize, 15);

    final menu = find.byKey(const ValueKey('dock-menu-spammer'));
    expect(tester.getSize(menu).height, 34);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('Compact keeps the labels readable above the smaller window', (
    tester,
  ) async {
    await _pumpDock(tester, size: DockSize.compact);

    final start = find.byKey(const ValueKey('dock-start-spammer'));
    expect(tester.getSize(start), const ui.Size(58, 26));
    final title = tester.widget<Text>(
      find.descendant(
        of: find.byKey(const ValueKey('dock-tile-spammer')),
        matching: find.text('Skill Spammer'),
      ),
    );
    expect(title.style?.fontSize, 12.5);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the header menu offers the three dock sizes', (tester) async {
    final harness = await _pumpDock(tester);
    await tester.tap(find.byKey(const ValueKey('dock-placement')));
    await tester.pumpAndSettle();
    expect(find.text('Compact size'), findsOneWidget);
    expect(find.text('Standard size'), findsOneWidget);
    expect(find.text('Large size'), findsOneWidget);

    await tester.tap(find.text('Large size'));
    await tester.pumpAndSettle();
    expect(harness.model.dockSize, DockSize.large);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the header Keep open toggle controls the idle collapse', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);
    expect(harness.model.dockBehavior, DockBehavior.alwaysOpen);
    await tester.pump(const Duration(seconds: 9));
    await tester.pump();
    expect(harness.model.dockCollapsed, isFalse);

    await tester.tap(find.byKey(const ValueKey('dock-keep-open')));
    await tester.pump();
    expect(harness.model.dockBehavior, DockBehavior.autoCollapse);
    await tester.pump(const Duration(seconds: 9));
    await tester.pump();
    expect(harness.model.dockCollapsed, isTrue);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('changing the dock size restarts the idle countdown', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);
    await tester.pump(const Duration(seconds: 4));
    await harness.model.setDockSize(DockSize.large);
    await tester.pump();
    await tester.pump(const Duration(seconds: 5));
    expect(
      harness.model.dockCollapsed,
      isFalse,
      reason: 'the pre-resize timer must not collapse the new geometry',
    );

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the dock auto-collapses after eight idle seconds', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);
    await tester.tap(find.byKey(const ValueKey('dock-placement')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Auto-collapse'));
    await tester.pumpAndSettle();
    expect(harness.model.dockBehavior, DockBehavior.autoCollapse);

    await tester.pump(const Duration(seconds: 8));
    await tester.pump();
    expect(harness.model.dockCollapsed, isTrue);
    expect(find.byKey(const ValueKey('dock-collapsed')), findsOneWidget);
  });

  testWidgets('Keep open disables the idle auto-collapse', (tester) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-placement')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Auto-collapse'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('dock-placement')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Keep open'));
    await tester.pumpAndSettle();
    expect(harness.model.dockBehavior, DockBehavior.alwaysOpen);
    final apply =
        harness.lastSentWithCmd('config.apply')!['args']
            as Map<String, dynamic>;
    final sections = apply['sections'] as Map<String, dynamic>;
    final settings = sections['settings'] as Map<String, dynamic>;
    final dock =
        ((settings['ui'] as Map<String, dynamic>)['quick_dock']
            as Map<String, dynamic>);
    expect(dock['behavior'], 'always_open');

    await tester.pump(const Duration(seconds: 9));
    await tester.pump();
    expect(harness.model.dockCollapsed, isFalse);
    expect(find.byKey(const ValueKey('dock-start-spammer')), findsOneWidget);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('hovering the tiny tab opens and keeps the dock open', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);
    await tester.tap(find.byKey(const ValueKey('dock-collapse')));
    await tester.pump();
    expect(harness.model.dockCollapsed, isTrue);

    final mouse = await tester.createGesture(kind: ui.PointerDeviceKind.mouse);
    await mouse.addPointer(location: const Offset(20, 20));
    await tester.pump();
    await mouse.moveTo(
      tester.getCenter(find.byKey(const ValueKey('dock-collapsed'))),
    );
    await tester.pump();
    expect(harness.model.dockCollapsed, isFalse);

    await mouse.removePointer();
    await tester.pump();
    expect(
      harness.model.dockCollapsed,
      isFalse,
      reason: 'hovering is an open action, not a temporary preview',
    );
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('a click ends hover expansion so the dock stays open', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);
    await tester.tap(find.byKey(const ValueKey('dock-keep-open')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-collapse')));
    await tester.pump();
    expect(harness.model.dockCollapsed, isTrue);

    final mouse = await tester.createGesture(kind: ui.PointerDeviceKind.mouse);
    await mouse.addPointer(location: const Offset(8, 8));
    await tester.pump();
    await mouse.moveTo(
      tester.getCenter(find.byKey(const ValueKey('dock-collapsed'))),
    );
    await tester.pump();
    expect(harness.model.dockCollapsed, isFalse);

    // Clicking the expanded launcher is explicit: leaving with the pointer must not undo it.
    await tester.tap(find.byKey(const ValueKey('quick-dock')));
    await tester.pump();
    await mouse.removePointer();
    await tester.pump();
    expect(harness.model.dockCollapsed, isFalse);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('a collapsed tool icon expands straight into its options', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);
    await tester.tap(find.byKey(const ValueKey('dock-collapse')));
    await tester.pump();
    expect(harness.model.dockCollapsed, isTrue);

    await tester.tap(find.byKey(const ValueKey('dock-collapsed-spammer')));
    await tester.pump();
    expect(harness.model.dockCollapsed, isFalse);
    expect(find.byKey(const ValueKey('dock-menu-title')), findsOneWidget);
    expect(
      tester.widget<Text>(find.byKey(const ValueKey('dock-menu-title'))).data,
      'Skill Spammer',
    );

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('last-used Buy options survive closing the menu', (tester) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-menu-buy')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-buy-plus')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-buy-plus')));
    await tester.pump();
    expect(harness.model.dockBuyCount, 22);

    await tester.tap(find.byKey(const ValueKey('dock-back')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-menu-buy')));
    await tester.pump();
    expect(
      tester.widget<Text>(find.byKey(const ValueKey('dock-buy-count'))).data,
      '22',
    );

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the Pet setup row asks the full shell for its tab', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-long-pet')));
    await tester.pump();
    expect(harness.model.setupRequest.value, ToolId.pet);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('a pinned Sell selection becomes a one-click tile', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-menu-sell')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-sell-row-0')));
    await tester.pump();
    await tester.ensureVisible(find.byKey(const ValueKey('dock-pin-name')));
    await tester.pump();
    await tester.enterText(
      find.byKey(const ValueKey('dock-pin-name')),
      'Junk row',
    );
    await tester.ensureVisible(find.byKey(const ValueKey('dock-pin')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-pin')));
    await tester.pump();
    expect(harness.model.dockPins.single.name, 'Junk row');
    expect(harness.model.dockTileIds, contains('pin:Junk row'));

    await tester.tap(find.byKey(const ValueKey('dock-back')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-start-pin:Junk row')));
    await tester.pump();
    expect(harness.lastSentWithCmd('tool.start')!['args'], {
      'tool': 'sell',
      'slots': [0, 1, 2, 3, 4, 5, 6, 7],
    });

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('edit mode removes, reorders and re-adds quick actions', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-edit')));
    await tester.pump();
    expect(find.byKey(const ValueKey('dock-edit-row-spammer')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('dock-edit-remove-spammer')));
    await tester.pump();
    expect(harness.model.dockTileIds, isNot(contains('spammer')));
    expect(find.byKey(const ValueKey('dock-tile-spammer')), findsNothing);

    await tester.tap(find.byKey(const ValueKey('dock-add-spammer')));
    await tester.pump();
    expect(harness.model.dockTileIds, contains('spammer'));

    final before = harness.model.dockTileIds.indexOf('buy');
    await tester.tap(find.byKey(const ValueKey('dock-edit-down-buy')));
    await tester.pump();
    expect(harness.model.dockTileIds.indexOf('buy'), before + 1);

    await tester.tap(find.byKey(const ValueKey('dock-edit')));
    await tester.pump();
    expect(find.byKey(const ValueKey('dock-tile-buy')), findsOneWidget);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the Buy menu sends per-run preset and count', (tester) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-menu-buy')));
    await tester.pump();
    expect(find.byKey(const ValueKey('dock-menu-start')), findsOneWidget);
    expect(find.text('HP potions'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('dock-buy-plus')));
    await tester.pump();
    expect(find.byKey(const ValueKey('dock-buy-count')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('dock-menu-start')));
    await tester.pump();
    final args = harness.lastSentWithCmd('tool.start')!['args'];
    expect(args, {'tool': 'buy', 'preset': 'HP potions', 'count': 21});

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the Sell menu toggles bag cells and sends slots', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-menu-sell')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-sell-cell-0')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-sell-cell-9')));
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('dock-menu-start')));
    await tester.pump();
    expect(harness.lastSentWithCmd('tool.start')!['args'], {
      'tool': 'sell',
      'slots': [0, 9],
    });

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the Sell menu selects and clears a whole bag line', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-menu-sell')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-sell-row-0')));
    await tester.pump();
    expect(harness.model.sellSlots, [0, 1, 2, 3, 4, 5, 6, 7]);
    expect(
      find.byKey(const ValueKey('dock-sell-count')),
      findsOneWidget,
      reason: 'the live count stays visible next to the row shortcuts',
    );

    // A second click on the row clears it again (v2 ToggleSellRow).
    await tester.tap(find.byKey(const ValueKey('dock-sell-row-0')));
    await tester.pump();
    expect(harness.model.sellSlots, isEmpty);

    await tester.tap(find.byKey(const ValueKey('dock-sell-row-3')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('dock-menu-start')));
    await tester.pump();
    expect(harness.lastSentWithCmd('tool.start')!['args'], {
      'tool': 'sell',
      'slots': [24, 25, 26, 27, 28, 29, 30, 31],
    });

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the dock can be moved between the five placements', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-placement')));
    await tester.pumpAndSettle();
    expect(find.text('Bottom right'), findsOneWidget);
    await tester.tap(find.text('Bottom right'));
    await tester.pumpAndSettle();
    expect(harness.model.dockPlacement, DockPlacement.bottomRight);

    await tester.tap(find.byKey(const ValueKey('dock-placement')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Top centre'));
    await tester.pumpAndSettle();
    expect(harness.model.dockPlacement, DockPlacement.topCentre);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the dock collapses to its resting form and expands again', (
    tester,
  ) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-collapse')));
    await tester.pump();
    expect(harness.model.dockCollapsed, isTrue);
    expect(find.byKey(const ValueKey('dock-collapsed')), findsOneWidget);
    expect(find.byKey(const ValueKey('dock-start-spammer')), findsNothing);

    await tester.tap(find.byKey(const ValueKey('dock-expand')));
    await tester.pump();
    expect(harness.model.dockCollapsed, isFalse);
    expect(find.byKey(const ValueKey('dock-collapsed')), findsNothing);
    expect(find.byKey(const ValueKey('dock-start-spammer')), findsOneWidget);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the top bar opens the dock and the dock opens the full shell', (
    tester,
  ) async {
    tester.view.physicalSize = const ui.Size(1600, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);
    final harness = AppHarness();
    addTearDown(harness.model.dispose);
    final connecting = harness.connect(config: _dockConfig());
    await tester.pumpWidget(ShellApp(model: harness.model));
    for (var i = 0; i < 3; i++) {
      await tester.pump();
    }
    await connecting;
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('app-bar-dock')));
    for (var i = 0; i < 6; i++) {
      await tester.pump();
    }
    expect(find.byKey(const ValueKey('quick-dock')), findsOneWidget);
    expect(find.byKey(const ValueKey('logs-toggle')), findsNothing);

    await tester.tap(find.byKey(const ValueKey('dock-open-full')));
    for (var i = 0; i < 6; i++) {
      await tester.pump();
    }
    expect(find.byKey(const ValueKey('quick-dock')), findsNothing);
    expect(find.byKey(const ValueKey('logs-toggle')), findsOneWidget);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the finished run bar offers the dock again', (tester) async {
    tester.view.physicalSize = const ui.Size(1600, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);
    final harness = AppHarness();
    addTearDown(harness.model.dispose);
    final connecting = harness.connect(config: _dockConfig());
    await tester.pumpWidget(ShellApp(model: harness.model));
    for (var i = 0; i < 3; i++) {
      await tester.pump();
    }
    await connecting;
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('app-bar-dock')));
    for (var i = 0; i < 6; i++) {
      await tester.pump();
    }
    harness.emitState(tool: ToolId.tuner, message: 'running');
    for (var i = 0; i < 6; i++) {
      await tester.pump();
    }
    expect(find.byKey(const ValueKey('mini-bar')), findsOneWidget);
    harness.emitState(
      tool: ToolId.tuner,
      phase: ToolPhase.idle,
      message: 'DONE',
    );
    harness.emitToolFinished(tool: ToolId.tuner);
    for (var i = 0; i < 6; i++) {
      await tester.pump();
    }
    expect(find.byKey(const ValueKey('mini-dock')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('mini-dock')));
    for (var i = 0; i < 6; i++) {
      await tester.pump();
    }
    expect(find.byKey(const ValueKey('quick-dock')), findsOneWidget);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });
}

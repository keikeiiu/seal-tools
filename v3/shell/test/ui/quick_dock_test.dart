/// The quick dock v1: the idle launcher (five quick starts, one secondary menu per tool, five
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
};

/// Pumps the dock widget alone, with the model already in dock mode.
Future<AppHarness> _pumpDock(WidgetTester tester) async {
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
    if (message['cmd'] == 'tool.start' || message['cmd'] == 'tool.stop') {
      fake.emitJson(okResponse(message['id'] as String, {}));
    }
  };
  await harness.model.enterDock();
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(
        backgroundColor: const Color(0xFF111318),
        body: Center(
          child: SizedBox(
            width: 300,
            height: 480,
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
      'dock-tile-holdspace',
      'dock-long-gem',
      'dock-long-tuner',
      'dock-long-pet',
    ]) {
      expect(find.byKey(ValueKey(key)), findsOneWidget, reason: key);
    }
    expect(find.text('Quick start'), findsOneWidget);
  });

  testWidgets('a tile body starts with the current options', (tester) async {
    final harness = await _pumpDock(tester);

    await tester.tap(find.byKey(const ValueKey('dock-tile-spammer')));
    await tester.pump();
    expect(harness.lastSentWithCmd('tool.start')!['args'], {'tool': 'spammer'});

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

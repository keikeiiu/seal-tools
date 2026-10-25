/// The Spammer tab: renders the `spammer` block, edits it, and Saves through the model's
/// validate→apply order with ONLY the spammer section.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/ui/tabs/spammer_tab.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

Map<String, dynamic> _block() => {
  'active': 'buffs',
  'presets': {
    'buffs': {'1': 1.2, '2': 2.5},
  },
  'priority': {
    'buffs': ['2', '1'],
  },
  'startup': {
    'buffs': {'lead_in': 0.5, 'step': 0.1},
  },
  'combos': {
    'buffs': [
      {
        'keys': ['1', '2'],
        'gap': 0.1,
      },
    ],
  },
};

Finder _field(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(TextField),
);

Future<AppHarness> _pumpSpammer(
  WidgetTester tester, {
  Map<String, dynamic>? spammer,
}) async {
  tester.view.physicalSize = const ui.Size(1000, 900);
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
      'spammer': spammer ?? _block(),
    },
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(body: SpammerTab(model: harness.model)),
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
  testWidgets('renders the spammer block and saves ONLY that block', (
    tester,
  ) async {
    final harness = await _pumpSpammer(tester);

    // The block's own data is rendered, in the core's order.
    expect(find.text('buffs'), findsWidgets);
    expect(
      tester.widget<TextField>(_field('key-name-0')).controller?.text,
      '2',
    );
    expect(
      tester.widget<TextField>(_field('key-seconds-0')).controller?.text,
      '2.5',
    );
    expect(
      tester.widget<TextField>(_field('key-name-1')).controller?.text,
      '1',
    );
    expect(tester.widget<TextField>(_field('lead-in')).controller?.text, '0.5');

    await tester.enterText(_field('key-seconds-0'), '9.5');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-spammer')), findsOneWidget);

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-spammer')));
    await _flush(tester);

    expect(harness.lastSentWithCmd('config.validate'), isNotNull);
    final apply = harness.lastSentWithCmd('config.apply');
    expect(apply, isNotNull);
    final sections =
        (apply!['args'] as Map<String, dynamic>)['sections']
            as Map<String, dynamic>;
    expect(sections.keys, ['spammer'], reason: 'only the owned block is sent');
    final block = sections['spammer'] as Map<String, dynamic>;
    expect(
      (block['presets'] as Map<String, dynamic>)['buffs'],
      containsPair('2', 9.5),
    );
    expect(
      block.containsKey('combos'),
      isFalse,
      reason: 'a field the tab does not edit is never sent back',
    );

    expect(find.byKey(const ValueKey('dirty-spammer')), findsNothing);
    expect(find.byKey(const ValueKey('save-ok-spammer')), findsOneWidget);
  });

  testWidgets('renders the core validation lines verbatim and does not apply', (
    tester,
  ) async {
    final harness = await _pumpSpammer(tester);
    await tester.enterText(_field('key-seconds-0'), '9.5');
    await tester.pump();

    harness.answerConfigCommands(
      errors: ['"buffs" has no keys to press', 'a second core refusal line'],
    );
    await tester.tap(find.byKey(const ValueKey('save-spammer')));
    await _flush(tester);

    expect(find.text('"buffs" has no keys to press'), findsOneWidget);
    expect(find.text('a second core refusal line'), findsOneWidget);
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    expect(
      find.byKey(const ValueKey('dirty-spammer')),
      findsOneWidget,
      reason: 'the draft stays dirty so the player can fix the core\'s line',
    );
  });

  testWidgets('Cancel restores the mirror value and clears the dirty marker', (
    tester,
  ) async {
    await _pumpSpammer(tester);

    await tester.enterText(_field('key-seconds-0'), '9.5');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-spammer')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('cancel-spammer')));
    await tester.pump();

    expect(find.byKey(const ValueKey('dirty-spammer')), findsNothing);
    expect(
      tester.widget<TextField>(_field('key-seconds-0')).controller?.text,
      '2.5',
    );
  });

  testWidgets(
    'a conflict shows the core rev and only an explicit reconcile re-seeds',
    (tester) async {
      final harness = await _pumpSpammer(tester);
      await tester.enterText(_field('key-seconds-0'), '9.5');
      await tester.pump();

      harness.answerConfigCommands(
        conflict: true,
        conflictRev: 12,
        conflictSections: {'spammer': null},
        reloadRev: 13,
        reloadedConfig: {
          'window': {'title': 'TW_LIVE'},
          'spammer': {
            'active': 'core',
            'presets': {
              'core': {'3': 3.0},
            },
            'priority': {
              'core': ['3'],
            },
            'startup': {
              'core': {'lead_in': 1.0, 'step': 0.2},
            },
          },
        },
      );
      await tester.tap(find.byKey(const ValueKey('save-spammer')));
      await _flush(tester);

      final conflictBanner = find.byKey(const ValueKey('conflict-spammer'));
      expect(conflictBanner, findsOneWidget);
      expect(find.textContaining('rev 12'), findsOneWidget);
      expect(find.textContaining('spammer'), findsWidgets);
      // The draft still holds the player's edit until they choose to reconcile.
      expect(find.byKey(const ValueKey('dirty-spammer')), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('reconcile-spammer')));
      await _flush(tester);

      expect(find.byKey(const ValueKey('conflict-spammer')), findsNothing);
      expect(find.byKey(const ValueKey('dirty-spammer')), findsNothing);
      expect(find.text('core'), findsWidgets);
      expect(
        tester.widget<TextField>(_field('key-name-0')).controller?.text,
        '3',
      );
      expect(
        tester.widget<TextField>(_field('key-seconds-0')).controller?.text,
        '3',
      );
    },
  );
}

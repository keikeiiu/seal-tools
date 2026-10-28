/// The Spammer tab: renders the `spammer` block, edits it, and Saves through the model's
/// validate→apply order with ONLY the spammer section.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/app.dart';
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
    expect((block['combos'] as Map<String, dynamic>)['buffs'], [
      {
        'keys': ['1', '2'],
        'gap': 0.1,
      },
    ], reason: 'combos rides the draft unchanged until the editor lands');

    expect(find.byKey(const ValueKey('dirty-spammer')), findsNothing);
    expect(find.byKey(const ValueKey('save-ok-spammer')), findsOneWidget);
  });

  testWidgets('rename moves all name-keyed maps and active, content intact', (
    tester,
  ) async {
    final harness = await _pumpSpammer(tester);

    await tester.enterText(
      find.byKey(const ValueKey('rename-preset-name')),
      'buffed',
    );
    await tester.tap(find.byKey(const ValueKey('rename-preset')));
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-spammer')), findsOneWidget);
    expect(find.byKey(const ValueKey('spammer-preset-note')), findsNothing);

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-spammer')));
    await _flush(tester);

    final apply = harness.lastSentWithCmd('config.apply')!;
    final block =
        ((apply['args'] as Map<String, dynamic>)['sections']
                as Map<String, dynamic>)['spammer']
            as Map<String, dynamic>;
    expect((block['presets'] as Map<String, dynamic>).keys, ['buffed']);
    expect((block['presets'] as Map<String, dynamic>)['buffed'], {
      '1': 1.2,
      '2': 2.5,
    });
    expect((block['priority'] as Map<String, dynamic>)['buffed'], ['2', '1']);
    expect((block['startup'] as Map<String, dynamic>)['buffed'], {
      'lead_in': 0.5,
      'step': 0.1,
    });
    expect((block['combos'] as Map<String, dynamic>)['buffed'], [
      {
        'keys': ['1', '2'],
        'gap': 0.1,
      },
    ], reason: 'the combos content survives the rename key move');
    expect(block['active'], 'buffed');
  });

  testWidgets('duplicate deep-copies under a unique name and keeps active', (
    tester,
  ) async {
    final harness = await _pumpSpammer(tester);

    await tester.tap(find.byKey(const ValueKey('duplicate-preset')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('duplicate-preset')));
    await tester.pump();

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-spammer')));
    await _flush(tester);

    final apply = harness.lastSentWithCmd('config.apply')!;
    final block =
        ((apply['args'] as Map<String, dynamic>)['sections']
                as Map<String, dynamic>)['spammer']
            as Map<String, dynamic>;
    final presets = block['presets'] as Map<String, dynamic>;
    expect(presets.keys.toSet(), {'buffs', 'buffs copy', 'buffs copy 2'});
    expect(presets['buffs copy'], {'1': 1.2, '2': 2.5});
    expect(presets['buffs copy 2'], {
      '1': 1.2,
      '2': 2.5,
    }, reason: 'the copy is a deep copy, not shared state');
    expect((block['combos'] as Map<String, dynamic>).keys.toSet(), {
      'buffs',
      'buffs copy',
      'buffs copy 2',
    });
    expect(block['active'], 'buffs', reason: 'duplicating does not select');
  });

  testWidgets('rename guards: existing name and empty name are refused', (
    tester,
  ) async {
    final harness = await _pumpSpammer(tester);

    await tester.enterText(
      find.byKey(const ValueKey('rename-preset-name')),
      'buffs',
    );
    await tester.tap(find.byKey(const ValueKey('rename-preset')));
    await tester.pump();
    expect(find.text('a preset named "buffs" already exists'), findsOneWidget);
    expect(find.byKey(const ValueKey('dirty-spammer')), findsNothing);

    await tester.enterText(
      find.byKey(const ValueKey('rename-preset-name')),
      '',
    );
    await tester.tap(find.byKey(const ValueKey('rename-preset')));
    await tester.pump();
    expect(find.text('a preset name cannot be empty'), findsOneWidget);
    expect(find.byKey(const ValueKey('dirty-spammer')), findsNothing);
    expect(harness.lastSentWithCmd('config.apply'), isNull);
  });
  testWidgets('the combos editor edits key order and gap into the payload', (
    tester,
  ) async {
    final harness = await _pumpSpammer(tester);

    expect(
      tester.widget<TextField>(_field('combo-0-key-0')).controller?.text,
      '1',
    );
    expect(
      tester.widget<TextField>(_field('combo-0-key-1')).controller?.text,
      '2',
    );
    await tester.ensureVisible(
      find.byKey(const ValueKey('combo-0-key-0-down')),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('combo-0-key-0-down')));
    await tester.pump();
    await tester.ensureVisible(_field('combo-0-gap'));
    await tester.pump();
    await tester.enterText(_field('combo-0-gap'), '0.5');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-spammer')), findsOneWidget);

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-spammer')));
    await _flush(tester);

    final apply = harness.lastSentWithCmd('config.apply')!;
    final block =
        ((apply['args'] as Map<String, dynamic>)['sections']
                as Map<String, dynamic>)['spammer']
            as Map<String, dynamic>;
    final combo =
        (asJsonList((block['combos'] as Map<String, dynamic>)['buffs']).first)
            as Map<String, dynamic>;
    expect(asJsonList(combo['keys']), ['2', '1']);
    expect(combo['gap'], 0.5);
  });

  testWidgets('a bare new combo blocks Save until it has two keys', (
    tester,
  ) async {
    final harness = await _pumpSpammer(tester);

    await tester.ensureVisible(find.byKey(const ValueKey('add-combo')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('add-combo')));
    await tester.pump();
    expect(find.byKey(const ValueKey('spammer-combo-note')), findsOneWidget);
    expect(
      find.text('combo 2 in "buffs" needs at least two keys'),
      findsOneWidget,
    );

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-spammer')));
    await _flush(tester);
    expect(harness.lastSentWithCmd('config.validate'), isNull);

    await tester.ensureVisible(find.byKey(const ValueKey('combo-1-add-key')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('combo-1-add-key')));
    await tester.pump();
    await tester.ensureVisible(find.byKey(const ValueKey('combo-1-add-key')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('combo-1-add-key')));
    await tester.pump();
    expect(find.byKey(const ValueKey('spammer-combo-note')), findsNothing);
    await tester.ensureVisible(_field('combo-1-key-0'));
    await tester.pump();
    await tester.enterText(_field('combo-1-key-0'), 'A');
    await tester.ensureVisible(_field('combo-1-key-1'));
    await tester.pump();
    await tester.enterText(_field('combo-1-key-1'), 'B');
    await tester.pump();

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-spammer')));
    await _flush(tester);
    final apply = harness.lastSentWithCmd('config.apply')!;
    final block =
        ((apply['args'] as Map<String, dynamic>)['sections']
                as Map<String, dynamic>)['spammer']
            as Map<String, dynamic>;
    final combos = asJsonList(
      (block['combos'] as Map<String, dynamic>)['buffs'],
    );
    expect(combos.length, 2);
    expect(asJsonList(asJsonMap(combos[1])['keys']), ['A', 'B']);
  });

  testWidgets('removing a combo writes the remaining list', (tester) async {
    final harness = await _pumpSpammer(tester);

    await tester.ensureVisible(find.byKey(const ValueKey('combo-0-remove')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('combo-0-remove')));
    await tester.pump();
    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-spammer')));
    await _flush(tester);

    final apply = harness.lastSentWithCmd('config.apply')!;
    final block =
        ((apply['args'] as Map<String, dynamic>)['sections']
                as Map<String, dynamic>)['spammer']
            as Map<String, dynamic>;
    expect(
      asJsonList((block['combos'] as Map<String, dynamic>)['buffs']),
      isEmpty,
    );
  });

  testWidgets('a single validation line carries the index-0 key', (
    tester,
  ) async {
    final harness = await _pumpSpammer(tester);
    await tester.enterText(_field('key-seconds-0'), '9.5');
    await tester.pump();

    harness.answerConfigCommands(errors: ['one core refusal line']);
    await tester.tap(find.byKey(const ValueKey('save-spammer')));
    await _flush(tester);

    expect(
      tester
          .widget<Text>(find.byKey(const ValueKey('save-error-spammer-0')))
          .data,
      'one core refusal line',
    );
    expect(find.byKey(const ValueKey('save-error-spammer')), findsNothing);
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
    expect(find.byKey(const ValueKey('save-error-spammer-0')), findsOneWidget);
    expect(find.byKey(const ValueKey('save-error-spammer-1')), findsOneWidget);
    expect(
      tester
          .widget<Text>(find.byKey(const ValueKey('save-error-spammer-0')))
          .data,
      '"buffs" has no keys to press',
    );
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

  testWidgets('the last preset cannot be deleted (v2 guard)', (tester) async {
    final harness = await _pumpSpammer(tester);

    await tester.tap(find.byKey(const ValueKey('delete-preset')));
    await tester.pump();
    await tester.pump();

    expect(find.text('The last preset cannot be deleted.'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('spammer-preset-note')),
      findsOneWidget,
      reason: 'the refusal uses the tab\'s existing preset note line',
    );
    // The guard is a refusal, not an edit: the draft stays clean and nothing is sent.
    expect(find.byKey(const ValueKey('dirty-spammer')), findsNothing);
    expect(harness.lastSentWithCmd('config.validate'), isNull);
    expect(harness.lastSentWithCmd('config.apply'), isNull);
  });

  testWidgets('a preset that is not the last one can still be deleted', (
    tester,
  ) async {
    final block = _block();
    (block['presets'] as Map<String, dynamic>)['second'] = {'3': 1.0};
    await _pumpSpammer(tester, spammer: block);

    // The first preset in sorted order is "buffs" (chars: b < s), and two of them exist.
    await tester.tap(find.byKey(const ValueKey('delete-preset')));
    await tester.pump();
    await tester.pump();

    expect(find.byKey(const ValueKey('spammer-preset-note')), findsNothing);
    expect(find.byKey(const ValueKey('dirty-spammer')), findsOneWidget);
  });
}

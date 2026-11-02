/// The Quest tab: renders flows/sequences/steps, edits them, and saves the whole `quest` block; the
/// core's dangling-reference line is rendered verbatim when a save is refused.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/config_json.dart';
import 'package:seal_shell/ui/tabs/quest_tab.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

Map<String, dynamic> _questBlock() => {
  'active': 'run1',
  'presets': {
    'run1': {
      'initial_wait_seconds': 2.5,
      'loops': 2,
      'steps': [
        {'action': 'click', 'value': null, 'delay_seconds': 0.3},
        {'action': 'key', 'value': '1', 'delay_seconds': 0.2},
      ],
    },
  },
  'active_sequence': 'seq1',
  'sequences': {
    'seq1': {
      'initial_wait_seconds': 5.0,
      'loops': 3,
      'entries': [
        {'preset': 'run1', 'times': 2},
      ],
    },
  },
  'guard_px': 8,
};

Finder _field(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(TextField),
);

Future<AppHarness> _pumpQuest(
  WidgetTester tester, {
  Map<String, dynamic>? quest,
}) async {
  tester.view.physicalSize = const ui.Size(1800, 2600);
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
      'quest': quest ?? _questBlock(),
    },
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(body: QuestTab(model: harness.model)),
    ),
  );
  await tester.pump();
  // The cursor guard is the collapsed Advanced section (§6 "Config tabs"): open it before the
  // guard field is asserted on or edited.
  await tester.tap(find.byKey(const ValueKey('fold-toggle-Advanced')));
  await tester.pump();
  return harness;
}

Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets('renders flows/sequences/steps and saves the whole quest block', (
    tester,
  ) async {
    final harness = await _pumpQuest(tester);

    // The selected flow's steps and the selected sequence's entries are rendered.
    expect(find.text('Steps — run1'), findsOneWidget);
    expect(find.text('Entries — seq1'), findsOneWidget);
    expect(
      tester.widget<TextField>(_field('quest-step-delay-1')).controller?.text,
      '0.2',
    );
    expect(
      tester.widget<TextField>(_field('quest-entry-times-0')).controller?.text,
      '2',
    );

    await tester.enterText(_field('quest-guard'), '12');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-quest')), findsOneWidget);

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-quest')));
    await _flush(tester);

    final apply = harness.lastSentWithCmd('config.apply');
    expect(apply, isNotNull);
    final sections =
        (apply!['args'] as Map<String, dynamic>)['sections']
            as Map<String, dynamic>;
    expect(sections.keys, ['quest']);
    final quest = sections['quest'] as Map<String, dynamic>;
    expect(quest['guard_px'], 12);
    expect(quest['active'], 'run1');
    final flow =
        (quest['presets'] as Map<String, dynamic>)['run1']
            as Map<String, dynamic>;
    expect(asJsonList(flow['steps']), hasLength(2));
    final sequence =
        (quest['sequences'] as Map<String, dynamic>)['seq1']
            as Map<String, dynamic>;
    expect(
      (asJsonList(sequence['entries']).first as Map<String, dynamic>)['preset'],
      'run1',
    );

    expect(find.byKey(const ValueKey('dirty-quest')), findsNothing);
    expect(find.byKey(const ValueKey('save-ok-quest')), findsOneWidget);
  });

  testWidgets('renders the core dangling-reference line verbatim, no apply', (
    tester,
  ) async {
    final harness = await _pumpQuest(tester);
    await tester.enterText(_field('quest-guard'), '12');
    await tester.pump();

    harness.answerConfigCommands(
      errors: [
        'sequence "seq1" names a flow that does not exist: "ghost"',
        'save refused',
      ],
    );
    await tester.tap(find.byKey(const ValueKey('save-quest')));
    await _flush(tester);

    expect(
      find.text('sequence "seq1" names a flow that does not exist: "ghost"'),
      findsOneWidget,
    );
    expect(find.text('save refused'), findsOneWidget);
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    expect(find.byKey(const ValueKey('dirty-quest')), findsOneWidget);
  });

  testWidgets('Cancel restores the mirror value', (tester) async {
    await _pumpQuest(tester);
    await tester.enterText(_field('quest-guard'), '12');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-quest')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('cancel-quest')));
    await tester.pump();

    expect(find.byKey(const ValueKey('dirty-quest')), findsNothing);
    expect(
      tester.widget<TextField>(_field('quest-guard')).controller?.text,
      '8',
    );
  });

  testWidgets('a non-key step hides its stored value (and the save keeps it)', (
    tester,
  ) async {
    final harness = await _pumpQuest(
      tester,
      quest: {
        'active': 'run1',
        'presets': {
          'run1': {
            'initial_wait_seconds': 2.5,
            'loops': 2,
            'steps': [
              {'action': 'click', 'value': 'stale', 'delay_seconds': 0.3},
              {'action': 'key', 'value': '1', 'delay_seconds': 0.2},
            ],
          },
        },
        'active_sequence': '',
        'sequences': <String, dynamic>{},
        'guard_px': 8,
      },
    );

    expect(find.byKey(const ValueKey('quest-step-value-0')), findsNothing);
    expect(find.byKey(const ValueKey('quest-step-value-1')), findsOneWidget);
    expect(
      tester.widget<TextField>(_field('quest-step-value-1')).controller?.text,
      '1',
    );

    await tester.enterText(_field('quest-guard'), '12');
    await tester.pump();
    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-quest')));
    await _flush(tester);
    final apply = harness.lastSentWithCmd('config.apply')!;
    final sections =
        (apply['args'] as Map<String, dynamic>)['sections']
            as Map<String, dynamic>;
    final step =
        ((sections['quest'] as Map<String, dynamic>)['presets']
                as Map<String, dynamic>)['run1']['steps'][0]
            as Map<String, dynamic>;
    expect(
      step['value'],
      'stale',
      reason: 'hiding the field must not drop the stored value',
    );
  });

  testWidgets('the last flow cannot be deleted (v2 guard)', (tester) async {
    final harness = await _pumpQuest(tester);

    await tester.tap(find.byKey(const ValueKey('quest-delete-flow')));
    await tester.pump();
    await tester.pump();

    expect(
      find.text('This is the only flow — create another before deleting it.'),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('quest-note')), findsOneWidget);
    // The guard refuses, it does not edit: the draft stays clean and nothing is sent.
    expect(find.byKey(const ValueKey('dirty-quest')), findsNothing);
    expect(harness.lastSentWithCmd('config.validate'), isNull);
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    expect(find.text('Steps — run1'), findsOneWidget);
  });

  testWidgets(
    'a second flow can be deleted, and a sequence delete needs no guard',
    (tester) async {
      final quest = _questBlock();
      (quest['presets'] as Map<String, dynamic>)['run2'] = {
        'initial_wait_seconds': 1.0,
        'loops': 1,
        'steps': <dynamic>[],
      };
      await _pumpQuest(tester, quest: quest);

      // Delete the edited flow (run1) while two exist: it is a normal draft edit.
      await tester.tap(find.byKey(const ValueKey('quest-delete-flow')));
      await tester.pump();
      await tester.pump();
      expect(find.byKey(const ValueKey('quest-note')), findsNothing);
      expect(find.byKey(const ValueKey('dirty-quest')), findsOneWidget);

      // v2 deletes a sequence with only a confirmation (its ConfirmAndDelete has no count guard):
      // removing one never removes what a single-flow run needs.
      await tester.tap(find.byKey(const ValueKey('quest-delete-sequence')));
      await tester.pump();
      await tester.pump();
      expect(find.byKey(const ValueKey('quest-note')), findsNothing);
      expect(find.byKey(const ValueKey('quest-no-sequence')), findsOneWidget);
    },
  );
}

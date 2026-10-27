/// The Gem tab: renders the merged gem block, edits the PORTABLE half and saves it through the
/// `settings:` overlay — never the calibrated positions/slots.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/ui/tabs/gem_tab.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

Map<String, dynamic> _gemBlock() => {
  'grades': ['N', 'G', 'DG'],
  'start_grade': 'N',
  'empty_mode': 'advance_grade_clear',
  'empty_streak': 2,
  'colored_gap_min': 40,
  'save_empty_captures': false,
  'move_mode': 'arduino',
  'grade_positions': {
    'N': [727, 696],
    'G': [777, 696],
    'DG': [827, 696],
    'Register': [835, 724],
    'Combine': [785, 871],
  },
  'resource_gems': [
    [450, 696],
    [520, 696],
    [590, 696],
  ],
  'result_gem_area': [610, 800, 220, 60],
  'empty_distance': 0.18,
};

Finder _field(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(TextField),
);

Future<AppHarness> _pumpGem(WidgetTester tester) async {
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
      'gem': _gemBlock(),
    },
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(body: GemTab(model: harness.model)),
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
  testWidgets('renders the gem block and saves ONLY the portable settings', (
    tester,
  ) async {
    final harness = await _pumpGem(tester);

    // Calibrated geometry is visible, labelled read-only.
    expect(find.textContaining('Grade N'), findsWidgets);
    expect(find.text('[727, 696]'), findsOneWidget);
    expect(find.text('[450, 696]'), findsOneWidget);
    expect(find.textContaining('read-only this wave'), findsOneWidget);

    // The grade ladder is fixed: rendered, but no add/remove/edit control exists.
    for (final grade in ['N', 'G', 'DG']) {
      expect(
        find.byKey(ValueKey('gem-grade-$grade')),
        findsOneWidget,
        reason: 'the fixed grade is rendered read-only',
      );
    }
    expect(find.byKey(const ValueKey('add-gem-grade')), findsNothing);
    expect(find.byKey(const ValueKey('new-gem-grade')), findsNothing);
    expect(find.byKey(const ValueKey('gem-grade-remove-0')), findsNothing);
    expect(find.byKey(const ValueKey('gem-grade-0')), findsNothing);

    await tester.enterText(_field('gem-empty-streak'), '5');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-gem')), findsOneWidget);

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-gem')));
    await _flush(tester);

    final apply = harness.lastSentWithCmd('config.apply');
    expect(apply, isNotNull);
    final sections =
        (apply!['args'] as Map<String, dynamic>)['sections']
            as Map<String, dynamic>;
    expect(sections.keys, ['settings']);
    final settings = sections['settings'] as Map<String, dynamic>;
    expect(settings.keys, ['gem']);
    final gem = settings['gem'] as Map<String, dynamic>;
    expect(gem['empty_streak'], 5);
    expect(
      gem.containsKey('grades'),
      isFalse,
      reason: 'the fixed grade ladder is not this tab\'s to send',
    );
    expect(
      gem.containsKey('grade_positions'),
      isFalse,
      reason: 'calibrated positions are not this tab\'s to send',
    );
    expect(gem.containsKey('resource_gems'), isFalse);
    expect(gem.containsKey('result_gem_area'), isFalse);
    expect(gem['move_mode'], 'arduino');

    expect(find.byKey(const ValueKey('dirty-gem')), findsNothing);
    expect(find.byKey(const ValueKey('save-ok-gem')), findsOneWidget);
  });

  testWidgets('renders the core validation lines verbatim and does not apply', (
    tester,
  ) async {
    final harness = await _pumpGem(tester);
    await tester.enterText(_field('gem-empty-streak'), '5');
    await tester.pump();

    harness.answerConfigCommands(
      errors: ['the start grade must be one of the grades'],
    );
    await tester.tap(find.byKey(const ValueKey('save-gem')));
    await _flush(tester);

    expect(
      find.text('the start grade must be one of the grades'),
      findsOneWidget,
    );
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    expect(find.byKey(const ValueKey('dirty-gem')), findsOneWidget);
  });

  testWidgets('Cancel restores the mirror value', (tester) async {
    await _pumpGem(tester);
    await tester.enterText(_field('gem-empty-streak'), '5');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-gem')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('cancel-gem')));
    await tester.pump();

    expect(find.byKey(const ValueKey('dirty-gem')), findsNothing);
    expect(
      tester.widget<TextField>(_field('gem-empty-streak')).controller?.text,
      '2',
    );
  });
}

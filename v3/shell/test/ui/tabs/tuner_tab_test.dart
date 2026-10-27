/// The Tuner tab: renders the merged tuner block, edits the PORTABLE half and saves it through the
/// `settings:` overlay — never the calibrated geometry.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/ui/tabs/tuner_tab.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

Map<String, dynamic> _tunerBlock() => {
  'grade_order': ['N', 'G', 'DG', 'XG', 'SG'],
  'target_grade': 'DG',
  'max_retries': 5,
  'save_captures': false,
  'ocr_retries': 2,
  'spring_mode': 'hid',
  'mouse_guard': 'stop',
  'guard_px': 8,
  'recenter_max': 20,
  'models': {'detector': 'models/ch_PP-OCRv4_det_infer.onnx'},
  'timing': {'click_enter_delay': 0.4, 'ocr_delay': 0.8},
  'grade_colors': {
    'yellow': {'r_min': 120},
  },
  'filter': {
    'enabled': true,
    'match_mode': 'any',
    'require_grade': 'DG',
    'rules': [
      {'name': '減少傷害', 'count': 2, 'min': 1, 'max': 1},
    ],
    'override_rules': <Object?>[],
  },
  'ocr': {
    'region': {'left': 1140, 'top': 840, 'width': 300, 'height': 320},
    'grade_area': {'x1': 1, 'y1': 1, 'x2': 232, 'y2': 44},
  },
  'spring_point': [800, 900],
};

/// A minimal `attributes.get` answer (the tuner's rule-name dropdowns read the dictionary here).
Map<String, dynamic> _attributesAnswer({List<String>? names}) => {
  'variant': 'tw',
  'dictionary': 'attributes.yaml',
  'attributes': {
    'attributes': [
      for (final name in names ?? const ['攻擊力', '減少傷害'])
        {
          'name': name,
          'category': 'attack',
          'variants': [name],
        },
    ],
    'per_level_stats': <String>[],
    'negative_attr_markers': <String>[],
    'text_fixes': <String, dynamic>{},
  },
};

Finder _field(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(TextField),
);

Future<AppHarness> _pumpTuner(
  WidgetTester tester, {
  Map<String, dynamic>? attributes,
  Map<String, dynamic>? tuner,
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
      'tuner': tuner ?? _tunerBlock(),
    },
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  // The tab fetches the dictionary on first build; answer it before the first frame it needs it.
  harness.onWrite = (line, fake) {
    final message = fake.sentJson(fake.sent.length - 1);
    if (message['cmd'] == 'attributes.get') {
      fake.emitJson(
        okResponse(message['id'] as String, attributes ?? _attributesAnswer()),
      );
    }
  };
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(body: TunerTab(model: harness.model)),
    ),
  );
  for (var i = 0; i < 8; i++) {
    await tester.pump();
  }
  return harness;
}

Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

/// The `tuner` section one save sent (through the settings overlay).
Map<String, dynamic> _savedTuner(AppHarness harness) {
  final apply = harness.lastSentWithCmd('config.apply')!;
  final sections =
      (apply['args'] as Map<String, dynamic>)['sections']
          as Map<String, dynamic>;
  return (sections['settings'] as Map<String, dynamic>)['tuner']
      as Map<String, dynamic>;
}

/// Scrolls a control into view and taps it (the rule lists sit below the first screen).
Future<void> _tapVisible(WidgetTester tester, String key) async {
  final finder = find.byKey(ValueKey(key));
  await tester.ensureVisible(finder);
  await tester.pump();
  await tester.tap(finder);
}

void main() {
  testWidgets('renders the tuner block and saves ONLY the portable settings', (
    tester,
  ) async {
    final harness = await _pumpTuner(tester);

    // The merged block's portable values are rendered, and the calibrated half is shown read-only.
    expect(
      tester.widget<TextField>(_field('max-retries')).controller?.text,
      '5',
    );
    expect(find.textContaining('left 1140, top 840'), findsOneWidget);
    expect(find.textContaining('read-only this wave'), findsOneWidget);

    await tester.enterText(_field('max-retries'), '7');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-tuner')), findsOneWidget);

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-tuner')));
    await _flush(tester);

    expect(harness.lastSentWithCmd('config.validate'), isNotNull);
    final apply = harness.lastSentWithCmd('config.apply');
    expect(apply, isNotNull);
    final sections =
        (apply!['args'] as Map<String, dynamic>)['sections']
            as Map<String, dynamic>;
    expect(sections.keys, [
      'settings',
    ], reason: 'portable saves use the overlay');
    final settings = sections['settings'] as Map<String, dynamic>;
    expect(settings.keys, ['tuner']);
    final tuner = settings['tuner'] as Map<String, dynamic>;
    expect(tuner['max_retries'], 7);
    expect(
      tuner.containsKey('ocr'),
      isFalse,
      reason: 'calibrated geometry is not this tab\'s to send',
    );
    expect(tuner.containsKey('models'), isFalse);
    expect(tuner.containsKey('grade_colors'), isFalse);
    expect(
      (tuner['filter'] as Map<String, dynamic>)['match_mode'],
      'any',
      reason: 'the untouched fields the tab owns stay in the draft',
    );

    expect(find.byKey(const ValueKey('dirty-tuner')), findsNothing);
    expect(find.byKey(const ValueKey('save-ok-tuner')), findsOneWidget);
  });

  testWidgets('renders the core validation lines verbatim and does not apply', (
    tester,
  ) async {
    final harness = await _pumpTuner(tester);
    await tester.enterText(_field('max-retries'), '7');
    await tester.pump();

    harness.answerConfigCommands(
      errors: ['the target grade must appear in the grade order'],
    );
    await tester.tap(find.byKey(const ValueKey('save-tuner')));
    await _flush(tester);

    expect(
      find.text('the target grade must appear in the grade order'),
      findsOneWidget,
    );
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    expect(find.byKey(const ValueKey('dirty-tuner')), findsOneWidget);
  });

  testWidgets('Cancel restores the mirror value', (tester) async {
    await _pumpTuner(tester);
    await tester.enterText(_field('max-retries'), '7');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-tuner')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('cancel-tuner')));
    await tester.pump();

    expect(find.byKey(const ValueKey('dirty-tuner')), findsNothing);
    expect(
      tester.widget<TextField>(_field('max-retries')).controller?.text,
      '5',
    );
  });

  testWidgets('(none) clears the target grade from the override', (
    tester,
  ) async {
    final harness = await _pumpTuner(tester);

    await tester.tap(find.byKey(const ValueKey('target-grade')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('(none — use the shipped default)').last);
    await tester.pumpAndSettle();

    expect(find.byKey(const ValueKey('dirty-tuner')), findsOneWidget);
    expect(find.text('(none — use the shipped default)'), findsOneWidget);

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-tuner')));
    await _flush(tester);

    final apply = harness.lastSentWithCmd('config.apply')!;
    final sections =
        (apply['args'] as Map<String, dynamic>)['sections']
            as Map<String, dynamic>;
    final tuner =
        (sections['settings'] as Map<String, dynamic>)['tuner']
            as Map<String, dynamic>;
    expect(
      tuner.containsKey('target_grade'),
      isFalse,
      reason: 'a cleared Option is OMITTED from the override, not sent as null',
    );
  });

  testWidgets('the grade ladder is fixed and read-only', (tester) async {
    await _pumpTuner(tester);

    for (final grade in ['N', 'G', 'DG', 'XG', 'SG']) {
      expect(
        find.byKey(ValueKey('grade-order-$grade')),
        findsOneWidget,
        reason: 'the fixed grade ladder is rendered',
      );
    }
    expect(find.byKey(const ValueKey('add-grade')), findsNothing);
    expect(find.byKey(const ValueKey('new-grade-name')), findsNothing);
    expect(find.byKey(const ValueKey('grade-remove-0')), findsNothing);
    expect(find.byKey(const ValueKey('grade-up-0')), findsNothing);
    expect(find.byKey(const ValueKey('grade-down-0')), findsNothing);
  });

  testWidgets('rule names are a dropdown of the loaded dictionary', (
    tester,
  ) async {
    final harness = await _pumpTuner(tester);

    expect(
      tester
          .widget<DropdownButton<String>>(
            find.byKey(const ValueKey('rules-name-0')),
          )
          .value,
      '減少傷害',
    );

    await _tapVisible(tester, 'rules-name-0');
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('rules-name-0-item-攻擊力')), findsWidgets);
    await tester.tap(find.byKey(const ValueKey('rules-name-0-item-攻擊力')).last);
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('dirty-tuner')), findsOneWidget);

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-tuner')));
    await _flush(tester);

    final filter = _savedTuner(harness)['filter'] as Map<String, dynamic>;
    expect((filter['rules'] as List).first['name'], '攻擊力');
  });

  testWidgets('a stored rule name outside the dictionary stays selected', (
    tester,
  ) async {
    final tuner = _tunerBlock();
    final rules = (tuner['filter'] as Map<String, dynamic>)['rules'] as List;
    (rules.first as Map<String, dynamic>)['name'] = 'UnknownAttr';
    final harness = await _pumpTuner(tester, tuner: tuner);

    expect(
      tester
          .widget<DropdownButton<String>>(
            find.byKey(const ValueKey('rules-name-0')),
          )
          .value,
      'UnknownAttr',
      reason: 'a stored name outside the dictionary is never lost',
    );

    harness.answerConfigCommands();
    await tester.enterText(_field('max-retries'), '6');
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('save-tuner')));
    await _flush(tester);

    final filter = _savedTuner(harness)['filter'] as Map<String, dynamic>;
    expect((filter['rules'] as List).first['name'], 'UnknownAttr');
  });

  testWidgets('an empty dictionary disables Add and says why', (tester) async {
    await _pumpTuner(tester, attributes: _attributesAnswer(names: []));

    expect(
      find.byKey(const ValueKey('rules-attributes-reason')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('override_rules-attributes-reason')),
      findsOneWidget,
    );
    expect(
      tester
          .widget<FilledButton>(find.byKey(const ValueKey('rules-add')))
          .onPressed,
      isNull,
    );
    expect(
      tester
          .widget<FilledButton>(
            find.byKey(const ValueKey('override_rules-add')),
          )
          .onPressed,
      isNull,
    );
    expect(
      tester
          .widget<DropdownButton<String>>(
            find.byKey(const ValueKey('rules-new-name')),
          )
          .onChanged,
      isNull,
    );
    expect(
      tester
          .widget<DropdownButton<String>>(
            find.byKey(const ValueKey('rules-name-0')),
          )
          .onChanged,
      isNull,
      reason: 'no dictionary names means no valid new pick',
    );
  });

  testWidgets('adding a rule picks a dictionary name and saves it', (
    tester,
  ) async {
    final harness = await _pumpTuner(tester);

    await _tapVisible(tester, 'rules-new-name');
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('rules-new-name-item-攻擊力')), findsWidgets);
    await tester.tap(
      find.byKey(const ValueKey('rules-new-name-item-攻擊力')).last,
    );
    await tester.pumpAndSettle();
    await _tapVisible(tester, 'rules-add');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-tuner')), findsOneWidget);

    harness.answerConfigCommands();
    await tester.tap(find.byKey(const ValueKey('save-tuner')));
    await _flush(tester);

    final filter = _savedTuner(harness)['filter'] as Map<String, dynamic>;
    final savedRules = filter['rules'] as List;
    expect(savedRules.length, 2);
    expect((savedRules.last as Map<String, dynamic>)['name'], '攻擊力');
    expect((savedRules.last as Map<String, dynamic>)['count'], 1);
  });
}

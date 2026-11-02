/// The Attributes tab: a read-only render of `attributes.get` — groups/counts, OCR variants, the
/// per-level/negative/text-fix sections, client-side search, and the "no client resolved" state.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/ui/tabs/attributes_tab.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

Map<String, dynamic> _attributesAnswer({
  String? variant = 'tw',
  String? dictionary = 'attributes.yaml',
}) => {
  'variant': variant,
  'dictionary': dictionary,
  'attributes': {
    'attributes': [
      {
        'name': '攻擊力',
        'category': 'attack',
        'variants': ['攻擊力', '攻击力'],
      },
      {
        'name': '減少傷害',
        'category': 'defense',
        'variants': ['減少傷害'],
      },
      {
        'name': '力量',
        'category': 'plv',
        'variants': ['力量'],
      },
    ],
    'per_level_stats': ['STAT-Y'],
    'negative_attr_markers': ['MARK-X'],
    'text_fixes': {
      'whole': <Object?>[],
      'substring': [
        {'from': '攻', 'to': '攻'},
      ],
      'simplified_traditional': <Object?>[],
      'final': <Object?>[],
      'regex': <Object?>[],
    },
  },
};

Future<AppHarness> _pumpAttributes(
  WidgetTester tester, {
  required Map<String, dynamic> answer,
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
    },
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  harness.onWrite = (line, fake) {
    final message = fake.sentJson(fake.sent.length - 1);
    if (message['cmd'] == 'attributes.get') {
      fake.emitJson(okResponse(message['id'] as String, answer));
    }
  };
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(body: AttributesTab(model: harness.model)),
    ),
  );
  await tester.pump();
  await tester.pump();
  return harness;
}

void main() {
  testWidgets(
    'renders groups/counts/OCR variants and filters by name and form',
    (tester) async {
      await _pumpAttributes(tester, answer: _attributesAnswer());

      expect(find.text('attributes.yaml'), findsOneWidget);
      expect(find.text('tw'), findsOneWidget);
      expect(find.text('attack (1)'), findsOneWidget);
      expect(find.text('defense (1)'), findsOneWidget);
      expect(find.text('plv (1)'), findsOneWidget);
      expect(find.text('攻擊力 · 攻击力'), findsOneWidget);
      expect(find.text('3 of 3 attributes'), findsOneWidget);
      // The separate read-only sections.
      expect(find.text('STAT-Y'), findsOneWidget);
      expect(find.text('MARK-X'), findsOneWidget);
      expect(find.text('攻 → 攻'), findsOneWidget);

      // Filter over names: only the matching attribute's category remains.
      await tester.enterText(
        find.byKey(const ValueKey('attributes-search')),
        '傷害',
      );
      await tester.pump();
      expect(find.text('1 of 3 attributes'), findsOneWidget);
      expect(find.byKey(const ValueKey('attribute-減少傷害')), findsOneWidget);
      expect(find.byKey(const ValueKey('attribute-攻擊力')), findsNothing);

      // Filter over an OCR VARIANT: simplified 攻击力 matches the traditional entry.
      await tester.enterText(
        find.byKey(const ValueKey('attributes-search')),
        '攻击',
      );
      await tester.pump();
      expect(find.byKey(const ValueKey('attribute-攻擊力')), findsOneWidget);
      expect(find.byKey(const ValueKey('attribute-減少傷害')), findsNothing);
    },
  );

  testWidgets(
    'zero matches hide the tables and show the EmptyState with Clear',
    (tester) async {
      await _pumpAttributes(tester, answer: _attributesAnswer());

      await tester.enterText(
        find.byKey(const ValueKey('attributes-search')),
        'NOT_A_NAME',
      );
      await tester.pump();

      expect(find.text('0 of 3 attributes'), findsOneWidget);
      expect(find.text("No names match 'NOT_A_NAME'"), findsOneWidget);
      // The tables are hidden while a query is active: an empty result must not look half-filtered.
      expect(find.text('attack (1)'), findsNothing);
      expect(find.text('STAT-Y'), findsNothing);
      expect(find.text('MARK-X'), findsNothing);
      expect(find.text('攻 → 攻'), findsNothing);

      await tester.tap(find.byKey(const ValueKey('attributes-clear-search')));
      await tester.pump();

      expect(find.text('3 of 3 attributes'), findsOneWidget);
      expect(find.text('attack (1)'), findsOneWidget);
      expect(find.text('STAT-Y'), findsOneWidget);
      expect(
        tester
            .widget<TextField>(find.byKey(const ValueKey('attributes-search')))
            .controller
            ?.text,
        '',
      );
    },
  );

  testWidgets('variant null renders the explicit no-client line', (
    tester,
  ) async {
    await _pumpAttributes(
      tester,
      answer: _attributesAnswer(variant: null, dictionary: null),
    );

    expect(find.byKey(const ValueKey('attributes-no-variant')), findsOneWidget);
    expect(find.textContaining('No client resolved'), findsOneWidget);
    // No attribute list or search is meaningful without a variant.
    expect(find.byKey(const ValueKey('attributes-search')), findsNothing);
    expect(find.byKey(const ValueKey('attributes-refresh')), findsOneWidget);
  });
}

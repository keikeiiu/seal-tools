/// Increment A: kept-alive tabs and unsaved-draft protection.
///
/// A draft must survive switching tabs; a core-side change to a block with a dirty draft must mark
/// the external-change banner (and wait for an explicit reconcile); with a clean draft the same
/// change re-seeds silently.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/ui/shell_app.dart';
import 'package:seal_shell/ui/tabs/spammer_tab.dart';

import '../../support/app_harness.dart';

Map<String, dynamic> _spammerBlock() => {
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
};

Map<String, dynamic> _changedSpammerBlock() => {
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
};

/// The Tuner tab lazily fetches the attribute dictionary on first build; answer it here so opening
/// that tab in a guard test leaves no request (and its 10 s timer) pending. The content does not
/// matter to these tests — only that the command is answered.
Map<String, dynamic> _attributesAnswer() => {
  'variant': 'tw',
  'dictionary': 'attributes.yaml',
  'attributes': {
    'attributes': <Object?>[],
    'per_level_stats': <String>[],
    'negative_attr_markers': <String>[],
    'text_fixes': <String, dynamic>{},
  },
};

Finder _field(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(TextField),
);

Future<AppHarness> _pumpShell(WidgetTester tester) async {
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
      'spammer': _spammerBlock(),
    },
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  harness.onWrite = (line, fake) {
    final message = fake.sentJson(fake.sent.length - 1);
    if (message['cmd'] == 'attributes.get') {
      fake.emitJson(okResponse(message['id'] as String, _attributesAnswer()));
    }
  };
  await tester.pumpWidget(ShellApp(model: harness.model));
  await tester.pump();
  return harness;
}

Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

Future<void> _openTab(WidgetTester tester, String label) async {
  await tester.tap(find.text(label));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('an unsaved draft survives switching tabs A -> B -> A', (
    tester,
  ) async {
    await _pumpShell(tester);
    await _openTab(tester, 'Spammer');
    expect(find.byType(SpammerTab), findsOneWidget);

    await tester.enterText(_field('key-seconds-0'), '9.5');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-spammer')), findsOneWidget);

    await _openTab(tester, 'Tuner');
    await _openTab(tester, 'Spammer');

    expect(
      find.byKey(const ValueKey('dirty-spammer')),
      findsOneWidget,
      reason: 'the tab was kept alive, so its unsaved draft survived',
    );
    expect(
      tester.widget<TextField>(_field('key-seconds-0')).controller?.text,
      '9.5',
    );
  });

  testWidgets(
    'a core change to a dirty block marks the external-change banner on return',
    (tester) async {
      final harness = await _pumpShell(tester);
      await _openTab(tester, 'Spammer');
      await tester.enterText(_field('key-seconds-0'), '9.5');
      await tester.pump();
      expect(find.byKey(const ValueKey('dirty-spammer')), findsOneWidget);

      harness.emitConfigChanged(
        rev: 9,
        block: 'spammer',
        values: _changedSpammerBlock(),
      );
      await _flush(tester);

      // Still the player's edit, now flagged.
      expect(
        find.byKey(const ValueKey('external-change-spammer')),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('dirty-spammer')), findsOneWidget);
      expect(
        tester.widget<TextField>(_field('key-seconds-0')).controller?.text,
        '9.5',
      );

      // The mark survives leaving and returning.
      await _openTab(tester, 'Tuner');
      await _openTab(tester, 'Spammer');
      expect(
        find.byKey(const ValueKey('external-change-spammer')),
        findsOneWidget,
      );

      // Explicit reconcile: reload the core's truth, then re-seed.
      harness.answerConfigCommands(
        reloadRev: 10,
        reloadedConfig: {
          'window': {'title': 'TW_LIVE'},
          'spammer': _changedSpammerBlock(),
        },
      );
      await tester.tap(
        find.byKey(const ValueKey('external-change-reconcile-spammer')),
      );
      await _flush(tester);

      expect(
        find.byKey(const ValueKey('external-change-spammer')),
        findsNothing,
      );
      expect(find.byKey(const ValueKey('dirty-spammer')), findsNothing);
      expect(
        tester.widget<TextField>(_field('key-name-0')).controller?.text,
        '3',
        reason: 'the draft now shows the core\'s block',
      );
    },
  );

  testWidgets('a core change to a clean block re-seeds silently', (
    tester,
  ) async {
    final harness = await _pumpShell(tester);
    await _openTab(tester, 'Spammer');
    expect(
      tester.widget<TextField>(_field('key-name-0')).controller?.text,
      '2',
    );

    harness.emitConfigChanged(
      rev: 9,
      block: 'spammer',
      values: _changedSpammerBlock(),
    );
    await _flush(tester);

    expect(find.byKey(const ValueKey('dirty-spammer')), findsNothing);
    expect(find.byKey(const ValueKey('external-change-spammer')), findsNothing);
    expect(
      tester.widget<TextField>(_field('key-name-0')).controller?.text,
      '3',
      reason: 'a clean draft re-seeds from the core without a banner',
    );
  });
}

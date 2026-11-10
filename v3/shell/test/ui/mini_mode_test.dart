/// The mini running bar (design: `.scratch/ui-audit/ui-demo-mini.html`, variant A):
/// the window while a foreground tool runs. The bar renders core facts only and has ONE primary
/// action per state: Stop while a run can be stopped, Full window otherwise. The resident Pet
/// Feeder never takes the bar over; it shows a paw badge beside the foreground tool.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/protocol/protocol.dart';
import 'package:seal_shell/ui/mini_mode.dart';
import 'package:seal_shell/ui/shell_app.dart';
import 'package:seal_shell/ui/theme.dart';

import '../support/app_harness.dart';

/// Pumps the mini bar itself at its real 460 × 168 content size, with the model already collapsed.
Future<AppHarness> _pumpBar(
  WidgetTester tester, {
  Map<String, dynamic>? config,
}) async {
  tester.view.physicalSize = const ui.Size(540, 260);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  final harness = AppHarness();
  addTearDown(harness.model.dispose);
  final connecting = harness.connect(config: config);
  await tester.pumpWidget(
    const MaterialApp(
      home: Scaffold(body: Center(child: CircularProgressIndicator())),
    ),
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  // Answer Stop so a test that presses it leaves no request timer pending.
  harness.onWrite = (line, fake) {
    final message = fake.sentJson(fake.sent.length - 1);
    if (message['cmd'] == 'tool.stop') {
      fake.emitJson(okResponse(message['id'] as String, {}));
    }
  };
  harness.emitState(
    tool: ToolId.tuner,
    message: 'Reading the grade line',
    countdown: 12,
  );
  for (var i = 0; i < 5; i++) {
    await tester.pump();
  }
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(
        backgroundColor: const Color(0xFF111318),
        body: Center(
          child: SizedBox(
            width: miniBarWidth,
            height: miniBarHeight,
            child: MiniRunningCard(model: harness.model),
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

Future<void> _flush(WidgetTester tester) async {
  for (var i = 0; i < 6; i++) {
    await tester.pump();
  }
}

void main() {
  testWidgets(
    'the running bar shows tool, phase, action, next moment and Stop',
    (tester) async {
      final harness = await _pumpBar(tester);

      expect(find.byKey(const ValueKey('mini-bar')), findsOneWidget);
      expect(find.text('Magic Tuner'), findsOneWidget);
      expect(find.text('running'), findsOneWidget);
      expect(find.text('Reading the grade line'), findsOneWidget);
      expect(find.byKey(const ValueKey('mini-when')), findsOneWidget);
      expect(find.byKey(const ValueKey('mini-stop')), findsOneWidget);
      expect(find.byKey(const ValueKey('mini-full')), findsNothing);

      await tester.tap(find.byKey(const ValueKey('mini-stop')));
      await _flush(tester);
      expect(harness.lastSentWithCmd('tool.stop'), isNotNull);
      expect(harness.lastSentWithCmd('tool.stop')!['args'], {'tool': 'tuner'});

      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 2));
    },
  );

  testWidgets('the running bar shows the live read detail', (tester) async {
    final harness = await _pumpBar(tester);
    harness.emitState(
      tool: ToolId.tuner,
      message: 'reading the grade line',
      countdown: 12,
      detail: const [
        {'label': 'Grade', 'value': 'DG'},
        {'label': 'Remaining', 'value': '9'},
        {'label': 'Attempt', 'value': '4'},
      ],
    );
    await _flush(tester);

    // The action sentence stays the primary line; the live facts are labelled chips below it.
    expect(find.text('reading the grade line'), findsOneWidget);
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('mini-fact-0')),
        matching: find.text('DG'),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('mini-fact-1')),
        matching: find.text('9'),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('mini-fact-2')),
        matching: find.text('4'),
      ),
      findsOneWidget,
    );
    // The full OCR fact set is available in the tooltip.
    expect(
      find.byTooltip('Grade: DG\nRemaining: 9\nAttempt: 4'),
      findsOneWidget,
    );

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the mini bar includes OCR attributes and the filter', (
    tester,
  ) async {
    final harness = await _pumpBar(tester);
    harness.emitState(
      tool: ToolId.tuner,
      message: '',
      countdown: 12,
      detail: const [
        {'label': 'Grade', 'value': 'N'},
        {'label': 'Remaining', 'value': '284'},
        {'label': 'Attempt', 'value': '2'},
        {'value': '魔法力=22'},
        {'value': '攻擊速度=4'},
        {'label': 'Filter', 'value': "'減少傷害' matched 0 attrs, need 2"},
      ],
    );
    await _flush(tester);

    expect(
      tester.widget<Text>(find.byKey(const ValueKey('mini-message'))).data,
      'Running',
    );
    expect(find.byKey(const ValueKey('mini-attributes')), findsOneWidget);
    expect(find.text('魔法力=22'), findsOneWidget);
    expect(find.text('攻擊速度=4'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('mini-fact-3')),
      findsNothing,
      reason: 'OCR attributes are inline text, not extra cards',
    );
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('mini-wide-fact-0')),
        matching: find.textContaining("'減少傷害' matched 0 attrs"),
      ),
      findsOneWidget,
    );
    expect(
      find.byTooltip(
        'Grade: N\nRemaining: 284\nAttempt: 2\n魔法力=22\n攻擊速度=4\n'
        "Filter: '減少傷害' matched 0 attrs, need 2",
      ),
      findsOneWidget,
    );

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('a filter-matched OCR attribute uses the accent colour', (
    tester,
  ) async {
    final harness = await _pumpBar(
      tester,
      config: {
        'tuner': {
          'filter': {
            'enabled': true,
            'rules': [
              {'name': '魔法力', 'count': 1, 'min': 1, 'max': 99},
            ],
          },
        },
      },
    );
    harness.emitState(
      tool: ToolId.tuner,
      message: 'reading',
      countdown: 12,
      detail: const [
        {'value': '魔法力=22'},
        {'value': '攻擊速度=4'},
      ],
    );
    await _flush(tester);

    final hit = tester.widget<Text>(
      find.byKey(const ValueKey('mini-attribute-0')),
    );
    final neutral = tester.widget<Text>(
      find.byKey(const ValueKey('mini-attribute-1')),
    );
    expect(hit.style?.fontWeight, FontWeight.w600);
    expect(neutral.style?.fontWeight, FontWeight.w400);
    expect(hit.style?.color, isNot(neutral.style?.color));

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets(
    'the resident Pet Feeder gets a badge; Stop targets the foreground run',
    (tester) async {
      final harness = await _pumpBar(tester);
      harness.emitState(tool: ToolId.pet, message: 'feeding row 1');
      await _flush(tester);

      expect(find.byKey(const ValueKey('mini-resident-pet')), findsOneWidget);
      expect(find.text('Magic Tuner'), findsOneWidget);
      expect(find.text('Pet Feeder'), findsNothing);

      await tester.tap(find.byKey(const ValueKey('mini-stop')));
      await _flush(tester);
      expect(harness.lastSentWithCmd('tool.stop')!['args'], {'tool': 'tuner'});

      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 2));
    },
  );

  testWidgets('a finished run switches the primary action to Full window', (
    tester,
  ) async {
    final harness = await _pumpBar(tester);
    harness.emitState(
      tool: ToolId.tuner,
      phase: ToolPhase.idle,
      message: 'DONE',
    );
    harness.emitToolFinished(
      tool: ToolId.tuner,
      message: 'Reached DG after 7 attempts',
      summary: {'grade': 'DG', 'attempts': 7},
      endedAt: '2026-10-30T12:34:56Z',
    );
    await _flush(tester);

    expect(find.text('Reached DG after 7 attempts'), findsOneWidget);
    expect(find.text('finished'), findsOneWidget);
    expect(find.byKey(const ValueKey('mini-stop')), findsNothing);
    expect(find.byKey(const ValueKey('mini-full')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('mini-full')));
    await _flush(tester);
    expect(harness.model.miniMode, isFalse);
    expect(harness.host.miniChromeCalls, [true, false]);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('the stopping state disables Stop with a spinner', (
    tester,
  ) async {
    final harness = await _pumpBar(tester);
    harness.emitState(
      tool: ToolId.tuner,
      phase: ToolPhase.stopping,
      message: 'Releasing the board…',
    );
    await _flush(tester);

    expect(find.text('Stopping…'), findsOneWidget);
    expect(
      tester
          .widget<FilledButton>(
            find.descendant(
              of: find.byKey(const ValueKey('mini-stop')),
              matching: find.byType(FilledButton),
            ),
          )
          .onPressed,
      isNull,
    );

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('a run swaps the shell for the bar; Full window restores it', (
    tester,
  ) async {
    tester.view.physicalSize = const ui.Size(1600, 2400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.reset);
    final harness = AppHarness();
    addTearDown(harness.model.dispose);
    final connecting = harness.connect();
    await tester.pumpWidget(ShellApp(model: harness.model));
    for (var i = 0; i < 3; i++) {
      await tester.pump();
    }
    await connecting;
    await tester.pump();
    expect(find.byKey(const ValueKey('logs-toggle')), findsOneWidget);

    harness.emitState(tool: ToolId.tuner, message: 'Running the rotation');
    for (var i = 0; i < 6; i++) {
      await tester.pump();
    }
    expect(find.byKey(const ValueKey('mini-bar')), findsOneWidget);
    expect(
      find.byKey(const ValueKey('logs-toggle')),
      findsNothing,
      reason: 'the full shell is parked offstage while the bar is shown',
    );

    await tester.tap(find.byKey(const ValueKey('mini-expand')));
    for (var i = 0; i < 6; i++) {
      await tester.pump();
    }
    expect(find.byKey(const ValueKey('mini-bar')), findsNothing);
    expect(find.byKey(const ValueKey('logs-toggle')), findsOneWidget);

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });
}

/// D — the cards consume the core's own status fields: message, countdown and rows render on a card
/// and update from a fresh `state` event with no reload. Audit result: the card code already rendered
/// all three (message line, countdown through the shared formatter/ticker, `_RowsView`); this test
/// pins that behaviour against the live-published shape so it cannot regress.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/protocol/protocol.dart';
import 'package:seal_shell/ui/card_grid.dart';
import 'package:seal_shell/ui/theme.dart';

import '../support/app_harness.dart';

Future<AppHarness> _pumpCards(WidgetTester tester) async {
  tester.view.physicalSize = const ui.Size(1600, 2400);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  final harness = AppHarness();
  addTearDown(harness.model.dispose);
  final connecting = harness.connect();
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(
        body: ListenableBuilder(
          listenable: harness.model,
          builder: (context, _) => CardGrid(model: harness.model),
        ),
      ),
    ),
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  await tester.pump();
  return harness;
}

Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets('a state event updates message, countdown and rows on the card', (
    tester,
  ) async {
    final harness = await _pumpCards(tester);

    harness.emitState(
      tool: ToolId.tuner,
      message: 'Starting in 5...',
      countdown: 5,
      rows: [
        AppHarness.row(
          name: 'row 1',
          status: 'OCR',
          detail: 'grade G',
          nextActionInSeconds: 5,
        ),
        AppHarness.row(name: 'row 2', status: 'attempt 2'),
      ],
    );
    await _flush(tester);

    expect(find.text('Starting in 5...'), findsOneWidget);
    expect(find.textContaining('next action in 5s'), findsOneWidget);
    expect(find.text('OCR'), findsOneWidget);
    expect(find.text('attempt 2'), findsOneWidget);
    expect(find.byKey(const ValueKey('row-tuner-0')), findsOneWidget);
    expect(find.byKey(const ValueKey('row-tuner-1')), findsOneWidget);

    // A fresh event replaces all three without any reload/config traffic.
    harness.emitState(
      tool: ToolId.tuner,
      message: 'OCR: attempt 3',
      countdown: 2,
      rows: [AppHarness.row(name: 'row 1', status: 'OCR')],
    );
    await _flush(tester);

    expect(find.text('Starting in 5...'), findsNothing);
    expect(find.text('OCR: attempt 3'), findsOneWidget);
    expect(find.textContaining('next action in 2s'), findsOneWidget);
    expect(find.byKey(const ValueKey('row-tuner-1')), findsNothing);
    expect(find.byKey(const ValueKey('row-tuner-0')), findsOneWidget);
    expect(harness.lastSentWithCmd('config.reload'), isNull);
    expect(harness.lastSentWithCmd('config.validate'), isNull);

    // Dispose the shared countdown ticker.
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });
}

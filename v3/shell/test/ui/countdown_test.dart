/// The card grid's ONE shared countdown ticker: displayed next-action seconds tick once per second,
/// reset when a fresh `state` event arrives, never invented for rows without the field, and the timer
/// dies with the page.
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
  testWidgets('the displayed countdown ticks, resets on state, and disposes', (
    tester,
  ) async {
    final harness = await _pumpCards(tester);

    harness.emitState(
      tool: ToolId.spammer,
      message: 'running',
      rows: [
        AppHarness.row(
          name: 'row 1',
          status: 'waiting',
          nextActionInSeconds: 5,
        ),
      ],
    );
    await _flush(tester);
    expect(find.text('in 5 s'), findsOneWidget);

    await tester.pump(const Duration(seconds: 1));
    expect(find.text('in 4 s'), findsOneWidget);
    await tester.pump(const Duration(seconds: 1));
    expect(find.text('in 3 s'), findsOneWidget);

    // A fresh state event re-publishes the baseline: the display restarts from it.
    harness.emitState(
      tool: ToolId.spammer,
      message: 'running',
      rows: [
        AppHarness.row(
          name: 'row 1',
          status: 'waiting',
          nextActionInSeconds: 5,
        ),
      ],
    );
    await _flush(tester);
    expect(find.text('in 5 s'), findsOneWidget);

    // A row without the field gets no countdown at all.
    harness.emitState(
      tool: ToolId.pet,
      message: 'row 1',
      rows: [AppHarness.row(name: 'row 1', status: 'boarding')],
    );
    await _flush(tester);
    expect(find.text('now'), findsNothing);

    // Leaving cancels the one ticker; no timer outlives the widget.
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });
}

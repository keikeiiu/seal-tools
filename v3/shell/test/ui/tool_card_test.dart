/// The Cards card as a rendering of the pins: a full running card (phase chip, message, countdown,
/// the four row fields with headers, Stop) versus an idle one-line tile (outlined Start, hints) —
/// plus the refusal/finished lines and the in-flight label ([PLAN-V3-UI-UX.md] §6 "Cards").
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/protocol/protocol.dart';
import 'package:seal_shell/ui/theme.dart';
import 'package:seal_shell/ui/tool_card.dart';

ToolSnapshot _snapshot({
  ToolId tool = ToolId.tuner,
  ToolPhase phase = ToolPhase.idle,
  String message = 'idle',
  double? countdown,
  List<ToolRow> rows = const [],
  String? error,
}) => ToolSnapshot(
  tool: tool,
  running: phase != ToolPhase.idle,
  phase: phase,
  message: message,
  countdownSeconds: countdown,
  rows: rows,
  error: error,
);

Widget _host(Widget card, {double height = 420}) => MaterialApp(
  theme: buildShellTheme(),
  home: Scaffold(
    body: Center(
      child: SizedBox(width: 560, height: height, child: card),
    ),
  ),
);

ToolCard _card({
  ToolId tool = ToolId.tuner,
  ToolSnapshot? snapshot,
  ToolFinished? finished,
  String? refusal,
  bool pending = false,
  bool ready = true,
  VoidCallback? onStart,
  VoidCallback? onStop,
  Widget? extras,
}) => ToolCard(
  tool: tool,
  snapshot: snapshot,
  finished: finished,
  refusal: refusal,
  pending: pending,
  ready: ready,
  extras: extras,
  onStart: onStart ?? () {},
  onStop: onStop ?? () {},
);

void main() {
  testWidgets('a running card shows the phase chip, message, headers and rows', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        _card(
          snapshot: _snapshot(
            phase: ToolPhase.running,
            message: 'fishing for a red gem',
            countdown: 60,
            rows: [
              ToolRow(
                name: 'row 1',
                status: 'boarding',
                detail: '藍天 · 3/min',
                nextActionInSeconds: 1800,
              ),
              ToolRow(name: 'row 2', status: 'waiting for a reload'),
            ],
          ),
        ),
      ),
    );

    expect(find.text('Magic Tuner'), findsOneWidget);
    expect(find.text('running'), findsOneWidget);
    expect(find.text('fishing for a red gem'), findsOneWidget);
    expect(find.text('next action in 1 m'), findsOneWidget);

    // The table has labels, and the four row fields render from the snapshot.
    expect(find.text('ROW'), findsOneWidget);
    expect(find.text('STATUS'), findsOneWidget);
    expect(find.text('DETAIL'), findsOneWidget);
    expect(find.text('NEXT'), findsOneWidget);
    expect(find.text('row 1'), findsOneWidget);
    expect(find.text('boarding'), findsOneWidget);
    expect(find.text('藍天 · 3/min'), findsOneWidget);
    expect(find.text('in 30 m'), findsOneWidget);
    expect(find.text('row 2'), findsOneWidget);
    expect(find.text('waiting for a reload'), findsOneWidget);

    // Running: Stop is the one action; there is no Start on a running card.
    expect(find.byKey(const ValueKey('start-tuner')), findsNothing);
    expect(
      tester
          .widget<FilledButton>(find.byKey(const ValueKey('stop-tuner')))
          .onPressed,
      isNotNull,
    );

    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 2));
  });

  testWidgets('an idle tool is a one-line tile with an outlined Start', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        _card(
          snapshot: _snapshot(message: 'Ready — target DG · 5 retries'),
        ),
      ),
    );

    expect(find.text('Magic Tuner'), findsOneWidget);
    expect(find.text('Ready — target DG · 5 retries'), findsOneWidget);
    // The tile has no phase chip, no rows table and no Stop.
    expect(find.byKey(const ValueKey('rows-empty-tuner')), findsNothing);
    expect(find.byKey(const ValueKey('stop-tuner')), findsNothing);
    expect(find.text('ROW'), findsNothing);
    expect(
      tester
          .widget<OutlinedButton>(find.byKey(const ValueKey('start-tuner')))
          .onPressed,
      isNotNull,
    );
  });

  testWidgets('Start and Stop call back; pending disables and labels the action', (
    tester,
  ) async {
    var starts = 0;
    var stops = 0;
    await tester.pumpWidget(
      _host(_card(onStart: () => starts++, onStop: () => stops++)),
    );

    await tester.tap(find.byKey(const ValueKey('start-tuner')));
    expect(starts, 1);

    // An in-flight START is shown on the idle tile: the verb becomes a labelled spinner and the
    // button is disabled, but the phase chip does not appear (the core has not published one).
    await tester.pumpWidget(
      _host(_card(pending: true, onStart: () => starts++)),
    );
    expect(find.text('Starting…'), findsOneWidget);
    expect(
      tester
          .widget<OutlinedButton>(find.byKey(const ValueKey('start-tuner')))
          .onPressed,
      isNull,
    );

    // A running tool's Stop carries the same in-flight rule.
    await tester.pumpWidget(
      _host(
        _card(
          snapshot: _snapshot(phase: ToolPhase.running),
          pending: true,
          onStart: () => starts++,
          onStop: () => stops++,
        ),
      ),
    );
    expect(find.text('Stopping…'), findsOneWidget);
    expect(
      tester
          .widget<FilledButton>(find.byKey(const ValueKey('stop-tuner')))
          .onPressed,
      isNull,
      reason: 'a command is already in flight',
    );
  });

  testWidgets('the in-flight Start label replaces the verb, never the phase', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        _card(
          pending: true,
          snapshot: _snapshot(phase: ToolPhase.idle),
        ),
      ),
    );
    expect(find.text('Starting…'), findsOneWidget);
    // The tile is still idle: the core has not published a phase.
    expect(find.text('running'), findsNothing);
  });

  testWidgets('a refusal line renders verbatim on the idle tile', (tester) async {
    await tester.pumpWidget(
      _host(
        _card(
          tool: ToolId.spammer,
          finished: const ToolFinished(
            tool: ToolId.spammer,
            reason: ToolStopKind.cancelled,
            message: 'run finished: cancelled',
          ),
          refusal: 'another tool is running',
          snapshot: _snapshot(tool: ToolId.spammer),
        ),
      ),
    );

    expect(find.byKey(const ValueKey('refusal-spammer')), findsOneWidget);
    expect(find.text('another tool is running'), findsOneWidget);
    expect(find.text('run finished: cancelled'), findsOneWidget);
  });

  testWidgets('Hold Space is its own idle tile with no configuration link', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        _card(
          tool: ToolId.holdspace,
          snapshot: _snapshot(tool: ToolId.holdspace, message: ''),
        ),
      ),
    );
    expect(find.text('Hold Space'), findsOneWidget);
    expect(find.text('Off · spacebar auto-pickup'), findsOneWidget);
    expect(find.byKey(const ValueKey('start-holdspace')), findsOneWidget);
  });
}

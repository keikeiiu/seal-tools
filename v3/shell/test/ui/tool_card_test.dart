/// The card as a rendering of the pins: phase, message, countdown, the four row fields, the
/// `rows: []` line, the finished/refusal lines and the Start/Stop enabled states.
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

Widget _host(Widget card) => MaterialApp(
  theme: buildShellTheme(),
  home: Scaffold(
    body: Center(child: SizedBox(width: 400, height: 320, child: card)),
  ),
);

ToolCard _card({
  ToolId tool = ToolId.tuner,
  ToolSnapshot? snapshot,
  ToolFinished? finished,
  String? refusal,
  String? holdSpaceLine,
  bool pending = false,
  bool ready = true,
  VoidCallback? onStart,
  VoidCallback? onStop,
}) => ToolCard(
  tool: tool,
  snapshot: snapshot,
  finished: finished,
  refusal: refusal,
  holdSpaceLine: holdSpaceLine,
  pending: pending,
  ready: ready,
  onStart: onStart ?? () {},
  onStop: onStop ?? () {},
);

void main() {
  testWidgets('renders a running snapshot with the pinned row fields', (
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
    expect(find.text('next action in 1m'), findsOneWidget);

    // The four row fields, each rendered from the snapshot, not inferred.
    expect(find.text('row 1'), findsOneWidget);
    expect(find.text('boarding'), findsOneWidget);
    expect(find.text('藍天 · 3/min'), findsOneWidget);
    expect(find.text('in 30m'), findsOneWidget);
    expect(find.text('row 2'), findsOneWidget);
    expect(find.text('waiting for a reload'), findsOneWidget);

    // Running: Start disabled, Stop enabled.
    expect(
      tester
          .widget<FilledButton>(find.byKey(const ValueKey('start-tuner')))
          .onPressed,
      isNull,
    );
    expect(
      tester
          .widget<OutlinedButton>(find.byKey(const ValueKey('stop-tuner')))
          .onPressed,
      isNotNull,
    );
  });

  testWidgets('renders rows:[] as its own line, not an empty gap', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        _card(
          snapshot: _snapshot(message: 'idle', rows: const []),
        ),
      ),
    );

    expect(find.byKey(const ValueKey('rows-empty-tuner')), findsOneWidget);
    expect(find.text('No rows to show'), findsOneWidget);
    // Idle: Start enabled, Stop disabled.
    expect(
      tester
          .widget<FilledButton>(find.byKey(const ValueKey('start-tuner')))
          .onPressed,
      isNotNull,
    );
    expect(
      tester
          .widget<OutlinedButton>(find.byKey(const ValueKey('stop-tuner')))
          .onPressed,
      isNull,
    );
  });

  testWidgets('Start and Stop call back; pending disables both', (
    tester,
  ) async {
    var starts = 0;
    var stops = 0;
    await tester.pumpWidget(
      _host(_card(onStart: () => starts++, onStop: () => stops++)),
    );

    await tester.tap(find.byKey(const ValueKey('start-tuner')));
    expect(starts, 1);

    // No snapshot -> idle -> Stop is disabled.
    await tester.tap(find.byKey(const ValueKey('stop-tuner')));
    expect(stops, 0);

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
    expect(
      tester
          .widget<FilledButton>(find.byKey(const ValueKey('start-tuner')))
          .onPressed,
      isNull,
    );
    expect(
      tester
          .widget<OutlinedButton>(find.byKey(const ValueKey('stop-tuner')))
          .onPressed,
      isNull,
      reason: 'a command is already in flight',
    );
  });

  testWidgets('shows the refusal line verbatim and the finish line', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        _card(
          tool: ToolId.spammer,
          holdSpaceLine: 'running — holding',
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
    // Hold Space reports on the Spammer card (it has no card of its own).
    expect(find.text('Hold Space: running — holding'), findsOneWidget);
  });
}

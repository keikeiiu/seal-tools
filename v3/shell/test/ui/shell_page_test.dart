/// The window as the model renders it: seven cards, the Hold Space chrome toggle, the status strip,
/// event-driven card updates, Start/Stop through the model (no optimistic phase) and the crash view's
/// Restart rule.
///
/// `testWidgets` runs in a FakeAsync zone: timers do not fire and futures that await them do not
/// complete outside a pump. The handshake is therefore started WITHOUT awaiting it and completed by
/// pumping (see [_pumpShell]); later stream/command delivery is flushed with pumps — never with an
/// awaited `Future.delayed` in the fake zone.
library;

import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/version.dart';
import 'package:seal_shell/protocol/protocol.dart';
import 'package:seal_shell/ui/shell_app.dart';
import 'package:seal_shell/ui/tool_card.dart';

import '../support/app_harness.dart';

/// Pumps the shell ready to interact.
///
/// testWidgets runs in a FakeAsync zone, so the spawn→hello chain is started WITHOUT awaiting it
/// and completed by pumping the frame loop — awaiting it before the first pump is the classic way to
/// park forever. The first frame is the connection surface (PLAN §2), the later pumps land hello,
/// then connecting is already complete.
Future<AppHarness> _pumpShell(
  WidgetTester tester, {
  FakeHost? host,
  Map<String, dynamic>? config,
}) async {
  // The grid is lazy: the default 800x600 surface would only lay out the first few cards, so a
  // "seven cards" assertion has to mean all seven were really built.
  tester.view.physicalSize = const ui.Size(1600, 2400);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  final harness = AppHarness(host: host);
  addTearDown(harness.model.dispose);
  final connecting = harness.connect(config: config);
  await tester.pumpWidget(ShellApp(model: harness.model));
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  await tester.pump();
  return harness;
}

/// Flushes microtask-delivered stream events and command completions into a rendered frame.
Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets(
    'eight cards including Hold Space, the rail groups and the status strip',
    (tester) async {
      final harness = await _pumpShell(tester);

      // D2: Hold Space is its own card now; the top-bar toggle is gone.
      expect(find.byType(ToolCard), findsNWidgets(8));
      expect(find.byKey(const ValueKey('card-holdspace')), findsOneWidget);
      expect(find.byKey(const ValueKey('holdspace-toggle')), findsNothing);

      // D1: the rail has the three groups and every destination.
      for (final key in const [
        'cards',
        'spammer',
        'tuner',
        'gem',
        'buy',
        'sell',
        'quest',
        'pet',
        'attributes',
        'arduino',
        'hotkeys',
        'setup',
        'calibrate',
      ]) {
        expect(
          find.byKey(ValueKey('rail-$key')),
          findsOneWidget,
          reason: 'the rail is the navigation now (PLAN-V3-UI-UX D1)',
        );
      }

      expect(
        tester.widget<Text>(find.byKey(const ValueKey('status-core'))).data,
        'core 0.1.0 · rev 7',
      );
      expect(
        tester.widget<Text>(find.byKey(const ValueKey('status-root'))).data,
        contains(r'root C:\SealTools'),
      );

      // The Hold Space CARD is the only place its state is started (D2), and the phase still comes
      // from the core: tapping Start sends the command, not a local flip.
      await tester.tap(find.byKey(const ValueKey('start-holdspace')));
      await tester.pump();
      expect(harness.lastRequest['cmd'], 'tool.start');
      expect(harness.lastRequest['args'], {'tool': 'holdspace'});
      harness.respondOk();
      await _flush(tester);
      expect(find.text('Hold Space'), findsOneWidget);
    },
  );

  testWidgets(
    'the app bar shows the product version while hello keeps the client identity',
    (tester) async {
      final harness = await _pumpShell(tester);

      expect(find.text('v$productVersion'), findsOneWidget);
      final hello = harness.lastSentWithCmd('hello')!;
      final args = hello['args'] as Map<String, dynamic>;
      expect(
        args['client'],
        '0.1.0',
        reason:
            'the app bar shows the product version; the handshake carries '
            'the protocol client identity, which stays 0.1.0',
      );
    },
  );

  testWidgets('the rail collapses from the top bar and Ctrl+B', (tester) async {
    await _pumpShell(tester);
    expect(find.byKey(const ValueKey('shell-rail')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('rail-toggle')));
    await tester.pump();
    expect(
      find.byKey(const ValueKey('shell-rail')),
      findsNothing,
      reason: 'the rail is page chrome, not a tab: hiding it must not unmount it forever',
    );
    expect(find.byKey(const ValueKey('cards-list')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('rail-toggle')));
    await tester.pump();
    expect(find.byKey(const ValueKey('shell-rail')), findsOneWidget);

    // Ctrl+B is the same toggle, so the sidebar is reachable without aiming at the button.
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyB);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();
    expect(find.byKey(const ValueKey('shell-rail')), findsNothing);
  });

  testWidgets(
    'state events update cards, including rows:[] and a two-row card',
    (tester) async {
      final harness = await _pumpShell(tester);

      harness.emitState(
        tool: ToolId.pet,
        message: 'row 2: looking',
        rows: [
          AppHarness.row(
            name: 'row 1',
            status: 'boarding',
            detail: '藍天 · 3/min',
            nextActionInSeconds: 1800,
          ),
          AppHarness.row(name: 'row 2', status: 'waiting for a reload'),
        ],
      );
      harness.emitState(
        tool: ToolId.tuner,
        running: false,
        phase: ToolPhase.idle,
        message: 'idle',
        rows: const [],
      );
      await _flush(tester);

      expect(find.text('row 2: looking'), findsOneWidget);
      expect(find.text('row 1'), findsOneWidget);
      expect(find.text('boarding'), findsOneWidget);
      expect(find.text('in 30 m'), findsOneWidget);
      // An idle tool is a one-line tile now: no phase chip, no rows table.
      expect(find.byKey(const ValueKey('rows-empty-tuner')), findsNothing);
      expect(
        tester
            .widget<OutlinedButton>(find.byKey(const ValueKey('start-tuner')))
            .onPressed,
        isNotNull,
      );
    },
  );

  testWidgets(
    'Start sends tool.start and the card never flips phase optimistically',
    (tester) async {
      final harness = await _pumpShell(tester);

      await tester.tap(find.byKey(const ValueKey('start-tuner')));
      await tester.pump();

      expect(harness.lastRequest['cmd'], 'tool.start');
      expect(harness.lastRequest['args'], {'tool': 'tuner'});
      // No state event yet: the tile shows the in-flight label and stays idle (no running chip).
      expect(find.text('Starting…'), findsOneWidget);
      expect(find.text('running'), findsNothing);

      harness.respondOk();
      await _flush(tester);

      expect(find.text('running'), findsNothing);
      expect(
        tester
            .widget<OutlinedButton>(find.byKey(const ValueKey('start-tuner')))
            .onPressed,
        isNotNull,
      );
    },
  );

  testWidgets('a refusal line shows on the card in the core\'s own words', (
    tester,
  ) async {
    final harness = await _pumpShell(tester);

    await tester.tap(find.byKey(const ValueKey('start-spammer')));
    await tester.pump();
    harness.respondError(
      'tool_busy',
      message: 'another tool is running',
      detail: {'tool': 'pet'},
    );
    await _flush(tester);

    expect(find.byKey(const ValueKey('refusal-spammer')), findsOneWidget);
    expect(find.text('another tool is running'), findsOneWidget);
  });

  testWidgets('a dead core shows the crash view with Restart', (tester) async {
    final harness = await _pumpShell(tester);
    harness.emitStderr('seal-core: cannot load config: boom');
    await _flush(tester);

    harness.crash();
    await _flush(tester);

    expect(find.text('The core is not running'), findsOneWidget);
    expect(find.byKey(const ValueKey('crash-restart')), findsOneWidget);
    expect(find.text('seal-core: cannot load config: boom'), findsOneWidget);
  });

  testWidgets('a bad_version crash hides Restart (replace the binaries)', (
    tester,
  ) async {
    final harness = AppHarness();
    addTearDown(harness.model.dispose);
    harness.onWrite = (line, fake) {
      final message = fake.sentJson(fake.sent.length - 1);
      if (message['cmd'] == 'hello') {
        fake.emitJson(
          errorResponse(
            message['id'] as String,
            'bad_version',
            message: 'protocol 2 is not 1',
            // PROTOCOL §6 pins `detail.expected` as the CORE's protocol version.
            detail: {'expected': 1},
          ),
        );
      }
    };
    // Start the failed handshake without awaiting it: under FakeAsync only pumps complete it.
    unawaited(harness.model.connect());
    await tester.pumpWidget(ShellApp(model: harness.model));
    for (var i = 0; i < 5; i++) {
      await tester.pump();
    }

    expect(find.text('The core is not running'), findsOneWidget);
    expect(find.byKey(const ValueKey('crash-restart')), findsNothing);
    // Assert the crash message itself (its own widget), not a text finder that may legitimately match
    // the same sentence in more than one place in the crash view.
    final crashMessage = tester
        .widget<Text>(find.byKey(const ValueKey('crash-message')))
        .data;
    expect(crashMessage, contains('protocol mismatch'));
    expect(crashMessage, contains('the core expects 1'));
  });

  testWidgets(
    'the pin starts off and a toggle applies at once then saves the exact payload',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpShell(tester, host: host);
      final pin = find.byKey(const ValueKey('app-bar-always-on-top'));

      expect(tester.widget<IconButton>(pin).isSelected, isFalse);
      expect(host.alwaysOnTopCalls, [
        false,
      ], reason: 'connect applies the absent default once');

      harness.answerConfigCommands();
      await tester.tap(pin);
      for (var i = 0; i < 4; i++) {
        await tester.pump();
      }

      final apply = harness.lastSentWithCmd('config.apply');
      expect(apply, isNotNull);
      expect((apply!['args'] as Map<String, dynamic>)['sections'], {
        'settings': {
          'ui': {'always_on_top': true},
        },
      }, reason: 'the pin writes the portable settings section, not a file');
      expect(host.alwaysOnTopCalls, [
        false,
        true,
      ], reason: 'the host is applied before the save completes');
      expect(tester.widget<IconButton>(pin).isSelected, isTrue);
      expect(harness.model.alwaysOnTop, isTrue);
    },
  );

  testWidgets(
    'a configured pin is applied at connect and toggles off to false',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpShell(
        tester,
        host: host,
        config: {
          'window': {'title': 'TW_LIVE'},
          'settings': {
            'ui': {'always_on_top': true},
          },
        },
      );
      final pin = find.byKey(const ValueKey('app-bar-always-on-top'));

      expect(tester.widget<IconButton>(pin).isSelected, isTrue);
      expect(host.alwaysOnTopCalls, [true]);

      harness.answerConfigCommands();
      await tester.tap(pin);
      for (var i = 0; i < 4; i++) {
        await tester.pump();
      }

      final apply = harness.lastSentWithCmd('config.apply');
      expect((apply!['args'] as Map<String, dynamic>)['sections'], {
        'settings': {
          'ui': {'always_on_top': false},
        },
      });
      expect(host.alwaysOnTopCalls, [true, false]);
      expect(tester.widget<IconButton>(pin).isSelected, isFalse);
      expect(harness.model.alwaysOnTop, isFalse);
    },
  );

  testWidgets(
    'a refused pin save reverts the host and shows the core line, not a lie',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpShell(tester, host: host);
      final pin = find.byKey(const ValueKey('app-bar-always-on-top'));
      const refusal = 'the settings block is malformed';

      harness.answerConfigCommands(errors: [refusal]);
      await tester.tap(pin);
      for (var i = 0; i < 4; i++) {
        await tester.pump();
      }

      expect(host.alwaysOnTopCalls, [
        false,
        true,
        false,
      ], reason: 'the optimistic apply is reverted to the saved mirror value');
      expect(tester.widget<IconButton>(pin).isSelected, isFalse);
      expect(harness.model.alwaysOnTop, isFalse);
      expect(find.text(refusal), findsOneWidget);
      expect(harness.lastSentWithCmd('config.apply'), isNull);

      // Let the SnackBar's own timer expire so the widget test leaves no pending timer.
      await tester.pump(const Duration(seconds: 5));
      await tester.pumpAndSettle();
    },
  );

  testWidgets(
    'the logs drawer opens the install logs folder and names the last run log',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpShell(tester, host: host);
      harness.emitToolFinished(tool: ToolId.pet);
      await _flush(tester);

      await tester.tap(find.byKey(const ValueKey('logs-toggle')));
      await tester.pump();

      final action = find.byKey(const ValueKey('open-logs-folder'));
      expect(action, findsOneWidget);
      expect(find.textContaining('pet.log'), findsOneWidget);

      await tester.tap(action);
      await tester.pump();

      expect(host.openedLogsFolders, ['pet.log']);
    },
  );
}

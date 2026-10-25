/// The app model's state transitions, driven through the real protocol client over a fake transport.
///
/// These are protocol-behaviour tests with no widgets: a `state` event becomes a card snapshot, a
/// `tool.finished` becomes a line, a crash becomes the crash view's facts, and the typed protocol
/// errors land on the model's fields without the model inventing text.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/app.dart';
import 'package:seal_shell/protocol/protocol.dart';

import '../support/app_harness.dart';

void main() {
  test('hello fills the connection and the config mirror', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);

    await harness.connect();

    expect(harness.model.isReady, isTrue);
    expect(harness.model.connection.status, ConnectionStatus.ready);
    expect(harness.model.connection.coreVersion, '0.1.0');
    expect(harness.model.connection.root, r'C:\SealTools');
    expect(harness.model.config.rev, 7);
    expect(harness.model.config.root, r'C:\SealTools');
    expect(harness.model.config.coreVersion, '0.1.0');
    // Config blocks stay opaque: the mirror carries the core's JSON, not a Dart model of it.
    expect(harness.model.config.config['window'], isA<Map<String, dynamic>>());
    expect(harness.model.logs.entries, isNotEmpty);
  });

  test('state events populate the cards, including a rows:[] card', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);
    await harness.connect();

    harness.emitState(
      tool: ToolId.pet,
      message: 'row 2: looking',
      countdown: 1800,
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
    await settle();

    final pet = harness.model.tools.snapshot(ToolId.pet);
    expect(pet, isNotNull);
    expect(pet!.phase, ToolPhase.running);
    expect(pet.message, 'row 2: looking');
    expect(pet.rows, hasLength(2));
    expect(pet.rows.first.name, 'row 1');
    expect(pet.rows.first.detail, '藍天 · 3/min');
    expect(pet.rows.first.nextActionInSeconds, 1800);
    expect(pet.rows.last.detail, isNull);

    final tuner = harness.model.tools.snapshot(ToolId.tuner);
    expect(tuner, isNotNull);
    expect(
      tuner!.rows,
      isEmpty,
      reason: 'rows:[] is a card fact, not an error',
    );
    expect(tuner.phase, ToolPhase.idle);
    expect(harness.model.tools.snapshots, hasLength(2));
  });

  test('tool.finished records the per-tool and last finish lines', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);
    await harness.connect();

    harness.emitToolFinished(
      tool: ToolId.tuner,
      reason: ToolStopKind.cancelled,
      message: 'run finished: cancelled',
    );
    await settle();

    expect(harness.model.tools.lastFinished?.tool, ToolId.tuner);
    expect(
      harness.model.tools.finished(ToolId.tuner)?.reason,
      ToolStopKind.cancelled,
    );
    expect(
      harness.model.tools.finished(ToolId.tuner)?.message,
      'run finished: cancelled',
    );
  });

  test('core.stopping is kept for the status surface', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);
    await harness.connect();

    harness.emitCoreStopping(CoreStoppingReason.shutdown);
    await settle();

    expect(harness.model.tools.stopping?.reason, CoreStoppingReason.shutdown);
  });

  test('a log event lands as a core-origin entry with its level', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);
    await harness.connect();

    harness.emitLog('warn', 'retrying the board');
    await settle();

    final entry = harness.model.logs.entries.last;
    expect(entry.origin, LogOrigin.core);
    expect(entry.level, 'warn');
    expect(entry.text, 'retrying the board');
  });

  test(
    'a crash is a failed connection with the stderr tail, cards kept',
    () async {
      final harness = AppHarness();
      addTearDown(harness.dispose);
      await harness.connect();
      harness.emitState(tool: ToolId.pet, message: 'feeding');
      await settle();

      harness.emitStderr('seal-core: cannot load config: boom');
      await settle();
      harness.crash();
      await settle();

      expect(harness.model.connection.status, ConnectionStatus.failed);
      expect(harness.model.connection.failure, ConnectionFailure.coreExit);
      expect(
        harness.model.connection.failureMessage,
        contains('the core exited'),
      );
      expect(harness.model.connection.stderrTail, [
        'seal-core: cannot load config: boom',
      ]);
      expect(harness.model.connection.canRestart, isTrue);
      // The last rendered state is deliberately kept (PLAN §2).
      expect(harness.model.tools.snapshot(ToolId.pet)?.message, 'feeding');
    },
  );

  test('restart is one new process and keeps the cards', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);
    await harness.connect();
    harness.emitState(tool: ToolId.pet, message: 'feeding');
    await settle();
    harness.crash();
    await settle();
    expect(harness.model.connection.status, ConnectionStatus.failed);

    await harness.model.restart();
    await settle();

    expect(harness.spawnAttempts, 2);
    expect(harness.transports, hasLength(2));
    expect(harness.model.connection.status, ConnectionStatus.ready);
    expect(harness.model.tools.snapshot(ToolId.pet)?.message, 'feeding');
  });

  test('bad_version fails the connection and does not offer Restart', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);
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

    await harness.model.connect();

    expect(harness.model.connection.status, ConnectionStatus.failed);
    expect(harness.model.connection.failure, ConnectionFailure.badVersion);
    expect(harness.model.connection.canRestart, isFalse);
    expect(
      harness.model.connection.failureMessage,
      contains('protocol mismatch'),
    );
    expect(harness.model.connection.failureMessage, contains('speaks 1'));
    expect(harness.model.connection.failureMessage, contains('expects 1'));
  });

  test('a spawn failure is a rendered state, not a thrown exception', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);
    harness.spawnError = StateError('seal-core.exe was not found');

    await harness.model.connect();

    expect(harness.model.connection.status, ConnectionStatus.failed);
    expect(harness.model.connection.failure, ConnectionFailure.spawn);
    expect(harness.model.connection.canRestart, isTrue);
    expect(
      harness.model.connection.failureMessage,
      contains('seal-core.exe was not found'),
    );
  });

  test(
    'a tool refusal is stored verbatim and the pending flag clears',
    () async {
      final harness = AppHarness();
      addTearDown(harness.dispose);
      await harness.connect();

      final started = harness.model.startTool(ToolId.spammer);
      expect(harness.lastRequest['cmd'], 'tool.start');
      expect(harness.lastRequest['args'], {'tool': 'spammer'});
      expect(harness.model.tools.isPending(ToolId.spammer), isTrue);
      harness.respondError(
        'tool_busy',
        message: 'another tool is running',
        detail: {'tool': 'pet'},
      );

      expect(await started, isFalse);
      expect(
        harness.model.tools.refusal(ToolId.spammer),
        'another tool is running',
        reason: 'the core\'s line is stored verbatim, never rewritten',
      );
      expect(harness.model.tools.isPending(ToolId.spammer), isFalse);
    },
  );

  test('a successful start clears the old refusal and returns true', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);
    await harness.connect();

    final started = harness.model.startTool(ToolId.tuner);
    harness.respondOk();
    expect(await started, isTrue);
    expect(harness.model.tools.refusal(ToolId.tuner), isNull);
  });

  test('commands before ready are refused by the model, not sent', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);

    final started = await harness.model.startTool(ToolId.spammer);

    expect(started, isFalse);
    expect(harness.transports, isEmpty);
    expect(harness.model.logs.entries.last.text, contains('not ready'));
  });

  test(
    'config.apply updates the rev and mirrors the effective sections',
    () async {
      final harness = AppHarness();
      addTearDown(harness.dispose);
      await harness.connect();

      final applied = harness.model.applyConfig({
        'tuner': {'guard_px': 9},
      });
      expect(harness.lastRequest['cmd'], 'config.apply');
      expect(harness.lastRequest['args'], {
        'rev': 7,
        'sections': {
          'tuner': {'guard_px': 9},
        },
      });
      harness.respondOk({
        'rev': 8,
        'sections': {
          'tuner': {'guard_px': 9},
        },
      });

      final result = await applied;
      expect(result?.rev, 8);
      expect(harness.model.config.rev, 8);
      expect(harness.model.config.config['tuner'], {'guard_px': 9});
    },
  );

  test(
    'a conflict is surfaced with the core rev, and nothing is retried',
    () async {
      final harness = AppHarness();
      addTearDown(harness.dispose);
      await harness.connect();

      final applied = harness.model.applyConfig({
        'quest': {'active': 'run1'},
      });
      final sentBefore = harness.transport.sent.length;
      harness.respondError(
        'conflict',
        message: 'config changed while you were editing',
        detail: {
          'rev': 12,
          'sections': {
            'quest': {'active': 'run2'},
          },
        },
      );

      final result = await applied;
      expect(result, isNull);
      expect(harness.model.config.conflict, isNotNull);
      expect(harness.model.config.conflict?.conflictRev, 12);
      expect(harness.model.config.lastError?.code, ErrorCode.conflict);
      // No hidden retry: the shell does not re-send on its own.
      expect(harness.transport.sent.length, sentBefore);
    },
  );

  test('config.changed overlays the block and the new rev', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);
    await harness.connect();

    harness.emitConfigChanged(rev: 9, block: 'tuner', values: {'guard_px': 4});
    await settle();

    expect(harness.model.config.rev, 9);
    expect(harness.model.config.config['tuner'], {'guard_px': 4});
  });
}

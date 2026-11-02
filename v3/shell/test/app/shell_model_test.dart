/// The app model's state transitions, driven through the real protocol client over a fake transport.
///
/// These are protocol-behaviour tests with no widgets: a `state` event becomes a card snapshot, a
/// `tool.finished` becomes a line, a crash becomes the crash view's facts, and the typed protocol
/// errors land on the model's fields without the model inventing text.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/app.dart';
import 'package:seal_shell/app/dock_placement.dart';
import 'package:seal_shell/protocol/protocol.dart';

import '../support/app_harness.dart';

/// A logical window rect, written once so the placement tests stay readable.
WindowRect _rect(int left, int top, int width, int height) =>
    WindowRect(left: left, top: top, width: width, height: height);

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

  test('a configured always-on-top is applied once at connect', () async {
    final host = FakeHost();
    final harness = AppHarness(host: host);
    addTearDown(harness.dispose);

    await harness.connect(
      config: {
        'window': {'title': 'TW_LIVE'},
        'settings': {
          'ui': {'always_on_top': true},
        },
      },
    );

    expect(harness.model.alwaysOnTop, isTrue);
    expect(host.alwaysOnTopCalls, [true]);

    // A config event updates the mirror but deliberately does not re-apply the window setting.
    harness.emitConfigChanged(
      rev: 9,
      block: 'settings',
      values: {
        'ui': {'always_on_top': true},
      },
    );
    await settle();
    expect(harness.model.alwaysOnTop, isTrue);
    expect(host.alwaysOnTopCalls, [
      true,
    ], reason: 'apply-on-connect is one-shot');
  });

  test('an absent always-on-top applies false at connect', () async {
    final host = FakeHost();
    final harness = AppHarness(host: host);
    addTearDown(harness.dispose);

    await harness.connect();

    expect(harness.model.alwaysOnTop, isFalse);
    expect(host.alwaysOnTopCalls, [false]);
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

  test('pointer.place sends the client-relative point exactly', () async {
    final harness = AppHarness();
    addTearDown(harness.dispose);
    await harness.connect();

    final placed = harness.model.pointerPlace(120, 340);
    expect(harness.lastRequest['cmd'], 'pointer.place');
    expect(harness.lastRequest['args'], {'x': 120, 'y': 340});
    harness.respondOk();
    final result = await placed;

    expect(result.ok, isTrue);
    expect(result.value, isEmpty);
    // A pointer action is run input, never a config write.
    expect(harness.lastSentWithCmd('config.validate'), isNull);
    expect(harness.lastSentWithCmd('config.apply'), isNull);
  });

  test(
    'pointer.click sends the point and a refusal keeps the core line',
    () async {
      final harness = AppHarness();
      addTearDown(harness.dispose);
      await harness.connect();

      final clicked = harness.model.pointerClick(12, 34);
      expect(harness.lastRequest['cmd'], 'pointer.click');
      expect(harness.lastRequest['args'], {'x': 12, 'y': 34});
      harness.respondError(
        'game_not_foreground',
        message: 'the game window is not in front',
      );
      final result = await clicked;

      expect(result.ok, isFalse);
      expect(result.error?.code, ErrorCode.gameNotForeground);
      expect(
        result.errorLine,
        'the game window is not in front',
        reason: 'the core\'s line is stored verbatim, never rewritten',
      );
      expect(harness.lastSentWithCmd('config.validate'), isNull);
      expect(harness.lastSentWithCmd('config.apply'), isNull);
    },
  );

  // ── window placement (v2 parity: the launcher reopens where the player left it) ─────────────

  test(
    'a remembered placement is applied after hello, with no config write',
    () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);

      await harness.connect(
        config: {
          'window': {'title': 'TW_LIVE'},
          'ui': {
            'left': 120,
            'top': 80,
            'width': 1000,
            'expanded_height': 700,
            'pinned': false,
          },
        },
      );

      // The machine `ui` block's v2 names map onto the host call in LOGICAL coordinates: the
      // runner does the DPI conversion, the shell never sees a scale factor.
      expect(host.appliedRects, [
        const WindowRect(left: 120, top: 80, width: 1000, height: 700),
      ]);
      expect(
        harness.model.windowPlacement,
        const WindowRect(left: 120, top: 80, width: 1000, height: 700),
      );
      // Restoring is a window action, not a config write — the core writes `ui.window` only when
      // the PLAYER moves the window.
      expect(harness.lastSentWithCmd('ui.window'), isNull);
    },
  );

  test('a placement on a monitor that still exists is kept exactly', () async {
    final host = FakeHost(
      primaryWorkArea: const WindowRect(
        left: 0,
        top: 0,
        width: 1920,
        height: 1040,
      ),
      virtualScreen: const WindowRect(
        left: 0,
        top: 0,
        width: 3840,
        height: 1040,
      ),
    );
    final harness = AppHarness(host: host);
    addTearDown(harness.dispose);

    await harness.connect(
      config: {
        'window': {'title': 'TW_LIVE'},
        'ui': {
          'left': 2100,
          'top': 100,
          'width': 1000,
          'expanded_height': 700,
          'pinned': false,
        },
      },
    );

    expect(host.appliedRects, [
      const WindowRect(left: 2100, top: 100, width: 1000, height: 700),
    ], reason: 'a secondary monitor that still exists is not clamped away');
    expect(
      harness.model.logs.entries.any(
        (entry) => entry.text.contains('window placement clamped'),
      ),
      isFalse,
    );
  });

  test(
    'an off-screen placement is clamped into the primary work area',
    () async {
      final host = FakeHost(
        primaryWorkArea: const WindowRect(
          left: 0,
          top: 0,
          width: 1920,
          height: 1040,
        ),
      );
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);

      await harness.connect(
        config: {
          'window': {'title': 'TW_LIVE'},
          'ui': {
            'left': 5000,
            'top': -900,
            'width': 4000,
            'expanded_height': 3000,
            'pinned': false,
          },
        },
      );

      expect(host.appliedRects, [
        const WindowRect(left: 0, top: 0, width: 1920, height: 1040),
      ], reason: 'the saved monitor is gone, so the primary one wins');
      expect(
        harness.model.logs.entries.any(
          (entry) => entry.text.contains('window placement clamped'),
        ),
        isTrue,
      );
    },
  );

  test(
    'no remembered placement means the current rect becomes the baseline',
    () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);

      await harness.connect();

      expect(host.appliedRects, isEmpty);
      expect(host.windowRectQueries, 1);
      expect(
        harness.model.windowPlacement,
        const WindowRect(left: 10, top: 10, width: 1280, height: 720),
      );
    },
  );

  test('a settled move is saved once with the LAST rect', () async {
    final host = FakeHost();
    final harness = AppHarness(
      host: host,
      windowSaveDebounce: const Duration(milliseconds: 20),
    );
    addTearDown(harness.dispose);
    await harness.connect();

    // Two reports inside the debounce are one placement: the player dragged once.
    host.emitMove(_rect(100, 50, 1200, 800));
    host.emitMove(_rect(140, 90, 1300, 840));
    await Future<void>.delayed(const Duration(milliseconds: 120));

    final request = harness.lastSentWithCmd('ui.window');
    expect(request, isNotNull);
    expect(request!['args'], {
      'left': 140,
      'top': 90,
      'width': 1300,
      'height': 840,
      // The whole five-field machine block is written by the command, so the stored pin rides
      // along unchanged (this feature owns no pin; `settings.ui.always_on_top` is the portable
      // one).
      'pinned': false,
    });
    expect(
      harness.sentMessages.where((message) => message['cmd'] == 'ui.window'),
      hasLength(1),
    );
    harness.respondOk();
    await settle();

    // The restore/move echo of the rect just saved is not new information.
    host.emitMove(_rect(140, 90, 1300, 840));
    await Future<void>.delayed(const Duration(milliseconds: 120));
    expect(
      harness.sentMessages.where((message) => message['cmd'] == 'ui.window'),
      hasLength(1),
    );
  });

  test('a stored pin is carried through unchanged', () async {
    final host = FakeHost();
    final harness = AppHarness(
      host: host,
      windowSaveDebounce: const Duration(milliseconds: 20),
    );
    addTearDown(harness.dispose);
    await harness.connect(
      config: {
        'window': {'title': 'TW_LIVE'},
        'ui': {
          'left': 100,
          'top': 100,
          'width': 1280,
          'expanded_height': 720,
          'pinned': true,
        },
      },
    );

    host.emitMove(_rect(200, 200, 1280, 720));
    await Future<void>.delayed(const Duration(milliseconds: 120));

    expect(harness.lastSentWithCmd('ui.window')!['args'], {
      'left': 200,
      'top': 200,
      'width': 1280,
      'height': 720,
      'pinned': true,
    });
    harness.respondOk();
    await settle();
  });

  test(
    'no placement is saved while the capture has the window hidden',
    () async {
      final host = FakeHost(visible: false);
      final harness = AppHarness(
        host: host,
        windowSaveDebounce: const Duration(milliseconds: 20),
      );
      addTearDown(harness.dispose);
      await harness.connect();

      host.emitMove(_rect(100, 50, 1200, 800));
      await Future<void>.delayed(const Duration(milliseconds: 120));
      expect(
        harness.lastSentWithCmd('ui.window'),
        isNull,
        reason: 'a hidden window has no placement the player chose',
      );

      // The show ends the capture; the next settled move is a real placement and saves.
      host.visible = true;
      host.emitMove(_rect(200, 120, 1200, 800));
      await Future<void>.delayed(const Duration(milliseconds: 120));
      expect(harness.lastSentWithCmd('ui.window')!['args'], {
        'left': 200,
        'top': 120,
        'width': 1200,
        'height': 800,
        'pinned': false,
      });
      harness.respondOk();
      await settle();
    },
  );

  test('a refused ui.window is logged verbatim and the model stays usable', () async {
    final host = FakeHost();
    final harness = AppHarness(
      host: host,
      windowSaveDebounce: const Duration(milliseconds: 20),
    );
    addTearDown(harness.dispose);
    await harness.connect();

    host.emitMove(_rect(100, 50, 1200, 800));
    await Future<void>.delayed(const Duration(milliseconds: 120));
    expect(harness.lastSentWithCmd('ui.window'), isNotNull);
    const refusal = 'ui.window field "pinned" has the wrong type';
    harness.respondError('bad_request', message: refusal);
    await settle();

    expect(
      harness.model.logs.entries.map((entry) => entry.text),
      contains(refusal),
      reason: 'the core\'s line reaches the status surface unchanged',
    );
    expect(harness.model.isReady, isTrue);
    // The failed placement is not remembered as saved, so a later move retries.
    host.emitMove(_rect(110, 60, 1200, 800));
    await Future<void>.delayed(const Duration(milliseconds: 120));
    expect(
      harness.sentMessages.where((message) => message['cmd'] == 'ui.window'),
      hasLength(2),
    );
    harness.respondOk();
    await settle();
  });

  test(
    'a host that cannot restore the placement logs it and stays ready',
    () async {
      final host = FakeHost(failWindowCalls: true);
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);

      await harness.connect(
        config: {
          'window': {'title': 'TW_LIVE'},
          'ui': {
            'left': 1,
            'top': 2,
            'width': 3,
            'expanded_height': 4,
            'pinned': false,
          },
        },
      );

      expect(harness.model.isReady, isTrue);
      expect(host.appliedRects, isEmpty);
      expect(
        harness.model.logs.entries.any(
          (entry) => entry.text.contains('cannot restore the window placement'),
        ),
        isTrue,
      );
    },
  );

  group('mini running bar', () {
    test('a foreground run collapses; the resident pet never does', () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);
      await harness.connect();

      harness.emitState(tool: ToolId.pet, message: 'feeding');
      await settle();
      expect(
        harness.model.miniMode,
        isFalse,
        reason: 'the resident feeder keeps the full window',
      );
      expect(host.miniChromeCalls, isEmpty);

      harness.emitState(tool: ToolId.tuner, message: 'reading the grade');
      await settle();
      expect(harness.model.miniMode, isTrue);
      expect(harness.model.miniTool, ToolId.tuner);
      expect(host.minSizeCalls, [(360, 96)]);
      expect(host.miniChromeCalls, [true]);
      expect(
        host.appliedRects.last,
        _rect(1920 - 360 - 16, 1040 * 18 ~/ 100, 360, 96),
        reason: 'default dock: right edge, 16 px margin, a fifth down',
      );
      expect(host.alwaysOnTopCalls.last, isTrue);
    });

    test('the bar stays on the finished run until "Full window"', () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);
      await harness.connect();
      harness.emitState(tool: ToolId.tuner, message: 'running');
      await settle();
      expect(harness.model.miniMode, isTrue);

      // The core's exit path publishes the final idle snapshot, then tool.finished.
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
      await settle();
      expect(harness.model.miniMode, isTrue);
      expect(harness.model.miniTool, ToolId.tuner);
      expect(host.miniChromeCalls, [
        true,
      ], reason: 'a finished run does not pop the full window over the game');

      await harness.model.showFullWindow();
      await settle();
      expect(harness.model.miniMode, isFalse);
      expect(host.miniChromeCalls, [true, false]);
      expect(host.minSizeCalls.last, (1120, 680));
      expect(host.appliedRects.last, _rect(10, 10, 1280, 720));
      expect(host.alwaysOnTopCalls.last, isFalse);
    });

    test('the bar remembers its last snapped position', () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);
      await harness.connect();
      harness.emitState(tool: ToolId.tuner, message: 'running');
      await settle();
      expect(harness.model.miniMode, isTrue);

      await harness.model.moveMiniWindow(_rect(5, 5, 360, 96));
      await settle();
      expect(host.currentWindowRect, _rect(5, 5, 360, 96));
      await harness.model.settleMiniWindow();
      await settle();
      expect(
        host.currentWindowRect,
        _rect(16, 16, 360, 96),
        reason: 'a drag near the corner snaps to the 16 px edge inset',
      );

      await harness.model.showFullWindow();
      await settle();
      // Finish that run, then the next start reuses the remembered edge position.
      harness.emitState(
        tool: ToolId.tuner,
        phase: ToolPhase.idle,
        message: 'DONE',
      );
      harness.emitToolFinished(tool: ToolId.tuner);
      await settle();
      harness.emitState(tool: ToolId.tuner, message: 'another run');
      await settle();
      expect(harness.model.miniMode, isTrue);
      expect(host.currentWindowRect, _rect(16, 16, 360, 96));
    });

    test('"Full window" suppresses collapsing until the next run', () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);
      await harness.connect();
      harness.emitState(tool: ToolId.tuner, message: 'running');
      await settle();
      expect(harness.model.miniMode, isTrue);

      await harness.model.showFullWindow();
      await settle();
      expect(harness.model.miniMode, isFalse);

      // More events from the SAME run do not re-collapse.
      harness.emitState(tool: ToolId.tuner, message: 'still running');
      await settle();
      expect(harness.model.miniMode, isFalse);

      // The run ends; the next start collapses again.
      harness.emitState(
        tool: ToolId.tuner,
        phase: ToolPhase.idle,
        message: 'DONE',
      );
      harness.emitToolFinished(tool: ToolId.tuner);
      await settle();
      harness.emitState(tool: ToolId.tuner, message: 'next run');
      await settle();
      expect(harness.model.miniMode, isTrue);
    });

    test('a manual collapse uses the running foreground tool', () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);
      await harness.connect();
      expect(harness.model.miniCandidate, isNull);

      harness.emitState(tool: ToolId.pet, message: 'feeding');
      await settle();
      expect(harness.model.miniCandidate, isNull);

      harness.emitState(tool: ToolId.holdspace, message: 'holding');
      await settle();
      expect(harness.model.miniMode, isTrue);
      // "Full window" suppresses auto-collapse; the top-bar Mini action still collapses on demand.
      await harness.model.showFullWindow();
      await settle();
      expect(harness.model.miniMode, isFalse);
      expect(harness.model.miniCandidate, ToolId.holdspace);
      await harness.model.collapseToMini();
      await settle();
      expect(harness.model.miniMode, isTrue);
      expect(harness.model.miniTool, ToolId.holdspace);
    });
  });

  group('quick dock', () {
    test('opens as a right rail and remembers the full placement', () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);
      await harness.connect();

      await harness.model.enterDock();
      await settle();
      expect(harness.model.dockMode, isTrue);
      expect(host.minSizeCalls, [(300, 480)]);
      expect(host.miniChromeCalls, [true]);
      expect(host.appliedRects.last, _rect(1604, 280, 300, 480));
      expect(host.alwaysOnTopCalls.last, isTrue);

      await harness.model.exitDock();
      await settle();
      expect(harness.model.dockMode, isFalse);
      expect(host.miniChromeCalls, [true, false]);
      expect(host.minSizeCalls.last, (1120, 680));
      expect(host.appliedRects.last, _rect(10, 10, 1280, 720));
      expect(host.alwaysOnTopCalls.last, isFalse);
    });

    test('all five placements resize and move the same dock window', () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);
      await harness.connect();
      harness.answerConfigCommands();
      await harness.model.enterDock();
      await settle();

      await harness.model.setDockPlacement(DockPlacement.bottomRight);
      await settle();
      expect(host.minSizeCalls.last, (340, 460));
      expect(host.appliedRects.last, _rect(1564, 564, 340, 460));

      await harness.model.setDockPlacement(DockPlacement.leftRail);
      await settle();
      expect(host.minSizeCalls.last, (300, 480));
      expect(host.appliedRects.last, _rect(16, 280, 300, 480));

      await harness.model.setDockPlacement(DockPlacement.topCentre);
      await settle();
      expect(host.minSizeCalls.last, (380, 500));
      expect(host.appliedRects.last, _rect(770, 16, 380, 500));

      await harness.model.setDockPlacement(DockPlacement.bottomLeft);
      await settle();
      expect(host.appliedRects.last, _rect(16, 564, 340, 460));
      final saved =
          harness.lastSentWithCmd('config.apply')!['args']
              as Map<String, dynamic>;
      final settings =
          (saved['sections'] as Map<String, dynamic>)['settings']
              as Map<String, dynamic>;
      final ui = (settings['ui'] as Map<String, dynamic>);
      final dock = ui['quick_dock'] as Map<String, dynamic>;
      expect(dock['placement'], 'bottom_left');
      expect(dock['collapsed'], false);
    });

    test('a saved dock preference is restored at connect', () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);
      await harness.connect(
        config: {
          'window': {'title': 'TW_LIVE'},
          'settings': {
            'ui': {
              'quick_dock': {'placement': 'bottom_right', 'collapsed': true},
            },
          },
        },
      );
      expect(harness.model.dockPlacement, DockPlacement.bottomRight);
      expect(harness.model.dockCollapsed, isTrue);

      await harness.model.enterDock();
      await settle();
      expect(host.minSizeCalls.last, (220, 44));
      expect(host.appliedRects.last, _rect(1684, 980, 220, 44));
    });

    test('the dock collapses to each placement\'s resting form', () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);
      await harness.connect();
      harness.answerConfigCommands();
      await harness.model.enterDock();
      await settle();

      await harness.model.setDockCollapsed(true);
      await settle();
      expect(harness.model.dockCollapsed, isTrue);
      expect(host.minSizeCalls.last, (46, 150));
      expect(host.appliedRects.last, _rect(1858, 445, 46, 150));

      await harness.model.setDockPlacement(DockPlacement.bottomRight);
      await settle();
      expect(host.minSizeCalls.last, (220, 44));
      expect(host.appliedRects.last, _rect(1684, 980, 220, 44));

      await harness.model.setDockCollapsed(false);
      await settle();
      expect(harness.model.dockCollapsed, isFalse);
      expect(host.minSizeCalls.last, (340, 460));
    });

    test('a dock start hands the window to the run bar', () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);
      await harness.connect();
      await harness.model.enterDock();
      await settle();
      expect(harness.model.dockMode, isTrue);

      harness.emitState(tool: ToolId.tuner, message: 'running');
      await settle();
      expect(harness.model.dockMode, isFalse);
      expect(harness.model.miniMode, isTrue);
      expect(harness.model.miniTool, ToolId.tuner);
      expect(host.minSizeCalls.last, (360, 96));
      expect(host.appliedRects.last, _rect(1544, 187, 360, 96));

      await harness.model.showFullWindow();
      await settle();
      expect(harness.model.miniMode, isFalse);
      expect(host.appliedRects.last, _rect(10, 10, 1280, 720));
      expect(host.miniChromeCalls.last, isFalse);
    });

    test('the finished run bar can hand the window back to the dock', () async {
      final host = FakeHost();
      final harness = AppHarness(host: host);
      addTearDown(harness.dispose);
      await harness.connect();
      await harness.model.enterDock();
      await settle();
      harness.emitState(tool: ToolId.tuner, message: 'running');
      await settle();
      harness.emitState(
        tool: ToolId.tuner,
        phase: ToolPhase.idle,
        message: 'DONE',
      );
      harness.emitToolFinished(tool: ToolId.tuner);
      await settle();

      await harness.model.openDockFromMini();
      await settle();
      expect(harness.model.miniMode, isFalse);
      expect(harness.model.dockMode, isTrue);
      expect(host.minSizeCalls.last, (300, 480));
      expect(host.appliedRects.last, _rect(1604, 280, 300, 480));

      await harness.model.showFullWindowFromDock();
      await settle();
      expect(harness.model.dockMode, isFalse);
      expect(host.appliedRects.last, _rect(10, 10, 1280, 720));
    });
  });
}

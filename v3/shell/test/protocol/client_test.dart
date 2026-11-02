/// The client's state machine over a fake transport: ids, out-of-order replies, events, timeouts,
/// closure and the handshake — all without a process.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/protocol/protocol.dart';

import '../support/fake_transport.dart';

/// A client wired to a [FakeTransport], with a helper for the handshake.
class _Core {
  _Core({Duration timeout = const Duration(seconds: 10)}) {
    transport = FakeTransport();
    client = ProtocolClient(
      transport,
      clientVersion: '1.2.3',
      requestTimeout: timeout,
    );
  }

  late final FakeTransport transport;
  late final ProtocolClient client;

  /// Answers `hello` and completes the handshake. Afterwards the transport's `onWrite` is cleared so
  /// each test can answer explicitly.
  Future<HelloResult> connect({int protocol = 1}) async {
    transport.onWrite = (line) {
      final message = transport.sentJson(transport.sent.length - 1);
      if (message['cmd'] == 'hello') {
        transport.emitJson(
          okResponse(message['id'] as String, helloResult(protocol: protocol)),
        );
      }
    };
    final hello = await client.handshake();
    transport.onWrite = null;
    return hello;
  }

  /// The id of the request at [index] in the sent log (0 is the hello).
  String idAt(int index) => transport.sentJson(index)['id'] as String;

  Future<void> dispose() async {
    await transport.dispose();
  }
}

void main() {
  group('handshake', () {
    test('sends hello with the shell version and stores the result', () async {
      final core = _Core();
      final hello = await core.connect();

      expect(core.transport.sent, hasLength(1));
      final sent = core.transport.sentJson(0);
      expect(sent['v'], 1);
      expect(sent['cmd'], 'hello');
      expect(sent['args'], {'protocol': 1, 'client': '1.2.3'});

      expect(core.client.isHandshaken, isTrue);
      expect(core.client.hello, same(hello));
      expect(hello.coreVersion, '0.1.0');
      expect(hello.root, r'C:\SealTools');
      expect(hello.rev, 7);

      await core.dispose();
    });

    test('a bad_version error is a typed failure with both versions', () async {
      final core = _Core();
      core.transport.onWrite = (line) {
        final message = core.transport.sentJson(core.transport.sent.length - 1);
        core.transport.emitJson(
          errorResponse(
            message['id'] as String,
            'bad_version',
            message: 'protocol 2 is not 1',
            detail: {'expected': 2},
          ),
        );
      };

      await expectLater(
        core.client.handshake(),
        throwsA(
          isA<HandshakeException>()
              .having(
                (error) => error.failure,
                'failure',
                HandshakeFailure.badVersion,
              )
              .having((error) => error.shellProtocol, 'shellProtocol', 1)
              .having((error) => error.coreExpected, 'coreExpected', 2)
              .having(
                (error) => error.coreMessage,
                'coreMessage',
                'protocol 2 is not 1',
              ),
        ),
      );
      expect(core.client.isHandshaken, isFalse);

      await core.dispose();
    });

    test('not_ready at handshake is typed too', () async {
      final core = _Core();
      core.transport.onWrite = (line) {
        final message = core.transport.sentJson(core.transport.sent.length - 1);
        core.transport.emitJson(
          errorResponse(
            message['id'] as String,
            'not_ready',
            message: 'hello must be the first request',
          ),
        );
      };

      await expectLater(
        core.client.handshake(),
        throwsA(
          isA<HandshakeException>().having(
            (error) => error.failure,
            'failure',
            HandshakeFailure.notReady,
          ),
        ),
      );

      await core.dispose();
    });

    test('a result with another protocol is a mismatch', () async {
      final core = _Core();
      await core.connect(protocol: 1); // sanity: the exact-match path works
      expect(core.client.hello!.protocol, 1);
      await core.dispose();

      final mismatch = _Core();
      mismatch.transport.onWrite = (line) {
        final message = mismatch.transport.sentJson(
          mismatch.transport.sent.length - 1,
        );
        mismatch.transport.emitJson(
          okResponse(message['id'] as String, helloResult(protocol: 2)),
        );
      };
      await expectLater(
        mismatch.client.handshake(),
        throwsA(
          isA<HandshakeException>()
              .having(
                (error) => error.failure,
                'failure',
                HandshakeFailure.protocolMismatch,
              )
              .having((error) => error.coreReported, 'coreReported', 2),
        ),
      );
      await mismatch.dispose();
    });

    test('a second handshake is refused', () async {
      final core = _Core();
      await core.connect();

      await expectLater(
        core.client.handshake(),
        throwsA(isA<ProtocolStateException>()),
      );

      await core.dispose();
    });

    test('commands before the handshake are refused by the client', () async {
      final core = _Core();

      await expectLater(
        core.client.request('config.reload'),
        throwsA(isA<ProtocolStateException>()),
      );
      await expectLater(
        core.client.toolStart(ToolId.spammer),
        throwsA(isA<ProtocolStateException>()),
      );

      await core.dispose();
    });
  });

  group('request correlation', () {
    test(
      'responses resolve by id even when they arrive out of order',
      () async {
        final core = _Core();
        await core.connect();

        final first = core.client.request('config.reload');
        final second = core.client.request('tool.stop');
        final firstId = core.idAt(1);
        final secondId = core.idAt(2);
        expect(firstId, isNot(secondId));

        // Answer the SECOND request first.
        core.transport.emitJson(okResponse(secondId, {}));
        final secondResponse = await second;
        expect(secondResponse.id, secondId);

        core.transport.emitJson(
          okResponse(firstId, {
            'rev': 9,
            'config': {
              'window': {'title': 'TW'},
            },
          }),
        );
        final firstResponse = await first;
        expect(firstResponse, isA<OkResponse>());
        expect((firstResponse as OkResponse).result['rev'], 9);

        await core.dispose();
      },
    );

    test('a response for an unknown id is logged, not fatal', () async {
      final core = _Core();
      await core.connect();
      final log = core.client.logs.firstWhere(
        (line) => line.contains('unknown request id "nope"'),
      );

      core.transport.emitJson(okResponse('nope', {}));

      await expectLater(log, completes);
      await core.dispose();
    });

    test('an event is delivered while a request is pending', () async {
      final core = _Core();
      await core.connect();

      final eventFuture = core.client.events.first;
      final pending = core.client.request('config.reload');
      expect(core.client.events.isBroadcast, isTrue);

      core.transport.emitJson({
        'v': 1,
        'event': 'state',
        'data': {
          'tool': 'pet',
          'running': true,
          'phase': 'running',
          'message': 'row 2: looking',
          'countdown_s': null,
          'rows': <Object?>[],
          'error': null,
        },
      });
      final event = await eventFuture;
      expect(event, isA<StateEvent>());
      expect((event as StateEvent).snapshot.tool, ToolId.pet);

      core.transport.emitJson(
        okResponse(core.idAt(1), {'rev': 9, 'config': <String, dynamic>{}}),
      );
      await expectLater(pending, completes);

      await core.dispose();
    });
  });

  group('failure modes', () {
    test('a request times out with a typed error', () async {
      final core = _Core(timeout: const Duration(milliseconds: 20));
      await core.connect();

      final request = core.client.request('config.reload');
      final id = core.idAt(1);

      await expectLater(
        request,
        throwsA(
          isA<ProtocolTimeoutException>()
              .having((error) => error.cmd, 'cmd', 'config.reload')
              .having((error) => error.id, 'id', id),
        ),
      );

      // The late reply is logged and dropped, not matched to anything.
      final log = core.client.logs.firstWhere(
        (line) => line.contains('unknown request id "$id"'),
      );
      core.transport.emitJson(okResponse(id, {}));
      await expectLater(log, completes);

      await core.dispose();
    });

    test('closing the transport fails pending and future requests', () async {
      final core = _Core();
      await core.connect();
      final pending = core.client.request('config.reload');

      core.transport.closeIncoming();

      await expectLater(pending, throwsA(isA<ProtocolClosedException>()));
      await expectLater(
        core.client.request('config.reload'),
        throwsA(isA<ProtocolClosedException>()),
      );
      expect(core.client.isClosed, isTrue);

      await core.dispose();
    });

    test('stderr lines are buffered for the crash view and logged', () async {
      final core = _Core();
      await core.connect();
      final log = core.client.logs.first;

      core.transport.emitDiagnostic('seal-core: cannot load config: boom');

      await expectLater(log, completes);
      expect(core.client.stderrTail, ['seal-core: cannot load config: boom']);

      await core.dispose();
    });
  });

  group('closing', () {
    test('close sends shutdown, closes stdin and closes the streams', () async {
      final core = _Core();
      await core.connect();
      var eventsDone = false;
      core.client.events.listen(null, onDone: () => eventsDone = true);
      core.transport.onWrite = (line) {
        final message = core.transport.sentJson(core.transport.sent.length - 1);
        if (message['cmd'] == 'shutdown') {
          core.transport.emitJson(okResponse(message['id'] as String, {}));
        }
      };

      await core.client.close();
      await pumpEventQueue();

      expect(core.transport.lastCmd(), 'shutdown');
      expect(core.transport.inputClosed, isTrue);
      expect(core.client.isClosed, isTrue);
      expect(eventsDone, isTrue);

      // Second close is a no-op; nothing throws.
      await core.client.close();
      await core.dispose();
    });

    test('terminate kills the transport and marks the client closed', () async {
      final core = _Core();
      await core.connect();

      await core.client.terminate();

      expect(core.transport.terminated, isTrue);
      expect(core.client.isClosed, isTrue);

      await core.dispose();
    });
  });

  group('typed command helpers', () {
    test('decode their results and surface protocol errors', () async {
      final core = _Core();
      await core.connect();
      var failStart = false;
      core.transport.onWrite = (line) {
        final message = core.transport.sentJson(core.transport.sent.length - 1);
        final id = message['id'] as String;
        switch (message['cmd']) {
          case 'config.validate':
            core.transport.emitJson(
              okResponse(id, {'errors': <String>[], 'warnings': <String>[]}),
            );
          case 'config.apply':
            core.transport.emitJson(
              okResponse(id, {
                'rev': 8,
                'sections': {
                  'tuner': {'guard_px': 9},
                },
              }),
            );
          case 'config.reload':
            core.transport.emitJson(
              okResponse(id, {'rev': 9, 'config': <String, dynamic>{}}),
            );
          case 'tool.start':
            if (failStart) {
              core.transport.emitJson(
                errorResponse(
                  id,
                  'tool_busy',
                  message: 'another tool is running',
                  detail: {'tool': 'pet'},
                ),
              );
            } else {
              core.transport.emitJson(okResponse(id, {}));
            }
          case 'tool.stop':
            core.transport.emitJson(okResponse(id, {}));
          case 'ui.window':
            core.transport.emitJson(okResponse(id, {}));
          case 'shutdown':
            core.transport.emitJson(okResponse(id, {}));
        }
      };

      expect(
        (await core.client.configValidate({'quest': <String, dynamic>{}}))
            .isClean,
        isTrue,
      );

      final applied = await core.client.configApply(
        rev: 7,
        sections: {
          'tuner': {'guard_px': 9},
        },
      );
      expect(applied.rev, 8);
      expect(applied.sections['tuner'], {'guard_px': 9});

      expect((await core.client.configReload()).rev, 9);
      await core.client.toolStart(ToolId.spammer);
      await core.client.toolStop();
      await core.client.uiWindow(
        const UiWindowArgs(
          left: 100,
          top: 100,
          width: 900,
          height: 700,
          pinned: false,
        ),
      );

      failStart = true;
      await expectLater(
        core.client.toolStart(ToolId.gem),
        throwsA(
          isA<CoreCommandException>()
              .having((error) => error.error.code, 'code', ErrorCode.toolBusy)
              .having((error) => error.error.busyTool, 'busyTool', 'pet'),
        ),
      );

      await core.dispose();
    });

    test('unknown events are still delivered as UnknownEvent', () async {
      final core = _Core();
      await core.connect();
      final eventFuture = core.client.events.first;

      core.transport.emitJson({
        'v': 1,
        'event': 'future.thing',
        'data': {'x': 1},
      });

      final event = await eventFuture;
      expect(event, isA<UnknownEvent>());
      expect(event.name, 'future.thing');

      await core.dispose();
    });
  });
}

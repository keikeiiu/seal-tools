/// NDJSON framing: one compact line out, one object per line in, typed failures at the boundary.
library;

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/protocol/protocol.dart';

void main() {
  group('encode', () {
    test('emits compact JSON and exactly one trailing newline', () {
      final line = encodeMessage({
        'v': 1,
        'id': '7',
        'ok': true,
        'result': {
          'display': {'scale': 1.5},
        },
      });

      expect(line, contains('"v":1'));
      expect(line, contains('"scale":1.5'));
      expect(line, isNot(contains(': ')));
      expect(line.endsWith('\n'), isTrue);
      expect(line.split('\n'), hasLength(2));
    });

    test('encodes a request as the protocol envelope', () {
      final request = Request(id: '1', cmd: 'hello', args: {'protocol': 1});
      final line = encodeRequest(request);

      expect(jsonDecode(line), request.toJson());
      expect(line.endsWith('\n'), isTrue);
    });
  });

  group('decode', () {
    test('a success response decodes to OkResponse', () {
      final message = decodeLine(
        '{"v":1,"id":"7","ok":true,"result":{"rev":8}}',
      );

      expect(message, isA<IncomingResponse>());
      final response = (message as IncomingResponse).response;
      expect(response, isA<OkResponse>());
      expect(response.id, '7');
      expect((response as OkResponse).result['rev'], 8);
    });

    test('an error response decodes to ErrorResponse with its code', () {
      final message = decodeLine(
        '{"v":1,"id":"9","ok":false,"error":{"code":"tool_busy",'
        '"message":"another tool is running","detail":{"tool":"pet"}}}',
      );
      final response = (message as IncomingResponse).response;

      expect(response, isA<ErrorResponse>());
      final error = (response as ErrorResponse).error;
      expect(error.code, ErrorCode.toolBusy);
      expect(error.busyTool, 'pet');
    });

    test('an event decodes to its typed family, not a response', () {
      final message = decodeLine(
        '{"v":1,"event":"log","data":{"level":"warn","line":"retried"}}',
      );

      expect(message, isA<IncomingEvent>());
      final event = (message as IncomingEvent).event;
      expect(event, isA<LogEvent>());
      expect((event as LogEvent).log.level, LogLevel.warn);
      expect(event.log.line, 'retried');
    });

    test('an unknown event name is UnknownEvent, not a failure', () {
      final event = (decodeLine(
        '{"v":1,"event":"future.thing","data":{"x":1}}',
      ) as IncomingEvent).event;

      expect(event, isA<UnknownEvent>());
      expect(event.name, 'future.thing');
      expect(event.data['x'], 1);
    });

    test('tolerates one trailing carriage return', () {
      final message = decodeLine('{"v":1,"id":"2","ok":true,"result":{}}\r');

      expect((message as IncomingResponse).response, isA<OkResponse>());
    });

    test('blank lines are a typed error, not a silent skip', () {
      for (final blank in ['', '   ', '\r', '\t']) {
        expect(
          () => decodeLine(blank),
          throwsA(isA<ProtocolDecodeException>()),
          reason: 'blank input "${blank.replaceAll('\r', r'\r')}"',
        );
      }
    });

    test('malformed JSON and non-objects are typed errors', () {
      expect(
        () => decodeLine('{"v":1,"id":'),
        throwsA(isA<ProtocolDecodeException>()),
      );
      expect(() => decodeLine('[]'), throwsA(isA<ProtocolDecodeException>()));
      expect(
        () => decodeLine('"just a string"'),
        throwsA(isA<ProtocolDecodeException>()),
      );
    });

    test(
      'a version mismatch and an envelope that is neither kind are errors',
      () {
        expect(
          () => decodeLine('{"v":2,"id":"1","ok":true,"result":{}}'),
          throwsA(
            isA<ProtocolDecodeException>().having(
              (error) => error.path,
              'path',
              'v',
            ),
          ),
        );
        expect(
          () => decodeLine('{"v":1,"id":"1"}'),
          throwsA(isA<ProtocolDecodeException>()),
        );
        expect(
          () => decodeLine('{"v":1,"ok":"yes"}'),
          throwsA(isA<ProtocolDecodeException>()),
        );
      },
    );

    test('a malformed known event payload is an error', () {
      expect(
        () => decodeLine('{"v":1,"event":"state","data":{"tool":"pet"}}'),
        throwsA(isA<ProtocolDecodeException>()),
      );
    });
  });

  group('SHELL-F1: an undecodable event is not fatal', () {
    test(
      'a malformed known EVENT becomes a marker and the stream continues',
      () async {
        final messages = await decodeLines(
          Stream.fromIterable([
            '{"v":1,"event":"state","data":{"tool":"pet"}}',
            '{"v":1,"event":"log","data":{"level":"info","line":"after"}}',
          ]),
        ).toList();

        expect(messages, hasLength(2));
        final marker = messages[0];
        expect(marker, isA<IncomingMalformedEvent>());
        final malformed = marker as IncomingMalformedEvent;
        expect(malformed.eventName, 'state');
        expect(malformed.error, isA<ProtocolDecodeException>());

        final event = (messages[1] as IncomingEvent).event;
        expect(event, isA<LogEvent>());
        expect((event as LogEvent).log.line, 'after');
      },
    );

    test(
      'a malformed RESPONSE is still fatal: its request could never settle',
      () async {
        await expectLater(
          decodeLines(
            Stream.fromIterable([
              '{"v":1,"id":"1","ok":true,"result":"not-a-map"}',
            ]),
          ).toList(),
          throwsA(isA<ProtocolDecodeException>()),
        );
      },
    );

    test('garbage that cannot be attributed to an event stays fatal', () async {
      await expectLater(
        decodeLines(Stream.fromIterable(['not json'])).toList(),
        throwsA(isA<ProtocolDecodeException>()),
      );
    });
  });

  group('byte stream decoding', () {
    test(
      'splits lines and keeps multi-byte UTF-8 intact across chunks',
      () async {
        final first = encodeMessage({
          'v': 1,
          'event': 'log',
          'data': {'level': 'info', 'line': '藍天 · 3/min'},
        });
        final second = encodeMessage({
          'v': 1,
          'event': 'log',
          'data': {'level': 'info', 'line': 'done'},
        });
        final bytes = utf8.encode(first + second);

        // Split INSIDE the multi-byte characters so the utf8 decoder has to carry state.
        final chunks = [
          bytes.sublist(0, 30),
          bytes.sublist(30, 31),
          bytes.sublist(31),
        ];

        final messages = await decodeByteStream(Stream.fromIterable(chunks))
            .toList();

        expect(messages, hasLength(2));
        final firstEvent = (messages[0] as IncomingEvent).event;
        expect(firstEvent, isA<LogEvent>());
        expect((firstEvent as LogEvent).log.line, '藍天 · 3/min');
        final secondEvent = (messages[1] as IncomingEvent).event;
        expect((secondEvent as LogEvent).log.line, 'done');
      },
    );

    test('a malformed line ends the stream with a typed error', () async {
      final stream = decodeLines(
        Stream.fromIterable([
          '{"v":1,"event":"log","data":{"level":"info","line":"a"}}',
          'not json',
        ]),
      );

      await expectLater(
        stream.toList(),
        throwsA(isA<ProtocolDecodeException>()),
      );
    });
  });
}

/// The fixture guard: every golden fixture decodes into its typed model, key shape included.
///
/// The fixture set is the cross-language contract ([PROTOCOL.md] §10). Three tests carry the weight:
/// coverage (a fixture with no decoder fails; a decoder with no fixture fails), shape (a decoded
/// model's keys equal the fixture's, recursively — the drift guard), and request round-trip.
///
/// [PROTOCOL.md]: ../../../v3/docs/spec/PROTOCOL.md
library;

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/protocol/protocol.dart';

import '../support/drift.dart';
import '../support/fixture_decoders.dart';
import '../support/fixture_set.dart';

void main() {
  final fixtures = loadFixtures();

  test('the fixture set is the protocol surface', () {
    // No hard-coded total: the set grows with the protocol, and the coverage test below is what
    // keeps the decoder map in step. These two assertions are about the SET's integrity.
    expect(fixtures, isNotEmpty);
    expect(
      fixtures.map((fixture) => fixture.caseName).toSet(),
      hasLength(fixtures.length),
      reason: 'fixture case names must be unique',
    );
    expect(fixtures.map((fixture) => fixture.kind).toSet(), {
      'request',
      'response',
      'event',
    });
  });

  test('every fixture case has a decoder, and every decoder a fixture', () {
    for (final kind in ['request', 'response', 'event']) {
      final expected = fixtures
          .where((fixture) => fixture.kind == kind)
          .map((fixture) => fixture.caseName)
          .toSet();
      final actual = decodersFor(kind).keys.toSet();

      final missing = expected.difference(actual);
      final stale = actual.difference(expected);
      expect(
        missing,
        isEmpty,
        reason:
            '$kind fixtures with no decoder (add them to fixture_decoders.dart): '
            '${missing.join(', ')}',
      );
      expect(
        stale,
        isEmpty,
        reason:
            '$kind decoders with no fixture (remove them or add the fixture): '
            '${stale.join(', ')}',
      );
    }
  });

  test('every fixture decodes into its typed model and keeps its shape', () {
    for (final fixture in fixtures) {
      final decoded = decodeFixture(
        fixture.caseName,
        fixture.kind,
        fixture.message,
      );
      expect(
        decoded.caseName,
        fixture.caseName,
        reason: 'decoder returned the wrong case for ${fixture.fileName}',
      );

      switch (fixture.kind) {
        case 'request':
          final request = decoded as DecodedRequestFixture;
          expectSameShape(
            request.request.toJson(),
            fixture.message,
            '${fixture.caseName}.request',
          );
          final typedArgs = request.typedArgs;
          if (typedArgs != null) {
            expectSameShape(
              typedArgs.toJson(),
              fixture.message['args'],
              '${fixture.caseName}.args',
            );
          }
        case 'response':
          if (fixture.caseName.startsWith('response.ok.')) {
            final ok = decoded as DecodedOkFixture;
            expectSameShape(
              ok.result.toJson(),
              fixture.message['result'],
              '${fixture.caseName}.result',
            );
          } else {
            final error = decoded as DecodedErrorFixture;
            expectSameShape(
              error.error.toJson(),
              fixture.message['error'],
              '${fixture.caseName}.error',
            );
            expect(
              error.error.rawCode,
              (fixture.message['error'] as Map<String, dynamic>)['code'],
            );
          }
        case 'event':
          final event = decoded as DecodedEventFixture;
          expectSameShape(
            event.event.data,
            fixture.message['data'],
            '${fixture.caseName}.data',
          );
        default:
          fail('unexpected fixture kind ${fixture.kind}');
      }
    }
  });

  test('every request fixture round-trips through its encoded line', () {
    for (final fixture in fixtures.where(
      (fixture) => fixture.kind == 'request',
    )) {
      final decoded = decodeFixture(
        fixture.caseName,
        fixture.kind,
        fixture.message,
      ) as DecodedRequestFixture;
      final line = encodeRequest(decoded.request);
      expect(line.endsWith('\n'), isTrue);
      expect(line.split('\n'), hasLength(2));
      final encoded = jsonDecode(line) as Map<String, dynamic>;
      expect(
        deepEquals(encoded, fixture.message),
        isTrue,
        reason: '${fixture.fileName} did not survive encode/decode',
      );
    }
  });

  test('the state event pins the row shape and every row field', () {
    final state = decodeFixture(
      'event.state',
      'event',
      fixtures
          .firstWhere((fixture) => fixture.caseName == 'event.state')
          .message,
    ) as DecodedEventFixture;

    expect(state.event, isA<StateEvent>());
    final snapshot = (state.event as StateEvent).snapshot;
    expect(snapshot.tool, ToolId.pet);
    expect(snapshot.phase, ToolPhase.running);
    expect(snapshot.rows, hasLength(2));
    expect(snapshot.rows.first.name, 'row 1');
    expect(snapshot.rows.first.status, 'boarding');
    expect(snapshot.rows.first.detail, isNotNull);
    expect(snapshot.rows.first.nextActionInSeconds, isNotNull);
    expect(snapshot.rows.last.detail, isNull);
    expect(snapshot.rows.last.nextActionInSeconds, isNull);

    final empty = decodeFixture(
      'event.state.empty',
      'event',
      fixtures
          .firstWhere((fixture) => fixture.caseName == 'event.state.empty')
          .message,
    ) as DecodedEventFixture;
    expect((empty.event as StateEvent).snapshot.rows, isEmpty);
  });

  test('the response payloads get the types the shell uses', () {
    final byCase = {
      for (final fixture in fixtures)
        fixture.caseName: decodeFixture(
          fixture.caseName,
          fixture.kind,
          fixture.message,
        ),
    };

    expect(
      (byCase['response.ok.hello']! as DecodedOkFixture).result,
      isA<HelloResult>(),
    );
    expect(
      (byCase['response.ok.config_reload']! as DecodedOkFixture).result,
      isA<ConfigReloadResult>(),
    );
    expect(
      (byCase['response.ok.config_apply.mixed']! as DecodedOkFixture).result,
      isA<ConfigApplyResult>(),
    );
    expect(
      (byCase['response.ok.capture']! as DecodedOkFixture).result,
      isA<CaptureResult>(),
    );
    expect(
      (byCase['response.ok.ocr_scan']! as DecodedOkFixture).result,
      isA<OcrScanResult>(),
    );
    expect(
      (byCase['response.ok.empty']! as DecodedOkFixture).result,
      isA<EmptyResult>(),
    );
    expect(
      (byCase['event.log']! as DecodedEventFixture).event,
      isA<LogEvent>(),
    );
    expect(
      (byCase['event.tool.finished']! as DecodedEventFixture).event,
      isA<ToolFinishedEvent>(),
    );
    expect(
      (byCase['event.core.stopping']! as DecodedEventFixture).event,
      isA<CoreStoppingEvent>(),
    );
    expect(
      (byCase['event.config.changed']! as DecodedEventFixture).event,
      isA<ConfigChangedEvent>(),
    );
  });

  test('every error code has a fixture, and every fixture code is known', () {
    final errorCodes = <String>{};
    for (final fixture in fixtures) {
      final decoded = decodeFixture(
        fixture.caseName,
        fixture.kind,
        fixture.message,
      );
      if (decoded is DecodedErrorFixture) {
        errorCodes.add(decoded.error.rawCode);
        expect(
          decoded.error.code,
          isNot(ErrorCode.unknown),
          reason: '${fixture.fileName} uses a code the Dart enum does not know',
        );
      }
    }
    // The set is the guard: a code added to PROTOCOL §6 without a fixture (or the other way round)
    // fails here, and the enum has to grow with it.
    for (final code in ErrorCode.values) {
      if (code == ErrorCode.unknown) {
        continue;
      }
      expect(
        errorCodes,
        contains(code.wire),
        reason: 'no fixture covers the error code "${code.wire}"',
      );
    }
  });
}

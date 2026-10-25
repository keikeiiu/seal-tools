/// The Calibrate tab: capture/frame/display, the seven-kind mark editor, save payloads, verbatim
/// refusals, stored marks and the artifact crop — all over the fake transport.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/protocol/protocol.dart';
import 'package:seal_shell/ui/tabs/calibrate_tab.dart';
import 'package:seal_shell/ui/tabs/calibration_widgets.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

/// A valid 1×1 transparent PNG — enough for `Image.memory` to decode in a widget test.
const String _tinyPng =
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=';

Map<String, dynamic> _displayJson() => {
  'scale': 1.5,
  'monitor_dpi': 144,
  'screen': {'w': 2560, 'h': 1440},
  'frame': {'x': 0, 'y': 0, 'w': 2865, 'h': 1789},
  'physical_client': {'x': 10, 'y': 20, 'w': 2865, 'h': 1789},
  'logical_client': {'x': 6, 'y': 13, 'w': 1910, 'h': 1193},
};

Map<String, dynamic> _captureAnswer({
  String frameId = 'f1',
  int width = 4,
  int height = 3,
}) => {
  'frame': {
    'format': 'png',
    'width': width,
    'height': height,
    'origin': {'x': 10, 'y': 20},
    'data': _tinyPng,
    'frame_id': frameId,
  },
  'display': _displayJson(),
};

/// One answer for the fake core: `{'result': …}` or `{'error': {code, message}}`.
typedef _Answer = Map<String, dynamic> Function(Map<String, dynamic> request);

Future<AppHarness> _pumpCalibrate(
  WidgetTester tester, {
  required _Answer answer,
}) async {
  tester.view.physicalSize = const ui.Size(1800, 2600);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  final harness = AppHarness();
  addTearDown(harness.model.dispose);
  await tester.pumpWidget(
    const MaterialApp(
      home: Scaffold(body: Center(child: CircularProgressIndicator())),
    ),
  );
  final connecting = harness.connect(
    config: {
      'window': {'title': 'TW_LIVE'},
    },
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  harness.onWrite = (line, fake) {
    final message = fake.sentJson(fake.sent.length - 1);
    final id = message['id'] as String;
    final reply = answer(message);
    if (reply.containsKey('error')) {
      final error = reply['error'] as Map<String, dynamic>;
      fake.emitJson(
        errorResponse(
          id,
          error['code'] as String,
          message: error['message'] as String,
        ),
      );
    } else {
      fake.emitJson(okResponse(id, reply['result'] as Map<String, dynamic>));
    }
  };
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(body: CalibrateTab(model: harness.model)),
    ),
  );
  // The tab fetches all seven kinds on mount; let the sequential loop finish.
  for (var i = 0; i < 8; i++) {
    await tester.pump();
  }
  return harness;
}

/// A fetch answer for one kind: not calibrated unless [mark] is given.
Map<String, dynamic> _fetchAnswer(
  String kind, {
  List<int>? mark,
  Map<String, dynamic>? artifact,
}) => {'kind': kind, 'mark': mark, 'artifact': artifact, 'rev': 7};

Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

String _resultText(WidgetTester tester, String key) => tester
    .widget<Text>(
      find.descendant(
        of: find.byKey(ValueKey(key)),
        matching: find.byType(Text),
      ),
    )
    .data!;

Finder _field(String key) => find.byKey(ValueKey(key));

Map<String, dynamic> _saveArgs(AppHarness harness) =>
    (harness.lastSentWithCmd('calibration.save')!['args']
        as Map<String, dynamic>);

void main() {
  testWidgets('capture renders the frame picture and the display summary', (
    tester,
  ) async {
    await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer()},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);

    expect(
      _resultText(tester, 'calibrate-capture-result'),
      contains('captured frame f1'),
    );
    expect(find.byKey(const ValueKey('calibrate-frame')), findsOneWidget);
    expect(find.textContaining('Frame 4×3 px'), findsOneWidget);
    expect(
      _resultText(tester, 'calibrate-display'),
      contains('scale 1.5 · monitor DPI 144'),
    );
  });

  testWidgets('a capture refusal renders the core line verbatim', (
    tester,
  ) async {
    await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {
          'error': {
            'code': 'game_not_foreground',
            'message': 'the game is not the foreground window',
          },
        },
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);

    expect(
      _resultText(tester, 'calibrate-capture-result'),
      contains('the game is not the foreground window'),
    );
    expect(find.byKey(const ValueKey('calibrate-frame')), findsNothing);
  });

  testWidgets(
    'a rect save sends frame_id/kind/region/display and shows pitch',
    (tester) async {
      final harness = await _pumpCalibrate(
        tester,
        answer: (request) => switch (request['cmd']) {
          'capture' => {'result': _captureAnswer(frameId: 'f9')},
          'calibration.fetch' => {
            'result': _fetchAnswer(
              (request['args'] as Map<String, dynamic>)['kind'] as String,
            ),
          },
          'calibration.save' => {
            'result': {
              'rev': 4,
              'kind': 'bag_grid',
              'mark': [10, 20, 240, 240],
              'derived': {
                'pitch': {'w': 30.0, 'h': 30.0},
              },
              'artifact': null,
            },
          },
          _ => {'result': <String, dynamic>{}},
        },
      );

      await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
      await _flush(tester);
      await tester.enterText(_field('calibrate-x'), '10');
      await tester.enterText(_field('calibrate-y'), '20');
      await tester.enterText(_field('calibrate-w'), '240');
      await tester.enterText(_field('calibrate-h'), '240');
      await tester.tap(find.byKey(const ValueKey('calibrate-save')));
      await _flush(tester);

      final args = _saveArgs(harness);
      expect(args['frame_id'], 'f9');
      expect(args['kind'], 'bag_grid');
      expect(args['region'], {'x': 10, 'y': 20, 'w': 240, 'h': 240});
      expect(args.containsKey('point'), isFalse);
      final display = args['display'] as Map<String, dynamic>;
      expect(display['dpi_scale'], 1.5);
      expect(display['monitor_dpi'], 144);
      expect(display['screen'], {'w': 2560, 'h': 1440});

      expect(
        _resultText(tester, 'calibrate-save-result'),
        contains('saved bag_grid at rev 4'),
      );
      expect(
        _resultText(tester, 'calibrate-pitch'),
        contains('derived pitch 30.00 × 30.00'),
      );
    },
  );

  testWidgets('a point kind sends point, not region', (tester) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer()},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        'calibration.save' => {
          'result': {
            'rev': 5,
            'kind': 'focus_point',
            'mark': [7, 8],
            'derived': <String, dynamic>{},
            'artifact': null,
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    await tester.tap(find.byKey(const ValueKey('calibrate-kind')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('focus_point').last);
    await tester.pumpAndSettle();
    await tester.enterText(_field('calibrate-x'), '7');
    await tester.enterText(_field('calibrate-y'), '8');
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);

    final args = _saveArgs(harness);
    expect(args['kind'], 'focus_point');
    expect(args['point'], {'x': 7, 'y': 8});
    expect(args.containsKey('region'), isFalse);
  });

  testWidgets('a bag-slot disagreement renders verbatim and keeps the draft', (
    tester,
  ) async {
    const line = "the bag slot does not agree with the bag grid's pitch";
    await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer()},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        'calibration.save' => {
          'error': {'code': 'bad_request', 'message': line},
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    await tester.tap(find.byKey(const ValueKey('calibrate-kind')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('bag_slot').last);
    await tester.pumpAndSettle();
    await tester.enterText(_field('calibrate-x'), '11');
    await tester.enterText(_field('calibrate-y'), '22');
    await tester.enterText(_field('calibrate-w'), '33');
    await tester.enterText(_field('calibrate-h'), '44');
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);

    expect(_resultText(tester, 'calibrate-save-result'), contains(line));
    // The draft stays: the player adjusts instead of retyping from zero.
    expect(
      tester.widget<TextField>(_field('calibrate-w')).controller?.text,
      '33',
    );
    expect(
      tester.widget<TextField>(_field('calibrate-h')).controller?.text,
      '44',
    );
  });

  testWidgets('unknown frame_id renders verbatim and never auto-recaptures', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer(frameId: 'evicted')},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        'calibration.save' => {
          'error': {
            'code': 'bad_request',
            'message': 'unknown frame_id "evicted" — capture the window again',
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);

    expect(
      _resultText(tester, 'calibrate-save-result'),
      contains('unknown frame_id "evicted" — capture the window again'),
    );
    final captures = harness.sentMessages
        .where((message) => message['cmd'] == 'capture')
        .length;
    expect(captures, 1, reason: 'the player must press Capture again');
  });

  testWidgets('a null stored mark shows Not calibrated yet', (tester) async {
    await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    expect(
      find.byKey(const ValueKey('calibrate-not-calibrated')),
      findsOneWidget,
    );
    expect(find.text('Not calibrated yet'), findsOneWidget);
  });

  testWidgets('a click sets the point fields; a letterbox click is ignored', (
    tester,
  ) async {
    await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer(width: 100, height: 75)},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    // Point kinds are the click surface.
    await tester.tap(find.byKey(const ValueKey('calibrate-kind')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('focus_point').last);
    await tester.pumpAndSettle();

    final frame = Frame.fromJson(
      _captureAnswer(width: 100, height: 75)['frame'] as Map<String, dynamic>,
      'frame',
    );
    final area = tester.getRect(
      find.byKey(const ValueKey('calibrate-picture-area')),
    );
    final layout = frameContainLayout(
      box: area.size,
      frameWidth: frame.width,
      frameHeight: frame.height,
    );
    // Letterbox is left of the picture for a 4:3 frame in a wider box.
    await tester.tapAt(area.topLeft + const Offset(5, 20));
    await tester.pump();
    expect(
      tester.widget<TextField>(_field('calibrate-x')).controller?.text,
      '0',
    );
    expect(
      tester.widget<TextField>(_field('calibrate-y')).controller?.text,
      '0',
    );

    await tester.tapAt(area.topLeft + layout.picture.center);
    await tester.pump();
    final expected = framePointToMark(
      local: layout.picture.size.center(Offset.zero),
      box: layout.picture.size,
      frame: frame,
      clientOrigin: const Offset(10, 20),
    )!;
    expect(
      tester.widget<TextField>(_field('calibrate-x')).controller?.text,
      '${expected.x}',
    );
    expect(
      tester.widget<TextField>(_field('calibrate-y')).controller?.text,
      '${expected.y}',
    );
    // The mark is drawn back on the picture (the painter is present).
    expect(find.byType(CustomPaint), findsWidgets);
  });

  testWidgets('a drag sets the rect fields and the save carries it', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer(width: 100, height: 75)},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        'calibration.save' => {
          'result': {
            'rev': 9,
            'kind': 'bag_grid',
            'mark': [0, 0, 2, 2],
            'derived': {
              'pitch': {'w': 0.25, 'h': 0.25},
            },
            'artifact': null,
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    final frame = Frame.fromJson(
      _captureAnswer(width: 100, height: 75)['frame'] as Map<String, dynamic>,
      'frame',
    );
    final area = tester.getRect(
      find.byKey(const ValueKey('calibrate-picture-area')),
    );
    final layout = frameContainLayout(
      box: area.size,
      frameWidth: frame.width,
      frameHeight: frame.height,
    );
    const delta = Offset(120, 80);
    final start = area.topLeft + layout.picture.center;
    await tester.dragFrom(start, delta);
    await _flush(tester);

    final expected = frameDragToMark(
      startLocal: layout.picture.size.center(Offset.zero),
      endLocal: layout.picture.size.center(Offset.zero) + delta,
      box: layout.picture.size,
      frame: frame,
      clientOrigin: const Offset(10, 20),
    )!;
    expect(
      tester.widget<TextField>(_field('calibrate-x')).controller?.text,
      '${expected.x}',
    );
    expect(
      tester.widget<TextField>(_field('calibrate-w')).controller?.text,
      '${expected.w}',
    );

    // The typed fields are authoritative: Save sends what they contain.
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);
    expect(_saveArgs(harness)['region'], {
      'x': expected.x,
      'y': expected.y,
      'w': expected.w,
      'h': expected.h,
    });
  });

  testWidgets('a tiny drag is ignored (no accidental 1x1 mark)', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer(width: 100, height: 75)},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    final area = tester.getRect(
      find.byKey(const ValueKey('calibrate-picture-area')),
    );
    await tester.dragFrom(area.center, const Offset(1, 1));
    await _flush(tester);

    expect(
      tester.widget<TextField>(_field('calibrate-x')).controller?.text,
      '0',
    );
    expect(
      tester.widget<TextField>(_field('calibrate-w')).controller?.text,
      '0',
    );
    expect(harness.lastSentWithCmd('calibration.save'), isNull);
  });

  testWidgets('typing overrides a gesture value before Save reads it', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer(width: 100, height: 75)},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        'calibration.save' => {
          'result': {
            'rev': 10,
            'kind': 'bag_grid',
            'mark': [0, 0, 2, 2],
            'derived': {
              'pitch': {'w': 0.25, 'h': 0.25},
            },
            'artifact': null,
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    final area = tester.getRect(
      find.byKey(const ValueKey('calibrate-picture-area')),
    );
    await tester.dragFrom(area.center, const Offset(120, 80));
    await _flush(tester);
    await tester.enterText(_field('calibrate-w'), '55');
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);

    expect((_saveArgs(harness)['region'] as Map<String, dynamic>)['w'], 55);
  });
  testWidgets('a stored artifact renders as the crop picture', (tester) async {
    await _pumpCalibrate(
      tester,
      answer: (request) {
        if (request['cmd'] != 'calibration.fetch') {
          return {'result': <String, dynamic>{}};
        }
        final kind =
            (request['args'] as Map<String, dynamic>)['kind'] as String;
        return {
          'result': kind == 'result_gem_area'
              ? _fetchAnswer(
                  kind,
                  mark: [1, 2, 30, 30],
                  artifact: {
                    'file': 'calib_gem_result.png',
                    'png_base64': _tinyPng,
                  },
                )
              : _fetchAnswer(kind),
        };
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-kind')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('result_gem_area').last);
    await tester.pumpAndSettle();

    expect(
      tester.widget<Text>(find.byKey(const ValueKey('calibrate-stored'))).data,
      contains('Stored mark [1, 2, 30, 30] (rev 7)'),
    );
    expect(find.byKey(const ValueKey('calibrate-crop')), findsOneWidget);
  });
}

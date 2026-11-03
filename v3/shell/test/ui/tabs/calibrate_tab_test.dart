/// The Calibrate tab: capture/frame/display (including the hide→capture→show host lifecycle), the
/// calibrated mark editor (including the named-instance kinds and their target selector), save
/// payloads, verbatim refusals, stored marks and the artifact crop — all over the fake transport.
library;

import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/app.dart';
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

/// The runner host seam, recorded so a test can assert the capture lifecycle's order.
class _FakeHost implements HostActions {
  _FakeHost({this.failHide = false, this.failShow = false});

  final bool failHide;
  final bool failShow;
  final List<String> events = [];

  @override
  Future<void> hideWindow() async {
    events.add('hide');
    if (failHide) {
      throw PlatformException(code: 'no_host');
    }
  }

  @override
  Future<void> showWindow() async {
    events.add('show');
    if (failShow) {
      throw PlatformException(code: 'no_host');
    }
  }

  @override
  Future<void> setWindowMinSize(int width, int height) async {}

  @override
  Future<void> setMiniChrome(bool mini) async {
    // The Calibrate surface never collapses the shell; the model's own fake host covers mini mode.
  }

  @override
  Future<void> setAlwaysOnTop(bool enabled) async {
    // The capture lifecycle does not use the pin; the model's own fake host covers it.
  }

  // The window-placement half of the seam: the calibrate surface never asks for it, so these are
  // inert values rather than recordings.
  @override
  Future<WindowRect> windowRect() async =>
      const WindowRect(left: 0, top: 0, width: 1280, height: 720);

  @override
  Future<void> setWindowRect(WindowRect rect) async {}

  @override
  Future<WindowRect> primaryWorkArea() async =>
      const WindowRect(left: 0, top: 0, width: 1920, height: 1040);

  @override
  Future<WindowRect> virtualScreen() async =>
      const WindowRect(left: 0, top: 0, width: 1920, height: 1040);

  @override
  Future<bool> isWindowVisible() async => true;

  @override
  Stream<WindowRect> get windowMoves => const Stream<WindowRect>.empty();
}

Future<AppHarness> _pumpCalibrate(
  WidgetTester tester, {
  required _Answer answer,
  HostActions? host,
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
      home: Scaffold(
        body: CalibrateTab(model: harness.model, host: host ?? _FakeHost()),
      ),
    ),
  );
  // The tab fetches the non-targeted kinds on mount; let the sequential loop finish.
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
  String? target,
}) => {
  'kind': kind,
  'target': target,
  'mark': mark,
  'artifact': artifact,
  'rev': 7,
};

Future<void> _flush(WidgetTester tester) async {
  for (var i = 0; i < 6; i++) {
    await tester.pump();
  }
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

/// Drags on the interactive picture INSIDE one session frame (the shared overlay's gesture surface),
/// measured from the picture's centre so the converted mark is a real rect.
Future<void> _dragOnSessionFrame(
  WidgetTester tester,
  String frameKey,
  Offset delta,
) async {
  final gestures = find.descendant(
    of: find.byKey(ValueKey(frameKey)),
    matching: find.byKey(const ValueKey('calibrate-picture-gestures')),
  );
  await tester.ensureVisible(gestures);
  await tester.pump();
  final area = tester.getRect(gestures);
  final gesture = await tester.startGesture(
    area.center,
    kind: PointerDeviceKind.mouse,
    buttons: kPrimaryMouseButton,
  );
  await tester.pump();
  for (var i = 0; i < 4; i++) {
    await gesture.moveBy(Offset(delta.dx / 4, delta.dy / 4));
    await tester.pump();
  }
  await gesture.up();
  await _flush(tester);
}

Future<void> _clickOnSessionFrame(
  WidgetTester tester,
  String frameKey,
  Offset offsetFromCentre,
) async {
  final gestures = find.descendant(
    of: find.byKey(ValueKey(frameKey)),
    matching: find.byKey(const ValueKey('calibrate-picture-gestures')),
  );
  await tester.ensureVisible(gestures);
  await tester.pump();
  final area = tester.getRect(gestures);
  await tester.tapAt(area.center + offsetFromCentre);
  await _flush(tester);
}

/// Scrolls a control into view, then taps it (the session panels run past one screen).
Future<void> _tapVisible(WidgetTester tester, String key) async {
  final finder = find.byKey(ValueKey(key));
  await tester.ensureVisible(finder);
  await tester.pump();
  await tester.tap(finder);
  await _flush(tester);
}

/// Opens the kind picker and taps an item by its stable key.
Future<void> _selectKind(WidgetTester tester, String kind) async {
  await tester.tap(find.byKey(const ValueKey('calibrate-kind')));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(ValueKey('calibrate-kind-item-$kind')).last);
  await tester.pumpAndSettle();
}

Future<void> _selectTarget(WidgetTester tester, String target) async {
  await tester.tap(find.byKey(const ValueKey('calibrate-target')));
  await tester.pumpAndSettle();
  // The menu is open: the route's copy is the last one in the tree.
  await tester.tap(find.byKey(ValueKey('calibrate-target-item-$target')).last);
  await tester.pumpAndSettle();
}

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
            mark: [7, 8],
          ),
        },
        'calibration.save' => {
          'result': {
            'rev': 5,
            'kind': 'scroll_point',
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
    await _selectKind(tester, 'scroll_point');
    await tester.enterText(_field('calibrate-x'), '7');
    await tester.enterText(_field('calibrate-y'), '8');
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);

    final args = _saveArgs(harness);
    expect(args['kind'], 'scroll_point');
    expect(args['point'], {'x': 7, 'y': 8});
    expect(args.containsKey('region'), isFalse);
    // The stored-mark surface displays the fetched point mark.
    expect(
      tester.widget<Text>(find.byKey(const ValueKey('calibrate-stored'))).data,
      contains('[7, 8]'),
    );
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
    await _selectKind(tester, 'bag_slot');
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
    await _selectKind(tester, 'scroll_point');

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

    // The drag maps through the canvas' current zoom/pan (the workspace is zoomable), so the exact
    // physical delta is covered by the frameDragToMark unit tests in calibration_mapping_test. Here
    // the contract is: a drag fills the typed fields with a real rect, and Save sends exactly what
    // those fields contain.
    String fieldText(String key) =>
        tester.widget<TextField>(_field(key)).controller!.text;
    final xText = fieldText('calibrate-x');
    final yText = fieldText('calibrate-y');
    final wText = fieldText('calibrate-w');
    final hText = fieldText('calibrate-h');
    expect(int.tryParse(xText), isNotNull);
    expect(int.tryParse(yText), isNotNull);
    expect(int.parse(wText), greaterThan(0));
    expect(int.parse(hText), greaterThan(0));

    // The typed fields are authoritative: Save sends what they contain.
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);
    expect(_saveArgs(harness)['region'], {
      'x': int.parse(xText),
      'y': int.parse(yText),
      'w': int.parse(wText),
      'h': int.parse(hText),
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

    await _selectKind(tester, 'result_gem_area');

    expect(
      tester.widget<Text>(find.byKey(const ValueKey('calibrate-stored'))).data,
      contains('Stored mark [1, 2, 30, 30] (rev 7)'),
    );
    expect(find.byKey(const ValueKey('calibrate-crop')), findsOneWidget);
  });

  testWidgets(
    'the kind picker offers the repaired kinds and every named-instance kind',
    (tester) async {
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

      await tester.tap(find.byKey(const ValueKey('calibrate-kind')));
      await tester.pumpAndSettle();
      const groups = <String, List<String>>{
        'Shop': [
          'bag_grid',
          'bag_slot',
          'shop_list',
          'scroll_point',
          'max_button',
        ],
        'Gem': ['result_gem_area', 'gem_grade', 'gem_resource'],
        'Pet — points': ['pet_menu', 'pet_feed', 'pet_close', 'pet_page_tab'],
        'Pet — bag': ['pet_bag_grid', 'pet_bag_slot'],
        'Pet — rows': ['pet_toggle', 'pet_boarding_slot', 'pet_feeder_strip'],
        'Tuner': ['spring_point'],
      };
      var count = 0;
      for (final entry in groups.entries) {
        for (final kind in entry.value) {
          count += 1;
          // The menu is OPEN here, so Flutter builds each item twice: once in the button's own
          // IndexedStack copy and once in the menu route. Presence (not singularity) is the claim.
          expect(
            find.byKey(ValueKey('calibrate-kind-item-$kind')),
            findsWidgets,
            reason: '$kind is offered in ${entry.key}',
          );
        }
      }
      expect(
        count,
        18,
        reason: 'seven repaired kinds + eleven named-instance kinds',
      );
      expect(
        find.byKey(const ValueKey('calibrate-kind-item-focus_point')),
        findsNothing,
      );
      expect(
        find.byKey(const ValueKey('calibrate-kind-item-tuner_region')),
        findsNothing,
      );
      expect(
        find.text('Shop · Focus / scroll point (scroll_point)'),
        findsWidgets,
      );
      expect(find.text('Gem · Grade point (gem_grade)'), findsWidgets);
      expect(
        find.text('Pet — rows · Row food strip (pet_feeder_strip)'),
        findsWidgets,
      );
      // Tap the menu route's copy; the button's hidden copy is also in the tree while open.
      await tester.tap(
        find.byKey(const ValueKey('calibrate-kind-item-bag_grid')).last,
      );
      await tester.pumpAndSettle();
    },
  );

  testWidgets('max_button saves the exact point payload', (tester) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer()},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
            mark: [10, 20],
          ),
        },
        'calibration.save' => {
          'result': {
            'rev': 6,
            'kind': 'max_button',
            'mark': [10, 20],
            'derived': <String, dynamic>{},
            'artifact': null,
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    await _selectKind(tester, 'max_button');
    await tester.enterText(_field('calibrate-x'), '10');
    await tester.enterText(_field('calibrate-y'), '20');
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);

    final args = _saveArgs(harness);
    expect(args['kind'], 'max_button');
    expect(args['point'], {'x': 10, 'y': 20});
    expect(args.containsKey('region'), isFalse);
    expect(
      args.containsKey('empty'),
      isFalse,
      reason: 'the empty verdict belongs to result_gem_area only',
    );
    expect(
      tester.widget<Text>(find.byKey(const ValueKey('calibrate-stored'))).data,
      contains('[10, 20]'),
    );
  });

  testWidgets(
    'result_gem_area needs the empty verdict and sends empty true with the crop',
    (tester) async {
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
              'rev': 7,
              'kind': 'result_gem_area',
              'mark': [1, 2, 30, 30],
              'derived': <String, dynamic>{},
              'artifact': {
                'file': 'calib_gem_result.png',
                'path': 'config/calib_gem_result.png',
                'width': 220,
                'height': 60,
              },
            },
          },
          _ => {'result': <String, dynamic>{}},
        },
      );

      await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
      await _flush(tester);
      await _selectKind(tester, 'result_gem_area');
      await tester.enterText(_field('calibrate-x'), '1');
      await tester.enterText(_field('calibrate-y'), '2');
      await tester.enterText(_field('calibrate-w'), '30');
      await tester.enterText(_field('calibrate-h'), '30');

      expect(
        tester
            .widget<FilledButton>(find.byKey(const ValueKey('calibrate-save')))
            .onPressed,
        isNull,
        reason: 'the empty question is unanswered',
      );
      await tester.tap(find.byKey(const ValueKey('calibrate-empty-yes')));
      await tester.pump();
      expect(
        tester
            .widget<FilledButton>(find.byKey(const ValueKey('calibrate-save')))
            .onPressed,
        isNotNull,
      );

      await tester.tap(find.byKey(const ValueKey('calibrate-save')));
      await _flush(tester);

      final args = _saveArgs(harness);
      expect(args['kind'], 'result_gem_area');
      expect(args['empty'], true);
      expect(args['region'], {'x': 1, 'y': 2, 'w': 30, 'h': 30});
      expect(
        _resultText(tester, 'calibrate-artifact'),
        contains('calib_gem_result.png'),
      );
    },
  );

  testWidgets(
    'result_gem_area with a gem in the box sends empty false and no artifact',
    (tester) async {
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
              'rev': 8,
              'kind': 'result_gem_area',
              'mark': [1, 2, 30, 30],
              'derived': <String, dynamic>{},
              'artifact': null,
            },
          },
          _ => {'result': <String, dynamic>{}},
        },
      );

      await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
      await _flush(tester);
      await _selectKind(tester, 'result_gem_area');
      await tester.enterText(_field('calibrate-x'), '1');
      await tester.enterText(_field('calibrate-y'), '2');
      await tester.enterText(_field('calibrate-w'), '30');
      await tester.enterText(_field('calibrate-h'), '30');
      await tester.tap(find.byKey(const ValueKey('calibrate-empty-no')));
      await tester.pump();
      await tester.tap(find.byKey(const ValueKey('calibrate-save')));
      await _flush(tester);

      final args = _saveArgs(harness);
      expect(args['empty'], false);
      expect(
        find.byKey(const ValueKey('calibrate-artifact')),
        findsNothing,
        reason: 'a non-empty box samples the colour fallback only',
      );
    },
  );

  testWidgets('the tuner note is present and no tuner save is offered', (
    tester,
  ) async {
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

    expect(find.byKey(const ValueKey('calibrate-tuner-note')), findsOneWidget);
    expect(find.textContaining('three-box session'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('calibrate-kind')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('calibrate-kind-item-tuner_region')),
      findsNothing,
    );
    await tester.tap(
      find.byKey(const ValueKey('calibrate-kind-item-bag_grid')).last,
    );
    await tester.pumpAndSettle();
  });

  testWidgets('a fetched unknown kind renders its line, not a crash', (
    tester,
  ) async {
    await _pumpCalibrate(
      tester,
      answer: (request) {
        if (request['cmd'] != 'calibration.fetch') {
          return {'result': <String, dynamic>{}};
        }
        final kind =
            (request['args'] as Map<String, dynamic>)['kind'] as String;
        if (kind == 'spring_point') {
          return {
            'result': {
              'kind': 'pet_menu',
              'mark': [1, 2],
              'artifact': null,
              'rev': 7,
            },
          };
        }
        return {'result': _fetchAnswer(kind)};
      },
    );

    await _selectKind(tester, 'spring_point');
    final text = tester
        .widget<Text>(find.byKey(const ValueKey('calibrate-stored')))
        .data;
    expect(text, contains('pet_menu'));
    expect(text, contains('[1, 2]'));
  });

  testWidgets('pet_menu saves an untargeted pet point without a target', (
    tester,
  ) async {
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
            'rev': 9,
            'kind': 'pet_menu',
            'mark': [4, 5],
            'derived': <String, dynamic>{},
            'artifact': null,
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    await _selectKind(tester, 'pet_menu');
    expect(find.byKey(const ValueKey('calibrate-target')), findsNothing);
    await tester.enterText(_field('calibrate-x'), '4');
    await tester.enterText(_field('calibrate-y'), '5');
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);

    final args = _saveArgs(harness);
    expect(args['kind'], 'pet_menu');
    expect(args['point'], {'x': 4, 'y': 5});
    expect(
      args.containsKey('target'),
      isFalse,
      reason: 'a non-targeted kind must omit target entirely',
    );
  });

  testWidgets('gem_grade requires a target and saves the named point', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) {
        if (request['cmd'] != 'calibration.fetch' &&
            request['cmd'] != 'calibration.save' &&
            request['cmd'] != 'capture') {
          return {'result': <String, dynamic>{}};
        }
        final args = asJsonMap(request['args']);
        final target = args['target'] as String?;
        switch (request['cmd']) {
          case 'capture':
            return {'result': _captureAnswer()};
          case 'calibration.fetch':
            return {
              'result': _fetchAnswer(
                args['kind'] as String,
                mark: target == 'DG' ? [30, 40] : null,
                target: target,
              ),
            };
          case 'calibration.save':
            return {
              'result': {
                'rev': 11,
                'kind': 'gem_grade',
                'target': target,
                'mark': [30, 40],
                'derived': <String, dynamic>{},
                'artifact': null,
              },
            };
        }
        return {'result': <String, dynamic>{}};
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    await _selectKind(tester, 'gem_grade');
    expect(find.byKey(const ValueKey('calibrate-target')), findsOneWidget);
    expect(
      tester
          .widget<FilledButton>(find.byKey(const ValueKey('calibrate-save')))
          .onPressed,
      isNull,
      reason: 'a targeted kind cannot save before a target is chosen',
    );

    await _selectTarget(tester, 'DG');
    await tester.enterText(_field('calibrate-x'), '30');
    await tester.enterText(_field('calibrate-y'), '40');
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);

    final args = _saveArgs(harness);
    expect(args['kind'], 'gem_grade');
    expect(args['target'], 'DG');
    expect(args['point'], {'x': 30, 'y': 40});
    expect(args.containsKey('region'), isFalse);
    expect(
      tester
          .widget<Text>(find.byKey(const ValueKey('calibrate-stored-target')))
          .data,
      contains('DG'),
    );
  });

  testWidgets('pet_toggle saves the targeted row rect and keeps empty out', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) {
        final args = asJsonMap(request['args']);
        final target = args['target'] as String?;
        return switch (request['cmd']) {
          'capture' => {'result': _captureAnswer()},
          'calibration.fetch' => {
            'result': _fetchAnswer(
              args['kind'] as String,
              mark: target == '2' ? [10, 20, 100, 60] : null,
              target: target,
            ),
          },
          'calibration.save' => {
            'result': {
              'rev': 12,
              'kind': 'pet_toggle',
              'target': target,
              'mark': [10, 20, 100, 60],
              'derived': <String, dynamic>{},
              'artifact': null,
            },
          },
          _ => {'result': <String, dynamic>{}},
        };
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    await _selectKind(tester, 'pet_toggle');
    expect(
      tester
          .widget<FilledButton>(find.byKey(const ValueKey('calibrate-save')))
          .onPressed,
      isNull,
    );
    await _selectTarget(tester, '2');
    await tester.enterText(_field('calibrate-x'), '10');
    await tester.enterText(_field('calibrate-y'), '20');
    await tester.enterText(_field('calibrate-w'), '100');
    await tester.enterText(_field('calibrate-h'), '60');
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);

    final args = _saveArgs(harness);
    expect(args['kind'], 'pet_toggle');
    expect(args['target'], '2');
    expect(args['region'], {'x': 10, 'y': 20, 'w': 100, 'h': 60});
    expect(args.containsKey('point'), isFalse);
    expect(args.containsKey('empty'), isFalse);
  });

  testWidgets('switching kind resets the target', (tester) async {
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

    await _selectKind(tester, 'pet_toggle');
    await _selectTarget(tester, '2');
    expect(
      tester
          .widget<DropdownButton<String>>(
            find.byKey(const ValueKey('calibrate-target')),
          )
          .value,
      '2',
    );

    await _selectKind(tester, 'pet_feed');
    await _selectKind(tester, 'pet_toggle');
    expect(
      tester
          .widget<DropdownButton<String>>(
            find.byKey(const ValueKey('calibrate-target')),
          )
          .value,
      isNull,
      reason: 'a target never carries over to a different kind',
    );
    expect(
      tester
          .widget<FilledButton>(find.byKey(const ValueKey('calibrate-save')))
          .onPressed,
      isNull,
    );
  });

  testWidgets('fetch follows the target and renders the returned mark', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) {
        if (request['cmd'] != 'calibration.fetch') {
          return {'result': <String, dynamic>{}};
        }
        final args = asJsonMap(request['args']);
        final target = args['target'] as String?;
        return {
          'result': _fetchAnswer(
            args['kind'] as String,
            mark: target == '2' ? [10, 20] : null,
            target: target,
          ),
        };
      },
    );

    await _selectKind(tester, 'pet_toggle');
    await _selectTarget(tester, '2');
    expect(harness.lastSentWithCmd('calibration.fetch')!['args'], {
      'kind': 'pet_toggle',
      'target': '2',
    });
    expect(
      tester.widget<Text>(find.byKey(const ValueKey('calibrate-stored'))).data,
      contains('[10, 20]'),
    );
    expect(
      tester
          .widget<Text>(find.byKey(const ValueKey('calibrate-stored-target')))
          .data,
      contains('2'),
    );

    await _selectTarget(tester, '3');
    expect(harness.lastSentWithCmd('calibration.fetch')!['args'], {
      'kind': 'pet_toggle',
      'target': '3',
    });
    expect(
      find.byKey(const ValueKey('calibrate-not-calibrated')),
      findsOneWidget,
    );
  });

  testWidgets('a core refusal for an unknown target renders verbatim', (
    tester,
  ) async {
    const line =
        'pet_toggle target "4" names a row the config does not carry — rows: 1, 2';
    await _pumpCalibrate(
      tester,
      answer: (request) {
        if (request['cmd'] != 'calibration.fetch') {
          return {'result': <String, dynamic>{}};
        }
        final args = asJsonMap(request['args']);
        if (args['target'] == '4') {
          return {
            'error': {'code': 'bad_request', 'message': line},
          };
        }
        return {
          'result': _fetchAnswer(
            args['kind'] as String,
            target: args['target'] as String?,
          ),
        };
      },
    );

    await _selectKind(tester, 'pet_toggle');
    await _selectTarget(tester, '4');
    expect(
      tester
          .widget<Text>(find.byKey(const ValueKey('calibrate-stored-error')))
          .data,
      line,
    );
  });

  testWidgets('capture hides the shell, sends capture, then shows it', (
    tester,
  ) async {
    final host = _FakeHost();
    await _pumpCalibrate(
      tester,
      host: host,
      answer: (request) {
        if (request['cmd'] == 'capture') {
          host.events.add('capture-sent');
          return {'result': _captureAnswer()};
        }
        if (request['cmd'] == 'calibration.fetch') {
          return {
            'result': _fetchAnswer(
              (request['args'] as Map<String, dynamic>)['kind'] as String,
            ),
          };
        }
        return {'result': <String, dynamic>{}};
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);

    expect(host.events, ['hide', 'capture-sent', 'show']);
    expect(
      _resultText(tester, 'calibrate-capture-result'),
      contains('captured frame f1'),
    );
    expect(find.byKey(const ValueKey('calibrate-host-note')), findsNothing);
  });

  testWidgets('a refused capture still shows the shell again', (tester) async {
    final host = _FakeHost();
    await _pumpCalibrate(
      tester,
      host: host,
      answer: (request) {
        if (request['cmd'] == 'capture') {
          host.events.add('capture-sent');
          return {
            'error': {
              'code': 'game_not_foreground',
              'message': 'the game is not the foreground window',
            },
          };
        }
        if (request['cmd'] == 'calibration.fetch') {
          return {
            'result': _fetchAnswer(
              (request['args'] as Map<String, dynamic>)['kind'] as String,
            ),
          };
        }
        return {'result': <String, dynamic>{}};
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);

    expect(host.events, ['hide', 'capture-sent', 'show']);
    expect(
      _resultText(tester, 'calibrate-capture-result'),
      contains('the game is not the foreground window'),
    );
    expect(find.byKey(const ValueKey('calibrate-frame')), findsNothing);
  });

  testWidgets('capture proceeds when the host channel is absent', (
    tester,
  ) async {
    final host = _FakeHost(failHide: true, failShow: true);
    await _pumpCalibrate(
      tester,
      host: host,
      answer: (request) {
        if (request['cmd'] == 'capture') {
          host.events.add('capture-sent');
          return {'result': _captureAnswer()};
        }
        if (request['cmd'] == 'calibration.fetch') {
          return {
            'result': _fetchAnswer(
              (request['args'] as Map<String, dynamic>)['kind'] as String,
            ),
          };
        }
        return {'result': <String, dynamic>{}};
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);

    expect(host.events, ['hide', 'capture-sent', 'show']);
    expect(
      _resultText(tester, 'calibrate-capture-result'),
      contains('captured frame f1'),
    );
    expect(
      find.byKey(const ValueKey('calibrate-host-note')),
      findsOneWidget,
      reason: 'a missing host channel is a note, never a failed capture',
    );
  });

  testWidgets('Copy as YAML copies the saved marks and writes nothing', (
    tester,
  ) async {
    String? copied;
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (call) async {
        if (call.method == 'Clipboard.setData') {
          copied = (call.arguments as Map)['text'] as String?;
        }
        return null;
      },
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        null,
      ),
    );
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
            mark:
                (request['args'] as Map<String, dynamic>)['kind'] == 'bag_grid'
                ? const [10, 20, 30, 40]
                : null,
          ),
        },
        _ => {'result': <String, dynamic>{}},
      },
    );
    await _flush(tester);
    final copy = find.byKey(const ValueKey('calibrate-copy-yaml'));
    await tester.ensureVisible(copy);
    await tester.pump();
    await tester.tap(copy);
    await _flush(tester);

    expect(copied, isNotNull);
    expect(copied, contains("'bag_grid':"));
    expect(copied, contains('  x: 10'));
    expect(copied, contains('  w: 30'));
    expect(
      harness.sentMessages.where(
        (message) => message['cmd'] == 'calibration.save',
      ),
      isEmpty,
      reason: 'copying is a read of values already on screen, not a save',
    );
  });

  testWidgets('Detect renders the core check verdict and differences', (
    tester,
  ) async {
    await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        'calibration.check' => {
          'result': {
            'live': _displayJson(),
            'stored': {
              'scale': 1.25,
              'monitor_dpi': 120,
              'screen': {'w': 2560, 'h': 1440},
              'client_size': {'w': 1910, 'h': 1193},
              'measured_at': '2026-10-31T12:00:00',
            },
            'differences': ['scale 1.5 → 1.25'],
            'matches': false,
            'note': null,
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );
    await _flush(tester);

    expect(find.byKey(const ValueKey('detect-panel')), findsOneWidget);
    expect(find.text('scale 1.5 → 1.25'), findsOneWidget);
    expect(find.textContaining('differs in 1'), findsOneWidget);
    expect(find.textContaining('saved scale 1.25'), findsOneWidget);
  });

  testWidgets('Detect says the game is closed instead of failing', (
    tester,
  ) async {
    await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        'calibration.check' => {
          'result': {
            'live': null,
            'stored': {'scale': 1.5, 'measured_at': '2026-10-31T12:00:00'},
            'differences': <String>[],
            'matches': null,
            'note': 'game window not open',
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );
    await _flush(tester);

    expect(find.text('game window not open'), findsOneWidget);
    expect(find.byKey(const ValueKey('detect-error')), findsNothing);
  });

  testWidgets(
    'a save sends the fetched rev and shows the core measured_at echo',
    (tester) async {
      final harness = await _pumpCalibrate(
        tester,
        answer: (request) => switch (request['cmd']) {
          'capture' => {'result': _captureAnswer()},
          'calibration.fetch' => {
            'result': _fetchAnswer(
              (request['args'] as Map<String, dynamic>)['kind'] as String,
              mark: [10, 20],
            ),
          },
          'calibration.save' => {
            'result': {
              'rev': 8,
              'kind': 'max_button',
              'mark': [10, 20],
              'derived': <String, dynamic>{},
              'artifact': null,
              'calibration': {
                'dpi_scale': 1.5,
                'monitor_dpi': 144,
                'screen': {'w': 2560, 'h': 1440},
                'client_size': [4, 3],
                'measured_at': '2026-11-01T00:00:00Z',
              },
            },
          },
          _ => {'result': <String, dynamic>{}},
        },
      );

      await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
      await _flush(tester);
      await _selectKind(tester, 'max_button');
      await tester.enterText(_field('calibrate-x'), '10');
      await tester.enterText(_field('calibrate-y'), '20');
      await tester.tap(find.byKey(const ValueKey('calibrate-save')));
      await _flush(tester);

      final args = _saveArgs(harness);
      expect(
        args['rev'],
        7,
        reason: 'the rev from the fetch travels with the save',
      );
      expect(
        tester
            .widget<Text>(
              find.byKey(const ValueKey('calibrate-stored-environment')),
            )
            .data,
        contains('measured 2026-11-01T00:00:00Z'),
        reason: 'the shell shows the core echo, not a re-derived stamp',
      );
    },
  );

  testWidgets('a conflict refuses the save and re-fetches the current mark', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer()},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
            mark: [10, 20],
          ),
        },
        'calibration.save' => {
          'error': {
            'code': 'conflict',
            'message': 'calibration changed while you were editing',
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );

    await tester.tap(find.byKey(const ValueKey('calibrate-capture')));
    await _flush(tester);
    await _selectKind(tester, 'max_button');
    await tester.enterText(_field('calibrate-x'), '10');
    await tester.enterText(_field('calibrate-y'), '20');
    final fetchesBefore = harness.sentMessages
        .where(
          (message) =>
              message['cmd'] == 'calibration.fetch' &&
              (message['args'] as Map<String, dynamic>)['kind'] == 'max_button',
        )
        .length;
    await tester.tap(find.byKey(const ValueKey('calibrate-save')));
    await _flush(tester);

    expect(
      find.textContaining('calibration changed while you were editing'),
      findsWidgets,
    );
    final fetchesAfter = harness.sentMessages
        .where(
          (message) =>
              message['cmd'] == 'calibration.fetch' &&
              (message['args'] as Map<String, dynamic>)['kind'] == 'max_button',
        )
        .length;
    expect(
      fetchesAfter,
      greaterThan(fetchesBefore),
      reason: 'a conflict refreshes the mark the core refused against',
    );
  });

  testWidgets('opening the workspace loads every mark with ONE fetch_all', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'calibration.fetch_all' => {
          'result': {
            'rev': 9,
            'calibration': {
              'dpi_scale': 1.5,
              'measured_at': '2026-11-01T00:00:00Z',
            },
            'marks': [
              {
                'kind': 'bag_grid',
                'target': null,
                'mark': [10, 20, 30, 40],
                'artifact': null,
              },
              {
                'kind': 'max_button',
                'target': null,
                'mark': [50, 60],
                'artifact': null,
              },
            ],
            'withdrawn': <Map<String, dynamic>>[],
          },
        },
        'calibration.check' => {
          'result': {
            'live': null,
            'stored': null,
            'differences': <String>[],
            'matches': null,
            'note': 'game window not open',
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );
    await _flush(tester);

    expect(
      harness.sentMessages.where(
        (message) => message['cmd'] == 'calibration.fetch_all',
      ),
      hasLength(1),
      reason: 'the workspace opens with one request, not 35',
    );
    expect(
      harness.sentMessages.where(
        (message) => message['cmd'] == 'calibration.fetch',
      ),
      isEmpty,
      reason: 'fetch_all covered every served mark',
    );
    expect(find.textContaining('10, 20, 30, 40'), findsWidgets);
  });

  testWidgets('the tooltip session drives hover, save and test over the core', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'calibration.fetch_all' => {
          'result': {
            'rev': 9,
            'marks': <Map<String, dynamic>>[],
            'calibration': null,
            'withdrawn': <Map<String, dynamic>>[],
          },
        },
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        'calibration.check' => {
          'result': {
            'live': null,
            'stored': null,
            'differences': <String>[],
            'matches': null,
            'note': 'game window not open',
          },
        },
        'calibration.tooltip.hover_capture' => {
          'result': {
            'frame': {
              'format': 'png',
              'width': 4,
              'height': 3,
              'origin': {'x': 0, 'y': 0},
              'data': _tinyPng,
              'frame_id': 'f2',
            },
            'display': _displayJson(),
            'placed': {'x': 300, 'y': 400},
            'line': 'hover captured',
          },
        },
        'calibration.tooltip.save' => {
          'result': {
            'rev': 8,
            'block': {
              'tooltip': {'offset_x': -8, 'offset_y': -6},
            },
          },
        },
        'calibration.tooltip.test' => {
          'result': {
            'lines': [
              {'text': 'panel line one', 'score': 0.9},
            ],
            'region': {'x': 292, 'y': 394, 'width': 320, 'height': 210},
            'debug_path': null,
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );
    await _flush(tester);

    final hover = find.byKey(const ValueKey('session-tooltip-hover'));
    await tester.ensureVisible(hover);
    await tester.tap(hover);
    await _flush(tester);
    final hoverArgs =
        harness.sentMessages.lastWhere(
              (message) =>
                  message['cmd'] == 'calibration.tooltip.hover_capture',
            )['args']
            as Map<String, dynamic>;
    expect(hoverArgs['point'], {'x': 0, 'y': 0});
    expect(hoverArgs['hover_delay_ms'], 700);
    expect(
      find.byKey(const ValueKey('session-tooltip-preview')),
      findsOneWidget,
    );

    await tester.enterText(
      find.byKey(const ValueKey('session-tooltip-offset-x')),
      '-8',
    );
    await tester.enterText(
      find.byKey(const ValueKey('session-tooltip-offset-y')),
      '-6',
    );
    await tester.enterText(
      find.byKey(const ValueKey('session-tooltip-width')),
      '320',
    );
    await tester.enterText(
      find.byKey(const ValueKey('session-tooltip-height')),
      '210',
    );
    final save = find.byKey(const ValueKey('session-tooltip-save'));
    await tester.ensureVisible(save);
    await tester.tap(save);
    await _flush(tester);
    final saveArgs =
        harness.sentMessages.lastWhere(
              (message) => message['cmd'] == 'calibration.tooltip.save',
            )['args']
            as Map<String, dynamic>;
    expect(saveArgs['offset_x'], -8);
    expect(saveArgs['width'], 320);

    final test = find.byKey(const ValueKey('session-tooltip-test'));
    await tester.ensureVisible(test);
    await tester.tap(test);
    await _flush(tester);
    expect(find.text('panel line one'), findsOneWidget);
  });

  testWidgets('the tuner session captures, checks and saves three boxes', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer(frameId: 'f9')},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        'calibration.check' => {
          'result': {
            'live': null,
            'stored': null,
            'differences': <String>[],
            'matches': null,
            'note': 'game window not open',
          },
        },
        'calibration.tuner.check' => {
          'result': {
            'geometry': {
              'region': {'left': 0, 'top': 0, 'width': 300, 'height': 200},
            },
            'measured_row_height': 19,
            'grade': 'DG',
            'remaining': 25000,
            'attributes': <List<String>>[],
            'matched': [
              {'name': 'STR', 'value': 10},
            ],
            'filter': {'enabled': true, 'passed': true, 'reason': 'ok'},
            'warnings': <String>[],
          },
        },
        'calibration.tuner.save' => {
          'result': {
            'rev': 12,
            'block': {
              'tuner': {'ocr': <String, dynamic>{}},
            },
            'derived': <String, dynamic>{},
            'artifact': {
              'file': 'calib_tuner.png',
              'path': 'config/calib_tuner.png',
            },
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );
    await _flush(tester);

    final capture = find.byKey(const ValueKey('session-tuner-capture'));
    await tester.ensureVisible(capture);
    await tester.tap(capture);
    await _flush(tester);

    final check = find.byKey(const ValueKey('session-tuner-check'));
    await tester.ensureVisible(check);
    await tester.tap(check);
    await _flush(tester);
    final checkArgs =
        harness.sentMessages.lastWhere(
              (message) => message['cmd'] == 'calibration.tuner.check',
            )['args']
            as Map<String, dynamic>;
    expect(checkArgs['frame_id'], 'f9');
    final boxes = checkArgs['boxes'] as Map<String, dynamic>;
    expect((boxes['grade'] as Map<String, dynamic>)['w'], 100);
    expect(find.byKey(const ValueKey('session-tuner-matched')), findsOneWidget);

    final save = find.byKey(const ValueKey('session-tuner-save'));
    await tester.ensureVisible(save);
    await tester.tap(save);
    await _flush(tester);
    final saveArgs =
        harness.sentMessages.lastWhere(
              (message) => message['cmd'] == 'calibration.tuner.save',
            )['args']
            as Map<String, dynamic>;
    expect(saveArgs['frame_id'], 'f9');
    expect(saveArgs['display'], isNotNull);
    expect(saveArgs['display']['dpi_scale'], 1.5);
    expect(find.textContaining('tuner saved'), findsOneWidget);
  });

  testWidgets('the pet session previews, saves and test-reads one row', (
    tester,
  ) async {
    final harness = await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'capture' => {'result': _captureAnswer(frameId: 'f7')},
        'calibration.fetch' => {
          'result': _fetchAnswer(
            (request['args'] as Map<String, dynamic>)['kind'] as String,
          ),
        },
        'calibration.check' => {
          'result': {
            'live': null,
            'stored': null,
            'differences': <String>[],
            'matches': null,
            'note': 'game window not open',
          },
        },
        'calibration.pet.preview' => {
          'result': {
            'rows': [
              {
                'slot_boxes': <Map<String, dynamic>>[],
                'count_regions': <Map<String, dynamic>>[],
                'problems': <String>[],
              },
            ],
            'bag_centres': <List<int>>[],
            'problem': null,
          },
        },
        'calibration.pet.save' => {
          'result': {
            'rev': 11,
            'block': {
              'pet': {'slots': <Map<String, dynamic>>[]},
            },
            'artifacts': [
              {'file': 'calib_pet.png', 'path': 'config/calib_pet.png'},
            ],
            'derived': <String, dynamic>{},
            'warnings': <String>[],
          },
        },
        'calibration.pet.test_read' => {
          'result': {
            'rows': [
              {
                'row': 1,
                'slots': [
                  {'index': 1, 'reads': <Map<String, dynamic>>[], 'count': 123},
                ],
              },
            ],
            'hidden_ms': 700,
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );
    await _flush(tester);

    final capture = find.byKey(const ValueKey('session-pet-capture'));
    await tester.ensureVisible(capture);
    await tester.tap(capture);
    await _flush(tester);

    final preview = find.byKey(const ValueKey('session-pet-preview'));
    await tester.ensureVisible(preview);
    await tester.tap(preview);
    await _flush(tester);
    final previewArgs =
        harness.sentMessages.lastWhere(
              (message) => message['cmd'] == 'calibration.pet.preview',
            )['args']
            as Map<String, dynamic>;
    final draft = previewArgs['draft'] as Map<String, dynamic>;
    expect((draft['rows'] as List).length, 1);
    expect(
      ((draft['rows'] as List).first as Map<String, dynamic>)['stacks'],
      2,
    );

    final save = find.byKey(const ValueKey('session-pet-save'));
    await tester.ensureVisible(save);
    await tester.tap(save);
    await _flush(tester);
    final saveArgs =
        harness.sentMessages.lastWhere(
              (message) => message['cmd'] == 'calibration.pet.save',
            )['args']
            as Map<String, dynamic>;
    expect(saveArgs['frame_id'], 'f7');
    expect(saveArgs['points'], isNotNull);
    expect(saveArgs['display']['dpi_scale'], 1.5);

    final test = find.byKey(const ValueKey('session-pet-test'));
    await tester.ensureVisible(test);
    await tester.tap(test);
    await _flush(tester);
    expect(find.textContaining('1 slot(s) read'), findsOneWidget);
  });

  testWidgets('the gallery shows the session saves as present, not v2 only', (
    tester,
  ) async {
    await _pumpCalibrate(
      tester,
      answer: (request) => switch (request['cmd']) {
        'calibration.fetch_all' => {
          'result': {
            'rev': 9,
            'calibration': {'dpi_scale': 1.5},
            'marks': [
              {
                'kind': 'bag_grid',
                'target': null,
                'mark': [10, 20, 30, 40],
                'artifact': null,
              },
            ],
            'gallery': [
              for (final name in ['tuner', 'gem', 'pet'])
                {
                  'name': name,
                  'file': 'calib_$name.png',
                  'width': 4,
                  'height': 3,
                  'png_base64': _tinyPng,
                },
            ],
            'withdrawn': <Map<String, dynamic>>[],
          },
        },
        'calibration.check' => {
          'result': {
            'live': null,
            'stored': null,
            'differences': <String>[],
            'matches': null,
            'note': 'game window not open',
          },
        },
        _ => {'result': <String, dynamic>{}},
      },
    );
    await _flush(tester);

    expect(find.text('present'), findsNWidgets(3));
    expect(find.text('v2 only'), findsNothing);
    expect(
      find.byKey(const ValueKey('calibrate-gallery-tuner')),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('calibrate-gallery-gem')), findsOneWidget);
    expect(find.byKey(const ValueKey('calibrate-gallery-pet')), findsOneWidget);
  });

  testWidgets(
    'the tuner session renders its frame and drags fill the three boxes in order',
    (tester) async {
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
      await _flush(tester);

      expect(
        find.byKey(const ValueKey('session-tuner-frame')),
        findsNothing,
        reason: 'the frame exists only once the session is captured',
      );

      final capture = find.byKey(const ValueKey('session-tuner-capture'));
      await tester.ensureVisible(capture);
      await tester.tap(capture);
      await _flush(tester);

      expect(
        find.byKey(const ValueKey('session-tuner-frame')),
        findsOneWidget,
        reason: 'the captured frame is rendered so the operator can box it',
      );

      String fieldText(String key) =>
          tester.widget<TextField>(_field(key)).controller!.text;

      // The first drag writes GRADE and advances the target to attr (v2's order).
      await _dragOnSessionFrame(
        tester,
        'session-tuner-frame',
        const Offset(80, 60),
      );
      expect(int.parse(fieldText('session-tuner-grade.x')), greaterThan(0));
      expect(int.parse(fieldText('session-tuner-grade.w')), greaterThan(0));
      expect(fieldText('session-tuner-attr.x'), '0');
      expect(fieldText('session-tuner-attr.w'), '100');
      final attrChip = tester.widget<ChoiceChip>(
        find.byKey(const ValueKey('session-tuner-target-attr')),
      );
      expect(attrChip.selected, isTrue);

      // The second drag writes ATTR, not grade again.
      await _dragOnSessionFrame(
        tester,
        'session-tuner-frame',
        const Offset(80, 60),
      );
      expect(int.parse(fieldText('session-tuner-attr.x')), greaterThan(0));
      expect(int.parse(fieldText('session-tuner-attr.w')), greaterThan(0));
      expect(fieldText('session-tuner-remaining.x'), '0');

      // The typed fields stay authoritative: Save sends what the drags wrote.
      final save = find.byKey(const ValueKey('session-tuner-save'));
      await tester.ensureVisible(save);
      await tester.tap(save);
      await _flush(tester);
      final saveArgs =
          harness.sentMessages.lastWhere(
                (message) => message['cmd'] == 'calibration.tuner.save',
              )['args']
              as Map<String, dynamic>;
      final boxes = saveArgs['boxes'] as Map<String, dynamic>;
      final grade = boxes['grade'] as Map<String, dynamic>;
      expect(grade['x'], int.parse(fieldText('session-tuner-grade.x')));
      expect(grade['w'], int.parse(fieldText('session-tuner-grade.w')));
      expect(
        (boxes['attr'] as Map<String, dynamic>)['w'],
        int.parse(fieldText('session-tuner-attr.w')),
      );
    },
  );

  testWidgets(
    'the pet session renders its frame; a drag fills the chosen rect and a click fills a point',
    (tester) async {
      final harness = await _pumpCalibrate(
        tester,
        answer: (request) => switch (request['cmd']) {
          'capture' => {
            'result': _captureAnswer(frameId: 'f7', width: 100, height: 75),
          },
          'calibration.fetch' => {
            'result': _fetchAnswer(
              (request['args'] as Map<String, dynamic>)['kind'] as String,
            ),
          },
          _ => {'result': <String, dynamic>{}},
        },
      );
      await _flush(tester);

      expect(find.byKey(const ValueKey('session-pet-frame')), findsNothing);
      final capture = find.byKey(const ValueKey('session-pet-capture'));
      await tester.ensureVisible(capture);
      await tester.tap(capture);
      await _flush(tester);
      expect(
        find.byKey(const ValueKey('session-pet-frame')),
        findsOneWidget,
        reason: 'the captured frame is rendered so the rows can be dragged',
      );

      String fieldText(String key) =>
          tester.widget<TextField>(_field(key)).controller!.text;

      // The default target is the toggle row: a drag fills its rect and leaves the others alone.
      await _dragOnSessionFrame(
        tester,
        'session-pet-frame',
        const Offset(80, 60),
      );
      expect(
        int.parse(fieldText('session-tuner-pet.toggle.x')),
        greaterThan(0),
      );
      expect(
        int.parse(fieldText('session-tuner-pet.toggle.w')),
        greaterThan(0),
      );
      expect(fieldText('session-tuner-pet.boarding.x'), '0');
      expect(fieldText('session-tuner-pet.boarding.w'), '0');

      // Pick the bag grid explicitly, then drag: the values land in the bag-grid fields.
      await _tapVisible(tester, 'session-pet-target-grid');
      await _dragOnSessionFrame(
        tester,
        'session-pet-frame',
        const Offset(80, 60),
      );
      expect(int.parse(fieldText('session-tuner-pet.grid.x')), greaterThan(0));
      expect(int.parse(fieldText('session-tuner-pet.grid.w')), greaterThan(0));

      // A POINT target takes a click instead of a drag.
      await _tapVisible(tester, 'session-pet-target-menu');
      await _clickOnSessionFrame(
        tester,
        'session-pet-frame',
        const Offset(30, 10),
      );
      expect(int.parse(fieldText('session-pet-menu-x')), greaterThan(0));
      expect(int.parse(fieldText('session-pet-menu-y')), greaterThan(0));

      // Save sends the fields the gestures wrote.
      final save = find.byKey(const ValueKey('session-pet-save'));
      await tester.ensureVisible(save);
      await tester.tap(save);
      await _flush(tester);
      final saveArgs =
          harness.sentMessages.lastWhere(
                (message) => message['cmd'] == 'calibration.pet.save',
              )['args']
              as Map<String, dynamic>;
      final rows = (saveArgs['rows'] as List).cast<Map<String, dynamic>>();
      final toggle = rows.first['toggle_label'] as Map<String, dynamic>;
      expect(toggle['x'], int.parse(fieldText('session-tuner-pet.toggle.x')));
      expect(toggle['w'], int.parse(fieldText('session-tuner-pet.toggle.w')));
      final bag = saveArgs['bag'] as Map<String, dynamic>;
      final grid = (bag['grid'] as List).cast<int>();
      expect(grid[0], int.parse(fieldText('session-tuner-pet.grid.x')));
      expect(grid[2], int.parse(fieldText('session-tuner-pet.grid.w')));
      final points = saveArgs['points'] as Map<String, dynamic>;
      final menu = points['menu'] as Map<String, dynamic>;
      expect(menu['x'], int.parse(fieldText('session-pet-menu-x')));
      expect(menu['y'], int.parse(fieldText('session-pet-menu-y')));
    },
  );

  testWidgets(
    'a drag on the hovered tooltip picture fills the offset and size fields',
    (tester) async {
      final harness = await _pumpCalibrate(
        tester,
        answer: (request) => switch (request['cmd']) {
          'calibration.fetch_all' => {
            'result': {
              'rev': 9,
              'marks': <Map<String, dynamic>>[],
              'calibration': null,
              'withdrawn': <Map<String, dynamic>>[],
            },
          },
          'calibration.fetch' => {
            'result': _fetchAnswer(
              (request['args'] as Map<String, dynamic>)['kind'] as String,
            ),
          },
          'calibration.tooltip.hover_capture' => {
            'result': {
              'frame': {
                'format': 'png',
                'width': 100,
                'height': 75,
                'origin': {'x': 10, 'y': 20},
                'data': _tinyPng,
                'frame_id': 'f2',
              },
              'display': _displayJson(),
              'placed': {'x': 0, 'y': 0},
              'line': 'hover captured — drag a box around the panel',
            },
          },
          _ => {'result': <String, dynamic>{}},
        },
      );
      await _flush(tester);

      final hover = find.byKey(const ValueKey('session-tooltip-hover'));
      await tester.ensureVisible(hover);
      await tester.tap(hover);
      await _flush(tester);
      expect(
        find.byKey(const ValueKey('session-tooltip-preview')),
        findsOneWidget,
      );

      await _dragOnSessionFrame(
        tester,
        'session-tooltip-preview',
        const Offset(100, 80),
      );

      String fieldText(String key) =>
          tester.widget<TextField>(_field(key)).controller!.text;
      // The offset is the box minus the point the tool placed the cursor on (0,0 here), so a
      // left/top drag gives non-negative offsets and a real size.
      expect(int.parse(fieldText('session-tooltip-offset-x')), greaterThan(0));
      expect(int.parse(fieldText('session-tooltip-width')), greaterThan(0));
      expect(int.parse(fieldText('session-tooltip-height')), greaterThan(0));
      expect(
        tester
            .widget<Text>(find.byKey(const ValueKey('session-tooltip-status')))
            .data,
        contains('panel box'),
      );

      final save = find.byKey(const ValueKey('session-tooltip-save'));
      await tester.ensureVisible(save);
      await tester.tap(save);
      await _flush(tester);
      final saveArgs =
          harness.sentMessages.lastWhere(
                (message) => message['cmd'] == 'calibration.tooltip.save',
              )['args']
              as Map<String, dynamic>;
      expect(
        saveArgs['offset_x'],
        int.parse(fieldText('session-tooltip-offset-x')),
      );
      expect(saveArgs['width'], int.parse(fieldText('session-tooltip-width')));
      expect(
        saveArgs['height'],
        int.parse(fieldText('session-tooltip-height')),
      );
    },
  );
}

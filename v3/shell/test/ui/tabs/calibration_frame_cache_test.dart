/// SHELL-F3: a captured frame's multi-MB base64 PNG is decoded ONCE, not on every build.
///
/// The calibration canvas calls `setState` for each pan/zoom update, so decoding in `build` re-ran
/// base64 decode and a full PNG decode for every pointer move. These tests inject a counting decoder
/// and prove the built widget keeps one decode across many rebuilds.
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:seal_shell/protocol/protocol.dart' as proto;
import 'package:seal_shell/ui/tabs/calibration_widgets.dart';

/// A valid 1x1 PNG: the widget must hand the decoder's bytes to `Image.memory`, which decodes real
/// image data even though the test never base64-decodes the wire payload itself.
final Uint8List _pixelPng = base64Decode(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAFgwJ/'
  'lK3Q6wAAAABJRU5ErkJggg==',
);

proto.Frame _frame(String id) => proto.Frame(
  format: 'png',
  width: 100,
  height: 60,
  origin: proto.Point(x: 0, y: 0),
  data: 'frame-$id-base64',
  frameId: id,
);

void main() {
  testWidgets('50 drag updates decode the frame exactly once', (tester) async {
    var decodes = 0;
    Uint8List? countingDecoder(String data) {
      decodes++;
      return _pixelPng;
    }

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 400,
            height: 400,
            child: CalibrationFramePicture(
              frame: _frame('f1'),
              maxHeight: 300,
              onRectMark: (_) {},
              decode: countingDecoder,
            ),
          ),
        ),
      ),
    );
    expect(decodes, 1, reason: 'the first build decodes the frame');

    final area = find.byKey(const ValueKey('calibrate-picture-area'));
    expect(area, findsOneWidget);
    final gesture = await tester.startGesture(tester.getCenter(area));
    for (var i = 0; i < 50; i++) {
      await gesture.moveBy(const Offset(1.5, 0.5));
      await tester.pump();
    }
    await gesture.up();
    await tester.pumpAndSettle();

    expect(
      decodes,
      1,
      reason: 'every drag setState must reuse the decoded bytes (SHELL-F3)',
    );
  });

  testWidgets('a parent rebuild does not decode the frame again', (tester) async {
    var decodes = 0;
    Uint8List? countingDecoder(String data) {
      decodes++;
      return _pixelPng;
    }

    final frame = _frame('f2');
    late StateSetter rebuild;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: StatefulBuilder(
            builder: (context, setState) {
              rebuild = setState;
              return CalibrationFramePicture(
                frame: frame,
                maxHeight: 300,
                decode: countingDecoder,
              );
            },
          ),
        ),
      ),
    );
    expect(decodes, 1);
    for (var i = 0; i < 5; i++) {
      rebuild(() {});
      await tester.pump();
    }
    expect(decodes, 1);
  });

  testWidgets('a stored crop decodes once across parent rebuilds', (tester) async {
    var decodes = 0;
    Uint8List? countingDecoder(String data) {
      decodes++;
      return _pixelPng;
    }

    late StateSetter rebuild;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: StatefulBuilder(
            builder: (context, setState) {
              rebuild = setState;
              return CalibrationCropPicture(
                file: 'tuner.grade.png',
                pngBase64: 'crop-base64',
                decode: countingDecoder,
              );
            },
          ),
        ),
      ),
    );
    expect(decodes, 1);
    for (var i = 0; i < 5; i++) {
      rebuild(() {});
      await tester.pump();
    }
    expect(decodes, 1);
  });
}

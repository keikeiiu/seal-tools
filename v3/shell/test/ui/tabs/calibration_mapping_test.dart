/// Pure tests for the frame-picture → mark mapping (no widgets): letterboxing, scale, origins,
/// rounding/clamping, drag normalisation and the click-vs-drag threshold.
library;

import 'dart:ui' as ui;

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/protocol/protocol.dart';
import 'package:seal_shell/ui/tabs/calibration_widgets.dart';

Frame _frame({
  int width = 200,
  int height = 100,
  int originX = 0,
  int originY = 0,
}) => Frame.fromJson({
  'format': 'png',
  'width': width,
  'height': height,
  'origin': {'x': originX, 'y': originY},
  'data': '',
  'frame_id': 'f1',
}, 'frame');

void main() {
  group('frameContainLayout', () {
    test('wide frame in a square box letterboxes top and bottom', () {
      final layout = frameContainLayout(
        box: const ui.Size(100, 100),
        frameWidth: 200,
        frameHeight: 100,
      );
      expect(layout.scale, 0.5);
      expect(layout.picture, const ui.Rect.fromLTWH(0, 25, 100, 50));
    });

    test('tall frame in a wide box letterboxes left and right', () {
      final layout = frameContainLayout(
        box: const ui.Size(200, 100),
        frameWidth: 100,
        frameHeight: 200,
      );
      expect(layout.scale, 0.5);
      expect(layout.picture, const ui.Rect.fromLTWH(75, 0, 50, 100));
    });

    test('upscales a small frame without stretching it', () {
      final layout = frameContainLayout(
        box: const ui.Size(150, 150),
        frameWidth: 50,
        frameHeight: 50,
      );
      expect(layout.scale, 3);
      expect(layout.picture, const ui.Rect.fromLTWH(0, 0, 150, 150));
    });
  });

  group('framePointToMark', () {
    test('picture centre maps to the frame centre', () {
      final mark = framePointToMark(
        local: const ui.Offset(50, 50),
        box: const ui.Size(100, 100),
        frame: _frame(),
        clientOrigin: ui.Offset.zero,
      );
      expect(mark, isNotNull);
      expect(mark!.isRect, isFalse);
      expect([mark.x, mark.y], [100, 50]);
    });

    test('picture corners map to the frame corners', () {
      final topLeft = framePointToMark(
        local: const ui.Offset(0, 25),
        box: const ui.Size(100, 100),
        frame: _frame(),
        clientOrigin: ui.Offset.zero,
      );
      final bottomRight = framePointToMark(
        local: const ui.Offset(99.8, 74.8),
        box: const ui.Size(100, 100),
        frame: _frame(),
        clientOrigin: ui.Offset.zero,
      );
      expect([topLeft!.x, topLeft.y], [0, 0]);
      expect([bottomRight!.x, bottomRight.y], [199, 99]);
    });

    test('a click in the letterbox is ignored, never clamped', () {
      final mark = framePointToMark(
        local: const ui.Offset(50, 10),
        box: const ui.Size(100, 100),
        frame: _frame(),
        clientOrigin: ui.Offset.zero,
      );
      expect(mark, isNull);
    });

    test('a non-1.0 display scale maps through the scale', () {
      // frame 50×50 shown at 3× in a 150×150 box: local (75,75) is the centre.
      final mark = framePointToMark(
        local: const ui.Offset(75, 75),
        box: const ui.Size(150, 150),
        frame: _frame(width: 50, height: 50),
        clientOrigin: ui.Offset.zero,
      );
      expect([mark!.x, mark.y], [25, 25]);
    });

    test('a sub-region capture subtracts the client origin', () {
      // The capture starts at screen (100,50) while the client starts at (20,10): the client-relative
      // offset of the frame's first pixel is (80,40).
      final mark = framePointToMark(
        local: const ui.Offset(0, 0),
        box: const ui.Size(200, 100),
        frame: _frame(originX: 100, originY: 50),
        clientOrigin: const ui.Offset(20, 10),
      );
      expect([mark!.x, mark.y], [80, 40]);
    });
  });

  group('frameDragToMark', () {
    test('a forward drag becomes an inclusive pixel rect', () {
      final mark = frameDragToMark(
        startLocal: const ui.Offset(10, 10),
        endLocal: const ui.Offset(60, 40),
        box: const ui.Size(100, 100),
        frame: _frame(),
        clientOrigin: ui.Offset.zero,
      );
      // y local 10 → (10-25)/.5 < 0 clamps to 0; established maths: x 20..120, y 0..30.
      expect(mark, isNotNull);
      expect(mark!.isRect, isTrue);
      expect(mark.x, 20);
      expect(mark.y, 0);
      expect(mark.w, 101);
      expect(mark.h, 31);
    });

    test('a click (no movement) is not a rect', () {
      final mark = frameDragToMark(
        startLocal: const ui.Offset(50, 50),
        endLocal: const ui.Offset(50, 50),
        box: const ui.Size(100, 100),
        frame: _frame(),
        clientOrigin: ui.Offset.zero,
      );
      expect(mark, isNull);
    });

    test('a drag shorter than the minimum is a click and is ignored', () {
      final mark = frameDragToMark(
        startLocal: const ui.Offset(50, 50),
        endLocal: const ui.Offset(52, 51),
        box: const ui.Size(100, 100),
        frame: _frame(),
        clientOrigin: ui.Offset.zero,
      );
      expect(mark, isNull);
    });

    test('an inverted drag normalises to the same rect', () {
      final forward = frameDragToMark(
        startLocal: const ui.Offset(20, 35),
        endLocal: const ui.Offset(70, 65),
        box: const ui.Size(100, 100),
        frame: _frame(),
        clientOrigin: ui.Offset.zero,
      );
      final inverted = frameDragToMark(
        startLocal: const ui.Offset(70, 65),
        endLocal: const ui.Offset(20, 35),
        box: const ui.Size(100, 100),
        frame: _frame(),
        clientOrigin: ui.Offset.zero,
      );
      expect(
        [inverted!.x, inverted.y, inverted.w, inverted.h],
        [forward!.x, forward.y, forward.w, forward.h],
      );
    });

    test('a drag past the picture edge clamps to the edge', () {
      final mark = frameDragToMark(
        startLocal: const ui.Offset(50, 50),
        endLocal: const ui.Offset(130, 130),
        box: const ui.Size(100, 100),
        frame: _frame(),
        clientOrigin: ui.Offset.zero,
      );
      // Both ends clamp to the picture: right/bottom edge is pixel 199/99.
      expect(mark, isNotNull);
      expect(mark!.x, 100);
      expect(mark.y, 50);
      expect(mark.w, 100);
      expect(mark.h, 50);
    });

    test('a drag starting in the letterbox clamps the start to the edge', () {
      final mark = frameDragToMark(
        startLocal: const ui.Offset(50, 5),
        endLocal: const ui.Offset(60, 40),
        box: const ui.Size(100, 100),
        frame: _frame(),
        clientOrigin: ui.Offset.zero,
      );
      expect(mark, isNotNull);
      expect(mark!.y, 0);
      expect(mark.h, 31);
    });
  });
}

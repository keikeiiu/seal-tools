/// The calibration session draft: the pending marks in draft order, replace-by-(kind, target),
/// remove/clear/reset, and the frame payload every session save starts from.
///
/// Pure Dart — no widgets, no pumps, no timers: the session is intended to be a plain object the
/// surfaces own and the tests can drive directly.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/calibration_result.dart';
import 'package:seal_shell/app/calibration_session.dart';
import 'package:seal_shell/protocol/protocol.dart' as wire;

wire.DisplayInfo _display() => wire.DisplayInfo(
  scale: 1.5,
  monitorDpi: 144,
  screen: wire.Size(w: 3840, h: 2160),
  frame: wire.Rect(x: 0, y: 0, w: 2889, h: 1836),
  physicalClient: wire.Rect(x: 12, y: 47, w: 2865, h: 1789),
  logicalClient: wire.Rect(x: 8, y: 31, w: 1910, h: 1193),
);

CalibrationSessionMark _rectMark(
  String kind, {
  int x = 10,
  int y = 20,
  String? target,
  String label = '',
}) => CalibrationSessionMark(
  kind: kind,
  shape: CalibrationMark.rect(x, y, 30, 40),
  target: target,
  label: label,
);

CalibrationSessionMark _pointMark(
  String kind, {
  int x = 5,
  int y = 6,
  String? target,
  String label = '',
}) => CalibrationSessionMark(
  kind: kind,
  shape: CalibrationMark.point(x, y),
  target: target,
  label: label,
);

void main() {
  test('the session starts empty and marks keep their draft order', () {
    final session = CalibrationSession();
    expect(session.hasFrame, isFalse);
    expect(session.frameId, isNull);
    expect(session.display, isNull);
    expect(session.marks, isEmpty);

    session.add(_rectMark('bag_grid', label: 'Bag grid'));
    session.add(_pointMark('scroll_point', label: 'Focus / scroll point'));
    session.add(_rectMark('pet_toggle', target: '2', label: 'Row 2 start/end'));

    expect(
      [for (final mark in session.marks) mark.kind],
      ['bag_grid', 'scroll_point', 'pet_toggle'],
      reason: 'the surfaces draw the draft in the order it was built',
    );
    final pointKinds = [for (final mark in session.pointMarks) mark.kind];
    expect(pointKinds, ['scroll_point']);
    expect(
      [for (final mark in session.rectMarks) mark.kind],
      ['bag_grid', 'pet_toggle'],
    );
    expect(session.marks[0].shape.values, [10, 20, 30, 40]);
    expect(session.marks[1].isPoint, isTrue);
    expect(session.marks[2].target, '2');
  });

  test('replace keeps the draft position and matches on kind AND target', () {
    final session = CalibrationSession()
      ..add(_rectMark('bag_grid', x: 1, label: 'Bag grid'))
      ..add(_rectMark('pet_toggle', target: '2', label: 'Row 2 start/end'))
      ..add(_rectMark('pet_toggle', target: '3', label: 'Row 3 start/end'));

    session.replace(_rectMark('bag_grid', x: 99, label: 'Bag grid'));

    expect(session.marks, hasLength(3));
    expect(session.marks[0].shape.values, [
      99,
      20,
      30,
      40,
    ], reason: 'a re-draw replaces in place, not at the end');

    // Only the SAME target is the same mark: row 3 is untouched by a row 2 re-draw.
    session.replace(_rectMark('pet_toggle', x: 7, target: '2'));
    expect(session.marks[1].shape.values, [7, 20, 30, 40]);
    expect(session.marks[2].target, '3');
    expect(session.marks[2].shape.values, [10, 20, 30, 40]);

    // A kind the draft does not have yet is appended.
    session.replace(_pointMark('scroll_point'));
    expect(session.marks, hasLength(4));
    expect(session.marks.last.kind, 'scroll_point');
  });

  test('remove names the slot; clear drops marks but keeps the frame', () {
    final session = CalibrationSession()
      ..setFrame('f1', _display())
      ..add(_rectMark('bag_grid'))
      ..add(_rectMark('pet_toggle', target: '2'))
      ..add(_rectMark('pet_toggle', target: '3'));

    expect(session.remove('bag_grid'), isTrue);
    expect(
      [for (final mark in session.marks) mark.kind],
      ['pet_toggle', 'pet_toggle'],
    );
    expect(
      session.remove('pet_toggle', target: '2'),
      isTrue,
      reason: 'a targeted kind is removed by its target',
    );
    expect(session.marks.single.target, '3');
    expect(session.remove('bag_grid'), isFalse);
    expect(session.remove('pet_toggle', target: '9'), isFalse);

    session.clear();
    expect(session.marks, isEmpty);
    expect(session.hasFrame, isTrue, reason: 'Reset keeps the captured frame');

    session.reset();
    expect(session.hasFrame, isFalse);
    expect(session.frameId, isNull);
    expect(session.display, isNull);
  });

  test('a re-capture replaces the frame and keeps the drawn marks', () {
    final session = CalibrationSession()..setFrame('f1', _display());
    session.add(_rectMark('bag_grid'));

    session.setFrame('f2', _display());

    expect(session.frameId, 'f2');
    expect(
      session.marks,
      hasLength(1),
      reason: 'marks are client-relative coordinates, not frame pixels',
    );
  });

  test('framePayload carries the frame id and the measured environment', () {
    final session = CalibrationSession()..setFrame('f1', _display());

    expect(session.hasFrame, isTrue);
    expect(session.framePayload(), {
      'frame_id': 'f1',
      'display': {
        'dpi_scale': 1.5,
        'monitor_dpi': 144,
        'screen': {'w': 3840, 'h': 2160},
      },
    });

    // No capture yet is a programming error, not a silent empty save.
    expect(
      () => CalibrationSession().framePayload(),
      throwsA(isA<StateError>()),
    );
  });
}

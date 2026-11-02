/// The window-placement geometry: the logical [WindowRect] the runner and the shell share, its
/// channel decoding, and the clamp that keeps a remembered placement on a monitor that is gone.
///
/// Pure Dart: no model, no host, no timers.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/host_actions.dart';

void main() {
  const area = WindowRect(left: 0, top: 0, width: 1920, height: 1040);

  test(
    'a rect decodes only a four-number channel map, rounding codec doubles',
    () {
      expect(
        WindowRect.fromChannel({
          'left': 10,
          'top': 20,
          'width': 800,
          'height': 600,
        }),
        const WindowRect(left: 10, top: 20, width: 800, height: 600),
      );
      expect(
        WindowRect.fromChannel({
          'left': 10.4,
          'top': 20.6,
          'width': 800.0,
          'height': 600.0,
        }),
        const WindowRect(left: 10, top: 21, width: 800, height: 600),
        reason: 'the codec may recover a whole pixel as a double',
      );
      expect(
        WindowRect.fromChannel({'left': 10, 'top': 20, 'width': 800}),
        isNull,
      );
      expect(
        WindowRect.fromChannel({
          'left': 'ten',
          'top': 20,
          'width': 800,
          'height': 600,
        }),
        isNull,
      );
      expect(WindowRect.fromChannel(null), isNull);
    },
  );

  test('the channel map round-trips', () {
    const rect = WindowRect(left: -50, top: 30, width: 1280, height: 720);
    expect(rect.toJson(), {
      'left': -50,
      'top': 30,
      'width': 1280,
      'height': 720,
    });
    expect(WindowRect.fromChannel(rect.toJson()), rect);
  });

  test('a rect already inside the work area is unchanged', () {
    const rect = WindowRect(left: 100, top: 50, width: 1200, height: 800);
    expect(rect.clampedTo(area), rect);
  });

  test(
    'a rect on a monitor that is gone is pulled back into the work area',
    () {
      const gone = WindowRect(left: 5000, top: -900, width: 4000, height: 3000);
      expect(
        gone.clampedTo(area),
        area,
        reason:
            'an entirely off-screen placement opens at the work area instead',
      );

      const partly = WindowRect(left: 1800, top: 900, width: 800, height: 600);
      expect(
        partly.clampedTo(area),
        const WindowRect(left: 1120, top: 440, width: 800, height: 600),
        reason: 'it keeps its size and slides fully inside',
      );
    },
  );

  test('a degenerate saved size falls back to two thirds of the work area', () {
    const tiny = WindowRect(left: 10, top: 10, width: 0, height: -5);
    expect(
      tiny.clampedTo(area),
      const WindowRect(left: 10, top: 10, width: 1280, height: 693),
      reason: 'a corrupt block must not restore an invisible window',
    );
    // A host that reports no work area at all is not a clamp source; the rect is left alone.
    expect(
      tiny.clampedTo(const WindowRect(left: 0, top: 0, width: 0, height: 0)),
      tiny,
    );
  });

  test('value equality is by the four fields', () {
    const one = WindowRect(left: 1, top: 2, width: 3, height: 4);
    const same = WindowRect(left: 1, top: 2, width: 3, height: 4);
    const other = WindowRect(left: 1, top: 2, width: 3, height: 5);
    expect(one, same);
    expect(one.hashCode, same.hashCode);
    expect(one, isNot(other));
    expect(one.toString(), 'WindowRect(1, 2, 3x4)');
  });

  test('encloses only a rect entirely inside the bounds', () {
    const screen = WindowRect(left: 0, top: 0, width: 1920, height: 1040);
    const inside = WindowRect(left: 100, top: 100, width: 800, height: 600);
    const pastRight = WindowRect(left: 1500, top: 100, width: 800, height: 600);
    const pastLeft = WindowRect(left: -10, top: 100, width: 800, height: 600);
    expect(screen.encloses(inside), isTrue);
    expect(
      screen.encloses(pastRight),
      isFalse,
      reason: 'partly off the record is not a placement that fits',
    );
    expect(screen.encloses(pastLeft), isFalse);
  });
}

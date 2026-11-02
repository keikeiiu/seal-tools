/// The shell's tiny host-window seam: the capture lifecycle's two window operations, the persisted
/// always-on-top preference, and the remembered window placement.
///
/// The runner owns the HWND and serves `seal/host`, so the Dart side contains no Win32 and no window
/// APIs; tests inject a fake. The capture flow is hide → one frame → `capture` → show, and the show
/// is unconditional (a refused capture must not leave the shell hidden). Always-on-top is applied
/// once on load and on toggle only — it never runs per frame and does not disturb hide/show.
///
/// **Coordinates are LOGICAL, everywhere.** The shell is DPI-unaware by contract
/// ([PLAN-V3-SHELL.md] §9): [WindowRect] values are the DIP-like units v2's `ui` block stores
/// (`left`/`top`/`width`/`expanded_height`), and the runner is the only place that multiplies or
/// divides them by the monitor's scale factor. That keeps a remembered placement round-tripping
/// through `local.yaml` without a conversion in Dart, and keeps the DPI conversion (and the rounding
/// that must come with it) in the one process that already does it for the window's creation.
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// A rectangle in the shell's LOGICAL screen space.
///
/// The runner speaks the same shape on `seal/host` and converts to physical pixels; see the library
/// doc. Used for the window's own rect, the primary monitor's work area and every restored placement.
@immutable
class WindowRect {
  const WindowRect({
    required this.left,
    required this.top,
    required this.width,
    required this.height,
  });

  final int left;
  final int top;
  final int width;
  final int height;

  /// The channel form: the runner's `setWindowRect` argument and the shape of every rect reply.
  Map<String, int> toJson() => {
    'left': left,
    'top': top,
    'width': width,
    'height': height,
  };

  /// Decodes a rect map from the runner; null when it is not a four-number rect.
  static WindowRect? fromChannel(Object? value) {
    if (value is! Map) {
      return null;
    }
    final left = _whole(value['left']);
    final top = _whole(value['top']);
    final width = _whole(value['width']);
    final height = _whole(value['height']);
    if (left == null || top == null || width == null || height == null) {
      return null;
    }
    return WindowRect(left: left, top: top, width: width, height: height);
  }

  /// This rect clamped into [area], the primary monitor's work area.
  ///
  /// A placement saved on a monitor that is now gone must not open the window off-screen: the size
  /// shrinks to the area at most, and the position slides back inside it — for a rect entirely
  /// outside, that is the work area's nearest corner. A degenerate (zero/negative) saved size falls
  /// back to two thirds of the work area, so a corrupt block cannot restore an invisible window.
  WindowRect clampedTo(WindowRect area) {
    if (area.width <= 0 || area.height <= 0) {
      return this;
    }
    var width = this.width > 0 ? this.width : area.width * 2 ~/ 3;
    var height = this.height > 0 ? this.height : area.height * 2 ~/ 3;
    if (width > area.width) {
      width = area.width;
    }
    if (height > area.height) {
      height = area.height;
    }
    return WindowRect(
      left: _clamp(left, area.left, area.left + area.width - width),
      top: _clamp(top, area.top, area.top + area.height - height),
      width: width,
      height: height,
    );
  }

  /// True when [other] lies entirely inside this rect — e.g. a remembered placement still fits on
  /// the monitors that exist.
  bool encloses(WindowRect other) =>
      other.left >= left &&
      other.top >= top &&
      other.left + other.width <= left + width &&
      other.top + other.height <= top + height;

  @override
  bool operator ==(Object other) =>
      other is WindowRect &&
      other.left == left &&
      other.top == top &&
      other.width == width &&
      other.height == height;

  @override
  int get hashCode => Object.hash(left, top, width, height);

  @override
  String toString() => 'WindowRect($left, $top, ${width}x$height)';
}

int? _whole(Object? value) {
  if (value is! num || !value.isFinite) {
    return null;
  }
  return value.round();
}

int _clamp(int value, int lower, int upper) {
  if (value < lower) {
    return lower;
  }
  if (value > upper) {
    return upper;
  }
  return value;
}

/// Window operations the shell needs from its runner host.
abstract class HostActions {
  /// Hide the shell window and complete only once the hide has been applied.
  Future<void> hideWindow();

  /// Show the shell window again (restored, normal state).
  Future<void> showWindow();

  /// Put the shell window above (or back below) other windows.
  Future<void> setAlwaysOnTop(bool enabled);

  /// Lower/restore the user-resizable minimum window size, in logical pixels. The mini running card
  /// needs a small window; hosts that do not implement it keep the full-shell minimum (a no-op).
  Future<void> setWindowMinSize(int width, int height) async {}

  /// Borderless, non-activating chrome for the mini running card (`false` restores the shell).
  /// A no-op on hosts that cannot change the window style.
  Future<void> setMiniChrome(bool mini) async {}

  /// The shell window's current rect in logical screen coordinates.
  Future<WindowRect> windowRect();

  /// Moves/resizes the shell window to [rect] (logical coordinates); the runner converts to physical
  /// pixels and applies it without activating the window.
  Future<void> setWindowRect(WindowRect rect);

  /// The primary monitor's work area in logical coordinates — the region a restored placement is
  /// clamped into.
  Future<WindowRect> primaryWorkArea();

  /// The bounding rect of every monitor in logical coordinates. A remembered placement that does
  /// not fit entirely inside it was saved on a monitor that is gone, so it must be clamped.
  Future<WindowRect> virtualScreen();

  /// True while the window is visible; false during the capture's hide, when a placement must not be
  /// remembered.
  Future<bool> isWindowVisible();

  /// Rect changes the runner reports, in logical coordinates: one per settled user move/resize (the
  /// runner sends on `WM_EXITSIZEMOVE`), plus maximize/restore. Silent while the window is hidden.
  Stream<WindowRect> get windowMoves;
}

/// The production implementation: `seal/host`, served by `windows/runner/flutter_window.cpp`.
///
/// Deliberately `const`: it is the default host in surfaces that never ask for window moves (the
/// calibrate tab) and in tests, and a `const` constructor must not need a Flutter binding. The
/// runner→Dart listener is therefore installed LAZILY, the first time [windowMoves] is read.
class MethodChannelHostActions implements HostActions {
  const MethodChannelHostActions();

  static const MethodChannel _channel = MethodChannel('seal/host');

  /// The process's one move stream; the shell's model is the only listener and cancels its
  /// subscription when it is disposed. Static because a `const` instance cannot own mutable state.
  static final StreamController<WindowRect> _moves =
      StreamController<WindowRect>.broadcast();

  static bool _listenerInstalled = false;

  @override
  Stream<WindowRect> get windowMoves {
    if (!_listenerInstalled) {
      _listenerInstalled = true;
      _channel.setMethodCallHandler(_onHostCall);
    }
    return _moves.stream;
  }

  static Future<dynamic> _onHostCall(MethodCall call) async {
    if (call.method != 'windowMoved') {
      return null;
    }
    final rect = WindowRect.fromChannel(call.arguments);
    if (rect != null && _moves.hasListener) {
      _moves.add(rect);
    }
    return null;
  }

  @override
  Future<void> hideWindow() => _channel.invokeMethod<void>('hideWindow');

  @override
  Future<void> showWindow() => _channel.invokeMethod<void>('showWindow');

  @override
  Future<void> setAlwaysOnTop(bool enabled) =>
      _channel.invokeMethod<void>('setAlwaysOnTop', enabled);

  @override
  Future<void> setWindowMinSize(int width, int height) =>
      _channel.invokeMethod<void>('setWindowMinSize', {
        'width': width,
        'height': height,
      });

  @override
  Future<void> setMiniChrome(bool mini) =>
      _channel.invokeMethod<void>('setMiniChrome', mini);

  @override
  Future<WindowRect> windowRect() async {
    final rect = WindowRect.fromChannel(
      await _channel.invokeMethod<Object?>('windowRect'),
    );
    if (rect == null) {
      throw StateError('the host returned no window rect');
    }
    return rect;
  }

  @override
  Future<WindowRect> primaryWorkArea() async {
    final rect = WindowRect.fromChannel(
      await _channel.invokeMethod<Object?>('primaryWorkArea'),
    );
    if (rect == null) {
      throw StateError('the host returned no primary work area');
    }
    return rect;
  }

  @override
  Future<WindowRect> virtualScreen() async {
    final rect = WindowRect.fromChannel(
      await _channel.invokeMethod<Object?>('virtualScreen'),
    );
    if (rect == null) {
      throw StateError('the host returned no virtual screen');
    }
    return rect;
  }

  @override
  Future<void> setWindowRect(WindowRect rect) =>
      _channel.invokeMethod<void>('setWindowRect', rect.toJson());

  /// Unknown (null) is treated as visible: skipping a save wrongly is worse than one extra write.
  @override
  Future<bool> isWindowVisible() async =>
      await _channel.invokeMethod<bool>('isWindowVisible') ?? true;
}

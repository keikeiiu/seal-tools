/// A recording [HostActions] for tests: the capture lifecycle's hide/show calls, the always-on-top
/// setting, and the window-placement seam (a settable rect plus a controllable move stream).
library;

import 'dart:async';

import 'package:seal_shell/app/host_actions.dart';

/// The default primary/virtual work area a fake host reports (a 1920×1040 logical desktop).
const WindowRect _defaultArea = WindowRect(
  left: 0,
  top: 0,
  width: 1920,
  height: 1040,
);

/// The default current window rect.
const WindowRect _defaultWindow = WindowRect(
  left: 10,
  top: 10,
  width: 1280,
  height: 720,
);

class FakeHost implements HostActions {
  FakeHost({
    this.failAlwaysOnTop = false,
    WindowRect? primaryWorkArea,
    WindowRect? virtualScreen,
    WindowRect? windowRect,
    this.visible = true,
    this.failWindowCalls = false,
  }) : primaryWorkAreaValue = primaryWorkArea ?? _defaultArea,
       virtualScreenValue = virtualScreen ?? primaryWorkArea ?? _defaultArea,
       currentWindowRect = windowRect ?? _defaultWindow;

  /// Ordered `setAlwaysOnTop` calls: the connect-time apply first, then each toggle (and a revert
  /// after a refused save).
  final List<bool> alwaysOnTopCalls = <bool>[];

  int hideCalls = 0;
  int showCalls = 0;

  /// When true `setAlwaysOnTop` throws, exercising the host-failure path.
  bool failAlwaysOnTop;

  /// When true the placement calls throw, exercising the host-failure path.
  bool failWindowCalls;

  /// The primary monitor's work area `primaryWorkArea()` reports.
  WindowRect primaryWorkAreaValue;

  /// Every monitor's bounding rect `virtualScreen()` reports.
  WindowRect virtualScreenValue;

  /// The window's CURRENT rect `windowRect()` reports; [emitMove] and `setWindowRect` keep it true.
  WindowRect currentWindowRect;

  /// What `isWindowVisible()` reports; false simulates the capture's hide.
  bool visible;

  /// Every rect the shell applied through `setWindowRect`, in order.
  final List<WindowRect> appliedRects = <WindowRect>[];

  /// How many times `windowRect()` was asked (the restore's baseline read).
  int windowRectQueries = 0;

  final StreamController<WindowRect> _moves =
      StreamController<WindowRect>.broadcast();

  @override
  Stream<WindowRect> get windowMoves => _moves.stream;

  /// Simulates the runner reporting a settled move/resize, making it the current rect.
  void emitMove(WindowRect rect) {
    currentWindowRect = rect;
    _moves.add(rect);
  }

  /// Closes the move stream; the harness does this on teardown.
  Future<void> closeMoves() => _moves.close();

  @override
  Future<void> hideWindow() async {
    hideCalls++;
    visible = false;
  }

  @override
  Future<void> showWindow() async {
    showCalls++;
    visible = true;
  }

  @override
  Future<void> setAlwaysOnTop(bool enabled) async {
    if (failAlwaysOnTop) {
      throw StateError('the host refused setAlwaysOnTop');
    }
    alwaysOnTopCalls.add(enabled);
  }

  @override
  Future<WindowRect> windowRect() async {
    if (failWindowCalls) {
      throw StateError('the host refused windowRect');
    }
    windowRectQueries++;
    return currentWindowRect;
  }

  @override
  Future<void> setWindowRect(WindowRect rect) async {
    if (failWindowCalls) {
      throw StateError('the host refused setWindowRect');
    }
    appliedRects.add(rect);
    currentWindowRect = rect;
  }

  @override
  Future<WindowRect> primaryWorkArea() async {
    if (failWindowCalls) {
      throw StateError('the host refused primaryWorkArea');
    }
    return primaryWorkAreaValue;
  }

  @override
  Future<WindowRect> virtualScreen() async {
    if (failWindowCalls) {
      throw StateError('the host refused virtualScreen');
    }
    return virtualScreenValue;
  }

  @override
  Future<bool> isWindowVisible() async => visible;
}

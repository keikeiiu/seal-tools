/// The shell's tiny host-window seam: the capture lifecycle's two window operations plus the
/// persisted always-on-top preference.
///
/// The runner owns the HWND and serves `seal/host` (`hideWindow`/`showWindow`/`setAlwaysOnTop`), so
/// the Dart side contains no Win32 and no window APIs; tests inject a fake. The capture flow is
/// hide → one frame → `capture` → show, and the show is unconditional (a refused capture must not
/// leave the shell hidden). Always-on-top is applied once on load and on toggle only — it never runs
/// per frame and does not disturb hide/show.
library;

import 'package:flutter/services.dart';

/// Window operations the shell needs from its runner host.
abstract class HostActions {
  /// Hide the shell window and complete only once the hide has been applied.
  Future<void> hideWindow();

  /// Show the shell window again (restored, normal state).
  Future<void> showWindow();

  /// Put the shell window above (or back below) other windows.
  Future<void> setAlwaysOnTop(bool enabled);
}

/// The production implementation: `seal/host`, served by `windows/runner/flutter_window.cpp`.
class MethodChannelHostActions implements HostActions {
  const MethodChannelHostActions();

  static const MethodChannel _channel = MethodChannel('seal/host');

  @override
  Future<void> hideWindow() => _channel.invokeMethod<void>('hideWindow');

  @override
  Future<void> showWindow() => _channel.invokeMethod<void>('showWindow');

  @override
  Future<void> setAlwaysOnTop(bool enabled) =>
      _channel.invokeMethod<void>('setAlwaysOnTop', enabled);
}

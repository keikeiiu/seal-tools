/// The shell's tiny host-window seam: the two window operations the capture lifecycle needs.
///
/// The runner owns the HWND and serves `seal/host` (`hideWindow`/`showWindow`), so the Dart side
/// contains no Win32 and no window APIs; tests inject a fake. The capture flow is
/// hide → one frame → `capture` → show, and the show is unconditional (a refused capture must not
/// leave the shell hidden).
library;

import 'package:flutter/services.dart';

/// Window operations the shell needs from its runner host.
abstract class HostActions {
  /// Hide the shell window and complete only once the hide has been applied.
  Future<void> hideWindow();

  /// Show the shell window again (restored, normal state).
  Future<void> showWindow();
}

/// The production implementation: `seal/host`, served by `windows/runner/flutter_window.cpp`.
class MethodChannelHostActions implements HostActions {
  const MethodChannelHostActions();

  static const MethodChannel _channel = MethodChannel('seal/host');

  @override
  Future<void> hideWindow() => _channel.invokeMethod<void>('hideWindow');

  @override
  Future<void> showWindow() => _channel.invokeMethod<void>('showWindow');
}

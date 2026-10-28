/// A recording [HostActions] for tests: the capture lifecycle's hide/show calls and the
/// always-on-top setting the model sends through `seal/host`.
library;

import 'package:seal_shell/app/host_actions.dart';

class FakeHost implements HostActions {
  FakeHost({this.failAlwaysOnTop = false});

  /// Ordered `setAlwaysOnTop` calls: the connect-time apply first, then each toggle (and a revert
  /// after a refused save).
  final List<bool> alwaysOnTopCalls = <bool>[];

  int hideCalls = 0;
  int showCalls = 0;

  /// When true `setAlwaysOnTop` throws, exercising the host-failure path.
  bool failAlwaysOnTop;

  @override
  Future<void> hideWindow() async {
    hideCalls++;
  }

  @override
  Future<void> showWindow() async {
    showCalls++;
  }

  @override
  Future<void> setAlwaysOnTop(bool enabled) async {
    if (failAlwaysOnTop) {
      throw StateError('the host refused setAlwaysOnTop');
    }
    alwaysOnTopCalls.add(enabled);
  }
}

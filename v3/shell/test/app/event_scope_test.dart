/// SHELL-F5: log lines and run events are scoped to the widgets that render them.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/app.dart';
import 'package:seal_shell/protocol/protocol.dart';

void main() {
  test('a log line does not notify the root shell model', () {
    final model = ShellModel(
      transportFactory: () async => throw StateError('unused'),
    );
    addTearDown(model.dispose);
    var notifications = 0;
    model.addListener(() => notifications++);

    model.logs.addStderr('seal-core: a line');
    model.logs.addShellNote('a shell note');

    expect(
      notifications,
      0,
      reason: 'the log drawer/status widgets listen to model.logs directly',
    );
  });

  test('a tool status change does not notify the root model unless the mode changes', () {
    final model = ShellModel(
      transportFactory: () async => throw StateError('unused'),
    );
    addTearDown(model.dispose);
    var notifications = 0;
    model.addListener(() => notifications++);

    // Not ready: `_syncMiniMode` cannot transition, so a snapshot update stays scoped.
    model.tools.beginCommand(ToolId.tuner);

    expect(notifications, 0);
  });
}

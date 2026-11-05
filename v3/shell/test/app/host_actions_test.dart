/// The one hide -> frame -> command -> show lifecycle (SHELL-F8).
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/host_actions.dart';

import '../support/fake_host.dart';

void main() {
  testWidgets('the window is shown again when the body throws', (tester) async {
    final host = FakeHost();

    await tester.runAsync(() async {
      await expectLater(
        withHiddenWindow<int>(
          host,
          () async => throw StateError('decoder bug'),
        ),
        throwsStateError,
      );
    });

    expect(host.lifecycle, ['hide', 'show']);
    expect(
      host.visible,
      isTrue,
      reason: 'a throw must not leave the shell hidden',
    );
  });

  testWidgets('a hide failure is reported but the command still runs', (
    tester,
  ) async {
    final host = FakeHost(failHide: true);
    String? note;

    final result = await tester.runAsync(
      () => withHiddenWindow(
        host,
        () async => 42,
        onHostError: (line) => note = line,
      ),
    );

    expect(result, 42);
    expect(host.lifecycle, ['hide', 'show']);
    expect(note, contains('could not hide itself'));
  });

  testWidgets('a show failure is reported after the command result', (
    tester,
  ) async {
    final host = FakeHost(failShow: true);
    String? note;

    final result = await tester.runAsync(
      () => withHiddenWindow(
        host,
        () async => 'answer',
        onHostError: (line) => note = line,
      ),
    );

    expect(result, 'answer');
    expect(note, contains('could not be shown again'));
  });
}

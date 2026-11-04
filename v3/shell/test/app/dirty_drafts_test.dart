/// SHELL-F6: the shell's aggregate dirty-draft flag, which the exit guard reads before shutdown.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/app.dart';

void main() {
  test('dirty tab groups aggregate into hasDirtyDrafts', () {
    final model = ShellModel(
      transportFactory: () async => throw StateError('unused'),
    );
    addTearDown(model.dispose);
    final group = ConfigDraftGroup(
      label: 'setup',
      parts: [
        ConfigDraft(
          blockName: 'ui',
          scope: ConfigScope.ownedBlock,
          editableKeys: const {'left'},
        ),
      ],
    );
    addTearDown(group.dispose);

    expect(model.hasDirtyDrafts, isFalse);
    model.setDraftDirty(group, true);
    expect(model.hasDirtyDrafts, isTrue);
    model.setDraftDirty(group, false);
    expect(model.hasDirtyDrafts, isFalse);
  });
}

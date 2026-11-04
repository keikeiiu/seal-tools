/// The app root's appearance wiring: one model, one MaterialApp, and the model's theme choice becoming
/// Flutter's `ThemeMode`. The window/title-bar side lives in the shell model tests (FakeHost records
/// the native request); this file pins the widget-tree side.
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/app.dart';
import 'package:seal_shell/ui/shell_app.dart';
import 'package:seal_shell/ui/theme.dart';

import '../support/app_harness.dart';

void main() {
  testWidgets('ShellApp maps the model theme choice to ThemeMode', (
    tester,
  ) async {
    final harness = AppHarness();
    addTearDown(harness.dispose);

    await tester.pumpWidget(ShellApp(model: harness.model));
    await tester.pump();

    MaterialApp app() => tester.widget<MaterialApp>(find.byType(MaterialApp));

    expect(app().themeMode, ThemeMode.dark);
    expect(app().darkTheme!.brightness, Brightness.dark);

    await harness.model.setThemeMode(ShellThemeMode.light);
    await tester.pump();
    expect(app().themeMode, ThemeMode.light);
    expect(app().theme!.brightness, Brightness.light);

    await harness.model.setThemeMode(ShellThemeMode.system);
    await tester.pump();
    expect(app().themeMode, ThemeMode.system);

    // Both themes carry their matching semantic extension, so StatusChip stays readable.
    expect(
      app().theme!.extension<ShellColors>()?.success,
      ShellColors.light.success,
    );
    expect(
      app().darkTheme!.extension<ShellColors>()?.success,
      ShellColors.dark.success,
    );
  });
}

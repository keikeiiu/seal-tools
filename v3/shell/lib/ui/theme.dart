/// The shell's visual tokens. Dart-side density only — the window's chrome (Mica etc.) is the
/// runner's C++ ([PLAN-V3-SHELL.md] §7).
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'package:flutter/material.dart';

/// Spacing steps; using names keeps the cards visually aligned with one decision per distance.
abstract final class ShellSpace {
  static const double xs = 4;
  static const double sm = 8;
  static const double md = 12;
  static const double lg = 16;
  static const double xl = 24;
}

/// The app theme. Dark by default because the tool is used beside a full-screen game at night; the
/// density is compact because a card carries a message, up to a few rows and two buttons.
ThemeData buildShellTheme() {
  final scheme = ColorScheme.fromSeed(
    seedColor: const Color(0xFF3B82F6),
    brightness: Brightness.dark,
  );
  return ThemeData(
    colorScheme: scheme,
    visualDensity: VisualDensity.compact,
    scaffoldBackgroundColor: scheme.surface,
  );
}

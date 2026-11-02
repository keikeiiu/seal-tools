/// The shell theme: M3 dark from the blue seed, plus the token layer the redesign needs
/// ([PLAN-V3-UI-UX.md] §4): `ShellColors` semantics, a declared type scale, card/input defaults.
///
/// Density stays compact (the tool is used beside a full-screen game) and colours still come from
/// `ColorScheme.fromSeed(0xFF3B82F6, dark)` — the palette was not reworked. This file also re-exports
/// [tokens.dart] so a widget that already imports the theme gets `ShellWidth`/`ShellRadius` without a
/// second import; the call sites still name a token, never a raw value.
///
/// [PLAN-V3-UI-UX.md]: ../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

import 'tokens.dart';

export 'tokens.dart';

/// Spacing steps; using names keeps the cards visually aligned with one decision per distance.
abstract final class ShellSpace {
  static const double xxs = 2;
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
  final base = ThemeData(
    colorScheme: scheme,
    visualDensity: VisualDensity.compact,
    fontFamily: 'Roboto',
    scaffoldBackgroundColor: scheme.surface,
    materialTapTargetSize: MaterialTapTargetSize.padded,
  );
  final textTheme = base.textTheme.copyWith(
    headlineSmall: base.textTheme.headlineSmall?.copyWith(
      fontSize: 22,
      height: 1.25,
      fontWeight: FontWeight.w600,
    ),
    titleLarge: base.textTheme.titleLarge?.copyWith(
      fontSize: 18,
      height: 1.3,
      fontWeight: FontWeight.w600,
    ),
    titleMedium: base.textTheme.titleMedium?.copyWith(
      fontSize: 16,
      height: 1.3,
      fontWeight: FontWeight.w600,
    ),
    titleSmall: base.textTheme.titleSmall?.copyWith(
      fontSize: 14,
      height: 1.35,
      fontWeight: FontWeight.w600,
    ),
    labelLarge: base.textTheme.labelLarge?.copyWith(
      fontSize: 13,
      height: 1.3,
      fontWeight: FontWeight.w600,
    ),
    bodyLarge: base.textTheme.bodyLarge?.copyWith(fontSize: 16, height: 1.5),
    bodyMedium: base.textTheme.bodyMedium?.copyWith(fontSize: 14, height: 1.5),
    bodySmall: base.textTheme.bodySmall?.copyWith(fontSize: 12.5, height: 1.5),
    labelSmall: base.textTheme.labelSmall?.copyWith(
      fontSize: 11,
      height: 1.3,
      fontWeight: FontWeight.w600,
    ),
  );

  final outline = OutlineInputBorder(
    borderRadius: BorderRadius.circular(ShellRadius.s),
    borderSide: BorderSide(color: scheme.outlineVariant),
  );

  return base.copyWith(
    textTheme: textTheme,
    extensions: const <ThemeExtension<dynamic>>[ShellColors.dark],
    cardTheme: CardThemeData(
      margin: EdgeInsets.zero,
      elevation: 0,
      color: scheme.surfaceContainerHigh,
      surfaceTintColor: Colors.transparent,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(ShellRadius.l),
        // The demo's cards separate from the page with a hairline, not a shadow: on a dark UI a
        // border reads as structure while a shadow reads as haze.
        side: BorderSide(color: scheme.outlineVariant.withValues(alpha: 0.5)),
      ),
    ),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: scheme.surfaceContainerLow,
      isDense: true,
      contentPadding: const EdgeInsets.symmetric(
        horizontal: ShellSpace.md,
        vertical: ShellSpace.sm + ShellSpace.xxs,
      ),
      border: outline,
      enabledBorder: outline,
      focusedBorder: outline.copyWith(
        borderSide: BorderSide(color: scheme.primary, width: 1.5),
      ),
      errorBorder: outline.copyWith(
        borderSide: BorderSide(color: scheme.error),
      ),
      focusedErrorBorder: outline.copyWith(
        borderSide: BorderSide(color: scheme.error, width: 1.5),
      ),
    ),
    dividerTheme: DividerThemeData(
      color: scheme.outlineVariant,
      space: 1,
      thickness: 1,
    ),
  );
}

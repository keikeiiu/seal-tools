/// Named design tokens for the shell ([PLAN-V3-UI-UX.md] §4): the semantic colours M3 does not
/// name, the width/radius/size steps the audits found re-derived at call sites, and the mono style.
///
/// Rule: a widget asks for a token, never a raw number or colour. Values are the plan's (§4 and the
/// live `ColorScheme` dump in `v3/.scratch/ui-audit/theme-dump.txt`); changing the look changes this
/// file, not call sites.
///
/// [PLAN-V3-UI-UX.md]: ../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

/// The meanings M3 does not name: success / warning / info. `blocked` stays `colorScheme.error`
/// because "blocked and requires action" is exactly M3's error role.
///
/// Colour is never the only channel: every chip pairs the colour with a dot and a word, every
/// selected bag cell gains a check, every banner an icon (PLAN-V3-UI-UX §4.1).
@immutable
class ShellColors extends ThemeExtension<ShellColors> {
  const ShellColors({
    required this.success,
    required this.onSuccess,
    required this.successContainer,
    required this.onSuccessContainer,
    required this.warning,
    required this.onWarning,
    required this.warningContainer,
    required this.onWarningContainer,
    required this.info,
  });

  /// Tuned for the dark `surface` (#111318); see the plan's table.
  static const ShellColors dark = ShellColors(
    success: Color(0xFF7EE2B8),
    onSuccess: Color(0xFF04291C),
    successContainer: Color(0xFF10382B),
    onSuccessContainer: Color(0xFFB9F5DA),
    warning: Color(0xFFF2CC7B),
    onWarning: Color(0xFF2A1E00),
    warningContainer: Color(0xFF3E2F10),
    onWarningContainer: Color(0xFFFFE2A8),
    info: Color(0xFF9ECAFF),
  );

  final Color success;
  final Color onSuccess;
  final Color successContainer;
  final Color onSuccessContainer;
  final Color warning;
  final Color onWarning;
  final Color warningContainer;
  final Color onWarningContainer;
  final Color info;

  @override
  ShellColors copyWith({
    Color? success,
    Color? onSuccess,
    Color? successContainer,
    Color? onSuccessContainer,
    Color? warning,
    Color? onWarning,
    Color? warningContainer,
    Color? onWarningContainer,
    Color? info,
  }) => ShellColors(
    success: success ?? this.success,
    onSuccess: onSuccess ?? this.onSuccess,
    successContainer: successContainer ?? this.successContainer,
    onSuccessContainer: onSuccessContainer ?? this.onSuccessContainer,
    warning: warning ?? this.warning,
    onWarning: onWarning ?? this.onWarning,
    warningContainer: warningContainer ?? this.warningContainer,
    onWarningContainer: onWarningContainer ?? this.onWarningContainer,
    info: info ?? this.info,
  );

  @override
  ShellColors lerp(ThemeExtension<ShellColors>? other, double t) {
    if (other is! ShellColors) {
      return this;
    }
    Color mix(Color a, Color b) => Color.lerp(a, b, t)!;
    return ShellColors(
      success: mix(success, other.success),
      onSuccess: mix(onSuccess, other.onSuccess),
      successContainer: mix(successContainer, other.successContainer),
      onSuccessContainer: mix(onSuccessContainer, other.onSuccessContainer),
      warning: mix(warning, other.warning),
      onWarning: mix(onWarning, other.onWarning),
      warningContainer: mix(warningContainer, other.warningContainer),
      onWarningContainer: mix(onWarningContainer, other.onWarningContainer),
      info: mix(info, other.info),
    );
  }
}

/// Widths with a meaning. A new field or panel takes one of these or the plan's next step, so the
/// 93 raw widths (22 values) the audit found cannot come back.
abstract final class ShellWidth {
  /// Longest prose measure (~60 characters at [bodyMedium] 14 px); see PLAN-V3-UI-UX §4.2.
  static const double prose = 620;
  static const double fieldSm = 140;
  static const double fieldMd = 170;
  static const double fieldLg = 220;
  static const double picker = 240;
  static const double labelColumn = 160;
  static const double rail = 216;
}

/// Radii: one for controls, one for panels, one for cards/dialogs; `chip` (999) for pills only.
abstract final class ShellRadius {
  static const double s = 6;
  static const double m = 10;
  static const double l = 14;
  static const double chip = 999;
}

/// Fixed sizes that are behaviour, not taste: the runner's minimum logical window, the idle tile
/// height, and the icon steps the chrome uses.
abstract final class ShellSize {
  static const double minWindowWidth = 1120;
  static const double minWindowHeight = 680;
  static const double idleTileMinHeight = 66;
  static const double statusStripHeight = 28;
  static const double iconSm = 14;
  static const double iconMd = 18;
  static const double iconLg = 28;
}

/// The mono role (coordinates, counts, logs). M3 has no mono slot in `TextTheme`, so it is a named
/// style the widgets copy a colour onto rather than a sixteenth ad-hoc `fontFamily` string.
abstract final class ShellText {
  static const TextStyle mono = TextStyle(
    fontFamily: 'Consolas',
    fontSize: 12.5,
    height: 1.5,
    fontFeatures: [FontFeature.tabularFigures()],
  );
}

/// Convenience for the semantic extension; the theme always installs [ShellColors.dark].
extension ShellColorsContext on BuildContext {
  ShellColors get shellColors =>
      Theme.of(this).extension<ShellColors>() ?? ShellColors.dark;
}

/// The same lookup when a builder only received a [ThemeData] (e.g. a row helper).
extension ShellColorsTheme on ThemeData {
  ShellColors get shellColors => extension<ShellColors>() ?? ShellColors.dark;
}

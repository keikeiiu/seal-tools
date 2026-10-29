/// [ShellProse] — prose, help and warnings capped at [ShellWidth.prose] (§4.2 of
/// [PLAN-V3-UI-UX.md]) and set in the body role (14 px), because the audit found descriptions
/// running the full pane at ~190–210 characters/line.
///
/// Data, tables and fields keep full width; this widget is for reading text only.
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

import '../theme.dart';

class ShellProse extends StatelessWidget {
  const ShellProse(
    this.text, {
    super.key,
    this.style,
    this.maxWidth = ShellWidth.prose,
    this.semanticLabel,
  });

  final String text;
  final TextStyle? style;
  final double maxWidth;
  final String? semanticLabel;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final resolved =
        style ??
        theme.textTheme.bodyMedium?.copyWith(
          color: theme.colorScheme.onSurfaceVariant,
        );
    return ConstrainedBox(
      constraints: BoxConstraints(maxWidth: maxWidth),
      child: Text(text, style: resolved, semanticsLabel: semanticLabel),
    );
  }
}

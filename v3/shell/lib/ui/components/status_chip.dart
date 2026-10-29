/// [StatusChip] — one semantic state as a dot + word (§5 of [PLAN-V3-UI-UX.md]).
///
/// Colour is never the only channel: the dot and the word carry the meaning too. Kinds map to
/// [ShellColors] (`success`/`warning`/`info`), `error` to the M3 error role (blocked, needs action)
/// and `muted` to "idle / not applicable".
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

import '../theme.dart';

enum StatusKind { success, warning, error, info, muted }

class StatusChip extends StatelessWidget {
  const StatusChip({super.key, required this.kind, required this.label});

  final StatusKind kind;
  final String label;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = context.shellColors;
    final (background, foreground, dot) = switch (kind) {
      StatusKind.success => (
        colors.successContainer,
        colors.onSuccessContainer,
        colors.success,
      ),
      StatusKind.warning => (
        colors.warningContainer,
        colors.onWarningContainer,
        colors.warning,
      ),
      StatusKind.error => (
        theme.colorScheme.errorContainer,
        theme.colorScheme.onErrorContainer,
        theme.colorScheme.error,
      ),
      StatusKind.info => (
        theme.colorScheme.surfaceContainerHighest,
        theme.colorScheme.onSurface,
        colors.info,
      ),
      StatusKind.muted => (
        theme.colorScheme.surfaceContainerHighest,
        theme.colorScheme.onSurfaceVariant,
        theme.colorScheme.outline,
      ),
    };
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: ShellSpace.sm,
        vertical: ShellSpace.xxs,
      ),
      decoration: BoxDecoration(
        color: background,
        borderRadius: BorderRadius.circular(ShellRadius.chip),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 6,
            height: 6,
            decoration: BoxDecoration(color: dot, shape: BoxShape.circle),
          ),
          const SizedBox(width: ShellSpace.xs + ShellSpace.xxs),
          // A chip may be placed in a narrow slot (a 250 px column, a 340 px aside); the label
          // shrinks with an ellipsis instead of overflowing its row.
          Flexible(
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.labelSmall?.copyWith(color: foreground),
            ),
          ),
        ],
      ),
    );
  }
}

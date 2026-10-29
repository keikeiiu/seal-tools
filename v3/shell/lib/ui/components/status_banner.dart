/// [StatusBanner] — one designed state as a panel (§8.4 of [PLAN-V3-UI-UX.md]): icon + title +
/// optional detail + one action. Colour reinforces a state that the words and icon already state.
///
/// Use it for readiness, validation refusals, save conflicts, display mismatches and scan results;
/// use [StatusChip] for a compact inline state.
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

import '../theme.dart';
import 'status_chip.dart';

class StatusBanner extends StatelessWidget {
  const StatusBanner({
    super.key,
    required this.kind,
    required this.title,
    this.detail,
    this.action,
    this.icon,
  });

  final StatusKind kind;
  final String title;
  final String? detail;
  final Widget? action;
  final IconData? icon;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = context.shellColors;
    final (background, foreground) = switch (kind) {
      StatusKind.success => (
        colors.successContainer,
        colors.onSuccessContainer,
      ),
      StatusKind.warning => (
        colors.warningContainer,
        colors.onWarningContainer,
      ),
      StatusKind.error => (
        theme.colorScheme.errorContainer,
        theme.colorScheme.onErrorContainer,
      ),
      StatusKind.info => (
        theme.colorScheme.surfaceContainerHigh,
        theme.colorScheme.onSurface,
      ),
      StatusKind.muted => (
        theme.colorScheme.surfaceContainerLow,
        theme.colorScheme.onSurfaceVariant,
      ),
    };
    final resolvedIcon =
        icon ??
        switch (kind) {
          StatusKind.success => Icons.check_circle_outline,
          StatusKind.warning => Icons.warning_amber_outlined,
          StatusKind.error => Icons.error_outline,
          StatusKind.info => Icons.info_outline,
          StatusKind.muted => Icons.info_outline,
        };
    return Material(
      color: background,
      borderRadius: BorderRadius.circular(ShellRadius.m),
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: ShellSpace.md,
          vertical: ShellSpace.sm + ShellSpace.xxs,
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(resolvedIcon, size: ShellSize.iconMd, color: foreground),
            const SizedBox(width: ShellSpace.sm),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: theme.textTheme.titleSmall?.copyWith(
                      color: foreground,
                    ),
                  ),
                  if (detail != null) ...[
                    const SizedBox(height: ShellSpace.xxs),
                    ConstrainedBox(
                      constraints: const BoxConstraints(
                        maxWidth: ShellWidth.prose,
                      ),
                      child: Text(
                        detail!,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: foreground,
                        ),
                      ),
                    ),
                  ],
                ],
              ),
            ),
            if (action != null) ...[
              const SizedBox(width: ShellSpace.md),
              action!,
            ],
          ],
        ),
      ),
    );
  }
}

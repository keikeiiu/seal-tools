/// [EmptyState] — the designed "nothing here yet" panel with one next action (§8.4 of
/// [PLAN-V3-UI-UX.md]). It replaces the grey one-line sentence and the wall of disabled controls.
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

import '../theme.dart';

class EmptyState extends StatelessWidget {
  const EmptyState({
    super.key,
    required this.icon,
    required this.title,
    required this.hint,
    this.action,
    this.compact = false,
  });

  final IconData icon;
  final String title;
  final String hint;
  final Widget? action;

  /// Inline variant for a section (smaller padding, one row) vs the panel variant.
  final bool compact;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final content = compact
        ? Row(
            children: [
              Icon(
                icon,
                size: ShellSize.iconMd,
                color: colors.onSurfaceVariant,
              ),
              const SizedBox(width: ShellSpace.sm),
              Expanded(
                child: Text(
                  '$title — $hint',
                  style: theme.textTheme.bodyMedium?.copyWith(
                    color: colors.onSurfaceVariant,
                  ),
                ),
              ),
              if (action != null) ...[
                const SizedBox(width: ShellSpace.md),
                action!,
              ],
            ],
          )
        : Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Icon(
                    icon,
                    size: ShellSize.iconMd,
                    color: colors.onSurfaceVariant,
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  Expanded(
                    child: Text(title, style: theme.textTheme.titleSmall),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.xs),
              Text(
                hint,
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: colors.onSurfaceVariant,
                ),
              ),
              if (action != null) ...[
                const SizedBox(height: ShellSpace.md),
                action!,
              ],
            ],
          );
    return Container(
      width: double.infinity,
      padding: EdgeInsets.all(compact ? ShellSpace.md : ShellSpace.lg),
      decoration: BoxDecoration(
        color: colors.surfaceContainerLow,
        borderRadius: BorderRadius.circular(ShellRadius.m),
        border: Border.all(color: colors.outlineVariant),
      ),
      child: content,
    );
  }
}

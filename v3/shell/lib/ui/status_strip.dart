/// The bottom status strip ([PLAN-V3-SHELL.md] §6): core identity, rev, root, and the log toggle.
///
/// It reads the model only. The root path is the core's from `hello` — the shell renders it and
/// never rewrites files under it ([DESIGN.md] §4).
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
/// [DESIGN.md]: ../../../docs/spec/DESIGN.md
library;

import 'package:flutter/material.dart';

import '../app/app.dart';
import 'theme.dart';

class StatusStrip extends StatelessWidget {
  const StatusStrip({
    super.key,
    required this.model,
    required this.logsVisible,
    required this.onToggleLogs,
  });

  final ShellModel model;
  final bool logsVisible;
  final VoidCallback onToggleLogs;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final connection = model.connection;
    final config = model.config;
    final root = config.root.isEmpty ? 'root ?' : 'root ${config.root}';
    return Material(
      color: theme.colorScheme.surfaceContainerLow,
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: ShellSpace.md,
          vertical: ShellSpace.xs,
        ),
        child: Row(
          children: [
            Icon(
              Icons.check_circle_outline,
              size: 14,
              color: theme.colorScheme.primary,
            ),
            const SizedBox(width: ShellSpace.xs),
            Text(
              'core ${connection.coreVersion ?? '?'} · rev ${config.rev}',
              key: const ValueKey('status-core'),
              style: theme.textTheme.labelSmall,
            ),
            const SizedBox(width: ShellSpace.md),
            Expanded(
              child: Text(
                root,
                key: const ValueKey('status-root'),
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ),
            TextButton(
              key: const ValueKey('logs-toggle'),
              onPressed: onToggleLogs,
              style: TextButton.styleFrom(
                visualDensity: VisualDensity.compact,
                padding: const EdgeInsets.symmetric(horizontal: ShellSpace.sm),
              ),
              child: Text(
                logsVisible
                    ? 'Hide logs'
                    : 'Logs (${model.logs.entries.length})',
              ),
            ),
          ],
        ),
      ),
    );
  }
}

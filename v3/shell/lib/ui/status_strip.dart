/// The bottom status strip: core identity on the left, the shortcut hints on the right
/// ([PLAN-V3-UI-UX.md] §6 Chrome).
///
/// It reads the model only. The root path is the core's from `hello` — the shell renders it and
/// never rewrites files under it ([DESIGN.md] §4). The logs drawer and the rail are toggled from the
/// top bar or Ctrl+L / Ctrl+B; the strip only advertises the shortcuts.
///
/// [PLAN-V3-UI-UX.md]: ../../../docs/plans/PLAN-V3-UI-UX.md
/// [DESIGN.md]: ../../../docs/spec/DESIGN.md
library;

import 'package:flutter/material.dart';

import '../app/app.dart';
import 'theme.dart';

class StatusStrip extends StatelessWidget {
  const StatusStrip({super.key, required this.model, this.logsVisible = false});

  final ShellModel model;
  final bool logsVisible;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final connection = model.connection;
    final config = model.config;
    final root = config.root.isEmpty ? 'root ?' : 'root ${config.root}';
    return Material(
      color: theme.colorScheme.surfaceContainerLow,
      child: Container(
        height: ShellSize.statusStripHeight,
        decoration: BoxDecoration(
          border: Border(
            top: BorderSide(
              color: theme.colorScheme.outlineVariant.withValues(alpha: 0.6),
            ),
          ),
        ),
        padding: const EdgeInsets.symmetric(horizontal: ShellSpace.md),
        child: Row(
          children: [
            Container(
              width: 6,
              height: 6,
              decoration: BoxDecoration(
                color: context.shellColors.success,
                shape: BoxShape.circle,
              ),
            ),
            const SizedBox(width: ShellSpace.xs + ShellSpace.xxs),
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
            Text(
              logsVisible
                  ? 'Ctrl+B sidebar · Ctrl+L logs · Esc close'
                  : 'Ctrl+B sidebar · Ctrl+L logs',
              key: const ValueKey('logs-hint'),
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

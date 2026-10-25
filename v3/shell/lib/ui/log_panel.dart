/// The log panel ([PLAN-V3-SHELL.md] §6's status surface): the model's ring buffer, newest at the top.
///
/// The origin is shown on every line so a core `log` event, a stderr line (which is NEVER protocol,
/// [PROTOCOL.md] §1) and a shell note are never confused.
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'package:flutter/material.dart';

import '../app/app.dart';
import 'theme.dart';

class LogPanel extends StatelessWidget {
  const LogPanel({super.key, required this.entries});

  final List<LogEntry> entries;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final style = theme.textTheme.bodySmall?.copyWith(
      fontFamily: 'Consolas',
      color: theme.colorScheme.onSurfaceVariant,
    );
    return Container(
      key: const ValueKey('log-panel'),
      height: 150,
      width: double.infinity,
      color: theme.colorScheme.surfaceContainerLowest,
      padding: const EdgeInsets.symmetric(
        horizontal: ShellSpace.md,
        vertical: ShellSpace.xs,
      ),
      child: entries.isEmpty
          ? Center(
              child: Text(
                'No log lines yet',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            )
          : ListView.builder(
              reverse: true,
              itemCount: entries.length,
              itemBuilder: (context, index) {
                final entry = entries[entries.length - 1 - index];
                return Text(_format(entry), style: style);
              },
            ),
    );
  }

  String _format(LogEntry entry) => switch (entry.origin) {
    LogOrigin.core => '[${entry.level ?? 'info'}] ${entry.text}',
    LogOrigin.stderr => 'stderr: ${entry.text}',
    LogOrigin.shell => 'shell: ${entry.text}',
  };
}

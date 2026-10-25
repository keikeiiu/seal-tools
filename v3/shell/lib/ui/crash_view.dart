/// The crash / connection surface ([PLAN-V3-SHELL.md] §2/§6): the failure in the core's own terms,
/// the stderr tail, and a single Restart button.
///
/// It deliberately shows the MODEL's captured stderr tail rather than reading a dead process. A
/// `bad_version` is a packaging bug — both binaries must be replaced — so it hides Restart.
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'package:flutter/material.dart';

import '../app/app.dart';
import 'theme.dart';

class CrashView extends StatelessWidget {
  const CrashView({super.key, required this.model});

  final ShellModel model;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final connection = model.connection;
    final tail = model.logs.stderrTail.isNotEmpty
        ? model.logs.stderrTail
        : connection.stderrTail;
    final root = connection.root;

    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(ShellSpace.xl),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 720),
          child: Card(
            child: Padding(
              padding: const EdgeInsets.all(ShellSpace.lg),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Icon(
                        Icons.error_outline,
                        color: theme.colorScheme.error,
                        size: 20,
                      ),
                      const SizedBox(width: ShellSpace.sm),
                      Text(
                        'The core is not running',
                        style: theme.textTheme.titleMedium,
                      ),
                    ],
                  ),
                  const SizedBox(height: ShellSpace.sm),
                  Text(
                    connection.failureMessage ?? 'The connection failed.',
                    key: const ValueKey('crash-message'),
                    style: theme.textTheme.bodyMedium,
                  ),
                  if (root != null) ...[
                    const SizedBox(height: ShellSpace.xs),
                    Text(
                      'root: $root',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                  if (tail.isNotEmpty) ...[
                    const SizedBox(height: ShellSpace.md),
                    Text('Stderr tail', style: theme.textTheme.labelLarge),
                    const SizedBox(height: ShellSpace.xs),
                    Container(
                      width: double.infinity,
                      constraints: const BoxConstraints(maxHeight: 180),
                      padding: const EdgeInsets.all(ShellSpace.sm),
                      decoration: BoxDecoration(
                        color: theme.colorScheme.surfaceContainerLowest,
                        borderRadius: const BorderRadius.all(
                          Radius.circular(ShellSpace.sm),
                        ),
                      ),
                      child: SingleChildScrollView(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            for (final line in tail)
                              Text(
                                line,
                                style: theme.textTheme.bodySmall?.copyWith(
                                  fontFamily: 'Consolas',
                                  color: theme.colorScheme.onSurfaceVariant,
                                ),
                              ),
                          ],
                        ),
                      ),
                    ),
                  ],
                  const SizedBox(height: ShellSpace.lg),
                  if (connection.canRestart)
                    FilledButton.icon(
                      key: const ValueKey('crash-restart'),
                      onPressed: () => model.restart(),
                      icon: const Icon(Icons.refresh, size: 18),
                      label: const Text('Restart core'),
                    )
                  else
                    Text(
                      'A protocol mismatch is not fixed by restarting: replace the '
                      'shell and core binaries with a matched pair.',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.error,
                      ),
                    ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

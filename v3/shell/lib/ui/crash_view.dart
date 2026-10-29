/// The crash / connection surface ([PLAN-V3-SHELL.md] §2/§6): the failure in the core's own terms,
/// the stderr tail, and a single Restart button.
///
/// It deliberately shows the MODEL's captured stderr tail rather than reading a dead process. A
/// `bad_version` is a packaging bug — both binaries must be replaced — so it hides Restart and states
/// that instead. Presentation follows the shell's state vocabulary: one error banner, one monospace
/// evidence panel, one filled action.
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'package:flutter/material.dart';

import '../app/app.dart';
import 'components/info_row.dart';
import 'components/status_banner.dart';
import 'components/status_chip.dart';
import 'components/surface_panel.dart';
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
    final restart = connection.canRestart
        ? FilledButton.icon(
            key: const ValueKey('crash-restart'),
            onPressed: () => model.restart(),
            icon: const Icon(Icons.refresh, size: 18),
            label: const Text('Restart core'),
          )
        : null;

    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(ShellSpace.xl),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 720),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              StatusBanner(
                kind: StatusKind.error,
                title: 'The core is not running',
                detail: connection.canRestart
                    ? 'Restarting the core usually clears this. The reason the '
                          'core gave is below.'
                    : 'A protocol mismatch is a packaging problem: repair it by '
                          'installing a matched shell + core pair.',
                action: restart,
              ),
              const SizedBox(height: ShellSpace.md),
              // The message keeps its own keyed Text: callers/tests read the core's sentence here,
              // and a bad_version needs the shell's explanation to stay selectable.
              Text(
                connection.failureMessage ?? 'The connection failed.',
                key: const ValueKey('crash-message'),
                style: theme.textTheme.bodyMedium,
              ),
              if (root != null) ...[
                const SizedBox(height: ShellSpace.sm),
                InfoRow(label: 'root', value: root, mono: true),
              ],
              if (tail.isNotEmpty) ...[
                const SizedBox(height: ShellSpace.lg),
                SurfacePanel(
                  title: 'Stderr tail',
                  description:
                      'stderr is never protocol; the core writes its own reason '
                      'here before it exits.',
                  child: Container(
                    width: double.infinity,
                    constraints: const BoxConstraints(maxHeight: 180),
                    padding: const EdgeInsets.all(ShellSpace.sm),
                    decoration: BoxDecoration(
                      color: theme.colorScheme.surfaceContainerLowest,
                      borderRadius: BorderRadius.circular(ShellRadius.s),
                    ),
                    child: SingleChildScrollView(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          for (final line in tail)
                            Text(
                              line,
                              style: ShellText.mono.copyWith(
                                color: theme.colorScheme.onSurfaceVariant,
                              ),
                            ),
                        ],
                      ),
                    ),
                  ),
                ),
              ],
              if (!connection.canRestart) ...[
                const SizedBox(height: ShellSpace.md),
                const StatusBanner(
                  kind: StatusKind.warning,
                  title: 'Protocol mismatch — replace the binaries',
                  detail:
                      'Restarting cannot fix a version mismatch. Install the shell '
                      'and core from the same release.',
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

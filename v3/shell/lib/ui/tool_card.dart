/// One tool card: the `ToolSnapshot` the core published, the pinned row lines, and Start/Stop.
///
/// The card renders; it decides nothing. Enabled states are UI convenience derived from the
/// core-published phase and the model's in-flight flag; a Start does NOT optimistically change the
/// card — the phase moves only when the core publishes `state` ([PLAN-V3-SHELL.md] §5).
///
/// The card must render `rows: []` without assuming non-empty: that means "no rows to show", never
/// "not known yet" ([PROTOCOL.md] §5), and the empty state is its own small line.
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'package:flutter/material.dart';

import '../protocol/protocol.dart';
import 'theme.dart';
import 'tool_labels.dart';

class ToolCard extends StatelessWidget {
  const ToolCard({
    super.key,
    required this.tool,
    required this.snapshot,
    required this.finished,
    required this.refusal,
    required this.pending,
    required this.ready,
    required this.onStart,
    required this.onStop,
    this.holdSpaceLine,
  });

  final ToolId tool;

  /// The core's last published snapshot, or null before the first `state` event.
  final ToolSnapshot? snapshot;

  /// The last `tool.finished` for this tool, shown under the rows.
  final ToolFinished? finished;

  /// The core's line for the last refused command, shown verbatim.
  final String? refusal;

  /// A start/stop is in flight (shell convenience): both buttons are disabled.
  final bool pending;

  /// The core connection is ready; commands are possible.
  final bool ready;

  final VoidCallback onStart;
  final VoidCallback onStop;

  /// The Spammer card's Hold Space report (TOOL-FLOWS: holdspace has no card of its own).
  final String? holdSpaceLine;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final phase = snapshot?.phase ?? ToolPhase.idle;
    final engaged = phase != ToolPhase.idle;
    final canStart = ready && !pending && !engaged;
    final canStop = ready && !pending && engaged;
    final countdown = countdownLabel(snapshot);
    final error = snapshot?.error;
    final holdSpace = holdSpaceLine;

    return Card(
      key: ValueKey('card-${tool.wire}'),
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.all(ShellSpace.md),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    toolDisplayName(tool),
                    style: theme.textTheme.titleMedium,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                _PhaseChip(phase: phase, engaged: engaged),
              ],
            ),
            const SizedBox(height: ShellSpace.xs),
            Text(
              snapshot?.message ?? 'No state published yet',
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.bodyMedium,
            ),
            if (countdown != null) ...[
              const SizedBox(height: 2),
              Text(
                countdown,
                key: ValueKey('countdown-${tool.wire}'),
                style: theme.textTheme.bodySmall,
              ),
            ],
            if (error != null) ...[
              const SizedBox(height: 2),
              Text(
                error,
                key: ValueKey('error-${tool.wire}'),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),
            ],
            const SizedBox(height: ShellSpace.xs),
            Expanded(
              child: _RowsView(tool: tool, snapshot: snapshot),
            ),
            if (holdSpace != null)
              Text(
                'Hold Space: $holdSpace',
                key: const ValueKey('holdspace-report'),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall,
              ),
            if (finished != null)
              Text(
                finished!.message,
                key: ValueKey('finished-${tool.wire}'),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.tertiary,
                ),
              ),
            if (refusal != null)
              Text(
                refusal!,
                key: ValueKey('refusal-${tool.wire}'),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),
            const SizedBox(height: ShellSpace.sm),
            Row(
              children: [
                Expanded(
                  child: FilledButton(
                    key: ValueKey('start-${tool.wire}'),
                    onPressed: canStart ? onStart : null,
                    style: FilledButton.styleFrom(
                      visualDensity: VisualDensity.compact,
                      padding: const EdgeInsets.symmetric(
                        horizontal: ShellSpace.sm,
                      ),
                    ),
                    child: const Text('Start'),
                  ),
                ),
                const SizedBox(width: ShellSpace.sm),
                Expanded(
                  child: OutlinedButton(
                    key: ValueKey('stop-${tool.wire}'),
                    onPressed: canStop ? onStop : null,
                    style: OutlinedButton.styleFrom(
                      visualDensity: VisualDensity.compact,
                      padding: const EdgeInsets.symmetric(
                        horizontal: ShellSpace.sm,
                      ),
                    ),
                    child: const Text('Stop'),
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _PhaseChip extends StatelessWidget {
  const _PhaseChip({required this.phase, required this.engaged});

  final ToolPhase phase;
  final bool engaged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final background = switch (phase) {
      ToolPhase.running => theme.colorScheme.primaryContainer,
      ToolPhase.starting ||
      ToolPhase.stopping => theme.colorScheme.tertiaryContainer,
      ToolPhase.idle => theme.colorScheme.surfaceContainerHighest,
    };
    final foreground = switch (phase) {
      ToolPhase.running => theme.colorScheme.onPrimaryContainer,
      ToolPhase.starting ||
      ToolPhase.stopping => theme.colorScheme.onTertiaryContainer,
      ToolPhase.idle => theme.colorScheme.onSurfaceVariant,
    };
    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: ShellSpace.sm,
        vertical: 2,
      ),
      decoration: BoxDecoration(
        color: background,
        borderRadius: const BorderRadius.all(Radius.circular(999)),
      ),
      child: Text(
        engaged ? phase.wire : 'idle',
        style: theme.textTheme.labelSmall?.copyWith(color: foreground),
      ),
    );
  }
}

/// The pinned row lines ([PROTOCOL.md] §5): name / status / detail / next_action_in_seconds.
/// A `rows: []` snapshot renders one quiet line instead of an empty gap.
class _RowsView extends StatelessWidget {
  const _RowsView({required this.tool, required this.snapshot});

  final ToolId tool;
  final ToolSnapshot? snapshot;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final rows = snapshot?.rows ?? const <ToolRow>[];
    if (rows.isEmpty) {
      return Align(
        alignment: Alignment.topLeft,
        child: Text(
          'No rows to show',
          key: ValueKey('rows-empty-${tool.wire}'),
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      );
    }

    return ListView.separated(
      padding: EdgeInsets.zero,
      itemCount: rows.length,
      separatorBuilder: (context, index) => const SizedBox(height: 2),
      itemBuilder: (context, index) {
        final row = rows[index];
        return Row(
          key: ValueKey('row-${tool.wire}-$index'),
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              flex: 2,
              child: Text(
                row.name,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall,
              ),
            ),
            Expanded(
              flex: 3,
              child: Text(
                row.status,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.primary,
                ),
              ),
            ),
            if (row.detail != null)
              Expanded(
                flex: 3,
                child: Text(
                  row.detail!,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ),
            if (row.nextActionInSeconds != null)
              Text(
                formatNextAction(row.nextActionInSeconds),
                style: theme.textTheme.labelSmall,
              ),
          ],
        );
      },
    );
  }
}

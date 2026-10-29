/// One tool card on the Cards screen: a full **running** card (the subject of the screen) or a
/// one-line **idle tile** with an outlined Start ([PLAN-V3-UI-UX.md] §6 "Cards").
///
/// The card renders; it decides nothing. Enabled states are UI convenience derived from the
/// core-published phase and the model's in-flight flag; a Start does NOT optimistically change the
/// card — the phase moves only when the core publishes `state` ([PLAN-V3-SHELL.md] §5).
///
/// The card must render `rows: []` without assuming non-empty: that means "no rows to show", never
/// "not known yet" ([PROTOCOL.md] §5). Idle tiles show no rows table at all.
///
/// [PLAN-V3-UI-UX.md]: ../../../docs/plans/PLAN-V3-UI-UX.md
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'package:flutter/material.dart';

import '../protocol/protocol.dart';
import 'components/status_chip.dart';
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
    this.elapsedTicks = 0,
    this.extras,
  });

  final ToolId tool;

  /// The core's last published snapshot, or null before the first `state` event.
  final ToolSnapshot? snapshot;

  /// The last `tool.finished` for this tool, shown under the rows.
  final ToolFinished? finished;

  /// The core's line for the last refused command, shown verbatim.
  final String? refusal;

  /// A start/stop is in flight (shell convenience): the action is disabled and labelled pending.
  final bool pending;

  /// The core connection is ready; commands are possible.
  final bool ready;

  final VoidCallback onStart;
  final VoidCallback onStop;

  /// Card-specific config controls (Buy picker, Quest pickers, Spammer rotation) shown on the idle
  /// tile; per-run choices stay on the card ([PLAN-V3-UI-UX.md] D3/§6).
  final Widget? extras;

  /// Whole seconds elapsed since this tool's last `state` event, driven by the grid's ONE ticker.
  /// Only the DISPLAYED countdown uses it: rows without `next_action_in_seconds` are untouched.
  final int elapsedTicks;

  @override
  Widget build(BuildContext context) {
    final phase = snapshot?.phase ?? ToolPhase.idle;
    if (phase == ToolPhase.idle) {
      return _IdleTile(
        tool: tool,
        snapshot: snapshot,
        finished: finished,
        refusal: refusal,
        pending: pending,
        ready: ready,
        onStart: onStart,
        extras: extras,
      );
    }
    return _RunningCard(
      tool: tool,
      snapshot: snapshot!,
      finished: finished,
      refusal: refusal,
      pending: pending,
      ready: ready,
      onStop: onStop,
      elapsedTicks: elapsedTicks,
    );
  }
}

/// The core's phase word for the chip, and the semantic kind it paints with.
(String, StatusKind) _phaseChip(ToolPhase phase) => switch (phase) {
  ToolPhase.running => ('running', StatusKind.success),
  ToolPhase.starting => ('starting', StatusKind.info),
  ToolPhase.stopping => ('stopping', StatusKind.warning),
  ToolPhase.idle => ('idle', StatusKind.muted),
};

String _idleLine(ToolId tool, ToolSnapshot? snapshot) {
  if (tool == ToolId.holdspace) {
    return 'Off · spacebar auto-pickup';
  }
  final message = snapshot?.message.trim() ?? '';
  return message.isEmpty ? 'Idle — ready to start' : message;
}

String _runningLine(ToolId tool, ToolSnapshot snapshot) {
  final message = snapshot.message.trim();
  if (message.isNotEmpty) {
    return message;
  }
  return tool == ToolId.holdspace ? 'Holding the spacebar' : 'Running';
}

class _IdleTile extends StatelessWidget {
  const _IdleTile({
    required this.tool,
    required this.snapshot,
    required this.finished,
    required this.refusal,
    required this.pending,
    required this.ready,
    required this.onStart,
    this.extras,
  });

  final ToolId tool;
  final ToolSnapshot? snapshot;
  final ToolFinished? finished;
  final String? refusal;
  final bool pending;
  final bool ready;
  final VoidCallback onStart;
  final Widget? extras;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final error = snapshot?.error;
    return Card(
      key: ValueKey('card-${tool.wire}'),
      child: Padding(
        padding: const EdgeInsets.all(ShellSpace.md),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(
                  toolIcon(tool),
                  size: ShellSize.iconMd,
                  color: theme.colorScheme.onSurfaceVariant,
                ),
                const SizedBox(width: ShellSpace.sm),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        toolDisplayName(tool),
                        style: theme.textTheme.titleSmall,
                        overflow: TextOverflow.ellipsis,
                      ),
                      const SizedBox(height: ShellSpace.xxs),
                      Text(
                        _idleLine(tool, snapshot),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: ShellSpace.sm),
                if (extras == null)
                  _StartButton(
                    tool: tool,
                    pending: pending,
                    ready: ready,
                    onStart: onStart,
                  ),
              ],
            ),
            if (extras != null) ...[
              const SizedBox(height: ShellSpace.sm),
              Row(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  Expanded(child: extras!),
                  const SizedBox(width: ShellSpace.sm),
                  _StartButton(
                    tool: tool,
                    pending: pending,
                    ready: ready,
                    onStart: onStart,
                  ),
                ],
              ),
            ],
            if (error != null)
              _CardLine(
                textKey: ValueKey('error-${tool.wire}'),
                text: error,
                color: theme.colorScheme.error,
              ),
            if (finished != null)
              _CardLine(
                textKey: ValueKey('finished-${tool.wire}'),
                text: finished!.message,
                color: theme.colorScheme.onSurfaceVariant,
              ),
            if (refusal != null)
              _CardLine(
                textKey: ValueKey('refusal-${tool.wire}'),
                text: refusal!,
                color: theme.colorScheme.error,
              ),
          ],
        ),
      ),
    );
  }
}

class _StartButton extends StatelessWidget {
  const _StartButton({
    required this.tool,
    required this.pending,
    required this.ready,
    required this.onStart,
  });

  final ToolId tool;
  final bool pending;
  final bool ready;
  final VoidCallback onStart;

  @override
  Widget build(BuildContext context) {
    return OutlinedButton(
      key: ValueKey('start-${tool.wire}'),
      onPressed: ready && !pending ? onStart : null,
      child: pending
          ? const _PendingLabel(label: 'Starting…')
          : const Text('Start'),
    );
  }
}

class _RunningCard extends StatelessWidget {
  const _RunningCard({
    required this.tool,
    required this.snapshot,
    required this.finished,
    required this.refusal,
    required this.pending,
    required this.ready,
    required this.onStop,
    required this.elapsedTicks,
  });

  final ToolId tool;
  final ToolSnapshot snapshot;
  final ToolFinished? finished;
  final String? refusal;
  final bool pending;
  final bool ready;
  final VoidCallback onStop;
  final int elapsedTicks;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = context.shellColors;
    final (phaseWord, kind) = _phaseChip(snapshot.phase);
    final countdown = countdownLabel(snapshot);
    final error = snapshot.error;
    return Card(
      key: ValueKey('card-${tool.wire}'),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(ShellRadius.l),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Container(height: 3, color: colors.success),
            Padding(
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
                      StatusChip(kind: kind, label: phaseWord),
                    ],
                  ),
                  const SizedBox(height: ShellSpace.xs),
                  Text(
                    _runningLine(tool, snapshot),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodyMedium,
                  ),
                  if (countdown != null) ...[
                    const SizedBox(height: ShellSpace.xxs),
                    Text(
                      countdown,
                      key: ValueKey('countdown-${tool.wire}'),
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                  if (error != null)
                    _CardLine(
                      textKey: ValueKey('error-${tool.wire}'),
                      text: error,
                      color: theme.colorScheme.error,
                    ),
                  const SizedBox(height: ShellSpace.sm),
                  _RowsView(
                    tool: tool,
                    snapshot: snapshot,
                    elapsedTicks: elapsedTicks,
                  ),
                  if (finished != null)
                    _CardLine(
                      textKey: ValueKey('finished-${tool.wire}'),
                      text: finished!.message,
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  if (refusal != null)
                    _CardLine(
                      textKey: ValueKey('refusal-${tool.wire}'),
                      text: refusal!,
                      color: theme.colorScheme.error,
                    ),
                  const SizedBox(height: ShellSpace.sm),
                  Row(
                    children: [
                      const Spacer(),
                      FilledButton(
                        key: ValueKey('stop-${tool.wire}'),
                        onPressed: ready && !pending ? onStop : null,
                        style: FilledButton.styleFrom(
                          backgroundColor: theme.colorScheme.errorContainer,
                          foregroundColor: theme.colorScheme.onErrorContainer,
                        ),
                        child: pending
                            ? const _PendingLabel(label: 'Stopping…')
                            : const Text('Stop'),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _PendingLabel extends StatelessWidget {
  const _PendingLabel({required this.label});

  final String label;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        const SizedBox(
          width: 14,
          height: 14,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
        const SizedBox(width: ShellSpace.sm),
        Text(label),
      ],
    );
  }
}

class _CardLine extends StatelessWidget {
  const _CardLine({
    this.textKey,
    required this.text,
    required this.color,
  });

  final Key? textKey;
  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final style = Theme.of(context).textTheme.bodySmall?.copyWith(color: color);
    return Padding(
      padding: const EdgeInsets.only(top: ShellSpace.xxs),
      child: Tooltip(
        message: text,
        child: Text(
          text,
          key: textKey,
          maxLines: 3,
          overflow: TextOverflow.ellipsis,
          style: style,
        ),
      ),
    );
  }
}

/// The pinned row lines ([PROTOCOL.md] §5): name / status / detail / next_action_in_seconds.
/// A `rows: []` snapshot renders one quiet line instead of an empty gap.
class _RowsView extends StatelessWidget {
  const _RowsView({
    required this.tool,
    required this.snapshot,
    this.elapsedTicks = 0,
  });

  final ToolId tool;
  final ToolSnapshot snapshot;

  /// Display-only elapsed seconds since the latest state event; the published value is the baseline.
  final int elapsedTicks;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final rows = snapshot.rows;
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

    final headerStyle = theme.textTheme.labelSmall?.copyWith(
      color: theme.colorScheme.onSurfaceVariant,
      letterSpacing: 0.6,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(flex: 2, child: Text('ROW', style: headerStyle)),
            Expanded(flex: 3, child: Text('STATUS', style: headerStyle)),
            Expanded(flex: 3, child: Text('DETAIL', style: headerStyle)),
            Text('NEXT', style: headerStyle),
          ],
        ),
        const SizedBox(height: ShellSpace.xxs),
        for (var index = 0; index < rows.length; index++)
          Padding(
            padding: const EdgeInsets.only(bottom: ShellSpace.xxs),
            child: _RowLine(
              key: ValueKey('row-${tool.wire}-$index'),
              row: rows[index],
              elapsedTicks: elapsedTicks,
            ),
          ),
      ],
    );
  }
}

class _RowLine extends StatelessWidget {
  const _RowLine({
    super.key,
    required this.row,
    required this.elapsedTicks,
  });

  final ToolRow row;
  final int elapsedTicks;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = context.shellColors;
    final base = theme.textTheme.bodySmall;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          flex: 2,
          child: Text(row.name, overflow: TextOverflow.ellipsis, style: base),
        ),
        Expanded(
          flex: 3,
          child: Text(
            row.status,
            overflow: TextOverflow.ellipsis,
            style: base?.copyWith(color: colors.success),
          ),
        ),
        Expanded(
          flex: 3,
          child: Text(
            row.detail ?? '',
            overflow: TextOverflow.ellipsis,
            style: base?.copyWith(color: theme.colorScheme.onSurfaceVariant),
          ),
        ),
        Text(
          row.nextActionInSeconds == null
              ? ''
              : formatNextAction(row.nextActionInSeconds! - elapsedTicks),
          style: theme.textTheme.labelSmall,
        ),
      ],
    );
  }
}

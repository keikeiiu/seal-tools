/// One tool card on the Cards screen: a full **running** card (the subject of the screen) or a
/// one-line **idle tile** with an outlined Start ([PLAN-V3-UI-UX.md] §6 "Cards").
///
/// The card renders; it decides nothing. Enabled states are UI convenience derived from the
/// core-published phase and the model's in-flight flag; a Start does NOT optimistically change the
/// card — the phase moves only when the core publishes `state` ([PLAN-V3-SHELL.md] §5).
///
/// The tile face stays one line like the demo. Cards with per-run/saved choices (Buy, Quest, Spammer)
/// put them behind the small options button and reveal them inside the card, so the default state is
/// still "icon + name + one-line state + Start".
///
/// The card must render `rows: []` without assuming non-empty: that means "no rows to show", never
/// "not known yet" ([PROTOCOL.md] §5). Idle tiles show no rows table at all.
///
/// [PLAN-V3-UI-UX.md]: ../../../docs/plans/PLAN-V3-UI-UX.md
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'package:flutter/material.dart';

import '../protocol/protocol.dart' hide Size;
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
    this.idleSummary,
    this.onOptions,
    this.optionsOpen = false,
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

  /// Card-specific config controls (Buy run choice, Quest flow, Spammer rotation). They are revealed
  /// inside the tile by the options button, so the collapsed tile stays one line.
  final Widget? extras;

  /// The shell's own one-line idle summary (e.g. `Ready · preset "potion" · 3 per run`). The core's
  /// `state` message is still shown when this is null.
  final String? idleSummary;

  /// Toggles [optionsOpen]; null means this tool has no options and shows no button.
  final VoidCallback? onOptions;

  /// Whether [extras] are revealed under the tile row.
  final bool optionsOpen;

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
        idleSummary: idleSummary,
        onOptions: onOptions,
        optionsOpen: optionsOpen,
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
  return message.isEmpty ? 'Ready' : message;
}

String _runningLine(ToolId tool, ToolSnapshot snapshot) {
  final message = snapshot.message.trim();
  if (message.isNotEmpty) {
    return message;
  }
  return tool == ToolId.holdspace ? 'Holding the spacebar' : 'Running';
}

/// The demo's scope word on a running card: the pet loop is the resident process, the rest act on the
/// foreground game window. Presentation only; Hold Space acts through the board, not the foreground
/// window, so it carries no scope word.
String? _scopeLabel(ToolId tool) => tool == ToolId.holdspace
    ? null
    : (tool == ToolId.pet ? 'resident' : 'foreground');

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
    this.idleSummary,
    this.onOptions,
    this.optionsOpen = false,
  });

  final ToolId tool;
  final ToolSnapshot? snapshot;
  final ToolFinished? finished;
  final String? refusal;
  final bool pending;
  final bool ready;
  final VoidCallback onStart;
  final Widget? extras;
  final String? idleSummary;
  final VoidCallback? onOptions;
  final bool optionsOpen;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final error = snapshot?.error;
    final summary = idleSummary ?? _idleLine(tool, snapshot);
    return Card(
      key: ValueKey('card-${tool.wire}'),
      color: theme.colorScheme.surfaceContainerLow,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(
          ShellSpace.md + ShellSpace.xxs,
          ShellSpace.md,
          ShellSpace.sm + ShellSpace.xxs,
          ShellSpace.md,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            ConstrainedBox(
              constraints: const BoxConstraints(
                minHeight: ShellSize.idleTileMinHeight - ShellSpace.md * 2,
              ),
              child: Row(
                children: [
                  Container(
                    width: 34,
                    height: 34,
                    decoration: BoxDecoration(
                      color: theme.colorScheme.surfaceContainerHigh,
                      borderRadius: BorderRadius.circular(ShellRadius.s + 3),
                    ),
                    child: Icon(
                      toolIcon(tool),
                      size: ShellSize.iconMd,
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  Expanded(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          toolDisplayName(tool),
                          style: theme.textTheme.titleSmall?.copyWith(
                            fontSize: 13.5,
                          ),
                          overflow: TextOverflow.ellipsis,
                        ),
                        const SizedBox(height: ShellSpace.xxs),
                        Text(
                          summary,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: theme.textTheme.bodySmall?.copyWith(
                            fontSize: 12,
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                  ),
                  if (onOptions != null)
                    IconButton(
                      key: ValueKey('options-${tool.wire}'),
                      tooltip: optionsOpen ? 'Hide options' : 'Run options',
                      visualDensity: VisualDensity.compact,
                      style: IconButton.styleFrom(
                        minimumSize: const Size(32, 32),
                        padding: EdgeInsets.zero,
                      ),
                      onPressed: onOptions,
                      icon: Icon(
                        optionsOpen ? Icons.expand_less : Icons.tune,
                        size: ShellSize.iconMd,
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  const SizedBox(width: ShellSpace.sm),
                  _StartButton(
                    tool: tool,
                    pending: pending,
                    ready: ready,
                    onStart: onStart,
                  ),
                ],
              ),
            ),
            if (optionsOpen && extras != null) ...[
              const SizedBox(height: ShellSpace.sm),
              extras!,
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
    final theme = Theme.of(context);
    return OutlinedButton(
      key: ValueKey('start-${tool.wire}'),
      onPressed: ready && !pending ? onStart : null,
      style: OutlinedButton.styleFrom(
        minimumSize: const Size(0, 30),
        padding: const EdgeInsets.symmetric(horizontal: ShellSpace.md),
        textStyle: theme.textTheme.labelLarge?.copyWith(fontSize: 12),
      ),
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
    final scope = _scopeLabel(tool);
    return Card(
      key: ValueKey('card-${tool.wire}'),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(ShellRadius.l),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Container(height: 3, color: colors.success),
            Padding(
              padding: const EdgeInsets.fromLTRB(
                ShellSpace.md + ShellSpace.xxs,
                ShellSpace.md,
                ShellSpace.md + ShellSpace.xxs,
                ShellSpace.sm,
              ),
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
                      const SizedBox(width: ShellSpace.xs + ShellSpace.xxs),
                      StatusChip(kind: kind, label: phaseWord),
                      if (scope != null) ...[
                        const SizedBox(width: ShellSpace.sm),
                        Text(
                          scope,
                          style: theme.textTheme.labelSmall?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ],
                  ),
                  const SizedBox(height: ShellSpace.xs),
                  Text(
                    _runningLine(tool, snapshot),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.bodyMedium?.copyWith(fontSize: 13.5),
                  ),
                  if (countdown != null) ...[
                    const SizedBox(height: ShellSpace.xxs),
                    Text(
                      countdown,
                      key: ValueKey('countdown-${tool.wire}'),
                      style: ShellText.mono.copyWith(
                        fontSize: 12,
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
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(
                ShellSpace.md + ShellSpace.xxs,
                0,
                ShellSpace.md + ShellSpace.xxs,
                ShellSpace.md,
              ),
              child: Row(
                children: [
                  const Spacer(),
                  FilledButton(
                    key: ValueKey('stop-${tool.wire}'),
                    onPressed: ready && !pending ? onStop : null,
                    style: FilledButton.styleFrom(
                      backgroundColor: theme.colorScheme.errorContainer,
                      foregroundColor: theme.colorScheme.onErrorContainer,
                      minimumSize: const Size(0, 32),
                      padding: const EdgeInsets.symmetric(
                        horizontal: ShellSpace.md,
                      ),
                      textStyle: theme.textTheme.labelLarge?.copyWith(
                        fontSize: 12,
                      ),
                    ),
                    child: pending
                        ? const _PendingLabel(label: 'Stopping…')
                        : Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              const Icon(Icons.stop, size: 14),
                              const SizedBox(width: ShellSpace.xs),
                              Text('Stop'),
                            ],
                          ),
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
          width: 13,
          height: 13,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
        const SizedBox(width: ShellSpace.xs),
        Text(label),
      ],
    );
  }
}

class _CardLine extends StatelessWidget {
  const _CardLine({this.textKey, required this.text, required this.color});

  final Key? textKey;
  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final style = Theme.of(context).textTheme.bodySmall?.copyWith(color: color);
    return Padding(
      padding: const EdgeInsets.only(top: ShellSpace.xs),
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
      letterSpacing: 0.7,
    );
    final border = BorderSide(
      color: theme.colorScheme.outlineVariant.withValues(alpha: 0.35),
    );
    return Container(
      decoration: BoxDecoration(
        border: Border(
          top: BorderSide(
            color: theme.colorScheme.outlineVariant.withValues(alpha: 0.5),
          ),
        ),
      ),
      padding: const EdgeInsets.only(top: ShellSpace.xs),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(bottom: ShellSpace.xxs),
            child: Row(
              children: [
                Expanded(flex: 2, child: Text('ROW', style: headerStyle)),
                Expanded(flex: 3, child: Text('STATUS', style: headerStyle)),
                Expanded(flex: 3, child: Text('DETAIL', style: headerStyle)),
                Text('NEXT', style: headerStyle),
              ],
            ),
          ),
          for (var index = 0; index < rows.length; index++)
            Container(
              decoration: index == 0
                  ? null
                  : BoxDecoration(border: Border(top: border)),
              padding: const EdgeInsets.symmetric(vertical: ShellSpace.xs),
              child: _RowLine(
                key: ValueKey('row-${tool.wire}-$index'),
                row: rows[index],
                elapsedTicks: elapsedTicks,
              ),
            ),
        ],
      ),
    );
  }
}

class _RowLine extends StatelessWidget {
  const _RowLine({super.key, required this.row, required this.elapsedTicks});

  final ToolRow row;
  final int elapsedTicks;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final base = theme.textTheme.bodySmall?.copyWith(fontSize: 12.5);
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
            style: base?.copyWith(color: theme.colorScheme.onSurfaceVariant),
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
          style: ShellText.mono.copyWith(
            fontSize: 11.5,
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ],
    );
  }
}

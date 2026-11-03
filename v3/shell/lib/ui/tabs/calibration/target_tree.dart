/// The Calibrate workspace's target tree: the 17 presentation marks (34 instances) grouped by the
/// tool that consumes them (Shop 5/5, Gem 3/9, Pet 9/20, Tuner session-only), each mark row carrying
/// a semantic status dot and a count, and a targeted kind expanding to its named instances
/// (PROTOCOL §12.8).
///
/// Multi-mark calibrations are listed INSIDE their tool's group with a `session` label, not in a
/// separate global section: Pet's hover tooltip and Tuner's OCR+spring session. Spring point is not
/// a normal mark any more — it is set inside the Tuner session, so one tuner capture covers all four
/// tuner targets (v2's flow). The Pet session was removed because it duplicated the nine Pet marks
/// already in the upper editor.
///
/// Selecting a kind or instance calls the tab's selection callbacks, which drive the canvas and the
/// mark editor. The tree decides nothing: the core owns which targets exist and every validity rule;
/// the tab owns what a selection means.
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'package:flutter/material.dart';

import '../../../app/app.dart';
import '../../components/surface_panel.dart';
import '../../theme.dart';
import 'calibration_catalog.dart';

class TargetTreePanel extends StatelessWidget {
  const TargetTreePanel({
    super.key,
    required this.selectedKind,
    required this.selectedTarget,
    required this.stored,
    required this.onSelectKind,
    required this.onSelectTarget,
    this.selectedSession,
    this.onSelectSession,
  });

  final String selectedKind;
  final String? selectedTarget;
  final Map<String, CalibrationFetchResult> stored;
  final ValueChanged<String> onSelectKind;
  final void Function(CalibrationKind kind, String target) onSelectTarget;

  /// The session-shaped calibration selected in the target tree (`tuner`, `tooltip` or `pet`), or
  /// null. Selecting one jumps to its card below and marks the row, exactly like selecting a kind
  /// selects the canvas/editor.
  final String? selectedSession;
  final ValueChanged<String>? onSelectSession;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    return SurfacePanel(
      title: 'Targets',
      trailing: Text(
        '${calibrationMarkSlots().length} marks · ${calibrationSessions.length} sessions',
        style: theme.textTheme.labelSmall?.copyWith(
          color: colors.onSurfaceVariant,
        ),
      ),
      padding: const EdgeInsets.all(ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          for (final group in calibrationToolGroups) ...[
            _GroupHeader(
              key: ValueKey('calibrate-tree-group-${group.tool}'),
              group: group,
              stored: stored,
              sessions: calibrationSessions
                  .where((session) => session.tool == group.tool)
                  .toList(growable: false),
            ),
            for (final kind in group.kinds) ...[
              _KindRow(
                kind: kind,
                selected: kind.name == selectedKind,
                fill: calibrationFill(kind, stored),
                savedCount: calibrationSavedCount(kind, stored),
                onTap: () => onSelectKind(kind.name),
              ),
              if (kind.isTargeted && kind.name == selectedKind)
                for (final target in kind.targets!)
                  _TargetRow(
                    kind: kind,
                    target: target,
                    selected: target == selectedTarget,
                    saved:
                        stored[calibrationStoredKey(kind.name, target)]?.mark,
                    rev: stored[calibrationStoredKey(kind.name, target)]?.rev,
                    onTap: () => onSelectTarget(kind, target),
                  ),
            ],
            for (final session in calibrationSessions.where(
              (session) => session.tool == group.tool,
            ))
              _SessionOnlyRow(
                key: ValueKey('calibrate-tree-session-${session.id}'),
                id: session.id,
                label: session.label,
                selected: selectedSession == session.id,
                onTap: onSelectSession == null
                    ? null
                    : () => onSelectSession!(session.id),
                reason: session.reason,
              ),
          ],
        ],
      ),
    );
  }
}

/// One tool's header: its saved/mark count, or `session` when the tool has no single marks (Tuner).
class _GroupHeader extends StatelessWidget {
  const _GroupHeader({
    super.key,
    required this.group,
    required this.stored,
    required this.sessions,
  });

  final CalibrationToolGroup group;
  final Map<String, CalibrationFetchResult> stored;
  final List<CalibrationSession> sessions;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final saved = group.kinds.fold(
      0,
      (n, kind) => n + calibrationSavedCount(kind, stored),
    );
    final meta = group.kinds.isEmpty
        ? '${sessions.length} session${sessions.length == 1 ? '' : 's'}'
        : '$saved / ${group.markCount}';
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        ShellSpace.xs,
        ShellSpace.xs,
        ShellSpace.xs,
        ShellSpace.xxs,
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(
              group.tool,
              style: theme.textTheme.labelSmall?.copyWith(
                color: colors.onSurfaceVariant,
                letterSpacing: 0.7,
              ),
            ),
          ),
          Text(
            meta,
            style: ShellText.mono.copyWith(
              fontSize: 11,
              color: colors.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }
}

/// 7 px status dot; the row's word/count carries the same meaning, colour is never the only channel.
class _StatusDot extends StatelessWidget {
  const _StatusDot(this.fill);

  final CalibrationFill fill;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final color = switch (fill) {
      CalibrationFill.saved => context.shellColors.success,
      CalibrationFill.partial => context.shellColors.warning,
      CalibrationFill.notSet => colors.outline,
    };
    return Container(
      width: 7,
      height: 7,
      decoration: BoxDecoration(color: color, shape: BoxShape.circle),
    );
  }
}

class _KindRow extends StatelessWidget {
  const _KindRow({
    required this.kind,
    required this.selected,
    required this.fill,
    required this.savedCount,
    required this.onTap,
  });

  final CalibrationKind kind;
  final bool selected;
  final CalibrationFill fill;
  final int savedCount;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final meta = switch (fill) {
      CalibrationFill.saved => 'saved',
      CalibrationFill.partial => '$savedCount/${kind.markCount}',
      CalibrationFill.notSet =>
        kind.isTargeted ? '0/${kind.markCount}' : 'not set',
    };
    return _TreeRow(
      key: ValueKey('calibrate-tree-${kind.name}'),
      selected: selected,
      onTap: onTap,
      leading: _StatusDot(fill),
      label: kind.label,
      meta: meta,
      labelStyle: theme.textTheme.bodySmall?.copyWith(
        color: selected ? colors.onPrimaryContainer : colors.onSurfaceVariant,
      ),
      metaStyle: ShellText.mono.copyWith(
        fontSize: 10.5,
        color: selected
            ? colors.onPrimaryContainer.withValues(alpha: 0.8)
            : colors.onSurfaceVariant,
      ),
    );
  }
}

class _TargetRow extends StatelessWidget {
  const _TargetRow({
    required this.kind,
    required this.target,
    required this.selected,
    required this.saved,
    required this.rev,
    required this.onTap,
  });

  final CalibrationKind kind;
  final String target;
  final bool selected;
  final CalibrationMark? saved;
  final int? rev;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    return Padding(
      padding: const EdgeInsets.only(left: ShellSpace.md),
      child: _TreeRow(
        key: ValueKey('calibrate-tree-target-${kind.name}-$target'),
        selected: selected,
        onTap: onTap,
        leading: _StatusDot(
          saved == null ? CalibrationFill.notSet : CalibrationFill.saved,
        ),
        label: target,
        meta: saved == null ? 'not set' : 'rev $rev',
        labelStyle: theme.textTheme.bodySmall?.copyWith(
          color: selected ? colors.onPrimaryContainer : colors.onSurface,
        ),
        metaStyle: ShellText.mono.copyWith(
          fontSize: 10.5,
          color: selected
              ? colors.onPrimaryContainer.withValues(alpha: 0.8)
              : colors.onSurfaceVariant,
        ),
      ),
    );
  }
}

class _SessionOnlyRow extends StatelessWidget {
  const _SessionOnlyRow({
    super.key,
    required this.id,
    required this.label,
    required this.reason,
    required this.selected,
    required this.onTap,
  });

  final String id;
  final String label;
  final String reason;
  final bool selected;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    return Tooltip(
      message: reason,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(ShellRadius.s),
        child: Container(
          margin: const EdgeInsets.symmetric(horizontal: ShellSpace.xxs),
          padding: const EdgeInsets.fromLTRB(
            ShellSpace.sm,
            ShellSpace.xxs,
            ShellSpace.xs,
            ShellSpace.xxs,
          ),
          decoration: BoxDecoration(
            color: selected
                ? colors.primaryContainer.withValues(alpha: 0.5)
                : null,
            borderRadius: BorderRadius.circular(ShellRadius.s),
          ),
          child: Row(
            children: [
              Icon(
                selected ? Icons.play_circle_fill : Icons.play_circle_outline,
                size: 12,
                color: selected ? colors.primary : colors.outline,
              ),
              const SizedBox(width: ShellSpace.xs),
              Expanded(
                child: Text(
                  label,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: selected ? colors.primary : colors.onSurface,
                    fontStyle: selected ? null : FontStyle.italic,
                  ),
                ),
              ),
              Text(
                selected ? 'selected' : 'session',
                key: selected
                    ? ValueKey('calibrate-tree-session-$id-selected')
                    : null,
                style: ShellText.mono.copyWith(
                  fontSize: 10.5,
                  color: selected ? colors.primary : colors.outline,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _TreeRow extends StatelessWidget {
  const _TreeRow({
    super.key,
    required this.selected,
    required this.onTap,
    required this.leading,
    required this.label,
    required this.meta,
    required this.labelStyle,
    required this.metaStyle,
  });

  final bool selected;
  final VoidCallback onTap;
  final Widget leading;
  final String label;
  final String meta;
  final TextStyle? labelStyle;
  final TextStyle? metaStyle;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    return Material(
      color: selected ? colors.primaryContainer : Colors.transparent,
      borderRadius: BorderRadius.circular(ShellRadius.s),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(ShellRadius.s),
        child: Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: ShellSpace.xs,
            vertical: ShellSpace.xs + ShellSpace.xxs,
          ),
          child: Row(
            children: [
              leading,
              const SizedBox(width: ShellSpace.sm),
              Expanded(
                child: Text(
                  label,
                  style: labelStyle,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: ShellSpace.xs),
              Text(meta, style: metaStyle, overflow: TextOverflow.ellipsis),
            ],
          ),
        ),
      ),
    );
  }
}

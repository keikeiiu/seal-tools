/// The Calibrate workspace's tool-scoped target tree: the active tool's presentation marks (Shop 5,
/// Gem 9, Pet 20) with a semantic status dot and a count, and a targeted kind expanding to its named
/// instances (PROTOCOL §12.8).
///
/// The tab owns the scope (`tool`): the tree only renders the kinds that tool consumes, so the left
/// column answers "what can I calibrate here?" instead of listing all four tools at once. Tuner has no
/// single marks; its one-capture OCR+spring session is a tool tab of its own (as is the Pet tooltip),
/// so no session rows live in this tree.
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
    required this.tool,
    required this.selectedKind,
    required this.selectedTarget,
    required this.stored,
    required this.onSelectKind,
    required this.onSelectTarget,
  });

  /// The active calibration tool. The tree is scoped to it: a tool tab shows its own marks only,
  /// so the left column is the "what to calibrate" list for the tool in front of the player.
  final String tool;

  final String selectedKind;
  final String? selectedTarget;
  final Map<String, CalibrationFetchResult> stored;
  final ValueChanged<String> onSelectKind;
  final void Function(CalibrationKind kind, String target) onSelectTarget;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    CalibrationToolGroup? group;
    for (final candidate in calibrationToolGroups) {
      if (candidate.tool == tool) {
        group = candidate;
        break;
      }
    }
    if (group == null) {
      return const SurfacePanel(
        title: 'Targets',
        child: Text('This tool is calibrated from its session tab.'),
      );
    }
    final saved = group.kinds.fold(
      0,
      (n, kind) => n + calibrationSavedCount(kind, stored),
    );
    return SurfacePanel(
      title: '$tool targets',
      trailing: Text(
        '$saved / ${group.markCount} saved',
        style: theme.textTheme.labelSmall?.copyWith(
          color: colors.onSurfaceVariant,
        ),
      ),
      padding: const EdgeInsets.all(ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
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
                  saved: stored[calibrationStoredKey(kind.name, target)]?.mark,
                  rev: stored[calibrationStoredKey(kind.name, target)]?.rev,
                  onTap: () => onSelectTarget(kind, target),
                ),
          ],
        ],
      ),
    );
  }
}

/// One tool's header: its saved/mark count, or `session` when the tool has no single marks (Tuner).
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

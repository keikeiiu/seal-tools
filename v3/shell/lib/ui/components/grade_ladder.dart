/// A read-only grade ladder shared by Magic Tuner and Gem Composer.
///
/// The ladder is fixed by the client/config and is never edited here; the component only explains the
/// current choice. It replaces two unrelated presentations (Tuner's `·`-separated text and Gem's
/// ordinary chips) with one path:
///
///   N → G → DG → …
///
/// Each node can carry one role — `target` (Tuner stops here), `minimum` (the filter's require_grade
/// floor) or `start` (Gem begins here) — and the connectors show which part of the path is active.
/// The roles use icon + word, never colour alone.
library;

import 'package:flutter/material.dart';

import '../theme.dart';

enum GradeLadderRole { target, minimum, start }

class GradeLadder extends StatelessWidget {
  const GradeLadder({
    super.key,
    required this.grades,
    required this.keyPrefix,
    this.caption,
    this.captionKey,
    this.target,
    this.minimum,
    this.start,
    this.readOnlyLabel = 'read-only',
  });

  /// The fixed order, left to right.
  final List<String> grades;

  /// The existing element-key prefix (`grade-order` / `gem-grade`) so the old selectors and tests
  /// keep naming one node per grade.
  final String keyPrefix;

  final String? caption;
  final Key? captionKey;

  /// Tuner's chosen stop grade.
  final String? target;

  /// Tuner's `require_grade` floor.
  final String? minimum;

  /// Gem's chosen start grade.
  final String? start;

  final String readOnlyLabel;

  int? _indexOf(String? grade) {
    if (grade == null) {
      return null;
    }
    final index = grades.indexOf(grade);
    return index < 0 ? null : index;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final targetIndex = _indexOf(target);
    final minimumIndex = _indexOf(minimum);
    final startIndex = _indexOf(start);
    final hasPath = targetIndex != null || startIndex != null;
    final roles = <GradeLadderRole>[
      if (targetIndex != null) GradeLadderRole.target,
      if (minimumIndex != null) GradeLadderRole.minimum,
      if (startIndex != null) GradeLadderRole.start,
    ];

    return Container(
      padding: const EdgeInsets.all(ShellSpace.sm),
      decoration: BoxDecoration(
        color: colors.surfaceContainerLow,
        border: Border.all(color: colors.outlineVariant),
        borderRadius: BorderRadius.circular(ShellRadius.m),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (caption != null) ...[
            Row(
              children: [
                Expanded(
                  child: Text(
                    caption!,
                    key: captionKey,
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: colors.onSurfaceVariant,
                    ),
                  ),
                ),
                Text(
                  readOnlyLabel,
                  style: ShellText.mono.copyWith(
                    fontSize: 9.5,
                    color: colors.onSurfaceVariant,
                  ),
                ),
              ],
            ),
            const SizedBox(height: ShellSpace.xs),
          ],
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            padding: const EdgeInsets.symmetric(vertical: ShellSpace.xxs),
            child: Row(
              children: [
                for (var index = 0; index < grades.length; index++) ...[
                  if (index > 0)
                    Icon(
                      Icons.arrow_right_alt,
                      size: 17,
                      color:
                          hasPath &&
                              _connectorActive(
                                index,
                                targetIndex: targetIndex,
                                startIndex: startIndex,
                              )
                          ? colors.primary
                          : colors.outlineVariant,
                    ),
                  _GradeNode(
                    key: ValueKey('$keyPrefix-${grades[index]}'),
                    grade: grades[index],
                    role: _roleFor(
                      index,
                      targetIndex: targetIndex,
                      minimumIndex: minimumIndex,
                      startIndex: startIndex,
                    ),
                    active: _nodeActive(
                      index,
                      targetIndex: targetIndex,
                      startIndex: startIndex,
                      hasPath: hasPath,
                    ),
                  ),
                ],
              ],
            ),
          ),
          if (roles.isNotEmpty) ...[
            const SizedBox(height: ShellSpace.xs),
            Wrap(
              spacing: ShellSpace.sm,
              runSpacing: ShellSpace.xxs,
              children: [
                if (roles.contains(GradeLadderRole.target))
                  _Legend(
                    key: ValueKey('$keyPrefix-target-legend'),
                    role: GradeLadderRole.target,
                  ),
                if (roles.contains(GradeLadderRole.minimum))
                  _Legend(
                    key: ValueKey('$keyPrefix-minimum-legend'),
                    role: GradeLadderRole.minimum,
                  ),
                if (roles.contains(GradeLadderRole.start))
                  _Legend(
                    key: ValueKey('$keyPrefix-start-legend'),
                    role: GradeLadderRole.start,
                  ),
              ],
            ),
          ],
        ],
      ),
    );
  }

  GradeLadderRole? _roleFor(
    int index, {
    required int? targetIndex,
    required int? minimumIndex,
    required int? startIndex,
  }) {
    if (index == targetIndex) {
      return GradeLadderRole.target;
    }
    if (index == minimumIndex) {
      return GradeLadderRole.minimum;
    }
    if (index == startIndex) {
      return GradeLadderRole.start;
    }
    return null;
  }

  bool _nodeActive(
    int index, {
    required int? targetIndex,
    required int? startIndex,
    required bool hasPath,
  }) {
    if (!hasPath) {
      return false;
    }
    if (startIndex != null) {
      return index >= startIndex;
    }
    if (targetIndex != null) {
      return index <= targetIndex;
    }
    return false;
  }

  bool _connectorActive(
    int nextIndex, {
    required int? targetIndex,
    required int? startIndex,
  }) {
    if (startIndex != null) {
      return nextIndex > startIndex;
    }
    if (targetIndex != null) {
      return nextIndex <= targetIndex;
    }
    return false;
  }
}

class _GradeNode extends StatelessWidget {
  const _GradeNode({
    super.key,
    required this.grade,
    this.role,
    required this.active,
  });

  final String grade;
  final GradeLadderRole? role;
  final bool active;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final roleColor = switch (role) {
      GradeLadderRole.target => colors.primary,
      GradeLadderRole.minimum => theme.shellColors.warning,
      GradeLadderRole.start => theme.shellColors.info,
      null => colors.outlineVariant,
    };
    final background = switch (role) {
      GradeLadderRole.target => colors.primaryContainer,
      GradeLadderRole.minimum => theme.shellColors.warningContainer,
      GradeLadderRole.start => colors.secondaryContainer,
      null => active ? colors.surfaceContainerHigh : colors.surface,
    };
    final icon = switch (role) {
      GradeLadderRole.target => Icons.flag_outlined,
      GradeLadderRole.minimum => Icons.shield_outlined,
      GradeLadderRole.start => Icons.play_arrow_outlined,
      null => null,
    };
    final roleLabel = switch (role) {
      GradeLadderRole.target => 'target',
      GradeLadderRole.minimum => 'minimum',
      GradeLadderRole.start => 'start',
      null => null,
    };
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 54,
          height: 38,
          decoration: BoxDecoration(
            color: background,
            border: Border.all(
              color: role == null ? colors.outlineVariant : roleColor,
            ),
            borderRadius: BorderRadius.circular(ShellRadius.s),
          ),
          alignment: Alignment.center,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (icon != null) ...[
                Icon(icon, size: 11, color: roleColor),
                const SizedBox(width: 2),
              ],
              Text(
                grade,
                style: theme.textTheme.labelLarge?.copyWith(
                  color: role == null ? colors.onSurface : roleColor,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ],
          ),
        ),
        if (roleLabel != null)
          Text(
            roleLabel,
            style: ShellText.mono.copyWith(fontSize: 9, color: roleColor),
          ),
      ],
    );
  }
}

class _Legend extends StatelessWidget {
  const _Legend({super.key, required this.role});

  final GradeLadderRole role;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final (icon, label, color) = switch (role) {
      GradeLadderRole.target => (
        Icons.flag_outlined,
        'Target — the run stops here',
        colors.primary,
      ),
      GradeLadderRole.minimum => (
        Icons.shield_outlined,
        'Minimum — lower grades are skipped',
        theme.shellColors.warning,
      ),
      GradeLadderRole.start => (
        Icons.play_arrow_outlined,
        'Start — the run advances right from here',
        colors.onSurfaceVariant,
      ),
    };
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 12, color: color),
        const SizedBox(width: 3),
        Text(
          label,
          style: theme.textTheme.labelSmall?.copyWith(
            fontSize: 9.5,
            color: colors.onSurfaceVariant,
          ),
        ),
      ],
    );
  }
}

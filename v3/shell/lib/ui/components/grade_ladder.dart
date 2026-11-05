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
    return Container(
      padding: const EdgeInsets.all(ShellSpace.md),
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
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  caption!,
                  key: captionKey,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: colors.onSurfaceVariant,
                  ),
                ),
                const SizedBox(width: ShellSpace.sm),
                Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: ShellSpace.xs,
                    vertical: 1,
                  ),
                  decoration: BoxDecoration(
                    border: Border.all(color: colors.outlineVariant),
                    borderRadius: BorderRadius.circular(ShellRadius.s),
                  ),
                  child: Text(
                    readOnlyLabel,
                    style: ShellText.mono.copyWith(
                      fontSize: 9,
                      color: colors.onSurfaceVariant,
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: ShellSpace.sm),
          ],
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            padding: const EdgeInsets.symmetric(vertical: ShellSpace.xxs),
            child: Row(
              children: [
                for (var index = 0; index < grades.length; index++) ...[
                  if (index > 0)
                    Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: ShellSpace.xxs,
                      ),
                      child: Icon(
                        Icons.arrow_right_alt,
                        size: 18,
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
                    ),
                  _GradeNode(
                    key: ValueKey('$keyPrefix-${grades[index]}'),
                    grade: grades[index],
                    roleKey: ValueKey('$keyPrefix-${grades[index]}-role'),
                    roles: _rolesFor(
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
        ],
      ),
    );
  }

  List<GradeLadderRole> _rolesFor(
    int index, {
    required int? targetIndex,
    required int? minimumIndex,
    required int? startIndex,
  }) => [
    if (index == targetIndex) GradeLadderRole.target,
    if (index == minimumIndex) GradeLadderRole.minimum,
    if (index == startIndex) GradeLadderRole.start,
  ];

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
    required this.roleKey,
    required this.roles,
    required this.active,
  });

  final String grade;
  final Key roleKey;
  final List<GradeLadderRole> roles;
  final bool active;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final hasTarget = roles.contains(GradeLadderRole.target);
    final hasMinimum = roles.contains(GradeLadderRole.minimum);
    final hasStart = roles.contains(GradeLadderRole.start);
    final roleColor = hasTarget
        ? colors.primary
        : hasMinimum
        ? theme.shellColors.warning
        : hasStart
        ? theme.shellColors.info
        : colors.outlineVariant;
    final background = hasTarget
        ? colors.primaryContainer
        : hasMinimum
        ? theme.shellColors.warningContainer
        : hasStart
        ? colors.secondaryContainer
        : active
        ? colors.surfaceContainerHigh
        : colors.surface;
    // When one grade is both target and minimum, the fill carries the target and the warning border
    // carries the floor; no extra icon/word is drawn in the label.
    final borderColor = hasTarget && hasMinimum
        ? theme.shellColors.warning
        : roleColor;
    final rolesText = roles.map(_roleLabel).join(' · ');
    return Semantics(
      key: roleKey,
      excludeSemantics: true,
      label: rolesText.isEmpty ? grade : '$grade · $rolesText',
      child: Container(
        constraints: const BoxConstraints(minWidth: 54, minHeight: 34),
        padding: const EdgeInsets.symmetric(
          horizontal: ShellSpace.sm,
          vertical: ShellSpace.xs,
        ),
        decoration: BoxDecoration(
          color: background,
          border: Border.all(
            color: roles.isEmpty ? colors.outlineVariant : borderColor,
            width: hasTarget && hasMinimum ? 2 : 1,
          ),
          borderRadius: BorderRadius.circular(ShellRadius.s),
        ),
        alignment: Alignment.center,
        child: Text(
          grade,
          style: theme.textTheme.labelLarge?.copyWith(
            color: roles.isEmpty ? colors.onSurface : roleColor,
            fontWeight: FontWeight.w700,
          ),
        ),
      ),
    );
  }

  static String _roleLabel(GradeLadderRole role) => switch (role) {
    GradeLadderRole.target => 'target',
    GradeLadderRole.minimum => 'minimum',
    GradeLadderRole.start => 'start',
  };
}

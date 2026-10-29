/// [SurfacePanel] — the plain panel container (§5 of [PLAN-V3-UI-UX.md]): optional title,
/// description and trailing action over arbitrary content, on the section surface.
///
/// [EditorSection] is the field form of this; use SurfacePanel when the content is not a field group
/// (a readiness banner, a table, a row list, a canvas).
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

import '../theme.dart';

class SurfacePanel extends StatelessWidget {
  const SurfacePanel({
    super.key,
    this.title,
    this.description,
    this.trailing,
    required this.child,
    this.padding = const EdgeInsets.all(ShellSpace.lg),
    this.color,
  });

  final String? title;
  final String? description;
  final Widget? trailing;
  final Widget child;
  final EdgeInsets padding;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final header = title == null && description == null && trailing == null
        ? null
        : Padding(
            padding: const EdgeInsets.only(bottom: ShellSpace.md),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      if (title != null)
                        Text(title!, style: theme.textTheme.titleSmall),
                      if (description != null) ...[
                        const SizedBox(height: ShellSpace.xxs),
                        ConstrainedBox(
                          constraints: const BoxConstraints(
                            maxWidth: ShellWidth.prose,
                          ),
                          child: Text(
                            description!,
                            style: theme.textTheme.bodySmall?.copyWith(
                              color: theme.colorScheme.onSurfaceVariant,
                            ),
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
                const SizedBox(width: ShellSpace.sm),
                if (trailing != null)
                  Flexible(
                    fit: FlexFit.loose,
                    child: Align(
                      alignment: Alignment.topRight,
                      child: trailing,
                    ),
                  ),
              ],
            ),
          );
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.lg),
      child: Material(
        color: color ?? theme.colorScheme.surfaceContainerLow,
        clipBehavior: Clip.antiAlias,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(ShellRadius.m),
          side: BorderSide(
            color: theme.colorScheme.outlineVariant.withValues(alpha: 0.5),
          ),
        ),
        child: Padding(
          padding: padding,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [?header, child],
          ),
        ),
      ),
    );
  }
}

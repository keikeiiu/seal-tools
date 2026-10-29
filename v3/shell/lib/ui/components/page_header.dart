/// [PageHeader] — the screen title block: headline, prose-capped subtitle and a right-side status
/// slot ([PLAN-V3-UI-UX.md] §6 "Config tabs (shared frame)", used by the Run screen first).
///
/// Only presentation: callers pass the text and the trailing widget; the header does not read the
/// model. The subtitle is capped by [ShellProse] so a wrapped sentence stays readable.
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

import 'shell_prose.dart';
import '../theme.dart';

class PageHeader extends StatelessWidget {
  const PageHeader({
    super.key,
    required this.title,
    required this.subtitle,
    this.trailing,
  });

  final String title;
  final String subtitle;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title, style: theme.textTheme.headlineSmall),
              const SizedBox(height: ShellSpace.xs),
              ShellProse(subtitle, style: theme.textTheme.bodyMedium),
            ],
          ),
        ),
        if (trailing != null) ...[
          const SizedBox(width: ShellSpace.lg),
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xxs),
            child: trailing!,
          ),
        ],
      ],
    );
  }
}

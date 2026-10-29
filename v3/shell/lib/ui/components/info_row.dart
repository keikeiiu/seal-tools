/// [InfoRow] — the spec-style `label: value` row (§2.5.4 of [PLAN-V3-UI-UX.md]): a 160 px label
/// column in a quieter role, the value in the body role (monospace for coordinates and counts).
///
/// Replaces the three hand-copied 220 px label columns and the space-padded tables the audit found.
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

import '../theme.dart';

class InfoRow extends StatelessWidget {
  const InfoRow({
    super.key,
    required this.label,
    required this.value,
    this.mono = false,
    this.labelWidth = ShellWidth.labelColumn,
  });

  final String label;
  final String value;
  final bool mono;
  final double labelWidth;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: ShellSpace.xxs),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: labelWidth,
            child: Text(
              label,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: mono
                  ? ShellText.mono.copyWith(color: theme.colorScheme.onSurface)
                  : theme.textTheme.bodySmall?.copyWith(
                      color: theme.colorScheme.onSurface,
                    ),
            ),
          ),
        ],
      ),
    );
  }
}

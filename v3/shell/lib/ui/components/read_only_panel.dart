/// [ReadOnlyPanel] — reference data, collapsed by default and demoted below the editable form
/// (§2.1/§2.5.3 of the plan: the audit found raw-array blocks owning the first screenful).
///
/// Rows are [InfoRow]s; the header states the summary so the player does not have to open it to know
/// whether it is configured.
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

import '../theme.dart';
import 'info_row.dart';

class ReadOnlyPanel extends StatefulWidget {
  const ReadOnlyPanel({
    super.key,
    required this.title,
    required this.rows,
    this.subtitle,
    this.initiallyOpen = false,
    this.openHint,
  });

  final String title;
  final List<InfoRow> rows;
  final String? subtitle;
  final bool initiallyOpen;

  /// The trailing label over the chevron while collapsed (e.g. "34 marks · 10 saved").
  final String? openHint;

  @override
  State<ReadOnlyPanel> createState() => _ReadOnlyPanelState();
}

class _ReadOnlyPanelState extends State<ReadOnlyPanel> {
  late bool _open = widget.initiallyOpen;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.lg),
      child: Material(
        color: colors.surfaceContainerLow,
        clipBehavior: Clip.antiAlias,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(ShellRadius.m),
          side: BorderSide(color: colors.outlineVariant.withValues(alpha: 0.5)),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            InkWell(
              key: ValueKey('readonly-toggle-${widget.title}'),
              onTap: () => setState(() => _open = !_open),
              child: Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: ShellSpace.lg,
                  vertical: ShellSpace.md,
                ),
                child: Row(
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(widget.title, style: theme.textTheme.titleSmall),
                          if (widget.subtitle != null)
                            Text(
                              widget.subtitle!,
                              style: theme.textTheme.bodySmall?.copyWith(
                                color: colors.onSurfaceVariant,
                              ),
                            ),
                        ],
                      ),
                    ),
                    if (widget.openHint != null)
                      Text(
                        widget.openHint!,
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: colors.onSurfaceVariant,
                        ),
                      ),
                    const SizedBox(width: ShellSpace.sm),
                    Icon(
                      _open ? Icons.expand_less : Icons.expand_more,
                      size: ShellSize.iconMd,
                      color: colors.onSurfaceVariant,
                    ),
                  ],
                ),
              ),
            ),
            if (_open)
              Padding(
                padding: const EdgeInsets.fromLTRB(
                  ShellSpace.lg,
                  0,
                  ShellSpace.lg,
                  ShellSpace.md,
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: widget.rows,
                ),
              ),
          ],
        ),
      ),
    );
  }
}

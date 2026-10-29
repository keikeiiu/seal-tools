/// [FoldableSection] — a panel whose fields are folded by default (the demo's "Advanced options"
/// pattern, §6 of [PLAN-V3-UI-UX.md]): the header always shows the title, the body only when open.
///
/// Unlike [ReadOnlyPanel] the content is editable, so the folded header is a full-width button with
/// the chevron; opening it keeps the panel surface contiguous with the form below.
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'package:flutter/material.dart';

import '../theme.dart';

class FoldableSection extends StatefulWidget {
  const FoldableSection({
    super.key,
    required this.title,
    required this.children,
    this.description,
    this.initiallyOpen = false,
  });

  final String title;
  final String? description;
  final List<Widget> children;
  final bool initiallyOpen;

  @override
  State<FoldableSection> createState() => _FoldableSectionState();
}

class _FoldableSectionState extends State<FoldableSection> {
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
              key: ValueKey('fold-toggle-${widget.title}'),
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
                          if (widget.description != null)
                            Text(
                              widget.description!,
                              style: theme.textTheme.bodySmall?.copyWith(
                                color: colors.onSurfaceVariant,
                              ),
                            ),
                        ],
                      ),
                    ),
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
                  ShellSpace.lg,
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: widget.children,
                ),
              ),
          ],
        ),
      ),
    );
  }
}

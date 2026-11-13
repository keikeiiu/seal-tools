/// [LogDrawer] — the 238 px bottom logs drawer (§5/§6 chrome of [PLAN-V3-UI-UX.md]).
///
/// Level badges are coloured from the semantic tokens; All/Errors filters the model's ring buffer.
/// Lines stay distinguishable by origin: core `log` events carry their level, stderr is never parsed
/// and shell notes are the shell talking to itself ([PROTOCOL.md] §1).
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';

enum _LogFilter { all, errors }

class LogDrawer extends StatefulWidget {
  const LogDrawer({
    super.key,
    required this.entries,
    this.onClose,
    this.onOpenLogsFolder,
    this.onExportDiagnostics,
    this.lastRunLogName,
  });

  final List<LogEntry> entries;
  final VoidCallback? onClose;

  /// Opens the install's `logs/` folder through the host seam. Null disables the action (e.g. a
  /// surface with no host).
  final VoidCallback? onOpenLogsFolder;

  /// Writes a support zip under `logs/` and reveals it. Null disables the action.
  final VoidCallback? onExportDiagnostics;

  /// The last run's `<tool>.log`, shown in the action's label so the player knows what they will
  /// find there; null before any run has finished.
  final String? lastRunLogName;

  @override
  State<LogDrawer> createState() => _LogDrawerState();
}

class _LogDrawerState extends State<LogDrawer> {
  _LogFilter _filter = _LogFilter.all;

  bool _isError(LogEntry entry) =>
      entry.level == 'error' || entry.origin == LogOrigin.stderr;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final entries = _filter == _LogFilter.all
        ? widget.entries
        : widget.entries.where(_isError).toList();
    return Container(
      key: const ValueKey('log-panel'),
      height: 238,
      width: double.infinity,
      decoration: BoxDecoration(
        color: colors.surfaceContainerLowest,
        border: Border(
          top: BorderSide(color: colors.outlineVariant.withValues(alpha: 0.7)),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          SizedBox(
            height: 40,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: ShellSpace.md),
              child: Row(
                children: [
                  Text('Logs', style: theme.textTheme.titleSmall),
                  const SizedBox(width: ShellSpace.md),
                  _FilterButton(
                    key: const ValueKey('log-filter-all'),
                    label: 'All',
                    selected: _filter == _LogFilter.all,
                    onTap: () => setState(() => _filter = _LogFilter.all),
                  ),
                  const SizedBox(width: ShellSpace.xs),
                  _FilterButton(
                    key: const ValueKey('log-filter-errors'),
                    label: 'Errors',
                    selected: _filter == _LogFilter.errors,
                    onTap: () => setState(() => _filter = _LogFilter.errors),
                  ),
                  const Spacer(),
                  if (widget.onExportDiagnostics != null)
                    TextButton.icon(
                      key: const ValueKey('export-diagnostics'),
                      onPressed: widget.onExportDiagnostics,
                      icon: const Icon(Icons.save_alt, size: ShellSize.iconSm),
                      label: const Text('Export diagnostics'),
                    ),
                  if (widget.onOpenLogsFolder != null)
                    TextButton.icon(
                      key: const ValueKey('open-logs-folder'),
                      onPressed: widget.onOpenLogsFolder,
                      icon: const Icon(
                        Icons.folder_open,
                        size: ShellSize.iconSm,
                      ),
                      label: Text(
                        widget.lastRunLogName == null
                            ? 'Open logs folder'
                            : 'Open logs folder — ${widget.lastRunLogName}',
                        style: theme.textTheme.labelSmall,
                      ),
                    ),
                  if (widget.onClose != null)
                    IconButton(
                      key: const ValueKey('log-close'),
                      tooltip: 'Close logs',
                      visualDensity: VisualDensity.compact,
                      icon: const Icon(Icons.close, size: ShellSize.iconMd),
                      onPressed: widget.onClose,
                    ),
                ],
              ),
            ),
          ),
          const Divider(height: 1),
          Expanded(
            child: entries.isEmpty
                ? Center(
                    child: Text(
                      _filter == _LogFilter.errors
                          ? 'No error lines'
                          : 'No log lines yet',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: colors.onSurfaceVariant,
                      ),
                    ),
                  )
                // Newest first at the TOP: the model appends (oldest → newest), so the builder
                // walks backwards and the list is not reversed.
                : ListView.builder(
                    padding: const EdgeInsets.fromLTRB(
                      ShellSpace.md,
                      ShellSpace.xs,
                      ShellSpace.md,
                      ShellSpace.sm,
                    ),
                    itemCount: entries.length,
                    itemBuilder: (context, index) {
                      final entry = entries[entries.length - 1 - index];
                      return _LogLine(entry: entry);
                    },
                  ),
          ),
        ],
      ),
    );
  }
}

class _FilterButton extends StatelessWidget {
  const _FilterButton({
    super.key,
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(ShellRadius.s),
      child: Container(
        padding: const EdgeInsets.symmetric(
          horizontal: ShellSpace.sm,
          vertical: ShellSpace.xxs,
        ),
        decoration: BoxDecoration(
          color: selected
              ? theme.colorScheme.primaryContainer
              : Colors.transparent,
          borderRadius: BorderRadius.circular(ShellRadius.s),
        ),
        child: Text(
          label,
          style: theme.textTheme.labelSmall?.copyWith(
            color: selected
                ? theme.colorScheme.onPrimaryContainer
                : theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ),
    );
  }
}

class _LogLine extends StatelessWidget {
  const _LogLine({required this.entry});

  final LogEntry entry;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = context.shellColors;
    final level =
        entry.level ??
        (entry.origin == LogOrigin.stderr
            ? 'error'
            : (entry.origin == LogOrigin.shell ? 'note' : 'info'));
    final origin = switch (entry.origin) {
      LogOrigin.core => '',
      LogOrigin.stderr => 'stderr',
      LogOrigin.shell => 'shell',
    };
    final (background, foreground) = switch (level) {
      'error' => (
        theme.colorScheme.errorContainer,
        theme.colorScheme.onErrorContainer,
      ),
      'warn' => (colors.warningContainer, colors.onWarningContainer),
      _ => (
        theme.colorScheme.surfaceContainerHighest,
        theme.colorScheme.onSurfaceVariant,
      ),
    };
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.xxs),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 44,
            child: Container(
              padding: const EdgeInsets.symmetric(vertical: 1),
              decoration: BoxDecoration(
                color: background,
                borderRadius: BorderRadius.circular(ShellRadius.s),
              ),
              alignment: Alignment.center,
              child: Text(
                level,
                style: theme.textTheme.labelSmall?.copyWith(color: foreground),
              ),
            ),
          ),
          const SizedBox(width: ShellSpace.xs),
          SizedBox(
            width: 42,
            child: origin.isEmpty
                ? null
                : Text(
                    origin,
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
          ),
          const SizedBox(width: ShellSpace.xs),
          Expanded(
            child: Text(
              entry.text,
              style: ShellText.mono.copyWith(
                color: theme.colorScheme.onSurface,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

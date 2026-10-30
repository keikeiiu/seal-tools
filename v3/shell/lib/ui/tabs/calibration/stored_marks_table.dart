/// The Calibrate workspace's stored-coordinate table: one row per actual mark (35), with surface,
/// consuming tool, shape, value in the `x · y` / `x · y · w×h` vocabulary, artifact state and stored
/// status.
///
/// The values come from the existing `calibration.fetch` results (the same map the target tree and
/// the mark editor read); the source of truth stays `local.yaml`, written by the core. A row says
/// "saved" only when the fetch returned a mark — it never claims a captured frame was persisted (a
/// saved mark stores the mark, artifact and environment, never the frame; PROTOCOL §12.7).
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../app/app.dart';
import '../../components/surface_panel.dart';
import '../../theme.dart';
import 'calibration_catalog.dart';

class StoredMarksTablePanel extends StatelessWidget {
  const StoredMarksTablePanel({
    super.key,
    required this.stored,
    required this.storedErrors,
    required this.toolFilter,
    required this.onToolFilterChanged,
    required this.onSelectSlot,
  });

  final Map<String, CalibrationFetchResult> stored;
  final Map<String, String> storedErrors;

  /// `All` or one of [calibrationTools].
  final String toolFilter;
  final ValueChanged<String> onToolFilterChanged;

  /// Tapping a row selects its kind/target in the workspace (drives the canvas and editor).
  final void Function(CalibrationMarkSlot slot) onSelectSlot;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final slots = [
      for (final slot in calibrationMarkSlots())
        if (toolFilter == 'All' || slot.tool == toolFilter) slot,
    ];
    final saved = calibrationSavedMarks(stored);
    final savedInView = slots
        .where((slot) => stored[slot.key]?.mark != null)
        .length;
    return SurfacePanel(
      title: 'Stored coordinates',
      description:
          'Every mark the core serves, in client-relative physical px. `local.yaml` is the source '
          'of truth; values come from `calibration.fetch`. A saved mark stores the mark, its crop '
          'and the display environment — never a frame.',
      trailing: Wrap(
        spacing: ShellSpace.sm,
        runSpacing: ShellSpace.xs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          SizedBox(
            width: ShellWidth.fieldSm,
            child: DropdownButtonHideUnderline(
              child: DropdownButton<String>(
                key: const ValueKey('calibrate-tool-filter'),
                value: toolFilter,
                isExpanded: true,
                isDense: true,
                style: theme.textTheme.bodySmall,
                items: [
                  for (final tool in ['All', ...calibrationTools])
                    DropdownMenuItem(
                      key: ValueKey('calibrate-tool-filter-item-$tool'),
                      value: tool,
                      child: Text('$tool tools'),
                    ),
                ],
                onChanged: (value) {
                  if (value != null) {
                    onToolFilterChanged(value);
                  }
                },
              ),
            ),
          ),
          TextButton.icon(
            key: const ValueKey('calibrate-copy-yaml'),
            onPressed: saved == 0
                ? null
                : () async {
                    // Copying is a read of data the screen already has; it writes nothing and is
                    // not a config import. All saved marks are copied, not just the filtered view.
                    final messenger = ScaffoldMessenger.of(context);
                    await Clipboard.setData(
                      ClipboardData(text: _storedMarksYaml(stored)),
                    );
                    messenger.showSnackBar(
                      SnackBar(content: Text('Copied $saved mark(s) as YAML')),
                    );
                  },
            icon: const Icon(Icons.copy_all_outlined, size: ShellSize.iconSm),
            label: const Text('Copy as YAML'),
          ),
        ],
      ),
      padding: const EdgeInsets.all(ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            '${slots.length} marks in view · $savedInView saved here · $saved of '
            '${calibrationMarkSlots().length} saved · source local.yaml (the core writes it)',
            key: const ValueKey('calibrate-table-count'),
            style: ShellText.mono.copyWith(
              fontSize: 11,
              color: colors.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: ShellSpace.sm),
          LayoutBuilder(
            builder: (context, constraints) {
              // The table never squeezes below a readable width; a narrow window scrolls sideways
              // instead of truncating every value.
              final width = constraints.maxWidth > 900
                  ? constraints.maxWidth
                  : 900.0;
              return SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                child: SizedBox(
                  width: width,
                  child: _MarkTable(
                    slots: slots,
                    stored: stored,
                    storedErrors: storedErrors,
                    onSelectSlot: onSelectSlot,
                  ),
                ),
              );
            },
          ),
        ],
      ),
    );
  }
}

class _MarkTable extends StatelessWidget {
  const _MarkTable({
    required this.slots,
    required this.stored,
    required this.storedErrors,
    required this.onSelectSlot,
  });

  final List<CalibrationMarkSlot> slots;
  final Map<String, CalibrationFetchResult> stored;
  final Map<String, String> storedErrors;
  final void Function(CalibrationMarkSlot slot) onSelectSlot;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final headerStyle = theme.textTheme.labelSmall?.copyWith(
      color: colors.onSurfaceVariant,
      letterSpacing: 0.7,
    );
    Widget header(String label) => Padding(
      padding: const EdgeInsets.fromLTRB(
        ShellSpace.sm,
        ShellSpace.xs,
        ShellSpace.sm,
        ShellSpace.xs,
      ),
      child: Text(label, style: headerStyle),
    );

    return Table(
      columnWidths: const {
        0: FlexColumnWidth(3),
        1: FixedColumnWidth(90),
        2: FixedColumnWidth(70),
        3: FlexColumnWidth(2),
        4: FixedColumnWidth(150),
        5: FixedColumnWidth(120),
      },
      defaultVerticalAlignment: TableCellVerticalAlignment.middle,
      border: TableBorder(
        horizontalInside: BorderSide(color: colors.outlineVariant, width: 0.5),
      ),
      children: [
        TableRow(
          decoration: BoxDecoration(color: colors.surfaceContainerLow),
          children: [
            header('SURFACE'),
            header('TOOL'),
            header('SHAPE'),
            header('MARK (CLIENT-RELATIVE PHYSICAL PX)'),
            header('ARTIFACT'),
            header('STATUS'),
          ],
        ),
        for (final slot in slots)
          TableRow(
            children: [
              _cell(
                theme,
                slot.label,
                key: ValueKey('calibrate-table-row-${slot.key}'),
                onTap: () => onSelectSlot(slot),
                mono: false,
              ),
              _cell(theme, slot.tool),
              _cell(theme, slot.shape),
              _cell(
                theme,
                calibrationMarkValue(stored[slot.key]?.mark),
                mono: true,
              ),
              _cell(theme, _artifactLabel(slot), mono: false),
              _statusCell(theme, slot),
            ],
          ),
      ],
    );
  }

  String _artifactLabel(CalibrationMarkSlot slot) {
    if (!slot.kind.hasArtifact) {
      return '—';
    }
    if (stored[slot.key]?.artifact != null) {
      return 'crop saved';
    }
    if (stored[slot.key]?.mark != null) {
      return 'no crop (empty: false)';
    }
    return 'not set';
  }

  Widget _cell(
    ThemeData theme,
    String text, {
    Key? key,
    VoidCallback? onTap,
    bool mono = false,
  }) {
    final child = Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: ShellSpace.sm,
        vertical: ShellSpace.xs,
      ),
      child: InkWell(
        onTap: onTap,
        child: Text(
          text,
          overflow: TextOverflow.ellipsis,
          style: mono
              ? ShellText.mono.copyWith(
                  fontSize: 11.5,
                  color: theme.colorScheme.onSurface,
                )
              : theme.textTheme.bodySmall,
        ),
      ),
    );
    return key == null ? child : KeyedSubtree(key: key, child: child);
  }

  Widget _statusCell(ThemeData theme, CalibrationMarkSlot slot) {
    final colors = theme.colorScheme;
    final error = storedErrors[slot.key];
    final mark = stored[slot.key]?.mark;
    final (label, color) = error != null
        ? ('fetch failed', colors.error)
        : mark != null
        ? ('saved', theme.shellColors.success)
        : ('not set', colors.outline);
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: ShellSpace.sm,
        vertical: ShellSpace.xs,
      ),
      child: Row(
        children: [
          Container(
            width: 6,
            height: 6,
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
          const SizedBox(width: ShellSpace.xs),
          Flexible(
            child: Text(
              label,
              style: theme.textTheme.labelSmall?.copyWith(color: color),
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }
}

/// The stored marks as a small YAML document the player can paste into an issue or keep beside a
/// backup of `local.yaml`. Values stay integers in client-relative physical px; point kinds have no
/// w/h because the wire form is `[x, y]` (PROTOCOL §12.7).
String _storedMarksYaml(Map<String, CalibrationFetchResult> stored) {
  final buffer = StringBuffer()
    ..writeln('# seal-tools calibration marks (client-relative physical px)')
    ..writeln('# source: calibration.fetch; local.yaml is the source of truth');
  for (final slot in calibrationMarkSlots()) {
    final mark = stored[slot.key]?.mark;
    if (mark == null) {
      continue;
    }
    buffer
      ..writeln("'${slot.key}':")
      ..writeln('  x: ${mark.x}')
      ..writeln('  y: ${mark.y}');
    if (mark.isRect) {
      buffer
        ..writeln('  w: ${mark.w}')
        ..writeln('  h: ${mark.h}');
    }
  }
  return buffer.toString();
}

/// The **Detect** panel: the canonical `calibration.check` answer, rendered.
///
/// This is the one place the shell compares the stored calibration with the live display. Before this
/// panel the header re-derived scale/client-size differences from a capture; the core's answer is now
/// rendered verbatim (G09), including the four states: game closed, nothing stored, matches, differs.
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../../app/core_actions.dart';
import '../../../app/shell_model.dart';
import '../../../protocol/json.dart';
import '../../components/status_chip.dart';
import '../../components/surface_panel.dart';
import '../../theme.dart';

class DetectPanel extends StatefulWidget {
  const DetectPanel({super.key, required this.model});

  final ShellModel model;

  @override
  State<DetectPanel> createState() => _DetectPanelState();
}

class _DetectPanelState extends State<DetectPanel> {
  CoreActionResult<JsonMap>? _result;
  bool _running = false;

  @override
  void initState() {
    super.initState();
    unawaited(_run());
  }

  Future<void> _run() async {
    setState(() => _running = true);
    final result = await widget.model.calibrationCheck();
    if (!mounted) return;
    setState(() {
      _result = result;
      _running = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SurfacePanel(
      key: const ValueKey('detect-panel'),
      title: 'Detect',
      description: 'The core compares the saved calibration with this PC and says whether they match.',
      trailing: TextButton.icon(
        key: const ValueKey('detect-refresh'),
        onPressed: _running ? null : _run,
        icon: const Icon(Icons.refresh, size: ShellSize.iconSm),
        label: const Text('Detect'),
      ),
      child: _running
          ? const Padding(
              padding: EdgeInsets.symmetric(vertical: ShellSpace.sm),
              child: LinearProgressIndicator(),
            )
          : _body(theme),
    );
  }

  Widget _body(ThemeData theme) {
    final result = _result;
    if (result == null) {
      return Text('Not checked yet.', style: theme.textTheme.bodyMedium);
    }
    if (!result.ok) {
      return Text(
        result.errorLine ?? 'calibration.check failed',
        key: const ValueKey('detect-error'),
        style: theme.textTheme.bodyMedium,
      );
    }
    final answer = result.value!;
    final note = answer['note'] as String?;
    final live = _object(answer['live']);
    final stored = _object(answer['stored']);
    final differences = _strings(answer['differences']);
    final matches = answer['matches'] as bool?;

    final chips = <Widget>[];
    if (note != null) {
      chips.add(StatusChip(kind: StatusKind.muted, label: note));
    } else if (matches == null || stored == null) {
      chips.add(
        const StatusChip(
          kind: StatusKind.muted,
          label: 'no stored calibration yet',
        ),
      );
    } else if (matches) {
      chips.add(
        const StatusChip(
          kind: StatusKind.success,
          label: 'stored calibration matches this PC',
        ),
      );
    } else {
      chips.add(
        StatusChip(
          kind: StatusKind.warning,
          label:
              'differs in ${differences.length} '
              '${differences.length == 1 ? "way" : "ways"}',
        ),
      );
    }

    final facts = <String>[
      if (live != null)
        'live scale ${_number(live['scale'])} on '
            '${_size(live['screen'])}'
      else if (note == null)
        'live display unreadable',
      if (stored != null)
        'saved scale ${_number(stored['scale'])}, '
            'DPI ${_number(stored['monitor_dpi'])}, '
            'client ${_size(stored['client_size'])}'
      else
        'nothing saved yet',
      if (stored != null && stored['measured_at'] is String)
        'measured ${stored['measured_at']}',
    ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.xs,
          children: chips,
        ),
        if (differences.isNotEmpty) ...[
          const SizedBox(height: ShellSpace.sm),
          for (final line in differences)
            Text(
              line,
              key: const ValueKey('detect-difference'),
              style: theme.textTheme.bodySmall,
            ),
        ],
        const SizedBox(height: ShellSpace.sm),
        for (final fact in facts) Text(fact, style: theme.textTheme.bodySmall),
      ],
    );
  }

  static JsonMap? _object(Object? value) =>
      value is Map ? value.cast<String, Object?>() : null;

  static List<String> _strings(Object? value) =>
      value is List ? value.map((item) => '$item').toList() : const [];

  static String _number(Object? value) => value is num
      ? (value == value.roundToDouble() ? '${value.toInt()}' : '$value')
      : '?';

  static String _size(Object? value) {
    final object = _object(value);
    if (object == null) return '?';
    final width = object['w'] ?? object['width'];
    final height = object['h'] ?? object['height'];
    return '${_number(width)}×${_number(height)}';
  }
}

/// The Calibrate workspace's session-shaped calibrations panel.
///
/// G01 is wired: **Tooltip offset + shape** drives the three shipped commands
/// (`calibration.tooltip.hover_capture` → `.save` → `.test`). The operator enters the hover point,
/// captures the live panel (shell hidden), reads the box from the picture, enters offset/size and
/// saves; Test read hovers again and shows the OCR lines.
///
/// The tuner three-box and pet-crop sessions are still listed with a disabled action and the reason:
/// their canvas increments are later frontend work, not a protocol gap.
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../../../app/core_actions.dart';
import '../../../app/host_actions.dart';
import '../../../app/shell_model.dart';
import '../../../protocol/json.dart';
import '../../components/status_chip.dart';
import '../../components/surface_panel.dart';
import '../../theme.dart';

class SessionCalibrationsPanel extends StatefulWidget {
  const SessionCalibrationsPanel({
    super.key,
    required this.model,
    required this.host,
  });

  final ShellModel model;
  final HostActions host;

  @override
  State<SessionCalibrationsPanel> createState() =>
      _SessionCalibrationsPanelState();
}

class _SessionCalibrationsPanelState extends State<SessionCalibrationsPanel> {
  final _x = TextEditingController(text: '0');
  final _y = TextEditingController(text: '0');
  final _delay = TextEditingController(text: '700');
  final _offsetX = TextEditingController(text: '0');
  final _offsetY = TextEditingController(text: '0');
  final _width = TextEditingController(text: '0');
  final _height = TextEditingController(text: '0');

  bool _busy = false;
  String? _status;
  bool _failed = false;
  Uint8List? _preview;
  List<String> _lines = const [];

  @override
  void dispose() {
    for (final controller in [
      _x,
      _y,
      _delay,
      _offsetX,
      _offsetY,
      _width,
      _height,
    ]) {
      controller.dispose();
    }
    super.dispose();
  }

  int? _number(TextEditingController controller) =>
      int.tryParse(controller.text.trim());

  Future<void> _run(Future<CoreActionResult<JsonMap>> Function() action) async {
    setState(() {
      _busy = true;
      _status = null;
      _failed = false;
    });
    try {
      await widget.host.hideWindow();
    } catch (_) {}
    final result = await action();
    try {
      await widget.host.showWindow();
    } catch (_) {}
    if (!mounted) return;
    setState(() {
      _busy = false;
      _failed = !result.ok;
      if (!result.ok) {
        _status = result.errorLine;
        return;
      }
      final answer = result.value!;
      final frame = answer['frame'];
      if (frame is Map && frame['data'] is String) {
        try {
          _preview = base64Decode(frame['data'] as String);
        } catch (_) {
          _preview = null;
        }
        final placed = answer['placed'];
        _status =
            'hover captured at '
            '${placed is Map ? '${placed['x']}, ${placed['y']}' : 'the point'}'
            ' — enter the box offset/size from the picture';
        return;
      }
      final rawLines = answer['lines'];
      if (rawLines is List) {
        _lines = [
          for (final line in rawLines)
            if (line is Map) '${line['text'] ?? ''}',
        ];
        _status = _lines.isEmpty
            ? 'the read returned no lines'
            : 'read ${_lines.length} line(s)';
        return;
      }
      _status = 'tooltip saved (rev ${answer['rev'] ?? '?'})';
    });
  }

  Future<void> _hover() => _run(
    () => widget.model.tooltipHoverCapture(
      x: _number(_x) ?? 0,
      y: _number(_y) ?? 0,
      hoverDelayMs: _number(_delay),
    ),
  );

  Future<void> _save() => _run(
    () => widget.model.tooltipSave(
      offsetX: _number(_offsetX) ?? 0,
      offsetY: _number(_offsetY) ?? 0,
      width: _number(_width) ?? 0,
      height: _number(_height) ?? 0,
      hoverDelayMs: _number(_delay),
    ),
  );

  Future<void> _test() => _run(
    () => widget.model.tooltipTest(
      x: _number(_x) ?? 0,
      y: _number(_y) ?? 0,
      hoverDelayMs: _number(_delay),
    ),
  );

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    return SurfacePanel(
      title: 'Session-shaped calibrations',
      description:
          'The tooltip session is wired to the core (§12.9). The tuner three-box and pet-crop '
          'sessions remain later frontend increments.',
      padding: const EdgeInsets.all(ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            spacing: ShellSpace.sm,
            runSpacing: ShellSpace.xs,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Text('Tooltip offset + shape', style: theme.textTheme.labelLarge),
              StatusChip(
                kind: _failed ? StatusKind.warning : StatusKind.success,
                label: _busy ? 'working' : 'wired',
              ),
            ],
          ),
          const SizedBox(height: ShellSpace.xs),
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: ShellWidth.prose),
            child: Text(
              'Capture hover (shell hides), read the panel box from the picture, enter '
              'offset/size, Save, then Test read.',
              style: theme.textTheme.bodySmall?.copyWith(
                color: colors.onSurfaceVariant,
              ),
            ),
          ),
          const SizedBox(height: ShellSpace.sm),
          Wrap(
            spacing: ShellSpace.sm,
            runSpacing: ShellSpace.sm,
            children: [
              _fieldBox('hover x', _x, 'session-tooltip-x'),
              _fieldBox('hover y', _y, 'session-tooltip-y'),
              _fieldBox('delay', _delay, 'session-tooltip-delay'),
              _fieldBox('offset x', _offsetX, 'session-tooltip-offset-x'),
              _fieldBox('offset y', _offsetY, 'session-tooltip-offset-y'),
              _fieldBox('width', _width, 'session-tooltip-width'),
              _fieldBox('height', _height, 'session-tooltip-height'),
            ],
          ),
          const SizedBox(height: ShellSpace.sm),
          Wrap(
            spacing: ShellSpace.sm,
            runSpacing: ShellSpace.xs,
            children: [
              OutlinedButton(
                key: const ValueKey('session-tooltip-hover'),
                onPressed: _busy ? null : _hover,
                child: const Text('Capture hover'),
              ),
              OutlinedButton(
                key: const ValueKey('session-tooltip-save'),
                onPressed: _busy ? null : _save,
                child: const Text('Save tooltip'),
              ),
              OutlinedButton(
                key: const ValueKey('session-tooltip-test'),
                onPressed: _busy ? null : _test,
                child: const Text('Test read'),
              ),
            ],
          ),
          if (_status != null) ...[
            const SizedBox(height: ShellSpace.sm),
            Text(
              _status!,
              key: const ValueKey('session-tooltip-status'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: _failed ? colors.error : colors.onSurface,
              ),
            ),
          ],
          if (_preview != null) ...[
            const SizedBox(height: ShellSpace.sm),
            ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 160),
              child: Image.memory(
                _preview!,
                key: const ValueKey('session-tooltip-preview'),
                fit: BoxFit.contain,
              ),
            ),
          ],
          for (final line in _lines)
            Text(
              line,
              key: const ValueKey('session-tooltip-line'),
              style: theme.textTheme.bodySmall,
            ),
          const SizedBox(height: ShellSpace.md),
          _locked(context, 'tuner', 'Tuner OCR 3-box'),
          _locked(context, 'pet', 'Pet reference crops'),
        ],
      ),
    );
  }

  Widget _fieldBox(
    String label,
    TextEditingController controller,
    String key,
  ) => SizedBox(
    width: 92,
    child: TextField(
      key: ValueKey(key),
      controller: controller,
      decoration: InputDecoration(labelText: label, isDense: true),
      keyboardType: TextInputType.number,
    ),
  );

  Widget _locked(BuildContext context, String id, String title) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.sm),
      child: Row(
        children: [
          Icon(Icons.lock_outline, size: 12, color: theme.colorScheme.outline),
          const SizedBox(width: ShellSpace.sm),
          Expanded(
            child: Text(
              id == 'tuner'
                  ? '$title — the three-box session needs the canvas increment.'
                  : '$title — needs the canvas session increment.',
              key: id == 'tuner'
                  ? const ValueKey('calibrate-tuner-note')
                  : null,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
          OutlinedButton(
            key: ValueKey('calibrate-run-session-$id'),
            onPressed: null,
            child: const Text('Run session'),
          ),
        ],
      ),
    );
  }
}

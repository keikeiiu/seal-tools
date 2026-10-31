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
import '../../../protocol/types.dart';
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

  // G02 tuner session: three boxes on a captured frame.
  final Map<String, TextEditingController> _boxFields = {
    for (final key in [
      'grade.x',
      'grade.y',
      'grade.w',
      'grade.h',
      'attr.x',
      'attr.y',
      'attr.w',
      'attr.h',
      'remaining.x',
      'remaining.y',
      'remaining.w',
      'remaining.h',
    ])
      key: TextEditingController(
        text: key.endsWith('.w') || key.endsWith('.h') ? '100' : '0',
      ),
  };
  final _rowHeight = TextEditingController(text: '0');
  final _springX = TextEditingController(text: '0');
  final _springY = TextEditingController(text: '0');
  CaptureResult? _tunerCapture;
  JsonMap? _tunerCheck;

  // G03 pet session: one row + the shared points, enough to drive preview/save/test_read.
  final Map<String, TextEditingController> _petFields = {
    for (final key in [
      'toggle.x',
      'toggle.y',
      'toggle.w',
      'toggle.h',
      'boarding.x',
      'boarding.y',
      'boarding.w',
      'boarding.h',
      'strip.x',
      'strip.y',
      'strip.w',
      'strip.h',
      'grid.x',
      'grid.y',
      'grid.w',
      'grid.h',
      'slot.x',
      'slot.y',
      'slot.w',
      'slot.h',
      'menu.x',
      'menu.y',
      'feed.x',
      'feed.y',
      'close.x',
      'close.y',
      'tab.x',
      'tab.y',
    ])
      key: TextEditingController(text: '0'),
  };
  final _petStacks = TextEditingController(text: '2');
  CaptureResult? _petCapture;
  JsonMap? _petPreview;

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
    for (final controller in _boxFields.values) {
      controller.dispose();
    }
    for (final controller in _petFields.values) {
      controller.dispose();
    }
    _petStacks.dispose();
    _rowHeight.dispose();
    _springX.dispose();
    _springY.dispose();
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

  Map<String, Map<String, int>> _boxes() {
    final boxes = <String, Map<String, int>>{};
    for (final name in ['grade', 'attr', 'remaining']) {
      boxes[name] = {
        'x': _number(_boxFields['$name.x']!) ?? 0,
        'y': _number(_boxFields['$name.y']!) ?? 0,
        'w': _number(_boxFields['$name.w']!) ?? 0,
        'h': _number(_boxFields['$name.h']!) ?? 0,
      };
    }
    return boxes;
  }

  Future<void> _captureTuner() async {
    setState(() {
      _busy = true;
      _status = null;
      _failed = false;
    });
    try {
      await widget.host.hideWindow();
    } catch (_) {}
    final result = await widget.model.captureFrame();
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
      _tunerCapture = result.value!;
      _status =
          'captured ${result.value!.frame.frameId} — drag the three boxes';
    });
  }

  Future<void> _checkTuner() async {
    final capture = _tunerCapture;
    if (capture == null) {
      setState(() {
        _failed = true;
        _status = 'Capture the screen first.';
      });
      return;
    }
    final result = await widget.model.tunerCheck(
      frameId: capture.frame.frameId,
      boxes: _boxes(),
      rowHeight: _number(_rowHeight),
    );
    if (!mounted) return;
    setState(() {
      _failed = !result.ok;
      if (!result.ok) {
        _status = result.errorLine;
        return;
      }
      _tunerCheck = result.value;
      final grade = result.value!['grade'] ?? '?';
      final remaining = result.value!['remaining'] ?? '?';
      final warnings = result.value!['warnings'];
      _status =
          'check: grade $grade, remaining $remaining'
          '${warnings is List && warnings.isNotEmpty ? ' — ${warnings.first}' : ''}';
    });
  }

  Future<void> _saveTuner() async {
    final capture = _tunerCapture;
    if (capture == null) {
      setState(() {
        _failed = true;
        _status = 'Capture the screen first.';
      });
      return;
    }
    final result = await widget.model.tunerSave(
      frameId: capture.frame.frameId,
      boxes: _boxes(),
      display: capture.display,
      rowHeight: _number(_rowHeight),
      springX: _number(_springX),
      springY: _number(_springY),
    );
    if (!mounted) return;
    setState(() {
      _failed = !result.ok;
      _status = result.ok
          ? 'tuner saved (rev ${result.value!['rev'] ?? '?'})'
          : result.errorLine;
    });
  }

  Widget _tunerSection(ThemeData theme, ColorScheme colors) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.xs,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text('Tuner OCR 3-box', style: theme.textTheme.labelLarge),
            StatusChip(
              kind: _tunerCapture == null
                  ? StatusKind.muted
                  : StatusKind.success,
              label: _tunerCapture == null ? 'needs capture' : 'captured',
            ),
          ],
        ),
        const SizedBox(height: ShellSpace.xs),
        Text(
          'The tuner three-box session: capture the screen, enter grade/attribute/remaining boxes '
          'and Check before Save. The region, bands and row pitch are derived by the core.',
          key: const ValueKey('calibrate-tuner-note'),
          style: theme.textTheme.bodySmall?.copyWith(
            color: colors.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: ShellSpace.sm),
        for (final name in ['grade', 'attr', 'remaining'])
          Padding(
            padding: const EdgeInsets.only(bottom: ShellSpace.xs),
            child: Wrap(
              spacing: ShellSpace.sm,
              runSpacing: ShellSpace.xs,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                SizedBox(
                  width: 70,
                  child: Text(name, style: theme.textTheme.bodySmall),
                ),
                for (final axis in ['x', 'y', 'w', 'h'])
                  _boxField('$name.$axis', _boxFields['$name.$axis']!),
              ],
            ),
          ),
        Wrap(
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.xs,
          children: [
            _fieldBox('row h', _rowHeight, 'session-tuner-row-height'),
            _fieldBox('spring x', _springX, 'session-tuner-spring-x'),
            _fieldBox('spring y', _springY, 'session-tuner-spring-y'),
          ],
        ),
        const SizedBox(height: ShellSpace.sm),
        Wrap(
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.xs,
          children: [
            OutlinedButton(
              key: const ValueKey('session-tuner-capture'),
              onPressed: _busy ? null : _captureTuner,
              child: const Text('Capture screen'),
            ),
            OutlinedButton(
              key: const ValueKey('session-tuner-check'),
              onPressed: _busy ? null : _checkTuner,
              child: const Text('Check boxes'),
            ),
            OutlinedButton(
              key: const ValueKey('session-tuner-save'),
              onPressed: _busy ? null : _saveTuner,
              child: const Text('Save tuner'),
            ),
          ],
        ),
        if (_tunerCheck != null && _tunerCheck!['matched'] is List) ...[
          const SizedBox(height: ShellSpace.sm),
          Text(
            'matched ${(_tunerCheck!['matched'] as List).length} attribute row(s)',
            key: const ValueKey('session-tuner-matched'),
            style: theme.textTheme.bodySmall,
          ),
        ],
      ],
    );
  }

  Widget _boxField(String key, TextEditingController controller) => SizedBox(
    width: 74,
    child: TextField(
      key: ValueKey('session-tuner-$key'),
      controller: controller,
      decoration: InputDecoration(labelText: key, isDense: true),
      keyboardType: TextInputType.number,
    ),
  );

  int? _petNumber(String key) => _number(_petFields[key]!);

  Map<String, num> _rect(String prefix) => {
    'x': _petNumber('$prefix.x') ?? 0,
    'y': _petNumber('$prefix.y') ?? 0,
    'w': _petNumber('$prefix.w') ?? 0,
    'h': _petNumber('$prefix.h') ?? 0,
  };

  List<JsonMap> _petRows() => [
    {
      'toggle_label': _rect('toggle'),
      'boarding_pet_slot': _rect('boarding'),
      'feeder_strip': _rect('strip'),
      'stacks': _number(_petStacks) ?? 2,
    },
  ];

  JsonMap _petPoints() => {
    'menu': _petPoint('menu'),
    'feed': _petPoint('feed'),
    'close': _petPoint('close'),
    'page_tabs': [
      [_petNumber('tab.x') ?? 0, _petNumber('tab.y') ?? 0],
    ],
  };

  /// The save's `points.menu/feed/close` are rects (v2 boxes them with a 10 px box around the
  /// marked point); the panel offers only the point.
  JsonMap _petPoint(String prefix) => {
    'x': _petNumber('$prefix.x') ?? 0,
    'y': _petNumber('$prefix.y') ?? 0,
    'w': 10,
    'h': 10,
  };

  JsonMap _petBag() => {
    'grid': [
      _petNumber('grid.x') ?? 0,
      _petNumber('grid.y') ?? 0,
      _petNumber('grid.w') ?? 0,
      _petNumber('grid.h') ?? 0,
    ],
    'slot': [
      _petNumber('slot.x') ?? 0,
      _petNumber('slot.y') ?? 0,
      _petNumber('slot.w') ?? 0,
      _petNumber('slot.h') ?? 0,
    ],
  };

  Future<void> _capturePet() async {
    setState(() {
      _busy = true;
      _failed = false;
      _status = null;
    });
    try {
      await widget.host.hideWindow();
    } catch (_) {}
    final result = await widget.model.captureFrame();
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
      _petCapture = result.value!;
      _status = 'captured ${result.value!.frame.frameId} — fill the row marks';
    });
  }

  Future<void> _previewPet() async {
    final result = await widget.model.petPreview(
      draft: {
        'rows': _petRows(),
        'bag_grid': _petBag()['grid'],
        'feeder_count_left_fraction': 0.4,
        'feeder_count_min_score': 0.75,
      },
    );
    if (!mounted) return;
    setState(() {
      _failed = !result.ok;
      if (!result.ok) {
        _status = result.errorLine;
        return;
      }
      _petPreview = result.value;
      final rows = result.value!['rows'];
      final problem = result.value!['problem'];
      _status =
          'preview: ${rows is List ? rows.length : 0} row(s)'
          '${problem is String ? ' — $problem' : ''}';
    });
  }

  Future<void> _savePet() async {
    final capture = _petCapture;
    if (capture == null) {
      setState(() {
        _failed = true;
        _status = 'Capture the screen first.';
      });
      return;
    }
    final result = await widget.model.petSave(
      frameId: capture.frame.frameId,
      rows: _petRows(),
      points: _petPoints(),
      bag: _petBag(),
      display: capture.display,
    );
    if (!mounted) return;
    setState(() {
      _failed = !result.ok;
      _status = result.ok
          ? 'pet saved (rev ${result.value!['rev'] ?? '?'})'
          : result.errorLine;
    });
  }

  Future<void> _testPet() async {
    final result = await widget.model.petTestRead(rows: _petRows());
    if (!mounted) return;
    setState(() {
      _failed = !result.ok;
      if (!result.ok) {
        _status = result.errorLine;
        return;
      }
      final rows = result.value!['rows'];
      final firstSlot = rows is List && rows.isNotEmpty && rows.first is Map
          ? (rows.first as Map)['slots']
          : null;
      _status =
          'test read: ${firstSlot is List ? firstSlot.length : 0} slot(s) read';
    });
  }

  Widget _petSection(ThemeData theme, ColorScheme colors) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.xs,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text('Pet crops / rows', style: theme.textTheme.labelLarge),
            StatusChip(
              kind: _petCapture == null ? StatusKind.muted : StatusKind.success,
              label: _petCapture == null ? 'needs capture' : 'captured',
            ),
          ],
        ),
        const SizedBox(height: ShellSpace.xs),
        Text(
          'One row drives the pet session: row rects, the shared points and the bag. Preview '
          'derives the count/ETA regions, Save writes the calibration half + crops, Test read '
          'reports every slot count.',
          style: theme.textTheme.bodySmall?.copyWith(
            color: colors.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: ShellSpace.sm),
        for (final prefix in ['toggle', 'boarding', 'strip', 'grid', 'slot'])
          Padding(
            padding: const EdgeInsets.only(bottom: ShellSpace.xs),
            child: Wrap(
              spacing: ShellSpace.xs,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                SizedBox(
                  width: 70,
                  child: Text(prefix, style: theme.textTheme.bodySmall),
                ),
                for (final axis in ['x', 'y', 'w', 'h'])
                  _boxField('pet.$prefix.$axis', _petFields['$prefix.$axis']!),
              ],
            ),
          ),
        Wrap(
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.xs,
          children: [
            _fieldBox('stacks', _petStacks, 'session-pet-stacks'),
            _fieldBox('menu x', _petFields['menu.x']!, 'session-pet-menu-x'),
            _fieldBox('menu y', _petFields['menu.y']!, 'session-pet-menu-y'),
            _fieldBox('feed x', _petFields['feed.x']!, 'session-pet-feed-x'),
            _fieldBox('feed y', _petFields['feed.y']!, 'session-pet-feed-y'),
            _fieldBox('close x', _petFields['close.x']!, 'session-pet-close-x'),
            _fieldBox('close y', _petFields['close.y']!, 'session-pet-close-y'),
            _fieldBox('tab x', _petFields['tab.x']!, 'session-pet-tab-x'),
            _fieldBox('tab y', _petFields['tab.y']!, 'session-pet-tab-y'),
          ],
        ),
        const SizedBox(height: ShellSpace.sm),
        Wrap(
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.xs,
          children: [
            OutlinedButton(
              key: const ValueKey('session-pet-capture'),
              onPressed: _busy ? null : _capturePet,
              child: const Text('Capture screen'),
            ),
            OutlinedButton(
              key: const ValueKey('session-pet-preview'),
              onPressed: _busy ? null : _previewPet,
              child: const Text('Preview'),
            ),
            OutlinedButton(
              key: const ValueKey('session-pet-save'),
              onPressed: _busy ? null : _savePet,
              child: const Text('Save pet'),
            ),
            OutlinedButton(
              key: const ValueKey('session-pet-test'),
              onPressed: _busy ? null : _testPet,
              child: const Text('Test read'),
            ),
          ],
        ),
        if (_petPreview != null) ...[
          const SizedBox(height: ShellSpace.sm),
          Text(
            'preview rows: ${(_petPreview!['rows'] as List?)?.length ?? 0}',
            key: const ValueKey('session-pet-preview-rows'),
            style: theme.textTheme.bodySmall,
          ),
        ],
      ],
    );
  }

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
          _tunerSection(theme, colors),
          const SizedBox(height: ShellSpace.md),
          _petSection(theme, colors),
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
}

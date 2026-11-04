/// The Calibrate workspace's session-shaped calibrations panel.
///
/// Two sessions remain, each owned by the tool that consumes it, and each draws the CAPTURED FRAME
/// with the shared gesture overlay ([CalibrationFramePicture]):
///
/// - **G01 Tooltip offset + shape** — `calibration.tooltip.hover_capture` → drag a box around the
///   panel on the captured picture → `.save` → `.test`. The offset is the box minus the point the
///   TOOL placed the cursor on (the shell does not read it back), and v2's 20×20 px minimum is the
///   core's own save rule.
/// - **G02 Tuner three-box** — capture the screen, then drag the grade / attribute / remaining boxes
///   IN ORDER on the picture (the target chips pick one explicitly too; a drag under 4 px is a click,
///   not a box). `Check boxes` measures the row pitch, `Save tuner` writes `tuner.ocr`.
///
/// The typed x/y/w/h (and point) fields stay AUTHORITATIVE: a gesture writes them, Save reads them, so
/// nothing lives only in picture state.
library;

import 'package:flutter/material.dart';

import '../../../app/calibration_result.dart';
import '../../../app/config_json.dart';
import '../../../app/core_actions.dart';
import '../../../app/host_actions.dart';
import '../../../app/shell_model.dart';
import '../../../protocol/json.dart';
import '../../../protocol/types.dart';
import '../../components/status_chip.dart';
import '../../components/surface_panel.dart';
import '../../theme.dart';
import '../calibration_widgets.dart';

class SessionCalibrationsPanel extends StatefulWidget {
  const SessionCalibrationsPanel({
    super.key,
    required this.model,
    required this.host,
    this.only,
    this.tooltipKey,
    this.tunerKey,
  });

  final ShellModel model;
  final HostActions host;

  /// Render one tool's session as the whole workspace (`tuner` or `tooltip`). The default null keeps
  /// the combined panel for the all-sessions tests; Calibrate passes the active tab id, so a Tuner or
  /// Tooltip tab is one capture-owned workspace instead of a section below the mark table.
  final String? only;

  /// Section anchors for the combined panel.
  final Key? tooltipKey;
  final Key? tunerKey;

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
  List<String> _lines = const [];

  // G01 tooltip session: the hovered frame and the point the CORE placed the cursor on; the drag is
  // measured from that point (CALIBRATION §1/PLAN §3.5).
  Frame? _tooltipFrame;
  DisplayInfo? _tooltipDisplay;
  Offset? _tooltipPlaced;

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

  /// The tuner surface the next gesture writes: grade → attr → remaining (v2's order), plus the
  /// optional spring POINT.
  String _tunerTarget = 'grade';

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
    final result = await withHiddenWindow(widget.host, action);
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
        Frame? decoded;
        DisplayInfo? display;
        try {
          decoded = Frame.fromJson(frame, 'frame');
          display = answer['display'] is Map
              ? DisplayInfo.fromJson(answer['display'], 'display')
              : null;
        } catch (_) {
          decoded = null;
        }
        if (decoded != null) {
          _tooltipFrame = decoded;
          _tooltipDisplay = display;
          final placed = answer['placed'];
          _tooltipPlaced = placed is Map
              ? Offset(
                  (placed['x'] as num).toDouble(),
                  (placed['y'] as num).toDouble(),
                )
              : null;
          final line = answer['line'];
          _status = line is String && line.isNotEmpty
              ? line
              : 'hover captured — drag a box around the panel on the picture';
          return;
        }
        _status = 'The captured picture could not be decoded.';
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

  // ── session canvas: frames + the gesture→field wiring ──────────────────────────────────────

  /// The client origin of a capture: every mark is CLIENT-RELATIVE physical px, so the picture's
  /// mapping subtracts the client's screen origin (CALIBRATION §4).
  Offset _clientOrigin(DisplayInfo? display) => display == null
      ? Offset.zero
      : Offset(
          display.physicalClient.x.toDouble(),
          display.physicalClient.y.toDouble(),
        );

  /// One session frame with the shared overlay. The callbacks are the only gesture surface this
  /// panel needs: a click writes a point, a drag writes a rect (the 4 px minimum and the letterbox
  /// rejection live in the shared widget, not here).
  Widget _sessionFrame({
    required Key key,
    required Frame frame,
    required DisplayInfo? display,
    required CalibrationMark? mark,
    String? markLabel,
    List<CalibrationOverlayMark> overlayMarks = const [],
    double maxHeight = 260,
    ValueChanged<CalibrationMark>? onPointMark,
    ValueChanged<CalibrationMark>? onRectMark,
  }) => CalibrationFramePicture(
    key: key,
    frame: frame,
    maxHeight: maxHeight,
    clientOrigin: _clientOrigin(display),
    mark: mark,
    markLabel: markLabel,
    overlayMarks: overlayMarks,
    onPointMark: onPointMark,
    onRectMark: onRectMark,
  );

  static const _tunerTargetNames = {
    'grade': 'Grade letter',
    'attr': 'Attribute rows',
    'remaining': 'Remaining count',
    'spring': 'Spring point',
  };

  /// The three boxes keep one colour across the picture, the card and the summary. These are
  /// semantic calibration colours, not page chrome, so they are fixed (and high-contrast on the
  /// captured game frame in both light and dark shells).
  static const Map<String, Color> _tunerBoxColors = {
    'grade': Color(0xFF34D399),
    'attr': Color(0xFF60A5FA),
    'remaining': Color(0xFFF59E0B),
  };
  static const Map<String, String> _tunerBoxHints = {
    'grade': 'one grade letter',
    'attr': 'three attribute rows',
    'remaining': 'the remaining count',
  };
  static const Color _cursorColor = Color(0xFFF472B6);

  /// One of the tuner's three boxes, from its typed (or dragged) fields.
  CalibrationMark? _tunerBoxMark(String name) {
    final x = _number(_boxFields['$name.x']!);
    final y = _number(_boxFields['$name.y']!);
    final w = _number(_boxFields['$name.w']!);
    final h = _number(_boxFields['$name.h']!);
    if (x == null || y == null || w == null || h == null || w < 1 || h < 1) {
      return null;
    }
    return CalibrationMark.rect(x, y, w, h);
  }

  CalibrationMark? _tunerMark() {
    if (_tunerTarget == 'spring') {
      final x = _number(_springX);
      final y = _number(_springY);
      return x == null || y == null ? null : CalibrationMark.point(x, y);
    }
    return _tunerBoxMark(_tunerTarget);
  }

  /// All drawn tuner boxes, including the selected one, so a drag's result stays visible on the
  /// picture even though the selected target is also the editable mark. The selected target is called
  /// out by its highlighted card/chip below the canvas.
  List<CalibrationOverlayMark> _tunerBoxOverlays() {
    final overlays = <CalibrationOverlayMark>[];
    for (final name in ['grade', 'attr', 'remaining']) {
      final mark = _tunerBoxMark(name);
      if (mark == null) {
        continue;
      }
      overlays.add(
        CalibrationOverlayMark(
          id: 'tuner-$name',
          mark: mark,
          label: _tunerTargetNames[name]!,
          color: _tunerBoxColors[name]!,
        ),
      );
    }
    return overlays;
  }

  List<CalibrationOverlayMark> _tooltipOverlays() {
    final placed = _tooltipPlaced;
    if (placed == null) {
      return const [];
    }
    return [
      CalibrationOverlayMark(
        id: 'tooltip-cursor',
        mark: CalibrationMark.point(placed.dx.round(), placed.dy.round()),
        label: 'cursor placed by the tool',
        color: _cursorColor,
      ),
    ];
  }

  String _tunerLabel() {
    final name = _tunerTargetNames[_tunerTarget] ?? _tunerTarget;
    final mark = _tunerMark();
    if (mark == null) {
      return '$name — drag it on the picture';
    }
    return mark.isRect
        ? '$name · x ${mark.x} · y ${mark.y} · ${mark.w}×${mark.h}'
        : '$name · x ${mark.x} · y ${mark.y}';
  }

  /// A drag on the tuner frame writes the selected box, then advances grade → attr → remaining so the
  /// three boxes can be drawn IN ORDER, exactly like v2.
  void _applyTunerRect(CalibrationMark mark) {
    setState(() {
      final target = _tunerTarget;
      if (target == 'spring') {
        return;
      }
      _boxFields['$target.x']!.text = '${mark.x}';
      _boxFields['$target.y']!.text = '${mark.y}';
      _boxFields['$target.w']!.text = '${mark.w}';
      _boxFields['$target.h']!.text = '${mark.h}';
      _tunerTarget = switch (target) {
        'grade' => 'attr',
        'attr' => 'remaining',
        _ => 'remaining',
      };
      _status =
          '${_tunerTargetNames[target]} dragged — '
          '${_tunerTargetNames[_tunerTarget]} is next';
      _failed = false;
    });
  }

  void _applyTunerPoint(CalibrationMark mark) {
    setState(() {
      _springX.text = '${mark.x}';
      _springY.text = '${mark.y}';
      _status = 'spring point ${mark.x}, ${mark.y}';
      _failed = false;
    });
  }

  /// The tooltip box currently in the fields: the placed hover point + the offset/size (CALIBRATION
  /// §1: the panel is anchored to the pointer, so the offset is box minus point).
  CalibrationMark? _tooltipMark() {
    final placed = _tooltipPlaced;
    final offsetX = _number(_offsetX);
    final offsetY = _number(_offsetY);
    final width = _number(_width);
    final height = _number(_height);
    if (placed == null ||
        offsetX == null ||
        offsetY == null ||
        width == null ||
        height == null ||
        width < 1 ||
        height < 1) {
      return null;
    }
    return CalibrationMark.rect(
      placed.dx.round() + offsetX,
      placed.dy.round() + offsetY,
      width,
      height,
    );
  }

  /// v2's tooltip rule: offset = box top-left − the point the TOOL placed the cursor on; size = the
  /// box. The minimum panel size is the core's save rule, not duplicated here.
  void _applyTooltipRect(CalibrationMark mark) {
    final placed = _tooltipPlaced;
    if (placed == null) {
      return;
    }
    setState(() {
      _offsetX.text = '${mark.x - placed.dx.round()}';
      _offsetY.text = '${mark.y - placed.dy.round()}';
      _width.text = '${mark.w}';
      _height.text = '${mark.h}';
      _status =
          'panel box ${mark.w}×${mark.h} — offset '
          '${mark.x - placed.dx.round()}, ${mark.y - placed.dy.round()}';
      _failed = false;
    });
  }

  Future<void> _captureTuner() async {
    setState(() {
      _busy = true;
      _status = null;
      _failed = false;
    });
    final result = await withHiddenWindow(
      widget.host,
      widget.model.captureFrame,
    );
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
          'captured ${result.value!.frame.frameId} — drag the three boxes on the picture '
          '(${_tunerTargetNames[_tunerTarget]} first)';
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

  /// Two-column session workspace: the captured picture stays put on the left while the controls
  /// scroll on the right, so a drag and its target fields are visible together. Narrow layouts keep
  /// the old vertical order because there is no honest way to show both at that width.
  Widget _sessionSplit({
    required Key key,
    required Widget picture,
    required Widget controls,
  }) {
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < 860) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              picture,
              const SizedBox(height: ShellSpace.sm),
              controls,
            ],
          );
        }
        return SizedBox(
          key: key,
          height: 470,
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(flex: 3, child: picture),
              const SizedBox(width: ShellSpace.md),
              Expanded(
                flex: 2,
                child: SingleChildScrollView(primary: false, child: controls),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _sessionPicturePlaceholder(
    ThemeData theme,
    String message, {
    required Key key,
    double height = 380,
  }) {
    final colors = theme.colorScheme;
    return Container(
      key: key,
      height: height,
      padding: const EdgeInsets.all(ShellSpace.md),
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: colors.surfaceContainerLow,
        border: Border.all(color: colors.outlineVariant),
        borderRadius: BorderRadius.circular(ShellRadius.m),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 360),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              Icons.photo_camera_outlined,
              size: 28,
              color: colors.onSurfaceVariant,
            ),
            const SizedBox(height: ShellSpace.xs),
            Text(
              message,
              textAlign: TextAlign.center,
              style: theme.textTheme.bodySmall?.copyWith(
                color: colors.onSurfaceVariant,
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _tooltipSection(ThemeData theme, ColorScheme colors) {
    final captured = _tooltipFrame != null;
    final panel = _tooltipMark();
    final controls = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _sessionCard(
          theme,
          key: const ValueKey('session-tooltip-step-hover'),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  const Icon(Icons.mouse_outlined, size: 16),
                  const SizedBox(width: ShellSpace.xs),
                  Text('1 · Hover point', style: theme.textTheme.labelLarge),
                ],
              ),
              const SizedBox(height: ShellSpace.xxs),
              Text(
                'The point the core places and clicks; the tooltip is anchored to it.',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: colors.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: ShellSpace.xs),
              Wrap(
                spacing: ShellSpace.sm,
                runSpacing: ShellSpace.sm,
                children: [
                  _fieldBox('hover x', _x, 'session-tooltip-x'),
                  _fieldBox('hover y', _y, 'session-tooltip-y'),
                  _fieldBox('delay ms', _delay, 'session-tooltip-delay'),
                ],
              ),
              const SizedBox(height: ShellSpace.xs),
              Align(
                alignment: Alignment.centerLeft,
                child: OutlinedButton.icon(
                  key: const ValueKey('session-tooltip-hover'),
                  onPressed: _busy ? null : _hover,
                  icon: const Icon(Icons.center_focus_strong, size: 16),
                  label: const Text('Capture hover'),
                ),
              ),
            ],
          ),
        ),
        _sessionCard(
          theme,
          key: const ValueKey('session-tooltip-step-box'),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  const Icon(Icons.crop_free, size: 16),
                  const SizedBox(width: ShellSpace.xs),
                  Text(
                    '2 · Panel box & offset',
                    style: theme.textTheme.labelLarge,
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.xxs),
              Text(
                'Drag on the picture (at least 20×20 px), or type the offset and panel size. '
                'Panel top-left = placed cursor + offset.',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: colors.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: ShellSpace.xs),
              Wrap(
                spacing: ShellSpace.sm,
                runSpacing: ShellSpace.sm,
                children: [
                  _fieldBox('offset x', _offsetX, 'session-tooltip-offset-x'),
                  _fieldBox('offset y', _offsetY, 'session-tooltip-offset-y'),
                  _fieldBox('width', _width, 'session-tooltip-width'),
                  _fieldBox('height', _height, 'session-tooltip-height'),
                ],
              ),
              const SizedBox(height: ShellSpace.xs),
              Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: ShellSpace.sm,
                  vertical: ShellSpace.xxs,
                ),
                decoration: BoxDecoration(
                  color: colors.surfaceContainerHighest.withValues(alpha: 0.5),
                  borderRadius: BorderRadius.circular(ShellRadius.s),
                ),
                child: Text(
                  _tooltipFormula(),
                  key: const ValueKey('session-tooltip-offset-summary'),
                  style: ShellText.mono.copyWith(
                    fontSize: 11,
                    color: colors.onSurfaceVariant,
                  ),
                ),
              ),
            ],
          ),
        ),
        Wrap(
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.xs,
          children: [
            OutlinedButton.icon(
              key: const ValueKey('session-tooltip-save'),
              onPressed: _busy ? null : _save,
              icon: const Icon(Icons.save_outlined, size: 16),
              label: const Text('Save tooltip'),
            ),
            OutlinedButton.icon(
              key: const ValueKey('session-tooltip-test'),
              onPressed: _busy ? null : _test,
              icon: const Icon(Icons.text_fields, size: 16),
              label: const Text('Test read'),
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
        if (_lines.isNotEmpty) ...[
          const SizedBox(height: ShellSpace.sm),
          _sessionCard(
            theme,
            key: const ValueKey('session-tooltip-read'),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  children: [
                    const Icon(Icons.subject, size: 16),
                    const SizedBox(width: ShellSpace.xs),
                    Text('Read result', style: theme.textTheme.labelLarge),
                  ],
                ),
                const SizedBox(height: ShellSpace.xxs),
                for (final line in _lines)
                  Padding(
                    padding: const EdgeInsets.only(top: ShellSpace.xxs),
                    child: Text(
                      line,
                      key: const ValueKey('session-tooltip-line'),
                      style: theme.textTheme.bodySmall,
                    ),
                  ),
              ],
            ),
          ),
        ],
      ],
    );
    final picture = captured
        ? _sessionFrame(
            key: const ValueKey('session-tooltip-preview'),
            frame: _tooltipFrame!,
            display: _tooltipDisplay,
            mark: panel,
            markLabel: panel == null
                ? 'Draw the panel around the cursor point'
                : 'Panel · offset from the placed cursor',
            overlayMarks: _tooltipOverlays(),
            maxHeight: 380,
            onRectMark: _applyTooltipRect,
          )
        : _sessionPicturePlaceholder(
            theme,
            'Capture a hover point to put the game screen here. The panel box is drawn next to '
            'these controls, so you can drag without scrolling away from them.',
            key: const ValueKey('session-tooltip-picture-empty'),
          );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.xs,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text('Tooltip offset + shape', style: theme.textTheme.labelLarge),
            StatusChip(
              kind: _failed
                  ? StatusKind.warning
                  : captured
                  ? StatusKind.success
                  : StatusKind.muted,
              label: _busy
                  ? 'working'
                  : captured
                  ? 'hover captured'
                  : 'needs hover',
            ),
          ],
        ),
        const SizedBox(height: ShellSpace.xs),
        ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: ShellWidth.prose),
          child: Text(
            'Two captures, no eyeballed numbers: the core places the cursor on your hover point '
            'and the panel is captured around it. The box you draw is the panel shape; the offset '
            'is box top-left minus the cursor point (v2 rule). Save, then Test read.',
            key: const ValueKey('calibrate-tooltip-note'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.onSurfaceVariant,
            ),
          ),
        ),
        const SizedBox(height: ShellSpace.sm),
        _sessionSteps(
          theme,
          colors,
          const ['Hover capture', 'Box the panel', 'Save + test read'],
          keyPrefix: 'session-tooltip',
          done: [captured, panel != null, _lines.isNotEmpty],
        ),
        const SizedBox(height: ShellSpace.sm),
        _sessionSplit(
          key: const ValueKey('session-tooltip-split'),
          picture: picture,
          controls: controls,
        ),
      ],
    );
  }

  String _tooltipFormula() {
    final placed = _tooltipPlaced;
    if (placed == null) {
      return 'Capture hover first — the cursor point anchors the offset.';
    }
    final offsetX = _number(_offsetX) ?? 0;
    final offsetY = _number(_offsetY) ?? 0;
    final width = _number(_width) ?? 0;
    final height = _number(_height) ?? 0;
    final left = placed.dx.round() + offsetX;
    final top = placed.dy.round() + offsetY;
    return 'cursor (${placed.dx.round()}, ${placed.dy.round()}) '
        '+ offset ($offsetX, $offsetY) → panel ($left, $top) $width×$height';
  }

  Widget _tunerSection(ThemeData theme, ColorScheme colors) {
    final drawn = [
      for (final name in ['grade', 'attr', 'remaining'])
        if (_tunerBoxMark(name) != null) name,
    ];
    final allDrawn = drawn.length == 3;
    final captured = _tunerCapture != null;
    final controls = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final name in ['grade', 'attr', 'remaining'])
          _tunerBoxCard(theme, colors, name),
        const SizedBox(height: ShellSpace.xs),
        _sessionCard(
          theme,
          key: const ValueKey('session-tuner-metrics'),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                'Measured pitch / optional spring point',
                style: theme.textTheme.labelSmall?.copyWith(
                  letterSpacing: 0.05,
                  color: colors.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: ShellSpace.xs),
              Wrap(
                spacing: ShellSpace.sm,
                runSpacing: ShellSpace.xs,
                children: [
                  _fieldBox(
                    'row height',
                    _rowHeight,
                    'session-tuner-row-height',
                  ),
                  _fieldBox('spring x', _springX, 'session-tuner-spring-x'),
                  _fieldBox('spring y', _springY, 'session-tuner-spring-y'),
                ],
              ),
              const SizedBox(height: ShellSpace.xxs),
              Text(
                'Leave row height 0 to let Check measure it. The spring point is optional and '
                'belongs to tuning runs, not to OCR.',
                style: theme.textTheme.labelSmall?.copyWith(
                  fontSize: 9.5,
                  color: colors.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: ShellSpace.sm),
        Wrap(
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.xs,
          children: [
            OutlinedButton.icon(
              key: const ValueKey('session-tuner-check'),
              onPressed: _busy ? null : _checkTuner,
              icon: const Icon(Icons.fact_check_outlined, size: 16),
              label: const Text('Check boxes'),
            ),
            OutlinedButton.icon(
              key: const ValueKey('session-tuner-save'),
              onPressed: _busy ? null : _saveTuner,
              icon: const Icon(Icons.save_outlined, size: 16),
              label: const Text('Save tuner'),
            ),
          ],
        ),
        if (_tunerCheck != null) ...[
          const SizedBox(height: ShellSpace.sm),
          _tunerCheckSummary(theme, colors),
        ],
        if (_status != null) ...[
          const SizedBox(height: ShellSpace.sm),
          Text(
            _status!,
            key: const ValueKey('session-tuner-status'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: _failed ? colors.error : colors.onSurfaceVariant,
            ),
          ),
        ],
      ],
    );
    final picture = captured
        ? _sessionFrame(
            key: const ValueKey('session-tuner-frame'),
            frame: _tunerCapture!.frame,
            display: _tunerCapture!.display,
            mark: _tunerTarget == 'spring' ? _tunerMark() : null,
            markLabel: _tunerTarget == 'spring'
                ? _tunerLabel()
                : 'Drag the selected box on the picture',
            overlayMarks: _tunerBoxOverlays(),
            maxHeight: 380,
            onPointMark: _tunerTarget == 'spring' ? _applyTunerPoint : null,
            onRectMark: _tunerTarget == 'spring' ? null : _applyTunerRect,
          )
        : _sessionPicturePlaceholder(
            theme,
            'Capture the game screen to put it here. The boxes and their fields stay on the '
            'right, so drag the picture and edit the selected box without scrolling away.',
            key: const ValueKey('session-tuner-picture-empty'),
          );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Wrap(
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.xs,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text(
              'Tuner OCR 3-box + Spring point',
              style: theme.textTheme.labelLarge,
            ),
            StatusChip(
              kind: captured ? StatusKind.success : StatusKind.muted,
              label: captured ? 'captured' : 'needs capture',
            ),
            StatusChip(
              kind: allDrawn ? StatusKind.success : StatusKind.muted,
              label: '${drawn.length} / 3 boxes',
            ),
          ],
        ),
        const SizedBox(height: ShellSpace.xs),
        Text(
          'The one tuner capture: the three-box session reads the grade letter, the three '
          'attribute rows and the remaining count, and the optional Spring point saves in the '
          'same pass. Capture the game, drag the three boxes in that order (a drag under 4 px is '
          'ignored), then click Spring point if you use it. Check derives the region, bands and '
          'row pitch; Save writes tuner.ocr and tuner.spring_point.',
          key: const ValueKey('calibrate-tuner-note'),
          style: theme.textTheme.bodySmall?.copyWith(
            color: colors.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: ShellSpace.sm),
        _sessionSteps(
          theme,
          colors,
          const ['Capture the game', 'Draw boxes + spring', 'Check, then save'],
          keyPrefix: 'session-tuner',
          done: [captured, allDrawn, _tunerCheck != null],
        ),
        const SizedBox(height: ShellSpace.sm),
        _sessionCard(
          theme,
          key: const ValueKey('session-tuner-capture-bar'),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Wrap(
                spacing: ShellSpace.sm,
                runSpacing: ShellSpace.xs,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  OutlinedButton.icon(
                    key: const ValueKey('session-tuner-capture'),
                    onPressed: _busy ? null : _captureTuner,
                    icon: const Icon(Icons.photo_camera_outlined, size: 16),
                    label: Text(
                      captured ? 'Recapture screen' : 'Capture screen',
                    ),
                  ),
                  Text(
                    'Game foreground · full client · tuner status window visible',
                    style: theme.textTheme.labelSmall?.copyWith(
                      fontSize: 9.5,
                      color: colors.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.xs),
              Text(
                'Target to drag',
                style: theme.textTheme.labelSmall?.copyWith(
                  letterSpacing: 0.05,
                  color: colors.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: ShellSpace.xxs),
              Wrap(
                spacing: ShellSpace.xs,
                runSpacing: ShellSpace.xs,
                children: [
                  for (final name in _tunerTargetNames.keys)
                    ChoiceChip(
                      key: ValueKey('session-tuner-target-$name'),
                      label: Text(_tunerTargetNames[name]!),
                      selected: _tunerTarget == name,
                      visualDensity: VisualDensity.compact,
                      onSelected: (_) => setState(() => _tunerTarget = name),
                    ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: ShellSpace.xs),
        Text(
          'Drag directly on the captured picture to draw the selected box — the x/y/w/h fields '
          'on the right are an alternative, not a replacement. The drag advances to the next box.',
          key: const ValueKey('session-tuner-drag-hint'),
          style: theme.textTheme.labelSmall?.copyWith(
            fontSize: 10,
            color: colors.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: ShellSpace.xs),
        _sessionSplit(
          key: const ValueKey('session-tuner-split'),
          picture: picture,
          controls: controls,
        ),
      ],
    );
  }

  Widget _tunerBoxCard(ThemeData theme, ColorScheme colors, String name) {
    final mark = _tunerBoxMark(name);
    final selected = _tunerTarget == name;
    final color = _tunerBoxColors[name]!;
    return Container(
      key: ValueKey('session-tuner-box-$name'),
      margin: const EdgeInsets.only(bottom: ShellSpace.xs),
      padding: const EdgeInsets.all(ShellSpace.sm),
      decoration: BoxDecoration(
        color: selected
            ? color.withValues(alpha: 0.10)
            : colors.surfaceContainerLow,
        border: Border.all(
          color: selected
              ? color.withValues(alpha: 0.8)
              : colors.outlineVariant.withValues(alpha: 0.5),
        ),
        borderRadius: BorderRadius.circular(ShellRadius.m),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Container(
                width: 8,
                height: 8,
                decoration: BoxDecoration(color: color, shape: BoxShape.circle),
              ),
              const SizedBox(width: ShellSpace.xs),
              InkWell(
                onTap: () => setState(() => _tunerTarget = name),
                borderRadius: BorderRadius.circular(ShellRadius.s),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: ShellSpace.xxs,
                    vertical: ShellSpace.xxs,
                  ),
                  child: Text(
                    _tunerTargetNames[name]!,
                    style: theme.textTheme.labelLarge?.copyWith(
                      color: selected ? color : null,
                    ),
                  ),
                ),
              ),
              const Spacer(),
              Text(
                mark == null ? 'not drawn' : '${mark.w}×${mark.h}',
                key: ValueKey('session-tuner-$name-status'),
                style: ShellText.mono.copyWith(
                  fontSize: 10.5,
                  color: selected ? color : colors.onSurfaceVariant,
                ),
              ),
            ],
          ),
          const SizedBox(height: ShellSpace.xxs),
          Text(
            _tunerBoxHints[name]!,
            style: theme.textTheme.labelSmall?.copyWith(
              fontSize: 9.5,
              color: colors.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: ShellSpace.xs),
          Wrap(
            spacing: ShellSpace.xs,
            runSpacing: ShellSpace.xs,
            children: [
              for (final axis in ['x', 'y', 'w', 'h'])
                _boxField(
                  '$name.$axis',
                  _boxFields['$name.$axis']!,
                  label: axis,
                ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _tunerCheckSummary(ThemeData theme, ColorScheme colors) {
    final check = _tunerCheck!;
    final geometry = asJsonMap(check['geometry']);
    final region = asJsonMap(geometry['region']);
    final matched = check['matched'] is List
        ? (check['matched'] as List).length
        : 0;
    final pitch = check['measured_row_height'] ?? geometry['row_height'] ?? '—';
    final filter = asJsonMap(check['filter']);
    final filterOn = filter['enabled'] == true;
    final filterPassed = filter['passed'] == true;
    final warnings = check['warnings'] is List
        ? (check['warnings'] as List<Object?>)
        : const <Object?>[];
    final reason = filter['reason'];
    return Container(
      key: const ValueKey('session-tuner-summary'),
      padding: const EdgeInsets.all(ShellSpace.sm),
      decoration: BoxDecoration(
        color: colors.surfaceContainerLow,
        border: Border.all(color: colors.outlineVariant),
        borderRadius: BorderRadius.circular(ShellRadius.m),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(
                child: Text('Check result', style: theme.textTheme.labelLarge),
              ),
              StatusChip(
                kind: warnings.isNotEmpty || (filterOn && !filterPassed)
                    ? StatusKind.warning
                    : StatusKind.success,
                label: warnings.isNotEmpty ? 'check with warnings' : 'derived',
              ),
            ],
          ),
          const SizedBox(height: ShellSpace.xs),
          Wrap(
            spacing: ShellSpace.xs,
            runSpacing: ShellSpace.xs,
            children: [
              _summaryMetric(
                theme,
                'region',
                _regionText(region),
                key: const ValueKey('session-tuner-summary-region'),
              ),
              _summaryMetric(
                theme,
                'row pitch',
                '$pitch px',
                key: const ValueKey('session-tuner-summary-pitch'),
              ),
              _summaryMetric(
                theme,
                'grade',
                '${check['grade'] ?? '—'}',
                key: const ValueKey('session-tuner-summary-grade'),
              ),
              _summaryMetric(
                theme,
                'remaining',
                '${check['remaining'] ?? '—'}',
                key: const ValueKey('session-tuner-summary-remaining'),
              ),
              _summaryMetric(
                theme,
                'filter',
                filterOn ? (filterPassed ? 'passed' : 'failed') : 'off',
                key: const ValueKey('session-tuner-summary-filter'),
              ),
            ],
          ),
          const SizedBox(height: ShellSpace.xs),
          Text(
            'matched $matched attribute row(s)'
            '${reason is String && reason.isNotEmpty ? ' · $reason' : ''}',
            key: const ValueKey('session-tuner-matched'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }

  Widget _summaryMetric(
    ThemeData theme,
    String label,
    String value, {
    Key? key,
  }) {
    return Container(
      key: key,
      padding: const EdgeInsets.symmetric(
        horizontal: ShellSpace.sm,
        vertical: ShellSpace.xxs,
      ),
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(ShellRadius.s),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            label,
            style: theme.textTheme.labelSmall?.copyWith(
              fontSize: 9,
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          Text(value, style: ShellText.mono.copyWith(fontSize: 11.5)),
        ],
      ),
    );
  }

  String _regionText(Map<String, dynamic> region) {
    if (region.isEmpty) {
      return '—';
    }
    final left = region['left'] ?? region['x'] ?? '?';
    final top = region['top'] ?? region['y'] ?? '?';
    final width = region['width'] ?? region['w'] ?? '?';
    final height = region['height'] ?? region['h'] ?? '?';
    return '($left, $top) · $width×$height';
  }

  Widget _sessionSteps(
    ThemeData theme,
    ColorScheme colors,
    List<String> steps, {
    required String keyPrefix,
    required List<bool> done,
  }) {
    return Wrap(
      spacing: ShellSpace.sm,
      runSpacing: ShellSpace.xs,
      children: [
        for (var index = 0; index < steps.length; index++)
          Container(
            key: ValueKey('$keyPrefix-step-${index + 1}'),
            padding: const EdgeInsets.symmetric(
              horizontal: ShellSpace.sm,
              vertical: ShellSpace.xxs,
            ),
            decoration: BoxDecoration(
              color: done[index]
                  ? colors.primaryContainer.withValues(alpha: 0.55)
                  : colors.surfaceContainerLow,
              borderRadius: BorderRadius.circular(ShellRadius.chip),
              border: Border.all(
                color: done[index]
                    ? colors.primary.withValues(alpha: 0.4)
                    : colors.outlineVariant.withValues(alpha: 0.5),
              ),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 16,
                  height: 16,
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                    color: done[index] ? colors.primary : colors.outline,
                    shape: BoxShape.circle,
                  ),
                  child: done[index]
                      ? Icon(Icons.check, size: 11, color: colors.onPrimary)
                      : Text(
                          '${index + 1}',
                          style: theme.textTheme.labelSmall?.copyWith(
                            fontSize: 9,
                            color: colors.surface,
                          ),
                        ),
                ),
                const SizedBox(width: ShellSpace.xs),
                Text(
                  steps[index],
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: done[index]
                        ? colors.primary
                        : colors.onSurfaceVariant,
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }

  Widget _sessionCard(
    ThemeData theme, {
    required Key key,
    required Widget child,
    EdgeInsets padding = const EdgeInsets.all(ShellSpace.sm),
  }) => Container(
    key: key,
    margin: const EdgeInsets.only(bottom: ShellSpace.sm),
    padding: padding,
    decoration: BoxDecoration(
      color: theme.colorScheme.surfaceContainerLow,
      border: Border.all(color: theme.colorScheme.outlineVariant),
      borderRadius: BorderRadius.circular(ShellRadius.m),
    ),
    child: child,
  );

  Widget _boxField(
    String key,
    TextEditingController controller, {
    String? label,
  }) => SizedBox(
    width: 68,
    child: TextField(
      key: ValueKey('session-tuner-$key'),
      controller: controller,
      decoration: InputDecoration(labelText: label ?? key, isDense: true),
      keyboardType: TextInputType.number,
    ),
  );

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    if (widget.only == 'tuner') {
      return SurfacePanel(
        title: 'Magic Tuner',
        description:
            'One tuner capture covers the three OCR boxes and the spring point; Check measures '
            'the frame, then Save writes them together.',
        padding: const EdgeInsets.all(ShellSpace.md),
        child: KeyedSubtree(
          key: widget.tunerKey,
          child: _tunerSection(theme, colors),
        ),
      );
    }
    if (widget.only == 'tooltip') {
      return SurfacePanel(
        title: 'Tooltip offset + shape',
        description:
            'One hover capture: the core places the cursor, captures the panel, and this panel '
            'measures the offset and shape used by the Pet Feeder.',
        padding: const EdgeInsets.all(ShellSpace.md),
        child: KeyedSubtree(
          key: widget.tooltipKey,
          child: _tooltipSection(theme, colors),
        ),
      );
    }
    return SurfacePanel(
      title: 'Sessions',
      description:
          'Only calibrations that need more than one mark: Tuner OCR + Spring point (one capture) '
          'and the Pet tooltip panel. The nine Pet marks are in the mark editor above.',
      padding: const EdgeInsets.all(ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          KeyedSubtree(
            key: widget.tooltipKey,
            child: _tooltipSection(theme, colors),
          ),
          const SizedBox(height: ShellSpace.md),
          KeyedSubtree(
            key: widget.tunerKey,
            child: _tunerSection(theme, colors),
          ),
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

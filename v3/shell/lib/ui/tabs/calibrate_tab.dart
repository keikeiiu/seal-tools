/// The Calibrate tab: capture the game client, draw one of the seven calibrated surfaces, save it, and
/// show what the core has stored.
///
/// This is wave 2's Calibrate surface (PLAN-V3-SHELL §6): the mark is CLIENT-RELATIVE PHYSICAL pixels
/// drawn on the captured full-client frame ([CALIBRATION.md] §1); the core derives the bag-grid pitch,
/// enforces the slot-vs-grid rule, writes the artifact crop and records the measured environment in
/// `local.yaml`. The widget owns no rule: it sends `{frame_id, kind, region|point, display}` (plus
/// v2's `empty: true|false` verdict for the result-gem area, PROTOCOL §12.7) and renders the core's
/// mark/derived/artifact answer or its refusal VERBATIM.
///
/// Capture requires the game to be the foreground window. Hiding the shell is runner work (OPEN.md), so
/// in this build the Capture button can legitimately answer `game_not_foreground`; the line is shown
/// as-is and the player brings the game forward and captures again. There is no hide hack and no
/// auto-retry. A save after another capture is fine (four-frame cache), but an `unknown frame_id` is
/// shown verbatim and requires a NEW Capture — never an automatic re-capture.
///
/// [CALIBRATION.md]: ../../../docs/spec/CALIBRATION.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../protocol/protocol.dart';
import '../theme.dart';
import 'action_result_line.dart';
import 'calibration_widgets.dart';
import 'fields.dart';

/// One served calibration kind: protocol name, player-facing label, shape, whether it owns a crop
/// artifact, and whether its save needs v2's empty-result verdict.
class _Kind {
  const _Kind(
    this.name,
    this.label, {
    required this.isRect,
    this.hasArtifact = false,
    this.requiresEmptyChoice = false,
  });

  final String name;
  final String label;
  final bool isRect;
  final bool hasArtifact;
  final bool requiresEmptyChoice;

  String get shapeLabel => isRect ? 'rectangle (x, y, w, h)' : 'point (x, y)';
}

/// The repaired kind table (PROTOCOL §4/§12.7): `focus_point` is now `scroll_point` (the field the
/// live shop reads) and `tuner_region` is withdrawn until v2's three-box session exists.
const List<_Kind> _kinds = [
  _Kind('bag_grid', 'Bag grid', isRect: true),
  _Kind('bag_slot', 'Bag slot', isRect: true),
  _Kind('shop_list', 'Shop list region', isRect: true),
  _Kind('scroll_point', 'Focus / scroll point', isRect: false),
  _Kind('max_button', 'MAX button', isRect: false),
  _Kind(
    'result_gem_area',
    'Result gem area',
    isRect: true,
    hasArtifact: true,
    requiresEmptyChoice: true,
  ),
  _Kind('spring_point', 'Spring point', isRect: false),
];

class CalibrateTab extends StatefulWidget {
  const CalibrateTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<CalibrateTab> createState() => _CalibrateTabState();
}

class _CalibrateTabState extends State<CalibrateTab>
    with AutomaticKeepAliveClientMixin<CalibrateTab> {
  @override
  bool get wantKeepAlive => true;

  final _x = TextEditingController(text: '0');
  final _y = TextEditingController(text: '0');
  final _w = TextEditingController(text: '0');
  final _h = TextEditingController(text: '0');

  String _kindName = _kinds.first.name;

  CoreActionResult<CaptureResult>? _capture;
  CoreActionResult<CalibrationSaveResult>? _save;
  final Map<String, CalibrationFetchResult> _stored = {};
  final Map<String, String> _storedErrors = {};

  bool _capturing = false;
  bool _saving = false;
  bool _loadingStored = false;

  /// v2's empty-result verdict for the gem result area: null = not answered, Save stays disabled.
  bool? _emptyChoice;

  _Kind get _selected => _kinds.firstWhere((kind) => kind.name == _kindName);

  _Kind? _kindFor(String name) {
    for (final kind in _kinds) {
      if (kind.name == name) {
        return kind;
      }
    }
    return null;
  }

  @override
  void initState() {
    super.initState();
    unawaited(_loadStoredAll());
  }

  @override
  void dispose() {
    _x.dispose();
    _y.dispose();
    _w.dispose();
    _h.dispose();
    super.dispose();
  }

  // ── stored marks ────────────────────────────────────────────────────────────────────────────

  Future<void> _loadStoredAll() async {
    if (!mounted) {
      return;
    }
    setState(() => _loadingStored = true);
    for (final kind in _kinds) {
      await _fetch(kind.name, notify: false);
    }
    if (!mounted) {
      return;
    }
    setState(() {
      _loadingStored = false;
      _fillEditorFromStored();
    });
  }

  Future<void> _fetch(String kindName, {bool notify = true}) async {
    final result = await widget.model.calibrationFetch(kindName);
    if (!mounted) {
      return;
    }
    setState(() {
      if (result.ok) {
        _stored[kindName] = result.value!;
        _storedErrors.remove(kindName);
      } else {
        _storedErrors[kindName] = result.errorLine ?? 'fetch failed';
      }
      if (notify) {
        _fillEditorFromStored();
      }
    });
  }

  /// The editor starts from the stored mark of the selected kind (typed fields are the baseline input;
  /// the frame is drawn to read, not yet an input surface).
  void _fillEditorFromStored() {
    final stored = _stored[_kindName]?.mark;
    if (stored == null) {
      return;
    }
    _x.text = '${stored.x}';
    _y.text = '${stored.y}';
    if (stored.isRect) {
      _w.text = '${stored.w}';
      _h.text = '${stored.h}';
    }
  }

  // ── actions ───────────────────────────────────────────────────────────────────────────────

  Future<void> _captureFrame() async {
    setState(() {
      _capturing = true;
      _save = null;
    });
    final result = await widget.model.captureFrame();
    if (!mounted) {
      return;
    }
    setState(() {
      _capturing = false;
      _capture = result;
    });
  }

  Future<void> _saveMark() async {
    final capture = _capture;
    if (capture == null || !capture.ok) {
      setState(() {
        _save = const CoreActionResult.failure(
          shellNote: 'capture the window first — the mark needs its frame',
        );
      });
      return;
    }
    final kind = _selected;
    if (kind.requiresEmptyChoice && _emptyChoice == null) {
      setState(() {
        _save = const CoreActionResult.failure(
          shellNote: 'answer the empty-result question first — it decides what the save samples',
        );
      });
      return;
    }
    final x = int.tryParse(_x.text.trim());
    final y = int.tryParse(_y.text.trim());
    final w = int.tryParse(_w.text.trim());
    final h = int.tryParse(_h.text.trim());
    if (x == null || y == null || (kind.isRect && (w == null || h == null))) {
      setState(() {
        _save = const CoreActionResult.failure(
          shellNote: 'the mark fields must be whole pixels',
        );
      });
      return;
    }

    setState(() => _saving = true);
    final frame = capture.value!.frame;
    final result = await widget.model.calibrationSave(
      frameId: frame.frameId,
      kind: kind.name,
      region: kind.isRect ? Rect(x: x, y: y, w: w!, h: h!) : null,
      point: kind.isRect ? null : Point(x: x, y: y),
      empty: kind.requiresEmptyChoice ? _emptyChoice : null,
      display: capture.value!.display,
    );
    if (!mounted) {
      return;
    }
    setState(() {
      _saving = false;
      _save = result;
      if (result.ok) {
        _stored[kind.name] = CalibrationFetchResult(
          kind: kind.name,
          mark: result.value!.mark,
          artifact: _stored[kind.name]?.artifact,
          rev: result.value!.rev,
        );
      }
    });
    if (result.ok) {
      await _fetch(kind.name);
    }
  }

  /// The mark currently in the typed fields — drawn on the picture and the ONLY source Save reads.
  CalibrationMark? _editorMark() {
    final x = int.tryParse(_x.text.trim());
    final y = int.tryParse(_y.text.trim());
    if (x == null || y == null) {
      return null;
    }
    if (!_selected.isRect) {
      return CalibrationMark.point(x, y);
    }
    final w = int.tryParse(_w.text.trim());
    final h = int.tryParse(_h.text.trim());
    if (w == null || h == null || w < 1 || h < 1) {
      return null;
    }
    return CalibrationMark.rect(x, y, w, h);
  }

  /// A click on the picture: write the point into the fields (the fields stay authoritative).
  void _applyPointMark(CalibrationMark mark) {
    setState(() {
      _x.text = '${mark.x}';
      _y.text = '${mark.y}';
    });
  }

  /// A drag on the picture: write the rect into the fields.
  void _applyRectMark(CalibrationMark mark) {
    setState(() {
      _x.text = '${mark.x}';
      _y.text = '${mark.y}';
      _w.text = '${mark.w}';
      _h.text = '${mark.h}';
    });
  }

  void _selectKind(String? name) {
    if (name == null || name == _kindName) {
      return;
    }
    setState(() {
      _kindName = name;
      _save = null;
      _emptyChoice = null;
      _fillEditorFromStored();
    });
  }

  // ── UI ─────────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final theme = Theme.of(context);
    final capture = _capture;
    final save = _save;
    final stored = _stored[_kindName];
    final storedError = _storedErrors[_kindName];
    final configuredTitle =
        asString(asJsonMap(widget.model.config.config['window'])['title']) ??
        '— (no title configured)';

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(
            ShellSpace.lg,
            ShellSpace.lg,
            ShellSpace.lg,
            ShellSpace.sm,
          ),
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Calibrate', style: theme.textTheme.titleLarge),
                    const SizedBox(height: 2),
                    Text(
                      'Capture the game client, mark one of the seven surfaces, and save it. '
                      'The core writes the mark and records the measured display.',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
              OutlinedButton.icon(
                key: const ValueKey('calibrate-refresh'),
                onPressed: _loadingStored ? null : _loadStoredAll,
                icon: const Icon(Icons.refresh, size: 18),
                label: const Text('Refresh stored'),
              ),
              const SizedBox(width: ShellSpace.sm),
              FilledButton.icon(
                key: const ValueKey('calibrate-capture'),
                onPressed: _capturing ? null : _captureFrame,
                icon: const Icon(Icons.photo_camera_outlined, size: 18),
                label: const Text('Capture'),
              ),
            ],
          ),
        ),
        Expanded(
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(horizontal: ShellSpace.lg),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                ReadOnlyValue(
                  label: 'Game window title',
                  value: configuredTitle,
                ),
                ReadOnlyValue(
                  label: 'Marks are',
                  value: 'client-relative physical px, on the captured full-client frame',
                ),
                const SizedBox(height: ShellSpace.md),
                if (_capturing)
                  Text(
                    'Capturing the game client…',
                    style: theme.textTheme.bodySmall,
                  ),
                if (capture != null)
                  ActionResultLine(
                    key: const ValueKey('calibrate-capture-result'),
                    label: 'Capture',
                    line: capture.ok
                        ? 'captured frame ${capture.value!.frame.frameId} '
                              '(${capture.value!.frame.width}×${capture.value!.frame.height} px)'
                        : capture.errorLine,
                    isError: !capture.ok,
                  ),
                if (capture == null)
                  Text(
                    'Capture the window to see the frame and draw marks.',
                    key: const ValueKey('calibrate-capture-first'),
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
                if (capture != null && capture.ok) ...[
                  CalibrationFramePicture(
                    key: const ValueKey('calibrate-frame'),
                    frame: capture.value!.frame,
                    mark: _editorMark(),
                    clientOrigin: Offset(
                      capture.value!.display.physicalClient.x.toDouble(),
                      capture.value!.display.physicalClient.y.toDouble(),
                    ),
                    onPointMark: _selected.isRect ? null : _applyPointMark,
                    onRectMark: _selected.isRect ? _applyRectMark : null,
                  ),
                  const SizedBox(height: ShellSpace.sm),
                  ActionResultLine(
                    key: const ValueKey('calibrate-display'),
                    label: 'Display',
                    line: displaySummary(capture.value!.display),
                  ),
                  const SizedBox(height: ShellSpace.lg),
                ],
                EditorSection(
                  title: 'Mark',
                  children: [
                    Wrap(
                      spacing: ShellSpace.md,
                      runSpacing: ShellSpace.sm,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: [
                        SizedBox(
                          width: 260,
                          child: InputDecorator(
                            decoration: const InputDecoration(
                              labelText: 'Kind',
                              isDense: true,
                              border: OutlineInputBorder(),
                            ),
                            child: DropdownButtonHideUnderline(
                              child: DropdownButton<String>(
                                key: const ValueKey('calibrate-kind'),
                                value: _kindName,
                                isExpanded: true,
                                isDense: true,
                                items: [
                                  for (final kind in _kinds)
                                    DropdownMenuItem(
                                      key: ValueKey(
                                        'calibrate-kind-item-${kind.name}',
                                      ),
                                      value: kind.name,
                                      child: Text(
                                        '${kind.label} (${kind.name})',
                                      ),
                                    ),
                                ],
                                onChanged: _selectKind,
                              ),
                            ),
                          ),
                        ),
                        Text(
                          '${_selected.label} · ${_selected.shapeLabel} · '
                          'client-relative physical px'
                          '${_selected.hasArtifact ? ' · writes a crop artifact' : ''}',
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: ShellSpace.sm),
                    Wrap(
                      spacing: ShellSpace.sm,
                      runSpacing: ShellSpace.sm,
                      children: [
                        _pixelField('calibrate-x', 'x', _x),
                        _pixelField('calibrate-y', 'y', _y),
                        if (_selected.isRect) ...[
                          _pixelField('calibrate-w', 'w', _w),
                          _pixelField('calibrate-h', 'h', _h),
                        ],
                      ],
                    ),
                    const SizedBox(height: ShellSpace.sm),
                    if (_selected.requiresEmptyChoice) ...[
                      Text(
                        "v2's empty-result question — answer it before saving:",
                        style: theme.textTheme.labelMedium,
                      ),
                      const SizedBox(height: ShellSpace.xs),
                      Wrap(
                        spacing: ShellSpace.sm,
                        runSpacing: ShellSpace.xs,
                        children: [
                          ChoiceChip(
                            key: const ValueKey('calibrate-empty-yes'),
                            label: const Text(
                              'The result box is EMPTY right now',
                            ),
                            selected: _emptyChoice == true,
                            onSelected: (_) =>
                                setState(() => _emptyChoice = true),
                          ),
                          ChoiceChip(
                            key: const ValueKey('calibrate-empty-no'),
                            label: const Text('A gem is in the box'),
                            selected: _emptyChoice == false,
                            onSelected: (_) =>
                                setState(() => _emptyChoice = false),
                          ),
                        ],
                      ),
                      const SizedBox(height: ShellSpace.xs),
                      Text(
                        'Empty saves the crop and samples the colour signature. A gem in the box '
                        'samples only the colour fallback — the strong crop test is not created.',
                        key: const ValueKey('calibrate-empty-hint'),
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                      const SizedBox(height: ShellSpace.sm),
                    ],
                    FilledButton(
                      key: const ValueKey('calibrate-save'),
                      onPressed:
                          _saving ||
                              (_selected.requiresEmptyChoice &&
                                  _emptyChoice == null)
                          ? null
                          : _saveMark,
                      child: _saving
                          ? const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Text('Save mark'),
                    ),
                    const SizedBox(height: ShellSpace.sm),
                    if (save != null)
                      ActionResultLine(
                        key: const ValueKey('calibrate-save-result'),
                        label: 'Save',
                        line: save.ok
                            ? 'saved ${save.value!.kind} at rev ${save.value!.rev} · '
                                  'mark [${save.value!.mark.label}]'
                            : save.errorLine,
                        isError: !save.ok,
                      ),
                    if (save != null && save.ok) ...[
                      if (save.value!.pitchLine != null)
                        ActionResultLine(
                          key: const ValueKey('calibrate-pitch'),
                          label: 'Derived',
                          line: save.value!.pitchLine,
                        ),
                      if (save.value!.artifact != null)
                        ActionResultLine(
                          key: const ValueKey('calibrate-artifact'),
                          label: 'Artifact',
                          line:
                              '${save.value!.artifact!.file} · '
                              '${save.value!.artifact!.width}×'
                              '${save.value!.artifact!.height} px · '
                              '${save.value!.artifact!.path ?? ''}',
                        ),
                    ],
                  ],
                ),
                EditorSection(
                  title: 'Tuner OCR geometry — not calibratable here yet',
                  children: [
                    Text(
                      "The tuner's OCR region, grade box and row bands are measured as v2's "
                      'three-box session (grade letter, attribute rows, remaining time) and the '
                      'core derives the region from them. That session is not built in v3 yet, so '
                      'no tuner mark is offered here.',
                      key: const ValueKey('calibrate-tuner-note'),
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
                EditorSection(
                  title: 'Stored mark — $_kindName',
                  children: [
                    if (storedError != null)
                      Text(
                        storedError,
                        key: const ValueKey('calibrate-stored-error'),
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.error,
                        ),
                      )
                    else if (stored == null || stored.mark == null)
                      Text(
                        'Not calibrated yet',
                        key: const ValueKey('calibrate-not-calibrated'),
                        style: theme.textTheme.bodySmall,
                      )
                    else ...[
                      Text(
                        stored.kind == _kindName
                            ? stored.storedLine
                            : 'core returned kind "${stored.kind}" · '
                                  '${stored.storedLine}',
                        key: const ValueKey('calibrate-stored'),
                        style: theme.textTheme.bodyMedium,
                      ),
                      if (stored.artifact != null) ...[
                        const SizedBox(height: ShellSpace.sm),
                        CalibrationCropPicture(
                          key: const ValueKey('calibrate-crop'),
                          file: stored.artifact!.file,
                          pngBase64: stored.artifact!.pngBase64,
                        ),
                      ],
                    ],
                  ],
                ),
                const SizedBox(height: ShellSpace.lg),
              ],
            ),
          ),
        ),
      ],
    );
  }

  Widget _pixelField(
    String key,
    String label,
    TextEditingController controller,
  ) {
    return SizedBox(
      width: 130,
      child: TextField(
        key: ValueKey(key),
        controller: controller,
        keyboardType: TextInputType.number,
        decoration: InputDecoration(
          labelText: '$label (px)',
          isDense: true,
          border: const OutlineInputBorder(),
        ),
      ),
    );
  }
}

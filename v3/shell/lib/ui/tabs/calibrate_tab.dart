/// The Calibrate tab: capture the game client, draw one of the calibrated surfaces, save it, and show
/// what the core has stored.
///
/// This is wave 2's Calibrate surface (PLAN-V3-SHELL §6): the mark is CLIENT-RELATIVE PHYSICAL pixels
/// drawn on the captured full-client frame ([CALIBRATION.md] §1); the core derives the bag-grid pitch,
/// enforces the slot-vs-grid rule, writes the artifact crop and records the measured environment in
/// `local.yaml`. The widget owns no rule: it sends `{frame_id, kind, region|point, display}` (plus a
/// named-instance `target`, PROTOCOL §12.8, and v2's `empty: true|false` verdict for the result-gem
/// area, PROTOCOL §12.7) and renders the core's mark/derived/artifact answer or its refusal VERBATIM.
///
/// Capture hides the shell, waits one frame, asks the core for the frame, then shows the shell again.
/// The show is UNCONDITIONAL — a refused capture must never leave the shell hidden. The core still
/// requires the game to be the foreground window, which the hide gives it; if the host channel is
/// absent the capture is still attempted and its refusal/line is shown verbatim. A save after another
/// capture is fine (four-frame cache), but an `unknown frame_id` is shown verbatim and requires a NEW
/// Capture — never an automatic re-capture.
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

/// One served calibration kind: protocol name, picker group, player-facing label, shape, whether it
/// owns a crop artifact, whether its save needs v2's empty-result verdict, and the spec's allowed
/// `target` values when the kind marks a NAMED INSTANCE (PROTOCOL §12.8).
class _Kind {
  const _Kind(
    this.name,
    this.group,
    this.label, {
    required this.isRect,
    this.hasArtifact = false,
    this.requiresEmptyChoice = false,
    this.targets,
  });

  final String name;
  final String group;
  final String label;
  final bool isRect;
  final bool hasArtifact;
  final bool requiresEmptyChoice;

  /// Null for a single mark; otherwise the presentation-side allowed values (1-based/player-facing).
  /// The CORE owns validity and its refusal lines; these are only the picker's choices.
  final List<String>? targets;

  bool get isTargeted => targets != null;

  String get shapeLabel => isRect ? 'rectangle (x, y, w, h)' : 'point (x, y)';
}

/// Named-instance targets (PROTOCOL §12.8), kept here as presentation constants only.
const List<String> _petPageTargets = ['ITEM1', 'ITEM2', 'ITEM3'];
const List<String> _petRowTargets = ['1', '2', '3', '4'];
const List<String> _gemGradeTargets = ['N', 'G', 'DG', 'Register', 'Combine'];
const List<String> _gemResourceTargets = ['1', '2', '3'];

/// The repaired + named-instance kind table (PROTOCOL §4/§12.7/§12.8). Groups only order the picker;
/// `focus_point`/`tuner_region` stay withdrawn.
const List<_Kind> _kinds = [
  // Shop
  _Kind('bag_grid', 'Shop', 'Bag grid', isRect: true),
  _Kind('bag_slot', 'Shop', 'Bag slot', isRect: true),
  _Kind('shop_list', 'Shop', 'Shop list region', isRect: true),
  _Kind('scroll_point', 'Shop', 'Focus / scroll point', isRect: false),
  _Kind('max_button', 'Shop', 'MAX button', isRect: false),
  // Gem
  _Kind(
    'result_gem_area',
    'Gem',
    'Result gem area',
    isRect: true,
    hasArtifact: true,
    requiresEmptyChoice: true,
  ),
  _Kind(
    'gem_grade',
    'Gem',
    'Grade point',
    isRect: false,
    targets: _gemGradeTargets,
  ),
  _Kind(
    'gem_resource',
    'Gem',
    'Resource slot point',
    isRect: false,
    targets: _gemResourceTargets,
  ),
  // Pet
  _Kind('pet_menu', 'Pet — points', '目錄 (menu) point', isRect: false),
  _Kind('pet_feed', 'Pet — points', 'Pet feed icon point', isRect: false),
  _Kind('pet_close', 'Pet — points', 'Boarding X point', isRect: false),
  _Kind(
    'pet_page_tab',
    'Pet — points',
    'Bag page tab point',
    isRect: false,
    targets: _petPageTargets,
  ),
  _Kind('pet_bag_grid', 'Pet — bag', 'Boarding bag grid', isRect: true),
  _Kind('pet_bag_slot', 'Pet — bag', 'Boarding bag slot', isRect: true),
  _Kind(
    'pet_toggle',
    'Pet — rows',
    'Row start/end button',
    isRect: true,
    targets: _petRowTargets,
  ),
  _Kind(
    'pet_boarding_slot',
    'Pet — rows',
    'Row boarding pet slot',
    isRect: true,
    targets: _petRowTargets,
  ),
  _Kind(
    'pet_feeder_strip',
    'Pet — rows',
    'Row food strip',
    isRect: true,
    targets: _petRowTargets,
  ),
  // Tuner
  _Kind('spring_point', 'Tuner', 'Spring point', isRect: false),
];

class CalibrateTab extends StatefulWidget {
  const CalibrateTab({
    super.key,
    required this.model,
    this.host = const MethodChannelHostActions(),
  });

  final ShellModel model;

  /// The runner host's window seam; tests inject a fake, production uses seal/host.
  final HostActions host;

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

  /// A host-side note (e.g. the window channel is absent) shown under the capture result.
  String? _hostNote;

  /// v2's empty-result verdict for the gem result area: null = not answered, Save stays disabled.
  bool? _emptyChoice;

  /// The NAMED INSTANCE for a targeted kind (PROTOCOL §12.8); null until the player picks one.
  String? _target;

  _Kind get _selected => _kinds.firstWhere((kind) => kind.name == _kindName);

  /// Stored marks are keyed by kind AND instance: a targeted kind has one mark per target.
  String _storedKey(String kind, String? target) =>
      target == null ? kind : '$kind@$target';

  String get _currentStoredKey => _storedKey(_kindName, _target);

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
    // Targeted kinds need a target, and the core refuses a fetch without one, so only the single-mark
    // kinds are loaded up front. The selected kind's instance is fetched when a target is chosen.
    for (final kind in _kinds.where((kind) => !kind.isTargeted)) {
      await _fetch(kind.name, notify: false);
    }
    final currentTarget = _target;
    if (_selected.isTargeted && currentTarget != null) {
      await _fetch(_kindName, target: currentTarget, notify: false);
    }
    if (!mounted) {
      return;
    }
    setState(() {
      _loadingStored = false;
      _fillEditorFromStored();
    });
  }

  Future<void> _fetch(
    String kindName, {
    String? target,
    bool notify = true,
  }) async {
    final result = await widget.model.calibrationFetch(
      kindName,
      target: target,
    );
    if (!mounted) {
      return;
    }
    final key = _storedKey(kindName, target);
    setState(() {
      if (result.ok) {
        _stored[key] = result.value!;
        _storedErrors.remove(key);
      } else {
        _storedErrors[key] = result.errorLine ?? 'fetch failed';
      }
      if (notify) {
        _fillEditorFromStored();
      }
    });
  }

  /// The editor starts from the stored mark of the selected kind/instance (typed fields are the
  /// baseline input; the frame is drawn to read, not yet an input surface).
  void _fillEditorFromStored() {
    final stored = _stored[_currentStoredKey]?.mark;
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

  /// hide → one frame → capture → show, with the show UNCONDITIONAL. The hide's reply is awaited
  /// before the frame wait so the core sees the game as the foreground window; a missing host
  /// channel is a note, not a failure — the capture is still attempted and its line rendered.
  Future<void> _captureFrame() async {
    setState(() {
      _capturing = true;
      _save = null;
      _hostNote = null;
    });
    var hostNote = '';
    try {
      try {
        await widget.host.hideWindow();
      } catch (_) {
        hostNote = 'the shell could not hide itself — the game has to be in front for a capture';
      }
      // One frame for the hide to be composited before the core checks the foreground window.
      await _waitOneFrame();
      final result = await widget.model.captureFrame();
      if (!mounted) {
        return;
      }
      setState(() {
        _capturing = false;
        _capture = result;
        _hostNote = hostNote.isEmpty ? null : hostNote;
      });
    } finally {
      // A refused capture (or a thrown error) must never leave the shell hidden.
      try {
        await widget.host.showWindow();
      } catch (_) {
        // No host channel (tests/other platforms): the window was never hidden, but say so rather
        // than failing the capture — the frame/refusal above still stands.
        if (mounted) {
          setState(() {
            _hostNote ??=
                'the shell could not be shown again — no host channel';
          });
        }
      }
    }
  }

  /// One frame for the compositor to apply the hide — but never longer than ~50 ms, because a hidden
  /// window may stop producing frames on some drivers and the capture must not hang on the lifecycle
  /// wait. In widget tests the frame completes on the next pump, which cancels the fallback timer.
  Future<void> _waitOneFrame() async {
    final waited = Completer<void>();
    final fallback = Timer(const Duration(milliseconds: 50), () {
      if (!waited.isCompleted) {
        waited.complete();
      }
    });
    unawaited(
      WidgetsBinding.instance.endOfFrame.whenComplete(() {
        fallback.cancel();
        if (!waited.isCompleted) {
          waited.complete();
        }
      }),
    );
    await waited.future;
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
    if (kind.isTargeted && _target == null) {
      setState(() {
        _save = const CoreActionResult.failure(
          shellNote: 'choose the named instance first — a targeted mark needs its target',
        );
      });
      return;
    }
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
      target: kind.isTargeted ? _target : null,
      display: capture.value!.display,
    );
    if (!mounted) {
      return;
    }
    final key = _currentStoredKey;
    setState(() {
      _saving = false;
      _save = result;
      if (result.ok) {
        _stored[key] = CalibrationFetchResult(
          kind: kind.name,
          target: result.value!.target,
          mark: result.value!.mark,
          artifact: _stored[key]?.artifact,
          rev: result.value!.rev,
        );
      }
    });
    if (result.ok) {
      await _fetch(kind.name, target: kind.isTargeted ? _target : null);
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
      // Switching kind never carries a target over: the new kind's instances are its own.
      _target = null;
      _fillEditorFromStored();
    });
  }

  /// The named instance changed: fetch that instance's stored mark; a targeted kind without a target
  /// shows no stored mark and keeps Save disabled.
  void _onTargetChanged(String? target) {
    setState(() {
      _target = target;
      _save = null;
    });
    if (target != null) {
      unawaited(_fetch(_kindName, target: target));
    }
  }

  // ── UI ─────────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final theme = Theme.of(context);
    final capture = _capture;
    final save = _save;
    final stored = _stored[_currentStoredKey];
    final storedError = _storedErrors[_currentStoredKey];
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
                      'Capture the game client, mark one of the calibrated surfaces, and save it. '
                      'The shell hides for the grab, waits one frame, and reopens after it; the core '
                      'writes the mark and records the measured display.',
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
                if (_hostNote != null)
                  Text(
                    _hostNote!,
                    key: const ValueKey('calibrate-host-note'),
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
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
                                        '${kind.group} · ${kind.label} '
                                        '(${kind.name})',
                                      ),
                                    ),
                                ],
                                onChanged: _selectKind,
                              ),
                            ),
                          ),
                        ),
                        Text(
                          '${_selected.group} · ${_selected.label} · '
                          '${_selected.shapeLabel} · client-relative physical px'
                          '${_selected.hasArtifact ? ' · writes a crop artifact' : ''}'
                          '${_selected.isTargeted ? ' · named instance' : ''}',
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                    if (_selected.isTargeted) ...[
                      const SizedBox(height: ShellSpace.sm),
                      Wrap(
                        spacing: ShellSpace.md,
                        runSpacing: ShellSpace.sm,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: [
                          SizedBox(
                            width: ShellWidth.fieldLg,
                            child: InputDecorator(
                              decoration: const InputDecoration(
                                labelText: 'Target instance',
                                isDense: true,
                                border: OutlineInputBorder(),
                              ),
                              child: DropdownButtonHideUnderline(
                                child: DropdownButton<String>(
                                  key: const ValueKey('calibrate-target'),
                                  value: _target,
                                  isExpanded: true,
                                  isDense: true,
                                  hint: const Text('Choose a target'),
                                  items: [
                                    for (final target
                                        in _selected.targets ??
                                            const <String>[])
                                      DropdownMenuItem(
                                        key: ValueKey(
                                          'calibrate-target-item-$target',
                                        ),
                                        value: target,
                                        child: Text(target),
                                      ),
                                  ],
                                  onChanged: _onTargetChanged,
                                ),
                              ),
                            ),
                          ),
                          Text(
                            '1-based/player-facing; the core validates the instance and refuses '
                            'with its own line.',
                            style: theme.textTheme.bodySmall?.copyWith(
                              color: theme.colorScheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                      ),
                    ],
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
                              (_selected.isTargeted && _target == null) ||
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
                  title:
                      'Stored mark — $_kindName'
                      '${_selected.isTargeted ? ' · target ${_target ?? '—'}' : ''}',
                  children: [
                    if (_selected.isTargeted && _target == null)
                      Text(
                        'Choose a target to see its stored mark.',
                        key: const ValueKey('calibrate-target-required'),
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      )
                    else if (storedError != null)
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
                      if (stored.target != null)
                        Text(
                          'Instance: ${stored.target} (echoed by the core)',
                          key: const ValueKey('calibrate-stored-target'),
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
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

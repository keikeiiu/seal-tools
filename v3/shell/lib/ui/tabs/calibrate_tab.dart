/// The Calibrate workspace: capture shelf (the core's four-frame FIFO), target tree for every served
/// mark, canvas with fit/1:1/pan, mark editor with the target/verdict/artifact rules, stored-coordinate
/// table, reference-image gallery and the session-shaped calibrations that still need protocol work.
///
/// The shell owns presentation only. It sends `capture` and `calibration.save`/`calibration.fetch`
/// with the mark in CLIENT-RELATIVE PHYSICAL pixels and the capture's own `DisplayInfo`; the core owns
/// every rule, the derived pitch, the artifact crop and the environment write, and its lines are
/// rendered VERBATIM. A saved mark stores the mark, artifact and display environment — never the
/// frame — so the screen never claims a frame is persisted. Every save needs a live `frame_id`; an
/// evicted frame needs a NEW capture (the shell never re-captures automatically).
///
/// Capture still hides the shell, waits one frame, asks the core and re-shows UNCONDITIONALLY; a
/// missing host channel is a note, not a failed capture. Selecting a different capture changes which
/// frame's measurement a save carries — the one place a `DisplayInfo` becomes a coordinate is
/// `calibration_widgets.dart`.
///
/// [CALIBRATION.md]: ../../../docs/spec/CALIBRATION.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../protocol/protocol.dart' as proto;
import '../components/empty_state.dart';
import '../components/shell_prose.dart';
import '../components/status_banner.dart';
import '../components/status_chip.dart';
import '../components/surface_panel.dart';
import '../theme.dart';
import 'action_result_line.dart';
import 'calibration/calibration_catalog.dart';
import 'calibration/capture_shelf.dart';
import 'calibration/detect_panel.dart';
import 'calibration/reference_gallery.dart';
import 'calibration/session_panel.dart';
import 'calibration/stored_marks_table.dart';
import 'calibration/target_tree.dart';
import 'calibration_widgets.dart';

/// The demo's workspace columns (PLAN-V3-UI-UX §6 "Calibrate"): captures + target tree 250 px,
/// flexible canvas, mark editor + reference images 340 px. Below the breakpoint the same panels stack
/// in one column instead of squeezing the canvas.
///
/// The breakpoint is CONTENT width, not window width: the default 1280 window leaves ~1064 px beside
/// the 216 px rail, and that must show the three-column workspace; 980 keeps the canvas usable while
/// a 1120 window (the enforced minimum) falls back to one column.
const double _captureColumnWidth = 250;
const double _editorColumnWidth = 340;
const double _workspaceBreakpoint = 980;

/// The canvas viewport's height; fit mode contains the frame inside (available width, this).
const double _canvasHeight = 420;

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

  String _kindName = calibrationKinds.first.name;

  /// The NAMED INSTANCE for a targeted kind (PROTOCOL §12.8); null until the player picks one.
  String? _target;

  /// v2's empty-result verdict for the gem result area: null = not answered, Save stays disabled.
  bool? _emptyChoice;

  /// True once the player changed the draft (typed a field, drew on the canvas, answered the empty
  /// question). A fetch never re-seeds a touched draft; a save/revert clears it.
  bool _draftTouched = false;

  /// The captures this screen took, newest first; the core keeps the last four, so 4 is the cap.
  final List<CalibrationCapture> _captures = <CalibrationCapture>[];
  String? _selectedFrameId;
  int _captureSequence = 0;

  /// Frame ids the core refused as `unknown frame_id` — the capture is unusable and needs a new one.
  final Set<String> _refusedFrameIds = <String>{};

  /// The last capture attempt's answer (a refusal is rendered as its core line).
  CoreActionResult<proto.CaptureResult>? _captureAttempt;
  String? _hostNote;
  String? _shelfNote;

  bool _capturing = false;
  bool _saving = false;
  bool _loadingStored = false;

  CoreActionResult<CalibrationSaveResult>? _save;

  /// Stored marks by [calibrationStoredKey] (fetched for all 18 kinds / 35 marks).
  final Map<String, CalibrationFetchResult> _stored = {};
  final Map<String, String> _storedErrors = {};

  /// The `calibration` block signature when the stored marks were last loaded; a change means another
  /// writer touched the stored calibration.
  String? _storedConfigSignature;

  /// The environment the last successful save from THIS screen wrote. The config mirror only refreshes
  /// the `calibration` block on `hello`/`config.reload`, so a fresh save is the better comparison
  /// baseline until then.
  StoredCalibrationEnvironment? _savedEnvironment;

  String _toolFilter = 'All';

  CalibrationKind get _selected => calibrationKindByName(_kindName);

  String get _currentStoredKey => calibrationStoredKey(_kindName, _target);

  /// The selected capture, or the newest one when the selection was evicted.
  CalibrationCapture? get _activeCapture {
    if (_captures.isEmpty) {
      return null;
    }
    for (final capture in _captures) {
      if (capture.frameId == _selectedFrameId) {
        return capture;
      }
    }
    return _captures.first;
  }

  /// The capture's own display environment, as the save will send it.
  StoredCalibrationEnvironment get _storedEnvironment =>
      _savedEnvironment ??
      StoredCalibrationEnvironment.fromConfig(
        widget.model.config.config['calibration'],
      );

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

  /// Fetches every served mark once (12 untargeted kinds + 23 named instances = 35) so the target
  /// tree, the stored table and the gallery tell the truth without opening each kind first. The core
  /// owns the answers; a fetch error is kept per key and rendered, not thrown.
  Future<void> _loadStoredAll() async {
    if (!mounted) {
      return;
    }
    setState(() => _loadingStored = true);
    // G08: ONE request instead of 35 sequential fetches. A core refusal or a pre-G08 fake falls
    // back to the per-slot loop so the screen still renders something truthful.
    final all = await widget.model.calibrationFetchAll();
    if (!mounted) {
      return;
    }
    if (all.ok && all.value!.marks.isNotEmpty) {
      final slots = {for (final slot in calibrationMarkSlots()) slot.key};
      setState(() {
        _stored.clear();
        _storedErrors.clear();
        for (final mark in all.value!.marks) {
          final key = calibrationStoredKey(mark.kind, mark.target);
          if (slots.contains(key)) {
            _stored[key] = mark;
          }
        }
        _loadingStored = false;
        _storedConfigSignature = _configSignature();
        if (!_draftTouched) {
          _fillEditorFromStored();
        }
      });
      return;
    }
    final slots = calibrationMarkSlots();
    final results = await Future.wait([
      for (final slot in slots)
        widget.model
            .calibrationFetch(slot.kind.name, target: slot.target)
            .then((result) => MapEntry(slot, result)),
    ]);
    if (!mounted) {
      return;
    }
    setState(() {
      for (final entry in results) {
        final slot = entry.key;
        final result = entry.value;
        if (result.ok) {
          _stored[slot.key] = result.value!;
          _storedErrors.remove(slot.key);
        } else {
          _storedErrors[slot.key] = result.errorLine ?? 'fetch failed';
        }
      }
      _loadingStored = false;
      _storedConfigSignature = _configSignature();
      if (!_draftTouched) {
        _fillEditorFromStored();
      }
    });
  }

  /// The `calibration` block of the shell's config mirror as a stable signature (external-change
  /// detection; no protocol command is added — the mirror arrives with `hello`/`config.reload`).
  String _configSignature() =>
      jsonEncode(asJsonMap(widget.model.config.config['calibration']));

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
    final key = calibrationStoredKey(kindName, target);
    setState(() {
      if (result.ok) {
        _stored[key] = result.value!;
        _storedErrors.remove(key);
      } else {
        _storedErrors[key] = result.errorLine ?? 'fetch failed';
      }
      if (notify && !_draftTouched) {
        _fillEditorFromStored();
      }
    });
  }

  /// Seeds the typed fields from the stored mark of the selected kind/instance, or to zero when it has
  /// none. Called when the SELECTION changes (never while the player types).
  void _seedEditorForSelection() {
    final stored = _stored[_currentStoredKey]?.mark;
    _x.text = '${stored?.x ?? 0}';
    _y.text = '${stored?.y ?? 0}';
    _w.text = '${stored?.isRect == true ? stored!.w : 0}';
    _h.text = '${stored?.isRect == true ? stored!.h : 0}';
  }

  /// The editor starts from (and re-seeds to) the stored mark; typed fields are the baseline input.
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

  // ── capture ────────────────────────────────────────────────────────────────────────────────

  /// hide → one frame → capture → show, with the show UNCONDITIONAL. The hide's reply is awaited
  /// before the frame wait so the core sees the game as the foreground window; a missing host
  /// channel is a note, not a failure — the capture is still attempted and its line rendered.
  Future<void> _captureFrame() async {
    setState(() {
      _capturing = true;
      _captureAttempt = null;
      _save = null;
      _hostNote = null;
      _shelfNote = null;
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
        _captureAttempt = result;
        _hostNote = hostNote.isEmpty ? null : hostNote;
        if (result.ok) {
          _recordCapture(result.value!);
        }
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

  /// Adds a successful capture to the shelf and enforces the core's own capacity: the last FOUR
  /// frames are kept, so a fifth evicts the oldest (PROTOCOL §8). Eviction is shown, never hidden;
  /// the evicted frame is dropped, not re-captured.
  void _recordCapture(proto.CaptureResult capture) {
    _captureSequence += 1;
    _captures.insert(
      0,
      CalibrationCapture(
        frame: capture.frame,
        display: capture.display,
        sequence: _captureSequence,
        capturedAt: DateTime.now(),
      ),
    );
    final evicted = <CalibrationCapture>[];
    while (_captures.length > 4) {
      evicted.add(_captures.removeLast());
    }
    _selectedFrameId = capture.frame.frameId;
    if (evicted.isNotEmpty) {
      final names = evicted
          .map((item) => '${item.label} (id ${item.frameId})')
          .join(', ');
      _shelfNote =
          'The core keeps the last four frames, so $names dropped out of its cache. A save '
          'needs a live frame id — capture again instead of re-using it.';
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

  // ── editor / save ──────────────────────────────────────────────────────────────────────────

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

  String? get _markLabel {
    final mark = _editorMark();
    if (mark == null) {
      return null;
    }
    final target = _selected.isTargeted ? ' · ${_target ?? 'no target'}' : '';
    final shape = mark.isRect
        ? 'x ${mark.x} · y ${mark.y} · ${mark.w}×${mark.h}'
        : 'x ${mark.x} · y ${mark.y}';
    return '${_selected.label}$target · $shape';
  }

  /// A click on the picture: write the point into the fields (the fields stay authoritative).
  void _applyPointMark(CalibrationMark mark) {
    setState(() {
      _draftTouched = true;
      _x.text = '${mark.x}';
      _y.text = '${mark.y}';
    });
  }

  /// A drag on the picture: write the rect into the fields.
  void _applyRectMark(CalibrationMark mark) {
    setState(() {
      _draftTouched = true;
      _x.text = '${mark.x}';
      _y.text = '${mark.y}';
      _w.text = '${mark.w}';
      _h.text = '${mark.h}';
    });
  }

  void _onFieldChanged() {
    setState(() => _draftTouched = true);
  }

  /// Resets the draft to the stored mark of the selection (the empty verdict is NOT stored, so it must
  /// be answered again; the hint next to the chips says so).
  void _revertDraft() {
    setState(() {
      _save = null;
      _emptyChoice = null;
      _draftTouched = false;
      _seedEditorForSelection();
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
      _draftTouched = false;
      _seedEditorForSelection();
    });
  }

  /// The named instance changed: fetch that instance's stored mark; a targeted kind without a target
  /// shows no stored mark and keeps Save disabled.
  void _onTargetChanged(String? target) {
    setState(() {
      _target = target;
      _save = null;
      _draftTouched = false;
      _seedEditorForSelection();
    });
    if (target != null) {
      unawaited(_fetch(_kindName, target: target));
    }
  }

  /// A target-tree instance row: selects its kind and instance in one step.
  void _selectTreeTarget(CalibrationKind kind, String target) {
    setState(() {
      if (_kindName != kind.name) {
        _kindName = kind.name;
        _emptyChoice = null;
      }
      _target = target;
      _save = null;
      _draftTouched = false;
      _seedEditorForSelection();
    });
    unawaited(_fetch(kind.name, target: target));
  }

  /// A stored-table row: selects the kind/instance so the canvas and editor follow it.
  void _selectSlot(CalibrationMarkSlot slot) {
    if (slot.kind.name == _kindName && slot.target == _target) {
      return;
    }
    setState(() {
      _kindName = slot.kind.name;
      _target = slot.target;
      _save = null;
      _emptyChoice = null;
      _draftTouched = false;
      _seedEditorForSelection();
    });
    if (slot.target != null) {
      unawaited(_fetch(slot.kind.name, target: slot.target));
    }
  }

  /// Save stays disabled while any answer the core requires is missing — target, empty verdict — and
  /// while there is no LIVE frame (an evicted/refused frame needs a new capture; never automatic).
  bool get _canSave {
    if (_saving) {
      return false;
    }
    final capture = _activeCapture;
    if (capture == null || _refusedFrameIds.contains(capture.frameId)) {
      return false;
    }
    if (_selected.isTargeted && _target == null) {
      return false;
    }
    if (_selected.requiresEmptyChoice && _emptyChoice == null) {
      return false;
    }
    return true;
  }

  Future<void> _saveMark() async {
    final capture = _activeCapture;
    if (capture == null) {
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
    final result = await widget.model.calibrationSave(
      frameId: capture.frame.frameId,
      kind: kind.name,
      region: kind.isRect ? proto.Rect(x: x, y: y, w: w!, h: h!) : null,
      point: kind.isRect ? null : proto.Point(x: x, y: y),
      empty: kind.requiresEmptyChoice ? _emptyChoice : null,
      target: kind.isTargeted ? _target : null,
      // The save carries the SELECTED CAPTURE's own measurement — never the newest capture's or a
      // cached one.
      display: capture.display,
      // G06: the rev the shell last read for this kind/instance. A mark saved by another writer
      // makes this stale and the core refuses with `conflict` rather than overwriting it.
      rev: _stored[_currentStoredKey]?.rev,
    );
    if (!mounted) {
      return;
    }
    final refusedFrame =
        !result.ok && (result.errorLine ?? '').contains('unknown frame_id');
    setState(() {
      _saving = false;
      _save = result;
      if (!result.ok && result.error?.code == proto.ErrorCode.conflict) {
        // Another writer changed the calibration: re-read the mark this kind now has, so the
        // player sees the current value the core refused against.
        unawaited(_fetch(kind.name, target: kind.isTargeted ? _target : null));
      }
      if (result.ok) {
        _draftTouched = false;
        // G07: prefer the core's environment echo (it carries the authoritative measured_at).
        final echo = result.value!.calibration;
        final fromEcho = echo == null
            ? null
            : StoredCalibrationEnvironment.fromCoreEcho(echo);
        _savedEnvironment = fromEcho != null && !fromEcho.isEmpty
            ? fromEcho
            : StoredCalibrationEnvironment.fromSave(
                capture.display,
                capture.frame.width,
                capture.frame.height,
              );
      }
      if (refusedFrame) {
        _refusedFrameIds.add(capture.frame.frameId);
        _shelfNote =
            'The core refused ${capture.label} (id ${capture.frame.frameId}) as an unknown '
            'frame id: it is no longer in the four-frame cache. Capture again — this screen never '
            're-captures automatically.';
      }
    });
    if (result.ok) {
      await _fetch(kind.name, target: kind.isTargeted ? _target : null);
      if (!mounted) {
        return;
      }
      setState(() => _storedConfigSignature = _configSignature());
    }
  }

  // ── UI ─────────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final theme = Theme.of(context);
    final capture = _activeCapture;
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
          child: _buildHeader(theme, capture),
        ),
        Expanded(
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(horizontal: ShellSpace.lg),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                _buildDisplayEnvironment(theme, capture),
                const SizedBox(height: ShellSpace.md),
                DetectPanel(model: widget.model),
                _buildExternalChangeBanner(),
                LayoutBuilder(
                  builder: (context, constraints) {
                    final wide = constraints.maxWidth >= _workspaceBreakpoint;
                    final left = Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [_buildCaptureShelf(theme), _buildTargetTree()],
                    );
                    final canvas = _buildCanvasPanel(theme, capture);
                    final right = Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        _buildMarkEditor(theme),
                        ReferenceGalleryPanel(stored: _stored),
                      ],
                    );
                    if (!wide) {
                      return Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [left, canvas, right],
                      );
                    }
                    return Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        SizedBox(width: _captureColumnWidth, child: left),
                        const SizedBox(width: ShellSpace.lg),
                        Expanded(child: canvas),
                        const SizedBox(width: ShellSpace.lg),
                        SizedBox(width: _editorColumnWidth, child: right),
                      ],
                    );
                  },
                ),
                StoredMarksTablePanel(
                  stored: _stored,
                  storedErrors: _storedErrors,
                  toolFilter: _toolFilter,
                  onToolFilterChanged: (value) =>
                      setState(() => _toolFilter = value),
                  onSelectSlot: _selectSlot,
                ),
                const SizedBox(height: ShellSpace.md),
                const SessionCalibrationsPanel(),
                const SizedBox(height: ShellSpace.lg),
              ],
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildHeader(ThemeData theme, CalibrationCapture? capture) {
    final captureLabel = _capturing ? 'Capturing…' : 'Capture';
    final captureIcon = _capturing
        ? const SizedBox(
            width: 16,
            height: 16,
            child: CircularProgressIndicator(strokeWidth: 2),
          )
        : const Icon(Icons.photo_camera_outlined, size: ShellSize.iconMd);
    // Save is the screen's only filled action; Capture is filled only while there is no frame yet,
    // so the two never compete for the eye.
    final captureButton = _captures.isEmpty
        ? FilledButton.icon(
            key: const ValueKey('calibrate-capture'),
            onPressed: _capturing ? null : _captureFrame,
            icon: captureIcon,
            label: Text(captureLabel),
          )
        : OutlinedButton.icon(
            key: const ValueKey('calibrate-capture'),
            onPressed: _capturing ? null : _captureFrame,
            icon: captureIcon,
            label: Text(captureLabel),
          );
    final storedEnvironment = _storedEnvironment;
    final differences = capture == null
        ? const <String>[]
        : storedEnvironment.differencesFrom(capture.display);
    final matchChip = capture == null
        ? null
        : storedEnvironment.isEmpty
        ? const StatusChip(
            key: ValueKey('calibrate-display-match'),
            kind: StatusKind.muted,
            label: 'no stored display environment',
          )
        : differences.isEmpty
        ? const StatusChip(
            key: ValueKey('calibrate-display-match'),
            kind: StatusKind.success,
            label: 'matches stored calibration',
          )
        : const StatusChip(
            key: ValueKey('calibrate-display-match'),
            kind: StatusKind.warning,
            label: 'display changed — recalibrate',
          );
    final displayChip = capture == null
        ? null
        : StatusChip(
            key: const ValueKey('calibrate-display-chip'),
            kind: StatusKind.info,
            label:
                '${capture.displayLabel} · screen '
                '${capture.display.screen.w}×${capture.display.screen.h}',
          );
    final heading = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text('Calibrate', style: theme.textTheme.headlineSmall),
        const SizedBox(height: ShellSpace.xs),
        // Align loosens the stretched Column's tight width so ShellProse's 620 px measure actually
        // applies (a ConstrainedBox under stretch is handed the full panel width).
        const Align(
          alignment: Alignment.centerLeft,
          child: ShellProse(
            'Capture the game once per display setup, then mark every surface the tools read. '
            'Marks are stored with the display environment they were measured on, so a screen or '
            'client-size change can be detected instead of silently mis-clicking.',
            style: TextStyle(fontSize: 14, height: 1.5),
          ),
        ),
      ],
    );
    final actions = Column(
      crossAxisAlignment: CrossAxisAlignment.end,
      children: [
        if (displayChip != null || matchChip != null) ...[
          Wrap(
            alignment: WrapAlignment.end,
            spacing: ShellSpace.sm,
            runSpacing: ShellSpace.sm,
            children: [?displayChip, ?matchChip],
          ),
          const SizedBox(height: ShellSpace.sm),
        ],
        Wrap(
          alignment: WrapAlignment.end,
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: ShellSpace.sm,
          runSpacing: ShellSpace.sm,
          children: [
            OutlinedButton.icon(
              key: const ValueKey('calibrate-refresh'),
              onPressed: _loadingStored ? null : _loadStoredAll,
              icon: const Icon(Icons.refresh, size: ShellSize.iconMd),
              label: Text(_loadingStored ? 'Refreshing…' : 'Refresh stored'),
            ),
            captureButton,
          ],
        ),
      ],
    );
    // Demo header: title + prose left, display chips/actions right when there is room; stacked at a
    // window narrow enough that side-by-side would squeeze the prose.
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < 900) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              heading,
              const SizedBox(height: ShellSpace.md),
              actions,
            ],
          );
        }
        return Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(child: heading),
            const SizedBox(width: ShellSpace.lg),
            actions,
          ],
        );
      },
    );
  }

  Widget _buildDisplayEnvironment(
    ThemeData theme,
    CalibrationCapture? capture,
  ) {
    if (capture == null) {
      return Padding(
        padding: const EdgeInsets.only(bottom: ShellSpace.md),
        // Align loosens the stretched column so the prose measure applies to the state line too.
        child: Align(
          alignment: Alignment.centerLeft,
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: ShellWidth.prose),
            child: Text(
              'Display environment: measured with each capture. Nothing captured yet.',
              key: const ValueKey('calibrate-display-empty'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        ),
      );
    }
    final display = capture.display;
    final storedEnvironment = _storedEnvironment;
    final differences = storedEnvironment.differencesFrom(display);
    final source = _savedEnvironment != null
        ? 'the last successful save from this screen'
        : 'the config mirror (`hello` / `config.reload`)';
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          ActionResultLine(
            key: const ValueKey('calibrate-display'),
            label: 'Display',
            line: displaySummary(display),
          ),
          Text(
            'Stored environment: ${storedEnvironment.summary}'
            '${storedEnvironment.measuredAt != null ? ' · measured ${storedEnvironment.measuredAt}' : ''}'
            ' — from $source. A save writes '
            '`calibration.{dpi_scale, monitor_dpi, screen, client_size}`; the mirror only '
            'refreshes on `hello`/`config.reload`.',
            key: const ValueKey('calibrate-stored-environment'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          if (differences.isNotEmpty) ...[
            const SizedBox(height: ShellSpace.sm),
            StatusBanner(
              key: const ValueKey('calibrate-display-mismatch'),
              kind: StatusKind.warning,
              title: 'This display does not match the stored calibration.',
              detail:
                  '${differences.join(' · ')}. Recalibrate before running a tool: no automatic '
                  'anchoring and no proportional transform is applied (CALIBRATION §5/§6). A '
                  'scale-only change leaves the stored physical marks valid.',
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildExternalChangeBanner() {
    final signature = _configSignature();
    if (_storedConfigSignature == null || signature == _storedConfigSignature) {
      return const SizedBox.shrink();
    }
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.md),
      child: StatusBanner(
        key: const ValueKey('calibrate-external-change'),
        kind: StatusKind.info,
        title: 'The stored calibration changed elsewhere.',
        detail:
            'The `calibration` block of the core\'s config no longer matches what this screen '
            'loaded. Refresh the stored marks before saving; the core is the writer.',
        action: TextButton(
          key: const ValueKey('calibrate-external-refresh'),
          onPressed: _loadingStored ? null : _loadStoredAll,
          child: const Text('Refresh stored'),
        ),
      ),
    );
  }

  Widget _buildCaptureShelf(ThemeData theme) {
    final capture = _captureAttempt;
    final hostNote = _hostNote;
    final statusChildren = <Widget>[
      if (_capturing)
        Row(
          key: const ValueKey('calibrate-capturing'),
          children: [
            const SizedBox(
              width: 14,
              height: 14,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
            const SizedBox(width: ShellSpace.sm),
            Expanded(
              child: Text(
                'Hiding the shell → waiting one frame → capturing the game only → '
                'restoring the shell. The show is unconditional.',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ),
          ],
        ),
      if (hostNote != null)
        Text(
          hostNote,
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
      if (capture != null && !capture.ok)
        Text(
          'Old captures stay usable. Press Capture to retry — a capture is never retried '
          'automatically.',
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
    ];
    return CaptureShelfPanel(
      captures: _captures,
      selectedFrameId: _activeCapture?.frameId,
      onSelect: (frameId) => setState(() => _selectedFrameId = frameId),
      onCapture: _captureFrame,
      capturing: _capturing,
      refusedFrameIds: _refusedFrameIds,
      status: statusChildren.isEmpty
          ? null
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: statusChildren,
            ),
      note: _shelfNote,
    );
  }

  Widget _buildTargetTree() => TargetTreePanel(
    selectedKind: _kindName,
    selectedTarget: _target,
    stored: _stored,
    onSelectKind: _selectKind,
    onSelectTarget: _selectTreeTarget,
  );

  Widget _buildCanvasPanel(ThemeData theme, CalibrationCapture? capture) {
    if (capture == null) {
      return SurfacePanel(
        padding: const EdgeInsets.all(ShellSpace.lg),
        child: KeyedSubtree(
          key: const ValueKey('calibrate-capture-first'),
          child: EmptyState(
            icon: Icons.center_focus_strong_outlined,
            title: 'No capture yet',
            hint:
                'Capture the window to see the frame and draw marks. Nothing about a mark is '
                'persisted until Save; the frame itself is only the write-time provenance.',
          ),
        ),
      );
    }
    final display = capture.display;
    return SurfacePanel(
      padding: const EdgeInsets.all(ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  '${capture.label} · ${capture.sizeLabel} · display '
                  '${formatCalibrationScale(display.scale)}× / '
                  '${display.physicalClient.w}×${display.physicalClient.h} client',
                  key: const ValueKey('calibrate-frame-caption'),
                  style: ShellText.mono.copyWith(
                    fontSize: 11,
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: ShellSpace.sm),
              Text(
                'measured ${capture.timeLabel}',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
          const SizedBox(height: ShellSpace.xs),
          CalibrationFramePicture(
            key: const ValueKey('calibrate-frame'),
            frame: capture.frame,
            maxHeight: _canvasHeight,
            mark: _editorMark(),
            markLabel: _markLabel,
            clientOrigin: Offset(
              display.physicalClient.x.toDouble(),
              display.physicalClient.y.toDouble(),
            ),
            onPointMark: _selected.isRect ? null : _applyPointMark,
            onRectMark: _selected.isRect ? _applyRectMark : null,
          ),
        ],
      ),
    );
  }

  Widget _buildMarkEditor(ThemeData theme) {
    final kind = _selected;
    final stored = _stored[_currentStoredKey];
    final storedError = _storedErrors[_currentStoredKey];
    final save = _save;
    final capture = _activeCapture;
    final evictionRisk =
        capture != null &&
        _captures.length >= 4 &&
        capture.frameId == _captures.last.frameId;
    return SurfacePanel(
      title: 'Mark',
      trailing: _markStatusChip(),
      padding: const EdgeInsets.all(ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          InputDecorator(
            decoration: const InputDecoration(
              labelText: 'Surface',
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
                  for (final entry in calibrationKinds)
                    DropdownMenuItem(
                      key: ValueKey('calibrate-kind-item-${entry.name}'),
                      value: entry.name,
                      child: Text(
                        '${entry.group} · ${entry.label} (${entry.name})',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                ],
                onChanged: _selectKind,
              ),
            ),
          ),
          const SizedBox(height: ShellSpace.xs),
          Text(
            '${kind.tool} · ${kind.shapeLabel} · client-relative physical px'
            '${kind.hasArtifact ? ' · writes a crop artifact' : ''}'
            '${kind.isTargeted ? ' · named instance' : ''}',
            key: const ValueKey('calibrate-kind-shape'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          if (kind.isTargeted) ...[
            const SizedBox(height: ShellSpace.sm),
            InputDecorator(
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
                    for (final target in kind.targets ?? const <String>[])
                      DropdownMenuItem(
                        key: ValueKey('calibrate-target-item-$target'),
                        value: target,
                        child: Text(target),
                      ),
                  ],
                  onChanged: _onTargetChanged,
                ),
              ),
            ),
            Text(
              '1-based/player-facing; the core validates the instance and refuses with its own line.',
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ],
          const SizedBox(height: ShellSpace.sm),
          Wrap(
            spacing: ShellSpace.sm,
            runSpacing: ShellSpace.sm,
            children: [
              _pixelField('calibrate-x', 'x', _x),
              _pixelField('calibrate-y', 'y', _y),
              if (kind.isRect) ...[
                _pixelField('calibrate-w', 'w', _w),
                _pixelField('calibrate-h', 'h', _h),
              ],
            ],
          ),
          if (evictionRisk) ...[
            const SizedBox(height: ShellSpace.sm),
            Text(
              'This frame is the oldest of the four and the next capture evicts it. Save this '
              'mark before capturing again — an evicted frame is refused, never re-captured.',
              key: const ValueKey('calibrate-eviction-risk'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.shellColors.warning,
              ),
            ),
          ],
          if (kind.requiresEmptyChoice) ...[
            const SizedBox(height: ShellSpace.md),
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
                  label: const Text('The result box is EMPTY right now'),
                  selected: _emptyChoice == true,
                  onSelected: (_) => setState(() {
                    _emptyChoice = true;
                    _draftTouched = true;
                  }),
                ),
                ChoiceChip(
                  key: const ValueKey('calibrate-empty-no'),
                  label: const Text('A gem is in the box'),
                  selected: _emptyChoice == false,
                  onSelected: (_) => setState(() {
                    _emptyChoice = false;
                    _draftTouched = true;
                  }),
                ),
              ],
            ),
            const SizedBox(height: ShellSpace.xs),
            Text(
              'Empty saves the crop and samples the colour signature. A gem in the box samples '
              'only the colour fallback — the strong crop test is not created. The verdict is '
              'not stored, so Revert clears it.',
              key: const ValueKey('calibrate-empty-hint'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ],
          if (kind.hasArtifact) ...[
            const SizedBox(height: ShellSpace.md),
            _buildArtifactPreview(theme, stored),
          ],
          const SizedBox(height: ShellSpace.md),
          Row(
            children: [
              OutlinedButton(
                key: const ValueKey('calibrate-revert'),
                onPressed: _draftTouched ? _revertDraft : null,
                child: const Text('Revert'),
              ),
              const SizedBox(width: ShellSpace.sm),
              Expanded(
                child: FilledButton(
                  key: const ValueKey('calibrate-save'),
                  onPressed: _canSave ? _saveMark : null,
                  child: _saving
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Text('Save mark'),
                ),
              ),
            ],
          ),
          if (save != null) ...[
            const SizedBox(height: ShellSpace.sm),
            ActionResultLine(
              key: const ValueKey('calibrate-save-result'),
              label: 'Save',
              line: save.ok
                  ? 'saved ${save.value!.kind} at rev ${save.value!.rev} · '
                        'mark [${save.value!.mark.label}]'
                  : save.errorLine,
              isError: !save.ok,
            ),
            if (save.ok) ...[
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
            if (!save.ok &&
                (save.errorLine ?? '').contains('unknown frame_id')) ...[
              Text(
                'Capture again to get a live frame id. This screen never re-captures '
                'automatically.',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),
              const SizedBox(height: ShellSpace.xs),
              Align(
                alignment: Alignment.centerLeft,
                child: OutlinedButton.icon(
                  key: const ValueKey('calibrate-capture-again'),
                  onPressed: _capturing ? null : _captureFrame,
                  icon: const Icon(
                    Icons.photo_camera_outlined,
                    size: ShellSize.iconSm,
                  ),
                  label: const Text('Capture again'),
                ),
              ),
            ],
          ],
          const Divider(height: ShellSpace.xl),
          _buildStoredMarkSection(theme, stored, storedError),
        ],
      ),
    );
  }

  Widget _buildArtifactPreview(
    ThemeData theme,
    CalibrationFetchResult? stored,
  ) {
    final artifact = stored?.artifact;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Crop preview', style: theme.textTheme.labelMedium),
        const SizedBox(height: ShellSpace.xs),
        if (artifact != null) ...[
          CalibrationCropPicture(
            key: const ValueKey('calibrate-crop'),
            file: artifact.file,
            pngBase64: artifact.pngBase64,
          ),
          Text(
            'Stored crop (${artifact.file}) — the strong empty-result reference. Re-saving with '
            '"a gem is in the box" keeps it; the colour signature is only the fallback.',
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ] else
          Text(
            'No stored crop yet. Answer "the box is empty now" and Save; the core writes '
            '`config/calib_gem_result.png` and samples the colour signature.',
            key: const ValueKey('calibrate-crop-missing'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
      ],
    );
  }

  Widget _buildStoredMarkSection(
    ThemeData theme,
    CalibrationFetchResult? stored,
    String? storedError,
  ) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          'Stored mark — $_kindName'
          '${_selected.isTargeted ? ' · target ${_target ?? '—'}' : ''}',
          style: theme.textTheme.labelMedium,
        ),
        const SizedBox(height: ShellSpace.xs),
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
                : 'core returned kind "${stored.kind}" · ${stored.storedLine}',
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
          Text(
            'A saved mark stores the mark, the crop and the display environment — not the '
            'frame.',
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ],
      ],
    );
  }

  Widget _markStatusChip() {
    final capture = _activeCapture;
    if (_saving) {
      return const StatusChip(kind: StatusKind.info, label: 'Saving…');
    }
    final save = _save;
    if (save != null && save.ok) {
      return StatusChip(
        kind: StatusKind.success,
        label: 'Saved · rev ${save.value!.rev}',
      );
    }
    if (save != null && !save.ok) {
      return const StatusChip(kind: StatusKind.error, label: 'Save refused');
    }
    if (capture == null) {
      return const StatusChip(kind: StatusKind.muted, label: 'no frame');
    }
    if (_draftTouched) {
      return const StatusChip(kind: StatusKind.warning, label: 'Unsaved draft');
    }
    final stored = _stored[_currentStoredKey];
    if (stored?.mark != null) {
      return StatusChip(
        kind: StatusKind.muted,
        label: 'Stored · rev ${stored!.rev}',
      );
    }
    return const StatusChip(kind: StatusKind.muted, label: 'not saved');
  }

  Widget _pixelField(
    String key,
    String label,
    TextEditingController controller,
  ) {
    return SizedBox(
      width: ShellWidth.fieldSm,
      child: TextField(
        key: ValueKey(key),
        controller: controller,
        keyboardType: TextInputType.number,
        decoration: InputDecoration(
          labelText: '$label (px)',
          isDense: true,
          border: const OutlineInputBorder(),
        ),
        onChanged: (_) => _onFieldChanged(),
      ),
    );
  }
}

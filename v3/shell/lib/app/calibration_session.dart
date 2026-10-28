/// One Calibrate surface's pending marks and the captured frame they were drawn on — the session
/// draft of the calibration plan's increment 0. Later increments build the four surfaces on it; this
/// file is plumbing, not a surface.
///
/// This is VIEW STATE ONLY: no config, no persistence, no timers, and deliberately NOT a
/// `ChangeNotifier`. A surface that renders from it must rebuild on change itself (a scoped
/// `ListenableBuilder`/`setState` owned by the surface): content built in a tab's `build()` and handed
/// to `buildConfigTab(child:)` does not rebuild from other notifiers — the Pet-tab count-line bug —
/// so the session's owner drives its own repaint.
///
/// Marks are CLIENT-RELATIVE PHYSICAL pixels in the captured frame's space ([CALIBRATION.md] §1),
/// which is why the frame's [DisplayInfo] travels with them: a save sends the core's `frame_id` (a
/// reference into its frame cache) plus the measured display, never the PNG bytes (PROTOCOL §8).
///
/// [CALIBRATION.md]: ../../../docs/spec/CALIBRATION.md
library;

import '../protocol/protocol.dart';
import 'calibration_result.dart';

/// One pending shape on the session's frame: the calibration [kind] it will be sent as, the drawn
/// shape, the named instance for a §12.8 targeted kind, and a surface label.
///
/// The shape reuses [CalibrationMark] — the existing stored/saved mark model, a point or a rect in
/// client-relative physical pixels — instead of a second point/rect type, because the canvas helpers
/// already produce exactly that. The session adds the identity a save needs on top of it.
class CalibrationSessionMark {
  const CalibrationSessionMark({
    required this.kind,
    required this.shape,
    this.target,
    this.label = '',
  });

  /// The calibration kind the mark will be sent as (`bag_grid`, `pet_toggle`, `gem_grade`, …).
  final String kind;

  /// The drawn point or rect, in the frame's client-relative physical pixels.
  final CalibrationMark shape;

  /// The player-facing instance name for a §12.8 targeted kind (`ITEM1`, row `"2"`, `N`), or null
  /// for an untargeted kind. Any 0-based index arithmetic stays the surface's job.
  final String? target;

  /// Display text for the surface's mark list. Never goes on the wire.
  final String label;

  /// True for a point mark, false for a rect mark ([CalibrationMark]'s own shape).
  bool get isPoint => !shape.isRect;

  /// The identity a re-draw replaces: the same [kind] AND [target] is the same slot of the draft.
  bool sameSlot(String otherKind, String? otherTarget) =>
      kind == otherKind && target == otherTarget;
}

/// The pending marks of one Calibrate surface and the capture they annotate.
class CalibrationSession {
  String? _frameId;
  DisplayInfo? _display;
  final List<CalibrationSessionMark> _marks = <CalibrationSessionMark>[];

  /// The core's frame id the marks are drawn over; null until the surface captures.
  String? get frameId => _frameId;

  /// The measurement that came WITH that frame ([PROTOCOL.md] §3); null until the surface captures.
  DisplayInfo? get display => _display;

  /// True once a capture has been taken — the precondition for a session save.
  bool get hasFrame => _frameId != null && _display != null;

  /// The pending marks in draft order (the order they were added; a replaced mark keeps its
  /// position). The surfaces draw this order, and a later payload keeps it.
  List<CalibrationSessionMark> get marks =>
      List<CalibrationSessionMark>.unmodifiable(_marks);

  /// Shape accessors for the canvas: the point marks, then the rect marks, each in draft order.
  List<CalibrationSessionMark> get pointMarks => [
    for (final mark in _marks)
      if (mark.isPoint) mark,
  ];
  List<CalibrationSessionMark> get rectMarks => [
    for (final mark in _marks)
      if (!mark.isPoint) mark,
  ];

  /// Records the capture the surface annotates. A re-capture does NOT clear the draft: the marks are
  /// client-relative coordinates, so only the explicit [clear] drops them.
  void setFrame(String frameId, DisplayInfo display) {
    _frameId = frameId;
    _display = display;
  }

  /// Appends [mark] at the end of the draft.
  void add(CalibrationSessionMark mark) => _marks.add(mark);

  /// Replaces the mark with the SAME (kind, target) in place — its draft position is kept — or
  /// appends it when the draft has none yet. Drawing a mark again is this call.
  void replace(CalibrationSessionMark mark) {
    final index = _marks.indexWhere(
      (existing) => existing.sameSlot(mark.kind, mark.target),
    );
    if (index < 0) {
      _marks.add(mark);
    } else {
      _marks[index] = mark;
    }
  }

  /// Removes the mark for ([kind], [target]); true when there was one.
  bool remove(String kind, {String? target}) {
    final index = _marks.indexWhere(
      (existing) => existing.sameSlot(kind, target),
    );
    if (index < 0) {
      return false;
    }
    _marks.removeAt(index);
    return true;
  }

  /// Drops every pending mark; the captured frame stays (a surface's Reset).
  void clear() => _marks.clear();

  /// Drops the frame and the marks (leaving the surface/session).
  void reset() {
    _frameId = null;
    _display = null;
    _marks.clear();
  }

  /// The frame arguments every session save needs: the core's frame id and that frame's measured
  /// display. `display` is the same environment subset `calibration.save` carries
  /// ([calibrationDisplayJson], the one place that key mapping lives); the full [DisplayInfo] stays
  /// here for the canvas's frame-to-screen mapping.
  ///
  /// Only valid with a captured frame: a surface cannot save marks without one, so calling this
  /// before [setFrame] is a programming error rather than a refusal to render.
  Map<String, dynamic> framePayload() {
    final frameId = _frameId;
    final display = _display;
    if (frameId == null || display == null) {
      throw StateError(
        'CalibrationSession.framePayload() needs a captured frame',
      );
    }
    return {'frame_id': frameId, 'display': calibrationDisplayJson(display)};
  }
}

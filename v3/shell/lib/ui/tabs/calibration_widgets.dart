/// Separable calibration view pieces: the captured frame picture and the mark/crop readers.
///
/// They are deliberately standalone so the wave-2 Calibrate surface and the read-only Pet/Setup
/// geometry panels can share them later (the plan's "one CalibrationCanvas" rule) without pulling the
/// tab's state along.
library;

import 'dart:convert';
import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart' show PointerScrollEvent;
import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../protocol/protocol.dart' hide Point, Rect, Size;
import '../theme.dart';

/// Base64 PNG → bytes, or null when the payload is not valid base64 (a bad frame must never crash the
/// render loop; the widget shows a line instead).
Uint8List? decodePngBase64(String data) {
  try {
    return base64Decode(data);
  } on FormatException {
    return null;
  }
}

/// The captured frame on the workspace canvas, with a real zoom control (`fit`, `1:1`, `pan`), an
/// OPTIONAL gesture overlay (a click sets a point mark, a drag sets a rect) and the selected mark
/// drawn with its label.
///
/// The rendered picture is mapped back to CLIENT-RELATIVE PHYSICAL pixels through
/// [framePointToMark]/[frameDragToMark] — the one conversion (CALIBRATION §4). The typed fields stay
/// authoritative: the tab passes [mark] in (built from its fields) and writes each gesture's mark
/// back into those fields, so Save always reads the fields, never hidden picture state.
///
/// Zoom is DISPLAY pixels per FRAME pixel, not `DisplayInfo.scale` (that is the cursor-space
/// conversion, CALIBRATION §4). Fit is derived from the latest layout and never cached across a size
/// change. A click in the letterbox is ignored, never clamped into a mark. Omit the callbacks for a
/// read-only picture (the Pet/Setup reuse case).
class CalibrationFramePicture extends StatefulWidget {
  const CalibrationFramePicture({
    super.key,
    required this.frame,
    this.maxHeight = 320,
    this.mark,
    this.markLabel,
    this.clientOrigin = ui.Offset.zero,
    this.onPointMark,
    this.onRectMark,
  });

  final Frame frame;

  /// The canvas viewport's height; fit mode contains the frame in `(available width, maxHeight)`.
  final double maxHeight;

  /// The mark currently in the typed fields, drawn on the picture.
  final CalibrationMark? mark;

  /// The label drawn on the canvas with the mark (`Bag grid · x 700 · y 400 · 900×500`), or null.
  final String? markLabel;

  /// Screen origin of the client (`display.physical_client`). A full-client capture has the same
  /// origin as the frame, so the offset is zero; a sub-region capture subtracts it.
  final ui.Offset clientOrigin;

  final ValueChanged<CalibrationMark>? onPointMark;
  final ValueChanged<CalibrationMark>? onRectMark;

  bool get interactive => onPointMark != null || onRectMark != null;

  @override
  State<CalibrationFramePicture> createState() =>
      _CalibrationFramePictureState();
}

class _CalibrationFramePictureState extends State<CalibrationFramePicture> {
  ui.Offset? _dragStartLocal;
  ui.Offset? _dragCurrentLocal;

  /// Display px per frame px; null = fit, derived from the latest layout and never cached across a
  /// size/monitor change (CALIBRATION §4).
  double? _zoom;

  /// The manual zoom's pan, in display px, applied to the centred picture.
  ui.Offset _pan = ui.Offset.zero;

  /// While pan mode is on, a left drag moves the view instead of drawing a mark.
  bool _panMode = false;

  static const double _zoomStep = 1.25;
  static const double _minZoom = 0.02;
  static const double _maxZoom = 8;

  void _setFit() {
    setState(() {
      _zoom = null;
      _pan = ui.Offset.zero;
    });
  }

  void _setOneToOne() {
    setState(() {
      _zoom = 1;
      _pan = ui.Offset.zero;
    });
  }

  void _zoomBy(double factor, double fitScale) {
    final current = _zoom ?? fitScale;
    setState(() {
      _zoom = (current * factor).clamp(_minZoom, _maxZoom).toDouble();
    });
  }

  /// Keep the picture reachable: clamp the pan to the overflow when the picture is larger than the
  /// viewport, and centre it (zero pan) when it is smaller.
  ui.Offset _clampPan(ui.Size viewport, ui.Size picture, ui.Offset pan) {
    final limitX = math.max(0.0, (picture.width - viewport.width) / 2);
    final limitY = math.max(0.0, (picture.height - viewport.height) / 2);
    return ui.Offset(
      pan.dx.clamp(-limitX, limitX).toDouble(),
      pan.dy.clamp(-limitY, limitY).toDouble(),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final frame = widget.frame;
    final bytes = decodePngBase64(frame.data);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Frame ${frame.width}×${frame.height} px · origin '
          '(${frame.origin.x}, ${frame.origin.y})',
          style: theme.textTheme.labelMedium,
        ),
        const SizedBox(height: ShellSpace.xs),
        if (bytes == null)
          Text(
            'The captured picture could not be decoded.',
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.error,
            ),
          )
        else
          _buildCanvas(theme, bytes),
      ],
    );
  }

  /// The zoom/viewport half of the canvas. Taking the non-null [bytes] as a parameter keeps the
  /// promotion out of the builder closures (a null decode never reaches here).
  Widget _buildCanvas(ThemeData theme, Uint8List bytes) {
    final frame = widget.frame;
    return LayoutBuilder(
      builder: (context, constraints) {
        final viewportWidth =
            constraints.maxWidth.isFinite && constraints.maxWidth > 0
            ? constraints.maxWidth
            : 720.0;
        final viewport = ui.Size(viewportWidth, widget.maxHeight);
        final fitScale = math.min(
          viewport.width / frame.width,
          viewport.height / frame.height,
        );
        final scale = _zoom ?? fitScale;
        final pictureSize = ui.Size(frame.width * scale, frame.height * scale);
        final pan = _clampPan(viewport, pictureSize, _pan);
        final position = ui.Offset(
          (viewport.width - pictureSize.width) / 2 + pan.dx,
          (viewport.height - pictureSize.height) / 2 + pan.dy,
        );
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            SizedBox(
              key: const ValueKey('calibrate-picture-area'),
              width: viewport.width,
              height: viewport.height,
              child: ClipRect(
                child: Listener(
                  onPointerSignal: _zoom == null
                      ? null
                      : (event) {
                          if (event is PointerScrollEvent) {
                            setState(() {
                              _pan = _clampPan(
                                viewport,
                                pictureSize,
                                _pan - event.scrollDelta,
                              );
                            });
                          }
                        },
                  child: Stack(
                    children: [
                      Positioned(
                        left: position.dx,
                        top: position.dy,
                        width: pictureSize.width,
                        height: pictureSize.height,
                        child: DecoratedBox(
                          decoration: BoxDecoration(
                            border: Border.all(
                              color: theme.colorScheme.outlineVariant,
                            ),
                          ),
                          child: _pictureArea(theme, bytes, pictureSize),
                        ),
                      ),
                      if (widget.mark != null && scale > 0)
                        Positioned(
                          left: position.dx,
                          top: position.dy,
                          width: pictureSize.width,
                          height: pictureSize.height,
                          child: IgnorePointer(
                            child: CustomPaint(
                              painter: _MarkPainter(
                                mark: widget.mark!,
                                scale: scale,
                                frameOrigin: ui.Offset(
                                  frame.origin.x.toDouble(),
                                  frame.origin.y.toDouble(),
                                ),
                                clientOrigin: widget.clientOrigin,
                                marquee: _marqueeLocal,
                                color: theme.colorScheme.primary,
                                label: widget.markLabel,
                                labelBackground: theme.colorScheme.primary,
                                labelForeground: theme.colorScheme.onPrimary,
                              ),
                            ),
                          ),
                        ),
                      if (_panMode && widget.interactive)
                        Positioned.fill(
                          child: GestureDetector(
                            key: const ValueKey('calibrate-pan-surface'),
                            behavior: HitTestBehavior.opaque,
                            onPanUpdate: (details) => setState(() {
                              _pan = _clampPan(
                                viewport,
                                pictureSize,
                                _pan + details.delta,
                              );
                            }),
                          ),
                        ),
                      Positioned(
                        right: ShellSpace.sm,
                        bottom: ShellSpace.sm,
                        child: _zoomToolbar(
                          theme,
                          fitScale: fitScale,
                          scale: scale,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
            const SizedBox(height: ShellSpace.xs),
            Wrap(
              spacing: ShellSpace.lg,
              runSpacing: ShellSpace.xs,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Text(
                  _panMode
                      ? 'Pan mode — drag to move the view; turn pan off to '
                            'draw marks.'
                      : widget.interactive
                      ? 'Marks are client-relative physical px on this '
                            'frame · click to set a point, drag to set a '
                            'rect.'
                      : 'Marks are client-relative physical px on this '
                            'frame.',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
                Text(
                  '${_zoom == null ? 'fit ' : ''}'
                  '${(scale * 100).round()}% · 1 display px = '
                  '${scale > 0 ? (1 / scale).toStringAsFixed(1) : '—'} '
                  'physical px',
                  key: const ValueKey('calibrate-zoom-line'),
                  style: ShellText.mono.copyWith(
                    fontSize: 11,
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ],
            ),
          ],
        );
      },
    );
  }

  /// `fit` / `1:1` / zoom in-out / pan — the demo's canvas control. Fit derives from the current
  /// layout; 1:1 is one display px per frame px; pan moves a view larger than the viewport.
  Widget _zoomToolbar(
    ThemeData theme, {
    required double fitScale,
    required double scale,
  }) {
    final colors = theme.colorScheme;
    final pct = (scale * 100).round();
    Widget smallIcon(
      String key,
      IconData icon,
      String tooltip,
      VoidCallback onPressed,
    ) => IconButton(
      key: ValueKey(key),
      tooltip: tooltip,
      onPressed: onPressed,
      icon: Icon(icon, size: ShellSize.iconSm),
      visualDensity: VisualDensity.compact,
      padding: EdgeInsets.zero,
      constraints: const BoxConstraints.tightFor(width: 28, height: 28),
    );
    final buttonStyle = TextButton.styleFrom(
      minimumSize: const Size(0, 28),
      padding: const EdgeInsets.symmetric(horizontal: ShellSpace.xs),
      visualDensity: VisualDensity.compact,
      textStyle: theme.textTheme.labelSmall,
    );
    return Material(
      color: colors.surfaceContainerHigh.withValues(alpha: 0.92),
      clipBehavior: Clip.antiAlias,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(ShellRadius.s),
        side: BorderSide(color: colors.outlineVariant),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: ShellSpace.xxs),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            smallIcon(
              'calibrate-zoom-out',
              Icons.remove,
              'Zoom out',
              () => _zoomBy(1 / _zoomStep, fitScale),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: ShellSpace.xs),
              child: Text(
                '${_zoom == null ? 'fit ' : ''}$pct%',
                key: const ValueKey('calibrate-zoom-label'),
                style: ShellText.mono.copyWith(fontSize: 11),
              ),
            ),
            smallIcon(
              'calibrate-zoom-in',
              Icons.add,
              'Zoom in',
              () => _zoomBy(_zoomStep, fitScale),
            ),
            TextButton(
              key: const ValueKey('calibrate-zoom-fit'),
              onPressed: _setFit,
              style: buttonStyle,
              child: const Text('Fit'),
            ),
            TextButton(
              key: const ValueKey('calibrate-zoom-one'),
              onPressed: _setOneToOne,
              style: buttonStyle,
              child: const Text('1:1'),
            ),
            IconButton(
              key: const ValueKey('calibrate-zoom-pan'),
              tooltip: 'Pan the view',
              isSelected: _panMode,
              onPressed: () => setState(() => _panMode = !_panMode),
              icon: const Icon(Icons.pan_tool_outlined, size: ShellSize.iconSm),
              selectedIcon: const Icon(Icons.pan_tool, size: ShellSize.iconSm),
              visualDensity: VisualDensity.compact,
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints.tightFor(width: 28, height: 28),
            ),
          ],
        ),
      ),
    );
  }

  ui.Rect? get _marqueeLocal {
    final start = _dragStartLocal;
    final current = _dragCurrentLocal;
    if (start == null || current == null) {
      return null;
    }
    return ui.Rect.fromPoints(start, current);
  }

  Widget _pictureArea(ThemeData theme, Uint8List bytes, ui.Size pictureSize) {
    final image = Image.memory(
      bytes,
      fit: BoxFit.fill,
      gaplessPlayback: true,
      errorBuilder: (context, error, stackTrace) => Padding(
        padding: const EdgeInsets.all(ShellSpace.sm),
        child: Text(
          'The captured picture could not be decoded.',
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.error,
          ),
        ),
      ),
    );
    if (!widget.interactive) {
      return image;
    }
    final frame = widget.frame;
    return GestureDetector(
      key: const ValueKey('calibrate-picture-gestures'),
      behavior: HitTestBehavior.opaque,
      onTapUp: widget.onPointMark == null
          ? null
          : (details) {
              final mark = framePointToMark(
                local: details.localPosition,
                box: pictureSize,
                frame: frame,
                clientOrigin: widget.clientOrigin,
              );
              if (mark != null) {
                widget.onPointMark!(mark);
              }
            },
      onPanStart: widget.onRectMark == null
          ? null
          : (details) => setState(() {
              _dragStartLocal = details.localPosition;
              _dragCurrentLocal = details.localPosition;
            }),
      onPanUpdate: widget.onRectMark == null
          ? null
          : (details) =>
                setState(() => _dragCurrentLocal = details.localPosition),
      onPanEnd: widget.onRectMark == null
          ? null
          : (details) {
              final start = _dragStartLocal;
              final current = _dragCurrentLocal;
              setState(() {
                _dragStartLocal = null;
                _dragCurrentLocal = null;
              });
              if (start == null || current == null) {
                return;
              }
              final mark = frameDragToMark(
                startLocal: start,
                endLocal: current,
                box: pictureSize,
                frame: frame,
                clientOrigin: widget.clientOrigin,
              );
              if (mark != null) {
                widget.onRectMark!(mark);
              }
            },
      child: image,
    );
  }
}

/// Paints the committed mark and the in-progress marquee. Coordinates are picture-local pixels: the
/// client-relative mark is mapped back with the same origin rule the input mapping uses. The mark's
/// label is drawn on the canvas so the selected surface is readable without reading the fields.
class _MarkPainter extends CustomPainter {
  _MarkPainter({
    required this.mark,
    required this.scale,
    required this.frameOrigin,
    required this.clientOrigin,
    required this.marquee,
    required this.color,
    this.label,
    this.labelBackground,
    this.labelForeground,
  });

  final CalibrationMark mark;
  final double scale;
  final ui.Offset frameOrigin;
  final ui.Offset clientOrigin;
  final ui.Rect? marquee;
  final Color color;

  /// The mark's label (`Bag grid · x 700 · y 400 · 900×500`), drawn above the shape when it fits.
  final String? label;
  final Color? labelBackground;
  final Color? labelForeground;

  ui.Offset get _originDelta => frameOrigin - clientOrigin;

  @override
  void paint(Canvas canvas, ui.Size size) {
    final stroke = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2;
    final originDelta = _originDelta;
    final ui.Rect shapeRect;
    if (mark.isRect) {
      final rect = ui.Rect.fromLTWH(
        (mark.x - originDelta.dx) * scale,
        (mark.y - originDelta.dy) * scale,
        mark.w * scale,
        mark.h * scale,
      );
      canvas.drawRect(rect, stroke);
      shapeRect = rect;
    } else {
      final center = ui.Offset(
        (mark.x - originDelta.dx) * scale,
        (mark.y - originDelta.dy) * scale,
      );
      canvas.drawLine(
        center - const ui.Offset(8, 0),
        center + const ui.Offset(8, 0),
        stroke,
      );
      canvas.drawLine(
        center - const ui.Offset(0, 8),
        center + const ui.Offset(0, 8),
        stroke,
      );
      shapeRect = ui.Rect.fromCenter(center: center, width: 0, height: 0);
    }
    final label = this.label;
    if (label != null && label.isNotEmpty) {
      _paintLabel(canvas, size, label, shapeRect);
    }
    final marquee = this.marquee;
    if (marquee != null) {
      final fill = Paint()
        ..color = color.withValues(alpha: 0.15)
        ..style = PaintingStyle.fill;
      canvas.drawRect(marquee, fill);
      canvas.drawRect(marquee, stroke);
    }
  }

  void _paintLabel(Canvas canvas, ui.Size size, String text, ui.Rect anchor) {
    final foreground = labelForeground ?? color;
    final painter = TextPainter(
      text: TextSpan(
        text: text,
        style: TextStyle(
          color: foreground,
          fontSize: 10.5,
          fontWeight: FontWeight.w600,
        ),
      ),
      maxLines: 1,
      ellipsis: '…',
      textDirection: TextDirection.ltr,
    )..layout(maxWidth: math.max(120.0, size.width - 4));
    final top = anchor.top - painter.height - 6 < 0
        ? anchor.top + 2
        : anchor.top - painter.height - 6;
    final left = anchor.left
        .clamp(0.0, math.max(0.0, size.width - painter.width - 8))
        .toDouble();
    final background = ui.Rect.fromLTWH(
      left,
      top,
      painter.width + 8,
      painter.height + 2,
    );
    canvas.drawRRect(
      RRect.fromRectAndRadius(background, const Radius.circular(4)),
      Paint()..color = labelBackground ?? color,
    );
    painter.paint(canvas, ui.Offset(left + 4, top + 1));
  }

  @override
  bool shouldRepaint(covariant _MarkPainter oldDelegate) =>
      oldDelegate.mark != mark ||
      oldDelegate.scale != scale ||
      oldDelegate.frameOrigin != frameOrigin ||
      oldDelegate.clientOrigin != clientOrigin ||
      oldDelegate.marquee != marquee ||
      oldDelegate.color != color ||
      oldDelegate.label != label ||
      oldDelegate.labelBackground != labelBackground ||
      oldDelegate.labelForeground != labelForeground;
}

/// A stored crop artifact (base64 PNG from `calibration.fetch`).
class CalibrationCropPicture extends StatelessWidget {
  const CalibrationCropPicture({
    super.key,
    required this.file,
    required this.pngBase64,
  });

  final String file;
  final String pngBase64;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final bytes = decodePngBase64(pngBase64);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text('Stored crop: $file', style: theme.textTheme.labelMedium),
        const SizedBox(height: ShellSpace.xs),
        if (bytes == null)
          Text(
            'The stored crop could not be decoded.',
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.error,
            ),
          )
        else
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 320, maxHeight: 240),
            child: Image.memory(
              bytes,
              fit: BoxFit.contain,
              gaplessPlayback: true,
              errorBuilder: (context, error, stackTrace) => Text(
                'The stored crop could not be decoded.',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),
            ),
          ),
      ],
    );
  }
}

/// The picture rectangle inside a [box] for a `BoxFit.contain` render of a frame, plus the scale.
///
/// The frame is never stretched: [scale] is `min(box.w/frame.w, box.h/frame.h)`, and [picture] is the
/// centred rectangle the pixels occupy. Everything outside [picture] is letterbox — the mapping
/// helpers below ignore clicks there (a letterbox click must not become a mark).
({ui.Rect picture, double scale}) frameContainLayout({
  required ui.Size box,
  required int frameWidth,
  required int frameHeight,
}) {
  if (box.isEmpty || frameWidth <= 0 || frameHeight <= 0) {
    return (picture: ui.Rect.zero, scale: 0);
  }
  final scale = math.min(box.width / frameWidth, box.height / frameHeight);
  final width = frameWidth * scale;
  final height = frameHeight * scale;
  return (
    picture: ui.Rect.fromLTWH(
      (box.width - width) / 2,
      (box.height - height) / 2,
      width,
      height,
    ),
    scale: scale,
  );
}

/// Maps a local offset in the picture BOX to a frame-local pixel, handling `BoxFit.contain`
/// letterboxing. Returns null outside the picture (letterbox) unless [clampInside] is true, in which
/// case the offset is clamped to the picture's edge.
///
/// ROUNDING RULE: the pixel is `((local - picture.origin) / scale).round()`, then clamped to
/// `0 .. frameWidth-1` / `0 .. frameHeight-1`. The frame's last pixel is reachable even when the
/// display rectangle's right/bottom edge rounds past it.
ui.Offset? framePixelFromLocal({
  required ui.Offset local,
  required ui.Size box,
  required int frameWidth,
  required int frameHeight,
  bool clampInside = false,
}) {
  final layout = frameContainLayout(
    box: box,
    frameWidth: frameWidth,
    frameHeight: frameHeight,
  );
  if (layout.scale <= 0) {
    return null;
  }
  final picture = layout.picture;
  if (!clampInside && !picture.contains(local)) {
    return null;
  }
  final x = clampInside
      ? local.dx.clamp(picture.left, picture.right)
      : local.dx;
  final y = clampInside
      ? local.dy.clamp(picture.top, picture.bottom)
      : local.dy;
  final pixelX = ((x - picture.left) / layout.scale).round().clamp(
    0,
    frameWidth - 1,
  );
  final pixelY = ((y - picture.top) / layout.scale).round().clamp(
    0,
    frameHeight - 1,
  );
  return ui.Offset(pixelX.toDouble(), pixelY.toDouble());
}

/// A frame-local pixel in the CLIENT-RELATIVE physical mark space.
///
/// Screen pixel = [frameOrigin] + frame-local pixel; the client-relative space starts at
/// [clientOrigin] on screen, so the mark is `frame-local + frameOrigin - clientOrigin`. The calibration
/// frame is the whole client, so the two origins are equal and the offset is zero; a sub-region
/// capture passes its own origins and the offset is subtracted. This shells the difference and is
/// documented as the one place that knows the frame-origin convention.
ui.Offset pixelToClientMark({
  required ui.Offset pixel,
  required ui.Offset frameOrigin,
  required ui.Offset clientOrigin,
}) => pixel + frameOrigin - clientOrigin;

/// Click position (picture box local) → a point mark, or null when the click landed in the letterbox.
CalibrationMark? framePointToMark({
  required ui.Offset local,
  required ui.Size box,
  required Frame frame,
  required ui.Offset clientOrigin,
}) {
  final pixel = framePixelFromLocal(
    local: local,
    box: box,
    frameWidth: frame.width,
    frameHeight: frame.height,
  );
  if (pixel == null) {
    return null;
  }
  final mark = pixelToClientMark(
    pixel: pixel,
    frameOrigin: ui.Offset(
      frame.origin.x.toDouble(),
      frame.origin.y.toDouble(),
    ),
    clientOrigin: clientOrigin,
  );
  return CalibrationMark.point(mark.dx.round(), mark.dy.round());
}

/// A drag (picture box local start/end) → a rect mark in client-relative physical px, or null when the
/// drag is too short to be a rect (a click) or landed entirely in the letterbox.
///
/// RULE: both drag ends are clamped into the picture (an end outside it stops at the edge rather than
/// vanishing), converted to pixels, normalised (bottom-right→top-left yields the same rect), and the
/// inclusive size is used: `w = maxPixelX - minPixelX + 1` (same for h). A drag shorter than
/// [minimumDragPixels] in DISPLAY pixels never becomes a rect — it is a click.
CalibrationMark? frameDragToMark({
  required ui.Offset startLocal,
  required ui.Offset endLocal,
  required ui.Size box,
  required Frame frame,
  required ui.Offset clientOrigin,
  double minimumDragPixels = 4,
}) {
  if ((endLocal - startLocal).distance < minimumDragPixels) {
    return null;
  }
  final start = framePixelFromLocal(
    local: startLocal,
    box: box,
    frameWidth: frame.width,
    frameHeight: frame.height,
    clampInside: true,
  );
  final end = framePixelFromLocal(
    local: endLocal,
    box: box,
    frameWidth: frame.width,
    frameHeight: frame.height,
    clampInside: true,
  );
  if (start == null || end == null) {
    return null;
  }
  final origin = ui.Offset(
    frame.origin.x.toDouble(),
    frame.origin.y.toDouble(),
  );
  final a = pixelToClientMark(
    pixel: start,
    frameOrigin: origin,
    clientOrigin: clientOrigin,
  );
  final b = pixelToClientMark(
    pixel: end,
    frameOrigin: origin,
    clientOrigin: clientOrigin,
  );
  final left = math.min(a.dx, b.dx).round();
  final top = math.min(a.dy, b.dy).round();
  final right = math.max(a.dx, b.dx).round();
  final bottom = math.max(a.dy, b.dy).round();
  return CalibrationMark.rect(left, top, right - left + 1, bottom - top + 1);
}

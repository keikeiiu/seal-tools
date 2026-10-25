/// Separable calibration view pieces: the captured frame picture and the mark/crop readers.
///
/// They are deliberately standalone so the wave-2 Calibrate surface and the read-only Pet/Setup
/// geometry panels can share them later (the plan's "one CalibrationCanvas" rule) without pulling the
/// tab's state along.
library;

import 'dart:convert';
import 'dart:ui' as ui;
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../protocol/protocol.dart';
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

/// The captured frame, bounded on screen, with an OPTIONAL gesture overlay: a click sets a point
/// mark, a drag sets a rect mark. Everything is reported as a client-relative physical-pixel
/// [CalibrationMark]; the widget maps coordinates and draws the marquee/committed mark, nothing else.
///
/// The typed fields stay authoritative: the tab passes [mark] in (built from its fields) and writes
/// each gesture's mark back into those fields, so Save always reads the fields, never hidden picture
/// state. Omit the callbacks for a read-only picture (the Pet/Setup reuse case).
class CalibrationFramePicture extends StatefulWidget {
  const CalibrationFramePicture({
    super.key,
    required this.frame,
    this.maxHeight = 320,
    this.mark,
    this.clientOrigin = ui.Offset.zero,
    this.onPointMark,
    this.onRectMark,
  });

  final Frame frame;
  final double maxHeight;

  /// The mark currently in the typed fields, drawn on the picture.
  final CalibrationMark? mark;

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
          LayoutBuilder(
            builder: (context, constraints) {
              final maxWidth = constraints.maxWidth.isFinite
                  ? math.min(constraints.maxWidth, 720.0)
                  : 720.0;
              final box = ui.Size(
                maxWidth,
                math.min(widget.maxHeight, boxHeightForWidth(maxWidth, frame)),
              );
              final layout = frameContainLayout(
                box: box,
                frameWidth: frame.width,
                frameHeight: frame.height,
              );
              final picture = layout.picture;
              final clientOrigin = widget.clientOrigin;
              return SizedBox(
                key: const ValueKey('calibrate-picture-area'),
                width: box.width,
                height: box.height,
                child: Stack(
                  children: [
                    Positioned.fromRect(
                      rect: picture,
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          border: Border.all(
                            color: theme.colorScheme.outlineVariant,
                          ),
                        ),
                        child: _pictureArea(theme, bytes, picture),
                      ),
                    ),
                    if (widget.mark != null && layout.scale > 0)
                      Positioned.fromRect(
                        rect: picture,
                        child: IgnorePointer(
                          child: CustomPaint(
                            painter: _MarkPainter(
                              mark: widget.mark!,
                              scale: layout.scale,
                              frameOrigin: ui.Offset(
                                frame.origin.x.toDouble(),
                                frame.origin.y.toDouble(),
                              ),
                              clientOrigin: clientOrigin,
                              marquee: _marqueeLocal,
                              color: theme.colorScheme.primary,
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              );
            },
          ),
        const SizedBox(height: ShellSpace.xs),
        Text(
          widget.interactive
              ? 'Marks are client-relative physical px on this frame · '
                    'click to set a point, drag to set a rect.'
              : 'Marks are client-relative physical px on this frame.',
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ],
    );
  }

  /// Keep the displayed box at the frame's aspect ratio so the layout has no letterbox at all; the
  /// mapping helper still handles the general contain case (the widget simply gives it an exact box).
  double boxHeightForWidth(double width, Frame frame) =>
      width * frame.height / frame.width;

  ui.Rect? get _marqueeLocal {
    final start = _dragStartLocal;
    final current = _dragCurrentLocal;
    if (start == null || current == null) {
      return null;
    }
    return ui.Rect.fromPoints(start, current);
  }

  Widget _pictureArea(ThemeData theme, Uint8List bytes, ui.Rect picture) {
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
                box: picture.size,
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
                box: picture.size,
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
/// client-relative mark is mapped back with the same origin rule the input mapping uses.
class _MarkPainter extends CustomPainter {
  _MarkPainter({
    required this.mark,
    required this.scale,
    required this.frameOrigin,
    required this.clientOrigin,
    required this.marquee,
    required this.color,
  });

  final CalibrationMark mark;
  final double scale;
  final ui.Offset frameOrigin;
  final ui.Offset clientOrigin;
  final ui.Rect? marquee;
  final Color color;

  ui.Offset get _originDelta => frameOrigin - clientOrigin;

  @override
  void paint(Canvas canvas, ui.Size size) {
    final stroke = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2;
    final originDelta = _originDelta;
    if (mark.isRect) {
      final rect = ui.Rect.fromLTWH(
        (mark.x - originDelta.dx) * scale,
        (mark.y - originDelta.dy) * scale,
        mark.w * scale,
        mark.h * scale,
      );
      canvas.drawRect(rect, stroke);
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

  @override
  bool shouldRepaint(covariant _MarkPainter oldDelegate) =>
      oldDelegate.mark != mark ||
      oldDelegate.scale != scale ||
      oldDelegate.frameOrigin != frameOrigin ||
      oldDelegate.clientOrigin != clientOrigin ||
      oldDelegate.marquee != marquee ||
      oldDelegate.color != color;
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

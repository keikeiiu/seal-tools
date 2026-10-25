/// Separable calibration view pieces: the captured frame picture and the mark/crop readers.
///
/// They are deliberately standalone so the wave-2 Calibrate surface and the read-only Pet/Setup
/// geometry panels can share them later (the plan's "one CalibrationCanvas" rule) without pulling the
/// tab's state along.
library;

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';

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

/// The captured frame, bounded on screen. The pixel size and origin are displayed so the player knows
/// exactly which pixels the marks refer to.
class CalibrationFramePicture extends StatelessWidget {
  const CalibrationFramePicture({
    super.key,
    required this.frame,
    this.maxHeight = 320,
  });

  final Frame frame;
  final double maxHeight;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
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
          ConstrainedBox(
            constraints: BoxConstraints(maxWidth: 720, maxHeight: maxHeight),
            child: DecoratedBox(
              decoration: BoxDecoration(
                border: Border.all(color: theme.colorScheme.outlineVariant),
              ),
              child: Image.memory(
                bytes,
                fit: BoxFit.contain,
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
              ),
            ),
          ),
        const SizedBox(height: ShellSpace.xs),
        Text(
          'Marks are client-relative physical px on this frame.',
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ],
    );
  }
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

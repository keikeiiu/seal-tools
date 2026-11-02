/// The Calibrate workspace's reference-image gallery: what image artifacts actually exist, and what
/// each one is.
///
/// Every reference screenshot is written by a v3 session save and echoed by the core's
/// `calibration.fetch`/`fetch_all` `gallery`:
///
/// - `calib_tuner.png` — Calibrate Tuner's Save (the three OCR bands drawn);
/// - `calib_gem.png` — Calibrate Gem's Save (click points + result box);
/// - `calib_pet.png` — Calibrate Pet's Save (row boxes/strips);
/// - `calib_gem_result.png` — the result-gem crop, also the empty-result reference.
///
/// A missing file is said to be missing — no placeholder is dressed up as a capture, and there is no
/// "v2 only" state any more: v3 writes all four.
///
/// [CALIBRATION.md]: ../../../docs/spec/CALIBRATION.md
library;

import 'package:flutter/material.dart';

import '../../../app/app.dart';
import '../../components/status_chip.dart';
import '../../components/surface_panel.dart';
import '../../theme.dart';
import '../calibration_widgets.dart' show CalibrationCropPicture;

class ReferenceGalleryPanel extends StatelessWidget {
  const ReferenceGalleryPanel({
    super.key,
    required this.stored,
    this.gallery = const [],
  });

  /// The stored-map fetches: `result_gem_area`'s artifact is the empty-result crop.
  final Map<String, CalibrationFetchResult> stored;

  /// The session saves' annotated screenshots, as the core echoes them.
  final List<CalibrationGalleryEntry> gallery;

  CalibrationGalleryEntry? _entry(String name) {
    for (final entry in gallery) {
      if (entry.name == name) return entry;
    }
    return null;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final resultCrop = stored['result_gem_area']?.artifact;
    final resultEntry = _entry('gem_result');
    return SurfacePanel(
      title: 'Reference images',
      padding: const EdgeInsets.all(ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _GalleryRow(
            key: const ValueKey('calibrate-gallery-result-gem'),
            icon: Icons.image_outlined,
            title: 'Result gem crop',
            description: resultCrop == null && resultEntry == null
                ? 'calib_gem_result.png — not captured. Save the result gem area with '
                      '"the box is empty now" to write the strong empty-result reference.'
                : 'calib_gem_result.png — present. The strong empty-result test; the colour '
                      'signature is only its fallback.',
            chip: resultCrop == null && resultEntry == null
                ? const _GalleryChip('not captured', StatusKind.muted)
                : const _GalleryChip('present', StatusKind.success),
            preview: resultCrop != null
                ? CalibrationCropPicture(
                    file: resultCrop.file,
                    pngBase64: resultCrop.pngBase64,
                  )
                : resultEntry != null
                ? CalibrationCropPicture(
                    file: resultEntry.file,
                    pngBase64: resultEntry.pngBase64,
                  )
                : null,
          ),
          _referenceRow(
            key: const ValueKey('calibrate-gallery-tuner'),
            name: 'tuner',
            title: 'calib_tuner.png (OCR bands drawn)',
            description:
                'Written by Calibrate Tuner → Save. The captured frame with the grade, '
                'attribute and remaining bands outlined.',
          ),
          _referenceRow(
            key: const ValueKey('calibrate-gallery-gem'),
            name: 'gem',
            title: 'calib_gem.png (points drawn)',
            description:
                'Written by Calibrate Gem → Save. The captured frame with the grade/Register/'
                'Combine and resource points dotted and the result box outlined.',
          ),
          _referenceRow(
            key: const ValueKey('calibrate-gallery-pet'),
            name: 'pet',
            title: 'calib_pet.png (row marks drawn)',
            description:
                'Written by Calibrate Pet → Save. The captured frame with each row\'s toggle, '
                'boarding slot and feeder strip outlined.',
          ),
          _GalleryRow(
            key: const ValueKey('calibrate-gallery-pet-crops'),
            icon: Icons.pets_outlined,
            title: 'Pet slot-empty / close-X crops',
            description:
                'Captured from the running game on the Pet tab and by the Pet run '
                '(the close-X crop is learned); stored as base64 crops in the config.',
            chip: const _GalleryChip('run-learned', StatusKind.info),
          ),
          const SizedBox(height: ShellSpace.xxs),
          Text(
            'All four reference screenshots are written by v3 session saves; a row says '
            '"not captured" until its Save has run.',
            key: const ValueKey('calibrate-gallery-note'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }

  Widget _referenceRow({
    required Key key,
    required String name,
    required String title,
    required String description,
  }) {
    final entry = _entry(name);
    return _GalleryRow(
      key: key,
      icon: Icons.article_outlined,
      title: title,
      description: description,
      chip: entry == null
          ? const _GalleryChip('not captured', StatusKind.muted)
          : const _GalleryChip('present', StatusKind.success),
      preview: entry == null
          ? null
          : CalibrationCropPicture(
              file: entry.file,
              pngBase64: entry.pngBase64,
            ),
    );
  }
}

class _GalleryRow extends StatelessWidget {
  const _GalleryRow({
    super.key,
    required this.icon,
    required this.title,
    required this.description,
    required this.chip,
    this.preview,
  });

  final IconData icon;
  final String title;
  final String description;
  final Widget chip;
  final Widget? preview;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final preview = this.preview;
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.sm),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xxs),
            child: Icon(icon, size: ShellSize.iconSm, color: colors.outline),
          ),
          const SizedBox(width: ShellSpace.sm),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Wrap(
                  spacing: ShellSpace.sm,
                  runSpacing: ShellSpace.xxs,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Text(title, style: theme.textTheme.labelMedium),
                    chip,
                  ],
                ),
                Text(
                  description,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: colors.onSurfaceVariant,
                  ),
                ),
                if (preview != null) ...[
                  const SizedBox(height: ShellSpace.xs),
                  preview,
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _GalleryChip extends StatelessWidget {
  const _GalleryChip(this.label, this.kind);

  final String label;
  final StatusKind kind;

  @override
  Widget build(BuildContext context) => StatusChip(kind: kind, label: label);
}

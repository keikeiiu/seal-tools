/// The Calibrate workspace's reference-image gallery: what image artifacts actually exist, and what
/// each one is.
///
/// Truthful states only (CALIBRATION §5/§7): the result-gem crop is the ONE shipped binary artifact and
/// it exists only after an `empty: true` save; the pet crops are learned by the Pet run (not frame
/// marks); `calib_tuner.png`/`calib_gem.png` are v2 conventions no v3 tool reads. A missing image is
/// said to be missing — no placeholder is dressed up as a capture.
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
  const ReferenceGalleryPanel({super.key, required this.stored});

  /// The stored-map fetches: `result_gem_area`'s artifact is the crop, when present.
  final Map<String, CalibrationFetchResult> stored;

  @override
  Widget build(BuildContext context) {
    final crop = stored['result_gem_area']?.artifact;
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
            description: crop == null
                ? 'calib_gem_result.png — not captured. Save the result gem area with '
                      '"the box is empty now" to write the strong empty-result reference.'
                : 'calib_gem_result.png — present. The strong empty-result test; the colour '
                      'signature is only its fallback.',
            chip: crop == null
                ? const _GalleryChip('not captured', StatusKind.muted)
                : const _GalleryChip('present', StatusKind.success),
            preview: crop == null
                ? null
                : CalibrationCropPicture(
                    file: crop.file,
                    pngBase64: crop.pngBase64,
                  ),
          ),
          _GalleryRow(
            key: const ValueKey('calibrate-gallery-pet-crops'),
            icon: Icons.pets_outlined,
            title: 'Pet slot-empty / close-X crops',
            description:
                'Captured from the running game on the Pet tab and by the Pet run '
                '(the close-X crop is learned); stored as base64 crops in local.yaml. '
                'Session features, not frame marks.',
            chip: const _GalleryChip('run-learned', StatusKind.info),
          ),
          _GalleryRow(
            key: const ValueKey('calibrate-gallery-v2-tuner'),
            icon: Icons.article_outlined,
            title: 'calib_tuner.png (bands drawn)',
            description:
                'v2 wrote a reference screenshot with the OCR bands drawn on it. v3 does not, '
                'and no tool reads it — kept only for migration.',
            chip: const _GalleryChip('v2 only', StatusKind.muted),
          ),
          _GalleryRow(
            key: const ValueKey('calibrate-gallery-v2-gem'),
            icon: Icons.article_outlined,
            title: 'calib_gem.png (points drawn)',
            description:
                'v2 wrote a reference screenshot with the click points and result box drawn on '
                'it. v3 does not, and no tool reads it — kept only for migration.',
            chip: const _GalleryChip('v2 only', StatusKind.muted),
          ),
          const SizedBox(height: ShellSpace.xxs),
          Text(
            'One artifact is written by v3 today: the result-gem crop. The other entries are the '
            'honest state of v2 images and run-learned crops.',
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
        ],
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
                    Text(title, style: theme.textTheme.labelLarge),
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

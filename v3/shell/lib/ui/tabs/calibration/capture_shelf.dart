/// The Calibrate workspace's capture shelf: the frames THIS screen has taken, exactly as the core's
/// four-frame FIFO sees them.
///
/// Every save references a LIVE `frame_id` (PROTOCOL §8): the shelf therefore shows which capture is
/// selected, pairs each frame with the `DisplayInfo` returned by ITS capture (a display is measured
/// per capture, never cached — CALIBRATION §4), marks the oldest frame as the next one evicted, and
/// says when the core refused a frame id. An evicted/unusable frame is never re-captured
/// automatically: the player presses Capture again.
///
/// [CALIBRATION.md]: ../../../docs/spec/CALIBRATION.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'package:flutter/material.dart';

import '../../../protocol/protocol.dart';
import '../../components/empty_state.dart';
import '../../components/surface_panel.dart';
import '../../theme.dart';
import '../calibration_widgets.dart' show decodePngBase64;
import 'calibration_catalog.dart';

/// The demo's shelf thumbnail size (`ui-demo.html` #calibrate `.capthumb`).
const double _thumbWidth = 52;
const double _thumbHeight = 34;

/// One capture this screen took: the frame, the measurement that came WITH it, the screen's own
/// sequence number (the core's ids are `f1`, `f2`, … and are not itself an ordinal) and the time.
class CalibrationCapture {
  const CalibrationCapture({
    required this.frame,
    required this.display,
    required this.sequence,
    required this.capturedAt,
  });

  final Frame frame;
  final DisplayInfo display;

  /// 1-based capture order on this screen, only for the label.
  final int sequence;
  final DateTime capturedAt;

  String get frameId => frame.frameId;

  String get label => 'Frame $sequence';

  String get timeLabel =>
      '${capturedAt.hour.toString().padLeft(2, '0')}:'
      '${capturedAt.minute.toString().padLeft(2, '0')}';

  String get sizeLabel => '${frame.width}×${frame.height} physical';

  /// That capture's own `DisplayInfo`, short form: scale and physical client size.
  String get displayLabel =>
      '${formatCalibrationScale(display.scale)}× · '
      '${display.physicalClient.w}×${display.physicalClient.h} client';
}

/// The Captures panel: the four retained frames, newest first, plus the capture action and whatever
/// status the tab wants under the list (progress line, core refusal, host note).
class CaptureShelfPanel extends StatelessWidget {
  const CaptureShelfPanel({
    super.key,
    required this.captures,
    required this.selectedFrameId,
    required this.onSelect,
    required this.onCapture,
    required this.capturing,
    this.refusedFrameIds = const <String>{},
    this.status,
    this.note,
  });

  /// Newest first. At most four; the tab enforces the FIFO on insert.
  final List<CalibrationCapture> captures;
  final String? selectedFrameId;
  final ValueChanged<String> onSelect;
  final VoidCallback onCapture;
  final bool capturing;

  /// Frame ids the core refused as `unknown frame_id`: an unusable capture needs a new one.
  final Set<String> refusedFrameIds;

  /// Status widgets shown under the list (capture result, host note, empty-capture copy).
  final Widget? status;

  /// An optional shelf line (e.g. the eviction note).
  final String? note;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final oldestId = captures.length >= 4 ? captures.last.frameId : null;
    final status = this.status;
    final note = this.note;
    return SurfacePanel(
      title: 'Captures',
      trailing: Text(
        '${captures.length} / 4',
        style: theme.textTheme.labelSmall?.copyWith(
          color: colors.onSurfaceVariant,
        ),
      ),
      padding: const EdgeInsets.all(ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (captures.isEmpty)
            EmptyState(
              icon: Icons.photo_camera_outlined,
              title: 'No capture yet',
              hint:
                  'Capture the window to see the frame and draw marks. The core keeps the last '
                  'four frames; a fifth evicts the oldest.',
              compact: true,
            )
          else
            for (final capture in captures)
              _CaptureTile(
                capture: capture,
                selected: capture.frameId == selectedFrameId,
                oldest: capture.frameId == oldestId,
                refused: refusedFrameIds.contains(capture.frameId),
                onSelect: () => onSelect(capture.frameId),
              ),
          const SizedBox(height: ShellSpace.sm),
          OutlinedButton.icon(
            key: const ValueKey('calibrate-capture-shelf'),
            onPressed: capturing ? null : onCapture,
            icon: const Icon(
              Icons.add_a_photo_outlined,
              size: ShellSize.iconSm,
            ),
            label: Text(
              capturing
                  ? 'Capturing…'
                  : (captures.isEmpty
                        ? 'Capture the window'
                        : 'Capture another'),
            ),
          ),
          if (status != null) ...[
            const SizedBox(height: ShellSpace.sm),
            status,
          ],
          if (note != null) ...[
            const SizedBox(height: ShellSpace.xs),
            Text(
              note,
              key: const ValueKey('calibrate-shelf-note'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: colors.onSurfaceVariant,
              ),
            ),
          ],
          const SizedBox(height: ShellSpace.sm),
          Text(
            'Saving needs a live frame id. Marks store the display environment, not the frame; an '
            'evicted frame is refused, never re-captured automatically.',
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }
}

class _CaptureTile extends StatelessWidget {
  const _CaptureTile({
    required this.capture,
    required this.selected,
    required this.oldest,
    required this.refused,
    required this.onSelect,
  });

  final CalibrationCapture capture;
  final bool selected;
  final bool oldest;
  final bool refused;
  final VoidCallback onSelect;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final borderColor = selected ? colors.primary : colors.outlineVariant;
    final bytes = decodePngBase64(capture.frame.data);
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.sm - ShellSpace.xxs),
      child: Material(
        color: selected
            ? colors.primaryContainer.withValues(alpha: 0.35)
            : colors.surfaceContainerLowest,
        clipBehavior: Clip.antiAlias,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(ShellRadius.s),
          side: BorderSide(color: borderColor, width: selected ? 1.5 : 0.5),
        ),
        child: InkWell(
          key: ValueKey('calibrate-capture-item-${capture.frameId}'),
          onTap: onSelect,
          child: Padding(
            padding: const EdgeInsets.all(ShellSpace.sm),
            child: Row(
              children: [
                ClipRRect(
                  borderRadius: BorderRadius.circular(ShellRadius.s),
                  child: SizedBox(
                    width: _thumbWidth,
                    height: _thumbHeight,
                    child: bytes == null
                        ? ColoredBox(color: colors.surfaceContainerHighest)
                        : Image.memory(
                            bytes,
                            fit: BoxFit.cover,
                            gaplessPlayback: true,
                            errorBuilder: (context, error, stackTrace) =>
                                ColoredBox(
                                  color: colors.surfaceContainerHighest,
                                ),
                          ),
                  ),
                ),
                const SizedBox(width: ShellSpace.sm),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          if (selected) ...[
                            Icon(
                              Icons.check_circle,
                              size: ShellSize.iconSm,
                              color: colors.primary,
                            ),
                            const SizedBox(width: ShellSpace.xxs),
                          ],
                          Flexible(
                            child: Text(
                              '${capture.label} · ${capture.timeLabel}',
                              style: theme.textTheme.labelLarge,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        ],
                      ),
                      Text(
                        '${capture.sizeLabel} · ${capture.displayLabel}',
                        style: ShellText.mono.copyWith(
                          fontSize: 11,
                          color: colors.onSurfaceVariant,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                      Text(
                        [
                          'id ${capture.frameId}',
                          if (refused) 'refused by the core — capture again',
                          if (oldest) 'next capture evicts this one',
                        ].join(' · '),
                        key: ValueKey(
                          'calibrate-capture-marker-${capture.frameId}',
                        ),
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: refused
                              ? colors.error
                              : oldest
                              ? context.shellColors.warning
                              : colors.onSurfaceVariant,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

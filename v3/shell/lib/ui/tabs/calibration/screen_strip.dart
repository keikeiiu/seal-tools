/// The Calibrate workspace's Screens strip: every live capture, newest first, with the assignment
/// count and the one assigned to the selected target marked. A click only PREVIEWS a screen; the
/// editor's explicit action assigns it, so a stray click cannot silently repoint a target.
library;

import 'package:flutter/material.dart';

import '../../components/surface_panel.dart';
import '../../theme.dart';
import 'capture_shelf.dart';

/// Height of one capture thumb; the strip keeps the canvas above the fold at 720 px.
const double _stripThumbHeight = 48;
const double _stripTileWidth = 118;

class ScreenStripPanel extends StatelessWidget {
  const ScreenStripPanel({
    super.key,
    required this.captures,
    required this.selectedFrameId,
    required this.assignedFrameId,
    required this.assignmentCounts,
    required this.onSelect,
    required this.onCapture,
    required this.capturing,
    required this.capacity,
    required this.targetLabel,
    this.assignedLabel = 'assigned',
    this.onDiscard,
    this.refusedFrameIds = const <String>{},
  });

  /// Newest first; at most [capacity] because the core keeps that many live frames.
  final List<CalibrationCapture> captures;
  final String? selectedFrameId;

  /// The frame assigned to the target currently selected in the editor.
  final String? assignedFrameId;
  final Map<String, int> assignmentCounts;
  final ValueChanged<String> onSelect;
  final VoidCallback onCapture;
  final bool capturing;
  final int capacity;
  final String targetLabel;

  /// `assigned` for mark targets; `using` for a session's current reused screen.
  final String assignedLabel;

  /// Explicit delete, after the parent's two-step confirmation. Null hides the delete affordance.
  final ValueChanged<String>? onDiscard;

  /// Frame ids the core refused as `unknown frame_id`.
  final Set<String> refusedFrameIds;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    return SurfacePanel(
      key: const ValueKey('calibrate-screen-strip'),
      padding: const EdgeInsets.fromLTRB(
        ShellSpace.sm,
        ShellSpace.xs,
        ShellSpace.sm,
        ShellSpace.sm,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Text(
                'Screens',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: colors.onSurfaceVariant,
                  letterSpacing: 0.08,
                ),
              ),
              const SizedBox(width: ShellSpace.sm),
              Text(
                '${captures.length}/$capacity live',
                key: const ValueKey('calibrate-screen-count'),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: colors.onSurfaceVariant,
                ),
              ),
              const Spacer(),
              Text(
                targetLabel,
                key: const ValueKey('calibrate-screen-target'),
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: colors.onSurfaceVariant,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ),
          const SizedBox(height: ShellSpace.xs),
          SizedBox(
            height: _stripThumbHeight + 40,
            child: ListView.separated(
              scrollDirection: Axis.horizontal,
              itemCount: captures.length + 1,
              separatorBuilder: (context, index) =>
                  const SizedBox(width: ShellSpace.sm),
              itemBuilder: (context, index) {
                if (index == 0) {
                  return _captureTile(theme);
                }
                final capture = captures[index - 1];
                return _screenTile(theme, capture);
              },
            ),
          ),
        ],
      ),
    );
  }

  Widget _captureTile(ThemeData theme) {
    final colors = theme.colorScheme;
    return SizedBox(
      width: _stripTileWidth,
      child: Material(
        color: colors.surfaceContainerLow,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(ShellRadius.s),
          side: BorderSide(color: colors.outlineVariant),
        ),
        child: InkWell(
          key: const ValueKey('calibrate-screen-capture'),
          borderRadius: BorderRadius.circular(ShellRadius.s),
          onTap: capturing ? null : onCapture,
          child: Padding(
            padding: const EdgeInsets.all(ShellSpace.sm),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                if (capturing)
                  const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                else
                  Icon(
                    Icons.photo_camera_outlined,
                    size: ShellSize.iconMd,
                    color: colors.primary,
                  ),
                const SizedBox(height: ShellSpace.xs),
                Text(
                  captures.isEmpty ? 'Capture screen' : 'Capture again',
                  textAlign: TextAlign.center,
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: colors.onSurfaceVariant,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _screenTile(ThemeData theme, CalibrationCapture capture) {
    final colors = theme.colorScheme;
    final selected = capture.frameId == selectedFrameId;
    final assigned = capture.frameId == assignedFrameId;
    final refused = refusedFrameIds.contains(capture.frameId);
    final count = assignmentCounts[capture.frameId] ?? 0;
    final borderColor = refused
        ? colors.error
        : selected
        ? colors.primary
        : assigned
        ? theme.shellColors.success
        : colors.outlineVariant;
    return SizedBox(
      width: _stripTileWidth,
      child: Material(
        color: colors.surfaceContainerLow,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(ShellRadius.s),
          side: BorderSide(
            color: borderColor,
            width: selected || assigned || refused ? 1.5 : 1,
          ),
        ),
        child: InkWell(
          key: ValueKey('calibrate-screen-${capture.frameId}'),
          borderRadius: BorderRadius.circular(ShellRadius.s),
          onTap: () => onSelect(capture.frameId),
          child: Padding(
            padding: const EdgeInsets.all(ShellSpace.xs),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                ClipRRect(
                  borderRadius: BorderRadius.circular(4),
                  child: SizedBox(
                    height: _stripThumbHeight,
                    child: capture.bytes == null
                        ? ColoredBox(color: colors.surfaceContainerHigh)
                        : Image.memory(
                            capture.bytes!,
                            fit: BoxFit.cover,
                            gaplessPlayback: true,
                            errorBuilder: (context, error, stackTrace) =>
                                ColoredBox(
                                  color: colors.surfaceContainerHigh,
                                  child: Icon(
                                    Icons.broken_image_outlined,
                                    size: ShellSize.iconMd,
                                    color: colors.onSurfaceVariant,
                                  ),
                                ),
                          ),
                  ),
                ),
                const SizedBox(height: ShellSpace.xxs),
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        capture.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: colors.onSurface,
                        ),
                      ),
                    ),
                    if (refused)
                      Icon(Icons.error_outline, size: 12, color: colors.error)
                    else if (count > 0)
                      Text(
                        '✓$count',
                        key: ValueKey(
                          'calibrate-screen-count-${capture.frameId}',
                        ),
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: theme.shellColors.success,
                        ),
                      ),
                    if (onDiscard != null)
                      SizedBox(
                        width: 16,
                        height: 16,
                        child: IconButton(
                          key: ValueKey(
                            'calibrate-screen-delete-${capture.frameId}',
                          ),
                          padding: EdgeInsets.zero,
                          constraints: const BoxConstraints.tightFor(
                            width: 16,
                            height: 16,
                          ),
                          tooltip: 'Delete screen',
                          onPressed: () => onDiscard!(capture.frameId),
                          icon: Icon(
                            Icons.close,
                            size: 12,
                            color: colors.onSurfaceVariant,
                          ),
                        ),
                      ),
                  ],
                ),
                SizedBox(
                  height: 12,
                  child: assigned
                      ? Text(
                          assignedLabel,
                          key: ValueKey(
                            'calibrate-screen-assigned-${capture.frameId}',
                          ),
                          style: theme.textTheme.labelSmall?.copyWith(
                            color: theme.shellColors.success,
                          ),
                        )
                      : refused
                      ? Text(
                          'evicted',
                          style: theme.textTheme.labelSmall?.copyWith(
                            color: colors.error,
                          ),
                        )
                      : null,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

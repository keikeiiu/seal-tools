/// The **mini running bar**: the shell window while a foreground tool runs.
///
/// A foreground run collapses the whole shell to a 360 × 96 borderless, always-on-top bar beside the
/// game (the model owns the geometry, the chrome switch and the always-on-top pin). The bar shows
/// what matters at a glance and in this order — tool + phase, the core's current action, the next
/// moment — plus ONE primary action: **Stop** while a run is active, **Full window** once nothing
/// can be stopped (finished / error / core lost). The resident Pet Feeder never takes over the bar;
/// while it runs alongside the foreground tool a paw badge says so, and Stop only stops the
/// foreground run.
///
/// Design: `.scratch/ui-audit/ui-demo-mini.html` (variant A, approved 2026-11-01). The bar renders
/// core facts only; it never invents a phase, a countdown or a schedule.
library;

import 'dart:async';

import 'package:flutter/foundation.dart' show kDebugMode;
import 'package:flutter/material.dart';

import '../app/app.dart';
import '../protocol/protocol.dart' hide Size;
import 'components/status_chip.dart';
import 'theme.dart';
import 'tool_labels.dart';

/// The bar's logical size; the runner's mini window is exactly this content area.
const double miniBarWidth = 360;
const double miniBarHeight = 96;

class MiniRunningCard extends StatefulWidget {
  const MiniRunningCard({super.key, required this.model});

  final ShellModel model;

  @override
  State<MiniRunningCard> createState() => _MiniRunningCardState();
}

class _MiniRunningCardState extends State<MiniRunningCard> {
  Timer? _ticker;
  int _elapsed = 0;

  /// The published countdown the ticker's [elapsed] offset belongs to; a new core fact restarts it.
  double? _lastCountdown;
  ToolId? _lastTool;

  WindowRect? _dragOrigin;
  Offset _dragDelta = Offset.zero;
  String? _loggedState;

  @override
  void initState() {
    super.initState();
    widget.model.tools.addListener(_onToolModelChanged);
    widget.model.config.addListener(_onConfigModelChanged);
    _ticker = Timer.periodic(const Duration(seconds: 1), _onTick);
    WidgetsBinding.instance.addPostFrameCallback((_) => _logMiniState());
  }

  /// SHELL-F5: mini/dock content renders tool/config facts directly, so it listens to those two
  /// sub-models instead of the whole shell rebuilding on every event.
  void _onToolModelChanged() {
    if (mounted) {
      _logMiniState();
      setState(() {});
    }
  }

  /// A temporary diagnostic for the collapsed bar: it records exactly the facts the bar received
  /// (phase/message/detail), so a blank Tuner line can be told apart from a rendering problem.
  void _logMiniState() {
    if (!kDebugMode || !widget.model.miniMode) {
      return;
    }
    final tool = widget.model.miniTool;
    if (tool == null) {
      return;
    }
    final snapshot = widget.model.tools.snapshot(tool);
    final signature = [
      tool.wire,
      snapshot?.phase.name ?? 'none',
      snapshot?.message ?? '',
      for (final line in snapshot?.detail ?? const <ToolDetail>[])
        '${line.label ?? ''}=${line.value}',
    ].join('|');
    if (signature == _loggedState) {
      return;
    }
    _loggedState = signature;
    final detail =
        snapshot?.detail
            .map((line) => '${line.label ?? ''}=${line.value}')
            .join('; ') ??
        '';
    widget.model.logs.addShellNote(
      'mini bar state: ${tool.wire} '
      'phase=${snapshot?.phase.name ?? 'none'} '
      'message="${snapshot?.message ?? ''}" '
      'detail=[$detail]',
    );
  }

  void _onConfigModelChanged() {
    if (mounted) {
      setState(() {});
    }
  }

  @override
  void didUpdateWidget(MiniRunningCard oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.model, widget.model)) {
      oldWidget.model.tools.removeListener(_onToolModelChanged);
      oldWidget.model.config.removeListener(_onConfigModelChanged);
      widget.model.tools.addListener(_onToolModelChanged);
      widget.model.config.addListener(_onConfigModelChanged);
    }
    final tool = widget.model.miniTool;
    final countdown = tool == null
        ? null
        : widget.model.tools.snapshot(tool)?.countdownSeconds;
    if (tool != _lastTool || countdown != _lastCountdown) {
      _lastTool = tool;
      _lastCountdown = countdown;
      _elapsed = 0;
    }
  }

  void _onTick(Timer timer) {
    final tool = widget.model.miniTool;
    if (tool == null) {
      return;
    }
    final snapshot = widget.model.tools.snapshot(tool);
    if (snapshot?.phase == ToolPhase.idle ||
        snapshot?.countdownSeconds == null) {
      return;
    }
    if (!mounted) {
      return;
    }
    setState(() => _elapsed++);
  }

  @override
  void dispose() {
    widget.model.tools.removeListener(_onToolModelChanged);
    widget.model.config.removeListener(_onConfigModelChanged);
    _ticker?.cancel();
    super.dispose();
  }

  void _onDragStart(DragStartDetails details) {
    _dragOrigin = widget.model.miniRect;
    _dragDelta = Offset.zero;
  }

  void _onDragUpdate(DragUpdateDetails details) {
    final origin = _dragOrigin;
    if (origin == null) {
      return;
    }
    _dragDelta += details.delta;
    unawaited(
      widget.model.moveMiniWindow(
        WindowRect(
          left: origin.left + _dragDelta.dx.round(),
          top: origin.top + _dragDelta.dy.round(),
          width: origin.width,
          height: origin.height,
        ),
      ),
    );
  }

  void _onDragEnd([Object? _]) {
    if (_dragOrigin == null) {
      return;
    }
    _dragOrigin = null;
    unawaited(widget.model.settleMiniWindow());
  }

  /// One compact line from the live `state.detail`, so the bar carries the facts v2's running card
  /// showed instead of only the action message. Tuner gets the v2 density (`DG · 9 left · attempt
  /// 4`); other tools get up to three labelled facts. Null when the snapshot has no detail.
  String? _detailSummary(ToolId tool, ToolSnapshot? snapshot) {
    final detail = snapshot?.detail ?? const [];
    if (detail.isEmpty) {
      return null;
    }
    String? value(String label) {
      for (final line in detail) {
        if (line.label == label) {
          return line.value;
        }
      }
      return null;
    }

    if (tool == ToolId.tuner) {
      final parts = <String>[
        if (value('Grade') case final grade? when grade.isNotEmpty) grade,
        if (value('Remaining') case final remaining? when remaining.isNotEmpty)
          '$remaining left',
        if (value('Attempt') case final attempt? when attempt.isNotEmpty)
          'attempt $attempt',
      ];
      if (parts.isNotEmpty) {
        return parts.join(' · ');
      }
    }
    final parts = <String>[];
    for (final line in detail) {
      final text = line.label == null || line.label!.isEmpty
          ? line.value
          : '${line.label} ${line.value}';
      if (text.trim().isEmpty) {
        continue;
      }
      parts.add(text);
      if (parts.length == 3) {
        break;
      }
    }
    return parts.isEmpty ? null : parts.join(' · ');
  }

  @override
  Widget build(BuildContext context) {
    // The bar is also usable on its own (tests, and a future always-on-top overlay), so it listens to
    // the model itself instead of relying on the page's root builder to rebuild it.
    return ListenableBuilder(
      listenable: widget.model,
      builder: (context, _) => _buildBar(context),
    );
  }

  Widget _buildBar(BuildContext context) {
    final theme = Theme.of(context);
    final tool = widget.model.miniTool;
    if (tool == null) {
      return const SizedBox.shrink();
    }
    final snapshot = widget.model.tools.snapshot(tool);
    final finished = widget.model.tools.finished(tool);
    final lost = !widget.model.isReady;
    final phase = lost ? ToolPhase.idle : (snapshot?.phase ?? ToolPhase.idle);
    final active = !lost && phase != ToolPhase.idle;
    final errorState = lost || finished?.reason == ToolStopKind.error;
    final cancelled = !lost && finished?.reason == ToolStopKind.cancelled;
    final busy =
        active &&
        (phase == ToolPhase.stopping || widget.model.tools.isPending(tool));
    final petRunning =
        (widget.model.tools.snapshot(ToolId.pet)?.phase ?? ToolPhase.idle) !=
        ToolPhase.idle;

    final (chipKind, chipLabel) = lost
        ? (StatusKind.error, 'core lost')
        : active
        ? switch (phase) {
            ToolPhase.running => (StatusKind.success, 'running'),
            ToolPhase.starting => (StatusKind.info, 'starting'),
            ToolPhase.stopping => (StatusKind.warning, 'stopping'),
            ToolPhase.idle => (StatusKind.muted, 'idle'),
          }
        : errorState
        ? (StatusKind.error, 'error')
        : cancelled
        ? (StatusKind.muted, 'stopped')
        : (StatusKind.success, 'finished');

    final message = lost
        ? 'The core stopped — reconnect in the full window'
        : active
        ? (snapshot?.message.trim().isNotEmpty ?? false
              ? snapshot!.message.trim()
              : 'Running')
        : (finished?.message ?? 'Run ended');
    // The live read facts outrank the generic action line while a run is active: the bar is the
    // only surface a collapsed player can see. Finished bar still shows the outcome.
    final detailSummary = active ? _detailSummary(tool, snapshot) : null;
    final barMessage = detailSummary ?? message;
    // The "next moment" belongs to a live run. The bar is 360 px wide, so drop the full window's
    // "next action " prefix ("in 11 s" instead of "next action in 11 s"); a stopped bar shows the
    // outcome sentence only.
    final whenFull = active
        ? countdownLabel(snapshot, elapsedSeconds: _elapsed)
        : null;
    final when = whenFull != null && whenFull.startsWith('next action ')
        ? whenFull.substring('next action '.length)
        : whenFull;

    return Semantics(
      label: 'Mini running window: ${toolDisplayName(tool)}, $chipLabel',
      child: GestureDetector(
        behavior: HitTestBehavior.translucent,
        onPanStart: _onDragStart,
        onPanUpdate: _onDragUpdate,
        onPanEnd: _onDragEnd,
        onPanCancel: _onDragEnd,
        child: ClipRRect(
          borderRadius: BorderRadius.circular(ShellRadius.l),
          child: Material(
            key: const ValueKey('mini-bar'),
            color: theme.colorScheme.surfaceContainerHigh,
            child: Container(
              width: miniBarWidth,
              height: miniBarHeight,
              padding: const EdgeInsets.all(ShellSpace.md),
              decoration: BoxDecoration(
                border: Border.all(
                  color: theme.colorScheme.outlineVariant.withValues(
                    alpha: 0.5,
                  ),
                ),
              ),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  SizedBox(
                    height: 32,
                    child: Row(
                      children: [
                        Tooltip(
                          message: 'Drag to move',
                          child: Icon(
                            Icons.drag_indicator,
                            size: 16,
                            color: theme.colorScheme.onSurfaceVariant
                                .withValues(alpha: 0.6),
                          ),
                        ),
                        const SizedBox(width: ShellSpace.xs),
                        Container(
                          width: 28,
                          height: 28,
                          decoration: BoxDecoration(
                            color: theme.colorScheme.surfaceContainerHighest,
                            borderRadius: BorderRadius.circular(ShellRadius.s),
                          ),
                          child: Icon(
                            toolIcon(tool),
                            size: ShellSize.iconMd,
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                        const SizedBox(width: ShellSpace.sm),
                        Expanded(
                          child: Text(
                            toolDisplayName(tool),
                            key: const ValueKey('mini-tool-name'),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: theme.textTheme.titleSmall?.copyWith(
                              fontSize: 13.5,
                            ),
                          ),
                        ),
                        const SizedBox(width: ShellSpace.xs),
                        StatusChip(
                          key: const ValueKey('mini-phase'),
                          kind: chipKind,
                          label: chipLabel,
                        ),
                        if (petRunning) ...[
                          const SizedBox(width: ShellSpace.xs),
                          Tooltip(
                            message: petRunning
                                ? 'Pet Feeder is running in the background'
                                : '',
                            child: Icon(
                              Icons.pets,
                              key: const ValueKey('mini-resident-pet'),
                              size: 14,
                              color: theme.colorScheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                        const SizedBox(width: ShellSpace.xs),
                        IconButton(
                          key: const ValueKey('mini-expand'),
                          tooltip: 'Full window',
                          visualDensity: VisualDensity.compact,
                          style: IconButton.styleFrom(
                            minimumSize: const Size(28, 28),
                            padding: EdgeInsets.zero,
                          ),
                          onPressed: () =>
                              unawaited(widget.model.showFullWindow()),
                          icon: Icon(
                            Icons.open_in_full,
                            size: 15,
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(height: ShellSpace.xs),
                  SizedBox(
                    height: 32,
                    child: Row(
                      children: [
                        Expanded(
                          child: Tooltip(
                            message: message,
                            child: Text(
                              barMessage,
                              key: const ValueKey('mini-message'),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.bodySmall?.copyWith(
                                fontSize: 12.5,
                                color: errorState
                                    ? theme.colorScheme.error
                                    : theme.colorScheme.onSurfaceVariant,
                              ),
                            ),
                          ),
                        ),
                        if (when != null && when.isNotEmpty) ...[
                          const SizedBox(width: ShellSpace.xs),
                          Text(
                            when,
                            key: const ValueKey('mini-when'),
                            style: ShellText.mono.copyWith(
                              fontSize: 11.5,
                              color: theme.colorScheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                        const SizedBox(width: ShellSpace.sm),
                        if (active)
                          _StopAction(
                            key: const ValueKey('mini-stop'),
                            busy: busy,
                            onPressed: () =>
                                unawaited(widget.model.stopTool(tool)),
                          )
                        else ...[
                          IconButton(
                            key: const ValueKey('mini-dock'),
                            tooltip: 'Quick dock',
                            visualDensity: VisualDensity.compact,
                            style: IconButton.styleFrom(
                              minimumSize: const Size(28, 32),
                              padding: EdgeInsets.zero,
                            ),
                            onPressed: () =>
                                unawaited(widget.model.openDockFromMini()),
                            icon: Icon(
                              Icons.dashboard_customize_outlined,
                              size: 15,
                              color: theme.colorScheme.onSurfaceVariant,
                            ),
                          ),
                          const SizedBox(width: ShellSpace.xs),
                          FilledButton.icon(
                            key: const ValueKey('mini-full'),
                            onPressed: () =>
                                unawaited(widget.model.showFullWindow()),
                            style: FilledButton.styleFrom(
                              backgroundColor:
                                  theme.colorScheme.primaryContainer,
                              foregroundColor:
                                  theme.colorScheme.onPrimaryContainer,
                              minimumSize: const Size(0, 32),
                              padding: const EdgeInsets.symmetric(
                                horizontal: ShellSpace.md,
                              ),
                              textStyle: theme.textTheme.labelLarge?.copyWith(
                                fontSize: 12,
                              ),
                            ),
                            icon: const Icon(Icons.open_in_full, size: 14),
                            label: const Text('Full window'),
                          ),
                        ],
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The bar's one primary action while a run can be stopped. Disabled with a spinner while the core
/// reports `stopping` (or a Stop command is still in flight), so the bar never lies about the phase.
class _StopAction extends StatelessWidget {
  const _StopAction({super.key, required this.busy, required this.onPressed});

  final bool busy;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return FilledButton(
      onPressed: busy ? null : onPressed,
      style: FilledButton.styleFrom(
        backgroundColor: theme.colorScheme.errorContainer,
        foregroundColor: theme.colorScheme.onErrorContainer,
        disabledBackgroundColor: theme.colorScheme.surfaceContainerHighest,
        disabledForegroundColor: theme.colorScheme.onSurfaceVariant,
        minimumSize: Size(busy ? 112 : 88, 32),
        padding: const EdgeInsets.symmetric(horizontal: ShellSpace.md),
        textStyle: theme.textTheme.labelLarge?.copyWith(fontSize: 12),
      ),
      child: busy
          ? Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                SizedBox(
                  width: 12,
                  height: 12,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
                const SizedBox(width: ShellSpace.xs),
                const Text('Stopping…'),
              ],
            )
          : Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Icon(Icons.stop, size: 14),
                const SizedBox(width: ShellSpace.xs),
                Text('Stop'),
              ],
            ),
    );
  }
}

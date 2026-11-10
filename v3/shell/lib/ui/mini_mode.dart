/// The **mini running bar**: the shell window while a foreground tool runs.
///
/// A foreground run collapses the whole shell to a 460 × 168 borderless, always-on-top bar beside the
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
const double miniBarWidth = 460;
const double miniBarHeight = 168;

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

  /// The last live OCR read, kept so the finished/result bar can still show what the run found after
  /// the core has moved to its terminal `state` (which no longer carries the live detail).
  List<_MiniFact> _lastReadFacts = const <_MiniFact>[];
  ToolId? _lastReadTool;

  @override
  void initState() {
    super.initState();
    widget.model.tools.addListener(_onToolModelChanged);
    widget.model.config.addListener(_onConfigModelChanged);
    _ticker = Timer.periodic(const Duration(seconds: 1), _onTick);
    _captureReadFacts();
    WidgetsBinding.instance.addPostFrameCallback((_) => _logMiniState());
  }

  /// SHELL-F5: mini/dock content renders tool/config facts directly, so it listens to those two
  /// sub-models instead of the whole shell rebuilding on every event.
  void _onToolModelChanged() {
    _captureReadFacts();
    if (mounted) {
      _logMiniState();
      setState(() {});
    }
  }

  /// Keep the latest live OCR facts for the terminal (finished/error) bar. A new run starting clears
  /// the previous run's read before its first capture, so the result never mixes two attempts.
  void _captureReadFacts() {
    final tool = widget.model.miniTool;
    if (tool == null) {
      _lastReadFacts = const <_MiniFact>[];
      _lastReadTool = null;
      return;
    }
    if (tool != _lastReadTool) {
      _lastReadFacts = const <_MiniFact>[];
      _lastReadTool = tool;
    }
    final snapshot = widget.model.tools.snapshot(tool);
    if (snapshot == null) {
      return;
    }
    if (snapshot.phase == ToolPhase.starting && snapshot.detail.isEmpty) {
      _lastReadFacts = const <_MiniFact>[];
      return;
    }
    if (snapshot.phase != ToolPhase.idle && snapshot.detail.isNotEmpty) {
      final facts = _detailFacts(tool, snapshot);
      if (facts.any((fact) => fact.inline || fact.wide)) {
        _lastReadFacts = facts;
      }
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

  /// The live `state.detail` as labelled facts. The old bar joined these into one ` · ` sentence;
  /// in the larger bar they become scannable chips, with long Filter text kept as its own line.
  List<_MiniFact> _detailFacts(ToolId tool, ToolSnapshot? snapshot) {
    final facts = <_MiniFact>[];
    for (final line in snapshot?.detail ?? const <ToolDetail>[]) {
      final rawLabel = (line.label ?? '').trim();
      final value = line.value.trim();
      if (value.isEmpty) {
        continue;
      }
      if (rawLabel == 'Filter') {
        facts.add(_MiniFact(rawLabel, value, wide: true));
        continue;
      }
      if (rawLabel.isNotEmpty) {
        final label = switch (rawLabel) {
          'Grade' => 'Grade',
          'Remaining' => 'Left',
          'Attempt' => 'Try',
          _ => rawLabel,
        };
        facts.add(_MiniFact(label, value));
        continue;
      }
      // OCR attribute lines arrive as "name=value"; split them so the chip has a label.
      final equals = value.indexOf('=');
      if (equals > 0 && equals < value.length - 1) {
        facts.add(
          _MiniFact(
            value.substring(0, equals),
            value.substring(equals + 1),
            inline: true,
          ),
        );
      } else {
        facts.add(_MiniFact('Info', value, inline: true));
      }
    }
    return facts;
  }

  /// OCR attributes that satisfy an enabled Tuner filter rule. The OCR detail carries no
  /// per-attribute matched flag, so the bar checks the rule name and the published min/max bounds
  /// locally; the run's overall verdict still comes from the core's Filter line.
  Set<String> _matchedAttributes(List<_MiniFact> facts) {
    final tuner = asJsonMap(widget.model.config.config['tuner']);
    final filter = asJsonMap(tuner['filter']);
    if (asBool(filter['enabled']) != true) {
      return const <String>{};
    }
    final rules = <Map<String, dynamic>>[
      for (final key in const ['rules', 'override_rules'])
        for (final rule in asJsonList(filter[key])) asJsonMap(rule),
    ];
    final matched = <String>{};
    for (final fact in facts) {
      final value = int.tryParse(fact.value.trim());
      for (final rule in rules) {
        if (asString(rule['name'])?.trim() != fact.label) {
          continue;
        }
        final min = asNumber(rule['min']);
        final max = asNumber(rule['max']);
        final inBounds =
            value == null ||
            ((min == null || value >= min) && (max == null || value <= max));
        if (inBounds) {
          matched.add(fact.label);
          break;
        }
      }
    }
    return matched;
  }

  /// The full OCR result set for the mini bar's hover tooltip. The visible line is one ellipsized
  /// row; v2's card showed every attribute, so the tooltip must not hide the rest.
  String? _detailTooltip(ToolSnapshot? snapshot) {
    final detail = snapshot?.detail ?? const <ToolDetail>[];
    if (detail.isEmpty) {
      return null;
    }
    return detail
        .map(
          (line) => line.label == null || line.label!.isEmpty
              ? line.value
              : '${line.label}: ${line.value}',
        )
        .where((line) => line.trim().isNotEmpty)
        .join('\n');
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
    // The core's action/outcome sentence stays the primary line; the live read facts are
    // structured chips below it instead of being glued into one ` · ` sentence.
    final facts = active
        ? _detailFacts(tool, snapshot)
        : (_lastReadTool == tool ? _lastReadFacts : const <_MiniFact>[]);
    final compactFacts = active
        ? facts.where((fact) => !fact.wide && !fact.inline).take(3).toList()
        : const <_MiniFact>[];
    final inlineFacts = facts.where((fact) => fact.inline).toList();
    final wideFacts = active
        ? facts.where((fact) => fact.wide).toList()
        : const <_MiniFact>[];
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
                crossAxisAlignment: CrossAxisAlignment.stretch,
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
                              fontSize: 14,
                              fontWeight: FontWeight.w600,
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
                  const SizedBox(height: ShellSpace.xxs),
                  Tooltip(
                    message: _detailTooltip(snapshot) ?? message,
                    child: Align(
                      alignment: Alignment.centerLeft,
                      child: Text(
                        message,
                        key: const ValueKey('mini-message'),
                        maxLines: facts.isEmpty ? 2 : 1,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.bodyMedium?.copyWith(
                          fontSize: 13.2,
                          height: 1.25,
                          fontWeight: FontWeight.w600,
                          color: errorState
                              ? theme.colorScheme.error
                              : theme.colorScheme.onSurface,
                        ),
                      ),
                    ),
                  ),
                  if (inlineFacts.isNotEmpty) ...[
                    const SizedBox(height: ShellSpace.xxs),
                    _MiniAttributeList(
                      key: const ValueKey('mini-attributes'),
                      facts: inlineFacts,
                      matched: _matchedAttributes(inlineFacts),
                    ),
                  ],
                  for (var index = 0; index < wideFacts.length; index++) ...[
                    const SizedBox(height: ShellSpace.xxs),
                    _MiniFactLine(
                      key: ValueKey('mini-wide-fact-$index'),
                      label: wideFacts[index].label,
                      value: wideFacts[index].value,
                      tooltip: null,
                    ),
                  ],
                  const Spacer(),
                  SizedBox(
                    height: 32,
                    child: Row(
                      children: [
                        if (compactFacts.isNotEmpty)
                          Expanded(
                            child: Wrap(
                              spacing: ShellSpace.xs,
                              runSpacing: ShellSpace.xxs,
                              children: [
                                for (
                                  var index = 0;
                                  index < compactFacts.length;
                                  index++
                                )
                                  _MiniFactChip(
                                    key: ValueKey('mini-fact-$index'),
                                    label: compactFacts[index].label,
                                    value: compactFacts[index].value,
                                  ),
                              ],
                            ),
                          )
                        else
                          const Spacer(),
                        if (when != null && when.isNotEmpty)
                          Tooltip(
                            message: 'Next core action',
                            child: Row(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                Icon(
                                  Icons.schedule,
                                  size: 12,
                                  color: theme.colorScheme.onSurfaceVariant,
                                ),
                                const SizedBox(width: 4),
                                Text(
                                  when,
                                  key: const ValueKey('mini-when'),
                                  style: ShellText.mono.copyWith(
                                    fontSize: 11.5,
                                    color: theme.colorScheme.onSurfaceVariant,
                                  ),
                                ),
                              ],
                            ),
                          ),
                        if (compactFacts.isNotEmpty || when != null)
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

/// A compact labelled fact in the mini bar. [wide] facts use the full row instead of a chip.
class _MiniFact {
  const _MiniFact(
    this.label,
    this.value, {
    this.wide = false,
    this.inline = false,
  });

  final String label;
  final String value;
  final bool wide;

  /// OCR attribute pairs render as one inline text run, not a bordered card/chip.
  final bool inline;
}

class _MiniFactChip extends StatelessWidget {
  const _MiniFactChip({super.key, required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final muted = theme.colorScheme.onSurfaceVariant;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainerHighest.withValues(
          alpha: 0.55,
        ),
        border: Border.all(
          color: theme.colorScheme.outlineVariant.withValues(alpha: 0.55),
        ),
        borderRadius: BorderRadius.circular(ShellRadius.s),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            label.toUpperCase(),
            style: ShellText.mono.copyWith(
              fontSize: 8.5,
              letterSpacing: 0.04,
              color: muted,
            ),
          ),
          const SizedBox(width: 4),
          Flexible(
            child: Text(
              value,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: ShellText.mono.copyWith(
                fontSize: 11.5,
                color: theme.colorScheme.onSurface,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// OCR attributes follow v2's `FormatStatus` order and stay one line each (`魔法力=22` on its own
/// row), not cards. An attribute named by the enabled filter is tinted with the primary colour; the
/// rest stay neutral.
class _MiniAttributeList extends StatelessWidget {
  const _MiniAttributeList({
    super.key,
    required this.facts,
    required this.matched,
  });

  final List<_MiniFact> facts;
  final Set<String> matched;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        for (var index = 0; index < facts.length; index++)
          Text(
            '${facts[index].label}=${facts[index].value}',
            key: ValueKey('mini-attribute-$index'),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: ShellText.mono.copyWith(
              fontSize: 10.5,
              height: 1.1,
              fontWeight: matched.contains(facts[index].label)
                  ? FontWeight.w600
                  : FontWeight.w400,
              color: matched.contains(facts[index].label)
                  ? theme.colorScheme.primary
                  : theme.colorScheme.onSurface,
            ),
          ),
      ],
    );
  }
}

class _MiniFactLine extends StatelessWidget {
  const _MiniFactLine({
    super.key,
    required this.label,
    required this.value,
    required this.tooltip,
  });

  final String label;
  final String value;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final muted = theme.colorScheme.onSurfaceVariant;
    final line = Row(
      children: [
        Text(
          label.toUpperCase(),
          style: ShellText.mono.copyWith(
            fontSize: 8.5,
            letterSpacing: 0.04,
            color: muted,
          ),
        ),
        const SizedBox(width: 6),
        Expanded(
          child: Text(
            value,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: theme.textTheme.bodySmall?.copyWith(
              fontSize: 11.5,
              color: theme.colorScheme.onSurface,
            ),
          ),
        ),
      ],
    );
    return tooltip == null ? line : Tooltip(message: tooltip!, child: line);
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

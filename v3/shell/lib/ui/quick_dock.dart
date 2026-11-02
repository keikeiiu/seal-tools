/// The **quick dock**: the compact launcher the shell shows between runs.
///
/// It is the idle state of the same borderless window that becomes the 360 × 96 run bar. The player
/// can put it on either screen edge (tall rail), in either bottom corner (bay) or at the top centre
/// (island); the content is the same ranked launcher in every form factor:
///
///   primary    five start tiles — body click starts with the current/last-used options
///   secondary  live phase and a chevron that opens ONE secondary menu per tool
///   tertiary   the three setup-only tools, the full-window escape, style/drag
///
/// The menu is drawn inside the compact window (not a flyout past its edge) so the window can stay
/// 300–380 px wide; opening another menu replaces it, Back/outside closes it. There is no optimistic
/// phase: a Start only shows an in-flight tile until the core publishes `state` / a refusal.
///
/// Design study: `.scratch/ui-audit/ui-demo-quickdock.html`. Placement persistence is the documented
/// follow-up (config/machine field); this build remembers it for the session.
library;

import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';

import '../app/app.dart';
import '../app/dock_placement.dart';
import '../protocol/protocol.dart' hide Size;
import 'components/status_chip.dart';
import 'theme.dart';
import 'tool_labels.dart';

const Set<String> _gemOptionKeys = {
  'start_grade',
  'empty_mode',
  'empty_streak',
  'colored_gap_min',
  'save_empty_captures',
  'move_mode',
};

const Set<String> _tunerOptionKeys = {
  'target_grade',
  'max_retries',
  'ocr_retries',
  'save_captures',
  'spring_mode',
  'mouse_guard',
  'guard_px',
  'recenter_max',
  'timing',
  'filter',
};

class QuickDock extends StatefulWidget {
  const QuickDock({super.key, required this.model});

  final ShellModel model;

  @override
  State<QuickDock> createState() => _QuickDockState();
}

class _QuickDockState extends State<QuickDock> {
  /// The open secondary menu, if any. One at a time, exactly like the design.
  ToolId? _menu;

  /// True while the quick-action list is being edited (remove/reorder/add, pins included).
  bool _editing = false;

  bool _savingPreset = false;
  String? _presetError;
  String? _pinError;
  late final TextEditingController _pinName;

  /// Auto-collapse the launcher after this much idle time (design study: 8 s). Any pointer
  /// interaction, an open menu or the collapsed state cancels it.
  static const _autoCollapseAfter = Duration(seconds: 8);
  Timer? _autoCollapse;

  // Hover state: hovering pauses the auto-collapse, and hovering the collapsed tab expands it.
  bool _hovering = false;
  bool _hoverExpanded = false;

  @override
  void initState() {
    super.initState();
    _pinName = TextEditingController();
    widget.model.addListener(_onDockModelChanged);
    _armAutoCollapse();
  }

  @override
  void dispose() {
    widget.model.removeListener(_onDockModelChanged);
    _autoCollapse?.cancel();
    _pinName.dispose();
    super.dispose();
  }

  void _onDockModelChanged() {
    if (!mounted) {
      return;
    }
    if (widget.model.dockCollapsed ||
        _menu != null ||
        _editing ||
        _hovering ||
        widget.model.dockBehavior != DockBehavior.autoCollapse) {
      _cancelAutoCollapse();
    } else {
      _armAutoCollapse();
    }
  }

  void _onHoverEnter(PointerEnterEvent event) {
    _hovering = true;
    _cancelAutoCollapse();
    if (model.dockCollapsed) {
      _hoverExpanded = true;
      unawaited(model.setDockCollapsed(false, persist: false));
    }
  }

  void _onHoverExit(PointerExitEvent event) {
    _hovering = false;
    if (_hoverExpanded) {
      _hoverExpanded = false;
      if (model.dockBehavior == DockBehavior.alwaysOpen) {
        unawaited(model.setDockCollapsed(true, persist: false));
        return;
      }
    }
    _resetAutoCollapse();
  }

  void _cancelAutoCollapse() {
    _autoCollapse?.cancel();
    _autoCollapse = null;
  }

  void _armAutoCollapse() {
    if (_autoCollapse != null ||
        !widget.model.dockMode ||
        widget.model.dockCollapsed ||
        widget.model.dockBehavior != DockBehavior.autoCollapse ||
        _hovering ||
        _menu != null ||
        _editing) {
      return;
    }
    _autoCollapse = Timer(_autoCollapseAfter, () {
      _autoCollapse = null;
      if (!mounted ||
          _hovering ||
          _menu != null ||
          _editing ||
          widget.model.dockCollapsed) {
        return;
      }
      unawaited(widget.model.setDockCollapsed(true, persist: false));
    });
  }

  void _resetAutoCollapse() {
    _cancelAutoCollapse();
    _armAutoCollapse();
  }

  WindowRect? _dragOrigin;
  Offset _dragDelta = Offset.zero;

  ShellModel get model => widget.model;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: model,
      builder: (context, _) {
        if (!model.dockMode) {
          return const SizedBox.shrink();
        }
        final theme = Theme.of(context);
        return Semantics(
          label: 'Quick dock',
          child: MouseRegion(
            onEnter: _onHoverEnter,
            onExit: _onHoverExit,
            child: ClipRRect(
              borderRadius: BorderRadius.circular(ShellRadius.l),
              child: Material(
                key: const ValueKey('quick-dock'),
                color: theme.colorScheme.surfaceContainerHigh,
                child: Listener(
                  onPointerDown: (_) => _resetAutoCollapse(),
                  child: GestureDetector(
                    behavior: HitTestBehavior.translucent,
                    onPanStart: _onDragStart,
                    onPanUpdate: _onDragUpdate,
                    onPanEnd: _onDragEnd,
                    onPanCancel: _onDragEnd,
                    child: Container(
                      decoration: BoxDecoration(
                        border: Border.all(
                          color: theme.colorScheme.outlineVariant.withValues(
                            alpha: 0.5,
                          ),
                        ),
                      ),
                      child: model.dockCollapsed
                          ? _collapsed(theme)
                          : Column(
                              children: [
                                _header(theme),
                                Expanded(
                                  child: SingleChildScrollView(
                                    padding: const EdgeInsets.fromLTRB(
                                      ShellSpace.xs,
                                      0,
                                      ShellSpace.xs,
                                      ShellSpace.sm,
                                    ),
                                    child: _menu == null
                                        ? _launcher(theme)
                                        : _secondaryMenu(theme, _menu!),
                                  ),
                                ),
                                _footer(theme),
                              ],
                            ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
      },
    );
  }

  // ── collapsed resting form ───────────────────────────────────────────────────────────────────

  /// The dock's resting form: a vertical tab on the rails, a status pill for the bays and a slim
  /// island at the top. It keeps three tool icons (and the running dot) glanceable; a click or the
  /// unfold button returns to the full launcher.
  Widget _collapsed(ThemeData theme) {
    final rail = model.dockPlacement.isRail;
    final running = _runningCount();
    final tools = <Widget>[
      for (final tool in const [ToolId.spammer, ToolId.buy, ToolId.sell])
        Padding(
          padding: const EdgeInsets.all(1),
          child: SizedBox(
            width: 26,
            height: 26,
            child: Stack(
              children: [
                Center(
                  child: Icon(
                    toolIcon(tool),
                    size: 15,
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
                if (tool == ToolId.spammer && running > 0)
                  Positioned(
                    right: 1,
                    top: 1,
                    child: Container(
                      width: 6,
                      height: 6,
                      decoration: BoxDecoration(
                        color: theme.colorScheme.primary,
                        shape: BoxShape.circle,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
    ];
    final expand = IconButton(
      key: const ValueKey('dock-expand'),
      tooltip: 'Expand quick dock',
      visualDensity: VisualDensity.compact,
      style: IconButton.styleFrom(
        minimumSize: const Size(26, 26),
        padding: EdgeInsets.zero,
      ),
      onPressed: () => unawaited(model.setDockCollapsed(false)),
      icon: Icon(
        Icons.unfold_more,
        size: 16,
        color: theme.colorScheme.onSurfaceVariant,
      ),
    );
    return InkWell(
      key: const ValueKey('dock-collapsed'),
      onTap: () => unawaited(model.setDockCollapsed(false)),
      child: Center(
        child: rail
            ? Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(
                    Icons.drag_indicator,
                    size: 14,
                    color: theme.colorScheme.onSurfaceVariant.withValues(
                      alpha: 0.6,
                    ),
                  ),
                  ...tools,
                  expand,
                ],
              )
            : Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  expand,
                  const SizedBox(width: ShellSpace.xxs),
                  ...tools,
                  if (running > 0)
                    Padding(
                      padding: const EdgeInsets.only(left: ShellSpace.xs),
                      child: Container(
                        width: 6,
                        height: 6,
                        decoration: BoxDecoration(
                          color: theme.colorScheme.primary,
                          shape: BoxShape.circle,
                        ),
                      ),
                    ),
                ],
              ),
      ),
    );
  }

  // ── chrome ────────────────────────────────────────────────────────────────────────────────────

  void _toggleEditing() {
    setState(() {
      _editing = !_editing;
      _menu = null;
      _presetError = null;
    });
    if (_editing) {
      _cancelAutoCollapse();
    } else {
      _resetAutoCollapse();
    }
  }

  Widget _header(ThemeData theme) {
    final running = _runningCount();
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        ShellSpace.xs,
        ShellSpace.sm,
        ShellSpace.xxs,
        ShellSpace.xs,
      ),
      child: Row(
        children: [
          if (_menu != null)
            IconButton(
              key: const ValueKey('dock-back'),
              tooltip: 'Back',
              visualDensity: VisualDensity.compact,
              style: IconButton.styleFrom(
                minimumSize: const Size(26, 26),
                padding: EdgeInsets.zero,
              ),
              onPressed: () {
                setState(() {
                  _menu = null;
                  _presetError = null;
                });
                _resetAutoCollapse();
              },
              icon: Icon(
                Icons.arrow_back,
                size: 16,
                color: theme.colorScheme.onSurfaceVariant,
              ),
            )
          else
            Text(
              _editing ? 'Edit dock' : 'Quick start',
              style: theme.textTheme.labelSmall?.copyWith(
                letterSpacing: 0.08,
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          const Spacer(),
          if (_menu == null && running > 0)
            Padding(
              padding: const EdgeInsets.only(right: ShellSpace.xs),
              child: StatusChip(
                kind: StatusKind.success,
                label: '$running running',
              ),
            ),
          IconButton(
            key: const ValueKey('dock-edit'),
            tooltip: _editing ? 'Done editing' : 'Edit quick actions',
            visualDensity: VisualDensity.compact,
            style: IconButton.styleFrom(
              minimumSize: const Size(26, 26),
              padding: EdgeInsets.zero,
            ),
            onPressed: _toggleEditing,
            icon: Icon(
              _editing ? Icons.check : Icons.edit_outlined,
              size: 15,
              color: _editing
                  ? theme.colorScheme.primary
                  : theme.colorScheme.onSurfaceVariant,
            ),
          ),
          PopupMenuButton<Object>(
            key: const ValueKey('dock-placement'),
            tooltip: 'Dock placement and behaviour',
            padding: EdgeInsets.zero,
            icon: Icon(
              Icons.dashboard_customize_outlined,
              size: 16,
              color: theme.colorScheme.onSurfaceVariant,
            ),
            onSelected: (choice) {
              if (choice is DockPlacement) {
                unawaited(model.setDockPlacement(choice));
              } else if (choice is DockBehavior) {
                unawaited(model.setDockBehavior(choice));
              }
            },
            itemBuilder: (context) => [
              for (final placement in DockPlacement.values)
                PopupMenuItem<Object>(
                  key: ValueKey('dock-placement-${placement.wire}'),
                  value: placement,
                  child: Row(
                    children: [
                      Icon(
                        _placementIcon(placement),
                        size: 16,
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                      const SizedBox(width: ShellSpace.sm),
                      Text(placement.label),
                      if (placement == model.dockPlacement) ...[
                        const Spacer(),
                        Icon(
                          Icons.check,
                          size: 16,
                          color: theme.colorScheme.primary,
                        ),
                      ],
                    ],
                  ),
                ),
              const PopupMenuDivider(),
              for (final behavior in DockBehavior.values)
                PopupMenuItem<Object>(
                  key: ValueKey('dock-behavior-${behavior.wire}'),
                  value: behavior,
                  child: Row(
                    children: [
                      Icon(
                        behavior == DockBehavior.autoCollapse
                            ? Icons.timer_outlined
                            : Icons.lock_open_outlined,
                        size: 16,
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                      const SizedBox(width: ShellSpace.sm),
                      Text(behavior.label),
                      if (behavior == model.dockBehavior) ...[
                        const Spacer(),
                        Icon(
                          Icons.check,
                          size: 16,
                          color: theme.colorScheme.primary,
                        ),
                      ],
                    ],
                  ),
                ),
            ],
          ),
          IconButton(
            key: const ValueKey('dock-collapse'),
            tooltip: 'Collapse to edge',
            visualDensity: VisualDensity.compact,
            style: IconButton.styleFrom(
              minimumSize: const Size(26, 26),
              padding: EdgeInsets.zero,
            ),
            onPressed: () {
              setState(() => _menu = null);
              unawaited(model.setDockCollapsed(true));
            },
            icon: Icon(
              Icons.unfold_less,
              size: 15,
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          IconButton(
            key: const ValueKey('dock-full'),
            tooltip: 'Full window',
            visualDensity: VisualDensity.compact,
            style: IconButton.styleFrom(
              minimumSize: const Size(26, 26),
              padding: EdgeInsets.zero,
            ),
            onPressed: () => unawaited(model.showFullWindowFromDock()),
            icon: Icon(
              Icons.open_in_full,
              size: 15,
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }

  Widget _footer(ThemeData theme) {
    final menu = _menu;
    final Widget action;
    if (menu == null) {
      action = OutlinedButton.icon(
        key: const ValueKey('dock-open-full'),
        onPressed: () => unawaited(model.showFullWindowFromDock()),
        style: OutlinedButton.styleFrom(
          foregroundColor: theme.colorScheme.onSurfaceVariant,
          side: BorderSide(color: theme.colorScheme.outlineVariant),
          padding: const EdgeInsets.symmetric(horizontal: ShellSpace.sm),
          textStyle: theme.textTheme.labelLarge?.copyWith(fontSize: 12),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(ShellRadius.s),
          ),
        ),
        icon: const Icon(Icons.open_in_full, size: 14),
        label: const Text('Open full window'),
      );
    } else {
      action = FilledButton(
        key: const ValueKey('dock-menu-start'),
        onPressed: _savingPreset || !model.isReady
            ? null
            : () => _activate(menu, fromMenu: true),
        style: FilledButton.styleFrom(
          backgroundColor: theme.colorScheme.primaryContainer,
          foregroundColor: theme.colorScheme.onPrimaryContainer,
          textStyle: theme.textTheme.labelLarge?.copyWith(fontSize: 12),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(ShellRadius.s),
          ),
        ),
        child: _savingPreset
            ? const SizedBox(
                width: 14,
                height: 14,
                child: CircularProgressIndicator(strokeWidth: 2),
              )
            : const Text('Start'),
      );
    }
    return Padding(
      padding: const EdgeInsets.fromLTRB(
        ShellSpace.sm,
        0,
        ShellSpace.sm,
        ShellSpace.sm,
      ),
      child: SizedBox(width: double.infinity, height: 32, child: action),
    );
  }

  // ── launcher ─────────────────────────────────────────────────────────────────────────────────

  Widget _launcher(ThemeData theme) {
    final ready = model.isReady;
    if (_editing) {
      return _editView(theme);
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final id in model.dockTileIds) _tile(theme, id, enabled: ready),
        if (model.dockTileIds.isEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: ShellSpace.sm),
            child: Text(
              'No quick actions — use Edit to add one.',
              key: const ValueKey('dock-empty-tiles'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        Padding(
          padding: const EdgeInsets.fromLTRB(
            ShellSpace.xs,
            ShellSpace.sm,
            ShellSpace.xs,
            ShellSpace.xxs,
          ),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  'LONG-RUNNING',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.labelSmall?.copyWith(
                    fontSize: 9.5,
                    letterSpacing: 0.06,
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ),
              Text(
                'SET UP IN FULL WINDOW',
                style: theme.textTheme.labelSmall?.copyWith(
                  fontSize: 9.5,
                  letterSpacing: 0.06,
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
        _longRow(theme, ToolId.pet),
        if (!ready)
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.sm),
            child: Text(
              'The core is not ready — reconnect from the full window.',
              key: const ValueKey('dock-core-note'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.error,
              ),
            ),
          ),
      ],
    );
  }

  Widget _tile(ThemeData theme, String id, {required bool enabled}) {
    final pin = _pinForId(id);
    final tool = pin == null ? _toolForId(id) : ToolId.sell;
    if (tool == null) {
      return const SizedBox.shrink();
    }
    final snapshot = model.tools.snapshot(tool);
    final phase = snapshot?.phase ?? ToolPhase.idle;
    final running = phase != ToolPhase.idle;
    final busy = model.tools.isPending(tool);
    final hasMenu =
        pin == null &&
        (tool == ToolId.spammer ||
            tool == ToolId.buy ||
            tool == ToolId.sell ||
            tool == ToolId.quest ||
            tool == ToolId.gem ||
            tool == ToolId.tuner);
    final title = pin == null ? toolDisplayName(tool) : 'Sell · ${pin.name}';
    final summary = pin == null
        ? _summary(tool)
        : '${pin.slots.length} bag cells · pinned';
    final phaseLabel = switch (phase) {
      ToolPhase.idle => 'idle',
      ToolPhase.starting => 'starting',
      ToolPhase.running => 'running',
      ToolPhase.stopping => 'stopping',
    };
    final phaseKind = switch (phase) {
      ToolPhase.idle => StatusKind.muted,
      ToolPhase.starting => StatusKind.info,
      ToolPhase.running => StatusKind.success,
      ToolPhase.stopping => StatusKind.warning,
    };
    final canAct = enabled && !busy;
    return Padding(
      key: ValueKey('dock-tile-$id'),
      padding: const EdgeInsets.symmetric(
        horizontal: ShellSpace.xs,
        vertical: ShellSpace.xxs,
      ),
      child: Row(
        children: [
          Container(
            width: 30,
            height: 30,
            decoration: BoxDecoration(
              color: running
                  ? theme.colorScheme.primaryContainer
                  : theme.colorScheme.surfaceContainerHighest,
              borderRadius: BorderRadius.circular(ShellRadius.s),
            ),
            child: Icon(
              toolIcon(tool),
              size: 16,
              color: running
                  ? theme.colorScheme.onPrimaryContainer
                  : theme.colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(width: ShellSpace.sm),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.titleSmall?.copyWith(fontSize: 13),
                ),
                Text(
                  summary,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.labelSmall?.copyWith(
                    fontSize: 10.5,
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: ShellSpace.xs),
          if (running) ...[
            StatusChip(kind: phaseKind, label: phaseLabel),
            const SizedBox(width: ShellSpace.xs),
          ],
          _tileAction(
            theme,
            tool,
            running: running,
            enabled: canAct,
            keyId: pin?.tileId,
            onStart: pin == null ? null : () => _startPin(pin),
          ),
          if (hasMenu)
            IconButton(
              key: ValueKey('dock-menu-$id'),
              tooltip: '${toolDisplayName(tool)} options',
              visualDensity: VisualDensity.compact,
              style: IconButton.styleFrom(
                minimumSize: const Size(28, 30),
                padding: EdgeInsets.zero,
              ),
              onPressed: enabled && !busy
                  ? () {
                      setState(() {
                        _menu = tool;
                        _presetError = null;
                      });
                      _cancelAutoCollapse();
                    }
                  : null,
              icon: const Icon(Icons.more_vert, size: 17),
            ),
        ],
      ),
    );
  }

  ToolId? _toolForId(String id) {
    for (final tool in ToolId.values) {
      if (tool != ToolId.pet && tool.wire == id) {
        return tool;
      }
    }
    return null;
  }

  DockPin? _pinForId(String id) {
    for (final pin in model.dockPins) {
      if (pin.tileId == id) {
        return pin;
      }
    }
    return null;
  }

  void _startPin(DockPin pin) {
    unawaited(model.startTool(ToolId.sell, runArgs: {'slots': pin.slots}));
  }

  Widget _editView(ThemeData theme) {
    final ids = model.dockTileIds;
    final missing = [
      for (final tool in ToolId.values)
        if (tool != ToolId.pet && !ids.contains(tool.wire)) tool,
    ];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          'QUICK ACTIONS',
          style: theme.textTheme.labelSmall?.copyWith(
            fontSize: 9.5,
            letterSpacing: 0.06,
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: ShellSpace.xxs),
        for (var index = 0; index < ids.length; index++)
          _editRow(theme, ids[index], index, ids.length),
        if (ids.isEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: ShellSpace.sm),
            child: Text(
              'No quick actions. Add one below.',
              style: theme.textTheme.bodySmall,
            ),
          ),
        if (missing.isNotEmpty) ...[
          const SizedBox(height: ShellSpace.sm),
          Text(
            'ADD BACK',
            style: theme.textTheme.labelSmall?.copyWith(
              fontSize: 9.5,
              letterSpacing: 0.06,
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: ShellSpace.xxs),
          Wrap(
            spacing: ShellSpace.xs,
            runSpacing: ShellSpace.xxs,
            children: [
              for (final tool in missing)
                OutlinedButton(
                  key: ValueKey('dock-add-${tool.wire}'),
                  onPressed: () => unawaited(model.addDockTool(tool)),
                  style: OutlinedButton.styleFrom(
                    minimumSize: const Size(0, 26),
                    padding: const EdgeInsets.symmetric(
                      horizontal: ShellSpace.sm,
                    ),
                    textStyle: theme.textTheme.labelSmall?.copyWith(
                      fontSize: 11,
                    ),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(ShellRadius.s),
                    ),
                  ),
                  child: Text(toolDisplayName(tool)),
                ),
            ],
          ),
        ],
      ],
    );
  }

  Widget _editRow(ThemeData theme, String id, int index, int count) {
    final pin = _pinForId(id);
    final tool = pin == null ? _toolForId(id) : ToolId.sell;
    if (tool == null) {
      return const SizedBox.shrink();
    }
    final label = pin == null ? toolDisplayName(tool) : 'Sell · ${pin.name}';
    final detail = pin == null ? null : '${pin.slots.length} bag cells';
    return Padding(
      key: ValueKey('dock-edit-row-$id'),
      padding: const EdgeInsets.symmetric(vertical: 1),
      child: Row(
        children: [
          Icon(
            toolIcon(tool),
            size: 15,
            color: theme.colorScheme.onSurfaceVariant,
          ),
          const SizedBox(width: ShellSpace.xs),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(label, style: theme.textTheme.bodyMedium),
                if (detail != null)
                  Text(
                    detail,
                    style: theme.textTheme.labelSmall?.copyWith(
                      fontSize: 9.5,
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
              ],
            ),
          ),
          IconButton(
            key: ValueKey('dock-edit-up-$id'),
            tooltip: 'Move up',
            visualDensity: VisualDensity.compact,
            style: IconButton.styleFrom(
              minimumSize: const Size(24, 24),
              padding: EdgeInsets.zero,
            ),
            onPressed: index == 0
                ? null
                : () => unawaited(model.moveDockTile(id, -1)),
            icon: const Icon(Icons.arrow_upward, size: 13),
          ),
          IconButton(
            key: ValueKey('dock-edit-down-$id'),
            tooltip: 'Move down',
            visualDensity: VisualDensity.compact,
            style: IconButton.styleFrom(
              minimumSize: const Size(24, 24),
              padding: EdgeInsets.zero,
            ),
            onPressed: index == count - 1
                ? null
                : () => unawaited(model.moveDockTile(id, 1)),
            icon: const Icon(Icons.arrow_downward, size: 13),
          ),
          IconButton(
            key: ValueKey('dock-edit-remove-$id'),
            tooltip: 'Remove from dock',
            visualDensity: VisualDensity.compact,
            style: IconButton.styleFrom(
              minimumSize: const Size(24, 24),
              padding: EdgeInsets.zero,
            ),
            onPressed: () => unawaited(model.removeDockTile(id)),
            icon: const Icon(Icons.close, size: 13),
          ),
        ],
      ),
    );
  }

  /// The dedicated Start/Stop control: the tile body itself is NOT a start target, so the two
  /// affordances are unambiguous — this button starts, the separate ⋮ icon opens the options.
  Widget _tileAction(
    ThemeData theme,
    ToolId tool, {
    required bool running,
    required bool enabled,
    String? keyId,
    VoidCallback? onStart,
  }) {
    final background = running
        ? theme.colorScheme.errorContainer
        : theme.colorScheme.primaryContainer;
    final foreground = running
        ? theme.colorScheme.onErrorContainer
        : theme.colorScheme.onPrimaryContainer;
    return SizedBox(
      height: 28,
      child: FilledButton(
        key: ValueKey('dock-start-${keyId ?? tool.wire}'),
        onPressed: !enabled
            ? null
            : () {
                if (running) {
                  unawaited(model.stopTool(tool));
                } else if (onStart != null) {
                  onStart();
                } else {
                  _activate(tool);
                }
              },
        style: FilledButton.styleFrom(
          backgroundColor: background,
          foregroundColor: foreground,
          disabledBackgroundColor: theme.colorScheme.surfaceContainerHighest,
          disabledForegroundColor: theme.colorScheme.onSurfaceVariant,
          minimumSize: const Size(44, 28),
          padding: const EdgeInsets.symmetric(horizontal: ShellSpace.sm),
          textStyle: theme.textTheme.labelSmall?.copyWith(
            fontSize: 11,
            fontWeight: FontWeight.w600,
          ),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(ShellRadius.s),
          ),
        ),
        child: Text(running ? 'Stop' : 'Start'),
      ),
    );
  }

  Widget _longRow(ThemeData theme, ToolId tool) {
    final phase = model.tools.snapshot(tool)?.phase ?? ToolPhase.idle;
    final running = phase != ToolPhase.idle;
    return InkWell(
      key: ValueKey('dock-long-${tool.wire}'),
      borderRadius: BorderRadius.circular(ShellRadius.s),
      onTap: () {
        model.requestSetup(tool);
        unawaited(model.showFullWindowFromDock());
      },
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: ShellSpace.xs,
          vertical: ShellSpace.xxs + ShellSpace.xxs,
        ),
        child: Row(
          children: [
            Icon(
              toolIcon(tool),
              size: 14,
              color: theme.colorScheme.onSurfaceVariant,
            ),
            const SizedBox(width: ShellSpace.sm),
            Expanded(
              child: Text(
                toolDisplayName(tool),
                style: theme.textTheme.bodySmall?.copyWith(
                  fontSize: 12,
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ),
            Text(
              _longState(tool, phase),
              style: theme.textTheme.labelSmall?.copyWith(
                fontSize: 10,
                color: running
                    ? theme.colorScheme.primary
                    : theme.colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(width: ShellSpace.xs),
            Icon(
              Icons.open_in_new,
              size: 13,
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ],
        ),
      ),
    );
  }

  // ── secondary menus ──────────────────────────────────────────────────────────────────────────

  Widget _secondaryMenu(ThemeData theme, ToolId tool) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(
            ShellSpace.xs,
            ShellSpace.xxs,
            ShellSpace.xs,
            ShellSpace.xs,
          ),
          child: Row(
            children: [
              Icon(toolIcon(tool), size: 16, color: theme.colorScheme.primary),
              const SizedBox(width: ShellSpace.xs),
              Expanded(
                child: Text(
                  toolDisplayName(tool),
                  key: const ValueKey('dock-menu-title'),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.titleSmall?.copyWith(fontSize: 13.5),
                ),
              ),
              const SizedBox(width: ShellSpace.xs),
              Text(
                _menuCaption(tool),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.labelSmall?.copyWith(
                  fontSize: 10,
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
        ...switch (tool) {
          ToolId.buy => [_buyMenu(theme)],
          ToolId.sell => [_sellMenu(theme)],
          ToolId.spammer => [_presetMenu(theme, tool)],
          ToolId.quest => [_presetMenu(theme, tool)],
          ToolId.gem => [_gemMenu(theme)],
          ToolId.tuner => [_tunerMenu(theme)],
          _ => [const SizedBox.shrink()],
        },
        if (_presetError != null)
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              _presetError!,
              key: const ValueKey('dock-menu-error'),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.error,
              ),
            ),
          ),
      ],
    );
  }

  Widget _buyMenu(ThemeData theme) {
    final presets = _buyPresets();
    final names = presets.keys.toList();
    final preset = model.dockBuyPreset ?? (names.isEmpty ? null : names.first);
    final count = model.dockBuyCount ?? _configuredBuyCount(preset);
    if (names.isEmpty) {
      return _emptyMenu(
        theme,
        'No buy presets configured. Set one up in Buy/Sell settings.',
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          'Preset',
          style: theme.textTheme.labelSmall?.copyWith(
            fontSize: 9.5,
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: ShellSpace.xs),
        Wrap(
          spacing: ShellSpace.xs,
          runSpacing: ShellSpace.xxs,
          children: [
            for (final name in names)
              ChoiceChip(
                key: ValueKey('dock-buy-preset-$name'),
                label: Text(name),
                selected: name == preset,
                onSelected: (_) => setState(() {
                  model.rememberDockBuyChoice(
                    preset: name,
                    count: _configuredBuyCount(name),
                  );
                }),
                visualDensity: VisualDensity.compact,
                labelStyle: theme.textTheme.labelSmall?.copyWith(
                  fontSize: 10.5,
                ),
              ),
          ],
        ),
        const SizedBox(height: ShellSpace.sm),
        Row(
          children: [
            Text(
              'Count',
              style: theme.textTheme.bodySmall?.copyWith(fontSize: 12),
            ),
            const Spacer(),
            IconButton(
              key: const ValueKey('dock-buy-minus'),
              visualDensity: VisualDensity.compact,
              onPressed: () => setState(
                () => model.rememberDockBuyChoice(
                  preset: preset,
                  count: count > 1 ? count - 1 : 1,
                ),
              ),
              icon: const Icon(Icons.remove, size: 14),
            ),
            Text(
              '$count',
              key: const ValueKey('dock-buy-count'),
              style: ShellText.mono.copyWith(fontSize: 12),
            ),
            IconButton(
              key: const ValueKey('dock-buy-plus'),
              visualDensity: VisualDensity.compact,
              onPressed: () => setState(
                () => model.rememberDockBuyChoice(
                  preset: preset,
                  count: count + 1,
                ),
              ),
              icon: const Icon(Icons.add, size: 14),
            ),
          ],
        ),
        Text(
          'Per-run only; the dock remembers this choice for the session.',
          style: theme.textTheme.labelSmall?.copyWith(
            fontSize: 9.5,
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ],
    );
  }

  Widget _sellMenu(ThemeData theme) {
    final selected = model.sellSlots;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Text(
              '${selected.length} selected',
              key: const ValueKey('dock-sell-count'),
              style: theme.textTheme.bodySmall?.copyWith(fontSize: 12),
            ),
            const Spacer(),
            TextButton(
              key: const ValueKey('dock-sell-all'),
              onPressed: () {
                for (var index = 0; index < 64; index++) {
                  if (!model.sellSlots.contains(index)) {
                    model.toggleSellSlot(index);
                  }
                }
                setState(() {});
              },
              style: TextButton.styleFrom(
                minimumSize: const Size(0, 26),
                padding: const EdgeInsets.symmetric(horizontal: ShellSpace.xs),
              ),
              child: const Text('All', style: TextStyle(fontSize: 11)),
            ),
            TextButton(
              key: const ValueKey('dock-sell-none'),
              onPressed: () {
                for (final index in model.sellSlots.toList()) {
                  model.toggleSellSlot(index);
                }
                setState(() {});
              },
              style: TextButton.styleFrom(
                minimumSize: const Size(0, 26),
                padding: const EdgeInsets.symmetric(horizontal: ShellSpace.xs),
              ),
              child: const Text('None', style: TextStyle(fontSize: 11)),
            ),
          ],
        ),
        const SizedBox(height: ShellSpace.xs),
        for (var row = 0; row < 8; row++)
          Row(
            children: [
              for (var column = 0; column < 8; column++) ...[
                if (column > 0) const SizedBox(width: 2),
                _sellCell(
                  theme,
                  row * 8 + column,
                  selected.contains(row * 8 + column),
                ),
              ],
              const Spacer(),
              _rowButton(theme, row, selected),
            ],
          ),
        const SizedBox(height: ShellSpace.xs),
        Text(
          'Click a cell, or the row button to fill/clear all 8 slots of that bag line.',
          style: theme.textTheme.labelSmall?.copyWith(
            fontSize: 9.5,
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: ShellSpace.sm),
        Text(
          'PINNED QUICK ACTIONS',
          style: theme.textTheme.labelSmall?.copyWith(
            fontSize: 9.5,
            letterSpacing: 0.06,
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: ShellSpace.xxs),
        if (model.dockPins.isNotEmpty)
          Wrap(
            spacing: ShellSpace.xs,
            runSpacing: ShellSpace.xxs,
            children: [
              for (final pin in model.dockPins)
                ActionChip(
                  key: ValueKey('dock-pin-chip-${pin.name}'),
                  label: Text(pin.name),
                  onPressed: () => _applyPin(pin),
                  visualDensity: VisualDensity.compact,
                  labelStyle: theme.textTheme.labelSmall?.copyWith(
                    fontSize: 10.5,
                  ),
                ),
            ],
          ),
        const SizedBox(height: ShellSpace.xs),
        Row(
          children: [
            Expanded(
              child: SizedBox(
                height: 30,
                child: TextField(
                  key: const ValueKey('dock-pin-name'),
                  controller: _pinName,
                  style: theme.textTheme.bodySmall?.copyWith(fontSize: 11.5),
                  decoration: InputDecoration(
                    isDense: true,
                    hintText: 'Name this selection',
                    hintStyle: theme.textTheme.bodySmall?.copyWith(
                      fontSize: 10.5,
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                    contentPadding: const EdgeInsets.symmetric(
                      horizontal: ShellSpace.xs,
                      vertical: ShellSpace.xxs,
                    ),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(ShellRadius.s),
                    ),
                  ),
                  onSubmitted: (_) => unawaited(_pinSelection()),
                ),
              ),
            ),
            const SizedBox(width: ShellSpace.xs),
            SizedBox(
              height: 28,
              child: FilledButton(
                key: const ValueKey('dock-pin'),
                onPressed: selected.isEmpty
                    ? null
                    : () => unawaited(_pinSelection()),
                style: FilledButton.styleFrom(
                  minimumSize: const Size(0, 28),
                  padding: const EdgeInsets.symmetric(
                    horizontal: ShellSpace.sm,
                  ),
                  textStyle: theme.textTheme.labelSmall?.copyWith(fontSize: 11),
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(ShellRadius.s),
                  ),
                ),
                child: const Text('Pin'),
              ),
            ),
          ],
        ),
        if (_pinError != null)
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xxs),
            child: Text(
              _pinError!,
              key: const ValueKey('dock-pin-error'),
              style: theme.textTheme.labelSmall?.copyWith(
                fontSize: 9.5,
                color: theme.colorScheme.error,
              ),
            ),
          ),
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xxs),
          child: Text(
            'A pinned selection becomes a tile you can start in one click.',
            style: theme.textTheme.labelSmall?.copyWith(
              fontSize: 9.5,
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ),
      ],
    );
  }

  Future<void> _pinSelection() async {
    if (model.sellSlots.isEmpty) {
      setState(() => _pinError = 'Select at least one bag cell first.');
      return;
    }
    final ok = await model.addDockPin(_pinName.text, model.sellSlots);
    if (!mounted) {
      return;
    }
    setState(() {
      if (ok) {
        _pinName.clear();
        _pinError = null;
      } else {
        _pinError = 'Enter a new name that is not already pinned.';
      }
    });
  }

  void _applyPin(DockPin pin) {
    setState(() {
      for (final slot in model.sellSlots.toList()) {
        model.toggleSellSlot(slot);
      }
      for (final slot in pin.slots) {
        if (!model.sellSlots.contains(slot)) {
          model.toggleSellSlot(slot);
        }
      }
    });
  }

  /// One bag slot in the compact grid. 24 px keeps the whole 8 × 8 grid plus its per-row shortcut
  /// column inside the 300 px dock.
  Widget _sellCell(ThemeData theme, int slot, bool selected) {
    return InkWell(
      key: ValueKey('dock-sell-cell-$slot'),
      onTap: () {
        model.toggleSellSlot(slot);
        setState(() {});
      },
      child: Container(
        width: 24,
        height: 24,
        decoration: BoxDecoration(
          color: selected
              ? theme.colorScheme.primaryContainer
              : theme.colorScheme.surfaceContainerHighest,
          borderRadius: BorderRadius.circular(4),
          border: Border.all(
            color: selected
                ? theme.colorScheme.primary
                : theme.colorScheme.outlineVariant.withValues(alpha: 0.4),
          ),
        ),
      ),
    );
  }

  /// The full Sell screen's `row N` shortcut: one click fills the whole bag line when any cell is
  /// off, clears it when every cell is on. Same in-memory selection, never config.
  Widget _rowButton(ThemeData theme, int row, List<int> selected) {
    final cells = [for (var column = 0; column < 8; column++) row * 8 + column];
    final allOn = cells.every(selected.contains);
    return Tooltip(
      message: 'Select or clear all 8 slots of bag row ${row + 1}',
      child: TextButton(
        key: ValueKey('dock-sell-row-$row'),
        onPressed: () => _toggleSellRow(row),
        style: TextButton.styleFrom(
          minimumSize: const Size(40, 24),
          padding: const EdgeInsets.symmetric(horizontal: ShellSpace.xxs),
          visualDensity: VisualDensity.compact,
          tapTargetSize: MaterialTapTargetSize.shrinkWrap,
        ),
        child: Text(
          'row ${row + 1}',
          style: theme.textTheme.labelSmall?.copyWith(
            fontSize: 9.5,
            color: allOn
                ? theme.colorScheme.primary
                : theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ),
    );
  }

  void _toggleSellRow(int row) {
    final cells = [for (var column = 0; column < 8; column++) row * 8 + column];
    final selected = model.sellSlots.toSet();
    final allOn = cells.every(selected.contains);
    setState(() {
      for (final cell in cells) {
        if (allOn || !selected.contains(cell)) {
          model.toggleSellSlot(cell);
        }
      }
    });
  }

  Widget _gemMenu(ThemeData theme) {
    final block = asJsonMap(model.config.config['gem']);
    final grades = asStringList(block['grades']);
    final startGrade = asString(block['start_grade']);
    final emptyMode = asString(block['empty_mode']) ?? 'stop';
    if (grades.isEmpty) {
      return _emptyMenu(
        theme,
        'No gem grades configured. Set them up in the full window.',
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _optionLabel(theme, 'START GRADE'),
        Wrap(
          spacing: ShellSpace.xs,
          runSpacing: ShellSpace.xxs,
          children: [
            for (final grade in grades)
              _optionChip(
                theme,
                'dock-gem-start-$grade',
                grade,
                grade == startGrade,
                _savingPreset
                    ? null
                    : () => _savePortableOption('gem', {'start_grade': grade}),
              ),
          ],
        ),
        const SizedBox(height: ShellSpace.sm),
        _optionLabel(theme, 'EMPTY SLOT BEHAVIOUR'),
        Wrap(
          spacing: ShellSpace.xs,
          runSpacing: ShellSpace.xxs,
          children: [
            for (final entry in const [
              ('stop', 'Stop'),
              ('advance_grade', 'Advance grade'),
              ('advance_grade_clear', 'Advance + clear'),
            ])
              _optionChip(
                theme,
                'dock-gem-empty-${entry.$1}',
                entry.$2,
                emptyMode == entry.$1,
                _savingPreset
                    ? null
                    : () =>
                          _savePortableOption('gem', {'empty_mode': entry.$1}),
              ),
          ],
        ),
        const SizedBox(height: ShellSpace.xs),
        _optionNote(
          theme,
          'Both choices save to the config; Start runs with them.',
        ),
      ],
    );
  }

  Widget _tunerMenu(ThemeData theme) {
    final block = asJsonMap(model.config.config['tuner']);
    var grades = asStringList(block['grade_order']);
    if (grades.isEmpty) {
      grades = const ['N', 'G', 'DG', 'XG', 'SG'];
    }
    final active = asString(block['target_grade']);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _optionLabel(theme, 'TARGET GRADE'),
        Wrap(
          spacing: ShellSpace.xs,
          runSpacing: ShellSpace.xxs,
          children: [
            for (final grade in grades)
              _optionChip(
                theme,
                'dock-tuner-grade-$grade',
                grade,
                grade == active,
                _savingPreset
                    ? null
                    : () =>
                          _savePortableOption('tuner', {'target_grade': grade}),
              ),
          ],
        ),
        const SizedBox(height: ShellSpace.sm),
        _optionLabel(theme, 'RUN LIMIT'),
        Row(
          children: [
            Expanded(
              child: Text(
                'Max retries',
                style: theme.textTheme.bodySmall?.copyWith(fontSize: 12),
              ),
            ),
            IconButton(
              key: const ValueKey('dock-tuner-retries-minus'),
              visualDensity: VisualDensity.compact,
              onPressed: _savingPreset
                  ? null
                  : () => _saveTunerRetries(
                      (asNumber(block['max_retries']) ?? 1).toInt() - 1,
                    ),
              icon: const Icon(Icons.remove, size: 14),
            ),
            Text(
              '${(asNumber(block['max_retries']) ?? 1).toInt()}',
              key: const ValueKey('dock-tuner-retries'),
              style: ShellText.mono.copyWith(fontSize: 12),
            ),
            IconButton(
              key: const ValueKey('dock-tuner-retries-plus'),
              visualDensity: VisualDensity.compact,
              onPressed: _savingPreset
                  ? null
                  : () => _saveTunerRetries(
                      (asNumber(block['max_retries']) ?? 1).toInt() + 1,
                    ),
              icon: const Icon(Icons.add, size: 14),
            ),
          ],
        ),
        _optionNote(
          theme,
          'The target and retry limit save to the config; Start uses them.',
        ),
      ],
    );
  }

  Widget _optionLabel(ThemeData theme, String text) => Text(
    text,
    style: theme.textTheme.labelSmall?.copyWith(
      fontSize: 9.5,
      letterSpacing: 0.06,
      color: theme.colorScheme.onSurfaceVariant,
    ),
  );

  Widget _optionNote(ThemeData theme, String text) => Text(
    text,
    style: theme.textTheme.labelSmall?.copyWith(
      fontSize: 9.5,
      color: theme.colorScheme.onSurfaceVariant,
    ),
  );

  Widget _optionChip(
    ThemeData theme,
    String key,
    String label,
    bool selected,
    VoidCallback? onSelected,
  ) => ChoiceChip(
    key: ValueKey(key),
    label: Text(label),
    selected: selected,
    showCheckmark: false,
    materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
    onSelected: onSelected == null ? null : (_) => onSelected(),
    visualDensity: VisualDensity.compact,
    labelStyle: theme.textTheme.labelSmall?.copyWith(fontSize: 10.5),
  );

  Future<void> _saveTunerRetries(int count) async {
    await _savePortableOption('tuner', {'max_retries': count.clamp(1, 99)});
  }

  Future<void> _savePortableOption(
    String block,
    Map<String, dynamic> changes,
  ) async {
    final merged = asJsonMap(model.config.config[block]);
    final keys = block == 'gem' ? _gemOptionKeys : _tunerOptionKeys;
    final values = <String, dynamic>{
      for (final key in keys)
        if (merged[key] != null) key: merged[key],
    };
    values.addAll(changes);
    setState(() {
      _savingPreset = true;
      _presetError = null;
    });
    final outcome = await model.saveConfig({
      'settings': {block: values},
    });
    if (!mounted) {
      return;
    }
    setState(() {
      _savingPreset = false;
      _presetError = outcome.saved ? null : outcome.errorLines.join(' · ');
    });
  }

  Widget _presetMenu(ThemeData theme, ToolId tool) {
    final block = tool == ToolId.spammer
        ? asJsonMap(model.config.config['spammer'])
        : asJsonMap(model.config.config['quest']);
    final names = (asJsonMap(block['presets']).keys.toList()..sort());
    final active = asString(block['active']);
    if (names.isEmpty) {
      return _emptyMenu(
        theme,
        'No ${toolDisplayName(tool)} presets configured. Set one up in the full window.',
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final name in names)
          InkWell(
            key: ValueKey('dock-preset-$name'),
            onTap: _savingPreset ? null : () => _saveActivePreset(tool, name),
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: ShellSpace.xs,
                vertical: ShellSpace.xs,
              ),
              child: Row(
                children: [
                  Icon(
                    name == active
                        ? Icons.radio_button_checked
                        : Icons.radio_button_off,
                    size: 16,
                    color: name == active
                        ? theme.colorScheme.primary
                        : theme.colorScheme.onSurfaceVariant,
                  ),
                  const SizedBox(width: ShellSpace.xs),
                  Expanded(
                    child: Text(
                      name,
                      style: theme.textTheme.bodySmall?.copyWith(fontSize: 12),
                    ),
                  ),
                ],
              ),
            ),
          ),
        const SizedBox(height: ShellSpace.xs),
        Text(
          'Picking one saves it as the active preset — the same rule as the Cards picker.',
          style: theme.textTheme.labelSmall?.copyWith(
            fontSize: 9.5,
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ],
    );
  }

  Widget _emptyMenu(ThemeData theme, String message) => Padding(
    padding: const EdgeInsets.symmetric(vertical: ShellSpace.sm),
    child: Text(
      message,
      style: theme.textTheme.bodySmall?.copyWith(
        fontSize: 11.5,
        color: theme.colorScheme.onSurfaceVariant,
      ),
    ),
  );

  // ── actions ─────────────────────────────────────────────────────────────────────────────────

  void _activate(ToolId tool, {bool fromMenu = false}) {
    if (tool == ToolId.holdspace) {
      final phase = model.tools.snapshot(tool)?.phase ?? ToolPhase.idle;
      if (phase != ToolPhase.idle) {
        unawaited(model.stopTool(tool));
      } else {
        unawaited(model.startTool(tool));
      }
      return;
    }
    JsonMap? runArgs;
    if (tool == ToolId.buy) {
      final presets = _buyPresets();
      final names = presets.keys.toList();
      final preset =
          model.dockBuyPreset ?? (names.isEmpty ? null : names.first);
      if (preset != null) {
        runArgs = {
          'preset': preset,
          'count': model.dockBuyCount ?? _configuredBuyCount(preset),
        };
      }
    } else if (tool == ToolId.sell) {
      final slots = model.sellSlots;
      if (slots.isNotEmpty) {
        runArgs = {'slots': slots};
      }
    }
    if (fromMenu) {
      setState(() {
        _menu = null;
        _presetError = null;
      });
    }
    unawaited(model.startTool(tool, runArgs: runArgs));
  }

  Future<void> _saveActivePreset(ToolId tool, String name) async {
    setState(() {
      _savingPreset = true;
      _presetError = null;
    });
    final outcome = await model.saveConfig({
      tool == ToolId.spammer ? 'spammer' : 'quest': {'active': name},
    });
    if (!mounted) {
      return;
    }
    setState(() {
      _savingPreset = false;
      _presetError = outcome.saved ? null : outcome.errorLines.join(' · ');
    });
  }

  // ── drag ────────────────────────────────────────────────────────────────────────────────────

  void _onDragStart(DragStartDetails details) {
    _dragOrigin = model.dockRect;
    _dragDelta = Offset.zero;
  }

  void _onDragUpdate(DragUpdateDetails details) {
    final origin = _dragOrigin;
    if (origin == null) {
      return;
    }
    _dragDelta += details.delta;
    unawaited(
      model.moveDockWindow(
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
    unawaited(model.settleDockWindow());
  }

  // ── text helpers ────────────────────────────────────────────────────────────────────────────

  Map<String, dynamic> _buyPresets() {
    final block = asJsonMap(model.config.config['buy_sell']);
    return asJsonMap(block['presets']);
  }

  int _configuredBuyCount(String? preset) {
    if (preset == null) {
      return 1;
    }
    final entry = _buyPresets()[preset];
    final count = entry is Map ? entry['count'] : null;
    return count is num ? count.toInt().clamp(1, 999) : 1;
  }

  int _runningCount() =>
      [ToolId.spammer, ToolId.buy, ToolId.sell, ToolId.quest, ToolId.holdspace]
          .where(
            (tool) =>
                (model.tools.snapshot(tool)?.phase ?? ToolPhase.idle) !=
                ToolPhase.idle,
          )
          .length;

  String _summary(ToolId tool) {
    final config = model.config.config;
    return switch (tool) {
      ToolId.spammer =>
        asString(asJsonMap(config['spammer'])['active']) ?? 'No rotation',
      ToolId.buy => _buySummary(),
      ToolId.sell =>
        model.sellSlots.isEmpty
            ? 'No bag cells selected'
            : '${model.sellSlots.length} bag cells',
      ToolId.quest =>
        asString(asJsonMap(config['quest'])['active']) ?? 'No flow',
      ToolId.gem =>
        '${asString(asJsonMap(config['gem'])['start_grade']) ?? 'start grade'} · ${asString(asJsonMap(config['gem'])['empty_mode']) ?? 'empty mode'}',
      ToolId.tuner =>
        '${asString(asJsonMap(config['tuner'])['target_grade']) ?? 'no target'} · grade run',
      ToolId.holdspace =>
        (model.tools.snapshot(tool)?.phase ?? ToolPhase.idle) == ToolPhase.idle
            ? 'Spacebar auto-pickup'
            : 'Engaged — click Stop',
      _ => '',
    };
  }

  String _buySummary() {
    final names = _buyPresets().keys.toList();
    if (names.isEmpty) {
      return 'No buy presets';
    }
    final preset = model.dockBuyPreset ?? names.first;
    return '$preset × ${model.dockBuyCount ?? _configuredBuyCount(preset)}';
  }

  String _longState(ToolId tool, ToolPhase phase) {
    if (tool == ToolId.pet && phase != ToolPhase.idle) {
      return 'resident · running';
    }
    return switch (phase) {
      ToolPhase.idle => 'setup',
      ToolPhase.starting => 'starting',
      ToolPhase.running => 'running',
      ToolPhase.stopping => 'stopping',
    };
  }

  String _menuCaption(ToolId tool) => switch (tool) {
    ToolId.buy => 'per-run options',
    ToolId.sell => 'bag cells for this run',
    ToolId.spammer => 'active rotation',
    ToolId.quest => 'active flow',
    ToolId.gem => 'saved start & empty mode',
    ToolId.tuner => 'saved target grade',
    _ => '',
  };

  IconData _placementIcon(DockPlacement placement) => switch (placement) {
    DockPlacement.rightRail => Icons.vertical_align_center,
    DockPlacement.leftRail => Icons.vertical_align_center,
    DockPlacement.bottomRight => Icons.south_east,
    DockPlacement.bottomLeft => Icons.south_west,
    DockPlacement.topCentre => Icons.north,
  };
}

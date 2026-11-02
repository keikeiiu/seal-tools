/// The Cards ("Run") screen: page header, the running tools as full cards above, the idle tools as
/// one-line tiles below, Hold Space included ([PLAN-V3-UI-UX.md] §6 "Cards", D2).
///
/// Every card's facts come from the model; this widget only projects them, plus ONE shared ticker for
/// the displayed next-action countdown. The ticker runs only while at least one published row carries
/// `next_action_in_seconds`, increments each card's display-only elapsed seconds once per second, and
/// is cancelled when the page leaves. A new `state` event for a tool resets that tool's elapsed count
/// (the snapshot object identity changes), so the core's published value stays the baseline. Rows
/// without `next_action_in_seconds` never show a countdown.
///
/// The one-line idle summary is the shell's own "what would Start do" line, read from the same config
/// the run reads (preset, armed slots, active rotation). It never invents a core fact: the card's live
/// phase/message/rows still come only from `state` events.
///
/// [PLAN-V3-UI-UX.md]: ../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../app/app.dart';
import '../protocol/protocol.dart';
import 'card_extras.dart';
import 'components/empty_state.dart';
import 'components/page_header.dart';
import 'components/status_chip.dart';
import 'theme.dart';
import 'tool_card.dart';
import 'tool_labels.dart';

class CardGrid extends StatefulWidget {
  const CardGrid({super.key, required this.model});

  final ShellModel model;

  @override
  State<CardGrid> createState() => _CardGridState();
}

class _CardGridState extends State<CardGrid> {
  Timer? _timer;

  /// Display-only elapsed whole seconds since each tool's latest `state` event.
  final Map<ToolId, int> _elapsed = {};

  /// The snapshot object each tool's elapsed count was started from; identity change = new event.
  final Map<ToolId, ToolSnapshot?> _snapshots = {};

  /// The tiles whose in-card options are open (Buy/Quest/Spammer); several may be open at once.
  final Set<ToolId> _openOptions = {};

  /// The Buy card's RUN choice (PROTOCOL §12.6): the preset the player picked and how many the run
  /// asks for. `null` means "not chosen yet" — resolution then falls back to the first preset in
  /// config order and that preset's own configured count, so the card and the Start button cannot
  /// disagree. Nothing here is persisted; the core receives it in `tool.start` args.
  String? _buyPreset;
  int? _buyCount;

  /// The saved `(preset, configured count)` the run choice above was last reconciled against. A
  /// per-run step is NOT persisted, so when the Buy/Sell screen saves a new configured count the
  /// saved value wins: without this, a stale step made the card keep a count the player had already
  /// changed on the Buy screen.
  String? _buyConfigSignature;

  String _buySignature(String? preset) =>
      '$preset:${_configuredBuyCount(preset)}';

  @override
  void initState() {
    super.initState();
    _adoptSnapshots();
    _syncBuyChoiceWithConfig();
    _syncTimer();
  }

  @override
  void didUpdateWidget(covariant CardGrid oldWidget) {
    super.didUpdateWidget(oldWidget);
    _adoptSnapshots();
    _syncBuyChoiceWithConfig();
    _syncTimer();
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  Map<String, dynamic> get _config => widget.model.config.config;

  Map<String, dynamic> get _buyPresets =>
      asJsonMap(asJsonMap(_config['buy_sell'])['presets']);

  /// First preset in CONFIG order is the default selection.
  String? get _resolvedBuyPreset {
    final names = _buyPresets.keys.toList();
    if (names.isEmpty) {
      return null;
    }
    final selected = _buyPreset;
    return selected != null && names.contains(selected)
        ? selected
        : names.first;
  }

  int _configuredBuyCount(String? preset) {
    final count =
        asNumber(asJsonMap(_buyPresets[preset])['count'])?.round() ?? 1;
    return count < 1 ? 1 : count;
  }

  int get _resolvedBuyCount {
    final chosen = _buyCount;
    if (chosen == null) {
      return _configuredBuyCount(_resolvedBuyPreset);
    }
    return chosen < 1 ? 1 : chosen;
  }

  void _onBuyPresetChanged(String? preset) {
    setState(() {
      _buyPreset = preset;
      _buyCount = _configuredBuyCount(preset);
      _buyConfigSignature = _buySignature(preset);
    });
  }

  void _onBuyCountChanged(int count) {
    setState(() => _buyCount = count < 1 ? 1 : count);
  }

  /// Discard a per-run count override when the SAVED config it was based on changes (the Buy/Sell
  /// screen's Save). An explicit preset choice is kept as long as that preset still exists; only the
  /// count returns to the configured value. Unrelated config changes keep the run choice, because the
  /// signature covers only the selected preset and its configured count.
  void _syncBuyChoiceWithConfig() {
    final preset = _resolvedBuyPreset;
    final signature = _buySignature(preset);
    final previous = _buyConfigSignature;
    _buyConfigSignature = signature;
    if (previous == null || previous == signature) {
      return;
    }
    _buyCount = null;
    if (_buyPreset != null && !_buyPresets.containsKey(_buyPreset)) {
      _buyPreset = null;
    }
  }

  void _toggleOptions(ToolId tool) {
    setState(() {
      if (!_openOptions.add(tool)) {
        _openOptions.remove(tool);
      }
    });
  }

  void _adoptSnapshots() {
    for (final tool in cardToolIds) {
      final snapshot = widget.model.tools.snapshot(tool);
      if (!identical(_snapshots[tool], snapshot)) {
        _snapshots[tool] = snapshot;
        _elapsed[tool] = 0;
      }
    }
  }

  bool _toolHasCountdown(ToolSnapshot? snapshot) =>
      snapshot?.rows.any((row) => row.nextActionInSeconds != null) ?? false;

  bool get _needsTimer => cardToolIds.any(
    (tool) => _toolHasCountdown(widget.model.tools.snapshot(tool)),
  );

  /// Start the one ticker when there is something to count down, stop it when there is not. Called on
  /// every model-driven rebuild, so the timer's lifetime follows the published rows.
  void _syncTimer() {
    if (_needsTimer) {
      _timer ??= Timer.periodic(const Duration(seconds: 1), (_) {
        setState(() {
          for (final tool in cardToolIds) {
            if (_toolHasCountdown(widget.model.tools.snapshot(tool))) {
              _elapsed[tool] = (_elapsed[tool] ?? 0) + 1;
            }
          }
        });
      });
    } else {
      _timer?.cancel();
      _timer = null;
    }
  }

  /// Start one card. `buy` carries its run choice and `sell` the next run's bag-cell selection as the
  /// optional §12.6 arguments; every other card sends the plain command. Nothing is persisted either
  /// way. An EMPTY sell selection sends NO `slots` key at all — never `[]`, which PROTOCOL §12.6
  /// refuses as a shell bug: the core then falls back to a migrated config (or answers its own
  /// "No slots are selected to sell." line, rendered verbatim like any refusal). A start does not
  /// clear the selection; the player may sell the same slots again.
  Future<bool> _startTool(ToolId tool) {
    if (tool == ToolId.buy) {
      final preset = _resolvedBuyPreset;
      if (preset == null) {
        return widget.model.startTool(tool);
      }
      return widget.model.startTool(
        tool,
        runArgs: {'preset': preset, 'count': _resolvedBuyCount},
      );
    }
    if (tool == ToolId.sell) {
      final slots = widget.model.sellSlots;
      if (slots.isEmpty) {
        return widget.model.startTool(tool);
      }
      return widget.model.startTool(tool, runArgs: {'slots': slots});
    }
    return widget.model.startTool(tool);
  }

  Widget? _extras(ToolId tool) => switch (tool) {
    ToolId.buy => BuyCardExtras(
      presets: _buyPresets,
      preset: _resolvedBuyPreset,
      count: _resolvedBuyCount,
      onPresetChanged: _onBuyPresetChanged,
      onCountChanged: _onBuyCountChanged,
    ),
    ToolId.spammer => SpammerCardExtras(model: widget.model),
    ToolId.quest => QuestCardExtras(model: widget.model),
    _ => null,
  };

  /// The shell's one-line "what would Start do" summary for an idle tile. These are derived from the
  /// same config/run state the Start uses; the live phase/message still come from the core.
  String _idleSummary(ToolId tool) {
    switch (tool) {
      case ToolId.spammer:
        final active = asString(asJsonMap(_config['spammer'])['active']);
        return active == null || active.isEmpty
            ? 'No rotation configured'
            : 'Ready · rotation "$active"';
      case ToolId.tuner:
        final grade = asString(asJsonMap(_config['tuner'])['target_grade']);
        return grade == null
            ? 'Ready · no target grade'
            : 'Ready · target $grade';
      case ToolId.gem:
        final grades = asJsonMap(_config['gem'])['grades'];
        final names = grades is List
            ? grades.whereType<String>().toList()
            : const <String>[];
        return names.isEmpty
            ? 'Ready · no grades configured'
            : 'Ready · ${names.join(' > ')}';
      case ToolId.buy:
        final preset = _resolvedBuyPreset;
        return preset == null
            ? 'Ready · no buy presets'
            : 'Ready · "$preset" · $_resolvedBuyCount per run';
      case ToolId.sell:
        final count = widget.model.sellSlots.length;
        return count == 0
            ? 'Ready · no slots armed'
            : 'Ready · $count slot${count == 1 ? '' : 's'} armed';
      case ToolId.quest:
        final quest = asJsonMap(_config['quest']);
        final sequence = asString(quest['active_sequence']);
        final flow = asString(quest['active']);
        final name = (sequence != null && sequence.isNotEmpty)
            ? sequence
            : flow;
        return name == null || name.isEmpty
            ? 'Ready · no flow selected'
            : 'Ready · flow "$name"';
      case ToolId.pet:
        // The machine block stores the breeding rows under `slots` (v3 CONFIG.md), not `rows`.
        final slots = asJsonList(asJsonMap(_config['pet'])['slots']);
        if (slots.isEmpty) {
          return 'Ready · no rows configured';
        }
        final enabled = slots
            .where((entry) => asJsonMap(entry)['enabled'] == true)
            .length;
        final rowWord = slots.length == 1 ? 'row' : 'rows';
        return enabled == slots.length
            ? 'Ready · ${slots.length} $rowWord'
            : 'Ready · $enabled of ${slots.length} $rowWord enabled';
      case ToolId.holdspace:
        return 'Off · spacebar auto-pickup';
    }
  }

  Widget _card(ToolId tool) {
    final options = _extras(tool);
    return ToolCard(
      tool: tool,
      snapshot: widget.model.tools.snapshot(tool),
      finished: widget.model.tools.finished(tool),
      refusal: widget.model.tools.refusal(tool),
      pending: widget.model.tools.isPending(tool),
      ready: widget.model.isReady,
      elapsedTicks: _elapsed[tool] ?? 0,
      extras: options,
      idleSummary: _idleSummary(tool),
      onOptions: options == null ? null : () => _toggleOptions(tool),
      optionsOpen: _openOptions.contains(tool),
      onStart: () => unawaited(_startTool(tool)),
      onStop: () => unawaited(widget.model.stopTool(tool)),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final running = <ToolId>[
      for (final tool in cardToolIds)
        if ((widget.model.tools.snapshot(tool)?.phase ?? ToolPhase.idle) !=
            ToolPhase.idle)
          tool,
    ];
    final idle = <ToolId>[
      for (final tool in cardToolIds)
        if (!running.contains(tool)) tool,
    ];
    final firstRun = cardToolIds.every(
      (tool) => widget.model.tools.snapshot(tool) == null,
    );

    Widget sectionTitle(String title, String meta) => Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.md),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.baseline,
        textBaseline: TextBaseline.alphabetic,
        children: [
          Text(
            title,
            style: theme.textTheme.titleSmall?.copyWith(
              fontSize: 13,
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(width: ShellSpace.sm),
          Text(
            meta,
            style: theme.textTheme.bodySmall?.copyWith(
              fontSize: 12,
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );

    return LayoutBuilder(
      builder: (context, constraints) {
        // Wide windows cap the reading/scanning width and centre it; without this the running cards
        // stretch to ~900 px on a 1920 display. 1240 matches the demo's content column.
        const contentMax = 1240.0;
        final viewport = constraints.maxWidth;
        final content = math.min(viewport, contentMax);
        final side = (viewport - content) / 2 + ShellSpace.xl;
        final width = content - ShellSpace.xl * 2;
        const runningGap = ShellSpace.md;
        const tileGap = 14.0;
        final runningColumns = width >= 680 ? 2 : 1;
        final idleColumns = width >= 1140 ? 3 : (width >= 640 ? 2 : 1);
        final runningWidth =
            (width - runningGap * (runningColumns - 1)) / runningColumns;
        final idleWidth = (width - tileGap * (idleColumns - 1)) / idleColumns;

        return ListView(
          key: const ValueKey('cards-list'),
          padding: EdgeInsets.fromLTRB(
            side,
            ShellSpace.lg,
            side,
            ShellSpace.xl,
          ),
          children: [
            PageHeader(
              title: 'Run',
              subtitle:
                  'One tool at a time. The running tool stays in front; everything '
                  'else waits its turn and stays out of the way.',
              trailing: StatusChip(
                kind: running.isEmpty ? StatusKind.muted : StatusKind.success,
                label: running.isEmpty ? 'Idle' : '${running.length} running',
              ),
            ),
            const SizedBox(height: ShellSpace.xl),
            if (running.isEmpty)
              Padding(
                padding: const EdgeInsets.only(bottom: ShellSpace.xl),
                child: EmptyState(
                  key: const ValueKey('cards-empty'),
                  icon: Icons.play_circle_outline,
                  title: firstRun ? 'No runs yet' : 'Everything is idle',
                  hint: firstRun
                      ? 'Pick a tool below and press Start. The shell tells the core; '
                            'the card fills in when the core answers.'
                      : 'Start a tool below. Only one tool uses the game at a time; '
                            'the rest wait.',
                ),
              )
            else ...[
              sectionTitle(
                'Running now',
                '${running.length} tool${running.length == 1 ? '' : 's'} · one at a time',
              ),
              Wrap(
                spacing: runningGap,
                runSpacing: runningGap,
                children: [
                  for (final tool in running)
                    SizedBox(width: runningWidth, child: _card(tool)),
                ],
              ),
              const SizedBox(height: ShellSpace.xl),
            ],
            sectionTitle('Available tools', '${idle.length} idle'),
            Wrap(
              spacing: tileGap,
              runSpacing: tileGap,
              children: [
                for (final tool in idle)
                  SizedBox(width: idleWidth, child: _card(tool)),
              ],
            ),
          ],
        );
      },
    );
  }
}

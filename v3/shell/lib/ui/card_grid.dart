/// The card grid: one card per PLAN-V3-SHELL §6 tool (seven — Hold Space has no card), in plan order.
///
/// Every card's facts come from the model; this widget only projects them, plus ONE shared ticker for
/// the displayed next-action countdown. The ticker lives here rather than in each card so seven cards
/// cannot drift or leak seven timers: it runs only while at least one published row carries
/// `next_action_in_seconds`, increments each card's display-only elapsed seconds once per second, and
/// is cancelled when the page leaves. A new `state` event for a tool resets that tool's elapsed count
/// (the snapshot object identity changes), so the core's published value stays the baseline. Rows
/// without `next_action_in_seconds` never show a countdown. Hold Space's report is attached to the
/// Spammer card because TOOL-FLOWS gives holdspace no card of its own.
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../app/app.dart';
import '../protocol/protocol.dart';
import 'card_extras.dart';
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

  /// The Buy card's RUN choice (PROTOCOL §12.6): the preset the player picked and how many the run
  /// asks for. `null` means "not chosen yet" — resolution then falls back to the first preset in
  /// config order and that preset's own configured count, so the card and the Start button cannot
  /// disagree. Nothing here is persisted; the core receives it in `tool.start` args.
  String? _buyPreset;
  int? _buyCount;

  @override
  void initState() {
    super.initState();
    _adoptSnapshots();
    _syncTimer();
  }

  @override
  void didUpdateWidget(covariant CardGrid oldWidget) {
    super.didUpdateWidget(oldWidget);
    _adoptSnapshots();
    _syncTimer();
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  // ── Buy card run choice (PROTOCOL §12.6) ─────────────────────────────────────────────────────

  Map<String, dynamic> get _buyPresets =>
      asJsonMap(asJsonMap(widget.model.config.config['buy_sell'])['presets']);

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
    });
  }

  void _onBuyCountChanged(int count) {
    setState(() => _buyCount = count < 1 ? 1 : count);
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

  @override
  Widget build(BuildContext context) {
    return GridView.builder(
      padding: const EdgeInsets.all(ShellSpace.lg),
      gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
        maxCrossAxisExtent: 460,
        mainAxisExtent: 360,
        crossAxisSpacing: ShellSpace.md,
        mainAxisSpacing: ShellSpace.md,
      ),
      itemCount: cardToolIds.length,
      itemBuilder: (context, index) {
        final tool = cardToolIds[index];
        return ToolCard(
          tool: tool,
          snapshot: widget.model.tools.snapshot(tool),
          finished: widget.model.tools.finished(tool),
          refusal: widget.model.tools.refusal(tool),
          pending: widget.model.tools.isPending(tool),
          ready: widget.model.isReady,
          elapsedTicks: _elapsed[tool] ?? 0,
          extras: switch (tool) {
            ToolId.buy => BuyCardExtras(
              presets: _buyPresets,
              preset: _resolvedBuyPreset,
              count: _resolvedBuyCount,
              onPresetChanged: _onBuyPresetChanged,
              onCountChanged: _onBuyCountChanged,
            ),
            ToolId.quest => QuestCardExtras(model: widget.model),
            _ => null,
          },
          holdSpaceLine: tool == ToolId.spammer
              ? holdSpaceReport(
                  widget.model.tools.snapshot(ToolId.holdspace),
                  widget.model.tools.refusal(ToolId.holdspace),
                )
              : null,
          onStart: () => unawaited(_startTool(tool)),
          onStop: () => unawaited(widget.model.stopTool(tool)),
        );
      },
    );
  }
}

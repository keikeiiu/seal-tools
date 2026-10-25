/// The shell window: chrome, the card grid, the log panel and the status strip.
///
/// One `ListenableBuilder` at the root renders the whole shell from the one model
/// ([PLAN-V3-SHELL.md] §5 — no InheritedNotifier is needed until a tab needs scoping). The window's
/// chrome (Mica, minimum size) belongs to the Windows runner's C++, not here.
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../app/app.dart';
import '../protocol/protocol.dart';
import 'card_grid.dart';
import 'crash_view.dart';
import 'hold_space_toggle.dart';
import 'log_panel.dart';
import 'status_strip.dart';
import 'tabs/attributes_tab.dart';
import 'tabs/buy_sell_tab.dart';
import 'tabs/gem_tab.dart';
import 'tabs/pet_tab.dart';
import 'tabs/setup_tab.dart';
import 'tabs/quest_tab.dart';
import 'tabs/spammer_tab.dart';
import 'tabs/tuner_tab.dart';
import 'theme.dart';

class ShellPage extends StatefulWidget {
  const ShellPage({super.key, required this.model});

  final ShellModel model;

  @override
  State<ShellPage> createState() => _ShellPageState();
}

class _ShellPageState extends State<ShellPage> {
  bool _logsVisible = false;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.model,
      builder: (context, _) {
        final model = widget.model;
        final connection = model.connection;
        final holdSpace = model.tools.snapshot(ToolId.holdspace);
        final holdSpaceEngaged =
            (holdSpace?.phase ?? ToolPhase.idle) != ToolPhase.idle;

        final ready = connection.status == ConnectionStatus.ready;
        return DefaultTabController(
          length: 9,
          child: Scaffold(
            appBar: AppBar(
              title: const Text('Seal Tools v3'),
              toolbarHeight: 48,
              elevation: 0,
              backgroundColor: Theme.of(context).colorScheme.surfaceContainer,
              actions: [
                HoldSpaceToggle(
                  snapshot: holdSpace,
                  pending: model.tools.isPending(ToolId.holdspace),
                  ready: model.isReady,
                  onToggle: () => unawaited(
                    holdSpaceEngaged
                        ? model.stopTool(ToolId.holdspace)
                        : model.startTool(ToolId.holdspace),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: ShellSpace.md,
                  ),
                  child: Center(
                    child: Text(
                      'v${model.shellVersion}',
                      style: Theme.of(context).textTheme.labelSmall,
                    ),
                  ),
                ),
              ],
              bottom: ready
                  ? const TabBar(
                      isScrollable: true,
                      tabs: [
                        Tab(text: 'Cards'),
                        Tab(text: 'Spammer'),
                        Tab(text: 'Tuner'),
                        Tab(text: 'Gem'),
                        Tab(text: 'Buy/Sell'),
                        Tab(text: 'Quest'),
                        Tab(text: 'Pet'),
                        Tab(text: 'Setup'),
                        Tab(text: 'Attributes'),
                      ],
                    )
                  : null,
            ),
            body: switch (connection.status) {
              ConnectionStatus.failed => CrashView(model: model),
              ConnectionStatus.ready => Column(
                children: [
                  Expanded(
                    child: TabBarView(
                      children: [
                        CardGrid(model: model),
                        SpammerTab(model: model),
                        TunerTab(model: model),
                        GemTab(model: model),
                        BuySellTab(model: model),
                        QuestTab(model: model),
                        PetTab(model: model),
                        SetupTab(model: model),
                        AttributesTab(model: model),
                      ],
                    ),
                  ),
                  if (_logsVisible) LogPanel(entries: model.logs.entries),
                  StatusStrip(
                    model: model,
                    logsVisible: _logsVisible,
                    onToggleLogs: () =>
                        setState(() => _logsVisible = !_logsVisible),
                  ),
                ],
              ),
              ConnectionStatus.idle ||
              ConnectionStatus.starting ||
              ConnectionStatus.handshaking => _ConnectingView(
                status: connection.status,
                stderrTail: model.logs.stderrTail,
              ),
            },
          ),
        );
      },
    );
  }
}

/// The first-frame surface ([PLAN-V3-SHELL.md] §2): the window paints before `hello`, showing that a
/// core is being started rather than a blank screen or a spinner forever.
class _ConnectingView extends StatelessWidget {
  const _ConnectingView({required this.status, required this.stderrTail});

  final ConnectionStatus status;
  final List<String> stderrTail;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final message = switch (status) {
      ConnectionStatus.idle => 'Waiting for the core to be started…',
      ConnectionStatus.starting => 'Starting the core…',
      ConnectionStatus.handshaking => 'Waiting for the core to say hello…',
      ConnectionStatus.ready || ConnectionStatus.failed => '',
    };
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const SizedBox(
            width: 28,
            height: 28,
            child: CircularProgressIndicator(strokeWidth: 3),
          ),
          const SizedBox(height: ShellSpace.md),
          Text(message, style: theme.textTheme.bodyMedium),
          if (stderrTail.isNotEmpty) ...[
            const SizedBox(height: ShellSpace.sm),
            Text(
              stderrTail.last,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.error,
              ),
            ),
          ],
        ],
      ),
    );
  }
}

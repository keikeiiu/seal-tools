/// The shell window: rail, top bar, content, logs drawer and status strip (D1 of
/// [PLAN-V3-UI-UX.md] §0/§3).
///
/// One `ListenableBuilder` at the root renders the whole shell from the one model
/// ([PLAN-V3-SHELL.md] §5 — no InheritedNotifier is needed until a destination needs scoping). The
/// rail is navigation only: it selects a destination; it decides nothing about the core. The window's
/// chrome (Mica, minimum size) belongs to the Windows runner's C++, not here.
///
/// Destinations are built lazily and then kept alive in an `IndexedStack`, matching the old
/// `TabBarView` + `AutomaticKeepAliveClientMixin` behaviour: opening a tab for the first time builds
/// it, and walking away does not lose its draft.
///
/// [PLAN-V3-UI-UX.md]: ../../../docs/plans/PLAN-V3-UI-UX.md
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app/app.dart';
import '../protocol/protocol.dart' hide Size;
import 'card_grid.dart';
import 'components/log_drawer.dart';
import 'components/nav_rail.dart';
import 'crash_view.dart';
import 'mini_mode.dart';
import 'quick_dock.dart';
import 'status_strip.dart';
import 'tabs/arduino_tab.dart';
import 'tabs/attributes_tab.dart';
import 'tabs/buy_sell_tab.dart';
import 'tabs/calibrate_tab.dart';
import 'tabs/gem_tab.dart';
import 'tabs/hotkeys_tab.dart';
import 'tabs/pet_tab.dart';
import 'tabs/quest_tab.dart';
import 'tabs/setup_tab.dart';
import 'tabs/spammer_tab.dart';
import 'tabs/tuner_tab.dart';
import 'theme.dart';
import 'tool_labels.dart';

class ShellPage extends StatefulWidget {
  const ShellPage({super.key, required this.model});

  final ShellModel model;

  @override
  State<ShellPage> createState() => _ShellPageState();
}

class _ShellPageState extends State<ShellPage> {
  int _selected = 0;
  bool _logsVisible = false;

  /// The rail can be collapsed to give the content panel the whole width (session-only, like the
  /// harness/GUI sidebar): the top-bar toggle or Ctrl+B.
  bool _railVisible = true;

  /// The full shell's subtree key: the mini bar keeps the shell mounted (and every tab's draft
  /// alive) by reparenting this keyed subtree offstage while the window is collapsed.
  final GlobalKey _shellKey = GlobalKey(debugLabel: 'full-shell');

  /// Destinations built at least once; the rest stay placeholders so opening a tab is lazy.
  final Set<int> _visited = {0};

  @override
  void initState() {
    super.initState();
    widget.model.setupRequest.addListener(_onSetupRequest);
  }

  @override
  void dispose() {
    widget.model.setupRequest.removeListener(_onSetupRequest);
    super.dispose();
  }

  void _onSetupRequest() {
    final tool = widget.model.setupRequest.value;
    if (tool == null) {
      return;
    }
    widget.model.setupRequest.value = null;
    final index = shellDestinations.indexWhere(
      (destination) => destination.key == tool.wire,
    );
    if (index >= 0) {
      _select(index);
    }
  }

  void _select(int index) {
    if (index == _selected) {
      return;
    }
    setState(() {
      _selected = index;
      _visited.add(index);
    });
  }

  void _toggleLogs() => setState(() => _logsVisible = !_logsVisible);

  void _toggleRail() => setState(() => _railVisible = !_railVisible);

  /// Esc is the demo's close key for the logs drawer; it must not steal Esc from the rest of the
  /// shell, so it only acts when the drawer is open (a closed drawer swallows nothing).
  void _closeLogs() {
    if (_logsVisible) {
      setState(() => _logsVisible = false);
    }
  }

  /// The pin applies to the window and saves the preference; a refused save is shown with the
  /// core's own line and the model has already put the window back on the saved value.
  Future<void> _toggleAlwaysOnTop() async {
    final outcome = await widget.model.setAlwaysOnTop(
      !widget.model.alwaysOnTop,
    );
    if (!mounted) {
      return;
    }
    if (outcome.saved) {
      return;
    }
    final lines = outcome.errorLines;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          lines.isEmpty ? 'Always-on-top was not saved.' : lines.join(' · '),
        ),
      ),
    );
  }

  List<Widget> get _pages => [
    // SHELL-F5: the card grid is the only page that renders every tool snapshot; a status/log event
    // rebuilds it (or Pet/Tuner, which listen themselves) instead of this whole shell.
    ListenableBuilder(
      listenable: Listenable.merge([widget.model.tools, widget.model.config]),
      builder: (context, _) => CardGrid(model: widget.model),
    ),
    SpammerTab(model: widget.model),
    TunerTab(model: widget.model),
    GemTab(model: widget.model),
    // Buy and Sell are separate screens (D3): each edits its own half of the `buy_sell` block.
    BuySellTab(model: widget.model, section: BuySellSection.buy),
    BuySellTab(model: widget.model, section: BuySellSection.sell),
    QuestTab(model: widget.model),
    PetTab(model: widget.model),
    AttributesTab(model: widget.model),
    // The Arduino tab shows the currently active tool from the tool model.
    ListenableBuilder(
      listenable: widget.model.tools,
      builder: (context, _) => ArduinoTab(model: widget.model),
    ),
    HotkeysTab(model: widget.model),
    SetupTab(model: widget.model),
    CalibrateTab(model: widget.model),
  ];

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.model,
      builder: (context, _) {
        final model = widget.model;
        // The full shell is always built for the same keyed subtree. While the mini bar is shown it
        // is reparented offstage inside a full-size OverflowBox (the 360×96 window would otherwise
        // force the shell's layout); expanding moves the same subtree back, so tab drafts survive.
        final full = KeyedSubtree(
          key: _shellKey,
          child: _buildFullShell(context, model),
        );
        if (!model.miniMode && !model.dockMode) {
          return full;
        }
        return Stack(
          children: [
            Offstage(
              offstage: true,
              child: ExcludeFocus(
                excluding: true,
                child: OverflowBox(
                  alignment: Alignment.topLeft,
                  minWidth: 1120,
                  maxWidth: 1120,
                  minHeight: 680,
                  maxHeight: 680,
                  child: full,
                ),
              ),
            ),
            Positioned.fill(
              child: ListenableBuilder(
                listenable: Listenable.merge([
                  model.tools,
                  model.config,
                  model.connection,
                ]),
                builder: (context, _) => model.miniMode
                    ? MiniRunningCard(model: model)
                    : QuickDock(model: model),
              ),
            ),
          ],
        );
      },
    );
  }

  /// The full tab shell: the crash/connecting surfaces until `ready`, then rail + top bar + content +
  /// status strip. Kept separate from [build] only so the mini bar can park it offstage.
  Widget _buildFullShell(BuildContext context, ShellModel model) {
    final connection = model.connection;
    final ready = connection.status == ConnectionStatus.ready;

    if (!ready) {
      return Scaffold(
        body: switch (connection.status) {
          ConnectionStatus.failed => CrashView(model: model),
          _ => ListenableBuilder(
            listenable: model.logs,
            builder: (context, _) => _ConnectingView(
              status: connection.status,
              stderrTail: model.logs.stderrTail,
            ),
          ),
        },
      );
    }

    final pages = _pages;
    return Scaffold(
      body: CallbackShortcuts(
        bindings: {
          const SingleActivator(LogicalKeyboardKey.keyL, control: true):
              _toggleLogs,
          const SingleActivator(LogicalKeyboardKey.keyB, control: true):
              _toggleRail,
          const SingleActivator(LogicalKeyboardKey.escape): _closeLogs,
        },
        child: Focus(
          autofocus: true,
          child: Column(
            children: [
              ListenableBuilder(
                listenable: Listenable.merge([model.logs, model.config]),
                builder: (context, _) => _TopBar(
                  model: model,
                  logsVisible: _logsVisible,
                  railVisible: _railVisible,
                  onToggleLogs: _toggleLogs,
                  onToggleRail: _toggleRail,
                  onTogglePin: () => unawaited(_toggleAlwaysOnTop()),
                ),
              ),
              Expanded(
                child: Row(
                  children: [
                    if (_railVisible)
                      ListenableBuilder(
                        listenable: model.tools,
                        builder: (context, _) => ShellRail(
                          selected: _selected,
                          onSelect: _select,
                          runningCount: cardRunningCount(model),
                        ),
                      ),
                    Expanded(
                      child: Column(
                        children: [
                          Expanded(
                            child: IndexedStack(
                              index: _selected,
                              children: [
                                for (
                                  var index = 0;
                                  index < pages.length;
                                  index++
                                )
                                  _visited.contains(index)
                                      ? pages[index]
                                      : const SizedBox.shrink(),
                              ],
                            ),
                          ),
                          if (_logsVisible)
                            ListenableBuilder(
                              listenable: model.logs,
                              builder: (context, _) => LogDrawer(
                                entries: model.logs.entries,
                                onClose: _toggleLogs,
                                onOpenLogsFolder: () =>
                                    unawaited(model.openLogsFolder()),
                                lastRunLogName: model.lastRunLogFile,
                              ),
                            ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              ListenableBuilder(
                listenable: model.config,
                builder: (context, _) =>
                    StatusStrip(model: model, logsVisible: _logsVisible),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The number of tools currently engaged, for the Cards badge (Hold Space included).
int cardRunningCount(ShellModel model) => cardToolIds
    .where(
      (tool) =>
          (model.tools.snapshot(tool)?.phase ?? ToolPhase.idle) !=
          ToolPhase.idle,
    )
    .length;

class _TopBar extends StatelessWidget {
  const _TopBar({
    required this.model,
    required this.logsVisible,
    required this.railVisible,
    required this.onToggleLogs,
    required this.onToggleRail,
    required this.onTogglePin,
  });

  final ShellModel model;
  final bool logsVisible;
  final bool railVisible;
  final VoidCallback onToggleLogs;
  final VoidCallback onToggleRail;
  final VoidCallback onTogglePin;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final ready = model.isReady;
    return Material(
      color: theme.colorScheme.surfaceContainer,
      child: SizedBox(
        height: 48,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: ShellSpace.md),
          child: Row(
            children: [
              // The window title ("Seal Tools v3") is the native Win32 caption, so the bar starts
              // with the sidebar toggle and then the core's state instead of repeating the product
              // name.
              IconButton(
                key: const ValueKey('rail-toggle'),
                tooltip: railVisible
                    ? 'Hide sidebar (Ctrl+B)'
                    : 'Show sidebar (Ctrl+B)',
                isSelected: railVisible,
                visualDensity: VisualDensity.compact,
                icon: const Icon(Icons.view_sidebar_outlined),
                selectedIcon: const Icon(Icons.view_sidebar),
                onPressed: onToggleRail,
              ),
              const SizedBox(width: ShellSpace.xs),
              _CoreStatus(model: model),
              const Spacer(),
              if (ready)
                IconButton(
                  key: const ValueKey('app-bar-dock'),
                  tooltip: 'Open the quick dock',
                  icon: const Icon(Icons.dashboard_customize_outlined),
                  onPressed: () => unawaited(model.enterDock()),
                ),
              if (ready && model.miniCandidate != null)
                IconButton(
                  key: const ValueKey('app-bar-mini'),
                  tooltip: 'Collapse to the mini window',
                  icon: const Icon(Icons.picture_in_picture_alt_outlined),
                  onPressed: () => unawaited(model.collapseToMini()),
                ),
              PopupMenuButton<ShellThemeMode>(
                key: const ValueKey('app-bar-theme'),
                tooltip: 'Appearance: ${model.themeMode.label}',
                onSelected: (mode) => unawaited(model.setThemeMode(mode)),
                icon: Icon(switch (model.themeMode) {
                  ShellThemeMode.system => Icons.brightness_auto_outlined,
                  ShellThemeMode.light => Icons.light_mode_outlined,
                  ShellThemeMode.dark => Icons.dark_mode_outlined,
                }),
                itemBuilder: (context) => [
                  for (final mode in ShellThemeMode.values)
                    PopupMenuItem<ShellThemeMode>(
                      value: mode,
                      child: Row(
                        children: [
                          Icon(
                            mode == model.themeMode
                                ? Icons.check
                                : Icons.circle_outlined,
                            size: ShellSize.iconSm,
                          ),
                          const SizedBox(width: ShellSpace.sm),
                          Text(mode.label),
                        ],
                      ),
                    ),
                ],
              ),
              TextButton.icon(
                key: const ValueKey('logs-toggle'),
                onPressed: onToggleLogs,
                style: TextButton.styleFrom(
                  foregroundColor: theme.colorScheme.onSurfaceVariant,
                  minimumSize: const Size(0, 30),
                  padding: const EdgeInsets.symmetric(
                    horizontal: ShellSpace.sm + ShellSpace.xxs,
                  ),
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(ShellRadius.s),
                    side: BorderSide(
                      color: theme.colorScheme.outlineVariant.withValues(
                        alpha: 0.5,
                      ),
                    ),
                  ),
                ),
                icon: Icon(
                  logsVisible ? Icons.close : Icons.subject,
                  size: ShellSize.iconSm,
                ),
                label: Text(
                  'Logs ${model.logs.entries.length}',
                  style: theme.textTheme.labelSmall?.copyWith(fontSize: 11.5),
                ),
              ),
              IconButton(
                key: const ValueKey('app-bar-always-on-top'),
                tooltip: 'Keep Seal Tools on top',
                isSelected: model.alwaysOnTop,
                icon: const Icon(Icons.push_pin_outlined),
                selectedIcon: const Icon(Icons.push_pin),
                onPressed: ready ? onTogglePin : null,
              ),
              Padding(
                padding: const EdgeInsets.only(left: ShellSpace.xs),
                child: Text(
                  'v$productVersion',
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The demo's top-bar core status: a success dot plus one line (`core ready · rev 7 · TW_LIVE`).
/// The dot is decorative; the words carry the meaning, and the state still comes from the connection.
class _CoreStatus extends StatelessWidget {
  const _CoreStatus({required this.model});

  final ShellModel model;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final title = asString(asJsonMap(model.config.config['window'])['title']);
    final ready = model.isReady;
    final parts = <String>[
      'core ${ready ? 'ready' : 'working'}',
      'rev ${model.config.rev}',
      if (title != null && title.isNotEmpty) title,
    ];
    return Row(
      key: const ValueKey('top-core-chip'),
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 6,
          height: 6,
          decoration: BoxDecoration(
            color: ready
                ? context.shellColors.success
                : context.shellColors.warning,
            shape: BoxShape.circle,
          ),
        ),
        const SizedBox(width: ShellSpace.xs + ShellSpace.xxs),
        Text(
          parts.join(' · '),
          style: theme.textTheme.bodySmall?.copyWith(
            fontSize: 12.5,
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ],
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
            width: ShellSize.iconLg,
            height: ShellSize.iconLg,
            child: CircularProgressIndicator(strokeWidth: 3),
          ),
          const SizedBox(height: ShellSpace.md),
          Text(message, style: theme.textTheme.bodyMedium),
          if (stderrTail.isNotEmpty) ...[
            const SizedBox(height: ShellSpace.sm),
            SizedBox(
              width: ShellWidth.prose,
              child: Text(
                stderrTail.last,
                textAlign: TextAlign.center,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

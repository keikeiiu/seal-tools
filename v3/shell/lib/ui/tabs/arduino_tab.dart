/// The Arduino tab: v2's Connection card rebuilt on the read-only `board.details` command, plus v3's
/// input tests and the explicit `board.probe` firmware query.
///
/// `board.details` ENUMERATES the serial ports and reports what a Start would do; it never opens the
/// port ([PROTOCOL.md] §4), so the card stays honest while a tool is running. `board.probe` is the
/// only action that opens the port — it is the explicit "Query firmware" here, disabled with a
/// reason while a tool is engaged instead of racing the running loop for the board.
///
/// The core owns the choice rule and its refusal sentence; this file owns the three v2 verdicts and
/// only the FALLBACK advice wording — a core-refusal sentence always renders first, so the duplicated
/// text cannot drift into a different answer than the core's.
///
/// Presentation follows [PLAN-V3-UI-UX.md] §6 "Arduino": the board card (a state chip + `InfoRow`
/// details) comes first, every enumerated port is a real row (icon + port + description + trailing
/// reason) instead of one space-padded string, and the test actions are outlined — with the
/// cursor-moving click separated and labelled, never a second filled primary.
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../app/arduino_details.dart';
import '../../protocol/protocol.dart';
import '../components/info_row.dart';
import '../components/shell_prose.dart';
import '../components/status_chip.dart';
import '../components/surface_panel.dart';
import '../theme.dart';
import '../tool_labels.dart';
import 'action_result_line.dart';

/// v2's no-board advice, and the FALLBACK only: the core sends the same sentence in
/// `choice.refusal` for `not_found` (it owns the rule + sentence), so the tab renders the core's
/// copy first and this text only covers a core that omits it — one source of truth on the wire,
/// with v2's wording still guaranteed.
const String _cableAdvice =
    'Plug the board in, then press Refresh. A charge-only cable is the usual cause — the board '
    'lights up and never appears as a port.';

class ArduinoTab extends StatefulWidget {
  const ArduinoTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<ArduinoTab> createState() => _ArduinoTabState();
}

class _ArduinoTabState extends State<ArduinoTab>
    with AutomaticKeepAliveClientMixin<ArduinoTab> {
  @override
  bool get wantKeepAlive => true;

  final _dx = TextEditingController(text: '100');
  final _dy = TextEditingController(text: '0');
  final _key = TextEditingController(text: 'E');

  CoreActionResult<BoardDetailsResult>? _details;
  CoreActionResult<BoardProbeResult>? _probe;
  CoreActionResult<Map<String, dynamic>>? _click;
  CoreActionResult<Map<String, dynamic>>? _move;
  CoreActionResult<Map<String, dynamic>>? _keyResult;
  bool _loadingDetails = true; // initState kicks the first read.
  bool _queryingFirmware = false;

  /// A level a successful Query firmware just reported, shown until the next details fetch (the
  /// core's cache is the authority after that, and a Refresh reads it).
  int? _probeLevel;

  @override
  void initState() {
    super.initState();
    // Fetched ONCE on open (and on Refresh): `board.details` is enumeration only, and re-reading it
    // on every rebuild of this State would be work for data that changes only on a plug/unplug.
    unawaited(_loadDetails());
  }

  @override
  void dispose() {
    _dx.dispose();
    _dy.dispose();
    _key.dispose();
    super.dispose();
  }

  Future<void> _loadDetails() async {
    if (!_loadingDetails) {
      setState(() => _loadingDetails = true);
    }
    final result = await widget.model.boardDetails();
    if (!mounted) {
      return;
    }
    setState(() {
      _loadingDetails = false;
      _details = result;
      // The core recomputed/returned its cache with the answer: it is the authority again.
      _probeLevel = null;
    });
  }

  /// The ONE action that opens the port: an explicit firmware query, never automatic.
  Future<void> _queryFirmware() async {
    setState(() => _queryingFirmware = true);
    final result = await widget.model.probeBoard();
    if (!mounted) {
      return;
    }
    setState(() {
      _queryingFirmware = false;
      _probe = result;
      final value = result.value;
      if (value != null) {
        _probeLevel = value.level;
      }
    });
  }

  Future<void> _runClick() async {
    final result = await widget.model.boardTestClick();
    if (!mounted) {
      return;
    }
    setState(() => _click = result);
  }

  Future<void> _runMove() async {
    final dx = int.tryParse(_dx.text.trim());
    final dy = int.tryParse(_dy.text.trim());
    if (dx == null || dy == null) {
      setState(() {
        _move = const CoreActionResult.failure(
          shellNote: 'dx and dy must be whole numbers (raw HID counts)',
        );
      });
      return;
    }
    final result = await widget.model.boardTestMove(dx: dx, dy: dy);
    if (!mounted) {
      return;
    }
    setState(() => _move = result);
  }

  Future<void> _runKey() async {
    final key = _key.text;
    if (key.isEmpty) {
      setState(() {
        _keyResult = const CoreActionResult.failure(
          shellNote: 'type the key to send',
        );
      });
      return;
    }
    final result = await widget.model.boardTestKey(key);
    if (!mounted) {
      return;
    }
    setState(() => _keyResult = result);
  }

  /// The tool a firmware query must wait for, or null when none is engaged.
  ///
  /// `board.probe` opens the COM port; a run that is using the board (or may be) must not have it
  /// taken away mid-loop, so Query firmware is disabled while any tool is not idle. The TAB itself is
  /// not gated: `board.details` opens nothing.
  ({ToolId tool, String name})? get _engagedTool {
    for (final entry in widget.model.tools.snapshots.entries) {
      if (entry.value.phase != ToolPhase.idle) {
        return (tool: entry.key, name: toolDisplayName(entry.key));
      }
    }
    return null;
  }

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    // Tool phases decide whether Query firmware is available, so the tab follows the model: a run
    // starting while the tab is open must disable the probe without a tab switch.
    return ListenableBuilder(
      listenable: widget.model,
      builder: (context, _) => _content(context),
    );
  }

  Widget _content(BuildContext context) {
    final theme = Theme.of(context);
    final engaged = _engagedTool;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(
            ShellSpace.lg,
            ShellSpace.lg,
            ShellSpace.lg,
            ShellSpace.sm,
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Arduino', style: theme.textTheme.headlineSmall),
                    const SizedBox(height: ShellSpace.xxs),
                    const ShellProse(
                      'What a Start would do with the board, the ports this machine has, and '
                      'the input tests. Clicks and moves act on the real cursor; the core\'s '
                      'refusals are shown verbatim.',
                    ),
                  ],
                ),
              ),
              OutlinedButton.icon(
                key: const ValueKey('arduino-refresh-details'),
                onPressed: _loadingDetails ? null : _loadDetails,
                icon: const Icon(Icons.refresh, size: ShellSize.iconMd),
                label: const Text('Refresh'),
              ),
            ],
          ),
        ),
        Expanded(
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(horizontal: ShellSpace.lg),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _boardPanel(theme),
                _portsPanel(theme),
                _firmwarePanel(theme, engaged),
                _testsPanel(theme),
                const SizedBox(height: ShellSpace.lg),
                const ShellProse(
                  'The board\'s own grammar is the contract: the shell sends intent, the core '
                  'opens the port, places the cursor closed-loop, and reports its refusals.',
                ),
                const SizedBox(height: ShellSpace.lg),
              ],
            ),
          ),
        ),
      ],
    );
  }

  /// The board card, FIRST (§6 "Arduino"): one state chip answers "will Start accept this board?",
  /// the verdict is the v2 sentence, and the launch data is a set of label/value rows instead of the
  /// old space-padded three-line string.
  Widget _boardPanel(ThemeData theme) {
    final result = _details;
    return SurfacePanel(title: 'Board', child: _connection(theme, result));
  }

  Widget _connection(
    ThemeData theme,
    CoreActionResult<BoardDetailsResult>? result,
  ) {
    if (result == null) {
      return Text(
        _loadingDetails ? 'Reading the serial ports…' : 'Not read yet.',
        key: const ValueKey('arduino-connection-pending'),
        style: theme.textTheme.bodySmall,
      );
    }
    if (!result.ok) {
      return ActionResultLine(
        key: const ValueKey('arduino-details-error'),
        label: 'Board details',
        line: result.errorLine,
        isError: true,
      );
    }
    final details = result.value!;
    final choice = details.choice;
    final (kind, stateWord, verdict) = switch (choice.state) {
      BoardChoiceState.ready => (
        StatusKind.success,
        'Ready',
        'Ready — Start will open ${choice.port ?? 'a port'}',
      ),
      BoardChoiceState.namedMissing => (
        StatusKind.error,
        'Will refuse',
        'Start will refuse — the named port is missing',
      ),
      BoardChoiceState.notFound => (
        StatusKind.error,
        'Will refuse',
        'Start will refuse — no Arduino found',
      ),
    };
    final advice = switch (choice.state) {
      BoardChoiceState.ready => null,
      // The core's own sentence names the port and says both ways out.
      BoardChoiceState.namedMissing =>
        choice.refusal ??
            'arduino.port names a port this machine does not have.',
      // The core's sentence wins when present (it carries v2's cable advice); the fallback keeps
      // the wording if a core ever omits it.
      BoardChoiceState.notFound => choice.refusal ?? _cableAdvice,
    };
    final expected = details.expected;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            StatusChip(
              key: const ValueKey('arduino-connection-status'),
              kind: kind,
              label: stateWord,
            ),
            const SizedBox(width: ShellSpace.sm),
            Expanded(
              child: Text(
                verdict,
                key: const ValueKey('arduino-connection-verdict'),
                style: theme.textTheme.bodyMedium,
              ),
            ),
          ],
        ),
        if (advice != null)
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: ShellWidth.prose),
              child: Text(
                advice,
                key: const ValueKey('arduino-advice'),
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ),
          ),
        const SizedBox(height: ShellSpace.md),
        // The DATA the launcher decides from: rows, not space-padded columns (the audit's §4.7
        // finding — proportional type never lines the old pseudo-columns up).
        KeyedSubtree(
          key: const ValueKey('arduino-looking-for'),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              InfoRow(label: 'VID', value: '0x${expected.vid}'),
              InfoRow(
                label: 'PID',
                value: expected.pids.map((pid) => '0x$pid').join(' / '),
              ),
              InfoRow(label: 'Baud', value: '${details.baud}'),
              InfoRow(
                label: 'arduino.port',
                value: expected.hasPortOverride
                    ? '${expected.port} — a named port wins over the IDs above'
                    : 'not set — the board is found by the IDs above',
              ),
            ],
          ),
        ),
      ],
    );
  }

  /// Every enumerated port, chosen first (the core's order), as a row: icon, port, description,
  /// trailing reason. The reason can no longer end up on its own line under a long description.
  Widget _portsPanel(ThemeData theme) {
    final details = _details?.value;
    final ports = details == null ? const <BoardPort>[] : details.ports;
    return SurfacePanel(
      title: 'Ports this machine has',
      child: ports.isEmpty
          ? Text(
              'This machine reports no serial ports at all.',
              key: const ValueKey('arduino-ports-empty'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [for (final port in ports) _portRow(theme, port)],
            ),
    );
  }

  Widget _portRow(ThemeData theme, BoardPort port) {
    final reason = port.chosen
        ? 'Start will open this one'
        : (port.matchesIds
              ? 'matches the expected ID'
              : 'does not match the expected IDs');
    final reasonColor = port.chosen
        ? theme.shellColors.success
        : theme.colorScheme.onSurfaceVariant;
    final description = port.description;
    return Padding(
      key: ValueKey('arduino-port-row-${port.port}'),
      padding: const EdgeInsets.only(bottom: ShellSpace.xs),
      child: Row(
        children: [
          Icon(
            port.chosen
                ? Icons.radio_button_checked
                : Icons.radio_button_unchecked,
            size: ShellSize.iconMd,
            color: reasonColor,
          ),
          const SizedBox(width: ShellSpace.sm),
          SizedBox(
            width: ShellWidth.fieldSm,
            child: Text(
              port.port,
              style: ShellText.mono.copyWith(
                color: theme.colorScheme.onSurface,
              ),
            ),
          ),
          const SizedBox(width: ShellSpace.md),
          Expanded(
            child: Text(
              description ?? '—',
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
          const SizedBox(width: ShellSpace.md),
          Text(
            reason,
            key: ValueKey('arduino-port-reason-${port.port}'),
            style: theme.textTheme.labelSmall?.copyWith(color: reasonColor),
          ),
        ],
      ),
    );
  }

  /// The CACHED level: "not checked yet" is normal (v2's wording), and the explicit Query firmware
  /// action is the only thing here that opens the port.
  Widget _firmwarePanel(
    ThemeData theme,
    ({ToolId tool, String name})? engaged,
  ) {
    final level = _probeLevel ?? _details?.value?.firmware?.level;
    final queried = _probeLevel != null;
    final headline = level == null
        ? 'Not checked yet — this is normal.'
        : (queried
              ? 'Protocol level $level (queried now).'
              : 'Protocol level $level (checked).');
    final detail = level == null
        ? 'The board is read the first time a tool opens it; Query firmware asks now.'
        : "Reported by the board's V reply.";
    final blocked = engaged != null || _queryingFirmware;
    return SurfacePanel(
      title: 'Board firmware',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            headline,
            key: const ValueKey('arduino-firmware'),
            style: theme.textTheme.bodyMedium,
          ),
          const SizedBox(height: ShellSpace.xxs),
          Text(
            detail,
            key: const ValueKey('arduino-firmware-detail'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: ShellSpace.sm),
          Wrap(
            spacing: ShellSpace.sm,
            runSpacing: ShellSpace.xs,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              OutlinedButton(
                key: const ValueKey('arduino-query-firmware'),
                onPressed: blocked ? null : _queryFirmware,
                child: Text(_queryingFirmware ? 'Querying…' : 'Query firmware'),
              ),
              if (engaged != null)
                Text(
                  'The board is busy while ${engaged.name} is running — stop the run before '
                  'querying the firmware.',
                  key: const ValueKey('arduino-query-busy'),
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
            ],
          ),
          if (_probe != null) ...[
            const SizedBox(height: ShellSpace.xs),
            ActionResultLine(
              key: const ValueKey('arduino-probe-result'),
              label: 'Probe',
              line: _probe!.ok
                  ? probeSummary(_probe!.value!)
                  : _probe!.errorLine,
              isError: !_probe!.ok,
            ),
          ],
        ],
      ),
    );
  }

  /// The three `board.test` actions. They are outlined, never filled: the page has no primary action
  /// because nothing here saves (§2.8). The click is separated and labelled because it moves the real
  /// cursor; the move and the key send are visibly ordinary.
  Widget _testsPanel(ThemeData theme) {
    return SurfacePanel(
      title: 'Input tests',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const ShellProse(
            'These act through the board on the real machine. Nothing is saved; each test '
            'runs only when you press it.',
          ),
          const SizedBox(height: ShellSpace.md),
          _testLead(
            theme,
            title: 'Click',
            description: 'Left-click once at the current cursor position.',
            cursorMoving: true,
          ),
          const SizedBox(height: ShellSpace.sm),
          OutlinedButton(
            key: const ValueKey('arduino-click'),
            onPressed: _runClick,
            child: const Text('Left click'),
          ),
          ActionResultLine(
            key: const ValueKey('arduino-click-result'),
            label: 'Click',
            line: _successOrError(_click),
            isError: _click != null && !_click!.ok,
          ),
          const Padding(
            padding: EdgeInsets.symmetric(vertical: ShellSpace.md),
            child: Divider(height: 1),
          ),
          _testLead(
            theme,
            title: 'Move',
            description: 'Raw HID counts — never scaled, never rounded.',
          ),
          const SizedBox(height: ShellSpace.sm),
          Row(
            children: [
              SizedBox(
                width: ShellWidth.fieldSm,
                child: TextField(
                  key: const ValueKey('arduino-move-dx'),
                  controller: _dx,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: 'dx',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                ),
              ),
              const SizedBox(width: ShellSpace.sm),
              SizedBox(
                width: ShellWidth.fieldSm,
                child: TextField(
                  key: const ValueKey('arduino-move-dy'),
                  controller: _dy,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: 'dy',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                ),
              ),
              const SizedBox(width: ShellSpace.md),
              OutlinedButton(
                key: const ValueKey('arduino-move'),
                onPressed: _runMove,
                child: const Text('Move'),
              ),
            ],
          ),
          ActionResultLine(
            key: const ValueKey('arduino-move-result'),
            label: 'Move',
            line: _successOrError(_move),
            isError: _move != null && !_move!.ok,
          ),
          const Padding(
            padding: EdgeInsets.symmetric(vertical: ShellSpace.md),
            child: Divider(height: 1),
          ),
          _testLead(
            theme,
            title: 'Key',
            description: 'One character, Enter, or F1–F12 — the core refuses anything it cannot send.',
          ),
          const SizedBox(height: ShellSpace.sm),
          Row(
            children: [
              SizedBox(
                width: ShellWidth.labelColumn,
                child: TextField(
                  key: const ValueKey('arduino-key-input'),
                  controller: _key,
                  decoration: const InputDecoration(
                    labelText: 'Key',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                ),
              ),
              const SizedBox(width: ShellSpace.md),
              OutlinedButton(
                key: const ValueKey('arduino-send-key'),
                onPressed: _runKey,
                child: const Text('Send key'),
              ),
            ],
          ),
          ActionResultLine(
            key: const ValueKey('arduino-key-result'),
            label: 'Key',
            line: _successOrError(_keyResult),
            isError: _keyResult != null && !_keyResult!.ok,
          ),
        ],
      ),
    );
  }

  Widget _testLead(
    ThemeData theme, {
    required String title,
    required String description,
    bool cursorMoving = false,
  }) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      Row(
        children: [
          Text(title, style: theme.textTheme.titleSmall),
          if (cursorMoving) ...[
            const SizedBox(width: ShellSpace.sm),
            const StatusChip(
              kind: StatusKind.warning,
              label: 'moves the cursor',
            ),
          ],
        ],
      ),
      const SizedBox(height: ShellSpace.xxs),
      ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: ShellWidth.prose),
        child: Text(
          description,
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ),
    ],
  );

  /// The core answers `{}` on success (no text of its own); a refusal carries its line.
  String? _successOrError(CoreActionResult<Map<String, dynamic>>? result) {
    if (result == null) {
      return null;
    }
    return result.ok ? 'sent' : result.errorLine;
  }
}

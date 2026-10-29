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
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../app/arduino_details.dart';
import '../../protocol/protocol.dart';
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
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Arduino', style: theme.textTheme.titleLarge),
                    const SizedBox(height: 2),
                    Text(
                      'What a Start would do with the board, the ports this machine has, and the '
                      'input tests. Clicks and moves act on the real cursor; the core\'s refusals '
                      'are shown verbatim.',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
              OutlinedButton.icon(
                key: const ValueKey('arduino-refresh-details'),
                onPressed: _loadingDetails ? null : _loadDetails,
                icon: const Icon(Icons.refresh, size: 18),
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
                _sectionTitle(theme, 'Connection'),
                _connection(theme),
                const SizedBox(height: ShellSpace.md),
                _sectionTitle(theme, 'Ports this machine has'),
                _ports(theme),
                const SizedBox(height: ShellSpace.md),
                _sectionTitle(theme, 'What the launcher looks for'),
                _lookingFor(theme),
                const SizedBox(height: ShellSpace.md),
                _sectionTitle(theme, 'Board firmware'),
                _firmware(theme, engaged),
                const SizedBox(height: ShellSpace.lg),
                Wrap(
                  spacing: ShellSpace.lg,
                  runSpacing: ShellSpace.lg,
                  crossAxisAlignment: WrapCrossAlignment.start,
                  children: [
                    _testBox(
                      theme,
                      'Click',
                      'Left-click once at the current cursor position.',
                      [
                        FilledButton(
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
                      ],
                    ),
                    _testBox(
                      theme,
                      'Move',
                      'Raw HID counts — never scaled, never rounded.',
                      [
                        Row(
                          children: [
                            SizedBox(
                              width: 120,
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
                              width: 120,
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
                          ],
                        ),
                        const SizedBox(height: ShellSpace.sm),
                        FilledButton(
                          key: const ValueKey('arduino-move'),
                          onPressed: _runMove,
                          child: const Text('Move'),
                        ),
                        ActionResultLine(
                          key: const ValueKey('arduino-move-result'),
                          label: 'Move',
                          line: _successOrError(_move),
                          isError: _move != null && !_move!.ok,
                        ),
                      ],
                    ),
                    _testBox(
                      theme,
                      'Key',
                      'One character, Enter, or F1–F12 — the core refuses anything it cannot send.',
                      [
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
                        const SizedBox(height: ShellSpace.sm),
                        FilledButton(
                          key: const ValueKey('arduino-send-key'),
                          onPressed: _runKey,
                          child: const Text('Send key'),
                        ),
                        ActionResultLine(
                          key: const ValueKey('arduino-key-result'),
                          label: 'Key',
                          line: _successOrError(_keyResult),
                          isError: _keyResult != null && !_keyResult!.ok,
                        ),
                      ],
                    ),
                  ],
                ),
                const SizedBox(height: ShellSpace.lg),
                Text(
                  'The board\'s own grammar is the contract: the shell sends intent, the core '
                  'opens the port, places the cursor closed-loop, and reports its refusals.',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
                const SizedBox(height: ShellSpace.lg),
              ],
            ),
          ),
        ),
      ],
    );
  }

  Widget _sectionTitle(ThemeData theme, String title) => Padding(
    padding: const EdgeInsets.only(bottom: ShellSpace.xs),
    child: Text(title, style: theme.textTheme.titleSmall),
  );

  /// v2's three-state light + verdict + advice. The light answers "what would Start do?", which is
  /// why `named_missing` is red even when another board matches the IDs: Start would refuse it.
  Widget _connection(ThemeData theme) {
    final result = _details;
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
    final outcome = switch (choice.state) {
      BoardChoiceState.ready => (
        light: theme.shellColors.success,
        verdict: 'Ready — Start will open ${choice.port ?? 'a port'}',
      ),
      BoardChoiceState.namedMissing => (
        light: theme.colorScheme.error,
        verdict: 'Start will refuse — the named port is missing',
      ),
      BoardChoiceState.notFound => (
        light: theme.colorScheme.error,
        verdict: 'Start will refuse — no Arduino found',
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
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Container(
              key: const ValueKey('arduino-connection-status'),
              width: 12,
              height: 12,
              decoration: BoxDecoration(
                color: outcome.light,
                shape: BoxShape.circle,
              ),
            ),
            const SizedBox(width: ShellSpace.sm),
            Expanded(
              child: Text(
                outcome.verdict,
                key: const ValueKey('arduino-connection-verdict'),
                style: theme.textTheme.bodyMedium,
              ),
            ),
          ],
        ),
        if (advice != null)
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              advice,
              key: const ValueKey('arduino-advice'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
      ],
    );
  }

  /// Every enumerated port, chosen first (the core's order), with v2's bullets and reasons.
  Widget _ports(ThemeData theme) {
    final details = _details?.value;
    final ports = details == null ? const <BoardPort>[] : details.ports;
    if (ports.isEmpty) {
      return Text(
        'This machine reports no serial ports at all.',
        key: const ValueKey('arduino-ports-empty'),
        style: theme.textTheme.bodySmall?.copyWith(
          color: theme.colorScheme.onSurfaceVariant,
        ),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [for (final port in ports) _portRow(theme, port)],
    );
  }

  Widget _portRow(ThemeData theme, BoardPort port) {
    final marker = port.chosen ? '●' : '○';
    final reason = port.chosen
        ? '— Start will open this one'
        : (port.matchesIds ? '— matches the expected ID' : null);
    final description = port.description;
    return Padding(
      padding: const EdgeInsets.only(bottom: 2),
      child: Text(
        '$marker  ${port.port}'
        '${description == null ? '' : '   $description'}'
        '${reason == null ? '' : '     $reason'}',
        key: ValueKey('arduino-port-row-${port.port}'),
        style: theme.textTheme.bodySmall?.copyWith(
          color: port.chosen
              ? theme.shellColors.success
              : theme.colorScheme.onSurfaceVariant,
        ),
      ),
    );
  }

  /// The DATA the launcher decides from, printed because it explains everything above (v2's card).
  Widget _lookingFor(ThemeData theme) {
    final details = _details?.value;
    if (details == null) {
      return const SizedBox.shrink();
    }
    final expected = details.expected;
    final pids = expected.pids.map((pid) => '0x$pid').join(' / ');
    final port = expected.hasPortOverride
        ? 'arduino.port: ${expected.port} — a named port wins over the ID above'
        : 'arduino.port: not set — the board is found by the ID above';
    return Text(
      'VID 0x${expected.vid}    PID $pids\nbaud ${details.baud}\n$port',
      key: const ValueKey('arduino-looking-for'),
      style: theme.textTheme.bodySmall,
    );
  }

  /// The CACHED level: "not checked yet" is normal (v2's wording), and the explicit Query firmware
  /// action is the only thing here that opens the port.
  Widget _firmware(ThemeData theme, ({ToolId tool, String name})? engaged) {
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
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          headline,
          key: const ValueKey('arduino-firmware'),
          style: theme.textTheme.bodyMedium,
        ),
        const SizedBox(height: 2),
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
            line: _probe!.ok ? probeSummary(_probe!.value!) : _probe!.errorLine,
            isError: !_probe!.ok,
          ),
        ],
      ],
    );
  }

  /// The core answers `{}` on success (no text of its own); a refusal carries its line.
  String? _successOrError(CoreActionResult<Map<String, dynamic>>? result) {
    if (result == null) {
      return null;
    }
    return result.ok ? 'sent' : result.errorLine;
  }

  Widget _testBox(
    ThemeData theme,
    String title,
    String description,
    List<Widget> children,
  ) => SizedBox(
    width: 320,
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: theme.textTheme.titleSmall),
        const SizedBox(height: 2),
        Text(
          description,
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: ShellSpace.sm),
        ...children,
      ],
    ),
  );
}

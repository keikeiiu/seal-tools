/// The Arduino tab: the no-hardware and hardware test surface. It reads the board's facts
/// (`board.probe`) and sends the calibrator's input tests (`board.test`: left click, move, key), each
/// rendering the core's own success/refusal line.
///
/// The primary **Probe** action lives on Setup; this tab keeps a probe READOUT at the top (fetched on
/// open and refreshable) so the player can see the board before sending a test. Tests are input
/// calibration: the board moves the cursor and clicks for real, so the buttons are explicit and
/// nothing is sent automatically.
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';
import 'action_result_line.dart';

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

  CoreActionResult<BoardProbeResult>? _probe;
  CoreActionResult<Map<String, dynamic>>? _click;
  CoreActionResult<Map<String, dynamic>>? _move;
  CoreActionResult<Map<String, dynamic>>? _keyResult;
  bool _probing = false;

  @override
  void initState() {
    super.initState();
    _runProbe();
  }

  @override
  void dispose() {
    _dx.dispose();
    _dy.dispose();
    _key.dispose();
    super.dispose();
  }

  Future<void> _runProbe() async {
    if (!mounted) {
      return;
    }
    setState(() => _probing = true);
    final result = await widget.model.probeBoard();
    if (!mounted) {
      return;
    }
    setState(() {
      _probing = false;
      _probe = result;
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

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final theme = Theme.of(context);
    final probe = _probe;

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
                      'Board facts and input tests. Clicks and moves act on the real cursor through '
                      'the board; the core\'s refusals are shown verbatim.',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
              OutlinedButton.icon(
                key: const ValueKey('arduino-refresh-probe'),
                onPressed: _probing ? null : _runProbe,
                icon: const Icon(Icons.refresh, size: 18),
                label: const Text('Refresh probe'),
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
                if (probe != null)
                  ActionResultLine(
                    key: const ValueKey('arduino-probe-result'),
                    label: 'Board',
                    line: probe.ok
                        ? probeSummary(probe.value!)
                        : probe.errorLine,
                    isError: !probe.ok,
                  )
                else
                  Text(
                    _probing ? 'Probing the board…' : 'No probe yet.',
                    style: theme.textTheme.bodySmall,
                  ),
                const SizedBox(height: ShellSpace.md),
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
                          width: 160,
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

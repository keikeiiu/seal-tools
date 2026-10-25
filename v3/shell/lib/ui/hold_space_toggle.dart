/// The Hold Space chrome toggle ([PLAN-V3-SHELL.md] §6: it is chrome, not a card; [PROTOCOL.md] §9
/// gives holdspace no card, and TOOL-FLOWS puts its reporting on the Spammer card's surface).
///
/// The toggle sends `tool.start`/`tool.stop` and reflects the CORE's phase — it does not flip
/// optimistically. Its refusal line is shown on the Spammer card (see [holdSpaceReport]).
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'package:flutter/material.dart';

import '../protocol/protocol.dart';

class HoldSpaceToggle extends StatelessWidget {
  const HoldSpaceToggle({
    super.key,
    required this.snapshot,
    required this.pending,
    required this.ready,
    required this.onToggle,
  });

  /// The core's last `state` snapshot for `holdspace`, or null.
  final ToolSnapshot? snapshot;

  /// A holdspace command is in flight (shell convenience).
  final bool pending;

  /// The connection is ready.
  final bool ready;

  final VoidCallback onToggle;

  @override
  Widget build(BuildContext context) {
    final phase = snapshot?.phase ?? ToolPhase.idle;
    final engaged = phase != ToolPhase.idle;
    return Tooltip(
      message: engaged
          ? 'Hold Space is engaged — this releases the spacebar'
          : 'Holds the spacebar down for the game to pick items up',
      child: TextButton.icon(
        key: const ValueKey('holdspace-toggle'),
        onPressed: ready && !pending ? onToggle : null,
        icon: Icon(
          engaged ? Icons.pause_circle_outline : Icons.play_circle_outline,
          size: 18,
        ),
        label: Text(engaged ? 'Release Space' : 'Hold Space'),
      ),
    );
  }
}

/// Shared rendering for read-only core action results (Setup's Find/Measure/Probe, Arduino's
/// Probe/Test). One renderer keeps the wording and the error styling identical across tabs, and every
/// refusal line is the core's own (a shell note only appears when the command could not be sent).
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../protocol/protocol.dart';
import '../theme.dart';

/// One labelled action outcome. Renders nothing until [line] exists.
class ActionResultLine extends StatelessWidget {
  const ActionResultLine({
    super.key,
    required this.label,
    required this.line,
    this.isError = false,
  });

  final String label;
  final String? line;
  final bool isError;

  @override
  Widget build(BuildContext context) {
    final line = this.line;
    if (line == null) {
      return const SizedBox.shrink();
    }
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.xs),
      child: Text(
        '$label: $line',
        style: theme.textTheme.bodySmall?.copyWith(
          color: isError ? theme.colorScheme.error : null,
        ),
      ),
    );
  }
}

/// `board.probe`'s one-line readout.
String probeSummary(BoardProbeResult result) =>
    'port ${result.port} · ${result.baud} baud · '
    'VID ${result.vid ?? '?'} · PID ${result.pid ?? '?'} · '
    'protocol level ${result.level}';

/// `window.measure`/capture's one-line `DisplayInfo` summary. Kept here so Setup's Measure and the
/// Calibrate surface describe the same measurement in exactly the same words.
String displaySummary(DisplayInfo display) =>
    'scale ${display.scale} · monitor DPI ${display.monitorDpi} · '
    'screen ${display.screen.w}x${display.screen.h} · '
    'client ${display.physicalClient.w}x${display.physicalClient.h} physical / '
    '${display.logicalClient.w}x${display.logicalClient.h} logical · '
    'frame ${display.frame.w}x${display.frame.h} at '
    '(${display.frame.x}, ${display.frame.y})';

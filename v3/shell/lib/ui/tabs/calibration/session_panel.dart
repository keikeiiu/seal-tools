/// The Calibrate workspace's session-shaped calibrations panel: the v2 calibrations that need more
/// than one frame mark (tooltip offset, tuner OCR three-box derivation, pet reference crops).
///
/// They are listed with what each needs and a DISABLED "Run session" action, because a
/// multi-command calibration session does not exist in the protocol yet (PROTOCOL §12.9; CALIBRATION
/// §7). The workspace never fakes them into the kind picker: the frame-mark kinds stay the only
/// calibratable marks in this build.
///
/// [CALIBRATION.md]: ../../../docs/spec/CALIBRATION.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'package:flutter/material.dart';

import '../../components/status_chip.dart';
import '../../components/surface_panel.dart';
import '../../theme.dart';

class SessionCalibrationsPanel extends StatelessWidget {
  const SessionCalibrationsPanel({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    return SurfacePanel(
      title: 'Session-shaped calibrations',
      description:
          'These need more than one mark or capture. The frame-mark protocol cannot express them '
          'yet, so each action stays disabled rather than being faked.',
      padding: const EdgeInsets.all(ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _SessionRow(
            id: 'tooltip',
            title: 'Tooltip offset + shape',
            detail:
                'Two captures: park the cursor, hover the panel, then box it. The offset is the '
                'box top-left minus the placed point. Needs a two-capture hover session.',
            chip: const _SessionChip('needs protocol'),
          ),
          _SessionRow(
            id: 'tuner',
            title: 'Tuner OCR 3-box',
            detail: const _TunerNote(),
            chip: const _SessionChip('needs protocol'),
          ),
          _SessionRow(
            id: 'pet',
            title: 'Pet reference crops',
            detail:
                'Food/queue icon crops are captured from the running game on the Pet tab; the '
                'close-X crop is learned by the run. Live-screen session features, not frame marks.',
            chip: const _SessionChip('on Pet tab'),
          ),
          const SizedBox(height: ShellSpace.xxs),
          Text(
            'The only calibration primitives the shell has are per-command '
            '`pointer.place`/`pointer.click`; a calibration action spanning several commands is a '
            'session increment and gets its own protocol shape when it lands.',
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.onSurfaceVariant,
            ),
          ),
        ],
      ),
    );
  }
}

/// v2's three-box session note. Kept as its own keyed Text because the Calibrate tab has always
/// explained why the tuner geometry is not calibratable here.
class _TunerNote extends StatelessWidget {
  const _TunerNote();

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Text(
      "The tuner's OCR region, grade box and row bands are measured as v2's three-box session "
      '(grade letter, attribute rows, remaining time) and the core derives the region from them. '
      'That session is not built in v3 yet, so no tuner mark is offered here.',
      key: const ValueKey('calibrate-tuner-note'),
      style: theme.textTheme.bodySmall?.copyWith(
        color: theme.colorScheme.onSurfaceVariant,
      ),
    );
  }
}

class _SessionChip extends StatelessWidget {
  const _SessionChip(this.label);

  final String label;

  @override
  Widget build(BuildContext context) =>
      StatusChip(kind: StatusKind.muted, label: label);
}

class _SessionRow extends StatelessWidget {
  const _SessionRow({
    required this.id,
    required this.title,
    required this.detail,
    required this.chip,
  });

  final String id;
  final String title;

  /// Either a plain description or a keyed note widget.
  final Object detail;
  final Widget chip;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final detailWidget = detail;
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.sm),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 5),
            child: Icon(Icons.lock_outline, size: 12, color: colors.outline),
          ),
          const SizedBox(width: ShellSpace.sm),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Wrap(
                  spacing: ShellSpace.sm,
                  runSpacing: ShellSpace.xxs,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Text(title, style: theme.textTheme.labelLarge),
                    chip,
                  ],
                ),
                if (detailWidget is Widget)
                  detailWidget
                else
                  ConstrainedBox(
                    constraints: const BoxConstraints(
                      maxWidth: ShellWidth.prose,
                    ),
                    child: Text(
                      detailWidget as String,
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: colors.onSurfaceVariant,
                      ),
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(width: ShellSpace.md),
          Tooltip(
            message: 'Not available yet — this calibration needs a protocol session.',
            child: OutlinedButton(
              key: ValueKey('calibrate-run-session-$id'),
              onPressed: null,
              child: const Text('Run session'),
            ),
          ),
        ],
      ),
    );
  }
}

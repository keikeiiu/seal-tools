/// Card extras: the small config-driven controls PLAN-V3-SHELL §6 puts ON the cards rather than only
/// in a tab. Nothing here owns a rule: names and values come from the merged config, and a persisted
/// change is a normal `config.validate`/`apply` through the model.
///
/// Buy card — the picker and stepper choose the item and amount FOR THIS RUN (PROTOCOL §12.6:
/// `tool.start {"tool":"buy","preset":<name>,"count":N}`). Nothing is persisted: no `config.apply`
/// is ever sent for it. The selected preset defaults to the first in config order with that preset's
/// configured count; changing the preset resets the count. With no presets configured the Start is
/// plain `{"tool":"buy"}` and the core's "No buy preset selected." refusal renders under the card
/// through the card's refusal line.
///
/// Quest card — `quest.active` / `quest.active_sequence` ARE config fields, so a picker change is
/// saved here as exactly `{quest: {active|active_sequence}}` (field-wise merge), only when the player
/// changes it. The empty sequence choice is the config's "single flow only" meaning.
///
/// Both layouts are width-adaptive: a grid card is ~360 px wide, so controls stack instead of
/// overflowing a fixed Row.
library;

import 'package:flutter/material.dart';

import '../app/app.dart';
import 'theme.dart';

/// A compact, non-deprecated dropdown: `DropdownButton` inside an `InputDecorator` (the form-field
/// variant's `value:` is deprecated, and these are controlled from the mirror, not form state).
Widget _cardDropdown({
  required Key key,
  required String label,
  required String? value,
  required List<DropdownMenuItem<String>> items,
  required ValueChanged<String?>? onChanged,
}) => InputDecorator(
  decoration: InputDecoration(
    labelText: label,
    isDense: true,
    border: const OutlineInputBorder(),
  ),
  child: DropdownButtonHideUnderline(
    child: DropdownButton<String>(
      key: key,
      value: value,
      isExpanded: true,
      isDense: true,
      items: items,
      onChanged: onChanged,
    ),
  ),
);

/// The Buy card's run-choice picker and count stepper (controlled by `CardGrid`, which owns the
/// selection so the Start button and the extras cannot disagree).
class BuyCardExtras extends StatelessWidget {
  const BuyCardExtras({
    super.key,
    required this.presets,
    required this.preset,
    required this.count,
    required this.onPresetChanged,
    required this.onCountChanged,
  });

  /// The config's `buy_sell.presets`, in config order.
  final Map<String, dynamic> presets;

  /// The selected preset, or null when no preset is configured.
  final String? preset;

  /// The run count (>= 1); the controller clamps it.
  final int count;

  final ValueChanged<String?> onPresetChanged;
  final ValueChanged<int> onCountChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final names = presets.keys.toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: 170,
          child: _cardDropdown(
            key: const ValueKey('buy-card-preset'),
            label: 'Buy item',
            value: preset,
            items: [
              for (final name in names)
                DropdownMenuItem(
                  key: ValueKey('buy-card-item-$name'),
                  value: name,
                  child: Text(name),
                ),
            ],
            onChanged: names.isEmpty ? null : onPresetChanged,
          ),
        ),
        const SizedBox(height: 2),
        if (names.isEmpty)
          Text(
            'No buy presets configured',
            key: const ValueKey('buy-card-count'),
            style: theme.textTheme.bodySmall,
          )
        else
          Row(
            children: [
              IconButton(
                key: const ValueKey('buy-card-count-minus'),
                tooltip: 'One less',
                visualDensity: VisualDensity.compact,
                onPressed: count <= 1 ? null : () => onCountChanged(count - 1),
                icon: const Icon(Icons.remove, size: 16),
              ),
              Text(
                '$count',
                key: const ValueKey('buy-card-count-value'),
                style: theme.textTheme.bodySmall,
              ),
              IconButton(
                key: const ValueKey('buy-card-count-plus'),
                tooltip: 'One more',
                visualDensity: VisualDensity.compact,
                onPressed: () => onCountChanged(count + 1),
                icon: const Icon(Icons.add, size: 16),
              ),
            ],
          ),
        Text(
          'Runs the selected item this many times — not saved to the config',
          key: const ValueKey('buy-card-note'),
          softWrap: true,
          style: theme.textTheme.labelSmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ],
    );
  }
}

/// The Quest card's flow/sequence pickers, persisted through the normal config save.
class QuestCardExtras extends StatefulWidget {
  const QuestCardExtras({super.key, required this.model});

  final ShellModel model;

  @override
  State<QuestCardExtras> createState() => _QuestCardExtrasState();
}

class _QuestCardExtrasState extends State<QuestCardExtras> {
  bool _saving = false;
  String? _error;

  Map<String, dynamic> get _quest =>
      asJsonMap(widget.model.config.config['quest']);

  List<String> get _flowNames =>
      asJsonMap(_quest['presets']).keys.toList()..sort();

  List<String> get _sequenceNames =>
      asJsonMap(_quest['sequences']).keys.toList()..sort();

  String get _active => asString(_quest['active']) ?? '';

  String get _activeSequence => asString(_quest['active_sequence']) ?? '';

  Future<void> _save(String field, String value) async {
    setState(() {
      _saving = true;
      _error = null;
    });
    final outcome = await widget.model.saveConfig({
      'quest': {field: value},
    });
    if (!mounted) {
      return;
    }
    setState(() {
      _saving = false;
      _error = outcome.saved ? null : outcome.errorLines.join('\n');
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final flows = _flowNames;
    final sequences = _sequenceNames;
    final flowPicker = _cardDropdown(
      key: const ValueKey('quest-card-flow'),
      label: 'Target flow',
      value: flows.contains(_active) ? _active : null,
      items: [
        for (final name in flows)
          DropdownMenuItem(
            key: ValueKey('quest-card-flow-item-$name'),
            value: name,
            child: Text(name),
          ),
      ],
      onChanged: _saving || flows.isEmpty
          ? null
          : (value) {
              if (value != null) {
                _save('active', value);
              }
            },
    );
    final sequencePicker = _cardDropdown(
      key: const ValueKey('quest-card-sequence'),
      label: 'Sequence',
      value: sequences.contains(_activeSequence) ? _activeSequence : '',
      items: [
        const DropdownMenuItem(
          key: ValueKey('quest-card-sequence-item-single-flow'),
          value: '',
          child: Text('Single flow only'),
        ),
        for (final name in sequences)
          DropdownMenuItem(
            key: ValueKey('quest-card-sequence-item-$name'),
            value: name,
            child: Text(name),
          ),
      ],
      onChanged: _saving
          ? null
          : (value) {
              if (value != null) {
                _save('active_sequence', value);
              }
            },
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        LayoutBuilder(
          builder: (context, constraints) {
            // A grid card is ~360 px; only a genuinely wide surface gets the two-up row.
            if (constraints.maxWidth >= 460) {
              return Row(
                children: [
                  Expanded(child: flowPicker),
                  const SizedBox(width: ShellSpace.sm),
                  Expanded(child: sequencePicker),
                ],
              );
            }
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                flowPicker,
                const SizedBox(height: ShellSpace.xs),
                sequencePicker,
              ],
            );
          },
        ),
        if (_saving)
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text('saving…', style: theme.textTheme.labelSmall),
          ),
        if (_error != null)
          Text(
            _error!,
            key: const ValueKey('quest-card-error'),
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            softWrap: true,
            style: theme.textTheme.labelSmall?.copyWith(
              color: theme.colorScheme.error,
            ),
          ),
      ],
    );
  }
}

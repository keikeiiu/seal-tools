/// Card extras: the small config-driven controls PLAN-V3-SHELL §6 puts ON the cards rather than only
/// in a tab. Nothing here owns a rule: names and values come from the merged config, and a persisted
/// change is a normal `config.validate`/`apply` through the model.
///
/// Buy card — the picker and count are DISPLAY-ONLY. The `buy_sell` block has no `active`-style field
/// (`LocalBuySell` carries presets/geometry only) and `tool.start` carries no run arguments, so the
/// run always uses the configured presets; the card renders the config's items and their per-preset
/// count and says so. That is the reported protocol gap, not a shell decision.
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

/// The Buy card's display-only preset picker and per-run count.
class BuyCardExtras extends StatefulWidget {
  const BuyCardExtras({super.key, required this.model});

  final ShellModel model;

  @override
  State<BuyCardExtras> createState() => _BuyCardExtrasState();
}

class _BuyCardExtrasState extends State<BuyCardExtras> {
  String? _selected;

  Map<String, dynamic> get _presets =>
      asJsonMap(asJsonMap(widget.model.config.config['buy_sell'])['presets']);

  List<String> get _names => _presets.keys.toList()..sort();

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final names = _names;
    final selected = _selected != null && names.contains(_selected)
        ? _selected
        : (names.isEmpty ? null : names.first);
    final count = asNumber(asJsonMap(_presets[selected])['count']) ?? 1;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: 170,
          child: _cardDropdown(
            key: const ValueKey('buy-card-preset'),
            label: 'Buy item',
            value: selected,
            items: [
              for (final name in names)
                DropdownMenuItem(
                  key: ValueKey('buy-card-item-$name'),
                  value: name,
                  child: Text(name),
                ),
            ],
            onChanged: (value) => setState(() => _selected = value),
          ),
        ),
        const SizedBox(height: 2),
        Text(
          names.isEmpty ? 'No buy presets configured' : 'count $count',
          key: const ValueKey('buy-card-count'),
          style: theme.textTheme.bodySmall,
        ),
        Text(
          'Display only — the run uses the config presets',
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

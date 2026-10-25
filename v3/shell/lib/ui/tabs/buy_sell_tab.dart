/// The Buy/Sell tab: the `buy_sell` top-level block's editable half — the named buy presets
/// (item name → scroll/row/count) and the sell cap — plus the calibrated geometry read-only.
///
/// The block mixes both halves: `presets` and `sell_cap` are the player's settings, while
/// `bag_grid`/`bag_slot`/`shop_rows`/`shop_region`/`scroll_point`/`max_button`/`sell_slots` are
/// machine calibration (wave 2). The draft filters to the editable keys, so a save can never blank
/// the geometry the core holds (PROTOCOL §7's field-wise merge keeps it).
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

/// The only fields this tab sends.
const Set<String> _editableBuySellKeys = {'presets', 'sell_cap'};

class BuySellTab extends StatefulWidget {
  const BuySellTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<BuySellTab> createState() => _BuySellTabState();
}

class _BuySellTabState extends State<BuySellTab>
    with ConfigTabState<BuySellTab> {
  @override
  ShellModel get model => widget.model;

  @override
  final ConfigDraft draft = ConfigDraft(
    blockName: 'buy_sell',
    scope: ConfigScope.ownedBlock,
    editableKeys: _editableBuySellKeys,
  );

  final _newPresetName = TextEditingController();
  String? _selectedPreset;

  @override
  void dispose() {
    _newPresetName.dispose();
    super.dispose();
  }

  // ── readers ──────────────────────────────────────────────────────────────────────────────

  Map<String, dynamic> _presets() => asJsonMap(draft.value['presets']);

  List<String> _presetNames() => _presets().keys.toList()..sort();

  String? _currentPreset() {
    final names = _presetNames();
    if (names.isEmpty) {
      return null;
    }
    final selected = _selectedPreset;
    if (selected != null && names.contains(selected)) {
      return selected;
    }
    return names.first;
  }

  Map<String, dynamic> _preset(String name) => asJsonMap(_presets()[name]);

  /// The calibrated half, from the merged mirror's top-level `buy_sell` block.
  Map<String, dynamic> _calibrated() =>
      asJsonMap(model.config.config['buy_sell']);

  // ── writers ──────────────────────────────────────────────────────────────────────────────

  Map<String, dynamic> _ensureMap(Map<String, dynamic> parent, String key) {
    final existing = parent[key];
    if (existing is Map<String, dynamic>) {
      return existing;
    }
    final created = <String, dynamic>{};
    parent[key] = created;
    return created;
  }

  void _setPresetField(String name, String field, num value) =>
      draft.edit((block) {
        _ensureMap(_ensureMap(block, 'presets'), name)[field] = value;
      });

  void _addPreset() {
    final name = _newPresetName.text.trim();
    if (name.isEmpty) {
      return;
    }
    draft.edit((block) {
      _ensureMap(
        _ensureMap(block, 'presets'),
        name,
      ).addAll({'scroll': 0, 'row': 0, 'count': 1});
    });
    _newPresetName.clear();
    setState(() => _selectedPreset = name);
  }

  void _deletePreset(String name) {
    draft.edit((block) {
      asJsonMap(block['presets']).remove(name);
    });
    setState(() => _selectedPreset = null);
  }

  void _setSellCap(num value) => draft.edit((block) {
    block['sell_cap'] = value;
  });

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    final preset = _currentPreset();
    final names = _presetNames();
    final calibrated = _calibrated();
    final values = preset == null ? const <String, dynamic>{} : _preset(preset);

    return buildConfigTab(
      title: 'Buy / Sell',
      description:
          'Buy presets (item name → scroll, row, count) and the sell cap. The calibrated geometry '
          'is read-only until the Buy/Sell calibration surface lands.',
      readOnly: [
        EditorSection(
          title: 'Calibrated geometry — read-only this wave',
          children: [
            ReadOnlyValue(
              label: 'Bag grid [x, y, w, h]',
              value: readOnlyText(calibrated['bag_grid']),
            ),
            ReadOnlyValue(
              label: 'Bag slot [x, y, w, h]',
              value: readOnlyText(calibrated['bag_slot']),
            ),
            ReadOnlyValue(
              label: 'Shop region [x, y, w, h]',
              value: readOnlyText(calibrated['shop_region']),
            ),
            ReadOnlyValue(
              label: 'Shop rows',
              value: readOnlyText(calibrated['shop_rows']),
            ),
            ReadOnlyValue(
              label: 'Focus / scroll point',
              value: readOnlyText(calibrated['scroll_point']),
            ),
            ReadOnlyValue(
              label: 'MAX button',
              value: readOnlyText(calibrated['max_button']),
            ),
            ReadOnlyValue(
              label: 'Sell slots',
              value: readOnlyText(calibrated['sell_slots']),
            ),
          ],
        ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EditorSection(
            title: 'Buy presets',
            children: [
              Row(
                children: [
                  DraftDropdownField(
                    key: const ValueKey('buy-preset-picker'),
                    label: 'Item (preset)',
                    value: preset,
                    options: names,
                    width: 240,
                    onChanged: (value) {
                      if (value != null) {
                        setState(() => _selectedPreset = value);
                      }
                    },
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  OutlinedButton(
                    key: const ValueKey('delete-buy-preset'),
                    onPressed: preset == null
                        ? null
                        : () => _deletePreset(preset),
                    child: const Text('Delete item'),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.sm),
              Row(
                children: [
                  SizedBox(
                    width: 240,
                    child: TextField(
                      key: const ValueKey('new-buy-preset'),
                      controller: _newPresetName,
                      decoration: const InputDecoration(
                        labelText: 'New item name',
                        isDense: true,
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  FilledButton(
                    key: const ValueKey('add-buy-preset'),
                    onPressed: _addPreset,
                    child: const Text('Add item'),
                  ),
                ],
              ),
              if (preset == null)
                const Padding(
                  key: ValueKey('buy-sell-empty'),
                  padding: EdgeInsets.only(top: ShellSpace.sm),
                  child: Text(
                    'No buy presets yet — add the item names the player buys.',
                  ),
                )
              else ...[
                const SizedBox(height: ShellSpace.sm),
                Text(
                  'Preset: $preset',
                  key: const ValueKey('buy-selected-preset'),
                  style: Theme.of(context).textTheme.labelLarge,
                ),
                const SizedBox(height: ShellSpace.xs),
                Wrap(
                  spacing: ShellSpace.md,
                  runSpacing: ShellSpace.sm,
                  children: [
                    DraftFieldSlot(
                      slotKey: 'buy-scroll',
                      syncToken: draft.seedRevision,
                      child: DraftNumberField(
                        label: 'Scroll notches',
                        integer: true,
                        value: asNumber(values['scroll']),
                        width: 160,
                        onChanged: (value) =>
                            _setPresetField(preset, 'scroll', value),
                      ),
                    ),
                    DraftFieldSlot(
                      slotKey: 'buy-row',
                      syncToken: draft.seedRevision,
                      child: DraftNumberField(
                        label: 'Row',
                        integer: true,
                        value: asNumber(values['row']),
                        width: 120,
                        onChanged: (value) =>
                            _setPresetField(preset, 'row', value),
                      ),
                    ),
                    DraftFieldSlot(
                      slotKey: 'buy-count',
                      syncToken: draft.seedRevision,
                      child: DraftNumberField(
                        label: 'Count per run',
                        integer: true,
                        value: asNumber(values['count']),
                        width: 150,
                        onChanged: (value) =>
                            _setPresetField(preset, 'count', value),
                      ),
                    ),
                  ],
                ),
              ],
            ],
          ),
          EditorSection(
            title: 'Sell',
            children: [
              DraftFieldSlot(
                slotKey: 'sell-cap',
                syncToken: draft.seedRevision,
                child: DraftNumberField(
                  label: 'Max slots per run (sell cap)',
                  integer: true,
                  value: asNumber(draft.value['sell_cap']),
                  width: 240,
                  onChanged: _setSellCap,
                ),
              ),
              const SizedBox(height: ShellSpace.xs),
              Text(
                'Selling is the one irreversible action: the cap is the hard per-run ceiling. '
                'The 64-slot grid is calibrated (read-only here) and the run order is the core\'s.',
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

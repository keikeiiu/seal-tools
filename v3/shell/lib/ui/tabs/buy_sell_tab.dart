/// The Buy/Sell tab: the `buy_sell` block's editable half — the named buy presets (item name →
/// scroll/row/count) and the sell cap — plus the calibrated geometry read-only, and the 8×8 bag-slot
/// picker for the NEXT Sell run.
///
/// The block mixes both halves: `presets` and `sell_cap` are the player's settings (the form's Save
/// sends exactly those), while `bag_grid`/`bag_slot`/`shop_rows`/`shop_region`/`scroll_point`/
/// `max_button` are machine calibration (drawn on the Calibrate tab, never sent — PROTOCOL §7's
/// field-wise merge keeps it).
///
/// The slot picker is NOT config: it is the Sell run's per-run selection, held in memory by
/// [ShellModel] and sent as `tool.start`'s `slots` argument (PROTOCOL §12.6). The selection can
/// differ every sell, so it must never be written to `local.yaml` — a saved selection can sell the
/// slots chosen last time, which is data loss rather than convenience.
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

/// The fields the form's Save sends. The sell-slot selection is not here: it is per-run state, not
/// config (see the library doc).
const Set<String> _editableBuySellKeys = {'presets', 'sell_cap'};

/// The bag is an 8×8 grid, row-major: index 0 is the top-left slot, 63 the bottom-right
/// (`BagGrid::SLOT_COUNT`/`centres` in `seal-domain`).
const int _bagColumns = 8;
const int _bagSlots = _bagColumns * _bagColumns;

class BuySellTab extends StatefulWidget {
  const BuySellTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<BuySellTab> createState() => _BuySellTabState();
}

class _BuySellTabState extends State<BuySellTab>
    with ConfigTabState<BuySellTab>, AutomaticKeepAliveClientMixin<BuySellTab> {
  @override
  ShellModel get model => widget.model;

  /// Keep the tab alive when the player switches away: an unsaved draft (and its conflict
  /// banner) must survive the switch. The sell selection is kept by the model, not the tab.
  @override
  bool get wantKeepAlive => true;

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

  /// The calibrated bag rectangle when the core would accept it — `BagGrid::is_valid_rect`'s rule:
  /// `[x, y, w, h]` with a positive width and height. Null means a Sell run refuses, so the grid is
  /// disabled and the player is pointed at the Calibrate tab.
  List<int>? _bagGridRect() {
    final values = asJsonList(_calibrated()['bag_grid']);
    if (values.length != 4) {
      return null;
    }
    final rect = <int>[];
    for (final value in values) {
      final number = asNumber(value);
      if (number == null) {
        return null;
      }
      rect.add(number.round());
    }
    if (rect[2] <= 0 || rect[3] <= 0) {
      return null;
    }
    return rect;
  }

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

  /// Edits the form's sell cap and rebuilds the tab: the status line under the grid quotes the cap,
  /// and content built in this `build()` does not repaint from the draft's own notification alone.
  void _setSellCap(num value) {
    setState(() {
      draft.edit((block) {
        block['sell_cap'] = value;
      });
    });
  }

  /// Toggles one bag cell in the model's in-memory selection. `setState` is what redraws the grid:
  /// the model is not a draft and does not notify (nothing else renders the selection).
  void _toggleSlot(int slot) {
    setState(() => model.toggleSellSlot(slot));
  }

  // ── v2's row shortcuts and bulk actions (the same memory selection) ─────────────────────────
  //
  // The model owns only a per-cell toggle, so the bulk actions are compositions of it in ONE
  // setState: no config command, no per-cell notification storm.

  /// v2 `ToggleSellRow`: selects all 8 cells of [row] when any are off, clears them when all are on.
  void _toggleRow(int row) {
    final cells = [
      for (var column = 0; column < _bagColumns; column++)
        row * _bagColumns + column,
    ];
    final selected = model.sellSlots.toSet();
    final allOn = cells.every(selected.contains);
    setState(() {
      for (final cell in cells) {
        if (allOn || !selected.contains(cell)) {
          model.toggleSellSlot(cell);
        }
      }
    });
  }

  /// v2 "Select all": every bag cell is selected for the next run.
  void _selectAll() {
    final selected = model.sellSlots.toSet();
    setState(() {
      for (var cell = 0; cell < _bagSlots; cell++) {
        if (!selected.contains(cell)) {
          model.toggleSellSlot(cell);
        }
      }
    });
  }

  /// v2 "Clear": nothing is selected for the next run.
  void _clearSelection() {
    setState(() {
      for (final cell in model.sellSlots) {
        model.toggleSellSlot(cell);
      }
    });
  }

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final theme = Theme.of(context);
    final preset = _currentPreset();
    final names = _presetNames();
    final calibrated = _calibrated();
    final values = preset == null ? const <String, dynamic>{} : _preset(preset);

    return buildConfigTab(
      title: 'Buy / Sell',
      description:
          'Buy presets (item name → scroll, row, count), the sell cap, and '
          'which bag slots the next Sell run sells — the selection is kept '
          'for the run and is not saved. The geometry is drawn on the '
          'Calibrate tab and shown read-only here.',
      readOnly: [
        EditorSection(
          title: 'Calibrated geometry — read-only (Calibrate tab)',
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
              const SizedBox(height: ShellSpace.md),
              _sellGrid(theme),
              const SizedBox(height: ShellSpace.xs),
              _selectionStatus(theme),
              const SizedBox(height: ShellSpace.sm),
              Text(
                'This selection is for the next run only — it is not saved.',
                key: const ValueKey('sell-selection-caption'),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: ShellSpace.xs),
              Text(
                'Selling is the one irreversible action: the cap is the hard '
                'per-run ceiling. Start Sell from its card; the core sells the '
                'selected slots highest index first and applies the cap at run '
                'time.',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  /// The 8×8 selector. Every cell is one bag slot; the geometry comes from the calibrated
  /// `bag_grid` exactly as the read-only map did (`w/8, h/8` pitch). Disabled with the core's own
  /// refusal line when the grid is not calibrated, because a Sell run could not run either.
  ///
  /// The bulk actions and the per-row shortcut buttons are v2 parity (`BuildSellTab`'s pick row and
  /// `MakeBagPicker(onRow:)`): they touch only the in-memory selection, never config.
  Widget _sellGrid(ThemeData theme) {
    final rect = _bagGridRect();
    if (rect == null) {
      return Text(
        "The bag grid isn't calibrated — save the Bag grid on the "
        'Calibrate tab.',
        key: const ValueKey('sell-grid-not-calibrated'),
        style: theme.textTheme.bodySmall?.copyWith(
          color: theme.colorScheme.onSurfaceVariant,
        ),
      );
    }
    final selected = model.sellSlots.toSet();
    final pitch =
        ' · bag pitch ${(rect[2] / _bagColumns).toStringAsFixed(1)}×'
        '${(rect[3] / _bagColumns).toStringAsFixed(1)} px';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Select the bag slots to sell$pitch',
          key: const ValueKey('sell-grid-pitch'),
          style: theme.textTheme.labelLarge,
        ),
        const SizedBox(height: ShellSpace.xs),
        Text(
          '${selected.length} of $_bagSlots slots selected',
          key: const ValueKey('sell-grid-count'),
          style: theme.textTheme.bodySmall,
        ),
        const SizedBox(height: ShellSpace.xs),
        Row(
          children: [
            OutlinedButton(
              key: const ValueKey('sell-select-all'),
              onPressed: _selectAll,
              child: const Text('Select all'),
            ),
            const SizedBox(width: ShellSpace.sm),
            OutlinedButton(
              key: const ValueKey('sell-clear'),
              onPressed: _clearSelection,
              child: const Text('Clear'),
            ),
          ],
        ),
        const SizedBox(height: ShellSpace.xs),
        for (var row = 0; row < _bagColumns; row++)
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              for (var column = 0; column < _bagColumns; column++)
                _sellCell(
                  theme,
                  row * _bagColumns + column,
                  selected.contains(row * _bagColumns + column),
                ),
              const SizedBox(width: ShellSpace.sm),
              _rowButton(row),
            ],
          ),
      ],
    );
  }

  /// v2's `row N` shortcut column: one click fills or clears the whole bag row.
  Widget _rowButton(int row) => Tooltip(
    key: ValueKey('sell-row-$row'),
    message: 'Select or clear all 8 slots of bag row ${row + 1}',
    child: TextButton(
      onPressed: () => _toggleRow(row),
      style: TextButton.styleFrom(
        padding: const EdgeInsets.symmetric(horizontal: ShellSpace.xs),
        visualDensity: VisualDensity.compact,
        minimumSize: const Size(0, 20),
        tapTargetSize: MaterialTapTargetSize.shrinkWrap,
      ),
      child: Text('row ${row + 1}', style: const TextStyle(fontSize: 10)),
    ),
  );

  /// v2 `PaintSellSlots`' status line: none / within the cap / over the cap. The over-cap state is
  /// the error colour and says what is true — the run refuses either way (the core enforces the cap
  /// in its plan).
  Widget _selectionStatus(ThemeData theme) {
    final count = model.sellSlots.length;
    final cap = asNumber(draft.value['sell_cap'])?.round();
    final over = cap != null && count > cap;
    final String text;
    if (count == 0) {
      text = 'No slots selected — click the grid to choose what to sell.';
    } else if (cap == null) {
      text = '$count slot(s) selected.';
    } else if (count > cap) {
      text =
          '$count slots selected, which is over the cap of $cap. Raise the '
          'cap on purpose, or narrow the selection — the run refuses either '
          'way.';
    } else {
      text = '$count slot(s) selected (cap $cap).';
    }
    return Text(
      text,
      key: const ValueKey('sell-selection-status'),
      style: theme.textTheme.bodySmall?.copyWith(
        color: over ? theme.colorScheme.error : null,
      ),
    );
  }

  Widget _sellCell(ThemeData theme, int slot, bool selected) {
    return InkWell(
      key: ValueKey('sell-cell-$slot'),
      onTap: () => _toggleSlot(slot),
      child: Tooltip(
        message: 'Bag slot ${slot + 1}',
        child: Container(
          width: 20,
          height: 20,
          margin: const EdgeInsets.all(1),
          decoration: BoxDecoration(
            color: selected
                ? theme.colorScheme.primary
                : theme.colorScheme.surfaceContainerHighest,
            border: Border.all(color: theme.colorScheme.outlineVariant),
          ),
        ),
      ),
    );
  }
}

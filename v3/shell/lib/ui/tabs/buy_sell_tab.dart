/// The Buy/Sell tab: the `buy_sell` top-level block's editable half — the named buy presets
/// (item name → scroll/row/count), the sell cap and the bag-slot selection the Sell card sells —
/// plus the calibrated geometry read-only.
///
/// The block mixes both halves: `presets`, `sell_cap` and `sell_slots` are the player's settings,
/// while `bag_grid`/`bag_slot`/`shop_rows`/`shop_region`/`scroll_point`/`max_button` are machine
/// calibration (drawn on the Calibrate tab). Two drafts keep the two saves independent: the form's
/// Save sends the presets + cap, while Save selection sends EXACTLY
/// `{buy_sell: {sell_slots: [...]}}` — neither can re-send (or blank) the other's fields, and no
/// draft ever sends the geometry (PROTOCOL §7's field-wise merge keeps it).
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

/// The form's Save sends these; the selection has its own draft and its own button.
const Set<String> _editableBuySellKeys = {'presets', 'sell_cap'};

/// The one field the selection's Save sends.
const Set<String> _selectionKeys = {'sell_slots'};

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
  /// banner) must survive the switch.
  @override
  bool get wantKeepAlive => true;

  @override
  final ConfigDraft draft = ConfigDraft(
    blockName: 'buy_sell',
    scope: ConfigScope.ownedBlock,
    editableKeys: _editableBuySellKeys,
  );

  /// The slot selection is a SEPARATE draft with its own Save: the two actions persist different
  /// halves of the block, and a form save must not re-send a stale selection (or vice versa).
  final ConfigDraft _selectionDraft = ConfigDraft(
    blockName: 'buy_sell',
    scope: ConfigScope.ownedBlock,
    editableKeys: _selectionKeys,
  );

  final _newPresetName = TextEditingController();
  String? _selectedPreset;

  bool _selectionSaving = false;
  ConfigSaveOutcome? _selectionOutcome;
  int? _seenSelectionRev;

  @override
  void initState() {
    super.initState();
    // The selection draft is deliberately NOT a part of `draftGroup` (its payload would collide
    // with the form's), so it is seeded and watched here: the tab is kept alive and the core may
    // write `buy_sell` while it is off-screen.
    _selectionDraft.seedFromMirror(model.config.config);
    _seenSelectionRev = model.config.rev;
    model.config.addListener(_onMirrorChanged);
  }

  @override
  void dispose() {
    model.config.removeListener(_onMirrorChanged);
    _selectionDraft.dispose();
    _newPresetName.dispose();
    super.dispose();
  }

  /// A core-side change while the tab is alive. The selection owns ONE field, so only a change to
  /// that field is an external edit of the selection: a rev bump from another block — or the form's
  /// own save, whose echo also names `buy_sell` — must not raise the banner.
  void _onMirrorChanged() {
    final rev = model.config.rev;
    if (_seenSelectionRev == rev) {
      return;
    }
    _seenSelectionRev = rev;
    if (!_selectionDraft.isDirty) {
      _selectionDraft.seedFromMirror(model.config.config, force: true);
      return;
    }
    final incoming = _calibrated()['sell_slots'];
    if (!jsonEquals(incoming, _selectionDraft.baseline['sell_slots'])) {
      _selectionDraft.markExternalChange();
    }
  }

  // ── the shared scaffold's draft lifecycle ─────────────────────────────────────────────────

  /// Cancel is the tab's "discard my edits": a pending selection edit goes too, next to the form's
  /// own draft.
  @override
  void cancel() {
    super.cancel();
    setState(() {
      _selectionDraft.reset();
      _selectionOutcome = null;
    });
  }

  /// The explicit reconcile after a conflict: the reloaded config is the truth for both drafts.
  @override
  Future<void> reconcile() async {
    await super.reconcile();
    if (!mounted) {
      return;
    }
    setState(() {
      _selectionDraft.seedFromMirror(model.config.config, force: true);
      _selectionOutcome = null;
    });
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

  /// The selected bag cells, ascending and filtered to real cells — the same filter
  /// `sell_slots_to_sell` applies before selling.
  List<int> _selectedSlots() {
    final slots = <int>{};
    for (final value in asJsonList(_selectionDraft.value['sell_slots'])) {
      final number = asNumber(value);
      final slot = number?.round();
      if (slot != null && slot >= 0 && slot < _bagSlots) {
        slots.add(slot);
      }
    }
    return slots.toList()..sort();
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

  void _setSellCap(num value) => draft.edit((block) {
    block['sell_cap'] = value;
  });

  /// Toggles one bag cell in the selection draft. The list is kept ascending so the save payload is
  /// deterministic; the core re-sorts highest-index-first at run time, file order is not data.
  void _toggleSlot(int slot) {
    final selected = _selectedSlots();
    if (!selected.remove(slot)) {
      selected.add(slot);
    }
    selected.sort();
    _selectionDraft.edit((block) {
      // Removing the key means "not stated", like the core's own optional fields: a grid that was
      // toggled and toggled back must not turn into an unsaved change that writes `[]`.
      if (selected.isEmpty &&
          !_selectionDraft.baseline.containsKey('sell_slots')) {
        block.remove('sell_slots');
      } else {
        block['sell_slots'] = selected;
      }
    });
  }

  Future<void> _saveSelection() async {
    if (_selectionSaving) {
      return;
    }
    final sections = _selectionDraft.payload();
    setState(() {
      _selectionSaving = true;
      _selectionOutcome = null;
    });
    final outcome = await model.saveConfig(sections);
    if (!mounted) {
      return;
    }
    setState(() {
      _selectionSaving = false;
      _selectionOutcome = outcome;
      // Only a WRITE rebases the draft. A refusal (validation line, conflict, protocol error) leaves
      // the selection exactly as the player left it: dirty, and saveable again.
      if (!outcome.saved) {
        return;
      }
      final echoed = outcome.result?.sections['buy_sell'];
      // The apply echo is the EFFECTIVE block (PROTOCOL §7), so it is the new truth for the
      // selection. An odd echo keeps what was sent, the draft's documented fallback.
      _selectionDraft.saved(echoed is Map<String, dynamic> ? echoed : null);
      // The selection save bumped the session rev, which the shared scaffold reads as "the core
      // changed this block while you were editing it". That is our own write: if the player has
      // unsaved form edits, re-baseline the form from the same echo and put the edits back, so the
      // false conflict banner never appears and no unsaved preset/cap edit is lost.
      if (echoed is Map<String, dynamic> && draft.hasExternalChange) {
        final unsaved = draft.value;
        draft.saved(echoed);
        draft.setValue(unsaved);
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
          'which bag slots the Sell card sells. The geometry is drawn on the '
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
              _selectionResult(theme),
              const SizedBox(height: ShellSpace.xs),
              Text(
                'Selling is the one irreversible action: the cap is the hard '
                'per-run ceiling. The core sells the selected slots highest '
                'index first and applies the cap at run time. The selection is '
                'saved on this machine with its own button; the cap rides the '
                'form\'s Save.',
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
  Widget _sellGrid(ThemeData theme) => ListenableBuilder(
    listenable: _selectionDraft,
    builder: (context, _) {
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
      final selected = _selectedSlots().toSet();
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
              ],
            ),
          const SizedBox(height: ShellSpace.sm),
          Row(
            children: [
              FilledButton(
                key: const ValueKey('save-sell-selection'),
                onPressed: _selectionDraft.isDirty && !_selectionSaving
                    ? _saveSelection
                    : null,
                child: _selectionSaving
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Text('Save selection'),
              ),
              if (_selectionDraft.isDirty)
                Padding(
                  padding: const EdgeInsets.only(left: ShellSpace.md),
                  child: Text(
                    'Unsaved selection',
                    key: const ValueKey('sell-selection-dirty'),
                    style: theme.textTheme.labelMedium?.copyWith(
                      color: theme.colorScheme.tertiary,
                    ),
                  ),
                ),
              if (_selectionDraft.isDirty)
                TextButton(
                  key: const ValueKey('discard-sell-selection'),
                  onPressed: () => setState(() {
                    _selectionDraft.reset();
                    _selectionOutcome = null;
                  }),
                  child: const Text('Discard selection'),
                ),
            ],
          ),
          if (_selectionDraft.hasExternalChange)
            Padding(
              padding: const EdgeInsets.only(top: ShellSpace.xs),
              child: Text(
                'The core changed this block while you were editing the '
                'selection.',
                key: const ValueKey('sell-selection-external-change'),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),
            ),
        ],
      );
    },
  );

  Widget _sellCell(ThemeData theme, int slot, bool selected) {
    return InkWell(
      key: ValueKey('sell-cell-$slot'),
      onTap: () => _toggleSlot(slot),
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
    );
  }

  /// The selection's own outcome: the core's lines verbatim, or the rev that was written.
  Widget _selectionResult(ThemeData theme) {
    final outcome = _selectionOutcome;
    if (outcome == null) {
      return const SizedBox.shrink();
    }
    if (outcome.saved) {
      return Padding(
        padding: const EdgeInsets.only(top: ShellSpace.xs),
        child: Text(
          'Selection saved — rev ${outcome.result?.rev ?? '?'}',
          key: const ValueKey('sell-selection-ok'),
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.primary,
          ),
        ),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        for (var index = 0; index < outcome.errorLines.length; index++)
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              outcome.errorLines[index],
              key: ValueKey('sell-selection-error-$index'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.error,
              ),
            ),
          ),
      ],
    );
  }
}

/// The Spammer tab: the `spammer` top-level block (PROTOCOL §9's tool id, CONFIG §3's owned block).
///
/// A preset is a key map (key → seconds) plus its ORDERED priority list and its opening
/// (`startup.lead_in/step`). The tab edits only those fields; `combos` is preserved untouched (the
/// draft filters to the editable keys, so the core's field-wise merge keeps it).
///
/// The widget keeps no rule: blank names are refused by the input (a name cannot be typed as
/// "nothing"), and everything the core decides — validation lines, rev/conflict — comes back through
/// the shared Save frame.
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../components/empty_state.dart';
import '../components/shell_prose.dart';
import '../theme.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

class SpammerTab extends StatefulWidget {
  const SpammerTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<SpammerTab> createState() => _SpammerTabState();
}

class _SpammerTabState extends State<SpammerTab>
    with ConfigTabState<SpammerTab>, AutomaticKeepAliveClientMixin<SpammerTab> {
  @override
  ShellModel get model => widget.model;

  /// Keep the tab alive when the player switches away: an unsaved draft (and its conflict
  /// banner) must survive the switch.
  @override
  bool get wantKeepAlive => true;

  @override
  final ConfigDraft draft = ConfigDraft(
    blockName: 'spammer',
    scope: ConfigScope.ownedBlock,
    editableKeys: const {'active', 'presets', 'priority', 'startup', 'combos'},
  );

  final _newPresetName = TextEditingController();
  final _renamePresetName = TextEditingController();
  final _newKeyName = TextEditingController();

  /// The empty-state actions point at these fields: the next action for a missing preset/key is to
  /// name it, and the name field lives beside the empty state.
  final _newPresetFocus = FocusNode();
  final _newKeyFocus = FocusNode();

  String? _editingPreset;
  String? _presetNote;

  @override
  void dispose() {
    _newPresetName.dispose();
    _renamePresetName.dispose();
    _newKeyName.dispose();
    _newPresetFocus.dispose();
    _newKeyFocus.dispose();
    super.dispose();
  }

  // ── draft readers ─────────────────────────────────────────────────────────────────────────

  Map<String, dynamic> _block(String key) => asJsonMap(draft.value[key]);

  List<String> _presetNames() {
    final names = <String>{
      ..._block('presets').keys,
      ..._block('priority').keys,
      ..._block('startup').keys,
      ..._block('combos').keys,
    };
    return names.toList()..sort();
  }

  String? _activePreset() {
    final active = asString(draft.value['active']);
    return active == null || active.isEmpty ? null : active;
  }

  String? _currentPreset() {
    final names = _presetNames();
    if (names.isEmpty) {
      return null;
    }
    final editing = _editingPreset;
    if (editing != null && names.contains(editing)) {
      return editing;
    }
    final active = _activePreset();
    if (active != null && names.contains(active)) {
      return active;
    }
    return names.first;
  }

  List<String> _keysFor(String preset) {
    final priority = _block('priority');
    final listed = asStringList(priority[preset]);
    if (listed.isNotEmpty) {
      return listed;
    }
    return _secondsKeys(preset);
  }

  List<String> _secondsKeys(String preset) {
    final perPreset = _block('presets')[preset];
    if (perPreset is! Map<String, dynamic>) {
      return const [];
    }
    return perPreset.keys.toList()..sort();
  }

  num? _secondsFor(String preset, String key) {
    final perPreset = _block('presets')[preset];
    return asNumber(asJsonMap(perPreset)[key]);
  }

  num _opening(String preset, String field) {
    final startup = _block('startup')[preset];
    return asNumber(asJsonMap(startup)[field]) ?? 0;
  }

  // ── draft writers ─────────────────────────────────────────────────────────────────────────

  Map<String, dynamic> _ensureMap(Map<String, dynamic> parent, String key) {
    final existing = parent[key];
    if (existing is Map<String, dynamic>) {
      return existing;
    }
    final created = <String, dynamic>{};
    parent[key] = created;
    return created;
  }

  List<dynamic> _ensureList(Map<String, dynamic> parent, String key) {
    final existing = parent[key];
    if (existing is List) {
      return existing;
    }
    final created = <dynamic>[];
    parent[key] = created;
    return created;
  }

  void _setActive(String? name) {
    draft.edit((value) {
      if (name == null) {
        value.remove('active');
      } else {
        value['active'] = name;
      }
    });
  }

  /// The preset-name keys in every map that carries them; their VALUES reference inner keys, never the
  /// preset name, so a rename moves only the keys and leaves the content untouched.
  static const _presetMaps = ['presets', 'priority', 'combos', 'startup'];

  /// Rename [from] to [to] in all four name-keyed maps plus `active`, in ONE draft edit. The two
  /// refusals are UI guards for JSON-map invariants (no empty, no duplicate key), not game rules.
  void _renamePreset() {
    final from = _currentPreset();
    final to = _renamePresetName.text.trim();
    if (from == null) {
      return;
    }
    if (to.isEmpty) {
      setState(() => _presetNote = 'a preset name cannot be empty');
      return;
    }
    if (_presetNames().contains(to)) {
      setState(() => _presetNote = 'a preset named "$to" already exists');
      return;
    }
    draft.edit((block) {
      for (final mapKey in _presetMaps) {
        final map = asJsonMap(block[mapKey]);
        if (!map.containsKey(from)) {
          continue;
        }
        final value = map.remove(from);
        map[to] = value;
        block[mapKey] = map;
      }
      if (asString(block['active']) == from) {
        block['active'] = to;
      }
    });
    setState(() {
      _editingPreset = to;
      _renamePresetName.clear();
      _presetNote = null;
    });
  }

  /// The original name behind a copy: strips a trailing ` copy` or ` copy N`, so duplicating a copy
  /// creates the next free `<base> copy N` rather than `<copy> copy`.
  String _copyBase(String name) {
    final match = RegExp(r'^(.*?) copy(?: \d+)?$').firstMatch(name);
    return match == null ? name : (match.group(1) ?? name);
  }

  /// Deep-copy [from] under a unique `<name> copy` / `<name> copy 2` name in all four maps. `active`
  /// is not touched: a duplicate is a new preset, not a new selection.
  void _duplicatePreset() {
    final from = _currentPreset();
    if (from == null) {
      return;
    }
    final names = _presetNames().toSet();
    final base = _copyBase(from);
    var to = '$base copy';
    var suffix = 2;
    while (names.contains(to)) {
      to = '$base copy $suffix';
      suffix += 1;
    }
    draft.edit((block) {
      for (final mapKey in _presetMaps) {
        final map = asJsonMap(block[mapKey]);
        final value = map[from];
        if (value == null) {
          continue;
        }
        map[to] = cloneJson(value);
        block[mapKey] = map;
      }
    });
    setState(() {
      _editingPreset = to;
      _presetNote = null;
    });
  }

  void _addPreset() {
    final name = _newPresetName.text.trim();
    if (name.isEmpty) {
      return;
    }
    draft.edit((value) {
      _ensureMap(value, 'presets');
      final priority = _ensureMap(value, 'priority');
      final startup = _ensureMap(value, 'startup');
      priority.putIfAbsent(name, () => <dynamic>[]);
      startup.putIfAbsent(
        name,
        () => <String, dynamic>{'lead_in': 0.0, 'step': 0.0},
      );
      final active = value['active'];
      if (active is! String || active.isEmpty) {
        value['active'] = name;
      }
    });
    _newPresetName.clear();
    setState(() => _editingPreset = name);
  }

  void _deletePreset(String name) {
    // v2's guard (its preset ConfirmAndDelete): a spammer config with no preset has no rotation a
    // run could press, so the last one cannot be deleted. Same refusal line; the draft is untouched.
    if (_block('presets').length <= 1) {
      setState(() => _presetNote = 'The last preset cannot be deleted.');
      return;
    }
    draft.edit((value) {
      asJsonMap(value['presets']).remove(name);
      asJsonMap(value['priority']).remove(name);
      asJsonMap(value['startup']).remove(name);
      asJsonMap(value['combos']).remove(name);
      if (value['active'] == name) {
        final remaining = _presetNames()..remove(name);
        value['active'] = remaining.isEmpty ? '' : remaining.first;
      }
    });
    setState(() {
      _editingPreset = null;
      _presetNote = null;
    });
  }

  void _addKey(String preset) {
    final key = _newKeyName.text.trim();
    if (key.isEmpty) {
      return;
    }
    draft.edit((value) {
      final keys = _ensureList(_ensureMap(value, 'priority'), preset);
      if (!keys.contains(key)) {
        keys.add(key);
      }
      _ensureMap(
        _ensureMap(value, 'presets'),
        preset,
      ).putIfAbsent(key, () => 1);
    });
    _newKeyName.clear();
  }

  void _removeKey(String preset, String key) {
    draft.edit((value) {
      _ensureList(_ensureMap(value, 'priority'), preset).remove(key);
      _ensureMap(_ensureMap(value, 'presets'), preset).remove(key);
    });
  }

  void _moveKey(String preset, int index, int delta) {
    draft.edit((value) {
      final keys = _ensureList(_ensureMap(value, 'priority'), preset);
      final target = index + delta;
      if (target < 0 || target >= keys.length) {
        return;
      }
      final item = keys.removeAt(index);
      keys.insert(target, item);
    });
  }

  void _renameKey(String preset, String oldKey, String newKey) {
    final renamed = newKey.trim();
    if (renamed.isEmpty || renamed == oldKey) {
      return;
    }
    draft.edit((value) {
      final priority = _ensureList(_ensureMap(value, 'priority'), preset);
      final index = priority.indexOf(oldKey);
      if (index >= 0) {
        priority[index] = renamed;
      }
      final seconds = _ensureMap(_ensureMap(value, 'presets'), preset);
      if (seconds.containsKey(oldKey)) {
        final existing = seconds.remove(oldKey);
        seconds[renamed] = existing;
      }
    });
  }

  void _setSeconds(String preset, String key, num seconds) {
    draft.edit((value) {
      _ensureMap(_ensureMap(value, 'presets'), preset)[key] = seconds;
    });
  }

  void _setOpening(String preset, String field, num seconds) {
    draft.edit((value) {
      _ensureMap(_ensureMap(value, 'startup'), preset)[field] = seconds;
    });
  }

  // ── combos ──────────────────────────────────────────────────────────────────────────────────

  List<dynamic> _combosFor(String preset) =>
      asJsonList(_block('combos')[preset]);

  /// The shell-side gate: every combo needs at least two keys before Save may run. Everything else
  /// about a key (its spelling, whether the core accepts it) is the core's validation line.
  String? _comboProblem() {
    for (final entry in _block('combos').entries) {
      final list = asJsonList(entry.value);
      for (var index = 0; index < list.length; index++) {
        final keys = asJsonList(asJsonMap(list[index])['keys']);
        if (keys.length < 2) {
          return 'combo ${index + 1} in "${entry.key}" needs at least two keys';
        }
      }
    }
    return null;
  }

  @override
  Future<void> save() async {
    if (_comboProblem() != null) {
      setState(() {}); // render the note; nothing is sent
      return;
    }
    await super.save();
  }

  /// Combo edits call this first: the conditional combos note is rendered OUTSIDE the draft
  /// ListenableBuilder, so the tab itself must rebuild to show/hide it (the fields' own draft listeners
  /// are not enough for that line).
  void _combosChanged() => setState(() {});

  void _addCombo(String preset) {
    _combosChanged();
    draft.edit((value) {
      _ensureList(
        _ensureMap(value, 'combos'),
        preset,
      ).add(<String, dynamic>{'keys': <dynamic>[], 'gap': 0.1});
    });
  }

  void _removeCombo(String preset, int index) {
    _combosChanged();
    draft.edit((value) {
      final combos = _ensureList(_ensureMap(value, 'combos'), preset);
      if (index >= 0 && index < combos.length) {
        combos.removeAt(index);
      }
    });
  }

  List<dynamic> _comboKeys(
    Map<String, dynamic> value,
    String preset,
    int index,
  ) {
    final combos = _ensureList(_ensureMap(value, 'combos'), preset);
    final combo = asJsonMap(combos[index]);
    final keys = asJsonList(combo['keys']).toList();
    combo['keys'] = keys;
    return keys;
  }

  void _addComboKey(String preset, int index) {
    _combosChanged();
    draft.edit((value) => _comboKeys(value, preset, index).add(''));
  }

  void _removeComboKey(String preset, int index, int keyIndex) {
    _combosChanged();
    draft.edit((value) {
      final keys = _comboKeys(value, preset, index);
      if (keyIndex >= 0 && keyIndex < keys.length) {
        keys.removeAt(keyIndex);
      }
    });
  }

  void _moveComboKey(String preset, int index, int keyIndex, int delta) {
    _combosChanged();
    draft.edit((value) {
      final keys = _comboKeys(value, preset, index);
      final target = keyIndex + delta;
      if (target < 0 || target >= keys.length) {
        return;
      }
      final item = keys.removeAt(keyIndex);
      keys.insert(target, item);
    });
  }

  void _setComboKey(String preset, int index, int keyIndex, String key) {
    _combosChanged();
    draft.edit((value) {
      final keys = _comboKeys(value, preset, index);
      if (keyIndex >= 0 && keyIndex < keys.length) {
        keys[keyIndex] = key;
      }
    });
  }

  void _setComboGap(String preset, int index, num gap) {
    _combosChanged();
    draft.edit((value) {
      final combos = _ensureList(_ensureMap(value, 'combos'), preset);
      asJsonMap(combos[index])['gap'] = gap;
    });
  }

  Widget _comboEditor(
    ThemeData theme,
    String preset,
    int comboIndex,
    dynamic combo,
  ) {
    final keys = asJsonList(asJsonMap(combo)['keys']);
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.md),
      child: Column(
        key: ValueKey('combo-$comboIndex'),
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(
                'Combo ${comboIndex + 1}',
                style: theme.textTheme.labelLarge,
              ),
              const SizedBox(width: ShellSpace.md),
              DraftFieldSlot(
                slotKey: 'combo-$comboIndex-gap',
                syncToken: draft.seedRevision,
                child: DraftNumberField(
                  label: 'Gap (s)',
                  value: asNumber(asJsonMap(combo)['gap']),
                  width: ShellWidth.fieldSm,
                  onChanged: (value) => _setComboGap(preset, comboIndex, value),
                ),
              ),
              const SizedBox(width: ShellSpace.sm),
              IconButton(
                key: ValueKey('combo-$comboIndex-remove'),
                tooltip: 'Remove combo',
                onPressed: () => _removeCombo(preset, comboIndex),
                icon: const Icon(Icons.delete_outline),
              ),
            ],
          ),
          for (var keyIndex = 0; keyIndex < keys.length; keyIndex++)
            Padding(
              padding: const EdgeInsets.only(top: ShellSpace.xs),
              child: Row(
                children: [
                  DraftFieldSlot(
                    slotKey: 'combo-$comboIndex-key-$keyIndex',
                    syncToken: draft.seedRevision,
                    child: DraftTextField(
                      label: 'Key ${keyIndex + 1}',
                      value: asString(keys[keyIndex]) ?? '',
                      width: ShellWidth.fieldMd,
                      onChanged: (value) =>
                          _setComboKey(preset, comboIndex, keyIndex, value),
                    ),
                  ),
                  IconButton(
                    key: ValueKey('combo-$comboIndex-key-$keyIndex-up'),
                    tooltip: 'Move up',
                    onPressed: keyIndex == 0
                        ? null
                        : () => _moveComboKey(preset, comboIndex, keyIndex, -1),
                    icon: const Icon(
                      Icons.arrow_upward,
                      size: ShellSize.iconMd,
                    ),
                  ),
                  IconButton(
                    key: ValueKey('combo-$comboIndex-key-$keyIndex-down'),
                    tooltip: 'Move down',
                    onPressed: keyIndex == keys.length - 1
                        ? null
                        : () => _moveComboKey(preset, comboIndex, keyIndex, 1),
                    icon: const Icon(
                      Icons.arrow_downward,
                      size: ShellSize.iconMd,
                    ),
                  ),
                  IconButton(
                    key: ValueKey('combo-$comboIndex-key-$keyIndex-remove'),
                    tooltip: 'Remove key',
                    onPressed: () =>
                        _removeComboKey(preset, comboIndex, keyIndex),
                    icon: const Icon(Icons.close, size: ShellSize.iconMd),
                  ),
                ],
              ),
            ),
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: TextButton.icon(
              key: ValueKey('combo-$comboIndex-add-key'),
              onPressed: () => _addComboKey(preset, comboIndex),
              icon: const Icon(Icons.add, size: ShellSize.iconSm),
              label: const Text('Add combo key'),
            ),
          ),
        ],
      ),
    );
  }

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  /// A UI-owned refusal/notice line (the core's own lines stay in the shared Save frame).
  Widget _noteLine(ThemeData theme, String text, Key key) => Padding(
    padding: const EdgeInsets.only(top: ShellSpace.xs),
    child: ShellProse(
      text,
      key: key,
      style: theme.textTheme.bodyMedium?.copyWith(
        color: theme.colorScheme.error,
      ),
    ),
  );

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final preset = _currentPreset();
    final names = _presetNames();
    final keys = preset == null ? const <String>[] : _keysFor(preset);
    final combos = preset == null ? const <dynamic>[] : _combosFor(preset);
    final theme = Theme.of(context);
    final comboProblem = _comboProblem();

    return buildConfigTab(
      title: 'Skill Spammer',
      description:
          'Presets of key rotations. The tab sends only the spammer block; the core merges it '
          'field-wise and is the only writer.',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EditorSection(
            title: 'Presets',
            // The row actions only apply to a selected preset, so they are absent (not disabled)
            // while the block has none.
            trailing: preset == null
                ? null
                : Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      OutlinedButton(
                        key: const ValueKey('duplicate-preset'),
                        onPressed: _duplicatePreset,
                        child: const Text('Duplicate preset'),
                      ),
                      const SizedBox(width: ShellSpace.sm),
                      // Destructive: outlined, never a filled primary.
                      OutlinedButton(
                        key: const ValueKey('delete-preset'),
                        style: OutlinedButton.styleFrom(
                          foregroundColor: theme.colorScheme.error,
                        ),
                        onPressed: () => _deletePreset(preset),
                        child: const Text('Delete preset'),
                      ),
                    ],
                  ),
            children: [
              if (preset != null)
                Wrap(
                  spacing: ShellSpace.md,
                  runSpacing: ShellSpace.sm,
                  children: [
                    DraftDropdownField(
                      label: 'Editing preset',
                      value: preset,
                      options: names,
                      width: ShellWidth.picker,
                      onChanged: (value) {
                        if (value != null) {
                          setState(() => _editingPreset = value);
                        }
                      },
                    ),
                    DraftDropdownField(
                      key: const ValueKey('active-preset'),
                      label: 'Active preset (runs)',
                      value: _activePreset(),
                      options: names,
                      width: ShellWidth.picker,
                      onChanged: _setActive,
                    ),
                  ],
                )
              else
                EmptyState(
                  key: const ValueKey('spammer-empty'),
                  icon: Icons.queue_music,
                  title: 'No presets yet',
                  hint:
                      'Name a preset below and add it, then give it the keys it rotates '
                      'in priority order.',
                  action: TextButton.icon(
                    key: const ValueKey('spammer-empty-add'),
                    onPressed: _newPresetFocus.requestFocus,
                    icon: const Icon(Icons.add, size: ShellSize.iconSm),
                    label: const Text('Add a preset'),
                  ),
                ),
              const SizedBox(height: ShellSpace.md),
              Wrap(
                spacing: ShellSpace.sm,
                runSpacing: ShellSpace.sm,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  SizedBox(
                    width: ShellWidth.picker,
                    child: TextField(
                      key: const ValueKey('new-preset-name'),
                      controller: _newPresetName,
                      focusNode: _newPresetFocus,
                      decoration: const InputDecoration(
                        labelText: 'New preset name',
                        isDense: true,
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                  FilledButton.tonal(
                    key: const ValueKey('add-preset'),
                    onPressed: _addPreset,
                    child: const Text('Add preset'),
                  ),
                ],
              ),
              if (preset != null) ...[
                const SizedBox(height: ShellSpace.sm),
                Wrap(
                  spacing: ShellSpace.sm,
                  runSpacing: ShellSpace.sm,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    SizedBox(
                      width: ShellWidth.picker,
                      child: TextField(
                        key: const ValueKey('rename-preset-name'),
                        controller: _renamePresetName,
                        decoration: const InputDecoration(
                          labelText: 'Rename preset to',
                          isDense: true,
                          border: OutlineInputBorder(),
                        ),
                      ),
                    ),
                    FilledButton.tonal(
                      key: const ValueKey('rename-preset'),
                      onPressed: _renamePreset,
                      child: const Text('Rename preset'),
                    ),
                  ],
                ),
              ],
              if (_presetNote != null)
                _noteLine(
                  theme,
                  _presetNote!,
                  const ValueKey('spammer-preset-note'),
                ),
            ],
          ),
          if (preset != null) ...[
            EditorSection(
              title: 'Keys & cooldowns',
              children: [
                const ShellProse(
                  'The opening plays before the rotation. Then each key fires in priority order, '
                  'waiting its own cooldown (Seconds). A leading * is the fast tap.',
                ),
                const SizedBox(height: ShellSpace.md),
                Text('Opening — $preset', style: theme.textTheme.labelLarge),
                const SizedBox(height: ShellSpace.xs),
                Wrap(
                  spacing: ShellSpace.md,
                  runSpacing: ShellSpace.sm,
                  children: [
                    DraftFieldSlot(
                      slotKey: 'lead-in',
                      syncToken: draft.seedRevision,
                      child: DraftNumberField(
                        label: 'Lead-in (s)',
                        value: _opening(preset, 'lead_in'),
                        width: ShellWidth.labelColumn,
                        onChanged: (value) =>
                            _setOpening(preset, 'lead_in', value),
                      ),
                    ),
                    DraftFieldSlot(
                      slotKey: 'startup-step',
                      syncToken: draft.seedRevision,
                      child: DraftNumberField(
                        label: 'Opening step (s)',
                        value: _opening(preset, 'step'),
                        width: ShellWidth.labelColumn,
                        onChanged: (value) =>
                            _setOpening(preset, 'step', value),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: ShellSpace.md),
                Text(
                  'Keys — $preset (priority order)',
                  style: theme.textTheme.labelLarge,
                ),
                const SizedBox(height: ShellSpace.xs),
                if (keys.isEmpty)
                  EmptyState(
                    key: const ValueKey('spammer-no-keys'),
                    icon: Icons.keyboard_outlined,
                    title: 'No keys yet',
                    hint: 'Add the keys this preset rotates, in the order they should fire.',
                    compact: true,
                    action: TextButton.icon(
                      key: const ValueKey('spammer-empty-add-key'),
                      onPressed: _newKeyFocus.requestFocus,
                      icon: const Icon(Icons.add, size: ShellSize.iconSm),
                      label: const Text('Add a key'),
                    ),
                  ),
                for (var index = 0; index < keys.length; index++)
                  Padding(
                    padding: const EdgeInsets.only(bottom: ShellSpace.xs),
                    child: Wrap(
                      spacing: ShellSpace.sm,
                      runSpacing: ShellSpace.xs,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: [
                        DraftFieldSlot(
                          slotKey: 'key-name-$index',
                          syncToken: draft.seedRevision,
                          child: DraftTextField(
                            label: 'Key ${index + 1}',
                            value: keys[index],
                            width: ShellWidth.fieldMd,
                            onChanged: (value) =>
                                _renameKey(preset, keys[index], value),
                          ),
                        ),
                        DraftFieldSlot(
                          slotKey: 'key-seconds-$index',
                          syncToken: draft.seedRevision,
                          child: DraftNumberField(
                            label: 'Seconds',
                            value: _secondsFor(preset, keys[index]),
                            width: ShellWidth.fieldSm,
                            onChanged: (value) =>
                                _setSeconds(preset, keys[index], value),
                          ),
                        ),
                        IconButton(
                          key: ValueKey('key-up-$index'),
                          tooltip: 'Move up',
                          onPressed: index == 0
                              ? null
                              : () => _moveKey(preset, index, -1),
                          icon: const Icon(
                            Icons.arrow_upward,
                            size: ShellSize.iconMd,
                          ),
                        ),
                        IconButton(
                          key: ValueKey('key-down-$index'),
                          tooltip: 'Move down',
                          onPressed: index == keys.length - 1
                              ? null
                              : () => _moveKey(preset, index, 1),
                          icon: const Icon(
                            Icons.arrow_downward,
                            size: ShellSize.iconMd,
                          ),
                        ),
                        IconButton(
                          key: ValueKey('key-remove-$index'),
                          tooltip: 'Remove',
                          onPressed: () => _removeKey(preset, keys[index]),
                          icon: const Icon(Icons.close, size: ShellSize.iconMd),
                        ),
                      ],
                    ),
                  ),
                const SizedBox(height: ShellSpace.sm),
                Wrap(
                  spacing: ShellSpace.sm,
                  runSpacing: ShellSpace.sm,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    SizedBox(
                      width: ShellWidth.fieldMd,
                      child: TextField(
                        key: const ValueKey('new-key-name'),
                        controller: _newKeyName,
                        focusNode: _newKeyFocus,
                        decoration: const InputDecoration(
                          labelText: 'New key',
                          isDense: true,
                          border: OutlineInputBorder(),
                        ),
                      ),
                    ),
                    TextButton.icon(
                      key: const ValueKey('add-key'),
                      onPressed: () => _addKey(preset),
                      icon: const Icon(Icons.add, size: ShellSize.iconSm),
                      label: const Text('Add key'),
                    ),
                  ],
                ),
              ],
            ),
            EditorSection(
              title: 'Rotation — combos',
              children: [
                const ShellProse(
                  'Combos send several keys together and must go out atomically; Gap (s) is the '
                  'pause between their steps. A combo needs at least two keys.',
                ),
                const SizedBox(height: ShellSpace.md),
                for (
                  var comboIndex = 0;
                  comboIndex < combos.length;
                  comboIndex++
                )
                  _comboEditor(theme, preset, comboIndex, combos[comboIndex]),
                Row(
                  children: [
                    FilledButton.tonal(
                      key: const ValueKey('add-combo'),
                      onPressed: () => _addCombo(preset),
                      child: const Text('Add combo'),
                    ),
                  ],
                ),
                if (comboProblem != null)
                  _noteLine(
                    theme,
                    comboProblem,
                    const ValueKey('spammer-combo-note'),
                  ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

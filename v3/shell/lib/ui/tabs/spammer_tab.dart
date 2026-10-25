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
    with ConfigTabState<SpammerTab> {
  @override
  ShellModel get model => widget.model;

  @override
  final ConfigDraft draft = ConfigDraft(
    blockName: 'spammer',
    scope: ConfigScope.ownedBlock,
    editableKeys: const {'active', 'presets', 'priority', 'startup'},
  );

  final _newPresetName = TextEditingController();
  final _newKeyName = TextEditingController();
  String? _editingPreset;

  @override
  void dispose() {
    _newPresetName.dispose();
    _newKeyName.dispose();
    super.dispose();
  }

  // ── draft readers ─────────────────────────────────────────────────────────────────────────

  Map<String, dynamic> _block(String key) => asJsonMap(draft.value[key]);

  List<String> _presetNames() {
    final names = <String>{
      ..._block('presets').keys,
      ..._block('priority').keys,
      ..._block('startup').keys,
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
    draft.edit((value) {
      asJsonMap(value['presets']).remove(name);
      asJsonMap(value['priority']).remove(name);
      asJsonMap(value['startup']).remove(name);
      if (value['active'] == name) {
        final remaining = _presetNames()..remove(name);
        value['active'] = remaining.isEmpty ? '' : remaining.first;
      }
    });
    setState(() => _editingPreset = null);
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

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    final preset = _currentPreset();
    final names = _presetNames();
    final keys = preset == null ? const <String>[] : _keysFor(preset);

    return buildConfigTab(
      title: 'Skill Spammer',
      description:
          'Presets of key rotations. The tab sends only the spammer block; the core merges it '
          'field-wise and is the only writer.',
      readOnly: [
        ReadOnlyValue(
          label: 'Combos',
          value: preset == null
              ? '—'
              : 'preserved (${asJsonList(_block('combos')[preset]).length} entries; '
                    'editing them is a later wave)',
        ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EditorSection(
            title: 'Presets',
            children: [
              Row(
                children: [
                  DraftDropdownField(
                    label: 'Editing preset',
                    value: preset,
                    options: names,
                    width: 260,
                    onChanged: (value) {
                      if (value != null) {
                        setState(() => _editingPreset = value);
                      }
                    },
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  OutlinedButton(
                    key: const ValueKey('delete-preset'),
                    onPressed: preset == null
                        ? null
                        : () => _deletePreset(preset),
                    child: const Text('Delete preset'),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.sm),
              Row(
                children: [
                  SizedBox(
                    width: 260,
                    child: TextField(
                      key: const ValueKey('new-preset-name'),
                      controller: _newPresetName,
                      decoration: const InputDecoration(
                        labelText: 'New preset name',
                        isDense: true,
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  FilledButton(
                    key: const ValueKey('add-preset'),
                    onPressed: _addPreset,
                    child: const Text('Add preset'),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.sm),
              DraftDropdownField(
                key: const ValueKey('active-preset'),
                label: 'Active preset (runs)',
                value: _activePreset(),
                options: names,
                width: 260,
                onChanged: _setActive,
              ),
            ],
          ),
          if (preset == null)
            const Padding(
              key: ValueKey('spammer-empty'),
              padding: EdgeInsets.only(bottom: ShellSpace.lg),
              child: Text(
                'No presets yet — add one, then add its keys in priority order.',
              ),
            )
          else ...[
            EditorSection(
              title: 'Opening — $preset',
              children: [
                Row(
                  children: [
                    DraftFieldSlot(
                      slotKey: 'lead-in',
                      syncToken: draft.seedRevision,
                      child: DraftNumberField(
                        label: 'Lead-in (s)',
                        value: _opening(preset, 'lead_in'),
                        width: 160,
                        onChanged: (value) =>
                            _setOpening(preset, 'lead_in', value),
                      ),
                    ),
                    const SizedBox(width: ShellSpace.md),
                    DraftFieldSlot(
                      slotKey: 'startup-step',
                      syncToken: draft.seedRevision,
                      child: DraftNumberField(
                        label: 'Opening step (s)',
                        value: _opening(preset, 'step'),
                        width: 160,
                        onChanged: (value) =>
                            _setOpening(preset, 'step', value),
                      ),
                    ),
                  ],
                ),
              ],
            ),
            EditorSection(
              title: 'Keys — $preset (priority order)',
              children: [
                for (var index = 0; index < keys.length; index++)
                  Padding(
                    padding: const EdgeInsets.only(bottom: ShellSpace.xs),
                    child: Row(
                      children: [
                        DraftFieldSlot(
                          slotKey: 'key-name-$index',
                          syncToken: draft.seedRevision,
                          child: DraftTextField(
                            label: 'Key ${index + 1}',
                            value: keys[index],
                            width: 180,
                            onChanged: (value) =>
                                _renameKey(preset, keys[index], value),
                          ),
                        ),
                        const SizedBox(width: ShellSpace.sm),
                        DraftFieldSlot(
                          slotKey: 'key-seconds-$index',
                          syncToken: draft.seedRevision,
                          child: DraftNumberField(
                            label: 'Seconds',
                            value: _secondsFor(preset, keys[index]),
                            width: 130,
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
                          icon: const Icon(Icons.arrow_upward, size: 18),
                        ),
                        IconButton(
                          key: ValueKey('key-down-$index'),
                          tooltip: 'Move down',
                          onPressed: index == keys.length - 1
                              ? null
                              : () => _moveKey(preset, index, 1),
                          icon: const Icon(Icons.arrow_downward, size: 18),
                        ),
                        IconButton(
                          key: ValueKey('key-remove-$index'),
                          tooltip: 'Remove',
                          onPressed: () => _removeKey(preset, keys[index]),
                          icon: const Icon(Icons.close, size: 18),
                        ),
                      ],
                    ),
                  ),
                const SizedBox(height: ShellSpace.sm),
                Row(
                  children: [
                    SizedBox(
                      width: 180,
                      child: TextField(
                        key: const ValueKey('new-key-name'),
                        controller: _newKeyName,
                        decoration: const InputDecoration(
                          labelText: 'New key',
                          isDense: true,
                          border: OutlineInputBorder(),
                        ),
                      ),
                    ),
                    const SizedBox(width: ShellSpace.sm),
                    FilledButton(
                      key: const ValueKey('add-key'),
                      onPressed: () => _addKey(preset),
                      child: const Text('Add key'),
                    ),
                  ],
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }
}

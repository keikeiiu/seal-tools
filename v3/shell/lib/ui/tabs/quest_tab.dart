/// The Quest tab: the `quest` top-level block — named flows (steps), named sequences that reference
/// flows, the active selections and the cursor guard.
///
/// The core owns the rules: a sequence entry naming a flow that does not exist is refused by
/// `config.validate`/`config.apply` on Save (PROTOCOL §4/§7), and the tab renders that line verbatim.
/// The pickers only OFFER existing names, so a typo cannot be authored in the UI while a hand-edited
/// file is still reported by the core.
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

const List<String> _questActions = [
  'click',
  'right_click',
  'key',
  'enter',
  'wait',
];

/// The whole block is owned by this tab: the flows/sequences ARE the config.
const Set<String> _editableQuestKeys = {
  'active',
  'presets',
  'active_sequence',
  'sequences',
  'guard_px',
};

class QuestTab extends StatefulWidget {
  const QuestTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<QuestTab> createState() => _QuestTabState();
}

class _QuestTabState extends State<QuestTab>
    with ConfigTabState<QuestTab>, AutomaticKeepAliveClientMixin<QuestTab> {
  @override
  ShellModel get model => widget.model;

  /// Keep the tab alive when the player switches away: an unsaved draft (and its conflict
  /// banner) must survive the switch.
  @override
  bool get wantKeepAlive => true;

  @override
  final ConfigDraft draft = ConfigDraft(
    blockName: 'quest',
    scope: ConfigScope.ownedBlock,
    editableKeys: _editableQuestKeys,
  );

  final _newFlow = TextEditingController();
  final _newSequence = TextEditingController();
  String? _selectedFlow;
  String? _selectedSequence;

  /// The tab's refusal line for the UI-owned deletion guards (v2's phrasing); null when there is
  /// nothing to say. A refusal from the CORE still renders through the shared Save frame.
  String? _questNote;

  /// v2's exact line when the last flow would be deleted (its flow `ConfirmAndDelete`).
  static const String _lastFlowRefusal =
      'This is the only flow — create another before deleting it.';

  @override
  void dispose() {
    _newFlow.dispose();
    _newSequence.dispose();
    super.dispose();
  }

  // ── readers ──────────────────────────────────────────────────────────────────────────────

  Map<String, dynamic> _flows() => asJsonMap(draft.value['presets']);

  List<String> _flowNames() => _flows().keys.toList()..sort();

  Map<String, dynamic> _flow(String name) => asJsonMap(_flows()[name]);

  List<Map<String, dynamic>> _steps(String name) => [
    for (final entry in asJsonList(_flow(name)['steps'])) asJsonMap(entry),
  ];

  Map<String, dynamic> _sequences() => asJsonMap(draft.value['sequences']);

  List<String> _sequenceNames() => _sequences().keys.toList()..sort();

  Map<String, dynamic> _sequence(String name) => asJsonMap(_sequences()[name]);

  List<Map<String, dynamic>> _entries(String name) => [
    for (final entry in asJsonList(_sequence(name)['entries']))
      asJsonMap(entry),
  ];

  String? _activeFlow() {
    final active = asString(draft.value['active']);
    return active == null || active.isEmpty ? null : active;
  }

  String? _activeSequence() {
    final active = asString(draft.value['active_sequence']);
    return active == null || active.isEmpty ? null : active;
  }

  String? _currentFlow() {
    final names = _flowNames();
    if (names.isEmpty) {
      return null;
    }
    final selected = _selectedFlow;
    if (selected != null && names.contains(selected)) {
      return selected;
    }
    final active = _activeFlow();
    if (active != null && names.contains(active)) {
      return active;
    }
    return names.first;
  }

  String? _currentSequence() {
    final names = _sequenceNames();
    if (names.isEmpty) {
      return null;
    }
    final selected = _selectedSequence;
    if (selected != null && names.contains(selected)) {
      return selected;
    }
    final active = _activeSequence();
    if (active != null && names.contains(active)) {
      return active;
    }
    return names.first;
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

  List<dynamic> _ensureList(Map<String, dynamic> parent, String key) {
    final existing = parent[key];
    if (existing is List) {
      return existing;
    }
    final created = <dynamic>[];
    parent[key] = created;
    return created;
  }

  void _setActiveFlow(String? name) => draft.edit((value) {
    if (name == null) {
      value.remove('active');
    } else {
      value['active'] = name;
    }
  });

  void _setActiveSequence(String? name) => draft.edit((value) {
    if (name == null) {
      value.remove('active_sequence');
    } else {
      value['active_sequence'] = name;
    }
  });

  void _addFlow() {
    final name = _newFlow.text.trim();
    if (name.isEmpty || _flows().containsKey(name)) {
      return;
    }
    draft.edit((value) {
      _ensureMap(_ensureMap(value, 'presets'), name).addAll({
        'initial_wait_seconds': 10.0,
        'loops': 1,
        'steps': <dynamic>[],
      });
      final active = value['active'];
      if (active is! String || active.isEmpty) {
        value['active'] = name;
      }
    });
    _newFlow.clear();
    setState(() {
      _selectedFlow = name;
      _questNote = null;
    });
  }

  void _deleteFlow(String name) {
    // v2's guard (its flow ConfirmAndDelete): the last flow IS the tab's usable configuration — a
    // quest config with no flow has nothing any run can replay. Same refusal line; no draft edit.
    if (_flowNames().length <= 1) {
      setState(() => _questNote = _lastFlowRefusal);
      return;
    }
    draft.edit((value) {
      asJsonMap(value['presets']).remove(name);
      if (value['active'] == name) {
        final remaining = _flowNames()..remove(name);
        value['active'] = remaining.isEmpty ? '' : remaining.first;
      }
    });
    setState(() {
      _selectedFlow = null;
      _questNote = null;
    });
  }

  void _setFlowField(String name, String field, Object? value) =>
      draft.edit((block) {
        final flow = _ensureMap(_ensureMap(block, 'presets'), name);
        if (value == null) {
          flow.remove(field);
        } else {
          flow[field] = value;
        }
      });

  void _setStepField(String flow, int index, String field, Object? value) =>
      draft.edit((block) {
        final steps = _ensureList(
          _ensureMap(_ensureMap(block, 'presets'), flow),
          'steps',
        );
        if (index >= steps.length) {
          return;
        }
        final step = asJsonMap(steps[index]);
        if (value == null) {
          step.remove(field);
        } else {
          step[field] = value;
        }
      });

  void _addStep(String flow) => draft.edit((block) {
    _ensureList(_ensureMap(_ensureMap(block, 'presets'), flow), 'steps').add(
      <String, dynamic>{
        'action': 'click',
        'value': null,
        'delay_seconds': 0.25,
      },
    );
  });

  void _removeStep(String flow, int index) => draft.edit((block) {
    final steps = _ensureList(
      _ensureMap(_ensureMap(block, 'presets'), flow),
      'steps',
    );
    if (index < steps.length) {
      steps.removeAt(index);
    }
  });

  void _moveStep(String flow, int index, int delta) => draft.edit((block) {
    final steps = _ensureList(
      _ensureMap(_ensureMap(block, 'presets'), flow),
      'steps',
    );
    final target = index + delta;
    if (target < 0 || target >= steps.length) {
      return;
    }
    final item = steps.removeAt(index);
    steps.insert(target, item);
  });

  void _addSequence() {
    final name = _newSequence.text.trim();
    if (name.isEmpty || _sequences().containsKey(name)) {
      return;
    }
    draft.edit((value) {
      _ensureMap(_ensureMap(value, 'sequences'), name).addAll({
        'initial_wait_seconds': 10.0,
        'loops': 1,
        'entries': <dynamic>[],
      });
    });
    _newSequence.clear();
    setState(() => _selectedSequence = name);
  }

  void _deleteSequence(String name) {
    draft.edit((value) {
      asJsonMap(value['sequences']).remove(name);
      if (value['active_sequence'] == name) {
        value['active_sequence'] = '';
      }
    });
    setState(() => _selectedSequence = null);
  }

  void _setSequenceField(String name, String field, Object? value) =>
      draft.edit((block) {
        final sequence = _ensureMap(_ensureMap(block, 'sequences'), name);
        if (value == null) {
          sequence.remove(field);
        } else {
          sequence[field] = value;
        }
      });

  void _setEntryField(
    String sequence,
    int index,
    String field,
    Object? value,
  ) => draft.edit((block) {
    final entries = _ensureList(
      _ensureMap(_ensureMap(block, 'sequences'), sequence),
      'entries',
    );
    if (index >= entries.length) {
      return;
    }
    final entry = asJsonMap(entries[index]);
    if (value == null) {
      entry.remove(field);
    } else {
      entry[field] = value;
    }
  });

  void _addEntry(String sequence) {
    final flow = _currentFlow() ?? '';
    draft.edit((block) {
      _ensureList(
        _ensureMap(_ensureMap(block, 'sequences'), sequence),
        'entries',
      ).add(<String, dynamic>{'preset': flow, 'times': 1});
    });
  }

  void _removeEntry(String sequence, int index) => draft.edit((block) {
    final entries = _ensureList(
      _ensureMap(_ensureMap(block, 'sequences'), sequence),
      'entries',
    );
    if (index < entries.length) {
      entries.removeAt(index);
    }
  });

  void _setGuard(num value) => draft.edit((block) {
    block['guard_px'] = value;
  });

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final flows = _flowNames();
    final flow = _currentFlow();
    final sequences = _sequenceNames();
    final sequence = _currentSequence();
    final steps = flow == null ? const <Map<String, dynamic>>[] : _steps(flow);
    final entries = sequence == null
        ? const <Map<String, dynamic>>[]
        : _entries(sequence);

    return buildConfigTab(
      title: 'Quest Hand-in',
      description:
          'Named flows replay input sequences; sequences run several flows in a row. The core '
          'refuses a save whose sequence names a flow that does not exist.',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EditorSection(
            title: 'Active',
            children: [
              Wrap(
                spacing: ShellSpace.md,
                runSpacing: ShellSpace.sm,
                children: [
                  DraftDropdownField(
                    key: const ValueKey('quest-active-flow'),
                    label: 'Active flow',
                    value: _activeFlow(),
                    options: flows,
                    width: ShellWidth.picker,
                    onChanged: _setActiveFlow,
                  ),
                  DraftDropdownField(
                    key: const ValueKey('quest-active-sequence'),
                    label: 'Active sequence (empty = flow only)',
                    value: _activeSequence(),
                    options: sequences,
                    width: 300,
                    onChanged: _setActiveSequence,
                  ),
                  OutlinedButton(
                    key: const ValueKey('quest-clear-sequence'),
                    onPressed: () => _setActiveSequence(''),
                    child: const Text('Single flow only'),
                  ),
                  DraftFieldSlot(
                    slotKey: 'quest-guard',
                    syncToken: draft.seedRevision,
                    child: DraftNumberField(
                      label: 'Cursor guard px',
                      integer: true,
                      value: asNumber(draft.value['guard_px']),
                      width: 180,
                      onChanged: _setGuard,
                    ),
                  ),
                ],
              ),
            ],
          ),
          EditorSection(
            title: 'Flows',
            children: [
              Row(
                children: [
                  DraftDropdownField(
                    key: const ValueKey('quest-flow-picker'),
                    label: 'Editing flow',
                    value: flow,
                    options: flows,
                    width: ShellWidth.picker,
                    onChanged: (value) {
                      if (value != null) {
                        // A refusal about the previously edited flow is stale once another is picked
                        // (v2's SelectionReport rule); a successful note was already cleared.
                        setState(() {
                          _selectedFlow = value;
                          _questNote = null;
                        });
                      }
                    },
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  OutlinedButton(
                    key: const ValueKey('quest-delete-flow'),
                    onPressed: flow == null ? null : () => _deleteFlow(flow),
                    child: const Text('Delete flow'),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.sm),
              Row(
                children: [
                  SizedBox(
                    width: ShellWidth.picker,
                    child: TextField(
                      key: const ValueKey('quest-new-flow'),
                      controller: _newFlow,
                      decoration: const InputDecoration(
                        labelText: 'New flow name',
                        isDense: true,
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  FilledButton(
                    key: const ValueKey('quest-add-flow'),
                    onPressed: _addFlow,
                    child: const Text('Add flow'),
                  ),
                ],
              ),
              if (_questNote != null)
                Padding(
                  padding: const EdgeInsets.only(top: ShellSpace.xs),
                  child: Text(
                    _questNote!,
                    key: const ValueKey('quest-note'),
                    style: Theme.of(context).textTheme.bodySmall
                        ?.copyWith(color: Theme.of(context).colorScheme.error),
                  ),
                ),
              if (flow == null)
                const Padding(
                  key: ValueKey('quest-no-flow'),
                  padding: EdgeInsets.only(top: ShellSpace.sm),
                  child: Text('No flows yet — add one, then add its steps.'),
                )
              else ...[
                const SizedBox(height: ShellSpace.sm),
                Wrap(
                  spacing: ShellSpace.md,
                  runSpacing: ShellSpace.sm,
                  children: [
                    DraftFieldSlot(
                      slotKey: 'quest-flow-initial-wait',
                      syncToken: draft.seedRevision,
                      child: DraftNumberField(
                        label: 'Initial wait (s)',
                        value: asNumber(_flow(flow)['initial_wait_seconds']),
                        width: 180,
                        onChanged: (value) =>
                            _setFlowField(flow, 'initial_wait_seconds', value),
                      ),
                    ),
                    DraftFieldSlot(
                      slotKey: 'quest-flow-loops',
                      syncToken: draft.seedRevision,
                      child: DraftNumberField(
                        label: 'Loops',
                        integer: true,
                        value: asNumber(_flow(flow)['loops']),
                        width: ShellWidth.fieldSm,
                        onChanged: (value) =>
                            _setFlowField(flow, 'loops', value),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: ShellSpace.sm),
                Text(
                  'Steps — $flow',
                  style: Theme.of(context).textTheme.labelLarge,
                ),
                for (var index = 0; index < steps.length; index++)
                  Padding(
                    padding: const EdgeInsets.only(top: ShellSpace.xs),
                    child: Row(
                      children: [
                        DraftDropdownField(
                          key: ValueKey('quest-step-action-$index'),
                          label: 'Action',
                          value: asString(steps[index]['action']),
                          options: _questActions,
                          width: ShellWidth.fieldMd,
                          onChanged: (value) =>
                              _setStepField(flow, index, 'action', value),
                        ),
                        if (asString(steps[index]['action']) == 'key') ...[
                          const SizedBox(width: ShellSpace.sm),
                          DraftFieldSlot(
                            slotKey: 'quest-step-value-$index',
                            syncToken: draft.seedRevision,
                            child: DraftTextField(
                              label: 'Key value',
                              helper: 'used by action=key',
                              value: asString(steps[index]['value']) ?? '',
                              width: 150,
                              onChanged: (value) => _setStepField(
                                flow,
                                index,
                                'value',
                                value.isEmpty ? null : value,
                              ),
                            ),
                          ),
                        ],
                        const SizedBox(width: ShellSpace.sm),
                        DraftFieldSlot(
                          slotKey: 'quest-step-delay-$index',
                          syncToken: draft.seedRevision,
                          child: DraftNumberField(
                            label: 'Delay (s)',
                            value: asNumber(steps[index]['delay_seconds']),
                            width: 150,
                            onChanged: (value) => _setStepField(
                              flow,
                              index,
                              'delay_seconds',
                              value,
                            ),
                          ),
                        ),
                        IconButton(
                          key: ValueKey('quest-step-up-$index'),
                          tooltip: 'Move up',
                          onPressed: index == 0
                              ? null
                              : () => _moveStep(flow, index, -1),
                          icon: const Icon(Icons.arrow_upward, size: 18),
                        ),
                        IconButton(
                          key: ValueKey('quest-step-down-$index'),
                          tooltip: 'Move down',
                          onPressed: index == steps.length - 1
                              ? null
                              : () => _moveStep(flow, index, 1),
                          icon: const Icon(Icons.arrow_downward, size: 18),
                        ),
                        IconButton(
                          key: ValueKey('quest-step-remove-$index'),
                          tooltip: 'Remove step',
                          onPressed: () => _removeStep(flow, index),
                          icon: const Icon(Icons.close, size: 18),
                        ),
                      ],
                    ),
                  ),
                const SizedBox(height: ShellSpace.sm),
                OutlinedButton(
                  key: const ValueKey('quest-add-step'),
                  onPressed: () => _addStep(flow),
                  child: const Text('Add step'),
                ),
              ],
            ],
          ),
          EditorSection(
            title: 'Sequences',
            children: [
              Row(
                children: [
                  DraftDropdownField(
                    key: const ValueKey('quest-sequence-picker'),
                    label: 'Editing sequence',
                    value: sequence,
                    options: sequences,
                    width: 260,
                    onChanged: (value) {
                      if (value != null) {
                        setState(() => _selectedSequence = value);
                      }
                    },
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  OutlinedButton(
                    key: const ValueKey('quest-delete-sequence'),
                    onPressed: sequence == null
                        ? null
                        : () => _deleteSequence(sequence),
                    child: const Text('Delete sequence'),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.sm),
              Row(
                children: [
                  SizedBox(
                    width: 260,
                    child: TextField(
                      key: const ValueKey('quest-new-sequence'),
                      controller: _newSequence,
                      decoration: const InputDecoration(
                        labelText: 'New sequence name',
                        isDense: true,
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  FilledButton(
                    key: const ValueKey('quest-add-sequence'),
                    onPressed: _addSequence,
                    child: const Text('Add sequence'),
                  ),
                ],
              ),
              if (sequence == null)
                const Padding(
                  key: ValueKey('quest-no-sequence'),
                  padding: EdgeInsets.only(top: ShellSpace.sm),
                  child: Text(
                    'No sequences — the active flow runs on its own.',
                  ),
                )
              else ...[
                const SizedBox(height: ShellSpace.sm),
                Wrap(
                  spacing: ShellSpace.md,
                  runSpacing: ShellSpace.sm,
                  children: [
                    DraftFieldSlot(
                      slotKey: 'quest-sequence-initial-wait',
                      syncToken: draft.seedRevision,
                      child: DraftNumberField(
                        label: 'Initial wait (s)',
                        value: asNumber(
                          _sequence(sequence)['initial_wait_seconds'],
                        ),
                        width: 180,
                        onChanged: (value) => _setSequenceField(
                          sequence,
                          'initial_wait_seconds',
                          value,
                        ),
                      ),
                    ),
                    DraftFieldSlot(
                      slotKey: 'quest-sequence-loops',
                      syncToken: draft.seedRevision,
                      child: DraftNumberField(
                        label: 'Master loops',
                        integer: true,
                        value: asNumber(_sequence(sequence)['loops']),
                        width: 150,
                        onChanged: (value) =>
                            _setSequenceField(sequence, 'loops', value),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: ShellSpace.sm),
                Text(
                  'Entries — $sequence',
                  style: Theme.of(context).textTheme.labelLarge,
                ),
                for (var index = 0; index < entries.length; index++)
                  Padding(
                    padding: const EdgeInsets.only(top: ShellSpace.xs),
                    child: Row(
                      children: [
                        DraftDropdownField(
                          key: ValueKey('quest-entry-flow-$index'),
                          label: 'Flow',
                          value: asString(entries[index]['preset']),
                          options: flows,
                          width: ShellWidth.fieldLg,
                          onChanged: (value) =>
                              _setEntryField(sequence, index, 'preset', value),
                        ),
                        const SizedBox(width: ShellSpace.sm),
                        DraftFieldSlot(
                          slotKey: 'quest-entry-times-$index',
                          syncToken: draft.seedRevision,
                          child: DraftNumberField(
                            label: 'Times',
                            integer: true,
                            value: asNumber(entries[index]['times']),
                            width: 120,
                            onChanged: (value) =>
                                _setEntryField(sequence, index, 'times', value),
                          ),
                        ),
                        IconButton(
                          key: ValueKey('quest-entry-remove-$index'),
                          tooltip: 'Remove entry',
                          onPressed: () => _removeEntry(sequence, index),
                          icon: const Icon(Icons.close, size: 18),
                        ),
                      ],
                    ),
                  ),
                const SizedBox(height: ShellSpace.sm),
                OutlinedButton(
                  key: const ValueKey('quest-add-entry'),
                  onPressed: () => _addEntry(sequence),
                  child: const Text('Add entry'),
                ),
              ],
            ],
          ),
        ],
      ),
    );
  }
}

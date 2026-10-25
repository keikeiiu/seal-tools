/// The Tuner tab: the PORTABLE tuner settings (`settings.tuner`) plus a read-only view of the
/// calibrated geometry.
///
/// Where the fields live is a config fact, not a UI choice: the merged hello mirror flattens the
/// shipped/default values into the top-level `tuner` block, but the CALIBRATED half (`tuner.ocr`,
/// `tuner.spring_point`) is machine state owned by wave 2's calibration surfaces, while the editable
/// half (`grade_order`, `target_grade`, retries, timing, springs, filter) is portable and must be
/// saved through the `settings:` overlay (PROTOCOL §4/§7 — the core stays the writer). The draft
/// therefore seeds from the merged block, filters to the portable keys, and sends
/// `{settings: {tuner: …}}`.
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

const List<String> _springModes = ['manual', 'hid'];
const List<String> _mouseGuards = ['off', 'stop', 'recenter'];
const List<String> _matchModes = ['any', 'all', 'per_attr'];

/// The portable fields this tab owns. Everything else in the merged tuner block (calibrated `ocr`,
/// `spring_point`, model paths, colour calibration) is read-only this wave.
const Set<String> _editableTunerKeys = {
  'grade_order',
  'target_grade',
  'max_retries',
  'ocr_retries',
  'save_captures',
  'spring_mode',
  'mouse_guard',
  'guard_px',
  'recenter_max',
  'timing',
  'filter',
};

class TunerTab extends StatefulWidget {
  const TunerTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<TunerTab> createState() => _TunerTabState();
}

class _TunerTabState extends State<TunerTab> with ConfigTabState<TunerTab> {
  @override
  ShellModel get model => widget.model;

  @override
  final ConfigDraft draft = ConfigDraft(
    blockName: 'tuner',
    scope: ConfigScope.portableSection,
    editableKeys: _editableTunerKeys,
  );

  final _newGrade = TextEditingController();
  final _newRuleName = TextEditingController();
  final _newOverrideName = TextEditingController();

  @override
  void dispose() {
    _newGrade.dispose();
    _newRuleName.dispose();
    _newOverrideName.dispose();
    super.dispose();
  }

  // ── readers ──────────────────────────────────────────────────────────────────────────────

  List<String> _gradeOrder() => asStringList(draft.value['grade_order']);

  String? _targetGrade() => asString(draft.value['target_grade']);

  Map<String, dynamic> _timing() => asJsonMap(draft.value['timing']);

  Map<String, dynamic> _filter() => asJsonMap(draft.value['filter']);

  String _matchMode() => asString(_filter()['match_mode']) ?? _matchModes.first;

  String? _requireGrade() => asString(_filter()['require_grade']);

  bool _filterEnabled() => asBool(_filter()['enabled']) ?? false;

  List<Map<String, dynamic>> _rules(String key) => [
    for (final entry in asJsonList(_filter()[key])) asJsonMap(entry),
  ];

  /// The calibrated half, from the merged mirror's top-level `tuner` block.
  Map<String, dynamic> _calibrated() => asJsonMap(model.config.config['tuner']);

  String _regionText(Object? value) {
    final region = asJsonMap(value);
    if (region.isEmpty) {
      return '—';
    }
    final left = asNumber(region['left'])?.round();
    final top = asNumber(region['top'])?.round();
    final width = asNumber(region['width'])?.round();
    final height = asNumber(region['height'])?.round();
    return 'left $left, top $top, ${width}x$height (client-relative physical px)';
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

  void _setGradeOrder(List<String> order) => draft.edit((value) {
    value['grade_order'] = order;
  });

  void _moveGrade(List<String> order, int index, int delta) {
    final target = index + delta;
    if (target < 0 || target >= order.length) {
      return;
    }
    final next = [...order];
    final item = next.removeAt(index);
    next.insert(target, item);
    _setGradeOrder(next);
  }

  void _setTargetGrade(String? grade) => draft.edit((value) {
    if (grade == null) {
      value.remove('target_grade');
    } else {
      value['target_grade'] = grade;
    }
  });

  void _setPortable(String key, Object? value) => draft.edit((map) {
    if (value == null) {
      map.remove(key);
    } else {
      map[key] = value;
    }
  });

  void _setTiming(String key, num value) => draft.edit((map) {
    _ensureMap(map, 'timing')[key] = value;
  });

  void _setFilter(void Function(Map<String, dynamic> filter) edit) =>
      draft.edit((map) => edit(_ensureMap(map, 'filter')));

  void _setRule(String listKey, int index, String field, Object? value) {
    _setFilter((filter) {
      final rules = filter[listKey];
      if (rules is! List || index >= rules.length) {
        return;
      }
      final rule = asJsonMap(rules[index]);
      if (value == null) {
        rule.remove(field);
      } else {
        rule[field] = value;
      }
    });
  }

  void _removeRule(String listKey, int index) {
    _setFilter((filter) {
      final rules = filter[listKey];
      if (rules is List && index < rules.length) {
        rules.removeAt(index);
      }
    });
  }

  void _addRule(String listKey) {
    final name = _newRuleName.text.trim();
    if (name.isEmpty) {
      return;
    }
    _setFilter((filter) {
      final rules = filter[listKey];
      if (rules is List) {
        rules.add(<String, dynamic>{'name': name, 'count': 1});
      } else {
        filter[listKey] = <dynamic>[
          <String, dynamic>{'name': name, 'count': 1},
        ];
      }
    });
    _newRuleName.clear();
  }

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    final order = _gradeOrder();
    final calibrated = _calibrated();
    final ocr = asJsonMap(calibrated['ocr']);
    final rules = _rules('rules');
    final overrides = _rules('override_rules');

    return buildConfigTab(
      title: 'Magic Tuner',
      description:
          'The tuner\'s portable settings. Saves go through the `settings:` overlay; the core '
          'merges field-wise and remains the only writer.',
      readOnly: [
        EditorSection(
          title: 'Calibrated geometry — read-only this wave',
          children: [
            ReadOnlyValue(
              label: 'OCR region',
              value: _regionText(ocr['region']),
            ),
            ReadOnlyValue(
              label: 'Grade area',
              value: readOnlyText(ocr['grade_area']),
            ),
            ReadOnlyValue(
              label: 'Spring point',
              value: readOnlyText(calibrated['spring_point']),
            ),
          ],
        ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EditorSection(
            title: 'Grades',
            children: [
              DraftDropdownField(
                key: const ValueKey('target-grade'),
                label: 'Target grade',
                value: _targetGrade(),
                options: order,
                width: 220,
                onChanged: _setTargetGrade,
              ),
              const SizedBox(height: ShellSpace.sm),
              for (var index = 0; index < order.length; index++)
                Padding(
                  padding: const EdgeInsets.only(bottom: ShellSpace.xs),
                  child: Row(
                    children: [
                      DraftFieldSlot(
                        slotKey: 'grade-$index',
                        syncToken: draft.seedRevision,
                        child: DraftTextField(
                          label: 'Grade ${index + 1}',
                          value: order[index],
                          width: 160,
                          onChanged: (value) {
                            final next = [...order];
                            next[index] = value;
                            _setGradeOrder(next);
                          },
                        ),
                      ),
                      IconButton(
                        key: ValueKey('grade-up-$index'),
                        tooltip: 'Move up',
                        onPressed: index == 0
                            ? null
                            : () => _moveGrade(order, index, -1),
                        icon: const Icon(Icons.arrow_upward, size: 18),
                      ),
                      IconButton(
                        key: ValueKey('grade-down-$index'),
                        tooltip: 'Move down',
                        onPressed: index == order.length - 1
                            ? null
                            : () => _moveGrade(order, index, 1),
                        icon: const Icon(Icons.arrow_downward, size: 18),
                      ),
                      IconButton(
                        key: ValueKey('grade-remove-$index'),
                        tooltip: 'Remove',
                        onPressed: () {
                          final next = [...order]..removeAt(index);
                          _setGradeOrder(next);
                        },
                        icon: const Icon(Icons.close, size: 18),
                      ),
                    ],
                  ),
                ),
              Row(
                children: [
                  SizedBox(
                    width: 160,
                    child: TextField(
                      key: const ValueKey('new-grade-name'),
                      controller: _newGrade,
                      decoration: const InputDecoration(
                        labelText: 'New grade',
                        isDense: true,
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  FilledButton(
                    key: const ValueKey('add-grade'),
                    onPressed: () {
                      final name = _newGrade.text.trim();
                      if (name.isEmpty || order.contains(name)) {
                        return;
                      }
                      _setGradeOrder([...order, name]);
                      _newGrade.clear();
                    },
                    child: const Text('Add grade'),
                  ),
                ],
              ),
            ],
          ),
          EditorSection(
            title: 'Retries and timing',
            children: [
              Wrap(
                spacing: ShellSpace.md,
                runSpacing: ShellSpace.sm,
                children: [
                  DraftFieldSlot(
                    slotKey: 'max-retries',
                    syncToken: draft.seedRevision,
                    child: DraftNumberField(
                      label: 'Max retries',
                      integer: true,
                      value: asNumber(draft.value['max_retries']),
                      width: 150,
                      onChanged: (value) => _setPortable('max_retries', value),
                    ),
                  ),
                  DraftFieldSlot(
                    slotKey: 'ocr-retries',
                    syncToken: draft.seedRevision,
                    child: DraftNumberField(
                      label: 'OCR retries',
                      integer: true,
                      value: asNumber(draft.value['ocr_retries']),
                      width: 150,
                      onChanged: (value) => _setPortable('ocr_retries', value),
                    ),
                  ),
                  DraftFieldSlot(
                    slotKey: 'click-enter-delay',
                    syncToken: draft.seedRevision,
                    child: DraftNumberField(
                      label: 'Click/enter delay (s)',
                      value: asNumber(_timing()['click_enter_delay']),
                      width: 180,
                      onChanged: (value) =>
                          _setTiming('click_enter_delay', value),
                    ),
                  ),
                  DraftFieldSlot(
                    slotKey: 'ocr-delay',
                    syncToken: draft.seedRevision,
                    child: DraftNumberField(
                      label: 'OCR delay (s)',
                      value: asNumber(_timing()['ocr_delay']),
                      width: 150,
                      onChanged: (value) => _setTiming('ocr_delay', value),
                    ),
                  ),
                ],
              ),
              DraftSwitchField(
                key: const ValueKey('save-captures'),
                label: 'Save captures',
                value: asBool(draft.value['save_captures']) ?? false,
                onChanged: (value) => _setPortable('save_captures', value),
              ),
            ],
          ),
          EditorSection(
            title: 'Springs',
            children: [
              Wrap(
                spacing: ShellSpace.md,
                runSpacing: ShellSpace.sm,
                children: [
                  DraftDropdownField(
                    key: const ValueKey('spring-mode'),
                    label: 'Spring mode',
                    value: asString(draft.value['spring_mode']),
                    options: _springModes,
                    width: 180,
                    onChanged: (value) => _setPortable('spring_mode', value),
                  ),
                  DraftDropdownField(
                    key: const ValueKey('mouse-guard'),
                    label: 'Mouse guard',
                    value: asString(draft.value['mouse_guard']),
                    options: _mouseGuards,
                    width: 180,
                    onChanged: (value) => _setPortable('mouse_guard', value),
                  ),
                  DraftFieldSlot(
                    slotKey: 'guard-px',
                    syncToken: draft.seedRevision,
                    child: DraftNumberField(
                      label: 'Guard px',
                      integer: true,
                      value: asNumber(draft.value['guard_px']),
                      width: 130,
                      onChanged: (value) => _setPortable('guard_px', value),
                    ),
                  ),
                  DraftFieldSlot(
                    slotKey: 'recenter-max',
                    syncToken: draft.seedRevision,
                    child: DraftNumberField(
                      label: 'Recenter max',
                      integer: true,
                      value: asNumber(draft.value['recenter_max']),
                      width: 140,
                      onChanged: (value) => _setPortable('recenter_max', value),
                    ),
                  ),
                ],
              ),
            ],
          ),
          EditorSection(
            title: 'Filter',
            children: [
              DraftSwitchField(
                key: const ValueKey('filter-enabled'),
                label: 'Filter enabled',
                value: _filterEnabled(),
                onChanged: (enabled) =>
                    _setFilter((filter) => filter['enabled'] = enabled),
              ),
              Wrap(
                spacing: ShellSpace.md,
                runSpacing: ShellSpace.sm,
                children: [
                  DraftDropdownField(
                    key: const ValueKey('match-mode'),
                    label: 'Match mode',
                    value: _matchMode(),
                    options: _matchModes,
                    width: 170,
                    onChanged: (value) =>
                        _setFilter((filter) => filter['match_mode'] = value),
                  ),
                  DraftDropdownField(
                    key: const ValueKey('require-grade'),
                    label: 'Require grade',
                    value: _requireGrade(),
                    options: order,
                    width: 170,
                    onChanged: (value) => _setFilter((filter) {
                      if (value == null) {
                        filter.remove('require_grade');
                      } else {
                        filter['require_grade'] = value;
                      }
                    }),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.sm),
              _RuleList(
                title: 'Rules',
                listKey: 'rules',
                rules: rules,
                syncToken: draft.seedRevision,
                newRuleName: _newRuleName,
                onAdd: () => _addRule('rules'),
                onRemove: (index) => _removeRule('rules', index),
                onField: (index, field, value) =>
                    _setRule('rules', index, field, value),
              ),
              const SizedBox(height: ShellSpace.sm),
              _RuleList(
                title: 'Override rules',
                listKey: 'override_rules',
                rules: overrides,
                syncToken: draft.seedRevision,
                newRuleName: _newOverrideName,
                onAdd: () => _addRule('override_rules'),
                onRemove: (index) => _removeRule('override_rules', index),
                onField: (index, field, value) =>
                    _setRule('override_rules', index, field, value),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// One editable filter-rule list (the normal rules and the overrides share the shape).
class _RuleList extends StatelessWidget {
  const _RuleList({
    required this.title,
    required this.listKey,
    required this.rules,
    required this.syncToken,
    required this.newRuleName,
    required this.onAdd,
    required this.onRemove,
    required this.onField,
  });

  final String title;
  final String listKey;
  final List<Map<String, dynamic>> rules;
  final int syncToken;
  final TextEditingController newRuleName;
  final VoidCallback onAdd;
  final ValueChanged<int> onRemove;
  final void Function(int index, String field, Object? value) onField;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: Theme.of(context).textTheme.labelLarge),
        for (var index = 0; index < rules.length; index++)
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Row(
              children: [
                DraftFieldSlot(
                  slotKey: '$listKey-name-$index',
                  syncToken: syncToken,
                  child: DraftTextField(
                    label: 'Name',
                    value: asString(rules[index]['name']) ?? '',
                    width: 180,
                    onChanged: (value) => onField(index, 'name', value),
                  ),
                ),
                const SizedBox(width: ShellSpace.sm),
                DraftFieldSlot(
                  slotKey: '$listKey-count-$index',
                  syncToken: syncToken,
                  child: DraftNumberField(
                    label: 'Count',
                    integer: true,
                    value: asNumber(rules[index]['count']),
                    width: 100,
                    onChanged: (value) => onField(index, 'count', value),
                  ),
                ),
                const SizedBox(width: ShellSpace.sm),
                DraftFieldSlot(
                  slotKey: '$listKey-min-$index',
                  syncToken: syncToken,
                  child: DraftTextField(
                    label: 'Min',
                    value:
                        asNumber(rules[index]['min'])?.round().toString() ?? '',
                    width: 90,
                    onChanged: (value) => onField(
                      index,
                      'min',
                      value.trim().isEmpty ? null : int.tryParse(value.trim()),
                    ),
                  ),
                ),
                const SizedBox(width: ShellSpace.sm),
                DraftFieldSlot(
                  slotKey: '$listKey-max-$index',
                  syncToken: syncToken,
                  child: DraftTextField(
                    label: 'Max',
                    value:
                        asNumber(rules[index]['max'])?.round().toString() ?? '',
                    width: 90,
                    onChanged: (value) => onField(
                      index,
                      'max',
                      value.trim().isEmpty ? null : int.tryParse(value.trim()),
                    ),
                  ),
                ),
                IconButton(
                  key: ValueKey('$listKey-remove-$index'),
                  tooltip: 'Remove rule',
                  onPressed: () => onRemove(index),
                  icon: const Icon(Icons.close, size: 18),
                ),
              ],
            ),
          ),
        Row(
          children: [
            SizedBox(
              width: 180,
              child: TextField(
                key: ValueKey('$listKey-new-name'),
                controller: newRuleName,
                decoration: InputDecoration(
                  labelText: 'New $title name',
                  isDense: true,
                  border: const OutlineInputBorder(),
                ),
              ),
            ),
            const SizedBox(width: ShellSpace.sm),
            FilledButton(
              key: ValueKey('$listKey-add'),
              onPressed: onAdd,
              child: Text('Add $title'),
            ),
          ],
        ),
      ],
    );
  }
}

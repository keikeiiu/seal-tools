/// The Tuner tab: the PORTABLE tuner settings (`settings.tuner`) plus a read-only view of the
/// calibrated geometry.
///
/// Where the fields live is a config fact, not a UI choice: the merged hello mirror flattens the
/// shipped/default values into the top-level `tuner` block, but the CALIBRATED half (`tuner.ocr`,
/// `tuner.spring_point`) is machine state owned by wave 2's calibration surfaces, while the editable
/// half (target grade, retries, timing, springs, filter) is portable and must be saved through the
/// `settings:` overlay (PROTOCOL §4/§7 — the core stays the writer). `grade_order` is NOT editable:
/// the player's live pass established the grade ladder is a fixed property of the client, so the tab
/// renders it read-only and only *chooses* a target/require grade from it. Filter rule NAMES are
/// chosen from the loaded attribute dictionary (`attributes.get`), not typed, because the player
/// cannot be expected to remember the OCR names; a stored name outside the dictionary still renders
/// as the current value so an existing config is never rewritten behind the player's back.
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../protocol/protocol.dart';
import '../components/foldable_section.dart';
import '../components/grade_ladder.dart';
import '../components/info_row.dart';
import '../components/read_only_panel.dart';
import '../components/surface_panel.dart';
import '../theme.dart';
import '../tool_labels.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

/// Sentinel for the target-grade dropdown: a config Option is cleared by OMITTING the field — a
/// JSON null is 'not stated' to the core's merge (validate's merge_overlay skips nulls) — so the
/// shipped default then applies.
const String _noTargetGrade = '(none — use the shipped default)';

const List<String> _springModes = ['manual', 'hid'];
const List<String> _mouseGuards = ['off', 'stop', 'recenter'];
const List<String> _matchModes = ['any', 'all', 'per_attr'];

/// The one place the fixed tuner grade ladder is defaulted when a config carries none.
const List<String> _defaultGradeOrder = ['N', 'G', 'DG', 'XG', 'SG'];

/// The portable fields this tab owns. Everything else in the merged tuner block (calibrated `ocr`,
/// `spring_point`, model paths, colour calibration) is read-only this wave. `grade_order` is fixed
/// and read-only too, so it is deliberately NOT here — a save must leave it to the config.
const Set<String> _editableTunerKeys = {
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

class _TunerTabState extends State<TunerTab>
    with ConfigTabState<TunerTab>, AutomaticKeepAliveClientMixin<TunerTab> {
  @override
  ShellModel get model => widget.model;

  /// Keep the tab alive when the player switches away: an unsaved draft (and its conflict
  /// banner) must survive the switch.
  @override
  bool get wantKeepAlive => true;

  @override
  final ConfigDraft draft = ConfigDraft(
    blockName: 'tuner',
    scope: ConfigScope.portableSection,
    editableKeys: _editableTunerKeys,
  );

  /// The loaded attribute dictionary, fetched lazily on first build. Rule names must come from it.
  CoreActionResult<AttributesResult>? _attributes;
  bool _loadingAttributes = false;

  /// The name chosen for the NEXT rule of each list (the Add button's input; not a config value).
  String? _newRulePick;
  String? _newOverridePick;

  /// The capture-cleanup maintenance row's own state (v2 kept a line beside the button): the last
  /// result line, whether it is the error colour, and whether the housekeeping request is in flight.
  String? _cleanupResult;
  bool _cleanupFailed = false;
  bool _cleaningCaptures = false;

  @override
  void initState() {
    super.initState();
    // The Last-run aside renders a `tool.finished` event, so the tab must rebuild when the tools
    // model publishes one (the config mirror alone does not move).
    model.tools.addListener(_onToolsChanged);
    unawaited(_loadAttributes());
  }

  @override
  void dispose() {
    model.tools.removeListener(_onToolsChanged);
    super.dispose();
  }

  void _onToolsChanged() {
    if (mounted) {
      setState(() {});
    }
  }

  // ── readers ──────────────────────────────────────────────────────────────────────────────

  /// The fixed grade ladder, from the merged config (not the draft: this tab never edits it).
  List<String> _gradeOrder() {
    final order = asStringList(
      asJsonMap(model.config.config['tuner'])['grade_order'],
    );
    return order.isEmpty ? _defaultGradeOrder : order;
  }

  String? _targetGrade() => asString(draft.value['target_grade']);

  /// Attribute names from the loaded dictionary, de-duplicated and sorted for the dropdowns.
  List<String> _attributeNames() {
    final result = _attributes;
    if (result == null || !result.ok) {
      return const [];
    }
    final names = <String>{
      for (final attribute in result.value!.attributes.attributes)
        if (attribute.name.isNotEmpty) attribute.name,
    }.toList()..sort();
    return names;
  }

  /// Why the rule Add buttons are disabled, or null when the dictionary has names.
  String? get _attributesReason {
    final result = _attributes;
    if (_loadingAttributes || result == null) {
      return 'Loading attribute names…';
    }
    if (!result.ok) {
      return 'The attribute dictionary could not be read (${result.errorLine}) — '
          'rule names cannot be chosen until it loads.';
    }
    if (!result.value!.hasVariant) {
      return 'No client variant is resolved, so no attribute dictionary is loaded — '
          'rule names cannot be chosen.';
    }
    if (_attributeNames().isEmpty) {
      return 'The attribute dictionary is empty for this client — '
          'rule names cannot be chosen.';
    }
    return null;
  }

  Future<void> _loadAttributes() async {
    setState(() => _loadingAttributes = true);
    final result = await model.attributesGet();
    if (!mounted) {
      return;
    }
    setState(() {
      _loadingAttributes = false;
      _attributes = result;
    });
  }

  /// The capture-cleanup button asks first: the command deletes files and cannot be undone.
  Future<void> _confirmCaptureCleanup() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Delete the capture images?'),
        content: const Text(
          'Every saved OCR capture PNG under logs/captures is deleted. '
          'This cannot be undone; the rest of the log is kept.',
        ),
        actions: [
          TextButton(
            key: const ValueKey('tuner-cleanup-cancel'),
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            key: const ValueKey('tuner-cleanup-confirm'),
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('Delete captures'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) {
      return;
    }
    await _cleanupCaptures();
  }

  /// Tuner housekeeping: `logs.cleanup_captures` deletes the saved OCR capture PNGs. It is NOT gated
  /// on a running tool — the command opens no game or board and takes no input lease
  /// ([PROTOCOL.md] §2's one explicit log-housekeeping exception), so it works mid-run too.
  Future<void> _cleanupCaptures() async {
    setState(() {
      _cleaningCaptures = true;
      _cleanupResult = null;
    });
    final result = await model.cleanupCaptures();
    if (!mounted) {
      return;
    }
    setState(() {
      _cleaningCaptures = false;
      _cleanupFailed = !result.ok;
      _cleanupResult = result.ok
          ? _cleanupLine(result.value!)
          : result.errorLine;
    });
  }

  /// v2's line ("Deleted N capture image(s).") plus the core-echoed directory, so the shell never
  /// guesses the root-relative path it just had the core clean.
  String _cleanupLine(Map<String, dynamic> result) {
    final deleted = asNumber(result['deleted'])?.round() ?? 0;
    final dir = asString(result['dir']) ?? 'logs/captures';
    return 'Deleted $deleted capture image(s) from $dir.';
  }

  /// A stored rule name outside the dictionary stays selectable as the current value; new picks
  /// come only from the dictionary.
  List<String> _nameOptions(String stored) {
    final names = _attributeNames();
    if (stored.isEmpty) {
      return names;
    }
    if (!names.contains(stored)) {
      return [stored, ...names];
    }
    return names;
  }

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

  String? _newRulePickFor(String listKey) =>
      listKey == 'rules' ? _newRulePick : _newOverridePick;

  void _setNewRulePick(String listKey, String? name) {
    setState(() {
      if (listKey == 'rules') {
        _newRulePick = name;
      } else {
        _newOverridePick = name;
      }
    });
  }

  void _addRule(String listKey) {
    final name = _newRulePickFor(listKey);
    if (name == null || name.isEmpty || _attributeNames().isEmpty) {
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
    _setNewRulePick(listKey, null);
  }

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
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
      aside: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ReadOnlyPanel(
            title: 'Calibrated geometry',
            subtitle: 'Drawn on the Calibrate screen',
            openHint: 'read-only',
            rows: [
              InfoRow(
                label: 'OCR region',
                value: _regionText(ocr['region']),
                mono: true,
              ),
              InfoRow(
                label: 'Grade area',
                value: readOnlyText(ocr['grade_area']),
                mono: true,
              ),
              InfoRow(
                label: 'Spring point',
                value: readOnlyText(calibrated['spring_point']),
                mono: true,
              ),
            ],
          ),
          // The demo's Last-run panel (G11): the core publishes the structured summary + UTC finish
          // instant on `tool.finished`; the shell only formats it. Absent until a run has finished.
          _LastRunPanel(finished: model.tools.finished(ToolId.tuner)),
          const SurfacePanel(
            title: 'How saving works',
            description:
                'Save validates, then applies the portable fields; the core merges '
                'field-wise and remains the only writer. Calibration is not edited here.',
            child: SizedBox.shrink(),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EditorSection(
            title: 'Grades',
            children: [
              GradeLadder(
                grades: order,
                keyPrefix: 'grade-order',
                caption: 'Fixed grade order',
                captionKey: const ValueKey('grade-order-caption'),
                target: _targetGrade(),
                minimum: _requireGrade(),
              ),
              const SizedBox(height: ShellSpace.md),
              Wrap(
                spacing: ShellSpace.md,
                runSpacing: ShellSpace.sm,
                children: [
                  DraftDropdownField(
                    key: const ValueKey('target-grade'),
                    label: 'Target grade',
                    value: _targetGrade() ?? _noTargetGrade,
                    options: [_noTargetGrade, ...order],
                    width: ShellWidth.fieldLg,
                    onChanged: (value) {
                      if (value == null || value == _noTargetGrade) {
                        _setTargetGrade(null);
                      } else {
                        _setTargetGrade(value);
                      }
                    },
                  ),
                  DraftDropdownField(
                    key: const ValueKey('require-grade'),
                    label: 'Minimum grade (filter)',
                    value: _requireGrade(),
                    options: order,
                    width: ShellWidth.fieldLg,
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
                      width: ShellWidth.fieldSm,
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
                    width: ShellWidth.fieldMd,
                    onChanged: (value) =>
                        _setFilter((filter) => filter['match_mode'] = value),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.sm),
              _RuleList(
                title: 'Rules',
                listKey: 'rules',
                rules: rules,
                syncToken: draft.seedRevision,
                attributeNames: _attributeNames(),
                attributesReason: _attributesReason,
                newRulePick: _newRulePick,
                onNewRulePick: (name) => _setNewRulePick('rules', name),
                onAdd: () => _addRule('rules'),
                onRemove: (index) => _removeRule('rules', index),
                onField: (index, field, value) =>
                    _setRule('rules', index, field, value),
                onNameOptions: _nameOptions,
              ),
              const SizedBox(height: ShellSpace.sm),
              _RuleList(
                title: 'Override rules',
                listKey: 'override_rules',
                rules: overrides,
                syncToken: draft.seedRevision,
                attributeNames: _attributeNames(),
                attributesReason: _attributesReason,
                newRulePick: _newOverridePick,
                onNewRulePick: (name) =>
                    _setNewRulePick('override_rules', name),
                onAdd: () => _addRule('override_rules'),
                onRemove: (index) => _removeRule('override_rules', index),
                onField: (index, field, value) =>
                    _setRule('override_rules', index, field, value),
                onNameOptions: _nameOptions,
              ),
            ],
          ),
          FoldableSection(
            title: 'Advanced options',
            description:
                'Diagnostics and capture recording. Nothing here changes how a '
                'normal run behaves.',
            children: [
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
              const SizedBox(height: ShellSpace.sm),
              DraftSwitchField(
                key: const ValueKey('save-captures'),
                label: 'Save captures',
                value: asBool(draft.value['save_captures']) ?? false,
                onChanged: (value) => _setPortable('save_captures', value),
              ),
              const SizedBox(height: ShellSpace.md),
              Row(
                children: [
                  OutlinedButton(
                    key: const ValueKey('tuner-cleanup-captures'),
                    // Housekeeping only: never disabled for a RUNNING tool (it opens no game or
                    // board — PROTOCOL §2's one log-housekeeping exception) — only while its own
                    // request is in flight.
                    onPressed: _cleaningCaptures
                        ? null
                        : _confirmCaptureCleanup,
                    child: const Text('Clean up capture images'),
                  ),
                  if (_cleaningCaptures) ...[
                    const SizedBox(width: ShellSpace.sm),
                    const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  ],
                ],
              ),
              const SizedBox(height: ShellSpace.xs),
              Text(
                'Deletes the saved OCR capture PNGs under the root\'s '
                'logs/captures. It removes image files only; the rest of the '
                'log stays.',
                key: const ValueKey('tuner-cleanup-caption'),
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
              if (_cleanupResult != null)
                Padding(
                  padding: const EdgeInsets.only(top: ShellSpace.xs),
                  child: Text(
                    _cleanupResult!,
                    key: const ValueKey('tuner-cleanup-result'),
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: _cleanupFailed
                          ? Theme.of(context).colorScheme.error
                          : null,
                    ),
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

/// One editable filter-rule list (the normal rules and the overrides share the shape).
///
/// The rule NAME is a dropdown of the loaded dictionary's attribute names — the player's live pass:
/// "the rules / attr for tuner should be selectable, no one would remember the options". A stored
/// name that the dictionary does not carry is kept as the current selection (never lose config);
/// new picks come only from the dictionary, and Add stays disabled with a reason when none load.
class _RuleList extends StatelessWidget {
  const _RuleList({
    required this.title,
    required this.listKey,
    required this.rules,
    required this.syncToken,
    required this.attributeNames,
    required this.attributesReason,
    required this.newRulePick,
    required this.onNewRulePick,
    required this.onAdd,
    required this.onRemove,
    required this.onField,
    required this.onNameOptions,
  });

  final String title;
  final String listKey;
  final List<Map<String, dynamic>> rules;
  final int syncToken;
  final List<String> attributeNames;
  final String? attributesReason;
  final String? newRulePick;
  final ValueChanged<String?> onNewRulePick;
  final VoidCallback onAdd;
  final ValueChanged<int> onRemove;
  final void Function(int index, String field, Object? value) onField;

  /// The options for one stored name: the dictionary names, with an unknown stored name kept first.
  final List<String> Function(String stored) onNameOptions;

  bool get _canAdd => attributeNames.isNotEmpty && newRulePick != null;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: theme.textTheme.labelLarge),
        for (var index = 0; index < rules.length; index++)
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Wrap(
              spacing: ShellSpace.sm,
              runSpacing: ShellSpace.sm,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                _NameDropdown(
                  dropdownKey: ValueKey('$listKey-name-$index'),
                  itemKeyPrefix: '$listKey-name-$index-item-',
                  label: 'Name',
                  value: asString(rules[index]['name']) ?? '',
                  options: onNameOptions(asString(rules[index]['name']) ?? ''),
                  onChanged: attributeNames.isEmpty
                      ? null
                      : (value) => onField(index, 'name', value),
                ),
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
            _NameDropdown(
              dropdownKey: ValueKey('$listKey-new-name'),
              itemKeyPrefix: '$listKey-new-name-item-',
              label: 'New $title name',
              value: newRulePick ?? '',
              options: attributeNames,
              onChanged: attributeNames.isEmpty ? null : onNewRulePick,
            ),
            const SizedBox(width: ShellSpace.sm),
            FilledButton.tonal(
              key: ValueKey('$listKey-add'),
              onPressed: _canAdd ? onAdd : null,
              child: Text('Add $title'),
            ),
          ],
        ),
        if (attributesReason != null)
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              attributesReason!,
              key: ValueKey('$listKey-attributes-reason'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.error,
              ),
            ),
          ),
      ],
    );
  }
}

/// A compact name dropdown: dictionary values, with the stored value kept selectable.
class _NameDropdown extends StatelessWidget {
  const _NameDropdown({
    required this.dropdownKey,
    required this.itemKeyPrefix,
    required this.label,
    required this.value,
    required this.options,
    required this.onChanged,
  });

  final Key dropdownKey;
  final String itemKeyPrefix;
  final String label;
  final String value;
  final List<String> options;
  final ValueChanged<String?>? onChanged;

  @override
  Widget build(BuildContext context) {
    final current = options.contains(value) ? value : null;
    return SizedBox(
      width: 180,
      child: InputDecorator(
        decoration: InputDecoration(
          labelText: label,
          isDense: true,
          border: const OutlineInputBorder(),
        ),
        child: DropdownButtonHideUnderline(
          child: DropdownButton<String>(
            key: dropdownKey,
            value: current,
            isExpanded: true,
            isDense: true,
            hint: const Text('Choose…'),
            items: [
              for (final option in options)
                DropdownMenuItem(
                  key: ValueKey('$itemKeyPrefix$option'),
                  value: option,
                  child: Text(option),
                ),
            ],
            onChanged: onChanged,
          ),
        ),
      ),
    );
  }
}

/// The demo's "Last run" aside (OPEN §6 G11): the core's structured `tool.finished` summary (grade /
/// attempts / captures saved / stop reason) plus its UTC `ended_at`, rendered as read-only rows. The
/// panel is absent until a run has finished, so it never invents a placeholder run.
class _LastRunPanel extends StatelessWidget {
  const _LastRunPanel({required this.finished});

  final ToolFinished? finished;

  @override
  Widget build(BuildContext context) {
    final run = finished;
    if (run == null) {
      return const SizedBox.shrink();
    }
    final summary = run.summary;
    String value(String key) {
      final raw = summary?[key];
      if (raw == null) {
        return '—';
      }
      final text = '$raw';
      return text.isEmpty ? '—' : text;
    }

    final ended = formatFinishedStamp(run.endedAt);
    return SurfacePanel(
      key: const ValueKey('tuner-last-run'),
      title: 'Last run',
      description: run.message,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          InfoRow(label: 'Grade', value: value('grade')),
          InfoRow(label: 'Attempts', value: value('attempts')),
          InfoRow(label: 'Captures saved', value: value('captures_saved')),
          InfoRow(label: 'Stopped by', value: value('stopped_by')),
          if (ended != null) InfoRow(label: 'Finished', value: ended),
        ],
      ),
    );
  }
}

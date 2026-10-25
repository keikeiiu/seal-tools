/// The Gem tab: the PORTABLE gem settings (`settings.gem`) plus a read-only view of the calibrated
/// grade positions and resource slots.
///
/// The split is the config's, not the UI's: `grades`, `start_grade`, `empty_mode`, `empty_streak`,
/// `colored_gap_min`, `save_empty_captures` and `move_mode` are portable and saved through the
/// `settings:` overlay; `gem.grade_positions`, `gem.resource_gems`, `gem.result_gem_area` and the
/// empty-slot signature are machine calibration and are shown, not sent (PROTOCOL §4/§7 — wave 2
/// owns the calibration canvases).
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

const List<String> _emptyModes = [
  'stop',
  'advance_grade',
  'advance_grade_clear',
];
const List<String> _moveModes = ['tuned', 'arduino'];

/// The portable fields this tab owns; everything else in the merged `gem` block is calibration.
const Set<String> _editableGemKeys = {
  'grades',
  'start_grade',
  'empty_mode',
  'empty_streak',
  'colored_gap_min',
  'save_empty_captures',
  'move_mode',
};

class GemTab extends StatefulWidget {
  const GemTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<GemTab> createState() => _GemTabState();
}

class _GemTabState extends State<GemTab> with ConfigTabState<GemTab> {
  @override
  ShellModel get model => widget.model;

  @override
  final ConfigDraft draft = ConfigDraft(
    blockName: 'gem',
    scope: ConfigScope.portableSection,
    editableKeys: _editableGemKeys,
  );

  final _newGrade = TextEditingController();

  @override
  void dispose() {
    _newGrade.dispose();
    super.dispose();
  }

  // ── readers ──────────────────────────────────────────────────────────────────────────────

  List<String> _grades() => asStringList(draft.value['grades']);

  String? _startGrade() => asString(draft.value['start_grade']);

  /// The calibrated half, from the merged mirror's top-level `gem` block.
  Map<String, dynamic> _calibrated() => asJsonMap(model.config.config['gem']);

  // ── writers ──────────────────────────────────────────────────────────────────────────────

  void _setGrades(List<String> grades) => draft.edit((value) {
    value['grades'] = grades;
  });

  void _setPortable(String key, Object? value) => draft.edit((map) {
    if (value == null) {
      map.remove(key);
    } else {
      map[key] = value;
    }
  });

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    final grades = _grades();
    final calibrated = _calibrated();
    final positions = asJsonMap(calibrated['grade_positions']);
    final positionsKeys = positions.keys.toList()..sort();
    final resourceSlots = asJsonList(calibrated['resource_gems']);

    return buildConfigTab(
      title: 'Gem Composer',
      description:
          'The gem grades and behaviour. Saves go through the `settings:` overlay; the calibrated '
          'positions and slots stay read-only until the calibration surfaces land.',
      readOnly: [
        EditorSection(
          title: 'Calibrated positions — read-only this wave',
          children: [
            if (positionsKeys.isEmpty)
              const ReadOnlyValue(label: 'Grade positions', value: '—'),
            for (final name in positionsKeys)
              ReadOnlyValue(
                label: 'Grade $name',
                value: readOnlyText(positions[name]),
              ),
            if (resourceSlots.isEmpty)
              const ReadOnlyValue(label: 'Resource slots', value: '—'),
            for (var index = 0; index < resourceSlots.length; index++)
              ReadOnlyValue(
                label: 'Resource slot ${index + 1}',
                value: readOnlyText(resourceSlots[index]),
              ),
            ReadOnlyValue(
              label: 'Result gem area',
              value: readOnlyText(calibrated['result_gem_area']),
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
              for (var index = 0; index < grades.length; index++)
                Padding(
                  padding: const EdgeInsets.only(bottom: ShellSpace.xs),
                  child: Row(
                    children: [
                      DraftFieldSlot(
                        slotKey: 'gem-grade-$index',
                        syncToken: draft.seedRevision,
                        child: DraftTextField(
                          label: 'Grade ${index + 1}',
                          value: grades[index],
                          width: 180,
                          onChanged: (value) {
                            final next = [...grades];
                            next[index] = value;
                            _setGrades(next);
                          },
                        ),
                      ),
                      IconButton(
                        key: ValueKey('gem-grade-remove-$index'),
                        tooltip: 'Remove',
                        onPressed: () {
                          final next = [...grades]..removeAt(index);
                          _setGrades(next);
                        },
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
                      key: const ValueKey('new-gem-grade'),
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
                    key: const ValueKey('add-gem-grade'),
                    onPressed: () {
                      final name = _newGrade.text.trim();
                      if (name.isEmpty || grades.contains(name)) {
                        return;
                      }
                      _setGrades([...grades, name]);
                      _newGrade.clear();
                    },
                    child: const Text('Add grade'),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.sm),
              DraftDropdownField(
                key: const ValueKey('gem-start-grade'),
                label: 'Start grade',
                value: _startGrade(),
                options: grades,
                width: 220,
                onChanged: (value) => _setPortable('start_grade', value),
              ),
            ],
          ),
          EditorSection(
            title: 'Empty result',
            children: [
              Wrap(
                spacing: ShellSpace.md,
                runSpacing: ShellSpace.sm,
                children: [
                  DraftDropdownField(
                    key: const ValueKey('gem-empty-mode'),
                    label: 'Empty mode',
                    value: asString(draft.value['empty_mode']),
                    options: _emptyModes,
                    width: 230,
                    onChanged: (value) => _setPortable('empty_mode', value),
                  ),
                  DraftFieldSlot(
                    slotKey: 'gem-empty-streak',
                    syncToken: draft.seedRevision,
                    child: DraftNumberField(
                      label: 'Empty streak',
                      integer: true,
                      value: asNumber(draft.value['empty_streak']),
                      width: 150,
                      onChanged: (value) => _setPortable('empty_streak', value),
                    ),
                  ),
                  DraftFieldSlot(
                    slotKey: 'gem-colored-gap',
                    syncToken: draft.seedRevision,
                    child: DraftNumberField(
                      label: 'Colored gap min',
                      integer: true,
                      value: asNumber(draft.value['colored_gap_min']),
                      width: 170,
                      onChanged: (value) =>
                          _setPortable('colored_gap_min', value),
                    ),
                  ),
                ],
              ),
              DraftSwitchField(
                key: const ValueKey('gem-save-empty-captures'),
                label: 'Save empty captures',
                value: asBool(draft.value['save_empty_captures']) ?? false,
                onChanged: (value) =>
                    _setPortable('save_empty_captures', value),
              ),
            ],
          ),
          EditorSection(
            title: 'Movement',
            children: [
              DraftDropdownField(
                key: const ValueKey('gem-move-mode'),
                label: 'Move mode',
                value: asString(draft.value['move_mode']),
                options: _moveModes,
                width: 220,
                onChanged: (value) => _setPortable('move_mode', value),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

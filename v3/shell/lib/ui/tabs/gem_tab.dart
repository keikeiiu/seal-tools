/// The Gem tab: the PORTABLE gem settings (`settings.gem`) plus a read-only view of the calibrated
/// grade positions and resource slots.
///
/// The split is the config's, not the UI's: `start_grade`, `empty_mode`, `empty_streak`,
/// `colored_gap_min`, `save_empty_captures` and `move_mode` are portable and saved through the
/// `settings:` overlay; `grades` is the composer's fixed grade ladder (the player's live pass: "the
/// config we set are final, don't need feature to add it"), so it is rendered read-only and no save
/// writes it; `gem.grade_positions`, `gem.resource_gems`, `gem.result_gem_area` and the empty-slot
/// signature are machine calibration and are shown, not sent (PROTOCOL §4/§7 — wave 2 owns the
/// calibration canvases).
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../components/foldable_section.dart';
import '../components/grade_ladder.dart';
import '../components/info_row.dart';
import '../components/read_only_panel.dart';
import '../components/shell_prose.dart';
import '../components/surface_panel.dart';
import '../theme.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

const List<String> _emptyModes = [
  'stop',
  'advance_grade',
  'advance_grade_clear',
];
const List<String> _moveModes = ['tuned', 'arduino'];

/// The portable fields this tab owns; everything else in the merged `gem` block is calibration or
/// the fixed grade ladder.
const Set<String> _editableGemKeys = {
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

class _GemTabState extends State<GemTab>
    with ConfigTabState<GemTab>, AutomaticKeepAliveClientMixin<GemTab> {
  @override
  ShellModel get model => widget.model;

  /// Keep the tab alive when the player switches away: an unsaved draft (and its conflict
  /// banner) must survive the switch.
  @override
  bool get wantKeepAlive => true;

  @override
  final ConfigDraft draft = ConfigDraft(
    blockName: 'gem',
    scope: ConfigScope.portableSection,
    editableKeys: _editableGemKeys,
  );

  // ── readers ──────────────────────────────────────────────────────────────────────────────

  /// The composer's fixed grade ladder, from the merged config (this tab never edits it).
  List<String> _grades() =>
      asStringList(asJsonMap(model.config.config['gem'])['grades']);

  String? _startGrade() => asString(draft.value['start_grade']);

  /// The calibrated half, from the merged mirror's top-level `gem` block.
  Map<String, dynamic> _calibrated() => asJsonMap(model.config.config['gem']);

  // ── writers ──────────────────────────────────────────────────────────────────────────────

  void _setPortable(String key, Object? value) => draft.edit((map) {
    if (value == null) {
      map.remove(key);
    } else {
      map[key] = value;
    }
  });

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  /// One read-only geometry row; the value is the stored array (or a dash when unset).
  InfoRow _geometryRow(String label, Object? value) =>
      InfoRow(label: label, value: readOnlyText(value), mono: true);

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final grades = _grades();
    final calibrated = _calibrated();
    final positions = asJsonMap(calibrated['grade_positions']);
    final positionsKeys = positions.keys.toList()..sort();
    final resourceSlots = asJsonList(calibrated['resource_gems']);

    return buildConfigTab(
      title: 'Gem Composer',
      description:
          'The composer\'s grades and behaviour. Saves go through the `settings:` overlay; the '
          'fixed grade ladder and the calibrated geometry are shown, not sent.',
      aside: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ReadOnlyPanel(
            title: 'Calibrated geometry',
            subtitle: 'read-only this wave — draw them in Calibrate',
            openHint: 'read-only',
            rows: [
              if (positionsKeys.isEmpty)
                const InfoRow(label: 'Grade positions', value: '—', mono: true),
              for (final name in positionsKeys)
                _geometryRow('Grade $name', positions[name]),
              if (resourceSlots.isEmpty)
                const InfoRow(label: 'Resource slots', value: '—', mono: true),
              for (var index = 0; index < resourceSlots.length; index++)
                _geometryRow(
                  'Resource slot ${index + 1}',
                  resourceSlots[index],
                ),
              _geometryRow('Result gem area', calibrated['result_gem_area']),
            ],
          ),
          const SurfacePanel(
            title: 'How saving works',
            child: ShellProse(
              'Save writes only the portable half — the grade ladder is fixed and the '
              'positions and slots are calibration. The core merges field-wise and remains '
              'the only writer.',
            ),
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
                grades: grades,
                keyPrefix: 'gem-grade',
                caption: 'Fixed grade ladder',
                captionKey: const ValueKey('gem-grades-caption'),
                start: _startGrade(),
              ),
              const SizedBox(height: ShellSpace.md),
              DraftDropdownField(
                key: const ValueKey('gem-start-grade'),
                label: 'Start grade',
                value: _startGrade(),
                options: grades,
                width: ShellWidth.fieldLg,
                onChanged: (value) => _setPortable('start_grade', value),
              ),
              const SizedBox(height: ShellSpace.xs),
              const ShellProse(
                'The run starts at this grade and advances up the ladder as grades complete.',
              ),
            ],
          ),
          EditorSection(
            title: 'Empty result',
            children: [
              const ShellProse(
                'After Empty streak consecutive unreadable results the run acts on Empty mode: '
                'stop, advance a grade, or advance after clearing the resource slots first. '
                'Colored gap min is the colour-composition fallback threshold (0–255).',
              ),
              const SizedBox(height: ShellSpace.md),
              Wrap(
                spacing: ShellSpace.md,
                runSpacing: ShellSpace.sm,
                children: [
                  DraftDropdownField(
                    key: const ValueKey('gem-empty-mode'),
                    label: 'Empty mode',
                    value: asString(draft.value['empty_mode']),
                    options: _emptyModes,
                    width: ShellWidth.picker,
                    onChanged: (value) => _setPortable('empty_mode', value),
                  ),
                  DraftFieldSlot(
                    slotKey: 'gem-empty-streak',
                    syncToken: draft.seedRevision,
                    child: DraftNumberField(
                      label: 'Empty streak',
                      integer: true,
                      value: asNumber(draft.value['empty_streak']),
                      width: ShellWidth.fieldSm,
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
                      width: ShellWidth.fieldMd,
                      onChanged: (value) =>
                          _setPortable('colored_gap_min', value),
                    ),
                  ),
                ],
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
                width: ShellWidth.fieldLg,
                onChanged: (value) => _setPortable('move_mode', value),
              ),
              const SizedBox(height: ShellSpace.xs),
              const ShellProse(
                'tuned sends hand-tuned relative moves (turn off Enhance pointer precision); '
                'arduino places the cursor on each route\'s calibrated destination point.',
              ),
            ],
          ),
          FoldableSection(
            title: 'Advanced',
            description: 'Empty-check diagnostics',
            children: [
              DraftSwitchField(
                key: const ValueKey('gem-save-empty-captures'),
                label: 'Save empty captures',
                value: asBool(draft.value['save_empty_captures']) ?? false,
                onChanged: (value) =>
                    _setPortable('save_empty_captures', value),
              ),
              const ShellProse(
                'Writes each empty-check crop under logs/captures and one detail line to '
                'gem.log. Debug only.',
              ),
            ],
          ),
        ],
      ),
    );
  }
}

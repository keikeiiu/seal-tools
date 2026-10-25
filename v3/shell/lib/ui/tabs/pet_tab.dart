/// The Pet tab: the PORTABLE feeder settings (`settings.pet`) plus a read-only view of the machine
/// rows and calibration crops the core persisted.
///
/// The split is the config's: `items_per_minute` and `pet_slot_occupied_above` are portable and
/// saved through the `settings:` overlay; the per-row geometry (`slots[]`), crops, bag grid and food
/// slots are machine calibration (wave 2) and are shown, not sent. The four-row CARD surface is the
/// Cards tab's — this tab never repeats it, it shows what the core has configured.
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

/// The portable pet fields this tab owns.
const Set<String> _editablePetKeys = {
  'items_per_minute',
  'pet_slot_occupied_above',
};

class PetTab extends StatefulWidget {
  const PetTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<PetTab> createState() => _PetTabState();
}

class _PetTabState extends State<PetTab>
    with ConfigTabState<PetTab>, AutomaticKeepAliveClientMixin<PetTab> {
  @override
  ShellModel get model => widget.model;

  @override
  bool get wantKeepAlive => true;

  @override
  final ConfigDraft draft = ConfigDraft(
    blockName: 'pet',
    scope: ConfigScope.portableSection,
    editableKeys: _editablePetKeys,
  );

  // ── readers ──────────────────────────────────────────────────────────────────────────────

  /// The merged machine block, from the mirror's top-level `pet`.
  Map<String, dynamic> _machine() => asJsonMap(model.config.config['pet']);

  List<Map<String, dynamic>> _rows() => [
    for (final entry in asJsonList(_machine()['slots'])) asJsonMap(entry),
  ];

  bool _hasCrop(Object? value) => value is String && value.isNotEmpty;

  String _yesNo(bool value) => value ? 'configured' : 'not configured';

  void _setPortable(String key, num value) => draft.edit((map) {
    map[key] = value;
  });

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final rows = _rows();
    final machine = _machine();
    final queue = asJsonList(machine['queue']);
    final foodSlots = asJsonList(machine['food_slots']);

    return buildConfigTab(
      title: 'Pet Feeder',
      description:
          'The feeder\'s portable settings. Saves go through the `settings:` overlay; the machine '
          'rows and crops stay read-only until the pet calibration surface lands.',
      readOnly: [
        EditorSection(
          title: 'Machine rows — read-only this wave',
          children: [
            if (rows.isEmpty)
              const ReadOnlyValue(
                label: 'Rows',
                value:
                    'No rows configured yet — the core will publish them once the pet '
                    'calibration is saved (wave 2).',
                key: ValueKey('pet-no-rows'),
              ),
            for (var index = 0; index < rows.length; index++)
              ReadOnlyValue(
                label: 'Row ${index + 1}',
                value: _rowText(rows[index]),
              ),
            ReadOnlyValue(
              label: 'Queue',
              value: queue.isEmpty
                  ? 'No pets queued'
                  : '${queue.length} queued entr${queue.length == 1 ? 'y' : 'ies'}',
            ),
          ],
        ),
        EditorSection(
          title: 'Calibration crops — read-only this wave',
          children: [
            ReadOnlyValue(
              label: 'Boarding X crop',
              value: _yesNo(_hasCrop(machine['close_button_png'])),
            ),
            ReadOnlyValue(
              label: 'Food icon crop',
              value: _yesNo(_hasCrop(machine['food_icon_png'])),
            ),
            ReadOnlyValue(
              label: 'Bag grid',
              value: readOnlyText(machine['bag_grid']),
            ),
            ReadOnlyValue(
              label: 'Food slots',
              value: foodSlots.isEmpty
                  ? '—'
                  : '${foodSlots.length} slot${foodSlots.length == 1 ? '' : 's'}',
            ),
            ReadOnlyValue(
              label: 'Return slot',
              value: readOnlyText(machine['return_slot']),
            ),
            ReadOnlyValue(
              label: 'Food load mode',
              value: readOnlyText(machine['food_load_mode']),
            ),
          ],
        ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EditorSection(
            title: 'Feeding',
            children: [
              DraftFieldSlot(
                slotKey: 'pet-items-per-minute',
                syncToken: draft.seedRevision,
                child: DraftNumberField(
                  label: 'Items per minute (fallback rate)',
                  helper: 'used only when the boarding window\'s rate line cannot be read',
                  integer: true,
                  value: asNumber(draft.value['items_per_minute']),
                  width: 260,
                  onChanged: (value) => _setPortable('items_per_minute', value),
                ),
              ),
              const SizedBox(height: ShellSpace.md),
              DraftFieldSlot(
                slotKey: 'pet-occupied-above',
                syncToken: draft.seedRevision,
                child: DraftNumberField(
                  label: 'Occupied above (0–1 distance)',
                  helper: 'how unlike the empty reference a slot must look before it counts as occupied',
                  value: asNumber(draft.value['pet_slot_occupied_above']),
                  width: 260,
                  onChanged: (value) =>
                      _setPortable('pet_slot_occupied_above', value),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  String _rowText(Map<String, dynamic> row) {
    final enabled = asBool(row['enabled']) ?? true;
    final stacks = asNumber(row['stacks']);
    final strip = readOnlyText(row['feeder_strip']);
    final parts = <String>[
      enabled ? 'enabled' : 'disabled',
      if (stacks != null) 'stacks $stacks',
      'feeder strip $strip',
    ];
    final label = asString(row['label']);
    if (label != null && label.isNotEmpty) {
      parts.insert(0, label);
    }
    return parts.join(' · ');
  }
}

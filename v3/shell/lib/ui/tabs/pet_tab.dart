/// The Pet tab: the feeder's SETUP, which the core reads — the machine rows (`pet.slots`: enabled
/// and stacks only), the queue (removal only), the bag food cells (page + 8×8 grid), the return slot,
/// the food-load mode and the run's timing/read thresholds — plus the portable `settings.pet`
/// fallbacks, all saved together in ONE `config.apply`.
///
/// What this tab deliberately does NOT do is capture (PLAN-V3-CALIBRATION ⚑10): a row's rects, the
/// reference crops, the page-tab points and the queue pet icons come from the live game, so they
/// arrive through the Calibrate tab or the run and stay READ-ONLY displays here; rows and queue
/// entries cannot be added or removed until that session exists. The read-only fields ride the save
/// verbatim — a list-typed config field replaces wholesale (PROTOCOL §7), so a row's marks, crops and
/// a queue entry's icon are echoed back exactly and a setup edit can never drop them.
///
/// Session readiness is the CORE's (`pet.preflight`, v2's Setup-so-far/Ready-to-run): this tab renders
/// its two checklists and its ready lines exactly as they arrive (no rule and no sentence is composed
/// here; `ℹ` marks an item the core says it did NOT check), reading them on open + Refresh and once
/// after a Save or a boarding tick. The per-row "boarding now" tick writes ONE config field through
/// `pet.set_boarding` (never the draft, never `config.apply`) and renders the MIRROR the core's
/// `config.changed` updates; while the rows draft is dirty the tick is disabled, because an A1 write
/// beside an unsaved draft would leave the two disagreeing.
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../app/pet_preflight.dart';
import '../theme.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

/// The portable pet fields (the `settings.pet` projection). They stay portable: a save writes them
/// under `settings`, never into the machine `pet` block.
const Set<String> _portablePetKeys = {
  'items_per_minute',
  'pet_slot_occupied_above',
};

/// The machine setup fields this tab owns in the top-level `pet` block. The calibration/session
/// fields (`menu_button`, `feed_icon`, `close_button`, `page_tabs`, `bag_grid`, `bag_slot`, the
/// `*_png` crops, `boarding_running`, `boarding_window_may_be_open`, `max_button`) have their own
/// writers (Calibrate / the run) and are displayed, never sent.
const Set<String> _machinePetKeys = {
  'slots',
  'queue',
  'food_slots',
  'return_slot',
  'food_load_mode',
  'reload_on_start',
  'action_wait_ms',
  'wait_after_empty_minutes',
  'feeder_count_left_fraction',
  'feeder_count_min_score',
};

/// The two accepted `food_load_mode` values (`seal-domain`'s `food_load::ALL`). `right_click` lets
/// the game pick the box; `drag` names it, which needs the row's feeder strip and a board that can
/// drag. `right_click` is the core's default when the field is unset.
const List<String> _foodLoadModes = ['right_click', 'drag'];
const String _defaultFoodLoadMode = 'right_click';

/// The boarding bag is an 8×8 grid, row-major: cell 0 is top-left (`BagGrid::centres`' order).
const int _bagColumns = 8;
const int _bagSlots = _bagColumns * _bagColumns;

/// The bag's ITEM1–ITEM3 pages; the config stores page indexes 0-based and the UI shows them 1-based.
const int _bagPages = 3;

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

  /// The portable half (`settings.pet`).
  late final ConfigDraft _portableDraft = ConfigDraft(
    blockName: 'pet',
    scope: ConfigScope.portableSection,
    editableKeys: _portablePetKeys,
  );

  /// The machine setup half (the owned top-level `pet` block).
  late final ConfigDraft _machineDraft = ConfigDraft(
    blockName: 'pet',
    scope: ConfigScope.ownedBlock,
    editableKeys: _machinePetKeys,
  );

  /// ONE Save over both halves, one `config.apply` carrying `pet` (machine) and `settings.pet`
  /// (portable) — the Setup tab's pattern.
  late final ConfigDraftGroup _group = _PetDraftGroup(
    label: 'pet',
    parts: [_machineDraft, _portableDraft],
  );

  @override
  ConfigDraft get draft => _portableDraft;

  @override
  ConfigDraftGroup get draftGroup => _group;

  /// The bag page the food-cell grid is showing. View state, not config.
  int _foodPage = 0;

  // The return slot as the UI shows it: page 1-based, cell 0..63. A local mirror of the draft so a
  // half-typed pair can be held without writing it (see [_writeReturnSlot]).
  int? _returnPage;
  int? _returnCell;
  int _returnSeedRevision = -1;

  /// The core's session checklists (`pet.preflight`), read on open + Refresh only; refreshed once
  /// after a successful Save or boarding tick because those are what change the saved config.
  CoreActionResult<PetPreflightResult>? _preflight;
  bool _loadingPreflight = true; // initState kicks the first read.

  /// True while one `pet.set_boarding` request is in flight (all ticks disabled meanwhile).
  bool _settingBoarding = false;

  /// The core's line for a refused boarding tick, rendered verbatim under the rows.
  String? _boardingError;

  /// The last config revision this tab rendered. The BOARDING ticks are mirror state (the core's A1
  /// write + `config.changed`), not draft state, so the tab must rebuild when the mirror does.
  int? _seenConfigRev;

  @override
  void initState() {
    super.initState();
    _seenConfigRev = model.config.rev;
    model.config.addListener(_onMirrorChanged);
    unawaited(_loadPreflight());
  }

  @override
  void dispose() {
    model.config.removeListener(_onMirrorChanged);
    _group.dispose();
    super.dispose();
  }

  void _onMirrorChanged() {
    final rev = model.config.rev;
    if (rev == _seenConfigRev) {
      return;
    }
    _seenConfigRev = rev;
    if (!mounted) {
      return;
    }
    // No command here: a boarding tick or a mid-run write changed the mirror, and the rows/tick
    // values rendered from it must follow. The preflight is NOT re-read here (open/Refresh/Save/tick
    // are its triggers), so an unrelated config change cannot cost a command.
    setState(() {});
  }

  /// The Save also refreshes the checklists: `pet.preflight` describes the SAVED config, so the
  /// moment it can change is the save that wrote it.
  @override
  Future<void> save() async {
    await super.save();
    if (!mounted) {
      return;
    }
    if (outcome?.saved ?? false) {
      await _loadPreflight();
    }
  }

  // ── readers ──────────────────────────────────────────────────────────────────────────────

  /// The merged machine `pet` block — also where the calibration this tab only DISPLAYS lives.
  Map<String, dynamic> _machine() => asJsonMap(model.config.config['pet']);

  /// The editable rows, from the machine draft so an unsaved tick/stacks edit renders immediately.
  List<Map<String, dynamic>> _rows() => [
    for (final entry in asJsonList(_machineDraft.value['slots']))
      asJsonMap(entry),
  ];

  List<Map<String, dynamic>> _queue() => [
    for (final entry in asJsonList(_machineDraft.value['queue']))
      asJsonMap(entry),
  ];

  /// The marked food cells as `[page, cell]` pairs, from the machine draft.
  List<List<int>> _foodSlots() {
    final slots = <List<int>>[];
    for (final entry in asJsonList(_machineDraft.value['food_slots'])) {
      final pair = asJsonList(entry);
      if (pair.length != 2) {
        continue;
      }
      final page = asNumber(pair[0])?.round();
      final cell = asNumber(pair[1])?.round();
      if (page != null && cell != null) {
        slots.add([page, cell]);
      }
    }
    return slots;
  }

  /// The calibrated boarding-bag rectangle when the core would accept it — the same rule as
  /// `BagGrid::is_valid_rect`: `[x, y, w, h]` with a positive width and height. The food-cell grid is
  /// only usable with it; the Calibrate tab owns the mark.
  List<int>? _bagGridRect() {
    final values = asJsonList(_machine()['bag_grid']);
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

  bool _hasCrop(Object? value) => value is String && value.isNotEmpty;

  String _yesNo(bool value) => value ? 'configured' : 'not configured';

  /// The effective food-load mode: an unset field means the core's `right_click` default.
  String get _foodLoadMode =>
      asString(_machineDraft.value['food_load_mode']) ?? _defaultFoodLoadMode;

  /// Mirrors the draft's return slot into the two fields when the draft was re-seeded (seed, Cancel,
  /// a save's echo, an external reconcile) — never mid-typing, because an edit does not bump the
  /// draft's seed revision.
  void _syncReturnFields() {
    if (_returnSeedRevision == _machineDraft.seedRevision) {
      return;
    }
    _returnSeedRevision = _machineDraft.seedRevision;
    final pair = asJsonList(_machineDraft.value['return_slot']);
    if (pair.length != 2) {
      _returnPage = null;
      _returnCell = null;
      return;
    }
    final page = asNumber(pair[0])?.round();
    final cell = asNumber(pair[1])?.round();
    _returnPage = page == null ? null : page + 1;
    _returnCell = cell;
  }

  String _rowMarksText(Map<String, dynamic> row) {
    final boarding = asBool(row['boarding_running']) == true;
    final crop = _hasCrop(row['pet_slot_empty_png'])
        ? 'empty-slot crop saved'
        : 'empty-slot crop not saved';
    return 'start/end ${readOnlyText(row['toggle_label'])} · '
        'pet slot ${readOnlyText(row['boarding_pet_slot'])} · '
        'food strip ${readOnlyText(row['feeder_strip'])} · '
        '$crop · boarding ${boarding ? 'running' : 'idle'}';
  }

  // ── writers ──────────────────────────────────────────────────────────────────────────────

  /// Portable-half edits also rebuild this State: the tab's stale caption and tick gating are built
  /// here, not by a draft listener, so a bare draft edit would leave them one frame stale (the
  /// Pet-tab count-line rule).
  void _setPortable(String key, num value) => setState(() {
    _portableDraft.edit((map) {
      map[key] = value;
    });
  });

  /// Machine-half edits go through here so the tab rebuilds: the draft notifies its own
  /// listeners, but the rendered count/marks are built by this State, not by a draft listener.
  void _setMachine(String key, Object? value) {
    setState(() {
      _machineDraft.edit((map) {
        map[key] = value;
      });
    });
  }

  void _setRow(int index, String field, Object? value) => setState(() {
    _machineDraft.edit((block) {
      final rows = asJsonList(block['slots']);
      if (index < 0 || index >= rows.length) {
        return;
      }
      asJsonMap(rows[index])[field] = value;
    });
  });

  void _removeQueued(int index) => setState(() {
    _machineDraft.edit((block) {
      final queue = asJsonList(block['queue']);
      if (index < 0 || index >= queue.length) {
        return;
      }
      queue.removeAt(index);
    });
  });

  void _toggleFoodCell(int cell) {
    final slots = _foodSlots();
    final existing = slots.indexWhere(
      (pair) => pair[0] == _foodPage && pair[1] == cell,
    );
    if (existing >= 0) {
      slots.removeAt(existing);
    } else {
      slots.add([_foodPage, cell]);
    }
    slots.sort((left, right) {
      final byPage = left[0].compareTo(right[0]);
      return byPage != 0 ? byPage : left[1].compareTo(right[1]);
    });
    _setMachine('food_slots', slots);
  }

  void _setReturnPage(num value) {
    final page = value.round();
    if (page < 1 || page > _bagPages) {
      return;
    }
    setState(() => _returnPage = page);
    _writeReturnSlot();
  }

  void _setReturnCell(num value) {
    final cell = value.round();
    if (cell < 0 || cell >= _bagSlots) {
      return;
    }
    setState(() => _returnCell = cell);
    _writeReturnSlot();
  }

  /// Writes the pair only when BOTH halves are known: the run right-clicks the cell the pair names,
  /// so a half-typed return slot must never reach the config. The page is stored 0-based.
  void _writeReturnSlot() {
    final page = _returnPage;
    final cell = _returnCell;
    if (page == null || cell == null) {
      return;
    }
    _setMachine('return_slot', [page - 1, cell]);
  }

  // ── session readiness: the checklists + the boarding ticks ───────────────────────────────

  /// Reads the core's Setup-so-far/Ready-to-run checklists. A READ on open, Refresh, a successful
  /// Save and a successful boarding tick only — never per rebuild (no polling).
  Future<void> _loadPreflight() async {
    if (!_loadingPreflight) {
      setState(() => _loadingPreflight = true);
    }
    final result = await model.petPreflight();
    if (!mounted) {
      return;
    }
    setState(() {
      _loadingPreflight = false;
      _preflight = result;
    });
  }

  /// The machine rows draft is dirty: the tick is an A1 core write outside the draft, so ticking
  /// then would make the draft disagree with the mirror (and the scaffold would flag it). The
  /// smallest honest UI is to disable the tick and say why.
  bool get _rowsDirty => _machineDraft.isDirty;

  /// ANY unsaved edit blocks the tick: a `config.changed` while a dirty part exists would be marked
  /// as an external change against that part even when the write touched only the machine block, so
  /// the tick would present the player with a confusing reconcile banner for their own action.
  bool get _draftDirty => _group.isDirty;

  /// The reason beside a blocked tick. The pinned wording names the rows, so it is kept when they
  /// are the dirty part; a portable-only edit still blocks the tick, with its own sentence.
  String get _tickBlockedReason =>
      _rowsDirty ? 'Save the row edits first' : 'Save the edits first';

  /// One row's `boarding_running`, from the MIRROR: the core's A1 write is the only writer (the run
  /// also writes it mid-run), so the tab renders what the config says rather than a local guess.
  bool _boardingRunning(int index) {
    final rows = asJsonList(_machine()['slots']);
    if (index < 0 || index >= rows.length) {
      return false;
    }
    return asBool(asJsonMap(rows[index])['boarding_running']) ?? false;
  }

  /// Toggles one row's boarding state through `pet.set_boarding` — never a config.apply and never the
  /// draft. `row` is 1-based (the wire's numbering). On success the mirror updates via the core's
  /// `config.changed` and the checklists are re-read once (the tick may change readiness).
  Future<void> _setBoarding(int row, bool running) async {
    if (_settingBoarding || _draftDirty) {
      return;
    }
    setState(() {
      _settingBoarding = true;
      _boardingError = null;
    });
    final result = await model.setPetBoarding(row: row, running: running);
    if (!mounted) {
      return;
    }
    setState(() {
      _settingBoarding = false;
      if (!result.ok) {
        _boardingError = result.errorLine;
      }
    });
    if (result.ok) {
      await _loadPreflight();
    }
  }

  /// A checklist row's marker: `✓` passed, `✗` failed, `ℹ` INFORMATIONAL. An informational item is
  /// one the core says it did NOT check (its detail starts "not checked by preflight"): it is
  /// `ok:true` because it is not a failure, but drawing it as a passed check would overstate what
  /// the core proved. The docs name this convention; the item's own text is left untouched.
  String _checkMarker(PetChecklistItem item) {
    if (!item.ok) {
      return '✗';
    }
    final detail = item.detail;
    if (detail != null && detail.startsWith('not checked by preflight')) {
      return 'ℹ';
    }
    return '✓';
  }

  /// One checklist list as rendered: the marker, the core's label and its detail. The shell adds no
  /// wording of its own and infers no rules from labels or counts — `ok` + `detail` are the contract,
  /// and the core's `lines` are rendered verbatim (untouched) elsewhere.
  String _checklistText(List<PetChecklistItem> items) {
    if (items.isEmpty) {
      return '(no checks)';
    }
    return [
      for (final item in items)
        '${_checkMarker(item)} ${item.label}'
            '${item.detail == null || item.detail!.isEmpty ? '' : ' — ${item.detail}'}',
    ].join('\n');
  }

  /// The checklists describe the SAVED config, so while ANY part of this tab's draft is dirty they
  /// are marked stale (a save re-reads them). This is broader than [_rowsDirty]: the portable
  /// `settings.pet` fields feed the same preflight, not just the machine rows.
  List<Widget> _preflightSection(ThemeData theme) {
    final result = _preflight;
    final stale = _group.isDirty;
    final value = result?.value;
    return [
      Row(
        children: [
          OutlinedButton(
            key: const ValueKey('pet-preflight-refresh'),
            onPressed: _loadingPreflight ? null : _loadPreflight,
            child: const Text('Refresh checklists'),
          ),
          if (value != null) ...[
            const SizedBox(width: ShellSpace.sm),
            Expanded(
              child: Text(
                value.ready
                    ? 'Ready to run.'
                    : 'Not ready yet — see the lines below.',
                key: const ValueKey('pet-preflight-ready'),
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: value.ready ? theme.colorScheme.primary : null,
                ),
              ),
            ),
          ],
        ],
      ),
      if (stale)
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xs),
          child: Text(
            'Based on the saved config — Save to refresh',
            key: const ValueKey('pet-preflight-stale'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ),
      if (result == null)
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xs),
          child: Text(
            _loadingPreflight ? 'Reading the pet setup…' : 'Not read yet.',
            key: const ValueKey('pet-preflight-pending'),
            style: theme.textTheme.bodySmall,
          ),
        )
      else if (!result.ok)
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xs),
          child: Text(
            result.errorLine ?? 'The core could not check the pet setup.',
            key: const ValueKey('pet-preflight-error'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.error,
            ),
          ),
        )
      else if (value != null) ...[
        const SizedBox(height: ShellSpace.sm),
        Text('Calibration', style: theme.textTheme.labelLarge),
        Text(
          _checklistText(value.calibration),
          key: const ValueKey('pet-preflight-calibration'),
          style: theme.textTheme.bodySmall,
        ),
        const SizedBox(height: ShellSpace.xs),
        Text('Setup so far', style: theme.textTheme.labelLarge),
        Text(
          _checklistText(value.session),
          key: const ValueKey('pet-preflight-session'),
          style: theme.textTheme.bodySmall,
        ),
        const SizedBox(height: ShellSpace.xs),
        Text(
          value.lines.isEmpty ? '' : value.lines.join('\n'),
          key: const ValueKey('pet-preflight-lines'),
          style: theme.textTheme.bodySmall,
        ),
      ],
    ];
  }

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    _syncReturnFields();
    final theme = Theme.of(context);
    final machine = _machine();
    final rows = _rows();
    final queue = _queue();
    final bagGrid = _bagGridRect();

    return buildConfigTab(
      title: 'Pet Feeder',
      description:
          'The feeder\'s setup: rows, queue, bag food cells and the run\'s mode and timings, '
          'saved together. The calibrated marks and reference crops are drawn on the Calibrate '
          'tab or learned by the run, and stay read-only here.',
      readOnly: [
        EditorSection(
          title: 'Calibrated marks & crops — read-only (Calibrate tab)',
          children: [
            ReadOnlyValue(
              label: 'Menu (目錄) point',
              value: readOnlyText(machine['menu_button']),
            ),
            ReadOnlyValue(
              label: 'Pet feed icon point',
              value: readOnlyText(machine['feed_icon']),
            ),
            ReadOnlyValue(
              label: 'Boarding X point',
              value: readOnlyText(machine['close_button']),
            ),
            ReadOnlyValue(
              label: 'Boarding bag grid',
              value: readOnlyText(machine['bag_grid']),
            ),
            ReadOnlyValue(
              label: 'Boarding bag slot',
              value: readOnlyText(machine['bag_slot']),
            ),
            ReadOnlyValue(
              label: 'Bag page tab points',
              value: readOnlyText(machine['page_tabs']),
            ),
            ReadOnlyValue(
              label: 'MAX button (pet override)',
              value: readOnlyText(machine['max_button']),
            ),
            ReadOnlyValue(
              label: 'Boarding X crop',
              value: _yesNo(_hasCrop(machine['close_button_png'])),
            ),
            ReadOnlyValue(
              label: 'Food icon crop',
              value: _yesNo(_hasCrop(machine['food_icon_png'])),
            ),
            ReadOnlyValue(
              label: 'Food cells used (run reading)',
              value: readOnlyText(machine['food_slots_used']),
            ),
          ],
        ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EditorSection(
            title: 'Setup so far & ready to run (from the saved config)',
            children: _preflightSection(theme),
          ),
          EditorSection(
            title: 'Breeding rows',
            children: [
              if (rows.isEmpty)
                const ReadOnlyValue(
                  key: ValueKey('pet-no-rows'),
                  label: 'Rows',
                  value:
                      'No breeding rows are configured yet — a migrated v2 config '
                      'carries them; adding a row needs the pet capture session (OPEN).',
                ),
              for (var index = 0; index < rows.length; index++)
                _rowEditor(index, rows[index]),
              if (_boardingError != null)
                Padding(
                  padding: const EdgeInsets.only(bottom: ShellSpace.xs),
                  child: Text(
                    _boardingError!,
                    key: const ValueKey('pet-boarding-error'),
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: theme.colorScheme.error,
                    ),
                  ),
                ),
              Text(
                'Rows come from the pet capture session (OPEN): tick one and set its '
                'stack count here; its rects and crops are read-only. Rows cannot be '
                'added or removed here.',
                key: const ValueKey('pet-rows-caption'),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
          EditorSection(
            title: 'Queue',
            children: [
              if (queue.isEmpty)
                Text(
                  'No pets queued — the return slot is then the fallback the run '
                  'right-clicks.',
                  key: const ValueKey('pet-queue-empty'),
                  style: theme.textTheme.bodySmall,
                ),
              for (var index = 0; index < queue.length; index++)
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        _queueText(queue[index]),
                        key: ValueKey('pet-queue-entry-$index'),
                        style: theme.textTheme.bodyMedium,
                      ),
                    ),
                    OutlinedButton(
                      key: ValueKey('pet-queue-remove-$index'),
                      onPressed: () => _removeQueued(index),
                      child: const Text('Remove'),
                    ),
                  ],
                ),
              const SizedBox(height: ShellSpace.xs),
              Text(
                'Removing an entry saves the shortened queue. Adding one needs the pet '
                'capture session (OPEN) — it crops the pet icon from the live bag.',
                key: const ValueKey('pet-queue-caption'),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
          EditorSection(
            title: 'Bag food cells',
            children: _foodCellEditor(theme, bagGrid),
          ),
          EditorSection(
            title: 'Return slot',
            children: [
              DraftFieldSlot(
                slotKey: 'pet-return-page',
                syncToken: _machineDraft.seedRevision,
                child: DraftNumberField(
                  label: 'Bag page (ITEM1 = 1)',
                  helper: 'the page the returned pet lands on',
                  integer: true,
                  value: _returnPage,
                  width: 200,
                  onChanged: _setReturnPage,
                ),
              ),
              const SizedBox(height: ShellSpace.sm),
              DraftFieldSlot(
                slotKey: 'pet-return-cell',
                syncToken: _machineDraft.seedRevision,
                child: DraftNumberField(
                  label: 'Cell (0–63, row-major)',
                  helper: 'both values are needed — the pair saves together',
                  integer: true,
                  value: _returnCell,
                  width: 220,
                  onChanged: _setReturnCell,
                ),
              ),
            ],
          ),
          EditorSection(
            title: 'Food loading',
            children: [
              DraftDropdownField(
                key: const ValueKey('pet-food-load-mode'),
                label: 'Food load mode',
                value: _foodLoadMode,
                options: _foodLoadModes,
                width: 240,
                onChanged: (value) {
                  if (value != null) {
                    _setMachine('food_load_mode', value);
                  }
                },
              ),
              const SizedBox(height: ShellSpace.xs),
              Text(
                'right_click lets the game choose the box; drag names it, which '
                'needs the row food strip and a board whose firmware can drag.',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
              DraftSwitchField(
                key: const ValueKey('pet-reload-on-start'),
                label: 'Reload on start',
                helper:
                    'open and close the boarding window once when a run begins',
                value: asBool(_machineDraft.value['reload_on_start']) ?? false,
                onChanged: (value) => _setMachine('reload_on_start', value),
              ),
            ],
          ),
          EditorSection(
            title: 'Timing & read thresholds',
            children: [
              DraftFieldSlot(
                slotKey: 'pet-action-wait',
                syncToken: _machineDraft.seedRevision,
                child: DraftNumberField(
                  label: 'Action wait (ms)',
                  helper: 'core default 1200',
                  integer: true,
                  value: asNumber(_machineDraft.value['action_wait_ms']),
                  width: 220,
                  onChanged: (value) =>
                      _setMachine('action_wait_ms', value.round()),
                ),
              ),
              const SizedBox(height: ShellSpace.sm),
              DraftFieldSlot(
                slotKey: 'pet-wait-after-empty',
                syncToken: _machineDraft.seedRevision,
                child: DraftNumberField(
                  label: 'Wait after empty (minutes)',
                  helper: 'core default 5',
                  integer: true,
                  value: asNumber(
                    _machineDraft.value['wait_after_empty_minutes'],
                  ),
                  width: 240,
                  onChanged: (value) =>
                      _setMachine('wait_after_empty_minutes', value.round()),
                ),
              ),
              const SizedBox(height: ShellSpace.sm),
              DraftFieldSlot(
                slotKey: 'pet-feeder-left-fraction',
                syncToken: _machineDraft.seedRevision,
                child: DraftNumberField(
                  label: 'Count read — left fraction (0–1)',
                  helper: 'core default 0.40',
                  value: asNumber(
                    _machineDraft.value['feeder_count_left_fraction'],
                  ),
                  width: 280,
                  onChanged: (value) =>
                      _setMachine('feeder_count_left_fraction', value),
                ),
              ),
              const SizedBox(height: ShellSpace.sm),
              DraftFieldSlot(
                slotKey: 'pet-feeder-min-score',
                syncToken: _machineDraft.seedRevision,
                child: DraftNumberField(
                  label: 'Count read — min score (0–1)',
                  helper: 'core default 0.75',
                  value: asNumber(
                    _machineDraft.value['feeder_count_min_score'],
                  ),
                  width: 280,
                  onChanged: (value) =>
                      _setMachine('feeder_count_min_score', value),
                ),
              ),
            ],
          ),
          EditorSection(
            title: 'Feeding (portable — saved in settings.pet)',
            children: [
              DraftFieldSlot(
                slotKey: 'pet-items-per-minute',
                syncToken: _portableDraft.seedRevision,
                child: DraftNumberField(
                  label: 'Items per minute (fallback rate)',
                  helper: 'used only when the boarding window\'s rate line cannot be read',
                  integer: true,
                  value: asNumber(_portableDraft.value['items_per_minute']),
                  width: 260,
                  onChanged: (value) => _setPortable('items_per_minute', value),
                ),
              ),
              const SizedBox(height: ShellSpace.md),
              DraftFieldSlot(
                slotKey: 'pet-occupied-above',
                syncToken: _portableDraft.seedRevision,
                child: DraftNumberField(
                  label: 'Occupied above (0–1 distance)',
                  helper:
                      'how unlike the empty reference a slot must look before it '
                      'counts as occupied',
                  value: asNumber(
                    _portableDraft.value['pet_slot_occupied_above'],
                  ),
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

  /// One row: the two editable fields and the marks/crops the capture session owns.
  Widget _rowEditor(int index, Map<String, dynamic> row) {
    final enabled = asBool(row['enabled']) ?? true;
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          DraftSwitchField(
            key: ValueKey('pet-row-$index-enabled'),
            label: 'Row ${index + 1} enabled',
            helper: 'a ticked row is fed; an unticked one is ignored',
            value: enabled,
            onChanged: (value) => _setRow(index, 'enabled', value),
          ),
          // v2's per-row "boarding now" tick. It is NOT a draft edit: `pet.set_boarding` writes the
          // one field through the core (A1) so the run's own mid-run updates cannot clobber it. It
          // renders the MIRROR, and is disabled while row edits are unsaved (the draft would
          // disagree with the mirror the core is writing).
          CheckboxListTile(
            key: ValueKey('pet-boarding-tick-${index + 1}'),
            value: _boardingRunning(index),
            dense: true,
            contentPadding: EdgeInsets.zero,
            controlAffinity: ListTileControlAffinity.leading,
            title: const Text('Boarding now'),
            subtitle: _draftDirty ? Text(_tickBlockedReason) : null,
            onChanged: _draftDirty || _settingBoarding
                ? null
                : (value) => _setBoarding(index + 1, value ?? false),
          ),
          DraftFieldSlot(
            slotKey: 'pet-row-$index-stacks',
            syncToken: _machineDraft.seedRevision,
            child: DraftNumberField(
              label: 'Stacks per reload',
              helper: '2 on the free row, 5 on a paid one — the food strip is divided by it',
              integer: true,
              value: asNumber(row['stacks']),
              width: 260,
              onChanged: (value) => _setRow(index, 'stacks', value.round()),
            ),
          ),
          const SizedBox(height: ShellSpace.xs),
          ReadOnlyValue(
            key: ValueKey('pet-row-$index-marks'),
            label: 'Row ${index + 1} marks — read-only',
            value: _rowMarksText(row),
          ),
        ],
      ),
    );
  }

  /// The page picker plus the 8×8 grid of the bag page it names. Gated on the calibrated boarding
  /// bag grid: without it the run has no cell to right-click, so the editor is disabled with the
  /// core's own start refusal.
  List<Widget> _foodCellEditor(ThemeData theme, List<int>? bagGrid) {
    if (bagGrid == null) {
      return [
        Text(
          "The boarding bag's grid isn't calibrated — save the Boarding bag grid "
          'and Boarding bag slot on the Calibrate tab. (Its own, not the shop\'s.)',
          key: const ValueKey('pet-food-not-calibrated'),
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ];
    }
    final slots = _foodSlots();
    final marked = {
      for (final pair in slots)
        if (pair[0] == _foodPage) pair[1],
    };
    final total = slots.length;
    return [
      Text(
        'The cells holding pet food. A page is the bag\'s ITEM tab the run opens '
        'before clicking a cell.',
        style: theme.textTheme.bodySmall,
      ),
      const SizedBox(height: ShellSpace.sm),
      Wrap(
        spacing: ShellSpace.sm,
        children: [
          for (var page = 0; page < _bagPages; page++)
            ChoiceChip(
              key: ValueKey('pet-food-page-$page'),
              label: Text('ITEM${page + 1}'),
              selected: _foodPage == page,
              onSelected: (_) => setState(() => _foodPage = page),
            ),
        ],
      ),
      const SizedBox(height: ShellSpace.xs),
      Text(
        '${marked.length} of $_bagSlots cells marked on this page · '
        '$total marked overall',
        key: const ValueKey('pet-food-count'),
        style: theme.textTheme.bodySmall,
      ),
      const SizedBox(height: ShellSpace.xs),
      for (var row = 0; row < _bagColumns; row++)
        Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (var column = 0; column < _bagColumns; column++)
              _foodCell(
                theme,
                row * _bagColumns + column,
                marked.contains(row * _bagColumns + column),
              ),
          ],
        ),
      const SizedBox(height: ShellSpace.xs),
      Text(
        'A marked page needs its ITEM tab point saved on the Calibrate tab — Start '
        'names the page that is missing one.',
        style: theme.textTheme.bodySmall?.copyWith(
          color: theme.colorScheme.onSurfaceVariant,
        ),
      ),
    ];
  }

  Widget _foodCell(ThemeData theme, int cell, bool selected) {
    return InkWell(
      key: ValueKey('pet-food-cell-$_foodPage-$cell'),
      onTap: () => _toggleFoodCell(cell),
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

  String _queueText(Map<String, dynamic> entry) {
    final label = asString(entry['label']);
    final species = asString(entry['species']);
    final parts = [
      label == null || label.isEmpty ? '(no label)' : label,
      species == null || species.isEmpty ? 'species not set' : species,
    ];
    return parts.join(' · ');
  }
}

/// The Pet tab's save payload over both draft parts. A half the config does not have is omitted:
/// a fresh install carries no machine `pet` block, and sending `{pet: {}}` would ask the core to
/// create an empty block for a no-op.
class _PetDraftGroup extends ConfigDraftGroup {
  _PetDraftGroup({required super.label, required super.parts});

  @override
  Map<String, dynamic> payload() {
    final sections = super.payload();
    final pet = sections['pet'];
    if (pet is Map<String, dynamic> && pet.isEmpty) {
      sections.remove('pet');
    }
    final settings = sections['settings'];
    if (settings is Map<String, dynamic>) {
      settings.removeWhere(
        (name, value) => value is Map<String, dynamic> && value.isEmpty,
      );
      if (settings.isEmpty) {
        sections.remove('settings');
      }
    }
    return sections;
  }
}

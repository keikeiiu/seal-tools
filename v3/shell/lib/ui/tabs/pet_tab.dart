/// The Pet tab: the feeder's SETUP, which the core reads — the machine rows (`pet.slots`: enabled
/// and stacks only), the queue (removal only), the bag food cells (page + 8×8 grid), the return slot,
/// the food-load mode and the run's timing/read thresholds — plus the portable `settings.pet`
/// fallbacks, all saved together in ONE `config.apply`.
///
/// What this tab deliberately does NOT do is capture the ROWS (PLAN-V3-CALIBRATION ⚑10): a row's
/// rects, the reference crops, the page-tab points and the queue pet icons come from the live game,
/// so they arrive through the Calibrate tab or the run and stay READ-ONLY displays here; ROWS still
/// cannot be added or removed until that session exists. The QUEUE is different: it can be added to
/// by icon capture and edited/removed here (increment 2). The read-only fields ride the save
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
///
/// Icons are CAPTURED, not drawn: "Capture pet icon" / "Capture food icon" ask the core for ONE bag
/// cell crop (`pet.capture_icon`), so the shell hides itself for the shot and shows again in a
/// `finally` — the same hide → one frame → command → show flow as the Calibrate tab's capture, and
/// the core neither clicks the ITEM tab nor moves the cursor: the player puts the page up. The
/// returned PNG goes into the DRAFT (a queue entry, or `pet.food_icon_png`) with a preview; nothing
/// is applied until the tab's one Save, and a refusal leaves the draft untouched and renders the
/// core's line verbatim.
///
/// The bag SCANS ("Scan the bag") are READ-ONLY and run the same hidden flow, ONE page per command:
/// the results are VIEW STATE — the core's 1-based food cells in CELL order, its report lines and
/// best-effort evidence, and the queue's per-entry hits/claimed/deduced lines — never a draft edit.
/// Only the explicit, confirmed "Apply N cells" writes: `pet.apply_food_scan` is an A1 field-precise
/// write that REPLACES the food cells and resets the used count (disabled while the draft is dirty),
/// and the food list then refreshes from the core's `config.changed`. The core cannot verify the
/// page actually displayed, so a wrong page yields a plausible result: the requested page and the
/// report are always shown and the player checks them before applying.
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../app/pet_preflight.dart';
import '../../app/pet_species.dart';
import '../../protocol/protocol.dart';
import '../components/empty_state.dart';
import '../components/info_row.dart';
import '../components/read_only_panel.dart';
import '../components/shell_prose.dart';
import '../components/status_banner.dart';
import '../components/status_chip.dart';
import '../components/surface_panel.dart';
import '../theme.dart';
import '../tool_labels.dart';
import 'calibration_widgets.dart';
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
/// `*_png` crops except the food icon, `boarding_running`, `boarding_window_may_be_open`,
/// `max_button`) have their own writers (Calibrate / the run) and are displayed, never sent.
///
/// `food_icon_png` IS sent: "Capture food icon" stores the core's crop in the draft and the tab's one
/// Save writes it (PLAN-V3-PET-SESSION §3: capture is a config edit, the A1 writer never carries it).
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
  'food_icon_png',
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

/// A number the way the read-only rows print it: no trailing `.0` on whole values.
String _valueText(num value) => value == value.roundToDouble()
    ? value.round().toString()
    : value.toString();

/// A point `[x, y]` the way the read-only panel shows it: `x 3 · y 4`, never a raw array
/// (PLAN-V3-UI-UX §5 `ReadOnlyPanel`).
String _pointText(Object? value) {
  final point = asJsonList(value);
  if (point.length != 2) {
    return '—';
  }
  final x = asNumber(point[0]);
  final y = asNumber(point[1]);
  if (x == null || y == null) {
    return '—';
  }
  return 'x ${_valueText(x)} · y ${_valueText(y)}';
}

/// A rectangle `[x, y, w, h]` as `x 120 · y 54 · 60×60`; a two-element value is a point.
String _rectText(Object? value) {
  final rect = asJsonList(value);
  if (rect.length != 4) {
    return _pointText(value);
  }
  final x = asNumber(rect[0]);
  final y = asNumber(rect[1]);
  final width = asNumber(rect[2]);
  final height = asNumber(rect[3]);
  if (x == null || y == null || width == null || height == null) {
    return '—';
  }
  return 'x ${_valueText(x)} · y ${_valueText(y)} · '
      '${_valueText(width)}×${_valueText(height)}';
}

/// A list of points (the bag's page-tab points) as `x 10 · y 10 · x 20 · y 10 · …`.
String _pointsText(Object? value) {
  final points = asJsonList(value);
  if (points.isEmpty) {
    return '—';
  }
  return points.map(_pointText).join(' · ');
}

/// The queue table's icon column and Remove column widths (the demo's compact table row).
const double _queueIconSize = 32;
const double _queueRemoveWidth = 88;

class PetTab extends StatefulWidget {
  const PetTab({
    super.key,
    required this.model,
    this.host = const MethodChannelHostActions(),
  });

  final ShellModel model;

  /// The runner host's window seam; tests inject a fake, production uses seal/host. The icon capture
  /// hides the shell and shows it again through it (the Calibrate capture flow's seam).
  final HostActions host;

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

  /// The Add-a-pet-by-icon form: view state, not config until a capture appends a queue entry.
  final TextEditingController _addLabel = TextEditingController();
  String? _addSpeciesValue;
  int _addPage = 0;
  int? _addCell;

  /// The food cell whose icon "Capture food icon" crops: the most recently MARKED food cell (v2's
  /// "click the food, then press this"), as `[page0, cell0]`. View state; null when none is marked.
  List<int>? _foodIconCell;

  /// The machine draft revision [_foodIconCell] was last validated against. A reseed (Cancel, a
  /// save's echo, reconcile, an external change) can drop or replace the marks, and the remembered
  /// source must never outlive the mark it names.
  int _foodIconSeedRevision = -1;

  /// One icon capture at a time — the window is hidden and the game must be in front, so a second
  /// request cannot start while one is in flight.
  bool _capturing = false;

  /// The last capture answers, kept for the previews/results (the DRAFT carries the stored icon).
  CoreActionResult<JsonMap>? _queueCapture;
  CoreActionResult<JsonMap>? _foodCapture;
  String? _queueCaptureNote;
  String? _foodCaptureNote;

  // ── the bag scans (read-only view state, never the draft) ────────────────────────────────

  /// The bag page the Scan-the-bag section reads (0-based view state; the wire is 1-based).
  int _scanPage = 0;

  /// One hide-window SCAN at a time (the window is hidden for the shot, so it cannot overlap a
  /// capture either — see [_windowBusy]).
  bool _scanning = false;

  /// The last scan answers, per kind: `pet.scan_food_page` / `pet.scan_queue_page` results. View
  /// state only — a scan never touches the draft; a new scan of a kind replaces its own result.
  CoreActionResult<JsonMap>? _foodScan;
  CoreActionResult<JsonMap>? _queueScan;
  String? _foodScanNote;
  String? _queueScanNote;

  /// The explicit apply of a food scan's cells (never automatic).
  bool _applying = false;
  CoreActionResult<JsonMap>? _applyResult;

  // ── the panel test read (read-only diagnostic, OPEN §6 G13) ────────────────────────────

  /// The 1-based bag cell whose tooltip is read; the page is [_scanPage] (same ITEM page picker).
  int _panelCell = 1;

  /// One hide-window panel read at a time (the window is hidden for it, like the scans).
  bool _panelReading = false;

  /// The last `pet.test_read_panel` answer and the host-channel note beside it.
  CoreActionResult<JsonMap>? _panelRead;
  String? _panelReadNote;

  /// Both the icon captures, the scans and the panel read hide the shell and need the game in front:
  /// only one such window command may be in flight at a time.
  bool get _windowBusy => _capturing || _scanning || _applying || _panelReading;

  @override
  void initState() {
    super.initState();
    _seenConfigRev = model.config.rev;
    model.config.addListener(_onMirrorChanged);
    // G12: the row cards show the live row beside the saved setup, so a `state` event must rebuild.
    model.tools.addListener(_onToolsChanged);
    // The add form starts from the last line already on the queue (v2's seed rule: the first capture
    // after a restart must not write an entry with no line because a picker reset to "none").
    for (final entry in _queue().reversed) {
      final species = asString(entry['species']);
      if (species != null && species.isNotEmpty) {
        _addSpeciesValue = species;
        break;
      }
    }
    // The food-icon source starts at the last marked cell (see [_syncFoodIconCell], first build).
    unawaited(_loadPreflight());
  }

  @override
  void dispose() {
    model.config.removeListener(_onMirrorChanged);
    model.tools.removeListener(_onToolsChanged);
    _addLabel.dispose();
    _group.dispose();
    super.dispose();
  }

  void _onToolsChanged() {
    if (mounted) {
      setState(() {});
    }
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

  /// Keeps the food-icon source a MARKED cell. On a reseed (Cancel, a save's echo, reconcile, an
  /// external change) the remembered cell is kept only while the CURRENT draft still marks it;
  /// otherwise it falls back to the newest mark or clears, so "Capture food icon" then shows the
  /// existing "Mark a food cell first…" disabled state instead of offering a cell the config no
  /// longer has. Marking/unmarking does not bump the seed revision, so this never fights
  /// [_toggleFoodCell]'s explicit selection while the player works.
  void _syncFoodIconCell() {
    if (_foodIconSeedRevision == _machineDraft.seedRevision) {
      return;
    }
    _foodIconSeedRevision = _machineDraft.seedRevision;
    final marked = _foodSlots();
    final source = _foodIconCell;
    final stillMarked =
        source != null &&
        marked.any((pair) => pair[0] == source[0] && pair[1] == source[1]);
    if (stillMarked) {
      return;
    }
    _foodIconCell = marked.isEmpty ? null : marked.last;
  }

  String _rowMarksText(Map<String, dynamic> row) {
    final boarding = asBool(row['boarding_running']) == true;
    final crop = _hasCrop(row['pet_slot_empty_png'])
        ? 'empty-slot crop saved'
        : 'empty-slot crop not saved';
    return 'start/end ${_rectText(row['toggle_label'])} · '
        'pet slot ${_rectText(row['boarding_pet_slot'])} · '
        'food strip ${_rectText(row['feeder_strip'])} · '
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

  void _setQueueField(int index, String field, Object? value) => setState(() {
    _machineDraft.edit((block) {
      final queue = asJsonList(block['queue']);
      if (index < 0 || index >= queue.length) {
        return;
      }
      asJsonMap(queue[index])[field] = value;
    });
  });

  void _toggleFoodCell(int cell) {
    final slots = _foodSlots();
    final existing = slots.indexWhere(
      (pair) => pair[0] == _foodPage && pair[1] == cell,
    );
    if (existing >= 0) {
      slots.removeAt(existing);
      if (_foodIconCell != null &&
          _foodIconCell![0] == _foodPage &&
          _foodIconCell![1] == cell) {
        _foodIconCell = null;
      }
    } else {
      slots.add([_foodPage, cell]);
      // v2's rule: "Capture food icon" takes the cell the player just marked.
      _foodIconCell = [_foodPage, cell];
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

  // ── icon captures (queue / food) ──────────────────────────────────────────────────────────

  void _setAddPage(int page) => setState(() => _addPage = page);

  void _setAddCell(int cell) => setState(() => _addCell = cell);

  String _addSourceText() {
    final cell = _addCell;
    if (cell == null) {
      return 'No source cell picked yet — click the cell holding the pet.';
    }
    final row = cell ~/ _bagColumns + 1;
    final column = cell % _bagColumns + 1;
    return 'Source: ITEM${_addPage + 1} · cell ${cell + 1} (row $row, column $column)';
  }

  String _foodIconSourceText() {
    final source = _foodIconCell;
    if (source == null) {
      return 'Mark a food cell first — the icon is cropped from that cell.';
    }
    final row = source[1] ~/ _bagColumns + 1;
    final column = source[1] % _bagColumns + 1;
    return 'Food icon source: ITEM${source[0] + 1} · cell ${source[1] + 1} '
        '(row $row, column $column)';
  }

  /// One frame for the compositor to apply the hide — but never longer than ~50 ms, because a hidden
  /// window may stop producing frames on some drivers and the capture must not hang on the lifecycle
  /// wait. This is the Calibrate capture flow's helper (private there), reproduced so both captures
  /// share the same behaviour; the flow itself is the same hide → one frame → command → show.
  Future<void> _waitOneFrame() async {
    final waited = Completer<void>();
    final fallback = Timer(const Duration(milliseconds: 50), () {
      if (!waited.isCompleted) {
        waited.complete();
      }
    });
    unawaited(
      WidgetsBinding.instance.endOfFrame.whenComplete(() {
        fallback.cancel();
        if (!waited.isCompleted) {
          waited.complete();
        }
      }),
    );
    await waited.future;
  }

  /// Runs ONE window command with the shell hidden: hide → one frame → [command] → show, the show
  /// UNCONDITIONAL — a refused capture/scan (or a thrown error) must never leave the shell hidden.
  /// This is the Calibrate capture flow's pattern, now shared by the icon captures and the bag scans;
  /// a missing host channel is a NOTE, not a failure (the request is still sent and its refusal
  /// rendered verbatim). The core validates the bag grid and the page's tab mark but does NOT click
  /// the page, so every caller's instruction is "put the page up".
  Future<({CoreActionResult<JsonMap> result, String? hostNote})>
  _runHiddenWindow(Future<CoreActionResult<JsonMap>> Function() command) async {
    var hostNote = '';
    CoreActionResult<JsonMap>? result;
    try {
      try {
        await widget.host.hideWindow();
      } catch (_) {
        hostNote = 'the shell could not hide itself — the game has to be in front for a capture';
      }
      await _waitOneFrame();
      result = await command();
    } finally {
      try {
        await widget.host.showWindow();
      } catch (_) {
        hostNote = hostNote.isEmpty
            ? 'the shell could not be shown again — no host channel'
            : hostNote;
      }
    }
    return (result: result, hostNote: hostNote.isEmpty ? null : hostNote);
  }

  /// hide → one frame → `pet.capture_icon` → show (see [_runHiddenWindow]).
  Future<({CoreActionResult<JsonMap> result, String? hostNote})> _captureIcon({
    required String kind,
    required int page,
    required int cell,
  }) => _runHiddenWindow(
    () => model.capturePetIcon(kind: kind, page: page, cell: cell),
  );

  /// Captures the selected bag cell's icon and appends a queue entry to the DRAFT — nothing is
  /// applied until the tab's Save. The entry carries the Add form's label/species, the core's rect
  /// and the captured PNG; a refusal changes nothing and renders the core's line verbatim.
  Future<void> _captureQueueIcon() async {
    final cell = _addCell;
    if (_windowBusy || cell == null || _bagGridRect() == null) {
      return;
    }
    setState(() {
      _capturing = true;
      _queueCapture = null;
      _queueCaptureNote = null;
    });
    final outcome = await _captureIcon(
      kind: 'queue',
      page: _addPage + 1,
      cell: cell + 1,
    );
    if (!mounted) {
      return;
    }
    final result = outcome.result;
    setState(() {
      _capturing = false;
      _queueCapture = result;
      _queueCaptureNote = outcome.hostNote;
    });
    final value = result.value;
    if (!result.ok || value == null) {
      return;
    }
    final label = _addLabel.text.trim();
    final species = _addSpeciesValue?.trim() ?? '';
    setState(() {
      _machineDraft.edit((block) {
        final queue = asJsonList(block['queue']);
        queue.add({
          'label': label.isEmpty ? null : label,
          'species': species.isEmpty ? null : species,
          'rect': asJsonList(value['rect']),
          'png': asString(value['png_base64']),
        });
      });
      _addLabel.clear();
    });
  }

  /// Captures the selected FOOD cell's icon into the draft's `food_icon_png` — the reference the
  /// food scan matches with. The draft is the only writer until Save; a refusal changes nothing.
  Future<void> _captureFoodIcon() async {
    final source = _foodIconCell;
    if (_windowBusy || source == null || _bagGridRect() == null) {
      return;
    }
    setState(() {
      _capturing = true;
      _foodCapture = null;
      _foodCaptureNote = null;
    });
    final outcome = await _captureIcon(
      kind: 'food',
      page: source[0] + 1,
      cell: source[1] + 1,
    );
    if (!mounted) {
      return;
    }
    final result = outcome.result;
    setState(() {
      _capturing = false;
      _foodCapture = result;
      _foodCaptureNote = outcome.hostNote;
    });
    final value = result.value;
    final png = value == null ? null : asString(value['png_base64']);
    if (!result.ok || png == null || png.isEmpty) {
      return;
    }
    _setMachine('food_icon_png', png);
  }

  // ── the bag scans (READ-ONLY view state + the explicit food apply) ───────────────────────

  /// The scan's cells as the wire's 1-BASED `[[page, cell], ...]` pairs — the scan's echoed page plus
  /// the core's CELL-order 1-based cells (order preserved; the core stores exactly what it is sent).
  List<List<int>> _scannedCells(Map<String, dynamic> value) {
    final page = asNumber(value['page'])?.round() ?? 1;
    final cells = <List<int>>[];
    for (final cell in asJsonList(value['cells'])) {
      final number = asNumber(cell)?.round();
      if (number != null) {
        cells.add([page, number]);
      }
    }
    return cells;
  }

  /// Scans the page for FOOD: hide → `pet.scan_food_page {page}` → show. READ-ONLY: the result is
  /// VIEW STATE (cells in the core's own 1-based CELL order, its report/evidence lines verbatim),
  /// never a draft edit; the explicit apply is a separate action the player confirms.
  Future<void> _scanFoodPage() async {
    if (_windowBusy) {
      return;
    }
    setState(() {
      _scanning = true;
      _foodScan = null;
      _foodScanNote = null;
      _applyResult = null;
    });
    final outcome = await _runHiddenWindow(
      () => model.scanPetFoodPage(page: _scanPage + 1),
    );
    if (!mounted) {
      return;
    }
    setState(() {
      _scanning = false;
      _foodScan = outcome.result;
      _foodScanNote = outcome.hostNote;
    });
  }

  /// Scans the page for the QUEUED pets, same hide→scan→show flow and read-only rule; the per-entry
  /// labels/hit cells/scores/runner-ups, claimed conflicts and deduced lines are the core's, verbatim.
  Future<void> _scanQueuePage() async {
    if (_windowBusy) {
      return;
    }
    setState(() {
      _scanning = true;
      _queueScan = null;
      _queueScanNote = null;
    });
    final outcome = await _runHiddenWindow(
      () => model.scanPetQueuePage(page: _scanPage + 1),
    );
    if (!mounted) {
      return;
    }
    setState(() {
      _scanning = false;
      _queueScan = outcome.result;
      _queueScanNote = outcome.hostNote;
    });
  }

  /// The panel test read: hide → `pet.test_read_panel {page,cell}` → show. READ-ONLY diagnostic
  /// (OPEN §6 G13): the core hovers the cell (never a click), captures the calibrated tooltip
  /// region and returns the region, raw OCR lines, its own parsed line and the numbers it saw. The
  /// result is view state only; nothing about the draft or the run changes.
  Future<void> _testReadPanel() async {
    if (_windowBusy) {
      return;
    }
    setState(() {
      _panelReading = true;
      _panelRead = null;
      _panelReadNote = null;
    });
    final outcome = await _runHiddenWindow(
      () => model.testReadPetPanel(page: _scanPage + 1, cell: _panelCell),
    );
    if (!mounted) {
      return;
    }
    setState(() {
      _panelReading = false;
      _panelRead = outcome.result;
      _panelReadNote = outcome.hostNote;
    });
  }

  /// The explicit apply: ask (the replace + used-count reset is the player's call), then ONE
  /// `pet.apply_food_scan` — an A1 write, never a `config.apply`. Disabled while the draft is dirty:
  /// the emitted `config.changed` beside a dirty part would banner the player's own action. The food
  /// list itself refreshes from the mirror (the core's event re-seeds the clean draft, and
  /// [_onMirrorChanged] rebuilds this State); the checklists are re-read because they describe the
  /// saved config this write changed.
  Future<void> _applyFoodScan() async {
    final scan = _foodScan;
    final value = scan?.value;
    if (_windowBusy || _draftDirty || value == null) {
      return;
    }
    final cells = _scannedCells(value);
    if (cells.isEmpty) {
      return;
    }
    final confirmed = await _confirmApply(
      cells.length,
      asNumber(value['page'])?.round() ?? 1,
    );
    if (!mounted || !confirmed) {
      return;
    }
    setState(() {
      _applying = true;
      _applyResult = null;
    });
    final result = await model.applyPetFoodScan(cells);
    if (!mounted) {
      return;
    }
    setState(() {
      _applying = false;
      _applyResult = result;
    });
    if (result.ok) {
      await _loadPreflight();
    }
  }

  /// The confirmation: the list is REPLACED, the used count RESET, and it takes effect immediately —
  /// so it is never silent, and it names the page the scan was requested for (the core cannot verify
  /// the page that was actually displayed) and tells the player to check the report/evidence first.
  Future<bool> _confirmApply(int count, int page) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Replace the food cells?'),
        content: Text(
          'Apply $count cell${count == 1 ? '' : 's'} from the ITEM$page scan? This REPLACES the '
          'tab\'s food-cell list and RESETS the used-cell count (the run\'s progress), exactly as '
          'the scan lists them. It applies immediately, even while a run is in progress — check the '
          'report and evidence first: a wrong page gives a plausible wrong list.',
        ),
        actions: [
          TextButton(
            key: const ValueKey('pet-scan-apply-cancel'),
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            key: const ValueKey('pet-scan-apply-confirm'),
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Replace the food cells'),
          ),
        ],
      ),
    );
    return confirmed ?? false;
  }

  /// The line under the apply button: the core's refusal VERBATIM, or the applied count and the rev
  /// the A1 write returned.
  String? _applyText() {
    final result = _applyResult;
    if (result == null) {
      return null;
    }
    if (!result.ok) {
      return result.errorLine;
    }
    final value = result.value;
    final rev = value == null ? null : asNumber(value['rev'])?.round();
    final scanValue = _foodScan?.value;
    final count = scanValue == null ? 0 : _scannedCells(scanValue).length;
    return 'Applied $count cell${count == 1 ? '' : 's'}'
        '${rev == null ? '' : ' (config rev $rev)'} — the used-cell count was reset.';
  }

  /// A number the way the core prints its scores: three decimals; an absent one is a dash.
  String _scoreText(Object? value) {
    final number = asNumber(value);
    return number == null ? '—' : number.toStringAsFixed(3);
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

  /// One checklist item as a row: the passed/failed/informational icon, the core's label and its
  /// detail. The shell adds no wording and infers no rule from a label or a count — `ok` + `detail`
  /// are the contract, and the core's own readiness lines are rendered untouched below the lists.
  Widget _checklistRow(ThemeData theme, String key, PetChecklistItem item) {
    final (icon, color) = switch (_checkMarker(item)) {
      '✗' => (Icons.error_outline, theme.colorScheme.error),
      'ℹ' => (Icons.info_outline, theme.colorScheme.onSurfaceVariant),
      _ => (Icons.check_circle_outline, theme.shellColors.success),
    };
    final detail = item.detail;
    return Padding(
      key: ValueKey(key),
      padding: const EdgeInsets.symmetric(vertical: ShellSpace.xxs),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: ShellSize.iconSm, color: color),
          const SizedBox(width: ShellSpace.xs + ShellSpace.xxs),
          Expanded(
            child: Wrap(
              spacing: ShellSpace.sm,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                Text(item.label, style: theme.textTheme.bodyMedium),
                if (detail != null && detail.isNotEmpty)
                  Text(
                    detail,
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  /// One of the core's two checklists as a titled group of rows — never one `\n`-joined `Text`
  /// (PLAN-V3-UI-UX §6 Pet): each item carries its own icon, label and detail.
  Widget _checklist(
    ThemeData theme,
    String title,
    String groupKey,
    List<PetChecklistItem> items,
  ) {
    return Column(
      key: ValueKey(groupKey),
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: theme.textTheme.labelLarge),
        if (items.isEmpty)
          Text('(no checks)', style: theme.textTheme.bodySmall)
        else
          for (var index = 0; index < items.length; index++)
            _checklistRow(theme, '$groupKey-$index', items[index]),
      ],
    );
  }

  /// The readiness state as the page's first block: a [StatusBanner] whose colour IS the core's
  /// verdict (never an uncoloured sentence), then the core's per-item checklists and its
  /// ready/refusal lines as individual rows. The lists describe the SAVED config, so while ANY part
  /// of this tab's draft is dirty they are marked stale (a save re-reads them) — broader than
  /// [_rowsDirty], because the portable `settings.pet` fields feed the same preflight.
  Widget _readinessSection(ThemeData theme) {
    final result = _preflight;
    final value = result?.value;
    final stale = _group.isDirty;
    final refresh = TextButton(
      key: const ValueKey('pet-preflight-refresh'),
      onPressed: _loadingPreflight ? null : _loadPreflight,
      child: const Text('Refresh checklists'),
    );
    final Widget banner;
    if (result == null) {
      banner = StatusBanner(
        key: const ValueKey('pet-preflight-pending'),
        kind: StatusKind.info,
        title: _loadingPreflight ? 'Reading the pet setup…' : 'Not read yet.',
        detail: 'The checklists come from the core — read-only, no board and no game access.',
        action: refresh,
      );
    } else if (!result.ok) {
      banner = StatusBanner(
        key: const ValueKey('pet-preflight-error'),
        kind: StatusKind.error,
        title: 'The core could not check the pet setup.',
        detail: result.errorLine,
        action: refresh,
      );
    } else {
      final ready = value?.ready ?? false;
      banner = StatusBanner(
        key: const ValueKey('pet-preflight-ready'),
        kind: ready ? StatusKind.success : StatusKind.warning,
        title: ready ? 'Ready to run.' : 'Not ready yet — see the lines below.',
        detail: 'The core mirrors what Start will accept — not an end-to-end run result.',
        action: refresh,
      );
    }
    final rows = <Widget>[
      if (stale)
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.sm),
          child: Text(
            'Based on the saved config — Save to refresh',
            key: const ValueKey('pet-preflight-stale'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ),
      if (value != null) ...[
        const SizedBox(height: ShellSpace.md),
        _checklist(
          theme,
          'Calibration',
          'pet-preflight-calibration',
          value.calibration,
        ),
        const SizedBox(height: ShellSpace.sm),
        _checklist(
          theme,
          'Setup so far',
          'pet-preflight-session',
          value.session,
        ),
        if (value.lines.isNotEmpty) ...[
          const SizedBox(height: ShellSpace.md),
          Column(
            key: const ValueKey('pet-preflight-lines'),
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              for (final line in value.lines)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: ShellSpace.xxs),
                  child: ShellProse(line, style: theme.textTheme.bodySmall),
                ),
            ],
          ),
        ],
      ],
    ];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        banner,
        if (rows.isNotEmpty) ...[
          const SizedBox(height: ShellSpace.sm),
          SurfacePanel(
            padding: const EdgeInsets.all(ShellSpace.md),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: rows,
            ),
          ),
        ] else
          const SizedBox(height: ShellSpace.lg),
      ],
    );
  }

  /// The calibrated marks and reference crops, DEMOTED below the editable form and collapsed
  /// (PLAN-V3-UI-UX §2.1/§2.5.3): reference data is read once, not the first screenful. Each value
  /// reads `x 120 · y 54 · 60×60`, never a raw JSON array, and the header states how many marks the
  /// config carries without opening the panel.
  Widget _referencePanel(Map<String, dynamic> machine) {
    final marks = <Object?>[
      machine['menu_button'],
      machine['feed_icon'],
      machine['close_button'],
      machine['bag_grid'],
      machine['bag_slot'],
      machine['page_tabs'],
      machine['max_button'],
    ];
    final configuredMarks = marks
        .where((mark) => asJsonList(mark).isNotEmpty)
        .length;
    final configuredCrops = [
      if (_hasCrop(machine['close_button_png'])) 'boarding X',
      if (_hasCrop(machine['food_icon_png'])) 'food icon',
    ];
    return ReadOnlyPanel(
      title: 'Calibrated marks & reference crops',
      subtitle:
          'Drawn on the Calibrate tab or learned by the run — read-only here, and never sent '
          'by this tab\'s Save.',
      openHint:
          '$configuredMarks of ${marks.length} marks · '
          '${configuredCrops.length} of 2 crops',
      rows: [
        InfoRow(
          label: 'Menu (目錄) point',
          value: _pointText(machine['menu_button']),
          mono: true,
        ),
        InfoRow(
          label: 'Pet feed icon point',
          value: _pointText(machine['feed_icon']),
          mono: true,
        ),
        InfoRow(
          label: 'Boarding X point',
          value: _pointText(machine['close_button']),
          mono: true,
        ),
        InfoRow(
          label: 'Boarding bag grid',
          value: _rectText(machine['bag_grid']),
          mono: true,
        ),
        InfoRow(
          label: 'Boarding bag slot',
          value: _rectText(machine['bag_slot']),
          mono: true,
        ),
        InfoRow(
          label: 'Bag page tab points',
          value: _pointsText(machine['page_tabs']),
          mono: true,
        ),
        InfoRow(
          label: 'MAX button (pet override)',
          value: _pointText(machine['max_button']),
          mono: true,
        ),
        InfoRow(
          label: 'Boarding X crop',
          value: _yesNo(_hasCrop(machine['close_button_png'])),
        ),
        InfoRow(
          label: 'Food icon crop',
          value: _yesNo(_hasCrop(machine['food_icon_png'])),
        ),
        InfoRow(
          label: 'Food cells used (run reading)',
          value: readOnlyText(machine['food_slots_used']),
          mono: true,
        ),
      ],
    );
  }

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    _syncReturnFields();
    _syncFoodIconCell();
    final theme = Theme.of(context);
    final machine = _machine();
    final rows = _rows();
    final queue = _queue();
    final bagGrid = _bagGridRect();

    return buildConfigTab(
      title: 'Pet Feeder',
      description:
          'Rows, queue, food plan and run behaviour. Calibrated marks and crops stay read-only '
          'here; fix those in Calibrate.',
      readOnly: [_referencePanel(machine)],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _readinessSection(theme),
          _petSummary(theme, rows.length, queue, _foodSlots().length),
          const SizedBox(height: ShellSpace.md),
          LayoutBuilder(
            builder: (context, constraints) {
              final wide = constraints.maxWidth >= 900;
              final left = Column(
                key: const ValueKey('pet-left-column'),
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  SurfacePanel(
                    title: 'Boarding rows',
                    description:
                        'A row\'s live boarding state is written the moment you tick it; the '
                        'enabled switch and stack count save with the tab\'s Save.',
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        if (rows.isEmpty)
                          const EmptyState(
                            key: ValueKey('pet-no-rows'),
                            icon: Icons.grid_view_outlined,
                            title: 'No breeding rows configured',
                            hint:
                                'A migrated v2 config carries them; adding a row needs the pet '
                                'capture session (OPEN). The feeder refuses on Start until one exists.',
                            compact: true,
                          )
                        else
                          for (var index = 0; index < rows.length; index++)
                            _rowCard(theme, index, rows[index]),
                        if (_boardingError != null)
                          Padding(
                            padding: const EdgeInsets.only(top: ShellSpace.xs),
                            child: Text(
                              _boardingError!,
                              key: const ValueKey('pet-boarding-error'),
                              style: theme.textTheme.bodySmall?.copyWith(
                                color: theme.colorScheme.error,
                              ),
                            ),
                          ),
                        const SizedBox(height: ShellSpace.xs),
                        ShellProse(
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
                  ),
                  SurfacePanel(
                    title: 'Queue',
                    description:
                        'Pets are boarded in this order when a row frees up.',
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        if (queue.isEmpty)
                          const EmptyState(
                            key: ValueKey('pet-queue-empty'),
                            icon: Icons.pets_outlined,
                            title: 'No pets queued',
                            hint:
                                'The return slot is then the fallback the run right-clicks. '
                                'Add a pet by icon below.',
                            compact: true,
                          )
                        else ...[
                          _queueHeader(theme),
                          for (var index = 0; index < queue.length; index++)
                            _queueEntryRow(theme, index, queue[index]),
                        ],
                        const SizedBox(height: ShellSpace.xs),
                        ShellProse(
                          'The icon is the pet\'s identity — the run matches on it, and the label is '
                          'for the log and this list only. Edits and a captured entry save with the '
                          'tab\'s Save.',
                          key: const ValueKey('pet-queue-caption'),
                          style: theme.textTheme.bodySmall?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                  ),
                  EditorSection(
                    title: 'Add a pet by icon',
                    children: _addPetEditor(theme),
                  ),
                ],
              );
              final right = Column(
                key: const ValueKey('pet-right-column'),
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  EditorSection(
                    title: 'Scan the bag (read-only)',
                    children: _scanSection(theme),
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
                          width: ShellWidth.fieldLg,
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
                        width: ShellWidth.picker,
                        onChanged: (value) {
                          if (value != null) {
                            _setMachine('food_load_mode', value);
                          }
                        },
                      ),
                      const SizedBox(height: ShellSpace.xs),
                      ShellProse(
                        'right_click lets the game choose the box; drag names it, which '
                        'needs the row food strip and a board whose firmware can drag.',
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                      DraftSwitchField(
                        key: const ValueKey('pet-reload-on-start'),
                        label: 'Reload on start',
                        helper: 'open and close the boarding window once when a run begins',
                        value:
                            asBool(_machineDraft.value['reload_on_start']) ??
                            false,
                        onChanged: (value) =>
                            _setMachine('reload_on_start', value),
                      ),
                    ],
                  ),
                  ExpansionTile(
                    key: const ValueKey('pet-advanced'),
                    initiallyExpanded: false,
                    maintainState: true,
                    tilePadding: EdgeInsets.zero,
                    childrenPadding: const EdgeInsets.only(top: ShellSpace.sm),
                    title: Text(
                      'Timing & thresholds',
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                    subtitle: Text(
                      'Action wait, count thresholds and portable rates',
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: Theme.of(context).colorScheme.onSurfaceVariant,
                      ),
                    ),
                    children: [
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
                              value: asNumber(
                                _machineDraft.value['action_wait_ms'],
                              ),
                              width: ShellWidth.fieldLg,
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
                              width: ShellWidth.picker,
                              onChanged: (value) => _setMachine(
                                'wait_after_empty_minutes',
                                value.round(),
                              ),
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
                                _machineDraft
                                    .value['feeder_count_left_fraction'],
                              ),
                              width: 280,
                              onChanged: (value) => _setMachine(
                                'feeder_count_left_fraction',
                                value,
                              ),
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
                              value: asNumber(
                                _portableDraft.value['items_per_minute'],
                              ),
                              width: 260,
                              onChanged: (value) =>
                                  _setPortable('items_per_minute', value),
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
                              onChanged: (value) => _setPortable(
                                'pet_slot_occupied_above',
                                value,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ],
              );
              if (!wide) {
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    left,
                    const SizedBox(height: ShellSpace.lg),
                    right,
                  ],
                );
              }
              return Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(child: left),
                  const SizedBox(width: ShellSpace.lg),
                  Expanded(child: right),
                ],
              );
            },
          ),
        ],
      ),
    );
  }

  /// A one-line read of the setup above the two columns: rows, queue, food marks and the pet that
  /// boards next. The full editors stay below; this only saves the player from opening every panel to
  /// answer "what will the next run do?".
  Widget _petSummary(
    ThemeData theme,
    int rows,
    List<Map<String, dynamic>> queue,
    int foodCells,
  ) {
    final next = queue.isEmpty
        ? null
        : asString(queue.first['label']) ?? 'pet 1';
    Widget pill(String key, String label, String value) => Container(
      key: ValueKey(key),
      padding: const EdgeInsets.symmetric(
        horizontal: ShellSpace.sm,
        vertical: ShellSpace.xxs,
      ),
      decoration: BoxDecoration(
        border: Border.all(color: theme.colorScheme.outlineVariant),
        borderRadius: BorderRadius.circular(ShellRadius.s),
      ),
      child: Text.rich(
        TextSpan(
          children: [
            TextSpan(
              text: '$label ',
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
            TextSpan(
              text: value,
              style: theme.textTheme.labelSmall?.copyWith(
                fontWeight: FontWeight.w600,
                color: theme.colorScheme.onSurface,
              ),
            ),
          ],
        ),
      ),
    );
    return Padding(
      padding: const EdgeInsets.only(top: ShellSpace.sm),
      child: Wrap(
        spacing: ShellSpace.xs,
        runSpacing: ShellSpace.xs,
        children: [
          pill('pet-summary-rows', 'rows', '$rows'),
          pill('pet-summary-queue', 'queued', '${queue.length}'),
          pill('pet-summary-food', 'food cells', '$foodCells / $_bagSlots'),
          if (next != null) pill('pet-summary-next', 'next', next),
        ],
      ),
    );
  }

  /// The queue's table header, so the section reads as one compact table (PLAN-V3-UI-UX §6 Pet:
  /// "Queue as `InfoRow` table") instead of a stack of full-width fields.
  Widget _queueHeader(ThemeData theme) {
    final style = theme.textTheme.labelSmall?.copyWith(
      color: theme.colorScheme.onSurfaceVariant,
    );
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.xs),
      child: Row(
        children: [
          const SizedBox(width: _queueIconSize),
          const SizedBox(width: ShellSpace.sm),
          Expanded(child: Text('Pet', style: style)),
          const SizedBox(width: ShellSpace.sm),
          Expanded(child: Text('Species', style: style)),
          const SizedBox(width: ShellSpace.xs),
          const SizedBox(width: _queueRemoveWidth),
        ],
      ),
    );
  }

  /// The game-data species options, keeping a value an older config already carries even when it is
  /// not in the current list (a custom/renamed species must not silently disappear on Save).
  List<String> _speciesOptions(String current) {
    final options = [...petSpecies];
    final trimmed = current.trim();
    if (trimmed.isNotEmpty && !options.contains(trimmed)) {
      options.insert(0, trimmed);
    }
    return options;
  }

  /// One queued pet as a table row: the captured icon (the identity the run matches), the editable
  /// label/species and Remove. Edits go to the machine draft and ride the tab's one Save — v2
  /// auto-saved each edit; v3 keeps everything in the draft until Save, so a half-typed label never
  /// reaches the file.
  Widget _queueEntryRow(
    ThemeData theme,
    int index,
    Map<String, dynamic> entry,
  ) {
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.sm),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _iconPreview(
            theme,
            asString(entry['png']),
            key: ValueKey('pet-queue-icon-$index'),
            size: _queueIconSize,
          ),
          const SizedBox(width: ShellSpace.sm),
          Expanded(
            child: DraftFieldSlot(
              slotKey: 'pet-queue-label-$index',
              syncToken: _machineDraft.seedRevision,
              child: DraftTextField(
                key: ValueKey('pet-queue-label-$index'),
                label: 'Label ${index + 1}',
                value: asString(entry['label']) ?? '',
                onChanged: (value) => _setQueueField(
                  index,
                  'label',
                  value.trim().isEmpty ? null : value,
                ),
              ),
            ),
          ),
          const SizedBox(width: ShellSpace.sm),
          Expanded(
            child: DraftFieldSlot(
              slotKey: 'pet-queue-species-$index',
              syncToken: _machineDraft.seedRevision,
              child: DraftDropdownField(
                key: ValueKey('pet-queue-species-$index'),
                label: 'Species (feeding-table line)',
                value:
                    _speciesOptions(asString(entry['species']) ?? '')
                        .contains(asString(entry['species']) ?? '')
                    ? asString(entry['species'])
                    : null,
                options: _speciesOptions(asString(entry['species']) ?? ''),
                onChanged: (value) => _setQueueField(
                  index,
                  'species',
                  (value ?? '').trim().isEmpty ? null : value,
                ),
              ),
            ),
          ),
          const SizedBox(width: ShellSpace.xs),
          SizedBox(
            width: _queueRemoveWidth,
            child: Align(
              alignment: Alignment.centerRight,
              child: TextButton(
                key: ValueKey('pet-queue-remove-$index'),
                onPressed: () => _removeQueued(index),
                child: const Text('Remove'),
              ),
            ),
          ),
        ],
      ),
    );
  }

  /// A base64 PNG preview (a captured icon). A bad payload renders a line, never a crash — the shared
  /// `decodePngBase64` is the same reader the Calibrate capture uses.
  Widget _iconPreview(
    ThemeData theme,
    String? pngBase64, {
    Key? key,
    double size = 48,
  }) {
    final bytes = pngBase64 == null || pngBase64.isEmpty
        ? null
        : decodePngBase64(pngBase64);
    return Container(
      key: key,
      width: size,
      height: size,
      clipBehavior: Clip.antiAlias,
      decoration: BoxDecoration(
        border: Border.all(color: theme.colorScheme.outlineVariant),
      ),
      child: bytes == null
          ? Center(
              child: Text(
                'no icon',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            )
          : Image.memory(
              bytes,
              fit: BoxFit.contain,
              gaplessPlayback: true,
              errorBuilder: (context, error, stackTrace) => Center(
                child: Text(
                  'no icon',
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: theme.colorScheme.error,
                  ),
                ),
              ),
            ),
    );
  }

  /// The one-line result under a capture button: the core's refusal VERBATIM, or what the capture
  /// added/stored. Nothing is applied either way — the draft carries it until Save.
  String? _captureText(
    CoreActionResult<JsonMap>? capture, {
    required String verb,
  }) {
    if (capture == null) {
      return null;
    }
    if (!capture.ok) {
      return capture.errorLine;
    }
    final value = capture.value;
    if (value == null) {
      return null;
    }
    final page = asNumber(value['page'])?.round() ?? 0;
    final cell = asNumber(value['cell'])?.round() ?? 0;
    final rect = asJsonList(value['rect']);
    final size = rect.length == 4 ? '${rect[2]}×${rect[3]} px' : 'the cell box';
    return 'Captured $size from ITEM$page · cell $cell — $verb';
  }

  /// v2's queue-add: put the page up, pick the cell holding the pet, capture the icon. The page is
  /// NOT clicked by the core, so the player switches to it (the hint says so) and the capture is a
  /// single foreground-gated command; a wrong page produces a plausible wrong crop, which is why the
  /// captured PNG + echoed page/cell/rect are always shown and nothing is applied before Save.
  List<Widget> _addPetEditor(ThemeData theme) {
    final capture = _queueCapture;
    if (_bagGridRect() == null) {
      return [
        ShellProse(
          "The boarding bag grid isn't calibrated — save the Boarding bag grid and "
          'Boarding bag slot on the Calibrate tab. The crop is taken at that grid\'s pitch.',
          key: const ValueKey('pet-add-not-calibrated'),
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ];
    }
    return [
      ShellProse(
        'Put the bag page holding the pet on screen: the shell hides for the shot, so the game has '
        'to be in front, and the core does NOT click the ITEM tab — the page you pick must already '
        'be up. Check the preview and the echoed page/cell/rect before Save: a wrong page gives a '
        'plausible wrong crop.',
        key: const ValueKey('pet-add-hint'),
        style: theme.textTheme.bodySmall,
      ),
      const SizedBox(height: ShellSpace.sm),
      Wrap(
        spacing: ShellSpace.sm,
        runSpacing: ShellSpace.xs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          SizedBox(
            width: 260,
            child: TextField(
              key: const ValueKey('pet-add-label'),
              controller: _addLabel,
              style: theme.textTheme.bodyMedium,
              decoration: const InputDecoration(
                labelText: 'Label (log and this list only)',
                isDense: true,
                border: OutlineInputBorder(),
              ),
            ),
          ),
          SizedBox(
            width: 300,
            child: DraftDropdownField(
              key: const ValueKey('pet-add-species'),
              label: 'Species (feeding-table line, optional)',
              value: _addSpeciesValue,
              options: _speciesOptions(_addSpeciesValue ?? ''),
              onChanged: (value) => setState(
                () => _addSpeciesValue = value?.trim().isEmpty ?? true
                    ? null
                    : value,
              ),
            ),
          ),
        ],
      ),
      const SizedBox(height: ShellSpace.xs),
      Wrap(
        spacing: ShellSpace.sm,
        children: [
          for (var page = 0; page < _bagPages; page++)
            ChoiceChip(
              key: ValueKey('pet-add-page-$page'),
              label: Text('ITEM${page + 1}'),
              selected: _addPage == page,
              onSelected: (_) => _setAddPage(page),
            ),
        ],
      ),
      const SizedBox(height: ShellSpace.xs),
      Text(
        _addSourceText(),
        key: const ValueKey('pet-add-source'),
        style: theme.textTheme.bodySmall,
      ),
      const SizedBox(height: ShellSpace.xs),
      for (var row = 0; row < _bagColumns; row++)
        Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            for (var column = 0; column < _bagColumns; column++)
              _sourceCell(theme, row * _bagColumns + column),
          ],
        ),
      const SizedBox(height: ShellSpace.xs),
      Wrap(
        spacing: ShellSpace.sm,
        runSpacing: ShellSpace.xs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          OutlinedButton(
            key: const ValueKey('pet-add-capture'),
            onPressed: _windowBusy || _addCell == null
                ? null
                : _captureQueueIcon,
            child: Text(_capturing ? 'Capturing…' : 'Capture pet icon'),
          ),
          _iconPreview(
            theme,
            capture?.value == null
                ? null
                : asString(capture?.value?['png_base64']),
            key: const ValueKey('pet-add-preview'),
          ),
        ],
      ),
      if (capture != null)
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xs),
          child: Text(
            _captureText(
                  capture,
                  verb: 'in the draft queue; Save writes it.',
                ) ??
                '',
            key: const ValueKey('pet-add-result'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: capture.ok ? null : theme.colorScheme.error,
            ),
          ),
        ),
      if (_queueCaptureNote != null)
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xs),
          child: Text(
            _queueCaptureNote!,
            key: const ValueKey('pet-add-note'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ),
    ];
  }

  /// One selectable bag cell of the Add form's source grid (single selection; the page comes from
  /// the chips above it).
  /// One bag cell, drawn like the Sell tab's grid: 26 px, rounded, and a check when selected so
  /// colour is not the only selected-state channel.
  Widget _bagCellVisual(ThemeData theme, {required bool selected}) {
    final colors = theme.colorScheme;
    return Container(
      width: 26,
      height: 26,
      margin: const EdgeInsets.all(2),
      decoration: BoxDecoration(
        color: selected ? colors.primary : colors.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(ShellRadius.s - 2),
        border: Border.all(
          color: selected ? colors.primary : colors.outlineVariant,
        ),
      ),
      alignment: Alignment.center,
      child: selected
          ? Icon(Icons.check, size: 13, color: colors.onPrimary)
          : null,
    );
  }

  Widget _sourceCell(ThemeData theme, int cell) {
    final selected = _addCell == cell;
    return InkWell(
      key: ValueKey('pet-add-cell-$_addPage-$cell'),
      onTap: () => _setAddCell(cell),
      child: _bagCellVisual(theme, selected: selected),
    );
  }

  /// The Scan-the-bag section: one page picker for both scans, the two READ-ONLY scan buttons, and
  /// the last results (view state). The core validates the page's tab mark but does NOT click the
  /// page — the hint carries that wrong-page caveat.
  List<Widget> _scanSection(ThemeData theme) {
    final cells = _foodScan?.value == null
        ? <List<int>>[]
        : _scannedCells(_foodScan!.value!);
    final iconSaved = _hasCrop(_machine()['food_icon_png']);
    return [
      ShellProse(
        'Put the bag page up first: the shell hides for the shot, so the game has to be in front, '
        'and the core does NOT click or verify the ITEM tab — the page you pick must already be up '
        '(a wrong page gives a plausible wrong result). Scans only LOOK; the apply below is the only '
        'write. Evidence files are best-effort and may be missing.',
        key: const ValueKey('pet-scan-hint'),
        style: theme.textTheme.bodySmall,
      ),
      const SizedBox(height: ShellSpace.sm),
      Wrap(
        spacing: ShellSpace.sm,
        children: [
          for (var page = 0; page < _bagPages; page++)
            ChoiceChip(
              key: ValueKey('pet-scan-page-$page'),
              label: Text('ITEM${page + 1}'),
              selected: _scanPage == page,
              onSelected: (_) => setState(() => _scanPage = page),
            ),
        ],
      ),
      if (!iconSaved)
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xs),
          child: ShellProse(
            'No food icon is saved yet — use Capture food icon below and Save; the scan matches '
            'that icon. The button still sends: the core refuses and names the field.',
            key: const ValueKey('pet-scan-no-food-icon'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ),
      const SizedBox(height: ShellSpace.xs),
      Wrap(
        spacing: ShellSpace.sm,
        runSpacing: ShellSpace.xs,
        children: [
          OutlinedButton(
            key: const ValueKey('pet-scan-food-page'),
            onPressed: _windowBusy ? null : _scanFoodPage,
            child: Text(_scanning ? 'Scanning…' : 'Scan this page for food'),
          ),
          OutlinedButton(
            key: const ValueKey('pet-scan-queue-page'),
            onPressed: _windowBusy ? null : _scanQueuePage,
            child: Text(_scanning ? 'Scanning…' : 'Scan this page for pets'),
          ),
        ],
      ),
      ..._foodScanWidgets(theme, cells),
      ..._queueScanWidgets(theme),
      ..._panelSection(theme),
    ];
  }

  /// The panel test read ([PROTOCOL.md] §4 `pet.test_read_panel`, OPEN §6 G13): pick the bag cell
  /// whose pet tooltip to read, press once. The game must already be in front and the chosen ITEM
  /// page up — the core hovers, it does not click the page tab or a focus point. Everything shown is
  /// the core's answer: the raw lines, its parsed line (or null), the numbers it saw and the saved
  /// evidence image.
  List<Widget> _panelSection(ThemeData theme) {
    final read = _panelRead;
    return [
      const SizedBox(height: ShellSpace.md),
      const Divider(),
      const SizedBox(height: ShellSpace.sm),
      Text('Test read one pet panel', style: theme.textTheme.titleSmall),
      const SizedBox(height: ShellSpace.xxs),
      ShellProse(
        'Hover the bag cell holding a pet and read its tooltip panel (growth and EXP). The core '
        'only hovers — it never clicks — so put the game in front with the ITEM page up first. '
        'Read-only: nothing is saved.',
        key: const ValueKey('pet-test-panel-hint'),
        style: theme.textTheme.bodySmall,
      ),
      const SizedBox(height: ShellSpace.xs),
      Wrap(
        spacing: ShellSpace.sm,
        runSpacing: ShellSpace.xs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          SizedBox(
            width: ShellWidth.fieldSm,
            child: TextFormField(
              key: const ValueKey('pet-test-panel-cell'),
              initialValue: '$_panelCell',
              decoration: const InputDecoration(
                labelText: 'Bag slot (1–64)',
                isDense: true,
              ),
              keyboardType: TextInputType.number,
              onChanged: (text) {
                final cell = int.tryParse(text.trim());
                if (cell == null || cell < 1 || cell > 64) {
                  return;
                }
                setState(() => _panelCell = cell);
              },
            ),
          ),
          OutlinedButton(
            key: const ValueKey('pet-test-panel-read'),
            onPressed: _windowBusy ? null : _testReadPanel,
            child: Text(_panelReading ? 'Reading…' : 'Test read panel'),
          ),
        ],
      ),
      ..._panelReadWidgets(theme, read),
    ];
  }

  List<Widget> _panelReadWidgets(
    ThemeData theme,
    CoreActionResult<JsonMap>? read,
  ) {
    if (read == null) {
      return const [];
    }
    final parts = <Widget>[];
    final value = read.value;
    if (!read.ok || value == null) {
      parts.add(
        _scanError(
          theme,
          'pet-test-panel-error',
          read,
          'The panel read failed.',
        ),
      );
    } else {
      final region = asJsonMap(value['region']);
      parts.add(
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xs),
          child: Text(
            'Read region: x ${asNumber(region['x'])?.round() ?? '?'} · '
            'y ${asNumber(region['y'])?.round() ?? '?'} · '
            '${asNumber(region['width'])?.round() ?? '?'}×'
            '${asNumber(region['height'])?.round() ?? '?'}',
            key: const ValueKey('pet-test-panel-region'),
            style: ShellText.mono.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ),
      );
      final lines = asStringList(value['lines']);
      parts.add(
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xs),
          child: Text(
            lines.isEmpty
                ? 'Read NOTHING — the saved image is the exact region the OCR was given, so a '
                      'wrong region and a pet that is not in that cell can be told apart by '
                      'looking at it.'
                : lines.join('\n'),
            key: const ValueKey('pet-test-panel-lines'),
            style: theme.textTheme.bodySmall,
          ),
        ),
      );
      final panel = value['panel'];
      final parsed = panel == null ? null : asString(asJsonMap(panel)['line']);
      parts.add(
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xs),
          child: Text(
            parsed == null
                ? 'Not a pet panel — no growth + EXP pair in it.'
                : 'Parsed: $parsed',
            key: const ValueKey('pet-test-panel-parsed'),
            style: parsed == null
                ? theme.textTheme.bodyMedium
                : theme.textTheme.bodyMedium?.copyWith(
                    color: theme.shellColors.success,
                  ),
          ),
        ),
      );
      final numbers = asStringList(value['numbers']);
      parts.add(
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xxs),
          child: Text(
            'Numbers seen: '
            '${numbers.isEmpty ? '(nothing read)' : numbers.join(', ')}',
            key: const ValueKey('pet-test-panel-numbers'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ),
      );
      final evidence = asString(value['evidence']);
      if (evidence != null) {
        parts.add(
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xxs),
            child: Text(
              'Evidence (best-effort): $evidence',
              key: const ValueKey('pet-test-panel-evidence'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        );
      }
    }
    if (_panelReadNote != null) {
      parts.add(_scanNoteText(theme, 'pet-test-panel-note', _panelReadNote!));
    }
    return parts;
  }

  List<Widget> _foodScanWidgets(ThemeData theme, List<List<int>> cells) {
    final scan = _foodScan;
    if (scan == null) {
      return const [];
    }
    final parts = <Widget>[];
    final value = scan.value;
    if (!scan.ok || value == null) {
      parts.add(
        _scanError(theme, 'pet-scan-food-error', scan, 'The food scan failed.'),
      );
    } else {
      final page = asNumber(value['page'])?.round() ?? 1;
      parts.add(
        Text(
          'Scanned ITEM$page — the page this command was requested for.',
          key: const ValueKey('pet-scan-food-page-echo'),
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      );
      final cellList = asJsonList(value['cells'])
          .map((cell) => '${asNumber(cell)?.round() ?? cell}')
          .join(', ');
      parts.add(
        Text(
          cells.isEmpty
              ? 'Found no food on this page.'
              : 'Found ${cells.length} cell${cells.length == 1 ? '' : 's'}: $cellList',
          key: const ValueKey('pet-scan-food-cells'),
          style: theme.textTheme.bodyMedium,
        ),
      );
      final report = asStringList(value['report']);
      if (report.isNotEmpty) {
        parts.add(
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              report.join('\n'),
              key: const ValueKey('pet-scan-food-report'),
              style: theme.textTheme.bodySmall,
            ),
          ),
        );
      }
      final evidence = asStringList(value['evidence']);
      if (evidence.isNotEmpty) {
        parts.add(
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              'Evidence (best-effort): ${evidence.join('  ·  ')}',
              key: const ValueKey('pet-scan-food-evidence'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        );
      }
      parts.add(
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.sm),
          child: Wrap(
            spacing: ShellSpace.sm,
            runSpacing: ShellSpace.xs,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              OutlinedButton(
                key: const ValueKey('pet-scan-apply-food'),
                onPressed: cells.isEmpty || _draftDirty || _windowBusy
                    ? null
                    : _applyFoodScan,
                child: Text(
                  'Apply ${cells.length} cell${cells.length == 1 ? '' : 's'} to the food list',
                ),
              ),
              if (_draftDirty)
                Text(
                  'Save the edits first',
                  key: const ValueKey('pet-scan-apply-dirty'),
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              if (_applying)
                Text(
                  'Applying…',
                  key: const ValueKey('pet-scan-applying'),
                  style: theme.textTheme.bodySmall,
                ),
            ],
          ),
        ),
      );
      final applyText = _applyText();
      if (applyText != null) {
        parts.add(
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              applyText,
              key: const ValueKey('pet-scan-apply-result'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: (_applyResult?.ok ?? false)
                    ? null
                    : theme.colorScheme.error,
              ),
            ),
          ),
        );
      }
    }
    if (_foodScanNote != null) {
      parts.add(_scanNoteText(theme, 'pet-scan-food-note', _foodScanNote!));
    }
    return parts;
  }

  List<Widget> _queueScanWidgets(ThemeData theme) {
    final scan = _queueScan;
    if (scan == null) {
      return const [];
    }
    final parts = <Widget>[];
    final value = scan.value;
    if (!scan.ok || value == null) {
      parts.add(
        _scanError(theme, 'pet-scan-queue-error', scan, 'The pet scan failed.'),
      );
    } else {
      final page = asNumber(value['page'])?.round() ?? 1;
      parts.add(
        Text(
          'Scanned ITEM$page — the page this command was requested for.',
          key: const ValueKey('pet-scan-queue-page-echo'),
          style: theme.textTheme.bodySmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
      );
      final lines = <String>[];
      var hasEmptyHits = false;
      for (final entry in asJsonList(value['per_entry'])) {
        final map = asJsonMap(entry);
        final index = asNumber(map['index'])?.round() ?? 0;
        final label = asString(map['label']) ?? '(no label)';
        final hits = asJsonList(map['hits']);
        if (hits.isEmpty) {
          hasEmptyHits = true;
          lines.add('#${index + 1} $label — no hit reported on this page');
          continue;
        }
        for (final hit in hits) {
          final match = asJsonMap(hit);
          lines.add(
            '#${index + 1} $label — cell ${asNumber(match['cell'])?.round() ?? 0} · '
            'score ${_scoreText(match['score'])} · runner-up ${_scoreText(match['runner_up'])}',
          );
        }
      }
      parts.add(
        Text(
          lines.isEmpty
              ? 'No queue icons are configured — there is nothing to look for.'
              : lines.join('\n'),
          key: const ValueKey('pet-scan-queue-entries'),
          style: theme.textTheme.bodySmall,
        ),
      );
      if (hasEmptyHits) {
        parts.add(
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              'A pet with no hit may be absent, or its stored icon may not decode — the scan '
              'report under logs/reads says which. Hits are shown in the core\'s order.',
              key: const ValueKey('pet-scan-queue-caption'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        );
      }
      final report = asStringList(value['report']);
      if (report.isNotEmpty) {
        parts.add(
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              report.join('\n'),
              key: const ValueKey('pet-scan-queue-report'),
              style: theme.textTheme.bodySmall,
            ),
          ),
        );
      }
      final claimedLines = [
        for (final entry in asJsonList(value['claimed']))
          'cell ${asNumber(asJsonMap(entry)['cell'])?.round() ?? 0} claimed by '
              '${asStringList(asJsonMap(entry)['labels']).join(', ')}',
      ];
      if (claimedLines.isNotEmpty) {
        parts.add(
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              claimedLines.join('\n'),
              key: const ValueKey('pet-scan-queue-claimed'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.tertiary,
              ),
            ),
          ),
        );
      }
      final deduced = asStringList(value['deduced']);
      if (deduced.isNotEmpty) {
        parts.add(
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              deduced.join('\n'),
              key: const ValueKey('pet-scan-queue-deduced'),
              style: theme.textTheme.bodySmall,
            ),
          ),
        );
      }
      final evidence = asStringList(value['evidence']);
      if (evidence.isNotEmpty) {
        parts.add(
          Padding(
            padding: const EdgeInsets.only(top: ShellSpace.xs),
            child: Text(
              'Evidence (best-effort): ${evidence.join('  ·  ')}',
              key: const ValueKey('pet-scan-queue-evidence'),
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
        );
      }
    }
    if (_queueScanNote != null) {
      parts.add(_scanNoteText(theme, 'pet-scan-queue-note', _queueScanNote!));
    }
    return parts;
  }

  /// The core's line for a refused scan, VERBATIM (never re-worded), under the scan buttons.
  Widget _scanError(
    ThemeData theme,
    String key,
    CoreActionResult<JsonMap> scan,
    String fallback,
  ) => Padding(
    padding: const EdgeInsets.only(top: ShellSpace.xs),
    child: Text(
      scan.errorLine ?? fallback,
      key: ValueKey(key),
      style: theme.textTheme.bodySmall?.copyWith(
        color: theme.colorScheme.error,
      ),
    ),
  );

  /// A host-channel note (the command is still sent and its own line still stands).
  Widget _scanNoteText(ThemeData theme, String key, String note) => Padding(
    padding: const EdgeInsets.only(top: ShellSpace.xs),
    child: Text(
      note,
      key: ValueKey(key),
      style: theme.textTheme.bodySmall?.copyWith(
        color: theme.colorScheme.onSurfaceVariant,
      ),
    ),
  );

  /// One row card. The header IS the row's state — `Row N`, the live chip and the next action —
  /// then the SAVED fields, then the immediate A1 control separated below with its cue: the tick
  /// writes through the core at once and is never part of this tab's draft (PLAN-V3-UI-UX §6 Pet).
  Widget _rowCard(ThemeData theme, int index, Map<String, dynamic> row) {
    final enabled = asBool(row['enabled']) ?? true;
    final boarding = _boardingRunning(index);
    // `boarding_window_may_be_open` is a config field the run writes while the boarding window is
    // open, so a boarding row with the window open is reloading rather than merely waiting. The
    // shell only renders the core's value; it never derives a schedule of its own.
    final windowOpen =
        asBool(_machine()['boarding_window_may_be_open']) ?? false;
    // G12: join the live row (core `state` rows, same order as the slots) to this configured row so
    // the saved setup and what the run is doing read together. Nothing is derived: the words are the
    // core's (`status`, `next_action`) and the seconds are its published baseline.
    final liveSnapshot = model.tools.snapshot(ToolId.pet);
    final liveRow =
        liveSnapshot != null &&
            liveSnapshot.phase != ToolPhase.idle &&
            index < liveSnapshot.rows.length
        ? liveSnapshot.rows[index]
        : null;
    final Widget chip;
    if (!enabled) {
      chip = const StatusChip(kind: StatusKind.muted, label: 'disabled');
    } else if (boarding && windowOpen) {
      chip = const StatusChip(kind: StatusKind.warning, label: 'reloading');
    } else if (boarding) {
      chip = const StatusChip(kind: StatusKind.success, label: 'boarding');
    } else {
      chip = const StatusChip(kind: StatusKind.muted, label: 'idle');
    }
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.md),
      child: Material(
        color: theme.colorScheme.surfaceContainerLowest,
        clipBehavior: Clip.antiAlias,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(ShellRadius.m),
          side: BorderSide(
            color: theme.colorScheme.outlineVariant.withValues(alpha: 0.5),
          ),
        ),
        child: Padding(
          padding: const EdgeInsets.all(ShellSpace.md),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Text('Row ${index + 1}', style: theme.textTheme.titleSmall),
                  const SizedBox(width: ShellSpace.sm),
                  chip,
                  const Spacer(),
                  if (_draftDirty)
                    Text(
                      'next: Save',
                      key: ValueKey('pet-row-$index-next'),
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                ],
              ),
              if (liveRow != null)
                Padding(
                  padding: const EdgeInsets.only(top: ShellSpace.xxs),
                  child: Text(
                    _liveRowLine(liveRow),
                    key: ValueKey('pet-row-$index-live'),
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
                ),
              const SizedBox(height: ShellSpace.xs),
              // Saved fields in one row (the demo's field grid): they ride the tab's ONE Save,
              // like the rest of this tab's setup.
              Wrap(
                spacing: ShellSpace.xl,
                runSpacing: ShellSpace.xs,
                crossAxisAlignment: WrapCrossAlignment.start,
                children: [
                  SizedBox(
                    width: 300,
                    child: DraftSwitchField(
                      key: ValueKey('pet-row-$index-enabled'),
                      label: 'Enabled',
                      helper: 'a ticked row is fed; an unticked one is ignored',
                      value: enabled,
                      onChanged: (value) => _setRow(index, 'enabled', value),
                    ),
                  ),
                  DraftFieldSlot(
                    slotKey: 'pet-row-$index-stacks',
                    syncToken: _machineDraft.seedRevision,
                    child: DraftNumberField(
                      label: 'Stacks per reload',
                      helper: 'the food strip is divided by it',
                      integer: true,
                      value: asNumber(row['stacks']),
                      width: 220,
                      onChanged: (value) =>
                          _setRow(index, 'stacks', value.round()),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.xs),
              Text(
                _rowMarksText(row),
                key: ValueKey('pet-row-$index-marks'),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(height: ShellSpace.xs),
              Divider(
                color: theme.colorScheme.outlineVariant.withValues(alpha: 0.5),
              ),
              // v2's per-row "boarding now" tick. It is NOT a draft edit: `pet.set_boarding` writes
              // the one field through the core (A1) so the run's own mid-run updates cannot clobber
              // it. It renders the MIRROR, and is disabled while anything here is unsaved (the draft
              // would disagree with the mirror the core is writing).
              CheckboxListTile(
                key: ValueKey('pet-boarding-tick-${index + 1}'),
                value: boarding,
                dense: true,
                contentPadding: EdgeInsets.zero,
                controlAffinity: ListTileControlAffinity.leading,
                title: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const Text('Boarding now'),
                    const SizedBox(width: ShellSpace.sm),
                    const StatusChip(
                      kind: StatusKind.info,
                      label: 'applies immediately',
                    ),
                  ],
                ),
                subtitle: _draftDirty ? Text(_tickBlockedReason) : null,
                onChanged: _draftDirty || _settingBoarding
                    ? null
                    : (value) => _setBoarding(index + 1, value ?? false),
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// The page picker plus the 8×8 grid of the bag page it names. Gated on the calibrated boarding
  /// bag grid: without it the run has no cell to right-click, so the editor is disabled with the
  /// core's own start refusal.
  List<Widget> _foodCellEditor(ThemeData theme, List<int>? bagGrid) {
    if (bagGrid == null) {
      return [
        ShellProse(
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
      ShellProse(
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
      ShellProse(
        'A marked page needs its ITEM tab point saved on the Calibrate tab — Start '
        'names the page that is missing one.',
        style: theme.textTheme.bodySmall?.copyWith(
          color: theme.colorScheme.onSurfaceVariant,
        ),
      ),
      const SizedBox(height: ShellSpace.sm),
      Text(
        _foodIconSourceText(),
        key: const ValueKey('pet-food-icon-source'),
        style: theme.textTheme.bodySmall,
      ),
      const SizedBox(height: ShellSpace.xs),
      Wrap(
        spacing: ShellSpace.sm,
        runSpacing: ShellSpace.xs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          OutlinedButton(
            key: const ValueKey('pet-food-icon-capture'),
            onPressed: _windowBusy || _foodIconCell == null
                ? null
                : _captureFoodIcon,
            child: Text(_capturing ? 'Capturing…' : 'Capture food icon'),
          ),
          _iconPreview(
            theme,
            asString(_machineDraft.value['food_icon_png']),
            key: const ValueKey('pet-food-icon-preview'),
          ),
        ],
      ),
      if (_foodCapture != null)
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xs),
          child: Text(
            _captureText(
                  _foodCapture,
                  verb: 'stored in the draft; Save writes it.',
                ) ??
                '',
            key: const ValueKey('pet-food-icon-result'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: _foodCapture!.ok ? null : theme.colorScheme.error,
            ),
          ),
        ),
      if (_foodCaptureNote != null)
        Padding(
          padding: const EdgeInsets.only(top: ShellSpace.xs),
          child: Text(
            _foodCaptureNote!,
            key: const ValueKey('pet-food-icon-note'),
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
        ),
    ];
  }

  Widget _foodCell(ThemeData theme, int cell, bool selected) {
    return InkWell(
      key: ValueKey('pet-food-cell-$_foodPage-$cell'),
      onTap: () => _toggleFoodCell(cell),
      child: _bagCellVisual(theme, selected: selected),
    );
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

/// The Pet row card's live line (G12): the core's live status plus its own next-action word/time.
/// Presentation only — the shell joins the live row to the configured row; it derives neither.
String _liveRowLine(ToolRow row) {
  final next = toolNextActionLabel(row);
  return next.isEmpty
      ? 'Live · ${row.status}'
      : 'Live · ${row.status} · next $next';
}

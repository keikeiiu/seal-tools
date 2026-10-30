/// The Pet tab: the feeder's setup editors (rows, queue, food cells, return slot, mode and timing)
/// plus the portable `settings.pet` fallbacks, saved together as ONE `config.apply` — with the
/// calibrated marks and reference crops displayed read-only and never sent.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/ui/components/status_banner.dart';
import 'package:seal_shell/ui/components/status_chip.dart';
import 'package:seal_shell/ui/tabs/pet_tab.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

Map<String, dynamic> _portablePet() => {
  'items_per_minute': 3,
  'pet_slot_occupied_above': 0.05,
};

Map<String, dynamic> _machinePet() => {
  ..._portablePet(),
  'menu_button': [1, 2],
  'feed_icon': [3, 4],
  'close_button': [5, 6],
  'close_button_png': 'iVBORw0KGgo=',
  'food_icon_png': 'iVBORw0KGgo=',
  'page_tabs': [
    [10, 10],
    [20, 10],
    [30, 10],
  ],
  'bag_grid': [100, 200, 300, 400],
  'bag_slot': [40, 40, 40, 40],
  'food_slots': [
    [1, 2],
    [1, 3],
  ],
  'food_slots_used': 0,
  'action_wait_ms': 1200,
  'wait_after_empty_minutes': 5,
  'feeder_count_left_fraction': 0.5,
  'feeder_count_min_score': 0.4,
  'reload_on_start': false,
  'food_load_mode': 'right_click',
  'return_slot': [1, 4],
  'slots': [
    {
      'enabled': true,
      'toggle_label': [10, 20, 100, 30],
      'boarding_pet_slot': [10, 60, 100, 80],
      'feeder_strip': [110, 20, 200, 80],
      'stacks': 5,
      'pet_slot_empty_png': 'iVBORw0KGgo=',
      'boarding_running': false,
    },
    {
      'enabled': false,
      'toggle_label': [10, 200, 100, 30],
      'boarding_pet_slot': [10, 240, 100, 80],
      'feeder_strip': [110, 200, 200, 80],
      'stacks': 2,
      'boarding_running': true,
    },
  ],
  'queue': [
    {
      'label': 'pet A',
      'species': 'Knight',
      'rect': [0, 0, 32, 32],
    },
    {
      'label': 'pet B',
      'species': 'Mage',
      'rect': [32, 0, 32, 32],
    },
  ],
};

Finder _field(String key) => find.descendant(
  of: find.byKey(ValueKey(key)),
  matching: find.byType(TextField),
);

/// A ready `pet.preflight` answer; tests override the lists/lines to exercise the checklist rendering.
Map<String, dynamic> _preflightAnswer({
  bool ready = true,
  List<Map<String, dynamic>>? calibration,
  List<Map<String, dynamic>>? session,
  List<String>? lines,
}) => {
  'ready': ready,
  'calibration':
      calibration ??
      [
        {'ok': true, 'label': 'Pet menu point', 'detail': 'marked'},
      ],
  'session':
      session ??
      [
        {'ok': true, 'label': 'return slot', 'detail': 'page 1, cell 4'},
      ],
  'lines':
      lines ??
      ['This tab is ready. 2 row(s), reloading every 205 min / 410 min.'],
};

/// A real 1×1 PNG (the fixture-style payload): the fake capture answers with it so the previews
/// decode. The core's real crop is the pitch box; a fake does not need the two to correspond.
const String _capturedPng =
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';

/// The fake core's answer to `pet.capture_icon`: echo the requested kind/page/cell and a crop box.
Map<String, dynamic> _captureAnswer(Map<String, dynamic> request) {
  final args = request['args'] as Map<String, dynamic>;
  return {
    'kind': args['kind'],
    'page': args['page'],
    'cell': args['cell'],
    'rect': [10, 40, 10, 10],
    'png_base64': _capturedPng,
  };
}

/// A fake `pet.scan_food_page` answer: 1-based pages/cells in CELL order, plus the core's report and
/// best-effort evidence lines. Overridable so a test can pick the page it scanned.
Map<String, dynamic> _foodScanAnswer({
  int page = 1,
  List<int> cells = const [13, 36],
  List<String>? report,
  List<String>? evidence,
}) => {
  'page': page,
  'cells': cells,
  'report':
      report ?? ['page $page: ${cells.length} cell(s) — ${cells.join(', ')}'],
  'evidence': evidence ?? ['logs/reads/scan_page${page}_2026.png'],
};

/// A fake `pet.scan_queue_page` answer. `index` is the 0-based queue order, `hits[].cell` is 1-based.
Map<String, dynamic> _queueScanAnswer({
  int page = 1,
  List<Map<String, dynamic>>? perEntry,
  List<Map<String, dynamic>>? claimed,
  List<String>? deduced,
  List<String>? evidence,
}) => {
  'page': page,
  'per_entry':
      perEntry ??
      [
        {
          'index': 0,
          'label': 'red',
          'hits': [
            {'cell': 13, 'score': 0.0, 'runner_up': 1.0},
          ],
        },
        {'index': 1, 'label': 'blue', 'hits': <Map<String, dynamic>>[]},
      ],
  'claimed':
      claimed ??
      [
        {
          'cell': 13,
          'labels': ['red', 'blue'],
        },
      ],
  'deduced': deduced ?? ['red ×1', 'blue ×0'],
  'evidence': evidence ?? ['logs/reads/scan_page${page}_2026.png'],
};

/// A fake `pet.test_read_panel` answer: the region, raw lines, the core's own parsed line, the
/// numbers it saw and the best-effort evidence path.
Map<String, dynamic> _panelReadAnswer({
  Map<String, dynamic>? region,
  List<String>? lines,
  Map<String, dynamic>? panel,
  List<String>? numbers,
  String? evidence,
}) => {
  'region': region ?? {'x': 33, 'y': 88, 'width': 120, 'height': 80},
  'lines': lines ?? ['(6階) 真蔚藍鳳凰 +7 [52.18%]'],
  'panel': panel ?? {'growth': 7, 'exp': 52.18, 'line': 'stage 6, +7, 52.18%'},
  'numbers': numbers ?? ['6', '7', '52.18'],
  'evidence': evidence ?? 'logs/reads/petpanel_2026.png',
};

/// The fake core's answers for the commands the Pet tab sends. The tab's RULES are the core's; a test
/// only ever needs the shape (and can override `pet.preflight` for a checklist case).
Map<String, dynamic> _petReply(Map<String, dynamic> request) {
  switch (request['cmd']) {
    case 'pet.preflight':
      return _preflightAnswer();
    case 'pet.set_boarding':
      return {'rev': 9, 'block': <String, dynamic>{}};
    case 'pet.capture_icon':
      return _captureAnswer(request);
    case 'pet.scan_food_page':
      return _foodScanAnswer(page: _requestPage(request, 1));
    case 'pet.scan_queue_page':
      return _queueScanAnswer(page: _requestPage(request, 1));
    case 'pet.test_read_panel':
      return _panelReadAnswer();
    case 'pet.apply_food_scan':
      return {'rev': 21, 'block': <String, dynamic>{}};
    case 'config.validate':
      return {'errors': <String>[], 'warnings': <String>[]};
    case 'config.apply':
      final args = request['args'];
      final sections = args is Map<String, dynamic> ? args['sections'] : null;
      return {'rev': 8, 'sections': sections ?? <String, dynamic>{}};
    default:
      return <String, dynamic>{};
  }
}

/// The `page` arg a fake request carries (the wire is 1-based; tests often just echo it).
int _requestPage(Map<String, dynamic> request, int fallback) {
  final args = request['args'];
  if (args is! Map<String, dynamic>) {
    return fallback;
  }
  final page = args['page'];
  return page is int ? page : fallback;
}

/// Installs the standard Save answers (`answerConfigCommands`) PLUS the pet reads the tab issues
/// around a save (the checklists are re-read after a successful one), so no request is left pending.
void _answerPetCore(AppHarness harness, {List<String> errors = const []}) {
  harness.answerConfigCommands(errors: errors);
  final configHandler = harness.onWrite!;
  harness.onWrite = (line, fake) {
    final message = fake.sentJson(fake.sent.length - 1);
    final cmd = message['cmd'];
    if (cmd == 'pet.preflight') {
      fake.emitJson(okResponse(message['id'] as String, _preflightAnswer()));
      return;
    }
    configHandler(line, fake);
  };
}

/// How many `pet.preflight` reads the tab has sent.
int _preflightCalls(AppHarness harness) => harness.sentMessages
    .where((message) => message['cmd'] == 'pet.preflight')
    .length;

/// Neither capture path may write config: no validate, no apply — the tab's one Save is the only
/// writer. Called on success AND refusal, before any Save in the test.
void _expectNoConfigWrites(AppHarness harness) {
  expect(harness.lastSentWithCmd('config.validate'), isNull);
  expect(harness.lastSentWithCmd('config.apply'), isNull);
}

/// The `data` of one keyed Text.
String _text(WidgetTester tester, String key) =>
    tester.widget<Text>(find.byKey(ValueKey(key))).data!;

/// The readiness banner: its kind IS the core's verdict, its title the sentence the tab states.
StatusBanner _banner(WidgetTester tester, String key) =>
    tester.widget<StatusBanner>(find.byKey(ValueKey(key)));

/// One checklist row's label and detail as rendered (the ✓/✗/ℹ marker is an icon, not text).
String _checklistRowText(WidgetTester tester, String key) => tester
    .widgetList<Text>(
      find.descendant(
        of: find.byKey(ValueKey(key)),
        matching: find.byType(Text),
      ),
    )
    .map((text) => text.data ?? '')
    .join(' — ');

/// A keyed column of one-line Texts, joined the way the old single `\n`-joined Text read.
String _linesText(WidgetTester tester, String key) => tester
    .widgetList<Text>(
      find.descendant(
        of: find.byKey(ValueKey(key)),
        matching: find.byType(Text),
      ),
    )
    .map((text) => text.data ?? '')
    .join('\n');

/// One row's boarding tick state (the widget renders the MIRROR).
bool? _tickValue(WidgetTester tester, int row) => tester
    .widget<CheckboxListTile>(find.byKey(ValueKey('pet-boarding-tick-$row')))
    .value;

Future<AppHarness> _pumpPet(
  WidgetTester tester, {
  Map<String, dynamic>? pet,
  FakeHost? host,
  Map<String, dynamic> Function(Map<String, dynamic> request)? answer,
}) async {
  tester.view.physicalSize = const ui.Size(1600, 3400);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  final harness = AppHarness(host: host);
  addTearDown(harness.model.dispose);
  await tester.pumpWidget(
    const MaterialApp(
      home: Scaffold(body: Center(child: CircularProgressIndicator())),
    ),
  );
  final connecting = harness.connect(
    config: {
      'window': {'title': 'TW_LIVE'},
      'pet': pet ?? _portablePet(),
    },
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  // The tab reads `pet.preflight` on open, so the fake core must answer BEFORE it is pumped.
  harness.onWrite = (line, fake) {
    final message = fake.sentJson(fake.sent.length - 1);
    final reply = answer?.call(message) ?? _petReply(message);
    final id = message['id'] as String;
    if (reply.containsKey('error')) {
      final error = reply['error'] as Map<String, dynamic>;
      fake.emitJson(
        errorResponse(
          id,
          error['code'] as String,
          message: error['message'] as String,
        ),
      );
    } else {
      fake.emitJson(okResponse(id, reply));
    }
  };
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(
        body: PetTab(model: harness.model, host: harness.host),
      ),
    ),
  );
  // One frame for the first build, one for the decoded `pet.preflight` answer's setState.
  await tester.pump();
  await tester.pump();
  return harness;
}

Future<void> _flush(WidgetTester tester) async {
  for (var i = 0; i < 5; i++) {
    await tester.pump();
  }
}

/// Saves and returns the `sections` the tab sent. Also answers the checklist re-read the Save starts
/// (the checklists describe the saved config, so a save re-reads them once).
Future<Map<String, dynamic>> _save(
  WidgetTester tester,
  AppHarness harness,
) async {
  _answerPetCore(harness);
  await tester.tap(find.byKey(const ValueKey('save-pet')));
  await _flush(tester);
  final apply = harness.lastSentWithCmd('config.apply');
  expect(apply, isNotNull);
  final args = apply!['args'] as Map<String, dynamic>;
  return args['sections'] as Map<String, dynamic>;
}

Future<void> _tap(WidgetTester tester, String key) async {
  final finder = find.byKey(ValueKey(key));
  // The Pet tab's editors run past one screen at the default test surface: scroll the control into
  // view so the tap lands on it rather than on whatever is painted at that offset.
  await tester.ensureVisible(finder);
  await tester.pump();
  await tester.tap(finder);
  await tester.pump();
  await tester.pump();
}

Future<void> _tapSwitch(WidgetTester tester, String key) async {
  final finder = find.descendant(
    of: find.byKey(ValueKey(key)),
    matching: find.byType(Switch),
  );
  await tester.ensureVisible(finder);
  await tester.pump();
  await tester.tap(finder);
  await tester.pump();
  await tester.pump();
}

/// The food-cell count line as rendered, so a missed toggle reports the actual text.
String? _countText(WidgetTester tester) {
  final line = tester.widget<Text>(
    find.byKey(const ValueKey('pet-food-count')),
  );
  return line.data;
}

void main() {
  testWidgets(
    'the dev shape has no rows, disables food grid, saves settings.pet',
    (tester) async {
      final harness = await _pumpPet(tester);

      // No machine rows/crops: the committed dev config is an explicit state, not a blank.
      expect(find.byKey(const ValueKey('pet-no-rows')), findsOneWidget);
      expect(find.byKey(const ValueKey('pet-queue-empty')), findsOneWidget);
      expect(
        find.text(
          "The boarding bag's grid isn't calibrated — save the Boarding bag grid "
          'and Boarding bag slot on the Calibrate tab. (Its own, not the shop\'s.)',
        ),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('pet-food-page-0')), findsNothing);
      final rateField = tester.widget<TextField>(
        _field('pet-items-per-minute'),
      );
      expect(rateField.controller?.text, '3');

      await tester.enterText(_field('pet-items-per-minute'), '5');
      await tester.pump();
      expect(find.byKey(const ValueKey('dirty-pet')), findsOneWidget);

      final sections = await _save(tester, harness);
      expect(sections.keys, [
        'settings',
      ], reason: 'there is no machine pet block to send');
      final settings = sections['settings'] as Map<String, dynamic>;
      final pet = settings['pet'] as Map<String, dynamic>;
      expect(pet['items_per_minute'], 5);
      expect(pet['pet_slot_occupied_above'], 0.05);
      expect(pet.containsKey('slots'), isFalse);
      expect(find.byKey(const ValueKey('save-ok-pet')), findsOneWidget);
    },
  );

  testWidgets(
    'rows: the tick and stacks save, and the read-only marks/crops ride along',
    (tester) async {
      final harness = await _pumpPet(tester, pet: _machinePet());

      expect(
        find.textContaining('start/end x 10 · y 20 · 100×30'),
        findsWidgets,
      );
      expect(find.textContaining('empty-slot crop saved'), findsOneWidget);
      expect(find.textContaining('boarding running'), findsOneWidget);
      expect(find.byKey(const ValueKey('pet-rows-caption')), findsOneWidget);

      await _tapSwitch(tester, 'pet-row-0-enabled');
      await tester.enterText(_field('pet-row-0-stacks'), '3');
      await tester.pump();
      expect(find.byKey(const ValueKey('dirty-pet')), findsOneWidget);

      final sections = await _save(tester, harness);
      expect(sections.keys, ['pet', 'settings']);
      final pet = sections['pet'] as Map<String, dynamic>;
      final slots = (pet['slots'] as List).cast<Map<String, dynamic>>();
      expect(slots, hasLength(2));
      expect(slots[0]['enabled'], false);
      expect(slots[0]['stacks'], 3);
      // The fields this tab does not own are echoed back exactly, not dropped.
      expect(slots[0]['toggle_label'], [10, 20, 100, 30]);
      expect(slots[0]['boarding_pet_slot'], [10, 60, 100, 80]);
      expect(slots[0]['feeder_strip'], [110, 20, 200, 80]);
      expect(slots[0]['pet_slot_empty_png'], 'iVBORw0KGgo=');
      expect(slots[0]['boarding_running'], false);
      expect(slots[1]['enabled'], false);
      expect(slots[1]['stacks'], 2);
      for (final key in [
        'menu_button',
        'bag_grid',
        'bag_slot',
        'page_tabs',
        'close_button_png',
        'food_slots_used',
        'max_button',
      ]) {
        expect(
          pet.containsKey(key),
          isFalse,
          reason: '$key is calibration/session state, not setup',
        );
      }
      final settings = sections['settings'] as Map<String, dynamic>;
      final portable = settings['pet'] as Map<String, dynamic>;
      expect(
        portable['items_per_minute'],
        3,
        reason: 'the portable half rides the same save',
      );
    },
  );

  testWidgets('removing a queue entry saves the shortened queue', (
    tester,
  ) async {
    final harness = await _pumpPet(tester, pet: _machinePet());
    expect(_field('pet-queue-label-1'), findsOneWidget);
    expect(find.byKey(const ValueKey('pet-queue-caption')), findsOneWidget);

    await _tap(tester, 'pet-queue-remove-0');

    final sections = await _save(tester, harness);
    final pet = sections['pet'] as Map<String, dynamic>;
    expect(pet['queue'], [
      {
        'label': 'pet B',
        'species': 'Mage',
        'rect': [32, 0, 32, 32],
      },
    ]);
  });

  testWidgets(
    'food cells save as [page, cell] pairs and never write food_slots_used',
    (tester) async {
      final harness = await _pumpPet(tester, pet: _machinePet());

      expect(
        _countText(tester),
        '0 of 64 cells marked on this page · 2 marked overall',
      );
      await _tap(tester, 'pet-food-cell-0-0');
      expect(
        _countText(tester),
        '1 of 64 cells marked on this page · 3 marked overall',
      );

      await _tap(tester, 'pet-food-page-1');
      expect(find.byKey(const ValueKey('pet-food-cell-1-5')), findsOneWidget);
      await _tap(tester, 'pet-food-cell-1-5');

      final sections = await _save(tester, harness);
      final pet = sections['pet'] as Map<String, dynamic>;
      expect(pet['food_slots'], [
        [0, 0],
        [1, 2],
        [1, 3],
        [1, 5],
      ]);
      expect(
        pet.containsKey('food_slots_used'),
        isFalse,
        reason:
            'the run reads the starting used-count; it is not a count we own',
      );
    },
  );

  testWidgets('the scalars and the load mode ride the same save', (
    tester,
  ) async {
    final harness = await _pumpPet(tester, pet: _machinePet());

    // The config stores the page 0-based; the field shows it 1-based.
    expect(
      tester.widget<TextField>(_field('pet-return-page')).controller?.text,
      '2',
    );
    expect(
      tester.widget<TextField>(_field('pet-return-cell')).controller?.text,
      '4',
    );

    await tester.enterText(_field('pet-return-page'), '3');
    await tester.pump();
    await tester.enterText(_field('pet-return-cell'), '7');
    await tester.pump();

    await _tap(tester, 'pet-food-load-mode');
    await tester.pumpAndSettle();
    await tester.tap(find.text('drag').last);
    await tester.pumpAndSettle();

    await _tapSwitch(tester, 'pet-reload-on-start');
    await tester.enterText(_field('pet-action-wait'), '800');
    await tester.enterText(_field('pet-wait-after-empty'), '10');
    await tester.enterText(_field('pet-feeder-left-fraction'), '0.6');
    await tester.enterText(_field('pet-feeder-min-score'), '0.8');
    await tester.pump();

    final sections = await _save(tester, harness);
    final pet = sections['pet'] as Map<String, dynamic>;
    expect(pet['return_slot'], [2, 7]);
    expect(pet['food_load_mode'], 'drag');
    expect(pet['reload_on_start'], true);
    expect(pet['action_wait_ms'], 800);
    expect(pet['wait_after_empty_minutes'], 10);
    expect(pet['feeder_count_left_fraction'], 0.6);
    expect(pet['feeder_count_min_score'], 0.8);
  });

  testWidgets(
    'the food-cell grid is disabled without a calibrated boarding bag grid',
    (tester) async {
      final machine = Map<String, dynamic>.of(_machinePet())
        ..remove('bag_grid');
      await _pumpPet(tester, pet: machine);

      expect(
        find.byKey(const ValueKey('pet-food-not-calibrated')),
        findsOneWidget,
      );
      expect(find.byKey(const ValueKey('pet-food-page-0')), findsNothing);
      expect(find.byKey(const ValueKey('pet-food-cell-0-0')), findsNothing);
      // Only the grid needs the bag mark; the setup fields stay editable.
      expect(find.byKey(const ValueKey('pet-row-0-enabled')), findsOneWidget);
      expect(_field('pet-row-0-stacks'), findsOneWidget);
    },
  );

  testWidgets(
    'a refused save renders the core line verbatim and keeps the edits',
    (tester) async {
      final harness = await _pumpPet(tester, pet: _machinePet());
      await _tapSwitch(tester, 'pet-row-0-enabled');

      _answerPetCore(harness, errors: ['a breeding row needs a stack count']);
      await tester.tap(find.byKey(const ValueKey('save-pet')));
      await _flush(tester);

      expect(find.text('a breeding row needs a stack count'), findsOneWidget);
      expect(harness.lastSentWithCmd('config.apply'), isNull);
      expect(find.byKey(const ValueKey('dirty-pet')), findsOneWidget);
    },
  );

  testWidgets('Cancel restores the machine and portable drafts', (
    tester,
  ) async {
    await _pumpPet(tester, pet: _machinePet());

    await _tapSwitch(tester, 'pet-row-0-enabled');
    await tester.enterText(_field('pet-row-0-stacks'), '3');
    await tester.enterText(_field('pet-items-per-minute'), '5');
    await tester.pump();
    expect(find.byKey(const ValueKey('dirty-pet')), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('cancel-pet')));
    await tester.pump();
    await tester.pump();

    expect(find.byKey(const ValueKey('dirty-pet')), findsNothing);
    expect(
      tester.widget<TextField>(_field('pet-row-0-stacks')).controller?.text,
      '5',
    );
    final rateField = tester.widget<TextField>(_field('pet-items-per-minute'));
    expect(rateField.controller?.text, '3');
  });

  testWidgets(
    'the checklists are read once on open and render the core lists and lines',
    (tester) async {
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        answer: (request) => request['cmd'] == 'pet.preflight'
            ? _preflightAnswer(
                ready: false,
                calibration: [
                  {'ok': true, 'label': 'Pet menu point', 'detail': 'marked'},
                  {
                    'ok': false,
                    'label': 'Boarding bag grid',
                    'detail': 'not marked',
                  },
                ],
                session: [
                  {
                    'ok': true,
                    'label': 'return slot',
                    'detail': 'page 2, cell 4',
                  },
                  {'ok': false, 'label': 'food cells'},
                  // INFORMATIONAL: ok:true, but the detail says the core did not check it. The
                  // shell must render it as information, never as a passed check.
                  {
                    'ok': true,
                    'label': 'board firmware',
                    'detail': 'not checked by preflight — normal',
                  },
                ],
                // The readiness lines are the CORE's, rendered verbatim — never composed here. The
                // list is arbitrary: v2's ready line, the new "nothing to feed" line, a fallback.
                lines: [
                  'This tab is ready. 2 row(s), reloading every 205 min / 410 min.',
                  'every row is unticked — nothing to feed',
                  'no pet icon is queued — the tool will use the return slot instead',
                ],
              )
            : _petReply(request),
      );

      expect(_preflightCalls(harness), 1);
      expect(harness.lastSentWithCmd('config.validate'), isNull);
      expect(harness.lastSentWithCmd('config.apply'), isNull);

      final banner = _banner(tester, 'pet-preflight-ready');
      expect(
        banner.kind,
        StatusKind.warning,
        reason: 'the verdict colour IS the core\'s readiness result',
      );
      expect(banner.title, 'Not ready yet — see the lines below.');
      expect(
        _checklistRowText(tester, 'pet-preflight-calibration-0'),
        'Pet menu point — marked',
      );
      expect(
        _checklistRowText(tester, 'pet-preflight-calibration-1'),
        'Boarding bag grid — not marked',
      );
      expect(
        _checklistRowText(tester, 'pet-preflight-session-0'),
        'return slot — page 2, cell 4',
      );
      expect(
        _checklistRowText(tester, 'pet-preflight-session-1'),
        'food cells',
      );
      expect(
        _checklistRowText(tester, 'pet-preflight-session-2'),
        'board firmware — not checked by preflight — normal',
      );
      // INFORMATIONAL: ok:true with a "not checked by preflight" detail renders the ℹ icon, never
      // the passed-check icon.
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('pet-preflight-session-2')),
          matching: find.byIcon(Icons.info_outline),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('pet-preflight-session-2')),
          matching: find.byIcon(Icons.check_circle_outline),
        ),
        findsNothing,
        reason: 'ok:true with a "not checked by preflight" detail is INFORMATION, not a passed check',
      );
      expect(
        _linesText(tester, 'pet-preflight-lines'),
        'This tab is ready. 2 row(s), reloading every 205 min / 410 min.\n'
        'every row is unticked — nothing to feed\n'
        'no pet icon is queued — the tool will use the return slot instead',
        reason:
            'the readiness lines are verbatim; the shell never composes one',
      );

      // A rebuild is not a re-read.
      await tester.pump();
      expect(_preflightCalls(harness), 1);
    },
  );

  testWidgets('Refresh re-reads the checklists; a rebuild does not', (
    tester,
  ) async {
    final harness = await _pumpPet(tester, pet: _machinePet());
    expect(_preflightCalls(harness), 1);
    final banner = _banner(tester, 'pet-preflight-ready');
    expect(banner.kind, StatusKind.success);
    expect(banner.title, 'Ready to run.');
    expect(
      _linesText(tester, 'pet-preflight-lines'),
      'This tab is ready. 2 row(s), reloading every 205 min / 410 min.',
    );

    await tester.pump();
    expect(_preflightCalls(harness), 1);

    await _tap(tester, 'pet-preflight-refresh');
    await _flush(tester);
    expect(_preflightCalls(harness), 2);
  });

  testWidgets(
    'a dirty draft marks the checklists stale and disables the boarding ticks',
    (tester) async {
      await _pumpPet(tester, pet: _machinePet());
      expect(find.byKey(const ValueKey('pet-preflight-stale')), findsNothing);

      // The PORTABLE half feeds the same preflight, so editing it alone marks the checklists stale.
      // It also blocks the ticks: their core write would flag the dirty portable part as an
      // external change (the scaffold's reconcile banner) — for the player's own tick.
      await tester.enterText(_field('pet-items-per-minute'), '5');
      await tester.pump();
      expect(find.byKey(const ValueKey('pet-preflight-stale')), findsOneWidget);
      expect(
        tester
            .widget<CheckboxListTile>(
              find.byKey(const ValueKey('pet-boarding-tick-1')),
            )
            .onChanged,
        isNull,
      );
      expect(find.text('Save the edits first'), findsNWidgets(2));

      await tester.tap(find.byKey(const ValueKey('cancel-pet')));
      await _flush(tester);
      expect(find.byKey(const ValueKey('pet-preflight-stale')), findsNothing);

      // A MACHINE-row edit disables the ticks too, with the reason beside each row.
      await tester.enterText(_field('pet-row-0-stacks'), '3');
      await tester.pump();

      expect(find.byKey(const ValueKey('pet-preflight-stale')), findsOneWidget);
      expect(
        _text(tester, 'pet-preflight-stale'),
        'Based on the saved config — Save to refresh',
      );
      final tick = tester.widget<CheckboxListTile>(
        find.byKey(const ValueKey('pet-boarding-tick-1')),
      );
      expect(
        tick.onChanged,
        isNull,
        reason:
            'the tick is an A1 write; a dirty row draft cannot coexist with it',
      );
      expect(find.text('Save the row edits first'), findsNWidgets(2));
    },
  );

  testWidgets(
    'a boarding tick sends exactly pet.set_boarding and never config.apply',
    (tester) async {
      final harness = await _pumpPet(tester, pet: _machinePet());
      expect(_preflightCalls(harness), 1);
      // The fixture's row 2 starts boarding; the tick renders the MIRROR.
      expect(_tickValue(tester, 2), isTrue);

      await _tap(tester, 'pet-boarding-tick-2');
      await _flush(tester);

      expect(harness.lastSentWithCmd('pet.set_boarding')!['args'], {
        'row': 2,
        'running': false,
      });
      expect(harness.lastSentWithCmd('config.apply'), isNull);
      expect(harness.lastSentWithCmd('config.validate'), isNull);
      expect(
        _preflightCalls(harness),
        2,
        reason: 'a successful tick re-reads the checklists',
      );

      // The core's ONE config.changed is what updates the tick: the tab renders the mirror.
      final pet = Map<String, dynamic>.of(_machinePet());
      final slots = (pet['slots'] as List).cast<Map<String, dynamic>>();
      slots[1] = <String, Object>{...slots[1], 'boarding_running': false};
      harness.emitConfigChanged(rev: 9, block: 'pet', values: pet);
      await _flush(tester);
      expect(_tickValue(tester, 2), isFalse);
    },
  );

  testWidgets('a refused boarding tick renders the core line verbatim', (
    tester,
  ) async {
    final harness = await _pumpPet(
      tester,
      pet: _machinePet(),
      answer: (request) => request['cmd'] == 'pet.set_boarding'
          ? {
              'error': {
                'code': 'bad_request',
                'message': 'pet.set_boarding names row 2, but this config has 2 rows (1..2)',
              },
            }
          : _petReply(request),
    );

    await _tap(tester, 'pet-boarding-tick-2');
    await _flush(tester);

    expect(
      _text(tester, 'pet-boarding-error'),
      'pet.set_boarding names row 2, but this config has 2 rows (1..2)',
    );
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    expect(
      _preflightCalls(harness),
      1,
      reason: 'a refusal changes nothing, so nothing is re-read',
    );
  });

  testWidgets('a successful Save re-reads the checklists once', (tester) async {
    final harness = await _pumpPet(tester, pet: _machinePet());
    expect(_preflightCalls(harness), 1);

    await _tapSwitch(tester, 'pet-row-0-enabled');
    await _save(tester, harness);

    expect(
      _preflightCalls(harness),
      2,
      reason:
          'the checklists describe the saved config, so the save re-reads them',
    );
    expect(
      find.byKey(const ValueKey('pet-preflight-stale')),
      findsNothing,
      reason: 'the Save left the draft clean, so the stale caption is gone',
    );
  });

  testWidgets(
    'capturing a pet icon hides, captures and shows in order, then drafts the entry',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.capture_icon') {
            // Records WHEN the command was sent, between the hide and the show.
            host.lifecycle.add('capture-sent');
            return _captureAnswer(request);
          }
          return _petReply(request);
        },
      );

      await tester.enterText(
        find.byKey(const ValueKey('pet-add-label')),
        'new pet',
      );
      await tester.enterText(
        find.byKey(const ValueKey('pet-add-species')),
        'Knight',
      );
      await _tap(tester, 'pet-add-page-1');
      await _tap(tester, 'pet-add-cell-1-34');
      expect(
        _text(tester, 'pet-add-source'),
        'Source: ITEM2 · cell 35 (row 5, column 3)',
      );
      await _tap(tester, 'pet-add-capture');
      await _flush(tester);

      expect(host.lifecycle, [
        'hide',
        'capture-sent',
        'show',
      ], reason: 'the shell hides for the shot and is shown again afterwards');
      expect(harness.lastSentWithCmd('pet.capture_icon')!['args'], {
        'kind': 'queue',
        'page': 2,
        'cell': 35,
      }, reason: 'page and cell are 1-BASED on the wire');
      _expectNoConfigWrites(harness);
      expect(find.byKey(const ValueKey('pet-add-preview')), findsOneWidget);
      expect(
        _text(tester, 'pet-add-result'),
        contains('Captured 10×10 px from ITEM2 · cell 35'),
      );

      // DRAFTED: the entry is in the queue list (removable like the others) and the one Save writes
      // the captured png/rect with the typed label/species.
      expect(_field('pet-queue-label-2'), findsOneWidget);
      final sections = await _save(tester, harness);
      final queue = (sections['pet'] as Map<String, dynamic>)['queue'] as List;
      expect(queue, hasLength(3));
      expect(queue[2]['label'], 'new pet');
      expect(queue[2]['species'], 'Knight');
      expect(queue[2]['rect'], [10, 40, 10, 10]);
      expect(queue[2]['png'], _capturedPng);

      // Removable before Save, like any other entry.
      await _tap(tester, 'pet-queue-remove-2');
      expect(find.byKey(const ValueKey('pet-queue-label-2')), findsNothing);
    },
  );

  testWidgets(
    'a refused capture shows the shell again and leaves the draft alone',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.capture_icon') {
            host.lifecycle.add('capture-sent');
            return {
              'error': {
                'code': 'tool_busy',
                'message': 'Magic Tuner is running — stop it before capturing',
              },
            };
          }
          return _petReply(request);
        },
      );

      await _tap(tester, 'pet-add-cell-0-7');
      await _tap(tester, 'pet-add-capture');
      await _flush(tester);

      expect(host.lifecycle, [
        'hide',
        'capture-sent',
        'show',
      ], reason: 'a refusal must never leave the shell hidden');
      expect(
        _text(tester, 'pet-add-result'),
        'Magic Tuner is running — stop it before capturing',
        reason: 'the core refusal is rendered verbatim',
      );
      expect(
        find.byKey(const ValueKey('pet-queue-label-2')),
        findsNothing,
        reason: 'a refused capture adds nothing to the queue draft',
      );
      _expectNoConfigWrites(harness);
    },
  );

  testWidgets(
    'a core bag_grid refusal is rendered verbatim and shows the shell again',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.capture_icon') {
            host.lifecycle.add('capture-sent');
            return {
              'error': {
                'code': 'bad_request',
                'message':
                    "The boarding bag grid isn't calibrated — Calibrate Pet.",
              },
            };
          }
          return _petReply(request);
        },
      );

      await _tap(tester, 'pet-add-cell-0-7');
      await _tap(tester, 'pet-add-capture');
      await _flush(tester);

      expect(host.lifecycle, ['hide', 'capture-sent', 'show']);
      expect(
        find.text("The boarding bag grid isn't calibrated — Calibrate Pet."),
        findsOneWidget,
      );
      _expectNoConfigWrites(harness);
    },
  );

  testWidgets(
    'capturing a food icon hides, captures and shows, then drafts food_icon_png',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.capture_icon') {
            host.lifecycle.add('capture-sent');
            return _captureAnswer(request);
          }
          return _petReply(request);
        },
      );

      // The source starts at the last restored food mark: food_slots [[1,2],[1,3]] are 0-based, so
      // page 2 (ITEM2) and cell 4 — and the payload below is 1-based.
      expect(_text(tester, 'pet-food-icon-source'), contains('ITEM2 · cell 4'));
      await _tap(tester, 'pet-food-icon-capture');
      await _flush(tester);

      expect(host.lifecycle, ['hide', 'capture-sent', 'show']);
      expect(harness.lastSentWithCmd('pet.capture_icon')!['args'], {
        'kind': 'food',
        'page': 2,
        'cell': 4,
      });
      expect(
        _text(tester, 'pet-food-icon-result'),
        contains('from ITEM2 · cell 4'),
      );
      _expectNoConfigWrites(harness);

      final sections = await _save(tester, harness);
      expect(
        (sections['pet'] as Map<String, dynamic>)['food_icon_png'],
        _capturedPng,
        reason: 'the captured food icon is the draft field the Save writes',
      );
    },
  );

  testWidgets('capture buttons are absent without a calibrated bag grid', (
    tester,
  ) async {
    final machine = Map<String, dynamic>.of(_machinePet())..remove('bag_grid');
    final host = FakeHost();
    final harness = await _pumpPet(tester, pet: machine, host: host);

    expect(
      find.byKey(const ValueKey('pet-add-not-calibrated')),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('pet-add-capture')), findsNothing);
    expect(find.byKey(const ValueKey('pet-food-icon-capture')), findsNothing);
    expect(harness.lastSentWithCmd('pet.capture_icon'), isNull);
    expect(host.hideCalls, 0, reason: 'no capture means no hide/show');
  });

  testWidgets('Capture food icon needs a marked food cell', (tester) async {
    final machine = Map<String, dynamic>.of(_machinePet())
      ..remove('food_slots');
    await _pumpPet(tester, pet: machine);

    expect(
      _text(tester, 'pet-food-icon-source'),
      'Mark a food cell first — the icon is cropped from that cell.',
    );
    final button = tester.widget<OutlinedButton>(
      find.byKey(const ValueKey('pet-food-icon-capture')),
    );
    expect(button.onPressed, isNull);
  });

  testWidgets('queue label/species edits ride the one config.apply', (
    tester,
  ) async {
    final harness = await _pumpPet(tester, pet: _machinePet());
    await tester.enterText(_field('pet-queue-label-0'), 'renamed');
    await tester.enterText(_field('pet-queue-species-0'), 'Mage');
    await tester.pump();

    final sections = await _save(tester, harness);
    final queue = (sections['pet'] as Map<String, dynamic>)['queue'] as List;
    expect(queue[0]['label'], 'renamed');
    expect(queue[0]['species'], 'Mage');
    expect(
      harness.sentMessages.where((message) => message['cmd'] == 'config.apply'),
      hasLength(1),
      reason: 'the tab keeps ONE Save; edits do not apply on their own',
    );
  });

  testWidgets(
    'the food-icon source clears when a draft reseed drops its mark',
    (tester) async {
      final machine = Map<String, dynamic>.of(_machinePet())
        ..remove('food_slots');
      await _pumpPet(tester, pet: machine);
      const noSource =
          'Mark a food cell first — the icon is cropped from that cell.';
      expect(_text(tester, 'pet-food-icon-source'), noSource);
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-food-icon-capture')),
            )
            .onPressed,
        isNull,
        reason: 'nothing is marked yet',
      );

      // Marking a cell makes it the capture source (v2's "the newest mark").
      await _tap(tester, 'pet-food-cell-0-5');
      expect(_text(tester, 'pet-food-icon-source'), contains('ITEM1 · cell 6'));
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-food-icon-capture')),
            )
            .onPressed,
        isNotNull,
      );

      // Cancel reseeds the draft back to the saved config (no food marks): the remembered source
      // must clear with the reseed — disabled state, and the stale cell is not offered anywhere.
      await tester.tap(find.byKey(const ValueKey('cancel-pet')));
      await _flush(tester);
      expect(_text(tester, 'pet-food-icon-source'), noSource);
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-food-icon-capture')),
            )
            .onPressed,
        isNull,
      );
      expect(find.textContaining('ITEM1 · cell 6'), findsNothing);
    },
  );

  testWidgets('a missing hide channel is noted and the capture still runs', (
    tester,
  ) async {
    final host = FakeHost(failHide: true);
    final harness = await _pumpPet(
      tester,
      pet: _machinePet(),
      host: host,
      answer: (request) {
        if (request['cmd'] == 'pet.capture_icon') {
          host.lifecycle.add('capture-sent');
          return _captureAnswer(request);
        }
        return _petReply(request);
      },
    );

    await _tap(tester, 'pet-add-cell-0-7');
    await _tap(tester, 'pet-add-capture');
    await _flush(tester);

    expect(host.lifecycle, [
      'hide',
      'capture-sent',
      'show',
    ], reason: 'a failed hide is a note, not a reason to skip the capture');
    expect(
      _text(tester, 'pet-add-note'),
      'the shell could not hide itself — the game has to be in front for a capture',
    );
    expect(
      harness.sentMessages.where(
        (message) => message['cmd'] == 'pet.capture_icon',
      ),
      hasLength(1),
      reason: 'the command is still sent through a missing host channel',
    );
    expect(
      _text(tester, 'pet-add-result'),
      contains('Captured 10×10 px'),
      reason: 'the capture result path is unchanged',
    );
    expect(_field('pet-queue-label-2'), findsOneWidget);
    _expectNoConfigWrites(harness);
  });

  testWidgets('a missing hide channel on the food path is noted too', (
    tester,
  ) async {
    final host = FakeHost(failHide: true);
    final harness = await _pumpPet(
      tester,
      pet: _machinePet(),
      host: host,
      answer: (request) {
        if (request['cmd'] == 'pet.capture_icon') {
          host.lifecycle.add('capture-sent');
          return _captureAnswer(request);
        }
        return _petReply(request);
      },
    );

    await _tap(tester, 'pet-food-icon-capture');
    await _flush(tester);

    expect(host.lifecycle, ['hide', 'capture-sent', 'show']);
    expect(
      _text(tester, 'pet-food-icon-note'),
      'the shell could not hide itself — the game has to be in front for a capture',
    );
    expect(
      _text(tester, 'pet-food-icon-result'),
      contains('from ITEM2 · cell 4'),
    );
    _expectNoConfigWrites(harness);
  });

  testWidgets('a missing show channel is noted and does not throw', (
    tester,
  ) async {
    final host = FakeHost(failShow: true);
    final harness = await _pumpPet(
      tester,
      pet: _machinePet(),
      host: host,
      answer: (request) {
        if (request['cmd'] == 'pet.capture_icon') {
          host.lifecycle.add('capture-sent');
          return _captureAnswer(request);
        }
        return _petReply(request);
      },
    );

    await _tap(tester, 'pet-add-cell-0-7');
    await _tap(tester, 'pet-add-capture');
    await _flush(tester);

    expect(host.lifecycle, ['hide', 'capture-sent', 'show']);
    expect(
      _text(tester, 'pet-add-note'),
      'the shell could not be shown again — no host channel',
    );
    expect(_text(tester, 'pet-add-result'), contains('Captured 10×10 px'));
    expect(_field('pet-queue-label-2'), findsOneWidget);
    _expectNoConfigWrites(harness);
  });

  testWidgets(
    'a refused capture with a failing show still renders the core line',
    (tester) async {
      final host = FakeHost(failShow: true);
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.capture_icon') {
            host.lifecycle.add('capture-sent');
            return {
              'error': {
                'code': 'game_not_foreground',
                'message': 'the game is not the foreground window',
              },
            };
          }
          return _petReply(request);
        },
      );

      await _tap(tester, 'pet-add-cell-0-7');
      await _tap(tester, 'pet-add-capture');
      await _flush(tester);

      expect(host.lifecycle, [
        'hide',
        'capture-sent',
        'show',
      ], reason: 'the show is still ATTEMPTED when the capture is refused');
      expect(
        _text(tester, 'pet-add-result'),
        'the game is not the foreground window',
        reason: 'the core refusal is rendered verbatim',
      );
      expect(
        _text(tester, 'pet-add-note'),
        'the shell could not be shown again — no host channel',
      );
      expect(_field('pet-queue-label-2'), findsNothing);
      _expectNoConfigWrites(harness);
    },
  );

  testWidgets('only one capture can be in flight at a time', (tester) async {
    final host = FakeHost();
    final harness = await _pumpPet(tester, pet: _machinePet(), host: host);
    // Swallow the capture reply so the request stays in flight for the assertions below.
    harness.onWrite = (line, fake) {};

    await _tap(tester, 'pet-add-cell-0-7');
    await _tap(tester, 'pet-add-capture');
    await tester.pump();
    await tester.pump();

    expect(
      tester
          .widget<OutlinedButton>(find.byKey(const ValueKey('pet-add-capture')))
          .onPressed,
      isNull,
      reason: 'the capture button is disabled while one capture is in flight',
    );
    expect(
      tester
          .widget<OutlinedButton>(
            find.byKey(const ValueKey('pet-food-icon-capture')),
          )
          .onPressed,
      isNull,
      reason: 'a second capture cannot start from the food path either',
    );

    // A tap on the disabled button starts nothing.
    await tester.tap(
      find.byKey(const ValueKey('pet-add-capture')),
      warnIfMissed: false,
    );
    await tester.pump();
    expect(
      harness.sentMessages.where(
        (message) => message['cmd'] == 'pet.capture_icon',
      ),
      hasLength(1),
      reason: 'exactly one pet.capture_icon is in flight',
    );
    expect(find.byKey(const ValueKey('pet-queue-label-2')), findsNothing);
    _expectNoConfigWrites(harness);

    // Answer the one in-flight request so the test leaves nothing pending; the buttons come back.
    harness.respondOk(_captureAnswer(harness.lastRequest));
    await _flush(tester);
    expect(
      tester
          .widget<OutlinedButton>(find.byKey(const ValueKey('pet-add-capture')))
          .onPressed,
      isNotNull,
    );
    expect(_field('pet-queue-label-2'), findsOneWidget);
  });

  testWidgets(
    'a food scan hides, scans and shows, and renders the cells, report and evidence',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.scan_food_page') {
            host.lifecycle.add('scan-sent');
            return _foodScanAnswer(
              page: 2,
              cells: [13, 36],
              report: [
                'page 2: 2 cell(s) — 13, 36',
                '  cell 13: 0.000 (limit 0.500)',
                '  runner-up: cell 1 at 1.000',
              ],
              evidence: [
                'logs/reads/scan_page2_1.png',
                'logs/reads/scan_1.txt',
              ],
            );
          }
          return _petReply(request);
        },
      );

      await _tap(tester, 'pet-scan-page-1');
      await _tap(tester, 'pet-scan-food-page');
      await _flush(tester);

      expect(host.lifecycle, ['hide', 'scan-sent', 'show']);
      expect(harness.lastSentWithCmd('pet.scan_food_page')!['args'], {
        'page': 2,
      });
      expect(_text(tester, 'pet-scan-food-cells'), 'Found 2 cells: 13, 36');
      expect(
        _text(tester, 'pet-scan-food-page-echo'),
        'Scanned ITEM2 — the page this command was requested for.',
        reason: 'the requested page is always shown next to the result',
      );
      expect(
        _text(tester, 'pet-scan-food-report'),
        'page 2: 2 cell(s) — 13, 36\n'
        '  cell 13: 0.000 (limit 0.500)\n'
        '  runner-up: cell 1 at 1.000',
        reason: 'the core report is rendered verbatim',
      );
      expect(
        _text(tester, 'pet-scan-food-evidence'),
        contains('logs/reads/scan_page2_1.png'),
      );
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-scan-apply-food')),
            )
            .onPressed,
        isNotNull,
      );
      expect(find.text('Apply 2 cells to the food list'), findsOneWidget);
      expect(find.byKey(const ValueKey('pet-scan-no-food-icon')), findsNothing);
      expect(
        find.byKey(const ValueKey('dirty-pet')),
        findsNothing,
        reason: 'a scan is view state — it never edits the draft',
      );
      _expectNoConfigWrites(harness);
    },
  );

  testWidgets(
    'a queue scan renders the hits, claimed conflicts, deduced and evidence',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.scan_queue_page') {
            host.lifecycle.add('scan-sent');
            return _queueScanAnswer(
              page: 2,
              perEntry: [
                {
                  'index': 0,
                  'label': 'red',
                  'hits': [
                    {'cell': 13, 'score': 0.02, 'runner_up': 0.31},
                  ],
                },
                {'index': 1, 'label': 'blue', 'hits': <Map<String, dynamic>>[]},
              ],
              claimed: [
                {
                  'cell': 13,
                  'labels': ['red', 'blue'],
                },
              ],
              deduced: ['red ×1', 'blue ×0'],
              evidence: [
                'logs/reads/scan_page2_2.png',
                'logs/reads/scan_2.txt',
              ],
            );
          }
          return _petReply(request);
        },
      );

      await _tap(tester, 'pet-scan-page-1');
      await _tap(tester, 'pet-scan-queue-page');
      await _flush(tester);

      expect(host.lifecycle, ['hide', 'scan-sent', 'show']);
      expect(harness.lastSentWithCmd('pet.scan_queue_page')!['args'], {
        'page': 2,
      });
      expect(
        _text(tester, 'pet-scan-queue-page-echo'),
        'Scanned ITEM2 — the page this command was requested for.',
      );
      expect(
        _text(tester, 'pet-scan-queue-entries'),
        '#1 red — cell 13 · score 0.020 · runner-up 0.310\n'
        '#2 blue — no hit reported on this page',
      );
      expect(
        find.byKey(const ValueKey('pet-scan-queue-caption')),
        findsOneWidget,
        reason: 'an empty hit list is not read as "this pet is not in the bag"',
      );
      expect(
        _text(tester, 'pet-scan-queue-claimed'),
        'cell 13 claimed by red, blue',
      );
      expect(_text(tester, 'pet-scan-queue-deduced'), 'red ×1\nblue ×0');
      expect(
        _text(tester, 'pet-scan-queue-evidence'),
        contains('logs/reads/scan_page2_2.png'),
      );
      expect(find.byKey(const ValueKey('dirty-pet')), findsNothing);
      _expectNoConfigWrites(harness);
    },
  );

  testWidgets(
    'a refused scan renders the core line verbatim and shows the shell again',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.scan_food_page') {
            host.lifecycle.add('scan-sent');
            return {
              'error': {
                'code': 'tool_busy',
                'message':
                    'Magic Tuner holds the game — stop it before scanning',
              },
            };
          }
          return _petReply(request);
        },
      );

      await _tap(tester, 'pet-scan-food-page');
      await _flush(tester);

      expect(host.lifecycle, [
        'hide',
        'scan-sent',
        'show',
      ], reason: 'a refused scan must never leave the shell hidden');
      expect(
        _text(tester, 'pet-scan-food-error'),
        'Magic Tuner holds the game — stop it before scanning',
        reason: 'the core refusal is rendered verbatim',
      );
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-scan-food-page')),
            )
            .onPressed,
        isNotNull,
        reason: 'the scan button comes back after the refusal',
      );
      _expectNoConfigWrites(harness);
    },
  );

  testWidgets(
    'the panel test read sends the chosen page/cell and renders the core answer verbatim',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.test_read_panel') {
            host.lifecycle.add('panel-sent');
            return _panelReadAnswer();
          }
          return _petReply(request);
        },
      );

      await _tap(tester, 'pet-scan-page-1');
      await tester.enterText(_field('pet-test-panel-cell'), '34');
      await _tap(tester, 'pet-test-panel-read');
      await _flush(tester);

      expect(host.lifecycle, ['hide', 'panel-sent', 'show']);
      expect(harness.lastSentWithCmd('pet.test_read_panel')!['args'], {
        'page': 2,
        'cell': 34,
      });
      expect(
        _text(tester, 'pet-test-panel-region'),
        'Read region: x 33 · y 88 · 120×80',
      );
      expect(
        _text(tester, 'pet-test-panel-lines'),
        '(6階) 真蔚藍鳳凰 +7 [52.18%]',
        reason: 'the raw OCR lines are shown untouched',
      );
      expect(
        _text(tester, 'pet-test-panel-parsed'),
        'Parsed: stage 6, +7, 52.18%',
        reason: 'the core parser\'s own line, never re-derived by the shell',
      );
      expect(
        _text(tester, 'pet-test-panel-numbers'),
        'Numbers seen: 6, 7, 52.18',
      );
      expect(
        _text(tester, 'pet-test-panel-evidence'),
        contains('logs/reads/petpanel_2026.png'),
      );
      expect(find.byKey(const ValueKey('dirty-pet')), findsNothing);
      _expectNoConfigWrites(harness);
    },
  );

  testWidgets(
    'a partial panel says so and still shows the lines/numbers the OCR read',
    (tester) async {
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        answer: (request) {
          if (request['cmd'] == 'pet.test_read_panel') {
            return {
              'region': {'x': 33, 'y': 88, 'width': 120, 'height': 80},
              'lines': ['(6階) 真蔚藍鳳凰 +7'],
              'panel': null,
              'numbers': ['6', '7'],
              'evidence': null,
            };
          }
          return _petReply(request);
        },
      );

      await _tap(tester, 'pet-test-panel-read');
      await _flush(tester);

      expect(
        _text(tester, 'pet-test-panel-parsed'),
        'Not a pet panel — no growth + EXP pair in it.',
      );
      expect(_text(tester, 'pet-test-panel-numbers'), 'Numbers seen: 6, 7');
      _expectNoConfigWrites(harness);
    },
  );

  testWidgets(
    'a refused panel read names the unset tooltip field and shows the shell again',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.test_read_panel') {
            host.lifecycle.add('panel-sent');
            return {
              'error': {
                'code': 'bad_request',
                'message':
                    "The tooltip/panel region isn't calibrated — save the tooltip "
                    'offset and size.',
                'detail': {'field': 'tooltip'},
              },
            };
          }
          return _petReply(request);
        },
      );

      await _tap(tester, 'pet-test-panel-read');
      await _flush(tester);

      expect(host.lifecycle, ['hide', 'panel-sent', 'show']);
      expect(
        _text(tester, 'pet-test-panel-error'),
        "The tooltip/panel region isn't calibrated — save the tooltip offset and size.",
        reason: 'the core refusal is rendered verbatim',
      );
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-test-panel-read')),
            )
            .onPressed,
        isNotNull,
      );
      _expectNoConfigWrites(harness);
    },
  );

  testWidgets(
    'applying a food scan asks, sends the 1-based pairs and refreshes the list from the mirror',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.scan_food_page') {
            host.lifecycle.add('scan-sent');
            return _foodScanAnswer(
              page: 2,
              cells: [13, 36],
              report: ['page 2: 2 cell(s) — 13, 36'],
            );
          }
          if (request['cmd'] == 'pet.apply_food_scan') {
            host.lifecycle.add('apply-sent');
            return {'rev': 21, 'block': <String, dynamic>{}};
          }
          return _petReply(request);
        },
      );
      await _tap(tester, 'pet-scan-page-1');
      await _tap(tester, 'pet-scan-food-page');
      await _flush(tester);
      expect(
        _preflightCalls(harness),
        1,
        reason: 'the open read is the only checklist read so far',
      );

      // The confirmation is required, and Cancel sends nothing.
      await _tap(tester, 'pet-scan-apply-food');
      await tester.pumpAndSettle();
      expect(find.textContaining('REPLACES'), findsOneWidget);
      expect(find.textContaining('RESETS'), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.textContaining('immediately'),
        ),
        findsOneWidget,
        reason:
            'the write takes effect at once, even during a run (the tab also carries the '
            '"applies immediately" cue per row, hence the dialog scope)',
      );
      expect(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.textContaining('ITEM2'),
        ),
        findsOneWidget,
        reason: 'the dialog names the page the scan was requested for',
      );
      await tester.tap(find.byKey(const ValueKey('pet-scan-apply-cancel')));
      await tester.pumpAndSettle();
      expect(harness.lastSentWithCmd('pet.apply_food_scan'), isNull);
      expect(host.lifecycle, ['hide', 'scan-sent', 'show']);

      await _tap(tester, 'pet-scan-apply-food');
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('pet-scan-apply-confirm')));
      await tester.pumpAndSettle();
      await _flush(tester);

      expect(harness.lastSentWithCmd('pet.apply_food_scan')!['args'], {
        'cells': [
          [2, 13],
          [2, 36],
        ],
      }, reason: '1-BASED page/cell pairs, taken from the scan result');
      expect(host.lifecycle, ['hide', 'scan-sent', 'show', 'apply-sent']);
      expect(
        _text(tester, 'pet-scan-apply-result'),
        contains('Applied 2 cells (config rev 21)'),
      );
      expect(
        _preflightCalls(harness),
        2,
        reason: 'a successful A1 write changed the saved config: exactly one checklist re-read',
      );
      _expectNoConfigWrites(harness);

      // The A1 write's one config.changed re-seeds the clean draft; the food list view follows it.
      final pet = Map<String, dynamic>.of(_machinePet());
      pet['food_slots'] = [
        [1, 12],
        [1, 35],
      ];
      pet['food_slots_used'] = 0;
      harness.emitConfigChanged(rev: 22, block: 'pet', values: pet);
      await _flush(tester);
      expect(
        _text(tester, 'pet-food-count'),
        '0 of 64 cells marked on this page · 2 marked overall',
        reason: 'the food-cells view refreshes from the mirror',
      );
    },
  );

  testWidgets('the apply button is disabled while the draft is dirty', (
    tester,
  ) async {
    final host = FakeHost();
    await _pumpPet(
      tester,
      pet: _machinePet(),
      host: host,
      answer: (request) {
        if (request['cmd'] == 'pet.scan_food_page') {
          host.lifecycle.add('scan-sent');
          return _foodScanAnswer(page: 1, cells: [13, 36]);
        }
        return _petReply(request);
      },
    );
    await _tap(tester, 'pet-scan-food-page');
    await _flush(tester);

    await tester.enterText(_field('pet-row-0-stacks'), '3');
    await tester.pump();

    expect(
      tester
          .widget<OutlinedButton>(
            find.byKey(const ValueKey('pet-scan-apply-food')),
          )
          .onPressed,
      isNull,
      reason: 'an A1 write beside a dirty draft would banner the player\'s own action',
    );
    expect(_text(tester, 'pet-scan-apply-dirty'), 'Save the edits first');
  });

  testWidgets('an empty food scan leaves Apply disabled', (tester) async {
    final host = FakeHost();
    await _pumpPet(
      tester,
      pet: _machinePet(),
      host: host,
      answer: (request) {
        if (request['cmd'] == 'pet.scan_food_page') {
          host.lifecycle.add('scan-sent');
          return _foodScanAnswer(page: 1, cells: []);
        }
        return _petReply(request);
      },
    );
    await _tap(tester, 'pet-scan-food-page');
    await _flush(tester);

    expect(_text(tester, 'pet-scan-food-cells'), 'Found no food on this page.');
    final apply = tester.widget<OutlinedButton>(
      find.byKey(const ValueKey('pet-scan-apply-food')),
    );
    expect(apply.onPressed, isNull);
    expect(find.text('Apply 0 cells to the food list'), findsOneWidget);
  });

  testWidgets(
    'a refused apply renders the core line verbatim and writes nothing',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.scan_food_page') {
            host.lifecycle.add('scan-sent');
            return _foodScanAnswer(page: 1, cells: [13, 36]);
          }
          if (request['cmd'] == 'pet.apply_food_scan') {
            host.lifecycle.add('apply-sent');
            return {
              'error': {
                'code': 'bad_request',
                'message': 'pet.apply_food_scan cells[1] is outside 1..64',
              },
            };
          }
          return _petReply(request);
        },
      );
      await _tap(tester, 'pet-scan-food-page');
      await _flush(tester);
      expect(
        _preflightCalls(harness),
        1,
        reason: 'the open read is the only checklist read so far',
      );
      await _tap(tester, 'pet-scan-apply-food');
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('pet-scan-apply-confirm')));
      await tester.pumpAndSettle();
      await _flush(tester);

      expect(
        _text(tester, 'pet-scan-apply-result'),
        'pet.apply_food_scan cells[1] is outside 1..64',
        reason: 'the core refusal is rendered verbatim',
      );
      expect(
        _preflightCalls(harness),
        1,
        reason:
            'a refused apply changes nothing, so there is no checklist re-read',
      );
      _expectNoConfigWrites(harness);
      expect(
        _text(tester, 'pet-food-count'),
        '0 of 64 cells marked on this page · 2 marked overall',
        reason: 'a refused apply leaves the food list untouched',
      );
    },
  );

  testWidgets(
    'a missing food icon is captioned but the scan button still sends',
    (tester) async {
      final machine = Map<String, dynamic>.of(_machinePet())
        ..remove('food_icon_png');
      await _pumpPet(tester, pet: machine);

      expect(
        find.byKey(const ValueKey('pet-scan-no-food-icon')),
        findsOneWidget,
      );
      expect(find.textContaining('Capture food icon'), findsWidgets);
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-scan-food-page')),
            )
            .onPressed,
        isNotNull,
        reason: 'the core owns the refusal; the shell only points at the missing icon',
      );
    },
  );

  testWidgets(
    'a food scan with a missing hide channel is noted, sent and rendered',
    (tester) async {
      final host = FakeHost(failHide: true);
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.scan_food_page') {
            host.lifecycle.add('scan-sent');
            return _foodScanAnswer(page: 1, cells: [13, 36]);
          }
          return _petReply(request);
        },
      );

      await _tap(tester, 'pet-scan-food-page');
      await _flush(tester);

      expect(host.lifecycle, [
        'hide',
        'scan-sent',
        'show',
      ], reason: 'a failed hide is a note, not a reason to skip the scan');
      expect(
        _text(tester, 'pet-scan-food-note'),
        'the shell could not hide itself — the game has to be in front for a capture',
      );
      expect(
        harness.sentMessages.where(
          (message) => message['cmd'] == 'pet.scan_food_page',
        ),
        hasLength(1),
        reason: 'the command is still sent through a missing host channel',
      );
      expect(
        _text(tester, 'pet-scan-food-cells'),
        'Found 2 cells: 13, 36',
        reason: 'the scan result path is unchanged',
      );
      _expectNoConfigWrites(harness);
    },
  );

  testWidgets(
    'a food scan with a missing show channel is noted and still renders',
    (tester) async {
      final host = FakeHost(failShow: true);
      final harness = await _pumpPet(
        tester,
        pet: _machinePet(),
        host: host,
        answer: (request) {
          if (request['cmd'] == 'pet.scan_food_page') {
            host.lifecycle.add('scan-sent');
            return _foodScanAnswer(page: 1, cells: [13, 36]);
          }
          return _petReply(request);
        },
      );

      await _tap(tester, 'pet-scan-food-page');
      await _flush(tester);

      expect(host.lifecycle, ['hide', 'scan-sent', 'show']);
      expect(
        host.showCalls,
        1,
        reason: 'the show is attempted and does not throw',
      );
      expect(
        _text(tester, 'pet-scan-food-note'),
        'the shell could not be shown again — no host channel',
      );
      expect(_text(tester, 'pet-scan-food-cells'), 'Found 2 cells: 13, 36');
      _expectNoConfigWrites(harness);
    },
  );

  testWidgets(
    'only one window command can be in flight across captures and scans',
    (tester) async {
      final host = FakeHost();
      final harness = await _pumpPet(tester, pet: _machinePet(), host: host);
      // A source cell so the capture button would be ENABLED when the window is free.
      await _tap(tester, 'pet-add-cell-0-7');
      // Withhold the next window command's reply so it stays in flight.
      harness.onWrite = (line, fake) {};

      // 1) A SCAN in flight disables both capture buttons and starts no capture.
      await _tap(tester, 'pet-scan-food-page');
      await tester.pump();
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-add-capture')),
            )
            .onPressed,
        isNull,
        reason: 'a scan holds the window, so a capture cannot start',
      );
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-food-icon-capture')),
            )
            .onPressed,
        isNull,
      );
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-scan-food-page')),
            )
            .onPressed,
        isNull,
        reason: 'the scan buttons are single-flight too',
      );
      await tester.tap(
        find.byKey(const ValueKey('pet-add-capture')),
        warnIfMissed: false,
      );
      await tester.pump();
      expect(
        harness.sentMessages.where(
          (message) => message['cmd'] == 'pet.capture_icon',
        ),
        isEmpty,
        reason:
            'a second window command is not sent while the scan is in flight',
      );
      expect(
        harness.sentMessages.where(
          (message) => message['cmd'] == 'pet.scan_food_page',
        ),
        hasLength(1),
      );

      // Answer the scan; the window is free again.
      harness.respondOk(_foodScanAnswer(page: 1));
      await _flush(tester);

      // 2) A CAPTURE in flight disables both scan buttons and starts no scan.
      await _tap(tester, 'pet-add-capture');
      await tester.pump();
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-scan-food-page')),
            )
            .onPressed,
        isNull,
        reason: 'a capture holds the window, so a scan cannot start',
      );
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-scan-queue-page')),
            )
            .onPressed,
        isNull,
      );
      await tester.tap(
        find.byKey(const ValueKey('pet-scan-queue-page')),
        warnIfMissed: false,
      );
      await tester.pump();
      expect(
        harness.sentMessages.where(
          (message) => message['cmd'] == 'pet.capture_icon',
        ),
        hasLength(1),
        reason: 'exactly one capture is in flight',
      );
      expect(
        harness.sentMessages.where(
          (message) => message['cmd'] == 'pet.scan_queue_page',
        ),
        isEmpty,
        reason: 'a second window command is not sent while the capture is in flight',
      );

      // Answer the capture so the test leaves nothing pending; the window is free again.
      harness.respondOk(_captureAnswer(harness.lastRequest));
      await _flush(tester);
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-scan-food-page')),
            )
            .onPressed,
        isNotNull,
      );
      expect(
        tester
            .widget<OutlinedButton>(
              find.byKey(const ValueKey('pet-add-capture')),
            )
            .onPressed,
        isNotNull,
      );
      _expectNoConfigWrites(harness);
    },
  );
}

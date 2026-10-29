/// The Hotkeys tab: names from `keys.names`, edits parsed by the domain through `keys.parse`, Save
/// as the portable `settings: {hotkeys: {codes}}` overlay, unnameable-code preservation, and the
/// shared external-change guard.
library;

import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/app.dart';
import 'package:seal_shell/ui/tabs/hotkeys_tab.dart';
import 'package:seal_shell/ui/theme.dart';

import '../../support/app_harness.dart';

const String _badLine =
    '"Zulu" is not a key. Use F1–24, Esc, CapsLock, Space, Tab, Enter, or a single letter/digit.';

Map<String, dynamic> _config({
  int start = 123,
  int quit = 122,
  int advance = 71,
  int pause = 20,
}) => {
  'window': {'title': 'TW_LIVE'},
  'hotkeys': {
    'start': start,
    'quit': quit,
    'advance_grade': advance,
    'pause': pause,
  },
};

int? _codeFor(String name) => switch (name) {
  'F12' => 123,
  'F11' => 122,
  'F10' => 121,
  'Escape' => 27,
  'G' || 'g' => 71,
  'CapsLock' => 20,
  _ => null,
};

String _displayName(int code) => switch (code) {
  123 => 'F12',
  122 => 'F11',
  121 => 'F10',
  27 => 'Escape',
  71 => 'G',
  20 => 'CapsLock',
  5 => '0x5',
  _ => '0x${code.toRadixString(16)}',
};

Future<AppHarness> _pumpHotkeys(
  WidgetTester tester, {
  Map<String, dynamic>? config,
}) async {
  tester.view.physicalSize = const ui.Size(1600, 2400);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.reset);
  final harness = AppHarness();
  addTearDown(harness.model.dispose);
  await tester.pumpWidget(
    const MaterialApp(
      home: Scaffold(body: Center(child: CircularProgressIndicator())),
    ),
  );
  final connecting = harness.connect(config: config ?? _config());
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  await connecting;
  harness.onWrite = (line, fake) {
    final message = fake.sentJson(fake.sent.length - 1);
    final id = message['id'] as String;
    final cmd = message['cmd'] as String;
    final args = asJsonMap(message['args']);
    switch (cmd) {
      case 'keys.names':
        final names = <String, String>{};
        for (final entry in asJsonMap(args['codes']).entries) {
          final code = asNumber(entry.value);
          if (code != null) {
            names[entry.key] = _displayName(code.round());
          }
        }
        fake.emitJson(okResponse(id, {'names': names}));
      case 'keys.parse':
        final codes = <String, int>{};
        final errors = <Map<String, dynamic>>[];
        for (final entry in asJsonMap(args['names']).entries) {
          final input = asString(entry.value) ?? '';
          final code = _codeFor(input);
          if (code == null) {
            errors.add({'field': entry.key, 'input': input, 'line': _badLine});
          } else {
            codes[entry.key] = code;
          }
        }
        fake.emitJson(okResponse(id, {'codes': codes, 'errors': errors}));
      case 'config.validate':
        fake.emitJson(
          okResponse(id, {'errors': <String>[], 'warnings': <String>[]}),
        );
      case 'config.apply':
        fake.emitJson(
          okResponse(id, {'rev': 8, 'sections': asJsonMap(args['sections'])}),
        );
      default:
        fake.emitJson(okResponse(id, {}));
    }
  };
  await tester.pumpWidget(
    MaterialApp(
      theme: buildShellTheme(),
      home: Scaffold(body: HotkeysTab(model: harness.model)),
    ),
  );
  for (var i = 0; i < 3; i++) {
    await tester.pump();
  }
  return harness;
}

Finder _field(String key) => find.byKey(ValueKey(key));

Future<void> _flush(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets('the stored codes render as the core names', (tester) async {
    await _pumpHotkeys(tester);

    expect(
      tester.widget<TextField>(_field('hotkey-start')).controller?.text,
      'F12',
    );
    expect(
      tester.widget<TextField>(_field('hotkey-quit')).controller?.text,
      'F11',
    );
    expect(
      tester.widget<TextField>(_field('hotkey-advance_grade')).controller?.text,
      'G',
    );
    expect(
      tester.widget<TextField>(_field('hotkey-pause')).controller?.text,
      'CapsLock',
    );
    // The raw stored codes are debug data now: they live in the section-header tooltip, not on the
    // frame, where the names are the point of the tab.
    final codesHelp = tester.widget<Tooltip>(
      find.byKey(const ValueKey('hotkey-stored-codes')),
    );
    expect(codesHelp.message, contains('start=123'));
  });

  testWidgets('a bad name renders the domain line verbatim and blocks apply', (
    tester,
  ) async {
    final harness = await _pumpHotkeys(tester);

    // A valid edit first, so the draft (and Save) is enabled.
    await tester.enterText(_field('hotkey-start'), 'F11');
    await _flush(tester);
    expect(find.byKey(const ValueKey('dirty-hotkeys')), findsOneWidget);

    await tester.enterText(_field('hotkey-pause'), 'Zulu');
    await _flush(tester);
    expect(find.byKey(const ValueKey('hotkey-error-pause')), findsOneWidget);
    expect(find.text(_badLine), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('save-hotkeys')));
    await _flush(tester);

    expect(harness.lastSentWithCmd('config.validate'), isNull);
    expect(harness.lastSentWithCmd('config.apply'), isNull);
    // The edited name that parsed is still in the draft; the bad box keeps its line.
    expect(find.byKey(const ValueKey('dirty-hotkeys')), findsOneWidget);
  });

  testWidgets('Save sends the portable settings.hotkeys codes exactly', (
    tester,
  ) async {
    final harness = await _pumpHotkeys(tester);

    await tester.enterText(_field('hotkey-start'), 'F11');
    await _flush(tester);
    await tester.tap(find.byKey(const ValueKey('save-hotkeys')));
    await _flush(tester);

    final apply = harness.lastSentWithCmd('config.apply');
    expect(apply, isNotNull);
    final sections =
        (apply!['args'] as Map<String, dynamic>)['sections']
            as Map<String, dynamic>;
    expect(sections.keys, ['settings'], reason: 'hotkeys is portable');
    expect(sections['settings'], {
      'hotkeys': {'start': 122, 'quit': 122, 'advance_grade': 71, 'pause': 20},
    });
  });

  testWidgets('an untouched unnameable code survives a save of another box', (
    tester,
  ) async {
    final harness = await _pumpHotkeys(tester, config: _config(pause: 5));

    // v2's fallback name for a code no name covers; it must display, not be refused.
    expect(
      tester.widget<TextField>(_field('hotkey-pause')).controller?.text,
      '0x5',
    );

    await tester.enterText(_field('hotkey-start'), 'F11');
    await _flush(tester);
    await tester.tap(find.byKey(const ValueKey('save-hotkeys')));
    await _flush(tester);

    final apply = harness.lastSentWithCmd('config.apply')!;
    final sections =
        (apply['args'] as Map<String, dynamic>)['sections']
            as Map<String, dynamic>;
    final hotkeys =
        (sections['settings'] as Map<String, dynamic>)['hotkeys']
            as Map<String, dynamic>;
    expect(
      hotkeys['pause'],
      5,
      reason: 'the untouched code is never re-parsed and is sent back verbatim',
    );
    expect(hotkeys['start'], 122);
  });

  testWidgets('a dirty hotkeys tab marks the external-change banner', (
    tester,
  ) async {
    final harness = await _pumpHotkeys(tester);

    await tester.enterText(_field('hotkey-start'), 'F11');
    await _flush(tester);
    expect(find.byKey(const ValueKey('dirty-hotkeys')), findsOneWidget);

    harness.emitConfigChanged(
      rev: 9,
      block: 'hotkeys',
      values: {'start': 999, 'quit': 122, 'advance_grade': 71, 'pause': 20},
    );
    await _flush(tester);

    expect(
      find.byKey(const ValueKey('external-change-hotkeys')),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('dirty-hotkeys')), findsOneWidget);
    // The player's edit is still in the field until an explicit reconcile.
    expect(
      tester.widget<TextField>(_field('hotkey-start')).controller?.text,
      'F11',
    );
  });
}

/// The Setup tab: one save across the blocks Setup owns — portable `window.title`, the game variant
/// switch (`settings.game.variant`, resolved/validated/dictionary-loaded by the core), the portable
/// Arduino ids (`settings.arduino.vid/pid/baud`) and the machine `arduino.port` — plus the three
/// read-only actions the core serves: Find window, Measure, Probe board.
///
/// No rule lives here: the variant's order (resolve → validate → LOAD the dictionary → write) and its
/// refusals are the core's, and every action result/refusal is rendered verbatim ([PROTOCOL.md]
/// §4/§7).
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../protocol/protocol.dart';
import '../theme.dart';
import 'action_result_line.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

class SetupTab extends StatefulWidget {
  const SetupTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<SetupTab> createState() => _SetupTabState();
}

class _SetupTabState extends State<SetupTab>
    with ConfigTabState<SetupTab>, AutomaticKeepAliveClientMixin<SetupTab> {
  @override
  ShellModel get model => widget.model;

  @override
  bool get wantKeepAlive => true;

  late final ConfigDraft _windowDraft = ConfigDraft(
    blockName: 'window',
    scope: ConfigScope.portableSection,
    editableKeys: const {'title'},
  );
  late final ConfigDraft _gameDraft = ConfigDraft(
    blockName: 'game',
    scope: ConfigScope.portableSection,
    editableKeys: const {'variant'},
  );
  late final ConfigDraft _arduinoSettingsDraft = ConfigDraft(
    blockName: 'arduino',
    scope: ConfigScope.portableSection,
    editableKeys: const {'vid', 'pid', 'baud'},
  );
  late final ConfigDraft _arduinoPortDraft = ConfigDraft(
    blockName: 'arduino',
    scope: ConfigScope.ownedBlock,
    editableKeys: const {'port'},
  );
  late final ConfigDraftGroup _group = ConfigDraftGroup(
    label: 'setup',
    parts: [_windowDraft, _gameDraft, _arduinoSettingsDraft, _arduinoPortDraft],
  );

  @override
  ConfigDraft get draft => _windowDraft;

  @override
  ConfigDraftGroup get draftGroup => _group;

  // Actions are read-only: results are state for this tab, never config writes.
  CoreActionResult<WindowFindResult>? _find;
  CoreActionResult<DisplayInfo>? _measure;
  CoreActionResult<BoardProbeResult>? _probe;

  @override
  void dispose() {
    _group.dispose();
    super.dispose();
  }

  // ── readers ──────────────────────────────────────────────────────────────────────────────

  Map<String, dynamic> _merged(String block) =>
      asJsonMap(model.config.config[block]);

  String get _configuredTitle {
    final title =
        asString(_windowDraft.value['title']) ??
        asString(_merged('window')['title']);
    return title == null || title.isEmpty ? '— (no title configured)' : title;
  }

  String get _activeVariant =>
      asString(_gameDraft.value['variant']) ??
      asString(_merged('game')['variant']) ??
      '—';

  List<String> _variantNames() {
    final variants = asJsonMap(_merged('game')['variants']);
    return variants.keys.toList()..sort();
  }

  String get _dictionaryName {
    final variants = asJsonMap(_merged('game')['variants']);
    final dictionary = asString(
      asJsonMap(variants[_activeVariant])['attributes'],
    );
    return dictionary ?? '—';
  }

  String get _pidText =>
      asJsonList(_arduinoSettingsDraft.value['pid'])
          .whereType<num>()
          .map((value) => value.round().toString())
          .join(', ');

  // ── writers ──────────────────────────────────────────────────────────────────────────────

  void _set(ConfigDraft draft, String key, Object? value) => draft.edit((map) {
    if (value == null) {
      map.remove(key);
    } else {
      map[key] = value;
    }
  });

  void _setPid(String text) {
    final ids = <int>[
      for (final part in text.split(',')) ?int.tryParse(part.trim()),
    ];
    _set(_arduinoSettingsDraft, 'pid', ids.isEmpty ? null : ids);
  }

  Future<void> _runFind() async {
    final result = await model.findWindow();
    if (!mounted) {
      return;
    }
    setState(() => _find = result);
  }

  Future<void> _runMeasure() async {
    final result = await model.measureWindow();
    if (!mounted) {
      return;
    }
    setState(() => _measure = result);
  }

  Future<void> _runProbe() async {
    final result = await model.probeBoard();
    if (!mounted) {
      return;
    }
    setState(() => _probe = result);
  }

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    return buildConfigTab(
      title: 'Setup',
      description:
          'The window title, the client variant and the board settings. The variant switch is '
          'resolved, validated and its dictionary loaded by the core before anything is written.',
      readOnly: [
        EditorSection(
          title: 'Configured now — read-only',
          children: [
            ReadOnlyValue(label: 'Configured title', value: _configuredTitle),
            ReadOnlyValue(
              label: 'Active variant',
              value: '$_activeVariant · dictionary $_dictionaryName',
            ),
          ],
        ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EditorSection(
            title: 'Window and client',
            children: [
              DraftFieldSlot(
                slotKey: 'setup-window-title',
                syncToken: _windowDraft.seedRevision,
                child: DraftTextField(
                  label: 'Game window title',
                  value: asString(_windowDraft.value['title']) ?? '',
                  width: 320,
                  onChanged: (value) => _set(
                    _windowDraft,
                    'title',
                    value.trim().isEmpty ? null : value,
                  ),
                ),
              ),
              const SizedBox(height: ShellSpace.md),
              DraftDropdownField(
                key: const ValueKey('setup-variant'),
                label: 'Client variant',
                value: asString(_gameDraft.value['variant']),
                options: _variantNames(),
                width: 260,
                onChanged: (value) => _set(_gameDraft, 'variant', value),
              ),
            ],
          ),
          EditorSection(
            title: 'Arduino',
            children: [
              Wrap(
                spacing: ShellSpace.md,
                runSpacing: ShellSpace.sm,
                children: [
                  DraftFieldSlot(
                    slotKey: 'setup-arduino-vid',
                    syncToken: _arduinoSettingsDraft.seedRevision,
                    child: DraftNumberField(
                      label: 'VID (decimal)',
                      integer: true,
                      value: asNumber(_arduinoSettingsDraft.value['vid']),
                      width: 160,
                      onChanged: (value) =>
                          _set(_arduinoSettingsDraft, 'vid', value),
                    ),
                  ),
                  DraftFieldSlot(
                    slotKey: 'setup-arduino-baud',
                    syncToken: _arduinoSettingsDraft.seedRevision,
                    child: DraftNumberField(
                      label: 'Baud',
                      integer: true,
                      value: asNumber(_arduinoSettingsDraft.value['baud']),
                      width: 150,
                      onChanged: (value) =>
                          _set(_arduinoSettingsDraft, 'baud', value),
                    ),
                  ),
                  DraftFieldSlot(
                    slotKey: 'setup-arduino-pid',
                    syncToken: _arduinoSettingsDraft.seedRevision,
                    child: DraftTextField(
                      label: 'PID list (comma separated)',
                      value: _pidText,
                      width: 260,
                      onChanged: _setPid,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.sm),
              DraftFieldSlot(
                slotKey: 'setup-arduino-port',
                syncToken: _arduinoPortDraft.seedRevision,
                child: DraftTextField(
                  label: 'COM port override (empty = find by IDs)',
                  value: asString(_arduinoPortDraft.value['port']) ?? '',
                  width: 320,
                  onChanged: (value) => _set(
                    _arduinoPortDraft,
                    'port',
                    value.trim().isEmpty ? '' : value,
                  ),
                ),
              ),
            ],
          ),
          EditorSection(
            title: 'Actions — read-only, served by the core',
            children: [
              Wrap(
                spacing: ShellSpace.sm,
                runSpacing: ShellSpace.sm,
                children: [
                  OutlinedButton(
                    key: const ValueKey('setup-find-window'),
                    onPressed: model.isReady ? _runFind : null,
                    child: const Text('Find window'),
                  ),
                  OutlinedButton(
                    key: const ValueKey('setup-measure-window'),
                    onPressed: model.isReady ? _runMeasure : null,
                    child: const Text('Measure'),
                  ),
                  OutlinedButton(
                    key: const ValueKey('setup-probe-board'),
                    onPressed: model.isReady ? _runProbe : null,
                    child: const Text('Probe board'),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.sm),
              ActionResultLine(
                key: const ValueKey('setup-find-result'),
                label: 'Find',
                line: _find == null
                    ? null
                    : _find!.ok
                    ? (_find!.value!.found
                          ? 'Window found: ${_find!.value!.title ?? '?'}'
                          : 'Window not found')
                    : _find!.errorLine,
                isError: _find != null && !_find!.ok,
              ),
              ActionResultLine(
                key: const ValueKey('setup-measure-result'),
                label: 'Measure',
                line: _measure == null ? null : _measureLine(_measure!),
                isError: _measure != null && !_measure!.ok,
              ),
              ActionResultLine(
                key: const ValueKey('setup-probe-result'),
                label: 'Board',
                line: _probe == null
                    ? null
                    : (_probe!.ok
                          ? probeSummary(_probe!.value!)
                          : _probe!.errorLine),
                isError: _probe != null && !_probe!.ok,
              ),
            ],
          ),
        ],
      ),
    );
  }

  String _measureLine(CoreActionResult<DisplayInfo> result) {
    if (!result.ok) {
      return result.errorLine ?? 'measure failed';
    }
    final display = result.value!;
    return displaySummary(display);
  }
}

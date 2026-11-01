/// The Setup tab: one save across the blocks Setup owns — the read-only portable `window.title`, the
/// game variant switch (`settings.game.variant`, resolved/validated/dictionary-loaded by the core),
/// the portable Arduino ids (`settings.arduino.vid/pid/baud`) and the machine `arduino.port` — plus
/// the three read-only actions the core serves (Find window, Measure, Probe board) and **Use detected
/// title**, which saves the detected title through the same draft/save path.
///
/// The player's live pass asked whether the game-window name should be editable and concluded not:
/// the title identifies the game window, so Setup displays the configured value read-only and lets
/// **Find window** detect it; **Use detected title** then saves the detected string through the same
/// draft/save path (a normal config save — never written without the player pressing it).
///
/// No other rule lives here: the variant's order (resolve → validate → LOAD the dictionary → write)
/// and its refusals are the core's, and every action result/refusal is rendered verbatim
/// ([PROTOCOL.md] §4/§7).
///
/// Presentation follows [PLAN-V3-UI-UX.md] §6 "Setup": the three probes render as outlined read-only
/// actions, **Use detected title** is the single filled action because it is the only one that writes
/// config, and the caption beside it says so. A detected title that differs from the configured one is
/// a warning banner, not an automatic write.
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../app/dock_placement.dart';
import '../../protocol/protocol.dart';
import '../components/info_row.dart';
import '../components/shell_prose.dart';
import '../components/status_banner.dart';
import '../components/status_chip.dart';
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
  late final ConfigDraftGroup _group = ConfigDraftGroup(
    label: 'setup',
    parts: [_windowDraft, _gameDraft],
  );

  @override
  ConfigDraft get draft => _windowDraft;

  @override
  ConfigDraftGroup get draftGroup => _group;

  // Actions: Find/Measure/Probe are read-only; Use detected title is the one normal config save.
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

  /// The configured title as stored (empty when nothing is configured), for the read-only row and
  /// the detected-vs-configured comparison.
  String get _configuredTitleValue =>
      asString(_merged('window')['title'])?.trim() ?? '';

  String get _configuredTitle => _configuredTitleValue.isEmpty
      ? '— (no title configured)'
      : _configuredTitleValue;

  /// The title the last `window.find` actually detected, or null when there is none to offer.
  String? get _detectedTitle {
    final find = _find;
    if (find == null || !find.ok || !find.value!.found) {
      return null;
    }
    final title = find.value!.title?.trim();
    return title == null || title.isEmpty ? null : title;
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
      asJsonList(_merged('arduino')['pid'])
          .whereType<num>()
          .map((value) => value.round().toString())
          .join(', ');

  String _arduinoValue(String key) {
    final value = _merged('arduino')[key];
    if (value is num) return value.round().toString();
    if (value is String && value.trim().isNotEmpty) return value;
    return '';
  }

  // ── writers ──────────────────────────────────────────────────────────────────────────────

  void _set(ConfigDraft draft, String key, Object? value) => draft.edit((map) {
    if (value == null) {
      map.remove(key);
    } else {
      map[key] = value;
    }
  });

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

  /// Saves the title the last Find detected through the normal draft/Save path. Only the player's
  /// press can do this; no config write happens without it.
  Future<void> _useDetectedTitle() async {
    final title = _detectedTitle;
    if (title == null || !model.isReady || saving) {
      return;
    }
    setState(() => _set(_windowDraft, 'title', title));
    await save();
  }

  // ── UI ───────────────────────────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final theme = Theme.of(context);
    final detectedTitle = _detectedTitle;
    // A mismatch is advisory: the player may still want to keep the configured title, so this can
    // never write anything itself (only Use detected title saves, on a press).
    final titleMismatch =
        detectedTitle != null && detectedTitle != _configuredTitleValue;
    return buildConfigTab(
      title: 'Setup',
      description:
          'The window title, the client variant and the board settings. The variant switch is '
          'resolved, validated and its dictionary loaded by the core before anything is written.',
      readOnly: [
        EditorSection(
          title: 'Configured now — read-only',
          children: [
            InfoRow(
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
              KeyedSubtree(
                key: const ValueKey('setup-window-title'),
                child: InfoRow(
                  label: 'Game window title',
                  value: _configuredTitle,
                ),
              ),
              const SizedBox(height: ShellSpace.xs),
              ShellProse(
                'The title identifies the game window, so it is read-only here: Find window '
                'detects it, then Use detected title saves it.',
                key: const ValueKey('setup-title-caption'),
              ),
              const SizedBox(height: ShellSpace.md),
              DraftDropdownField(
                key: const ValueKey('setup-variant'),
                label: 'Client variant',
                value: asString(_gameDraft.value['variant']),
                options: _variantNames(),
                width: ShellWidth.picker,
                onChanged: (value) => _set(_gameDraft, 'variant', value),
              ),
            ],
          ),
          EditorSection(
            title: 'Quick dock',
            children: [
              const ShellProse(
                'A compact launcher between runs: five quick starts, one chevron each for the '
                'per-run options, and the long-running tools left to the full window. Choose where '
                'it sits here or from the dock header. The choice is session-only in this build '
                '(persisting it needs a machine/config field).',
              ),
              const SizedBox(height: ShellSpace.sm),
              Wrap(
                spacing: ShellSpace.sm,
                runSpacing: ShellSpace.sm,
                children: [
                  for (final placement in DockPlacement.values)
                    ChoiceChip(
                      key: ValueKey('setup-dock-${placement.wire}'),
                      label: Text(placement.label),
                      selected: model.dockPlacement == placement,
                      onSelected: (_) =>
                          unawaited(model.setDockPlacement(placement)),
                    ),
                ],
              ),
              const SizedBox(height: ShellSpace.md),
              OutlinedButton.icon(
                key: const ValueKey('setup-open-dock'),
                onPressed: model.isReady
                    ? () => unawaited(model.enterDock())
                    : null,
                icon: const Icon(Icons.dashboard_customize_outlined, size: 16),
                label: const Text('Open quick dock'),
              ),
            ],
          ),
          EditorSection(
            title: 'Arduino',
            children: [
              const ShellProse(
                'Shipped board-matching values: automatic detection uses them when the COM '
                'port is empty. They are shown read-only — the shipped Pro Micro values are '
                'correct, and the Arduino tab reports what a Start would choose.',
              ),
              const SizedBox(height: ShellSpace.sm),
              InfoRow(
                key: const ValueKey('setup-arduino-vid'),
                label: 'VID (decimal)',
                value: _arduinoValue('vid'),
                mono: true,
              ),
              InfoRow(
                key: const ValueKey('setup-arduino-baud'),
                label: 'Baud',
                value: _arduinoValue('baud'),
                mono: true,
              ),
              InfoRow(
                key: const ValueKey('setup-arduino-pid'),
                label: 'PID list',
                value: _pidText.isEmpty ? '—' : _pidText,
                mono: true,
              ),
              InfoRow(
                key: const ValueKey('setup-arduino-port'),
                label: 'COM port override',
                value: _arduinoValue('port').isEmpty
                    ? 'automatic (by board IDs)'
                    : _arduinoValue('port'),
                mono: true,
              ),
            ],
          ),
          EditorSection(
            title: 'Probes and the one save',
            children: [
              const ShellProse(
                'Find window, Measure and Probe board only read — they write nothing. Use '
                'detected title is the only action here that applies a config save.',
              ),
              const SizedBox(height: ShellSpace.md),
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
              const SizedBox(height: ShellSpace.md),
              Row(
                children: [
                  FilledButton.icon(
                    key: const ValueKey('setup-use-detected-title'),
                    onPressed:
                        model.isReady && !saving && _detectedTitle != null
                        ? _useDetectedTitle
                        : null,
                    icon: const Icon(Icons.title, size: ShellSize.iconMd),
                    label: const Text('Use detected title'),
                  ),
                  const SizedBox(width: ShellSpace.md),
                  Flexible(
                    child: Text(
                      'Applies a config save — the three probes above do not.',
                      key: const ValueKey('setup-use-detected-caption'),
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ),
                ],
              ),
              if (titleMismatch)
                Padding(
                  padding: const EdgeInsets.only(top: ShellSpace.md),
                  child: StatusBanner(
                    key: const ValueKey('setup-title-mismatch'),
                    kind: StatusKind.warning,
                    title: 'The detected title differs from the configured one',
                    detail:
                        'Configured "$_configuredTitleValue" · detected "$detectedTitle". '
                        'Use detected title replaces the configured value with the detected '
                        'one, as a config save.',
                  ),
                ),
              const SizedBox(height: ShellSpace.md),
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

/// The Hotkeys tab: the player edits NAMES; the portable `settings.hotkeys` section stores four VK
/// CODES (the merged hello flattens that section to top-level `hotkeys`, but that is the READ view,
/// not the write path: `config.apply` writes portable sections under `settings:`).
///
/// The name↔code rule is the domain's, not this widget's:
/// - display: `keys.names` turns the stored codes into names (v2's hex fallback `"0x5"` included, so a
///   hand-edited config still displays);
/// - edit: each changed box is run through `keys.parse`, which answers the codes it could compute and
///   ONE line per bad box — those lines render verbatim under the field, and Save sends nothing while
///   any edited box is bad;
/// - Save: the tab sends `settings: {hotkeys: {codes}}` for `config.validate`/`apply`.
///
/// Only the fields the player EDITED are re-parsed. An untouched box keeps the code the config holds,
/// even when that code has no name (`0x5`): re-parsing a name the domain itself cannot produce would
/// turn "save another field" into a refusal for a config v2 accepted ([PROTOCOL.md] §12.5).
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';
import 'config_tab_scaffold.dart';
import 'fields.dart';

class HotkeysTab extends StatefulWidget {
  const HotkeysTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<HotkeysTab> createState() => _HotkeysTabState();
}

class _HotkeysTabState extends State<HotkeysTab>
    with ConfigTabState<HotkeysTab>, AutomaticKeepAliveClientMixin<HotkeysTab> {
  @override
  ShellModel get model => widget.model;

  @override
  bool get wantKeepAlive => true;

  @override
  final ConfigDraft draft = ConfigDraft(
    blockName: 'hotkeys',
    scope: ConfigScope.portableSection,
    editableKeys: hotkeyFields.toSet(),
  );

  final Map<String, TextEditingController> _controllers = {
    for (final field in hotkeyFields) field: TextEditingController(),
  };

  /// The domain's verbatim line for a bad box, keyed by field.
  final Map<String, String> _fieldErrors = {};

  /// Fields whose NAME the player edited; only these are re-parsed on Save.
  final Set<String> _edited = {};

  String? _namesError;
  bool _loadingNames = false;
  int? _seenRev;

  @override
  void initState() {
    super.initState();
    _seenRev = model.config.rev;
    model.config.addListener(_onConfigChanged);
    unawaited(_loadNames());
  }

  @override
  void dispose() {
    model.config.removeListener(_onConfigChanged);
    for (final controller in _controllers.values) {
      controller.dispose();
    }
    super.dispose();
  }

  /// A core-side change re-seeds the draft through the shared guard when it is clean; refresh the
  /// displayed names then. A dirty draft is left alone (the banner asks for an explicit reconcile).
  void _onConfigChanged() {
    final rev = model.config.rev;
    if (rev == _seenRev) {
      return;
    }
    _seenRev = rev;
    if (!draft.isDirty) {
      unawaited(_loadNames());
    }
  }

  /// The stored codes in the draft, as integers (the payload's real shape).
  Map<String, int> get _storedCodes => {
    for (final field in hotkeyFields)
      if (asNumber(draft.value[field]) case final code?) field: code.round(),
  };

  Future<void> _loadNames() async {
    final codes = _storedCodes;
    if (codes.isEmpty) {
      return;
    }
    setState(() => _loadingNames = true);
    final result = await model.keyNames(codes: codes);
    if (!mounted) {
      return;
    }
    setState(() {
      _loadingNames = false;
      if (result.ok) {
        _namesError = null;
        for (final field in hotkeyFields) {
          final name = result.value!.names[field];
          _controllers[field]!.text = name ?? '${codes[field] ?? ''}';
        }
      } else {
        _namesError = result.errorLine;
      }
    });
  }

  /// Parse ONE edited box through the domain rule; a parseable name writes its code into the draft
  /// (that is what makes Save dirty), a bad one records the domain's line for that box.
  Future<void> _onNameChanged(String field, String value) async {
    _edited.add(field);
    final result = await model.parseKeyNames(names: {field: value});
    if (!mounted) {
      return;
    }
    setState(() {
      if (!result.ok) {
        _fieldErrors[field] = result.errorLine ?? 'keys.parse failed';
        return;
      }
      final errors = result.value!.errors;
      if (errors.isNotEmpty) {
        // The core answers at most one error per field today; first-line is defensive only.
        _fieldErrors[field] = errors.first.line;
        return;
      }
      final code = result.value!.codes[field];
      if (code != null) {
        draft.edit((block) => block[field] = code);
        _fieldErrors.remove(field);
      }
    });
  }

  /// Save re-parses ONLY the edited boxes (an untouched unnameable code must survive), refuses while
  /// any edited box is bad, and otherwise sends the owned block through the shared frame.
  @override
  Future<void> save() async {
    if (_edited.isEmpty) {
      return;
    }
    final names = {
      for (final field in _edited) field: _controllers[field]!.text,
    };
    final parsed = await model.parseKeyNames(names: names);
    if (!mounted) {
      return;
    }
    if (!parsed.ok) {
      setState(() => _namesError = parsed.errorLine);
      return;
    }
    final errors = parsed.value!.errors;
    if (errors.isNotEmpty) {
      setState(() {
        _fieldErrors
          ..clear()
          ..addEntries(
            errors.map((error) => MapEntry(error.field, error.line)),
          );
      });
      return;
    }
    setState(() {
      _fieldErrors.clear();
      draft.edit((block) {
        for (final entry in parsed.value!.codes.entries) {
          block[entry.key] = entry.value;
        }
      });
    });
    await super.save();
    if (!mounted || !(outcome?.saved ?? false)) {
      return;
    }
    // Show the core's canonical spelling after a save (a typed 12 becomes F12).
    _edited.clear();
    await _loadNames();
  }

  @override
  void cancel() {
    super.cancel();
    _edited.clear();
    _fieldErrors.clear();
    unawaited(_loadNames());
  }

  @override
  Future<void> reconcile() async {
    await super.reconcile();
    if (!mounted) {
      return;
    }
    _edited.clear();
    _fieldErrors.clear();
    await _loadNames();
  }

  String get _storedCodesLine {
    final codes = _storedCodes;
    return codes.isEmpty
        ? '— (no codes stored)'
        : [for (final entry in codes.entries) '${entry.key}=${entry.value}']
              .join(' · ');
  }

  /// The raw VK codes are debug data: the names are what the tab edits, so the codes live in a
  /// tooltip on the section header instead of taking a line of the frame (§6 "Hotkeys").
  String get _storedCodesTooltip {
    final codes = _storedCodes;
    return 'Stored codes (debug): '
        '${codes.isEmpty ? '— (none stored)' : _storedCodesLine}';
  }

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final theme = Theme.of(context);
    return buildConfigTab(
      title: 'Hotkeys',
      description:
          'The keys that start, quit, advance the grade and pause. The config stores codes; the '
          'core translates the names you type and names the code it holds.',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          EditorSection(
            title: 'Keys',
            trailing: Tooltip(
              key: const ValueKey('hotkey-stored-codes'),
              message: _storedCodesTooltip,
              child: Icon(
                Icons.info_outline,
                size: ShellSize.iconMd,
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
            children: [
              // The accepted-name help appears ONCE, above the group: repeating it under all four
              // fields made the help outmass the inputs (§2.5.3).
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Icon(
                    Icons.keyboard_outlined,
                    size: ShellSize.iconMd,
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                  const SizedBox(width: ShellSpace.sm),
                  Expanded(
                    child: Text(
                      'Accepted names: F1–F24, Esc, CapsLock, Space, Tab, Enter, or a single '
                      'letter/digit. The core names what it stores; a hand-edited code shows as '
                      'its raw value.',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: ShellSpace.md),
              for (final field in hotkeyFields)
                Padding(
                  padding: const EdgeInsets.only(bottom: ShellSpace.sm),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      SizedBox(
                        width: ShellWidth.fieldLg,
                        child: TextField(
                          key: ValueKey('hotkey-$field'),
                          controller: _controllers[field],
                          decoration: InputDecoration(
                            labelText: hotkeyLabels[field] ?? field,
                            isDense: true,
                            border: const OutlineInputBorder(),
                          ),
                          onChanged: (value) => _onNameChanged(field, value),
                        ),
                      ),
                      if (_fieldErrors[field] != null)
                        Padding(
                          padding: const EdgeInsets.only(top: ShellSpace.xs),
                          child: Text(
                            _fieldErrors[field]!,
                            key: ValueKey('hotkey-error-$field'),
                            style: theme.textTheme.bodySmall?.copyWith(
                              color: theme.colorScheme.error,
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
              if (_loadingNames)
                Text(
                  'Reading the stored key names…',
                  style: theme.textTheme.bodySmall,
                ),
              if (_namesError != null)
                Text(
                  _namesError!,
                  key: const ValueKey('hotkey-names-error'),
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.error,
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

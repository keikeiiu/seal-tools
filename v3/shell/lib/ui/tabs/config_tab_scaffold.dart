/// The shared frame of a wave-1 tab: header, scrolling editor, Save/Cancel bar, and the CORE's
/// validation/refusal lines rendered verbatim.
///
/// The Save order is a protocol rule, not a widget choice: `config.validate` first, then
/// `config.apply` ([PLAN-V3-SHELL.md] §5), and it lives in `ShellModel.saveConfig`. The tab only
/// renders the outcome. A conflict is shown with the core's current rev and changed block names, and
/// the draft is re-seeded only when the player chooses to reconcile — never automatically.
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../../protocol/protocol.dart';
import '../components/page_header.dart';
import '../components/status_chip.dart';
import '../theme.dart';

/// State shared by every tab: the draft, the in-flight flag, the last outcome and the three actions.
mixin ConfigTabState<T extends StatefulWidget> on State<T> {
  ShellModel get model;
  ConfigDraft get draft;

  ConfigDraftGroup? _defaultDraftGroup;

  /// The drafts this tab saves. A single-section tab uses the one [draft]; Setup returns a group.
  ConfigDraftGroup get draftGroup => _defaultDraftGroup ??= ConfigDraftGroup(
    label: draft.blockName,
    parts: [draft],
  );

  ConfigSaveOutcome? outcome;
  bool saving = false;

  int? _seenConfigRev;

  /// Seeds the draft once from the current mirror, then watches the mirror for changes made while
  /// this tab is off-screen (tabs are kept alive): a dirty draft is MARKED and waits for an explicit
  /// reconcile, a clean draft re-seeds silently.
  @override
  void initState() {
    super.initState();
    draftGroup.seedFromMirror(model.config.config);
    _seenConfigRev = model.config.rev;
    model.config.addListener(_onConfigChanged);
  }

  @override
  void dispose() {
    model.config.removeListener(_onConfigChanged);
    _defaultDraftGroup?.dispose();
    super.dispose();
  }

  void _onConfigChanged() {
    final rev = model.config.rev;
    if (_seenConfigRev == rev) {
      return;
    }
    _seenConfigRev = rev;
    draftGroup.observeMirrorChange(model.config.config);
  }

  Future<void> save() async {
    if (saving || !draftGroup.isDirty) {
      return;
    }
    setState(() {
      saving = true;
      outcome = null;
    });
    final result = await model.saveConfig(draftGroup.payload());
    if (!mounted) {
      return;
    }
    setState(() {
      saving = false;
      outcome = result;
      final applied = result.result;
      if (result.saved && applied != null) {
        draftGroup.savedFrom(applied.sections);
      }
    });
  }

  void cancel() {
    setState(() {
      outcome = null;
      // Discarding edits after an external change shows the CORE's latest value, not the stale one.
      if (draftGroup.hasExternalChange) {
        draftGroup.seedFromMirror(model.config.config, force: true);
      } else {
        draftGroup.reset();
      }
    });
  }

  /// The explicit reconcile: re-read the files from the core, then accept them into the draft. The
  /// player has seen what changed before this runs (the conflict banner).
  Future<void> reconcile() async {
    final reloaded = await model.reloadConfig();
    if (!mounted) {
      return;
    }
    setState(() {
      outcome = null;
      draftGroup.seedFromMirror(model.config.config, force: reloaded != null);
    });
  }

  /// The title/description/scrolling body, rendered from the latest draft.
  Widget buildConfigTab({
    required String title,
    required String description,
    required Widget child,
    List<Widget> readOnly = const [],
    Widget? aside,
  }) => ListenableBuilder(
    listenable: draftGroup,
    builder: (context, _) => ConfigTabScaffold(
      title: title,
      description: description,
      group: draftGroup,
      saving: saving,
      outcome: outcome,
      onSave: save,
      onCancel: cancel,
      onReconcile: reconcile,
      readOnly: readOnly,
      aside: aside,
      externalChange: draftGroup.hasExternalChange,
      externalChangeRev: model.config.rev,
      child: child,
    ),
  );
}

class ConfigTabScaffold extends StatelessWidget {
  const ConfigTabScaffold({
    super.key,
    required this.title,
    required this.description,
    required this.group,
    required this.saving,
    required this.outcome,
    required this.onSave,
    required this.onCancel,
    required this.onReconcile,
    required this.child,
    this.readOnly = const [],
    this.aside,
    this.externalChange = false,
    this.externalChangeRev,
  });

  final String title;
  final String description;
  final ConfigDraftGroup group;
  final bool saving;
  final ConfigSaveOutcome? outcome;
  final Future<void> Function() onSave;
  final VoidCallback onCancel;
  final Future<void> Function() onReconcile;
  final Widget child;

  /// Extra lines under the editor header (e.g. a warning that a card field is read-only this wave).
  final List<Widget> readOnly;

  /// Optional right column at [ShellWidth.aside]: reference data or context that should stay beside
  /// the form instead of scrolling away (the demo's geometry/last-run cards).
  final Widget? aside;

  /// The core changed this block while the draft was dirty (marked while the tab may have been
  /// off-screen). The banner waits for the explicit reconcile.
  final bool externalChange;

  /// The mirror revision the external-change banner shows.
  final int? externalChangeRev;

  @override
  Widget build(BuildContext context) {
    final asideWidget = aside;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(
            ShellSpace.lg,
            ShellSpace.lg,
            ShellSpace.lg,
            ShellSpace.sm,
          ),
          child: PageHeader(
            title: title,
            subtitle: description,
            trailing: group.isDirty
                ? KeyedSubtree(
                    key: ValueKey('dirty-${group.label}'),
                    child: const StatusChip(
                      kind: StatusKind.warning,
                      label: 'Unsaved changes',
                    ),
                  )
                : const StatusChip(
                    kind: StatusKind.muted,
                    label: 'All changes saved',
                  ),
          ),
        ),
        Expanded(
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: SingleChildScrollView(
                  // The bottom inset keeps the last line clear of the save bar instead of clipping
                  // it at the scroll viewport edge.
                  padding: const EdgeInsets.fromLTRB(
                    ShellSpace.lg,
                    0,
                    ShellSpace.lg,
                    ShellSpace.sm,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    // Editable content first; calibrated/reference data is demoted below it
                    // (§2.1/§2.5.3 of the design audit).
                    children: [child, ...readOnly],
                  ),
                ),
              ),
              if (asideWidget != null)
                SizedBox(
                  width: ShellWidth.aside,
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.fromLTRB(
                      0,
                      0,
                      ShellSpace.lg,
                      ShellSpace.sm,
                    ),
                    child: asideWidget,
                  ),
                ),
            ],
          ),
        ),
        _SaveBar(
          group: group,
          saving: saving,
          outcome: outcome,
          onSave: onSave,
          onCancel: onCancel,
          onReconcile: onReconcile,
          externalChange: externalChange,
          externalChangeRev: externalChangeRev,
        ),
      ],
    );
  }
}

class _SaveBar extends StatelessWidget {
  const _SaveBar({
    required this.group,
    required this.saving,
    required this.outcome,
    required this.onSave,
    required this.onCancel,
    required this.onReconcile,
    required this.externalChange,
    required this.externalChangeRev,
  });

  final ConfigDraftGroup group;
  final bool saving;
  final ConfigSaveOutcome? outcome;
  final Future<void> Function() onSave;
  final VoidCallback onCancel;
  final Future<void> Function() onReconcile;
  final bool externalChange;
  final int? externalChangeRev;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final result = outcome;
    final conflict = result?.conflict;
    return Material(
      color: theme.colorScheme.surfaceContainer,
      child: Padding(
        padding: const EdgeInsets.all(ShellSpace.md),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (externalChange && conflict == null) ...[
              Text(
                'The core changed this block while you were editing it (now at rev ${externalChangeRev ?? '?'}).',
                key: ValueKey('external-change-${group.label}'),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),
              TextButton(
                key: ValueKey('external-change-reconcile-${group.label}'),
                onPressed: saving ? null : onReconcile,
                child: const Text('Discard my edits and use the core version'),
              ),
            ],
            if (conflict != null) ...[
              Text(
                'The core changed the config underneath this tab '
                '(now at rev ${conflict.conflictRev ?? '?'}: '
                '${_changedNames(conflict)}).',
                key: ValueKey('conflict-${group.label}'),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.error,
                ),
              ),
              TextButton(
                key: ValueKey('reconcile-${group.label}'),
                onPressed: saving ? null : onReconcile,
                child: const Text(
                  'Discard my changes and use the core version',
                ),
              ),
            ],
            if (result != null && !result.saved)
              for (var index = 0; index < result.errorLines.length; index++)
                Padding(
                  padding: const EdgeInsets.only(bottom: 2),
                  child: Text(
                    result.errorLines[index],
                    key: ValueKey('save-error-${group.label}-$index'),
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: theme.colorScheme.error,
                    ),
                  ),
                ),
            if (result != null && result.saved)
              Text(
                'Saved — rev ${result.result?.rev ?? '?'}',
                key: ValueKey('save-ok-${group.label}'),
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.primary,
                ),
              ),
            if (result != null)
              for (final line in result.warningLines)
                Text(line, style: theme.textTheme.bodySmall),
            const SizedBox(height: ShellSpace.xs),
            Row(
              children: [
                // The dirty state is stated once, in the header chip. The bar reports outcomes only
                // (the error/saved lines above), so it no longer repeats "Editing"/"No changes".
                const Spacer(),
                TextButton(
                  key: ValueKey('cancel-${group.label}'),
                  onPressed: group.isDirty && !saving ? onCancel : null,
                  child: const Text('Cancel'),
                ),
                const SizedBox(width: ShellSpace.sm),
                FilledButton(
                  key: ValueKey('save-${group.label}'),
                  onPressed: group.isDirty && !saving ? onSave : null,
                  child: saving
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Text('Save'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  String _changedNames(ErrorObject conflict) {
    final sections = conflict.conflictSections;
    if (sections == null || sections.isEmpty) {
      return 'another block';
    }
    return sections.keys.join(', ');
  }
}

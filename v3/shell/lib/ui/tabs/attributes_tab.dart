/// The Attributes tab: a read-only view of the dictionary the core has loaded for the active client
/// (`attributes.get`). Grouped by category with counts, the OCR variant spellings per entry, and the
/// per-level/negative/text-fix tables as separate read-only sections.
///
/// The payload is decoded app-locally ([AttributesResult]): it is not a wire type, so no `lib/protocol`
/// change exists or is needed. The search field filters client-side over names and OCR forms only — it
/// is a view, not a rule.
///
/// Presentation follows [PLAN-V3-UI-UX.md] §6 "Attributes": a search field over a dictionary table, an
/// [EmptyState] for zero matches with a Clear action, and the unfiltered reference tables hidden while
/// a query is active (they are not searchable, so leaving them up made the filter look half-applied).
///
/// [PLAN-V3-UI-UX.md]: ../../../../docs/plans/PLAN-V3-UI-UX.md
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../components/empty_state.dart';
import '../components/info_row.dart';
import '../components/shell_prose.dart';
import '../components/status_banner.dart';
import '../components/status_chip.dart';
import '../components/surface_panel.dart';
import '../theme.dart';

class AttributesTab extends StatefulWidget {
  const AttributesTab({super.key, required this.model});

  final ShellModel model;

  @override
  State<AttributesTab> createState() => _AttributesTabState();
}

class _AttributesTabState extends State<AttributesTab>
    with AutomaticKeepAliveClientMixin<AttributesTab> {
  @override
  bool get wantKeepAlive => true;

  final _search = TextEditingController();
  CoreActionResult<AttributesResult>? _result;
  bool _loading = false;
  String _query = '';
  int? _seenRev;

  @override
  void initState() {
    super.initState();
    widget.model.config.addListener(_onConfigChanged);
    _seenRev = widget.model.config.rev;
    unawaited(_refresh());
  }

  @override
  void dispose() {
    widget.model.config.removeListener(_onConfigChanged);
    _search.dispose();
    super.dispose();
  }

  /// A variant switch (or any config write) can change the loaded dictionary; refresh on the next
  /// revision so the tab cannot show the previous client's dictionary.
  void _onConfigChanged() {
    final rev = widget.model.config.rev;
    if (_seenRev == rev) {
      return;
    }
    _seenRev = rev;
    unawaited(_refresh());
  }

  Future<void> _refresh() async {
    if (!mounted) {
      return;
    }
    setState(() => _loading = true);
    final result = await widget.model.attributesGet();
    if (!mounted) {
      return;
    }
    setState(() {
      _loading = false;
      _result = result;
    });
  }

  /// Clears the view filter without touching the core: the dictionary itself is read-only data.
  void _clearSearch() {
    _search.clear();
    setState(() => _query = '');
  }

  @override
  Widget build(BuildContext context) {
    super.build(context); // AutomaticKeepAliveClientMixin bookkeeping.
    final theme = Theme.of(context);
    final result = _result;

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
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Attributes', style: theme.textTheme.headlineSmall),
                    const SizedBox(height: ShellSpace.xxs),
                    const ShellProse(
                      'The dictionary the core loaded for the active client — read-only. '
                      'The tuner\'s filter rules match the names shown here.',
                    ),
                  ],
                ),
              ),
              OutlinedButton(
                key: const ValueKey('attributes-refresh'),
                onPressed: _loading ? null : _refresh,
                child: Text(_loading ? 'Reading…' : 'Refresh'),
              ),
            ],
          ),
        ),
        Expanded(
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(horizontal: ShellSpace.lg),
            child: _body(theme, result),
          ),
        ),
      ],
    );
  }

  Widget _body(ThemeData theme, CoreActionResult<AttributesResult>? result) {
    if (_loading && result == null) {
      return const Padding(
        padding: EdgeInsets.only(top: ShellSpace.md),
        child: Row(
          children: [
            SizedBox(
              width: ShellSize.iconMd,
              height: ShellSize.iconMd,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
            SizedBox(width: ShellSpace.sm),
            Text('Loading dictionary…'),
          ],
        ),
      );
    }
    if (result == null) {
      return const SizedBox.shrink();
    }
    if (!result.ok) {
      return Padding(
        padding: const EdgeInsets.only(top: ShellSpace.md),
        child: StatusBanner(
          key: const ValueKey('attributes-error'),
          kind: StatusKind.error,
          title: 'The dictionary could not be read',
          detail: result.errorLine ?? 'the dictionary could not be read',
        ),
      );
    }

    final value = result.value!;
    if (!value.hasVariant) {
      return Padding(
        padding: const EdgeInsets.only(top: ShellSpace.md),
        child: SurfacePanel(
          title: 'Dictionary',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const InfoRow(label: 'Client variant', value: '—'),
              const InfoRow(label: 'Dictionary file', value: '—'),
              const SizedBox(height: ShellSpace.md),
              Text(
                'No client resolved — the core has no active variant, so no attribute '
                'dictionary is loaded.',
                key: const ValueKey('attributes-no-variant'),
                style: theme.textTheme.bodyMedium,
              ),
            ],
          ),
        ),
      );
    }

    final dictionary = value.attributes;
    final groups = dictionary.byCategory;
    final query = _query.trim();
    final filtering = query.isNotEmpty;
    final filtered = <({String category, List<AttributeDef> entries})>[];
    for (final group in groups) {
      final entries = group.entries
          .where((attribute) => attribute.matches(query))
          .toList();
      if (entries.isNotEmpty || !filtering) {
        filtered.add((category: group.category, entries: entries));
      }
    }
    final shown = filtered.fold<int>(
      0,
      (sum, group) => sum + group.entries.length,
    );
    final total = dictionary.attributes.length;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SurfacePanel(
          title: 'Dictionary',
          trailing: Text(
            '$shown of $total attributes',
            key: const ValueKey('attributes-count'),
            style: theme.textTheme.labelSmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              InfoRow(label: 'Client variant', value: value.variant ?? '—'),
              InfoRow(label: 'Dictionary file', value: value.dictionary ?? '—'),
              const SizedBox(height: ShellSpace.md),
              TextField(
                key: const ValueKey('attributes-search'),
                controller: _search,
                decoration: InputDecoration(
                  labelText: 'Search names and OCR forms',
                  prefixIcon: const Icon(Icons.search, size: ShellSize.iconMd),
                  suffixIcon: filtering
                      ? IconButton(
                          key: const ValueKey('attributes-search-clear'),
                          tooltip: 'Clear search',
                          iconSize: ShellSize.iconMd,
                          onPressed: _clearSearch,
                          icon: const Icon(Icons.close),
                        )
                      : null,
                ),
                onChanged: (value) => setState(() => _query = value),
              ),
              const SizedBox(height: ShellSpace.md),
              if (filtering && shown == 0)
                EmptyState(
                  icon: Icons.search_off,
                  title: 'No names match \'$query\'',
                  hint:
                      'Clear the search to show all $total names and their OCR forms.',
                  action: OutlinedButton(
                    key: const ValueKey('attributes-clear-search'),
                    onPressed: _clearSearch,
                    child: const Text('Clear search'),
                  ),
                )
              else ...[
                // One header line makes the two columns read as a table rather than as loose text.
                Row(
                  children: [
                    SizedBox(
                      width: ShellWidth.fieldLg,
                      child: Text(
                        'Name',
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ),
                    Expanded(
                      child: Text(
                        'OCR variants',
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: ShellSpace.xs),
                for (final group in filtered) ...[
                  Padding(
                    padding: const EdgeInsets.only(top: ShellSpace.md),
                    child: Text(
                      '${group.category} (${group.entries.length})',
                      key: ValueKey('attribute-category-${group.category}'),
                      style: theme.textTheme.titleSmall,
                    ),
                  ),
                  const SizedBox(height: ShellSpace.xxs),
                  for (final attribute in group.entries)
                    Padding(
                      padding: const EdgeInsets.only(bottom: ShellSpace.xxs),
                      child: Row(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          SizedBox(
                            width: ShellWidth.fieldLg,
                            child: Text(
                              attribute.name,
                              key: ValueKey('attribute-${attribute.name}'),
                              style: theme.textTheme.bodyMedium,
                            ),
                          ),
                          Expanded(
                            child: Text(
                              attribute.variants.join(' · '),
                              style: theme.textTheme.bodySmall?.copyWith(
                                color: theme.colorScheme.onSurfaceVariant,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                ],
              ],
            ],
          ),
        ),
        // The per-level/negative/text-fix tables are reference data, not searchable entries: a
        // filter that showed them unfiltered made the search look half-applied (§6 "Attributes").
        if (!filtering) _referenceTables(theme, dictionary),
        const SizedBox(height: ShellSpace.lg),
      ],
    );
  }

  /// The tables that are NOT filtered by the search, rendered only while the search is empty.
  Widget _referenceTables(ThemeData theme, AttributesDictionary dictionary) {
    final rows = <Widget>[
      if (dictionary.perLevelStats.isNotEmpty)
        InfoRow(
          label: 'Per-level stat names',
          value: dictionary.perLevelStats.join(' · '),
        ),
      if (dictionary.negativeMarkers.isNotEmpty)
        InfoRow(
          label: 'Negative markers',
          value: dictionary.negativeMarkers.join(' · '),
        ),
      for (final entry in dictionary.textFixes.entries)
        if (entry.value.isNotEmpty)
          InfoRow(
            label: 'Text fixes — ${entry.key}',
            value: entry.value
                .map((fix) => '${fix.from} → ${fix.to}')
                .join(' · '),
          ),
    ];
    if (rows.isEmpty) {
      return const SizedBox.shrink();
    }
    return SurfacePanel(
      title: 'Reference tables',
      description:
          'Shown while the search is empty — these are not attribute names and the '
          'search does not filter them.',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: rows,
      ),
    );
  }
}

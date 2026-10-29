/// The Attributes tab: a read-only view of the dictionary the core has loaded for the active client
/// (`attributes.get`). Grouped by category with counts, the OCR variant spellings per entry, and the
/// per-level/negative/text-fix tables as separate read-only sections.
///
/// The payload is decoded app-locally ([AttributesResult]): it is not a wire type, so no `lib/protocol`
/// change exists or is needed. The search field filters client-side over names and OCR forms only — it
/// is a view, not a rule.
library;

import 'package:flutter/material.dart';

import '../../app/app.dart';
import '../theme.dart';
import 'fields.dart';

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
    _refresh();
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
    _refresh();
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
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text('Attributes', style: theme.textTheme.titleLarge),
                    const SizedBox(height: 2),
                    Text(
                      'The dictionary the core loaded for the active client — read-only. '
                      'The tuner\'s filter rules match the names shown here.',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
              OutlinedButton(
                key: const ValueKey('attributes-refresh'),
                onPressed: _loading ? null : _refresh,
                child: const Text('Refresh'),
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
        child: Text('Loading dictionary…'),
      );
    }
    if (result == null) {
      return const SizedBox.shrink();
    }
    if (!result.ok) {
      return Padding(
        padding: const EdgeInsets.only(top: ShellSpace.md),
        child: Text(
          result.errorLine ?? 'the dictionary could not be read',
          key: const ValueKey('attributes-error'),
          style: theme.textTheme.bodyMedium?.copyWith(
            color: theme.colorScheme.error,
          ),
        ),
      );
    }

    final value = result.value!;
    if (!value.hasVariant) {
      return Padding(
        padding: const EdgeInsets.only(top: ShellSpace.md),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            ReadOnlyValue(label: 'Client variant', value: '—'),
            ReadOnlyValue(label: 'Dictionary file', value: '—'),
            const SizedBox(height: ShellSpace.sm),
            Text(
              'No client resolved — the core has no active variant, so no attribute '
              'dictionary is loaded.',
              key: const ValueKey('attributes-no-variant'),
              style: theme.textTheme.bodyMedium,
            ),
          ],
        ),
      );
    }

    final dictionary = value.attributes;
    final groups = dictionary.byCategory;
    final filtered = <({String category, List<AttributeDef> entries})>[];
    for (final group in groups) {
      final entries = group.entries
          .where((attribute) => attribute.matches(_query))
          .toList();
      if (entries.isNotEmpty || _query.trim().isEmpty) {
        filtered.add((category: group.category, entries: entries));
      }
    }
    final shown = filtered.fold<int>(
      0,
      (sum, group) => sum + group.entries.length,
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        ReadOnlyValue(label: 'Client variant', value: value.variant ?? '—'),
        ReadOnlyValue(label: 'Dictionary file', value: value.dictionary ?? '—'),
        const SizedBox(height: ShellSpace.md),
        SizedBox(
          width: 320,
          child: TextField(
            key: const ValueKey('attributes-search'),
            controller: _search,
            decoration: const InputDecoration(
              labelText: 'Search names and OCR forms',
              isDense: true,
              border: OutlineInputBorder(),
            ),
            onChanged: (value) => setState(() => _query = value),
          ),
        ),
        const SizedBox(height: ShellSpace.xs),
        Text(
          '$shown of ${dictionary.attributes.length} attributes',
          key: const ValueKey('attributes-count'),
          style: theme.textTheme.labelSmall?.copyWith(
            color: theme.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: ShellSpace.md),
        for (final group in filtered)
          Padding(
            padding: const EdgeInsets.only(bottom: ShellSpace.md),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  '${group.category} (${group.entries.length})',
                  key: ValueKey('attribute-category-${group.category}'),
                  style: theme.textTheme.titleSmall,
                ),
                const SizedBox(height: ShellSpace.xs),
                for (final attribute in group.entries)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 2),
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
            ),
          ),
        _readOnlyList(theme, 'Per-level stat names', dictionary.perLevelStats),
        _readOnlyList(
          theme,
          'Negative attribute markers',
          dictionary.negativeMarkers,
        ),
        for (final entry in dictionary.textFixes.entries)
          _fixTable(theme, entry.key, entry.value),
        const SizedBox(height: ShellSpace.lg),
      ],
    );
  }

  Widget _readOnlyList(ThemeData theme, String title, List<String> values) {
    if (values.isEmpty) {
      return const SizedBox.shrink();
    }
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: theme.textTheme.titleSmall),
          const SizedBox(height: ShellSpace.xs),
          Text(values.join(' · '), style: theme.textTheme.bodySmall),
        ],
      ),
    );
  }

  Widget _fixTable(ThemeData theme, String name, List<TextFixEntry> fixes) {
    if (fixes.isEmpty) {
      return const SizedBox.shrink();
    }
    return Padding(
      padding: const EdgeInsets.only(bottom: ShellSpace.md),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Text fixes — $name', style: theme.textTheme.titleSmall),
          const SizedBox(height: ShellSpace.xs),
          for (final fix in fixes)
            Text(
              '${fix.from} → ${fix.to}',
              style: theme.textTheme.bodySmall?.copyWith(
                fontFamily: 'Consolas',
              ),
            ),
        ],
      ),
    );
  }
}

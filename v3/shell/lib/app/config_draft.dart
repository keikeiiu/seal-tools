/// A tab's editable copy of ONE config section — the shared draft pattern of PLAN-V3-SHELL §5/§6.
///
/// The core is the only writer and the only rule author. A draft is therefore:
///
/// - **seeded from the core's merged mirror**, filtered to the keys the tab may edit (a tab never
///   sends a field it does not own — PROTOCOL §7);
/// - **dirty** when its value differs from its baseline (JSON equality, map order is not data);
/// - **reset** on Cancel by copying the baseline back;
/// - **re-baselined** on a successful Save from the apply echo (the effective section the core
///   returned), so the dirty marker tells the truth after a write.
///
/// The scope decides the request shape: an owned top-level block goes as `{block: value}`; a
/// portable section goes under the `settings:` overlay (PROTOCOL §4/§7). Tuner and Gem edit portable
/// fields, so their drafts are [ConfigScope.portable] even though their merged mirror is the
/// flattened top-level block.
library;

import 'package:flutter/foundation.dart';

import 'config_json.dart';

/// Where a draft's value lives in a `config.apply` request.
enum ConfigScope {
  /// An owned top-level `local.yaml` block (`spammer`, `buy_sell`, `quest`).
  ownedBlock,

  /// A portable section of the `settings:` projection (`tuner`, `gem`).
  portableSection,
}

class ConfigDraft extends ChangeNotifier {
  ConfigDraft({
    required this.blockName,
    required this.scope,
    this.editableKeys = const {},
  });

  /// The config block/section name (`spammer`, `tuner`, …).
  final String blockName;

  /// Where the value is written (top level or under `settings`).
  final ConfigScope scope;

  /// The only keys this tab edits. Empty means the whole block (a block whose fields are all owned).
  /// Seeding FILTERS to these, so anything the tab does not own is never sent back (PROTOCOL §7).
  final Set<String> editableKeys;

  Map<String, dynamic> _value = const {};
  Map<String, dynamic> _baseline = const {};
  bool _seeded = false;
  int _seedRevision = 0;

  /// Bumped whenever the draft is re-seeded from the core (seed/reset/save). Widgets that hold a
  /// `TextEditingController` key their state on it, so an external re-baseline can never leave a
  /// stale text field showing the player's previous edit.
  int get seedRevision => _seedRevision;

  /// Whether the draft has been seeded from the core mirror at least once.
  bool get isSeeded => _seeded;

  /// The editable value the widgets bind to.
  Map<String, dynamic> get value => _value;

  /// The last value the core is known to have (the draft's "saved" state).
  Map<String, dynamic> get baseline => _baseline;

  /// True when the value differs from the baseline — the Save button's gate.
  bool get isDirty => !jsonEquals(_value, _baseline);

  /// Seeds from the merged config mirror. A dirty draft is left alone unless [force], which is the
  /// explicit reconcile path (a conflict is never resolved behind the player's back).
  void seedFromMirror(Map<String, dynamic> mergedConfig, {bool force = false}) {
    if (_seeded && isDirty && !force) {
      return;
    }
    final merged = asJsonMap(mergedConfig[blockName]);
    final seeded = pickKeys(merged, editableKeys);
    _baseline = seeded;
    _value = cloneJsonMap(seeded);
    _seeded = true;
    _seedRevision++;
    notifyListeners();
  }

  /// Replaces the whole editable value (a widget edited a nested field and built a new map).
  void setValue(Map<String, dynamic> next) {
    _value = _editableOnly(next);
    notifyListeners();
  }

  /// A nested replacement helper: copies the current value, lets [edit] mutate the copy, stores it.
  void edit(void Function(Map<String, dynamic> value) edit) {
    final next = cloneJsonMap(_value);
    edit(next);
    setValue(next);
  }

  /// Cancel: back to the last known core value.
  void reset() {
    _value = cloneJsonMap(_baseline);
    _seedRevision++;
    notifyListeners();
  }

  /// Save succeeded: the apply echo is the new truth. A missing/odd echo keeps the sent value.
  void saved(Map<String, dynamic>? echoedSection) {
    final effective = echoedSection == null
        ? cloneJsonMap(_value)
        : _editableOnly(echoedSection);
    _baseline = effective;
    _value = cloneJsonMap(effective);
    _seedRevision++;
    notifyListeners();
  }

  /// The `config.apply` sections payload — ONLY this tab's section.
  Map<String, dynamic> payload() => switch (scope) {
    ConfigScope.ownedBlock => {blockName: cloneJsonMap(_value)},
    ConfigScope.portableSection => {
      'settings': {blockName: cloneJsonMap(_value)},
    },
  };

  /// The section the core would echo for this draft (for reading a result).
  Map<String, dynamic>? echoOf(Map<String, dynamic> sections) {
    if (scope == ConfigScope.ownedBlock) {
      final own = sections[blockName];
      return own is Map<String, dynamic> ? own : null;
    }
    final settings = sections['settings'];
    if (settings is! Map<String, dynamic>) {
      return null;
    }
    final own = settings[blockName];
    return own is Map<String, dynamic> ? own : null;
  }

  Map<String, dynamic> cloneJsonMap(Map<String, dynamic> source) =>
      cloneJson(source) as Map<String, dynamic>;

  Map<String, dynamic> _editableOnly(Map<String, dynamic> source) =>
      pickKeys(source, editableKeys);
}

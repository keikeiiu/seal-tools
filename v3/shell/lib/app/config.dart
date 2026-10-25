/// The config sub-model: the merged config and revision the shell mirrors, plus the command
/// outcomes the tabs need. The blocks stay opaque `Map<String, dynamic>` values — the shell copies
/// what it does not own and never composes `local.yaml` ([PROTOCOL.md] §7, [DESIGN.md] §4).
///
/// One judgement lives here and is marked: after a successful `config.apply` the shell overlays the
/// returned effective sections onto its mirrored config. That is a UI convenience — the CORE is the
/// writer and `config.reload` is the whole truth; the overlay only saves a round-trip for the blocks
/// the response names.
library;

import 'package:flutter/foundation.dart';

import '../protocol/protocol.dart';

class ConfigModel extends ChangeNotifier {
  JsonMap _config = const {};
  int _rev = 0;
  String _root = '';
  String _coreVersion = '';
  ConfigValidateResult? _validation;
  ConfigApplyResult? _apply;
  ConfigReloadResult? _reload;
  ErrorObject? _lastError;
  ErrorObject? _conflict;

  /// The merged config from `hello`/`config.reload`, overlaid by applied sections. Opaque blocks.
  JsonMap get config => _config;

  /// The session revision the shell echos back on every save ([PROTOCOL.md] §7).
  int get rev => _rev;

  /// The application root from `hello`.
  String get root => _root;

  /// The core's version from `hello` (mirrored here so the status strip needs one sub-model).
  String get coreVersion => _coreVersion;

  /// The last `config.validate` answer, or null before the first save attempt.
  ConfigValidateResult? get lastValidation => _validation;

  /// The last successful `config.apply` answer.
  ConfigApplyResult? get lastApply => _apply;

  /// The last successful `config.reload` answer.
  ConfigReloadResult? get lastReload => _reload;

  /// The typed protocol error from the last config command, when it failed.
  ErrorObject? get lastError => _lastError;

  /// The `conflict` error that opened the reconcile state ([PROTOCOL.md] §7), or null.
  ///
  /// The shell does not resolve it automatically: the player re-saves after seeing what changed, so
  /// this is data for the future reconcile sheet, not an action.
  ErrorObject? get conflict => _conflict;

  /// One block of the mirrored config, or null.
  Object? block(String name) => _config[name];

  // ── mutations: ShellModel only ────────────────────────────────────────────────────────────

  void applyHello(HelloResult hello) {
    _config = hello.config;
    _rev = hello.rev;
    _root = hello.root;
    _coreVersion = hello.coreVersion;
    _lastError = null;
    _conflict = null;
    notifyListeners();
  }

  /// Mirrors a `config.changed` event: the core wrote a block the UI displays ([PROTOCOL.md] §5).
  void applyChanged(ConfigChanged changed) {
    _rev = changed.rev;
    _config = {..._config, changed.block: changed.values};
    notifyListeners();
  }

  void recordValidation(ConfigValidateResult result) {
    _validation = result;
    _lastError = null;
    notifyListeners();
  }

  void recordApply(ConfigApplyResult result) {
    _apply = result;
    _rev = result.rev;
    // UI convenience: the response carries the effective values of the sections just written.
    _config = {..._config, ...result.sections};
    _lastError = null;
    _conflict = null;
    notifyListeners();
  }

  void recordReload(ConfigReloadResult result) {
    _reload = result;
    _config = result.config;
    _rev = result.rev;
    _lastError = null;
    _conflict = null;
    notifyListeners();
  }

  /// Records a typed protocol error; [conflict] is kept specifically because it is the reconcile
  /// signal ([PROTOCOL.md] §7). The message is displayed verbatim.
  void recordError(ErrorObject error) {
    _lastError = error;
    _conflict = error.code == ErrorCode.conflict ? error : null;
    notifyListeners();
  }

  /// Clears the last error (a dismissible banner's action).
  void clearError() {
    if (_lastError == null) {
      return;
    }
    _lastError = null;
    _conflict = null;
    notifyListeners();
  }
}

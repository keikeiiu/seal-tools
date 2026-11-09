/// The shell's ONE `ChangeNotifier` ([PLAN-V3-SHELL.md] §5): it owns the [ProtocolClient], the four
/// sub-models the UI renders, and the commands the cards call.
///
/// What lives where, and why:
///
/// - **Protocol bookkeeping** (the model's decisions): the revision echoed on `config.apply`, the
///   `validate`-then-`apply` order a save is ([PROTOCOL.md] §7), clearing cards on a new process,
///   the connection lifecycle, and capturing the stderr tail at a crash. These are rules, and no
///   widget re-implements them.
/// - **UI convenience** (marked where it happens): the "command in flight" flag that disables a
///   card's buttons, and the refusal line stored verbatim per card. Neither is state the core owns.
///
/// It never decides game policy: a Start sends `tool.start`; the card changes only when the core
/// publishes `state` ([PLAN-V3-SHELL.md] §5).
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'dart:async';

import 'package:flutter/foundation.dart';

import '../protocol/protocol.dart';
import 'arduino_details.dart';
import 'attributes_result.dart';
import 'calibration_result.dart';
import 'config.dart';
import 'config_draft.dart';
import 'config_json.dart';
import 'connection.dart';
import 'core_actions.dart';
import 'dock_placement.dart';
import 'host_actions.dart';
import 'key_names_result.dart';
import 'logs.dart';
import 'pet_preflight.dart';
import 'theme_preference.dart';
import 'tools.dart';
import 'version.dart';

/// Creates the transport for a connection attempt. The real one spawns a core process; tests return
/// a fake. One call per spawn, so a restart gets a fresh transport from the same factory.
typedef CoreTransportFactory = Future<Transport> Function();

/// The bag is 8×8: a sell selection names bag cells 0..63 (`BagGrid::SLOT_COUNT` in `seal-domain`).
const int _sellSlotCount = 64;

class ShellModel extends ChangeNotifier {
  ShellModel({
    required this.transportFactory,
    HostActions? host,
    this.shellVersion = protocolClientVersion,
    this.requestTimeout = const Duration(seconds: 10),
    this.handshakeTimeout = const Duration(seconds: 5),
    this.windowSaveDebounce = const Duration(milliseconds: 600),
  }) : host = host ?? const MethodChannelHostActions() {
    connection.addListener(_onSubmodelChanged);
    // Config writes are infrequent and several chrome surfaces read the mirror; run/status events
    // (tools) must NOT notify the root (SHELL-F5). Logs have no model listener at all.
    config.addListener(_onSubmodelChanged);
    tools.addListener(_onRunAffectingChanged);
  }

  /// How a connection is created: the shell's `main.dart` supplies the process spawner, tests a
  /// fake. One call per spawn, so a restart gets a fresh transport from the same factory.
  final CoreTransportFactory transportFactory;

  /// The runner window seam. Production uses the `seal/host` method channel; tests inject a fake.
  final HostActions host;

  /// How long a user move/resize must settle before the placement is written with `ui.window`.
  /// A drag reports a rect on every step; one write per settled placement keeps `local.yaml` quiet.
  final Duration windowSaveDebounce;

  /// The shell's PROTOCOL CLIENT identity, sent in `hello`; supplied by `main.dart` from
  /// `version.dart`. It is not the product version (the app bar shows [productVersion]) and the
  /// protocol library's default deliberately stays `0.0.0` — the version choice belongs to the shell.
  final String shellVersion;

  /// Per-request deadline for commands ([PLAN-V3-SHELL.md] §4: a shell constant, not protocol).
  final Duration requestTimeout;

  /// `hello` gets its own shorter deadline so a wedged core shows the crash view quickly.
  final Duration handshakeTimeout;

  /// Connection lifecycle and the crash facts.
  final ConnectionModel connection = ConnectionModel();

  /// The eight cards' core-published facts.
  final ToolsModel tools = ToolsModel();

  /// The mirrored config + revision and command outcomes.
  final ConfigModel config = ConfigModel();

  /// The status surface's ring buffer.
  final LogBuffer logs = LogBuffer();

  ProtocolClient? _client;
  StreamSubscription<ProtocolEvent>? _eventSubscription;
  StreamSubscription<String>? _logSubscription;
  Future<void>? _connectInFlight;
  bool _closing = false;
  bool _disposed = false;
  final Set<ConfigDraftGroup> _dirtyDraftGroups = <ConfigDraftGroup>{};

  /// Set by the first failure of this connection; the crash tail and the handshake catch can both
  /// notice the same dead pipe, and only one of them may count (SHELL-F4).
  bool _failureReported = false;

  /// The live client, for the wiring and for tests; widgets use the commands below.
  ProtocolClient? get client => _client;

  bool get isReady => connection.isReady;

  /// True once [shutdown] started; the app is going away and no reconnect is attempted.
  bool get isClosing => _closing;

  /// True when at least one kept-alive config tab has unsaved edits (SHELL-F6). The tabs register
  /// themselves through [setDraftDirty]; the exit guard uses this before shutting the core down.
  bool get hasDirtyDrafts => _dirtyDraftGroups.isNotEmpty;

  /// Called by [ConfigTabState] when a tab's draft-group dirty state changes. Only a CHANGE notifies;
  /// the app bar and the close guard do not need a rebuild per keystroke.
  void setDraftDirty(ConfigDraftGroup group, bool dirty) {
    final changed = dirty
        ? _dirtyDraftGroups.add(group)
        : _dirtyDraftGroups.remove(group);
    if (changed) {
      notifyListeners();
    }
  }

  /// The persisted always-on-top preference from the merged config mirror
  /// (`settings.ui.always_on_top`, false when the block or field is absent).
  ///
  /// The runner applies it once after `hello` and on each toggle; a `config.changed` event updates
  /// this mirror but deliberately does not re-apply it (the toggle's own save is the only writer).
  bool get alwaysOnTop {
    final settings = asJsonMap(config.config['settings']);
    final portable = asBool(asJsonMap(settings['ui'])['always_on_top']);
    if (portable != null) {
      return portable;
    }
    // A core that flattens the portable overlay into the merged config exposes the same field on
    // the top-level `ui` section; read it too so the setting works either way.
    return asBool(asJsonMap(config.config['ui'])['always_on_top']) ?? false;
  }

  /// Applies [enabled] to the window at once, then persists it as `settings.ui.always_on_top`
  /// through the normal validate → apply path.
  ///
  /// A refusal puts the host back on the saved mirror value and returns the outcome so the caller
  /// can surface the core's line; the pin therefore never claims a setting the config refused.
  Future<ConfigSaveOutcome> setAlwaysOnTop(bool enabled) async {
    try {
      await host.setAlwaysOnTop(enabled);
    } catch (error) {
      logs.addShellNote('cannot set always-on-top: $error');
      return ConfigSaveOutcome(note: 'cannot set always-on-top: $error');
    }
    final outcome = await saveConfig({
      'settings': {
        'ui': {'always_on_top': enabled},
      },
    });
    if (!outcome.saved) {
      await _applyAlwaysOnTopFromMirror();
    }
    return outcome;
  }

  /// The host's copy of the persisted preference. Called once after `hello`, and after a refused
  /// toggle so the window and the mirror stay in agreement.
  Future<void> _applyAlwaysOnTopFromMirror() async {
    try {
      await host.setAlwaysOnTop(alwaysOnTop);
    } catch (error) {
      logs.addShellNote('cannot set always-on-top: $error');
    }
  }

  // ── window placement (v2 parity: the launcher reopens where the player left it) ─────────────

  StreamSubscription<WindowRect>? _windowMoveSubscription;
  Timer? _windowSaveTimer;
  WindowRect? _lastSavedWindow;
  WindowRect? _pendingWindow;

  // ── mini running card ────────────────────────────────────────────────────────────────────────
  // A foreground run collapses the window to the 360×96 bar beside the game. The full placement is
  // read from the runner before the collapse and restored when the player expands; window moves
  // while mini are ignored so a resize cannot overwrite the player's remembered placement. The
  // resident Pet Feeder never collapses the window (Pet is excluded from `_foregroundTool`).
  static const WindowRect _fullMinimum = WindowRect(
    left: 0,
    top: 0,
    width: 1120,
    height: 680,
  );
  static const int _miniWidth = 360;
  static const int _miniHeight = 96;
  bool _miniActive = false;
  bool _miniSuppressed = false;
  bool _miniSyncing = false;
  ToolId? _miniTool;

  /// The per-run file log the Logs drawer should point at: the most recently finished tool's
  /// `<tool>.log`, or null before any run has finished. The file lives in the install's `logs/`.
  String? get lastRunLogFile {
    // Prefer the run that is happening right now (its log is growing); otherwise the most recent
    // finished run, so the label still points at the file the player just asked about.
    for (final entry in tools.snapshots.entries) {
      if (entry.value.running) {
        return '${entry.key.wire}.log';
      }
    }
    final finished = tools.lastFinished;
    return finished == null ? null : '${finished.tool.wire}.log';
  }

  /// Open the install's `logs/` folder (selecting the last run's log when known). A host failure is
  /// only a log note: the action is a convenience, never shell state.
  Future<void> openLogsFolder() async {
    try {
      await host.openLogsFolder(highlight: lastRunLogFile);
    } catch (error) {
      logs.addShellNote('cannot open the logs folder: $error');
    }
  }

  /// True when the run was started from the quick dock, so an optional after-run return can go back
  /// to the dock instead of the full window. Session state, not config.
  bool _miniReturnToDock = false;
  WindowRect? _miniRestoreRect;
  WindowRect? _miniRect;

  /// The mini bar's last position (session memory; snapped to an edge when it was left).
  WindowRect? _miniBarRect;
  WindowRect? _miniWorkArea;

  // ── quick dock ───────────────────────────────────────────────────────────────────────────────
  // The idle companion window: a launcher, not a second dashboard. It owns the same compact chrome
  // as the run bar; when a run starts it hands the window to the mini bar, and the finished bar can
  // hand it back. Placement is session/v1 only (a config field is the documented follow-up).
  bool _dockActive = false;
  bool _dockCollapsed = false;
  DockBehavior _dockBehavior = DockBehavior.alwaysOpen;
  DockPlacement _dockPlacement = DockPlacement.rightRail;
  DockSize _dockSize = DockSize.standard;
  WindowRect? _dockRestoreRect;
  WindowRect? _dockWorkArea;
  WindowRect? _dockRectValue;

  // The app appearance, persisted as `settings.ui.theme` (dark is the pre-setting default).
  ShellThemeMode _themeMode = ShellThemeMode.dark;

  // Last-used Buy choice for the dock's next run (per-run args; session state, not config).
  String? _dockBuyPreset;
  int? _dockBuyCount;

  /// A setup-row request from the dock: the full shell selects the matching tab when it returns.
  final ValueNotifier<ToolId?> _setupRequest = ValueNotifier<ToolId?>(null);

  /// The dock's editable quick-action list: tool ids (`spammer`, `buy`, …) and pinned Sell ids
  /// (`pin:<name>`). Persisted with the placement preference.
  static const List<String> _defaultDockTiles = [
    'spammer',
    'buy',
    'sell',
    'quest',
    'gem',
    'tuner',
    'holdspace',
  ];
  List<String> _dockTiles = [..._defaultDockTiles];
  List<DockPin> _dockPins = const [];

  /// The placement the shell last applied or saved. A move event that reports the SAME rect is not
  /// news — the restore's own `SetWindowPos` reports one too — so it never schedules a write.
  WindowRect? get windowPlacement => _lastSavedWindow;

  /// Restores `ui`'s remembered placement after `hello` and starts watching the runner's rect
  /// reports.
  ///
  /// No placement stated (a fresh install, or v2's optional `LocalUi` fields left unset) means the
  /// window keeps its created rect, which becomes the baseline a later move is compared against.
  ///
  /// A placement that still fits on the monitors that exist is applied exactly; one saved on a
  /// monitor that is now gone is clamped into the PRIMARY work area, so the window opens somewhere
  /// the player can see and reach instead of off-screen.
  Future<void> _restoreWindowPlacement() async {
    if (_disposed) {
      return;
    }
    _windowMoveSubscription ??= host.windowMoves.listen(_onWindowMoved);
    final saved = _savedWindowRect();
    if (saved != null) {
      try {
        final virtualScreen = await host.virtualScreen();
        var placed = saved;
        if (!virtualScreen.encloses(saved)) {
          placed = saved.clampedTo(await host.primaryWorkArea());
        }
        // Mark it BEFORE applying: the runner's own `windowMoved` for this SetWindowPos can reach
        // Dart before the method reply, and it must compare equal to something (otherwise a launch
        // with a remembered placement would write it straight back once).
        final previous = _lastSavedWindow;
        _lastSavedWindow = placed;
        try {
          await host.setWindowRect(placed);
        } catch (error) {
          _lastSavedWindow = previous;
          rethrow;
        }
        if (placed != saved) {
          logs.addShellNote(
            'window placement clamped into the primary monitor '
            '($saved → $placed)',
          );
        }
        return;
      } catch (error) {
        logs.addShellNote('cannot restore the window placement: $error');
      }
    }
    try {
      _lastSavedWindow = await host.windowRect();
    } catch (error) {
      logs.addShellNote('cannot read the window placement: $error');
    }
  }

  /// The machine `ui` block's remembered placement, or null when it is not all there: v2's
  /// `LocalUi` keeps left/top/width/expanded_height optional, and a partial one is not a placement.
  WindowRect? _savedWindowRect() {
    final ui = asJsonMap(config.config['ui']);
    final left = asNumber(ui['left'])?.round();
    final top = asNumber(ui['top'])?.round();
    final width = asNumber(ui['width'])?.round();
    final height = asNumber(ui['expanded_height'])?.round();
    if (left == null || top == null || width == null || height == null) {
      return null;
    }
    return WindowRect(left: left, top: top, width: width, height: height);
  }

  void _onWindowMoved(WindowRect rect) {
    if (!isReady || _closing || _disposed || _miniActive) {
      return;
    }
    if (rect == _lastSavedWindow || rect == _pendingWindow) {
      return;
    }
    _pendingWindow = rect;
    _windowSaveTimer?.cancel();
    _windowSaveTimer = Timer(windowSaveDebounce, () {
      unawaited(_savePendingWindow());
    });
  }

  Future<void> _savePendingWindow() async {
    final rect = _pendingWindow;
    _pendingWindow = null;
    if (rect == null || _disposed || _closing || !isReady) {
      return;
    }
    // The restore's own `SetWindowPos` can report its rect before the restore's await resumed; the
    // pending copy is then already the saved placement and there is nothing to write.
    if (rect == _lastSavedWindow) {
      return;
    }
    // The capture lifecycle hides the window (hide → capture → show). A hidden window has no
    // placement the player chose, so nothing is written then; the next move after the show is a
    // fresh placement.
    final bool visible;
    try {
      visible = await host.isWindowVisible();
    } catch (error) {
      logs.addShellNote('cannot read the window visibility: $error');
      return;
    }
    if (!visible || _disposed || _closing || !isReady) {
      return;
    }
    // `ui.window` carries the WHOLE five-field machine block, so the stored pin travels through
    // unchanged: `pinned` belongs to v2's machine `ui` block, while the portable pin is
    // `settings.ui.always_on_top` (`setAlwaysOnTop`) — this feature never edits either.
    final pinned = asBool(asJsonMap(config.config['ui'])['pinned']) ?? false;
    final saved = await saveWindow(
      UiWindowArgs(
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
        pinned: pinned,
      ),
    );
    if (saved) {
      _lastSavedWindow = rect;
    }
    // A refusal is already on the log/status through `_voidCommand`; the next settled move retries.
  }

  /// Spawns (or reuses) the core, performs `hello`, and fills the config mirror.
  ///
  /// Safe to call more than once: a call while a connection attempt is in flight joins it. A failure
  /// lands in [connection] as a failed state — this future completes normally either way, because a
  /// failed connection is a RENDERED state, not an exception for `main` to swallow.
  Future<void> connect() {
    if (_closing || _disposed) {
      return Future<void>.value();
    }
    final inFlight = _connectInFlight;
    if (inFlight != null) {
      return inFlight;
    }
    final future = _connect().whenComplete(() => _connectInFlight = null);
    _connectInFlight = future;
    return future;
  }

  Future<void> _connect() async {
    if (_closing || _disposed) {
      return;
    }
    _failureReported = false;
    connection.markStarting();
    final Transport transport;
    try {
      transport = await transportFactory();
    } catch (error) {
      _fail(ConnectionFailure.spawn, 'the core did not start: $error');
      return;
    }

    final client = ProtocolClient(
      transport,
      clientVersion: shellVersion,
      requestTimeout: requestTimeout,
    );
    _client = client;
    _eventSubscription = client.events.listen(
      _onEvent,
      onDone: () => _onCoreOutputClosed(client),
    );
    _logSubscription = client.logs.listen(logs.addStderr);
    connection.markHandshaking();

    final HelloResult hello;
    try {
      hello = await client.handshake(timeout: handshakeTimeout);
    } on HandshakeException catch (error) {
      final failure = switch (error.failure) {
        HandshakeFailure.badVersion => ConnectionFailure.badVersion,
        HandshakeFailure.protocolMismatch => ConnectionFailure.badVersion,
        HandshakeFailure.notReady => ConnectionFailure.handshake,
      };
      _fail(failure, _describeHandshake(error));
      await _detachClient(client);
      return;
    } catch (error) {
      _fail(ConnectionFailure.handshake, 'the handshake failed: $error');
      await _detachClient(client);
      return;
    }

    if (_closing || _disposed) {
      await _detachClient(client);
      return;
    }
    config.applyHello(hello);
    connection.markReady(coreVersion: hello.coreVersion, root: hello.root);
    logs.addShellNote(
      'core ${hello.coreVersion} connected '
      '(rev ${hello.rev}, root ${hello.root})',
    );
    // The shell owns its own placement preference in the portable `settings.ui.quick_dock` block
    // (`placement` + `collapsed`); take it before any compact window is shown.
    _restoreDockPreference();
    _restoreThemePreference();
    // The config mirror is now the truth; apply the persisted pin exactly once. Hide/show stays
    // untouched: this is one SetWindowPos, not part of the capture's frame lifecycle.
    await _applyAlwaysOnTopFromMirror();
    // Then put the window back where the player left it (v2 parity), clamped into the primary
    // monitor, and start watching for the moves this run will remember.
    await _restoreWindowPlacement();
  }

  /// One click, one new process, one new handshake ([PLAN-V3-SHELL.md] §2) — never an auto loop.
  ///
  /// The previous process is stopped BEFORE the new one spawns: a graceful [ProtocolClient.close]
  /// first (EOF is the safe stop), then an explicit terminate so a wedged or already-crashed core can
  /// never outlive the restart and keep driving the board beside the new one (SHELL-F1).
  Future<void> restart() async {
    if (_closing || _disposed) {
      return;
    }
    final previous = _client;
    _client = null;
    await _cancelSubscriptions();
    if (previous != null) {
      try {
        await previous.close();
      } catch (_) {
        // The old connection is dead or dying either way; the new process is the point.
      }
      try {
        await previous.terminate(reason: 'the shell restarted the core');
      } catch (_) {
        // Termination is idempotent; a process that refused to die cannot block the restart forever.
      }
    }
    // Cards keep the last rendered state (PLAN-V3-SHELL §2) — nothing is cleared here.
    await connect();
  }

  /// Sends `shutdown`, then EOF on the core's stdin — the safe stop ([PROTOCOL.md] §1/§2).
  Future<void> shutdown() async {
    if (_closing) {
      return;
    }
    _closing = true;
    final client = _client;
    _client = null;
    await _cancelSubscriptions();
    if (client != null) {
      try {
        await client.close();
      } catch (_) {
        // EOF is the load-bearing stop; a missed acknowledgement is not a failed shutdown.
      }
    }
    connection.markStopped();
  }

  // ── tool commands ─────────────────────────────────────────────────────────────────────────

  /// `tool.start {tool}` ([PROTOCOL.md] §4). Returns false when the command was refused; the core's
  /// own line is then on the card ([PROTOCOL.md] §6). The card's phase still changes only when the
  /// core publishes `state`.
  /// `tool.start {tool}`. [runArgs] are the optional per-run arguments PROTOCOL §12.6 defines: `buy`
  /// takes `preset`/`count` and `sell` takes `slots`. When omitted the typed client call is used
  /// unchanged.
  Future<bool> startTool(ToolId tool, {JsonMap? runArgs}) {
    if (runArgs == null) {
      return _toolCommand(tool, (client) => client.toolStart(tool));
    }
    return _toolCommand(tool, (client) async {
      final response = await client.request('tool.start', {
        'tool': tool.wire,
        ...runArgs,
      });
      if (response is ErrorResponse) {
        throw CoreCommandException(response.error);
      }
    });
  }

  /// `tool.stop {tool}`, or none to stop whatever runs ([PROTOCOL.md] §4).
  Future<bool> stopTool([ToolId? tool]) =>
      _toolCommand(tool, (client) => client.toolStop(tool));

  // ── the Sell card's per-run selection (PROTOCOL §12.6) ────────────────────────────────────

  /// The bag cells the NEXT Sell run sells — IN MEMORY for this process only.
  ///
  /// The player's correction ("sell selection could be different every sell") made this a per-RUN
  /// argument rather than config: v2 kept it in memory, and persisting it (v3.0.1 did) risks selling
  /// the slots picked last time — data loss, not convenience. The model outlives the tabs, so the
  /// selection survives a tab switch; a restart starts empty. It is deliberately NOT persisted and a
  /// start never clears it: the player may run the same selection again.
  final Set<int> _sellSlots = <int>{};

  /// The selection as bag cells, ascending — what the Sell card puts on the wire. (The core dedupes
  /// and sorts highest-first itself; wire order is not data.)
  List<int> get sellSlots => _sellSlots.toList()..sort();

  /// Toggles one bag cell. An index outside 0..63 is not a bag cell and is ignored. No notification:
  /// the Buy/Sell tab is the only UI reader and it rebuilds from its own `setState`.
  void toggleSellSlot(int slot) {
    if (slot < 0 || slot >= _sellSlotCount) {
      return;
    }
    if (!_sellSlots.remove(slot)) {
      _sellSlots.add(slot);
    }
  }

  /// `ui.window` — the shell's own window geometry, written by the core ([PROTOCOL.md] §4).
  Future<bool> saveWindow(UiWindowArgs args) =>
      _voidCommand('ui.window', (client) => client.uiWindow(args));

  /// `window.find`: locate the configured game window. "Not found" is a RESULT, not an error; a
  /// config/Win32 refusal comes back typed and is rendered verbatim ([PROTOCOL.md] §4).
  Future<CoreActionResult<WindowFindResult>> findWindow() =>
      _coreAction('window.find', WindowFindResult.fromJson);

  /// `window.measure`: the `DisplayInfo` the shell maps frames with ([PROTOCOL.md] §3/§4).
  Future<CoreActionResult<DisplayInfo>> measureWindow() => _coreAction(
    'window.measure',
    (result) => DisplayInfo.fromJson(result['display'], 'display'),
  );

  /// `board.probe`: port and firmware facts. No board is `board_unavailable`, shown verbatim — the
  /// no-hardware path ([PROTOCOL.md] §4/§6).
  Future<CoreActionResult<BoardProbeResult>> probeBoard() =>
      _coreAction('board.probe', BoardProbeResult.fromJson);

  /// `board.details` — READ-ONLY: enumerate the serial ports and report what a Start would use,
  /// without opening the port ([PROTOCOL.md] §4). It stays answerable while a tool holds the game or
  /// the board, which is exactly when the Arduino tab's facts matter; the core owns the choice rule
  /// and its refusal sentence, and the received firmware level is the session's cache.
  Future<CoreActionResult<BoardDetailsResult>> boardDetails() =>
      _coreAction('board.details', BoardDetailsResult.fromJson);

  /// `attributes.get`: the dictionary the core already loaded for the active client. The payload is
  /// decoded app-locally (the Attributes surface consumes an untyped result; no protocol type exists).
  Future<CoreActionResult<AttributesResult>> attributesGet() =>
      _coreAction('attributes.get', AttributesResult.fromJson);

  /// `pet.preflight` — READ-ONLY: the Pet tab's Setup-so-far/Ready-to-run facts and v2's ready lines,
  /// computed by the core from the MERGED CONFIG ([PROTOCOL.md] §4). It opens no board, window or OCR,
  /// so it is not gated on the tool arbiter; the shell renders its lists/lines and invents no rules.
  Future<CoreActionResult<PetPreflightResult>> petPreflight() =>
      _coreAction('pet.preflight', PetPreflightResult.fromJson);

  /// `pet.set_boarding {row, running}` — ONE field through the core's field-precise writer
  /// ([PROTOCOL.md] §4): NOT a `config.apply`, and not part of the tab's draft, so a user tick and the
  /// run's own mid-run writes cannot clobber each other. `row` is 1-BASED. The core emits exactly one
  /// `config.changed {block:"pet"}` AFTER the response — that is what updates the mirror the tick
  /// renders from; a refusal is the core's line, rendered verbatim.
  Future<CoreActionResult<JsonMap>> setPetBoarding({
    required int row,
    required bool running,
  }) => _coreAction(
    'pet.set_boarding',
    (result) => result,
    args: {'row': row, 'running': running},
  );

  /// `pet.capture_icon {"kind":"queue"|"food","page":<1-based>,"cell":<1-based>}` — CAPTURE-ONLY:
  /// the core validates the input lease, the calibrated `pet.bag_grid` and the requested page's
  /// 2-point tab mark, then crops ONE bag cell at the run's own pitch ([PROTOCOL.md] §4). It neither
  /// clicks nor moves the cursor, so the requested page must already be up; the SHELL hides itself
  /// for the shot and shows again in a `finally` (the Pet tab reuses the Calibrate capture pattern).
  /// The answer is `{kind,page,cell,rect:[x,y,w,h],png_base64}`; refusals are the core's lines,
  /// rendered verbatim, and no config write happens here — only the tab's Save writes.
  Future<CoreActionResult<JsonMap>> capturePetIcon({
    required String kind,
    required int page,
    required int cell,
  }) => _coreAction(
    'pet.capture_icon',
    (result) => result,
    args: {'kind': kind, 'page': page, 'cell': cell},
  );

  /// `pet.scan_food_page {"page":<1-based>}` — READ-ONLY ([PROTOCOL.md] §4): input lease, one
  /// foreground-gated capture (the shell hides itself and puts the page up), the run's own matcher
  /// against the configured `pet.food_icon_png`, evidence under `<root>/logs/reads/`. The answer is
  /// `{page, cells:[1-BASED, CELL order], report:[lines], evidence:[paths]}`; refusals are the core's
  /// lines, verbatim, and with a missing icon `bad_request` names `food_icon_png`. No config write.
  Future<CoreActionResult<JsonMap>> scanPetFoodPage({required int page}) =>
      _coreAction(
        'pet.scan_food_page',
        (result) => result,
        args: {'page': page},
      );

  /// `pet.scan_queue_page {"page":<1-based>}` — the same READ-ONLY policy for the queued pet icons:
  /// `{page, per_entry:[{index (0-based), label, hits:[{cell (1-based), score, runner_up}]}],
  /// claimed:[{cell, labels}], deduced:[lines], evidence:[paths]}`. No queued icons is empty arrays,
  /// not an error; no config write.
  Future<CoreActionResult<JsonMap>> scanPetQueuePage({required int page}) =>
      _coreAction(
        'pet.scan_queue_page',
        (result) => result,
        args: {'page': page},
      );

  /// `pet.test_read_panel {"page":<1-based>,"cell":<1-based>,"delay_ms":<optional>}` — the
  /// READ-ONLY panel diagnostic ([PROTOCOL.md] §4, OPEN §6 G13): hovers the cell (never clicks),
  /// captures the calibrated tooltip region and returns `{region, lines, panel|null, numbers,
  /// evidence|null}`. The panel is the core parser's own `line`; the shell renders it verbatim and
  /// never re-derives growth/EXP. A refusal names the unset `tooltip`/`bag_grid` field or the busy
  /// holder's line; no config write.
  Future<CoreActionResult<JsonMap>> testReadPetPanel({
    required int page,
    required int cell,
    int? delayMs,
  }) => _coreAction(
    'pet.test_read_panel',
    (result) => result,
    args: {'page': page, 'cell': cell, 'delay_ms': ?delayMs},
  );

  /// `pet.apply_food_scan {"cells":[[1-based page, 1-based cell], ...]}` — THE explicit write the
  /// food scan feeds ([PROTOCOL.md] §4): one A1 field-precise patch that REPLACES `pet.food_slots`
  /// and resets `pet.food_slots_used`, then returns `{rev, block}` and emits exactly one
  /// `config.changed {block:"pet"}` AFTER the response. An EMPTY selection is refused
  /// (`bad_request` `cells`) so nothing-found cannot clear a live run's cells. A refusal is the
  /// core's line, verbatim; this is never a `config.apply` and the tab's Save stays the only draft
  /// writer.
  Future<CoreActionResult<JsonMap>> applyPetFoodScan(List<List<int>> cells) =>
      _coreAction(
        'pet.apply_food_scan',
        (result) => result,
        args: {'cells': cells},
      );

  /// `logs.cleanup_captures` — the Tuner tab's "Clean up capture images" housekeeping
  /// ([PROTOCOL.md] §2/§4). The result carries the deleted count and the ROOT-RELATIVE directory; the
  /// core logs per-file failures and still answers ok, so a refusal here is a real protocol problem
  /// and its line is rendered verbatim.
  ///
  /// Housekeeping, not a run: it opens no game or board and takes no input lease, so it is
  /// deliberately NOT gated on the tool arbiter — it works while a write tool or the feeder is busy.
  Future<CoreActionResult<JsonMap>> cleanupCaptures() =>
      _coreAction('logs.cleanup_captures', (result) => result);

  /// `board.test` — the calibrator's input tests. A command reaches the board only after the port
  /// opens; every refusal line is the core's ([PROTOCOL.md] §4/§6).
  Future<CoreActionResult<JsonMap>> boardTestClick() => _coreAction(
    'board.test',
    (result) => result,
    args: const {'kind': 'click'},
  );

  Future<CoreActionResult<JsonMap>> boardTestMove({
    required int dx,
    required int dy,
  }) => _coreAction(
    'board.test',
    (result) => result,
    args: {'kind': 'move', 'dx': dx, 'dy': dy},
  );

  Future<CoreActionResult<JsonMap>> boardTestKey(String key) => _coreAction(
    'board.test',
    (result) => result,
    args: {'kind': 'key', 'key': key},
  );

  /// `pointer.place {x,y}`: closed-loop cursor placement at a CLIENT-relative physical point — the
  /// Calibrate surfaces' "place the cursor here" action (calibration plan increment 0). The core runs
  /// the same window/foreground/board guards a tool does (a foreground tool holding the game is
  /// `tool_busy`), so a refusal is the core's own line, rendered verbatim like every action result.
  Future<CoreActionResult<JsonMap>> pointerPlace(int x, int y) =>
      _coreAction('pointer.place', (result) => result, args: {'x': x, 'y': y});

  /// `pointer.click {x,y}`: place, then click — v2's "Place cursor + click" / "Test Click".
  Future<CoreActionResult<JsonMap>> pointerClick(int x, int y) =>
      _coreAction('pointer.click', (result) => result, args: {'x': x, 'y': y});

  /// `capture`: the game client as PNG plus the `DisplayInfo` measurement. The full client is the
  /// calibration frame; a region is only for the capture command's general use. `allowCovered` is
  /// the calibration exemption: the shell hides itself first, but Windows can still report the
  /// hidden shell as the foreground window, so only the calibration screens ask to skip the gate.
  Future<CoreActionResult<CaptureResult>> captureFrame({
    Rect? region,
    bool allowCovered = false,
  }) {
    final args = <String, dynamic>{
      if (region != null) 'region': region.toJson(),
      if (allowCovered) 'allow_covered': true,
    };
    return _coreAction('capture', CaptureResult.fromJson, args: args);
  }

  /// `capture.discard`: remove one live screen from the core cache. This does NOT touch saved
  /// calibration marks/coordinates — only the in-memory frame and the shell-side assignment.
  Future<CoreActionResult<JsonMap>> discardFrame(String frameId) => _coreAction(
    'capture.discard',
    (result) => result,
    args: {'frame_id': frameId},
  );

  /// `calibration.save`: the mark drawn on the captured frame, plus the capture's measured display so
  /// the core records the environment. Refusals (bag-slot pitch disagreement, unknown frame_id, …)
  /// come back as the core's typed error line.
  Future<CoreActionResult<CalibrationSaveResult>> calibrationSave({
    required String frameId,
    required String kind,
    Rect? region,
    Point? point,
    bool? empty,
    String? target,
    DisplayInfo? display,
    int? rev,
  }) => _coreAction(
    'calibration.save',
    CalibrationSaveResult.fromJson,
    args: {
      'frame_id': frameId,
      'kind': kind,
      // Named-instance kinds carry the 1-based/player-facing instance name (PROTOCOL §12.8); a kind
      // without a target must omit the field entirely.
      'target': ?target,
      if (region != null) 'region': region.toJson(),
      if (point != null) 'point': point.toJson(),
      // v2's empty verdict for `result_gem_area` (PROTOCOL §12.7): true saves the crop and samples
      // the signature, false samples the signature only. Omitted for every other kind.
      'empty': ?empty,
      if (display != null) 'display': calibrationDisplayJson(display),
      // G06: optimistic concurrency. A stale rev is refused as `conflict` instead of quietly
      // overwriting another writer's mark.
      'rev': ?rev,
    },
  );

  /// `calibration.fetch_all {}`: every stored mark in ONE local read (G08). Returns null marks only
  /// when the core refuses (the workspace then falls back to per-slot fetches and shows the refusal).
  Future<CoreActionResult<CalibrationFetchAllResult>> calibrationFetchAll() =>
      _coreAction('calibration.fetch_all', CalibrationFetchAllResult.fromJson);

  /// `calibration.tooltip.hover_capture`: place + focus-click the hover point, wait the panel delay
  /// and capture, so the shell can show the panel and measure the offset box (G01).
  Future<CoreActionResult<JsonMap>> tooltipHoverCapture({
    required int x,
    required int y,
    int? hoverDelayMs,
    bool allowCovered = false,
  }) => _coreAction(
    'calibration.tooltip.hover_capture',
    (result) => result,
    args: {
      'point': {'x': x, 'y': y},
      'hover_delay_ms': ?hoverDelayMs,
      if (allowCovered) 'allow_covered': true,
    },
  );

  /// `calibration.tooltip.save`: the panel box measured from the hover capture, relative to the
  /// hover point. The core validates the size (>= 20 px) and writes the tooltip block.
  Future<CoreActionResult<JsonMap>> tooltipSave({
    required int offsetX,
    required int offsetY,
    required int width,
    required int height,
    int? hoverDelayMs,
  }) => _coreAction(
    'calibration.tooltip.save',
    (result) => result,
    args: {
      'offset_x': offsetX,
      'offset_y': offsetY,
      'width': width,
      'height': height,
      'hover_delay_ms': ?hoverDelayMs,
    },
  );

  /// `calibration.tooltip.test`: hover the point again, OCR the stored panel box and return the
  /// lines plus the debug crop (G01's live read).
  Future<CoreActionResult<JsonMap>> tooltipTest({
    required int x,
    required int y,
    int? hoverDelayMs,
    bool allowCovered = false,
  }) => _coreAction(
    'calibration.tooltip.test',
    (result) => result,
    args: {
      'point': {'x': x, 'y': y},
      'hover_delay_ms': ?hoverDelayMs,
      'debug': true,
      if (allowCovered) 'allow_covered': true,
    },
  );

  /// `calibration.pet.preview`: pure row/count/ETA derivation from the draft (G03).
  Future<CoreActionResult<JsonMap>> petPreview({required JsonMap draft}) =>
      _coreAction(
        'calibration.pet.preview',
        (result) => result,
        args: {'draft': draft},
      );

  /// `calibration.pet.save`: rows + points + bag + display on a captured frame; writes the
  /// calibration half, the crops and the annotated calib_pet.png (G03).
  Future<CoreActionResult<JsonMap>> petSave({
    required String frameId,
    required List<JsonMap> rows,
    required JsonMap points,
    required JsonMap bag,
    required DisplayInfo display,
  }) => _coreAction(
    'calibration.pet.save',
    (result) => result,
    args: {
      'frame_id': frameId,
      'rows': rows,
      'points': points,
      'bag': bag,
      'feeder_count_left_fraction': 0.4,
      'feeder_count_min_score': 0.75,
      'display': calibrationDisplayJson(display),
    },
  );

  /// `calibration.pet.test_read`: hover every draft row/slot once and report each count (G03).
  Future<CoreActionResult<JsonMap>> petTestRead({
    required List<JsonMap> rows,
  }) => _coreAction(
    'calibration.pet.test_read',
    (result) => result,
    args: {
      'rows': rows,
      'feeder_count_left_fraction': 0.4,
      'feeder_count_min_score': 0.75,
    },
  );

  /// `calibration.tuner.check`: the three boxes' derived geometry + one OCR/filter read (G02). Pure;
  /// no write. The boxes are client-relative physical rects.
  Future<CoreActionResult<JsonMap>> tunerCheck({
    required String frameId,
    required Map<String, Map<String, int>> boxes,
    int? rowHeight,
    int? springX,
    int? springY,
  }) => _coreAction(
    'calibration.tuner.check',
    (result) => result,
    args: {
      'frame_id': frameId,
      'boxes': boxes,
      'row_height': ?rowHeight,
      if (springX != null && springY != null)
        'spring_point': {'x': springX, 'y': springY},
    },
  );

  /// `calibration.tuner.save`: writes `tuner.ocr` (and `tuner.spring_point` when given), stamps the
  /// environment and writes the annotated `calib_tuner.png` (G02).
  Future<CoreActionResult<JsonMap>> tunerSave({
    required String frameId,
    required Map<String, Map<String, int>> boxes,
    required DisplayInfo display,
    int? rowHeight,
    int? springX,
    int? springY,
  }) => _coreAction(
    'calibration.tuner.save',
    (result) => result,
    args: {
      'frame_id': frameId,
      'boxes': boxes,
      'row_height': ?rowHeight,
      if (springX != null && springY != null)
        'spring_point': {'x': springX, 'y': springY},
      'display': calibrationDisplayJson(display),
    },
  );

  /// `calibration.check {}`: the core's canonical comparison of the stored calibration with this PC
  /// (G09). READ-ONLY: the game being closed is a result with a note, never an error.
  Future<CoreActionResult<JsonMap>> calibrationCheck() =>
      _coreAction('calibration.check', (result) => result);

  /// `calibration.fetch`: the stored mark (null = not calibrated yet) and, for artifact kinds, the
  /// stored crop as base64 PNG.
  Future<CoreActionResult<CalibrationFetchResult>> calibrationFetch(
    String kind, {
    String? target,
  }) => _coreAction(
    'calibration.fetch',
    CalibrationFetchResult.fromJson,
    args: {'kind': kind, 'target': ?target},
  );

  /// `keys.names`: display names for the stored VK codes (the domain's inverse rule, hex fallback
  /// included). The Hotkeys tab shows these; it invents no name of its own.
  Future<CoreActionResult<KeyNamesResult>> keyNames({
    required Map<String, int> codes,
  }) => _coreAction(
    'keys.names',
    KeyNamesResult.fromJson,
    args: {'codes': codes},
  );

  /// `keys.parse`: typed names → VK codes. The domain answers every parseable field and one line per
  /// bad box; the tab renders those lines verbatim and never parses a name itself.
  Future<CoreActionResult<KeyParseResult>> parseKeyNames({
    required Map<String, String> names,
  }) => _coreAction(
    'keys.parse',
    KeyParseResult.fromJson,
    args: {'names': names},
  );

  // ── config commands: validate, then apply ([PLAN-V3-SHELL.md] §5 / PROTOCOL §7) ───────────

  /// `config.validate`: the player-facing lines a save would refuse.
  Future<ConfigValidateResult?> validateConfig(ConfigSections sections) async {
    final client = _client;
    if (!isReady || client == null) {
      logs.addShellNote('cannot validate config: the core is not ready');
      return null;
    }
    try {
      final result = await client.configValidate(sections);
      config.recordValidation(result);
      return result;
    } on CoreCommandException catch (error) {
      config.recordError(error.error);
      logs.addShellNote(error.error.message);
      return null;
    } on ProtocolTimeoutException catch (error) {
      _noteTimeout(error);
      return null;
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('cannot validate config: ${error.message}');
      return null;
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return null;
    }
  }

  /// `config.apply {rev, sections}`. The revision echoed is the one the shell last saw
  /// ([PROTOCOL.md] §7); a `conflict` is recorded on [config] with the core's rev/sections and shown
  /// verbatim — the shell does not retry by itself.
  Future<ConfigApplyResult?> applyConfig(ConfigSections sections) async {
    final client = _client;
    if (!isReady || client == null) {
      logs.addShellNote('cannot save config: the core is not ready');
      return null;
    }
    final rev = config.rev;
    try {
      final result = await client.configApply(rev: rev, sections: sections);
      config.recordApply(result);
      return result;
    } on CoreCommandException catch (error) {
      config.recordError(error.error);
      logs.addShellNote(error.error.message);
      return null;
    } on ProtocolTimeoutException catch (error) {
      _noteTimeout(error);
      return null;
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('cannot save config: ${error.message}');
      return null;
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return null;
    }
  }

  /// `config.reload`: re-read the files; the whole merged config replaces the mirror.
  Future<ConfigReloadResult?> reloadConfig() async {
    final client = _client;
    if (!isReady || client == null) {
      logs.addShellNote('cannot reload config: the core is not ready');
      return null;
    }
    try {
      final result = await client.configReload();
      config.recordReload(result);
      return result;
    } on CoreCommandException catch (error) {
      config.recordError(error.error);
      logs.addShellNote(error.error.message);
      return null;
    } on ProtocolTimeoutException catch (error) {
      _noteTimeout(error);
      return null;
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('cannot reload config: ${error.message}');
      return null;
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return null;
    }
  }

  /// A tab's Save: the protocol order is `config.validate` FIRST, then `config.apply`
  /// ([PLAN-V3-SHELL.md] §5 / PROTOCOL §7). The tab never sends apply directly, and the core's
  /// validation lines are returned verbatim for the tab to render.
  ///
  /// The outcome is data, not an exception: a refused save is a rendered state on the tab.
  Future<ConfigSaveOutcome> saveConfig(ConfigSections sections) async {
    if (!isReady || _client == null) {
      const note = 'the core is not ready';
      logs.addShellNote('cannot save config: $note');
      return const ConfigSaveOutcome(note: note);
    }
    final validation = await validateConfig(sections);
    if (validation == null) {
      return ConfigSaveOutcome(
        error: config.lastError,
        note: config.lastError == null ? 'validation failed' : null,
      );
    }
    if (validation.errors.isNotEmpty) {
      return ConfigSaveOutcome(validation: validation);
    }
    final applied = await applyConfig(sections);
    if (applied == null) {
      return ConfigSaveOutcome(
        validation: validation,
        error: config.lastError,
        note: config.lastError == null ? 'the save failed' : null,
      );
    }
    return ConfigSaveOutcome(validation: validation, result: applied);
  }

  // ── plumbing ─────────────────────────────────────────────────────────────────────────────

  void _onEvent(ProtocolEvent event) {
    if (_disposed) {
      return;
    }
    switch (event) {
      case StateEvent():
      case ToolFinishedEvent():
        tools.applyEvent(event);
      case LogEvent(:final log):
        logs.addCoreLog(log);
      case ConfigChangedEvent(:final changed):
        config.applyChanged(changed);
      case CoreStoppingEvent(:final stopping):
        tools.applyEvent(event);
        logs.addShellNote('core stopping: ${stopping.reason.wire}');
      case UnknownEvent(:final name):
        logs.addShellNote('unrecognised event "$name"');
    }
  }

  void _onCoreOutputClosed(ProtocolClient client) {
    if (_disposed || !identical(_client, client)) {
      return;
    }
    final tail = List<String>.of(client.stderrTail);
    final exit = client.exitCode;
    final exitSuffix = exit == null ? '' : ' (exit $exit)';
    logs.captureStderrTail(tail);
    if (client.protocolFailure) {
      // The output could not be understood (an undecodable RESPONSE or a broken write path): report a
      // protocol failure, not a fake "the core exited"; the client has already terminated the process
      // so a live core is never left running unreferenced (SHELL-F1).
      _fail(
        ConnectionFailure.protocol,
        '${client.closeReason ?? 'the core output could not be decoded'}$exitSuffix',
        stderrTail: tail,
        exitCode: exit,
      );
    } else {
      _fail(
        ConnectionFailure.coreExit,
        tail.isEmpty
            ? 'the core exited$exitSuffix'
            : 'the core exited$exitSuffix — last line on stderr: ${tail.last}',
        stderrTail: tail,
        exitCode: exit,
      );
    }
    // The last rendered state is deliberately kept (PLAN §2): the crash view replaces the grid but a
    // restart starts from the same cards.
  }

  Future<bool> _toolCommand(
    ToolId? tool,
    Future<void> Function(ProtocolClient client) send,
  ) async {
    final client = _client;
    if (!isReady || client == null) {
      logs.addShellNote('the core is not ready');
      return false;
    }
    if (tool != null) {
      tools.beginCommand(tool);
      tools.clearRefusal(tool);
    }
    try {
      await send(client);
      return true;
    } on CoreCommandException catch (error) {
      if (tool != null) {
        tools.refuse(tool, error.error.message);
      }
      logs.addShellNote(error.error.message);
      return false;
    } on ProtocolTimeoutException catch (error) {
      final line =
          'the core did not answer ${error.cmd} within '
          '${error.timeout.inSeconds}s';
      if (tool != null) {
        tools.refuse(tool, line);
      }
      logs.addShellNote(line);
      return false;
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('the core is gone: ${error.message}');
      return false;
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return false;
    } finally {
      if (tool != null) {
        tools.endCommand(tool);
      }
    }
  }

  /// One read-only command with a typed result: the core's answer (or its refusal) is data for the
  /// Setup surface; nothing is thrown at the widget.
  Future<CoreActionResult<T>> _coreAction<T>(
    String cmd,
    T Function(JsonMap result) decode, {
    JsonMap args = const {},
  }) async {
    final client = _client;
    if (!isReady || client == null) {
      const note = 'the core is not ready';
      logs.addShellNote('cannot send $cmd: $note');
      return CoreActionResult<T>.failure(shellNote: note);
    }
    try {
      final response = await client.request(cmd, args);
      return switch (response) {
        OkResponse(:final result) => CoreActionResult<T>.success(
          decode(result),
        ),
        ErrorResponse(:final error) => CoreActionResult<T>.failure(
          coreError: error,
        ),
      };
    } on ProtocolTimeoutException catch (error) {
      _noteTimeout(error);
      return CoreActionResult<T>.failure(
        shellNote:
            'the core did not answer $cmd within ${error.timeout.inSeconds}s',
      );
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('cannot send $cmd: ${error.message}');
      return CoreActionResult<T>.failure(
        shellNote: 'the core is gone: ${error.message}',
      );
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return CoreActionResult<T>.failure(shellNote: error.message);
    } on ProtocolDecodeException catch (error) {
      logs.addShellNote('cannot read the $cmd result: ${error.message}');
      return CoreActionResult<T>.failure(
        shellNote: 'unexpected $cmd result: ${error.message}',
      );
    }
  }

  Future<bool> _voidCommand(
    String what,
    Future<void> Function(ProtocolClient client) send,
  ) async {
    final client = _client;
    if (!isReady || client == null) {
      logs.addShellNote('cannot send $what: the core is not ready');
      return false;
    }
    try {
      await send(client);
      return true;
    } on CoreCommandException catch (error) {
      logs.addShellNote(error.error.message);
      return false;
    } on ProtocolTimeoutException catch (error) {
      _noteTimeout(error);
      return false;
    } on ProtocolClosedException catch (error) {
      logs.addShellNote('cannot send $what: ${error.message}');
      return false;
    } on ProtocolStateException catch (error) {
      logs.addShellNote(error.message);
      return false;
    }
  }

  void _fail(
    ConnectionFailure failure,
    String message, {
    List<String> stderrTail = const [],
    int? exitCode,
  }) {
    if (_failureReported) {
      return;
    }
    _failureReported = true;
    final tail = stderrTail.isEmpty
        ? List<String>.of(_client?.stderrTail ?? const [])
        : stderrTail;
    logs.captureStderrTail(tail);
    connection.markFailed(
      failure: failure,
      message: message,
      stderrTail: tail,
      exitCode: exitCode,
    );
    logs.addShellNote(message);
  }

  String _describeHandshake(HandshakeException error) {
    final message = switch (error.failure) {
      HandshakeFailure.badVersion =>
        'protocol mismatch: the shell speaks ${error.shellProtocol}, '
            'the core expects ${error.coreExpected ?? '?'}',
      HandshakeFailure.protocolMismatch =>
        'protocol mismatch: the shell speaks ${error.shellProtocol}, '
            'the core reports ${error.coreReported ?? '?'}',
      HandshakeFailure.notReady =>
        'the core refused the handshake: '
            '${error.coreMessage ?? 'not ready'}',
    };
    final coreMessage = error.coreMessage;
    if (coreMessage == null || message.contains(coreMessage)) {
      return message;
    }
    return '$message — $coreMessage';
  }

  void _noteTimeout(ProtocolTimeoutException error) {
    logs.addShellNote(
      'the core did not answer ${error.cmd} within '
      '${error.timeout.inSeconds}s',
    );
  }

  /// Drops a client whose connection never became the shell's own (handshake failure, or a connect
  /// that finished while the shell was shutting down). Closing is the graceful EOF; the explicit
  /// terminate then guarantees the half-started process is gone — nothing holds a reference to it
  /// afterwards, so leaving it alive would be an orphan (SHELL-F1).
  Future<void> _detachClient(ProtocolClient client) async {
    if (identical(_client, client)) {
      _client = null;
    }
    await _cancelSubscriptions();
    try {
      await client.close();
    } catch (_) {
      // Already closed or the process is gone; nothing to recover.
    }
    try {
      await client.terminate(reason: 'the shell dropped the connection');
    } catch (_) {
      // Termination is idempotent; a process that refused to die cannot block the teardown.
    }
  }

  Future<void> _cancelSubscriptions() async {
    final events = _eventSubscription;
    final logLines = _logSubscription;
    _eventSubscription = null;
    _logSubscription = null;
    await events?.cancel();
    await logLines?.cancel();
  }

  // ── mini running card ────────────────────────────────────────────────────────────────────────

  /// True while the window is collapsed to the running tool's compact card.
  bool get miniMode => _miniActive;

  /// The tool the mini bar is following, or null when the full shell is shown.
  ToolId? get miniTool => _miniTool;

  /// The compact rect applied by the last collapse/move, for tests/diagnostics.
  WindowRect? get miniRect => _miniRect;

  /// The foreground tool a manual "Mini" action could collapse onto, or null.
  ToolId? get miniCandidate => _miniActive ? null : _foregroundTool();

  /// The tool a foreground run is showing, if any. Pet is excluded: the resident feeder runs
  /// alongside a foreground tool and the full window must stay usable for it. Hold Space and every
  /// foreground card do collapse.
  ToolId? _foregroundTool() {
    for (final tool in ToolId.values) {
      if (tool == ToolId.pet) continue;
      final phase = tools.snapshot(tool)?.phase ?? ToolPhase.idle;
      if (phase != ToolPhase.idle) {
        return tool;
      }
    }
    return null;
  }

  /// Collapse when a foreground tool starts. When the run ends the bar STAYS (Finished/Error) until
  /// the player presses its "Full window" action; it never pops the shell over the game on its own.
  /// [exitMiniMode]'s `suppress` keeps the full window for the rest of the current run.
  void _syncMiniMode() {
    if (!isReady || _closing || _disposed || _miniSyncing) {
      return;
    }
    final active = _foregroundTool();
    if (active == null) {
      // Optional after-run return: restore the window the player was in before Start. The default
      // keeps the bar until its Full window action; the switch is a persisted Setup preference.
      if (_miniActive && returnAfterRun && !_miniSuppressed) {
        final toDock = _miniReturnToDock;
        _miniReturnToDock = false;
        _miniSyncing = true;
        unawaited(
          (toDock ? openDockFromMini() : exitMiniMode()).whenComplete(
            () => _miniSyncing = false,
          ),
        );
        return;
      }
      // A new run may collapse again; if the bar is showing a finished run, it stays until expanded.
      if (!_miniActive) {
        _miniSuppressed = false;
      }
      return;
    }
    if (_miniActive) {
      if (_miniTool != active) {
        _miniTool = active;
        notifyListeners();
      }
      return;
    }
    if (_miniSuppressed) {
      return;
    }
    // The persistent Setup switch can keep the full window. The dock is already the compact window,
    // so it still hands over to the run bar (never two windows / two Stops).
    if (!_dockActive && !collapseOnRunStart) {
      return;
    }
    // The dock is the idle launcher: when something starts, the same compact window becomes the
    // run bar (it must never show two windows, or a duplicate Stop).
    if (_dockActive) {
      _miniSyncing = true;
      unawaited(_dockToMini(active).whenComplete(() => _miniSyncing = false));
      return;
    }
    _miniSyncing = true;
    unawaited(enterMiniMode(active).whenComplete(() => _miniSyncing = false));
  }

  /// The persisted Setup preference for automatic collapse on a run start
  /// (`settings.ui.collapse_on_run_start`, default true). Off keeps the full window until the
  /// player presses the app-bar Mini action; the dock still turns into the run bar because the dock
  /// itself is the compact window.
  bool get collapseOnRunStart => _uiBool('collapse_on_run_start', true);

  /// The persisted after-run preference (`settings.ui.return_after_run`, default false): when on,
  /// a finished/stopped run restores the window the player was in before Start (the quick dock if it
  /// started there, otherwise the full window). Off keeps the documented "bar stays until Full
  /// window" behaviour.
  bool get returnAfterRun => _uiBool('return_after_run', false);

  bool _uiBool(String key, bool fallback) {
    final settings = asJsonMap(config.config['settings']);
    final portable = asBool(asJsonMap(settings['ui'])[key]);
    if (portable != null) {
      return portable;
    }
    return asBool(asJsonMap(config.config['ui'])[key]) ?? fallback;
  }

  /// Persists a Setup window-behaviour switch through the normal validate → apply path.
  Future<ConfigSaveOutcome> setCollapseOnRunStart(bool enabled) =>
      _saveUiBool('collapse_on_run_start', enabled);

  /// Persists the after-run switch (see [returnAfterRun]).
  Future<ConfigSaveOutcome> setReturnAfterRun(bool enabled) =>
      _saveUiBool('return_after_run', enabled);

  Future<ConfigSaveOutcome> _saveUiBool(String key, bool enabled) =>
      saveConfig({
        'settings': {
          'ui': {key: enabled},
        },
      });

  /// The full window's "Mini" action: collapse onto the current foreground run, if there is one.
  Future<void> collapseToMini() async {
    final tool = _foregroundTool();
    if (tool != null) {
      await enterMiniMode(tool);
    }
  }

  /// Collapse the window to [tool]'s compact bar: remember the full placement, lower the runner's
  /// minimum size, switch to borderless non-activating chrome, move to the remembered/default edge
  /// position and pin the window on top.
  Future<void> enterMiniMode(ToolId tool) async {
    if (_miniActive || _disposed || tool == ToolId.pet) {
      return;
    }
    _miniActive = true;
    _miniTool = tool;
    _miniReturnToDock = false;
    notifyListeners();
    try {
      _miniRestoreRect = await host.windowRect();
      final work = await host.primaryWorkArea();
      _miniWorkArea = work;
      await host.setWindowMinSize(_miniWidth, _miniHeight);
      await host.setMiniChrome(true);
      final rect = _miniStartRect(work);
      _miniRect = rect;
      await host.setWindowRect(rect);
      await host.setAlwaysOnTop(true);
    } catch (error) {
      logs.addShellNote('cannot collapse to the mini bar: $error');
      await exitMiniMode();
    }
  }

  /// Restore the full window. [suppress] keeps the full window until the current run ends (the mini
  /// bar's "Full window" action); the bar's last position is remembered for the next collapse.
  Future<void> exitMiniMode({bool suppress = false}) async {
    if (suppress) {
      _miniSuppressed = true;
    }
    if (!_miniActive) {
      return;
    }
    final work = _miniWorkArea;
    if (work != null) {
      try {
        final current = await host.windowRect();
        _miniBarRect = _snappedRect(current, work);
      } catch (_) {
        // Keeping the previous remembered position is harmless.
      }
    }
    _miniActive = false;
    _miniTool = null;
    _miniReturnToDock = false;
    notifyListeners();
    final restore = _miniRestoreRect;
    _miniRestoreRect = null;
    try {
      await host.setMiniChrome(false);
      await host.setWindowMinSize(_fullMinimum.width, _fullMinimum.height);
      if (restore != null) {
        await host.setWindowRect(restore);
      }
      await host.setAlwaysOnTop(alwaysOnTop);
    } catch (error) {
      logs.addShellNote('cannot restore the full window: $error');
    }
    _miniRect = null;
  }

  /// The mini bar's "Full window" button: leave the bar for this run, without re-collapsing until
  /// the next start.
  Future<void> showFullWindow() => exitMiniMode(suppress: true);

  /// Moves the mini bar while the player drags it (clamped to the work area; the snap happens on
  /// release via [settleMiniWindow]).
  Future<void> moveMiniWindow(WindowRect target) async {
    if (!_miniActive) {
      return;
    }
    final work = _miniWorkArea;
    final rect = work == null
        ? target
        : WindowRect(
            left: target.left,
            top: target.top,
            width: _miniWidth,
            height: _miniHeight,
          ).clampedTo(work);
    if (rect == _miniRect) {
      return;
    }
    _miniRect = rect;
    try {
      await host.setWindowRect(rect);
    } catch (error) {
      logs.addShellNote('cannot move the mini bar: $error');
    }
  }

  /// Snaps the mini bar to its nearest work-area edge after a drag (16 px margin, 28 px threshold).
  Future<void> settleMiniWindow() async {
    final work = _miniWorkArea;
    if (!_miniActive || work == null) {
      return;
    }
    try {
      final current = await host.windowRect();
      final snapped = _snappedRect(current, work);
      _miniBarRect = snapped;
      if (snapped == _miniRect) {
        return;
      }
      _miniRect = snapped;
      await host.setWindowRect(snapped);
    } catch (error) {
      logs.addShellNote('cannot settle the mini bar: $error');
    }
  }

  // ── quick dock ───────────────────────────────────────────────────────────────────────────────

  /// True while the compact window is the idle launcher rather than the run bar.
  bool get dockMode => _dockActive;

  /// The player's chosen form factor / side (persisted with the layout below).
  DockPlacement get dockPlacement => _dockPlacement;

  /// The player's chosen quick-dock size preset (Compact / Standard / Large).
  DockSize get dockSize => _dockSize;

  /// True while the dock is the small resting form (edge tab / pill / island).
  bool get dockCollapsed => _dockCollapsed;

  /// Whether the idle dock auto-collapses after a pause or stays open until the player collapses it.
  DockBehavior get dockBehavior => _dockBehavior;

  /// Choose the size preset. Resizes an open dock immediately and is remembered for the next launch.
  Future<void> setDockSize(DockSize size) async {
    if (size == _dockSize) {
      return;
    }
    _dockSize = size;
    notifyListeners();
    await _resizeDockGeometry();
    await _saveDockPreference();
  }

  /// Choose the idle behaviour; "Keep open" also expands a collapsed dock immediately and is
  /// remembered for the next launch.
  Future<void> setDockBehavior(DockBehavior behavior) async {
    if (behavior == _dockBehavior) {
      return;
    }
    _dockBehavior = behavior;
    notifyListeners();
    if (behavior == DockBehavior.alwaysOpen && _dockCollapsed) {
      _dockCollapsed = false;
      notifyListeners();
      await _resizeDockGeometry();
    }
    await _saveDockPreference();
  }

  /// The app appearance, persisted as the portable `settings.ui.theme` value.
  ShellThemeMode get themeMode => _themeMode;

  /// Switch the whole shell (and the native title bar) between light, dark and the Windows setting.
  Future<void> setThemeMode(ShellThemeMode mode) async {
    if (mode == _themeMode) {
      return;
    }
    _themeMode = mode;
    notifyListeners();
    unawaited(_applyTitleBarTheme());
    final outcome = await saveConfig({
      'settings': {
        'ui': {'theme': mode.wire},
      },
    });
    if (!outcome.saved) {
      logs.addShellNote(
        'theme preference was not saved: ${outcome.errorLines.join(' · ')}',
      );
    }
  }

  /// Reads the portable `settings.ui.theme`; absent/invalid values keep the dark default.
  void _restoreThemePreference() {
    final settings = asJsonMap(config.config['settings']);
    var ui = asJsonMap(settings['ui']);
    if (ui['theme'] == null) {
      ui = asJsonMap(config.config['ui']);
    }
    final mode = ShellThemeMode.fromWire(asString(ui['theme']));
    if (mode != _themeMode) {
      _themeMode = mode;
      notifyListeners();
    }
    unawaited(_applyTitleBarTheme());
  }

  /// 0 follows the Windows setting, 1 forces light, 2 forces dark (the runner owns DWM).
  Future<void> _applyTitleBarTheme() async {
    final mode = switch (_themeMode) {
      ShellThemeMode.system => 0,
      ShellThemeMode.light => 1,
      ShellThemeMode.dark => 2,
    };
    try {
      await host.setTitleBarTheme(mode);
    } catch (error) {
      logs.addShellNote('cannot match the title bar to the theme: $error');
    }
  }

  /// Last-used Buy preset/count, kept for the dock session so reopening the dock after a run does
  /// not silently fall back to the configured defaults.
  String? get dockBuyPreset => _dockBuyPreset;
  int? get dockBuyCount => _dockBuyCount;

  void rememberDockBuyChoice({String? preset, int? count}) {
    _dockBuyPreset = preset;
    _dockBuyCount = count;
  }

  /// The dock's setup row asks the full shell to open this tool's tab.
  ValueNotifier<ToolId?> get setupRequest => _setupRequest;

  void requestSetup(ToolId tool) {
    _setupRequest.value = tool;
  }

  /// The dock's ordered quick-action ids (tool wires and `pin:<name>` entries).
  List<String> get dockTileIds => List.unmodifiable(_dockTiles);

  /// The pinned Sell selections available to the dock.
  List<DockPin> get dockPins => List.unmodifiable(_dockPins);

  /// Add/keep a pinned Sell selection as a quick action. Returns false for an empty/duplicate name.
  Future<bool> addDockPin(String name, List<int> slots) async {
    final clean = name.trim();
    if (clean.isEmpty || slots.isEmpty) {
      return false;
    }
    if (_dockPins.any((pin) => pin.name.toLowerCase() == clean.toLowerCase())) {
      return false;
    }
    final pin = DockPin(name: clean, slots: List<int>.of(slots)..sort());
    _dockPins = [..._dockPins, pin];
    _dockTiles = [..._dockTiles, pin.tileId];
    notifyListeners();
    await _saveDockPreference();
    return true;
  }

  /// Remove a tool or pinned action from the dock; removing a pin deletes the pin itself.
  Future<void> removeDockTile(String id) async {
    if (!_dockTiles.contains(id)) {
      return;
    }
    _dockTiles = [..._dockTiles]..remove(id);
    if (id.startsWith('pin:')) {
      final name = id.substring(4);
      _dockPins = [
        for (final pin in _dockPins)
          if (pin.name != name) pin,
      ];
    }
    notifyListeners();
    await _saveDockPreference();
  }

  /// Move a quick action one slot up (-1) or down (+1).
  Future<void> moveDockTile(String id, int delta) async {
    final from = _dockTiles.indexOf(id);
    if (from < 0) {
      return;
    }
    final to = from + delta;
    if (to < 0 || to >= _dockTiles.length) {
      return;
    }
    final next = [..._dockTiles];
    final item = next.removeAt(from);
    next.insert(to, item);
    _dockTiles = next;
    notifyListeners();
    await _saveDockPreference();
  }

  /// Re-add a tool that was removed from the dock (appended at the end).
  Future<void> addDockTool(ToolId tool) async {
    if (tool == ToolId.pet || _dockTiles.contains(tool.wire)) {
      return;
    }
    _dockTiles = [..._dockTiles, tool.wire];
    notifyListeners();
    await _saveDockPreference();
  }

  bool _isDockTool(String id) =>
      ToolId.values.any((tool) => tool != ToolId.pet && tool.wire == id);

  void _restoreDockLayout(Map<String, dynamic> dock) {
    final pins = <DockPin>[];
    final names = <String>{};
    for (final raw in asJsonList(dock['pins'])) {
      final pin = DockPin.fromJson(raw);
      if (pin != null && names.add(pin.name)) {
        pins.add(pin);
      }
    }
    _dockPins = pins;
    final rawTiles = dock['tiles'];
    if (rawTiles is! List) {
      _dockTiles = [..._defaultDockTiles];
      return;
    }
    final tiles = <String>[];
    for (final raw in rawTiles) {
      final id = asString(raw);
      if (id == null || tiles.contains(id)) {
        continue;
      }
      if (_isDockTool(id)) {
        tiles.add(id);
      } else if (id.startsWith('pin:') && pins.any((pin) => pin.tileId == id)) {
        tiles.add(id);
      }
    }
    _dockTiles = tiles;
  }

  /// Reads the portable `settings.ui.quick_dock` preference; absent/invalid values keep the defaults
  /// (right rail, expanded). The merged top-level `ui` is read too, for cores that flatten the overlay.
  void _restoreDockPreference() {
    final settings = asJsonMap(config.config['settings']);
    var dock = asJsonMap(asJsonMap(settings['ui'])['quick_dock']);
    if (dock.isEmpty) {
      dock = asJsonMap(asJsonMap(config.config['ui'])['quick_dock']);
    }
    final wire = asString(dock['placement']);
    _dockPlacement = DockPlacement.values.firstWhere(
      (placement) => placement.wire == wire,
      orElse: () => DockPlacement.rightRail,
    );
    final sizeWire = asString(dock['size']);
    _dockSize = DockSize.values.firstWhere(
      (size) => size.wire == sizeWire,
      orElse: () => DockSize.standard,
    );
    _dockCollapsed = asBool(dock['collapsed']) ?? false;
    final behaviorWire = asString(dock['behavior']);
    _dockBehavior = DockBehavior.values.firstWhere(
      (behavior) => behavior.wire == behaviorWire,
      orElse: () => DockBehavior.alwaysOpen,
    );
    _restoreDockLayout(dock);
  }

  /// Persists the placement/collapsed/layout through the normal validate → apply path. A refusal is
  /// logged but does not fight the player's choice in the running window; the next connect re-reads
  /// the core's mirror.
  Future<void> _saveDockPreference() async {
    if (!isReady || _closing || _disposed) {
      return;
    }
    final outcome = await saveConfig({
      'settings': {
        'ui': {
          'quick_dock': {
            'placement': _dockPlacement.wire,
            'size': _dockSize.wire,
            'collapsed': _dockCollapsed,
            'behavior': _dockBehavior.wire,
            'tiles': _dockTiles,
            'pins': [for (final pin in _dockPins) pin.toJson()],
          },
        },
      },
    });
    if (!outcome.saved) {
      logs.addShellNote(
        'quick dock preference was not saved: '
        '${outcome.errorLines.join(' · ')}',
      );
    }
  }

  /// The dock's current rect, for tests/diagnostics.
  WindowRect? get dockRect => _dockRectValue;

  (int, int) get _dockWindowSize => _dockCollapsed
      ? _dockPlacement.collapsedSizeFor(_dockSize)
      : _dockPlacement.sizeFor(_dockSize);

  /// Open the idle launcher: remember the full placement, lower the minimum size to the dock,
  /// switch to borderless non-activating chrome and place it at [dockPlacement].
  Future<void> enterDock() async {
    if (_dockActive || _miniActive || !isReady || _closing || _disposed) {
      return;
    }
    // The dock is the idle launcher; a foreground run owns the compact window (the run bar).
    if (_foregroundTool() != null) {
      return;
    }
    // "Keep open" means the launcher comes back expanded even if it was manually collapsed earlier.
    if (_dockBehavior == DockBehavior.alwaysOpen) {
      _dockCollapsed = false;
    }
    _dockActive = true;
    notifyListeners();
    try {
      _dockRestoreRect = await host.windowRect();
      final work = await host.primaryWorkArea();
      _dockWorkArea = work;
      final (width, height) = _dockWindowSize;
      await host.setWindowMinSize(width, height);
      await host.setMiniChrome(true);
      final rect = _dockRectFor(work);
      _dockRectValue = rect;
      await host.setWindowRect(rect);
      await host.setAlwaysOnTop(true);
    } catch (error) {
      logs.addShellNote('cannot open the quick dock: $error');
      await exitDock();
    }
  }

  /// Leave the dock for the full window, restoring the remembered full rect, chrome and pin.
  Future<void> exitDock() async {
    if (!_dockActive) {
      return;
    }
    _dockActive = false;
    notifyListeners();
    final restore = _dockRestoreRect;
    _dockRestoreRect = null;
    try {
      await host.setMiniChrome(false);
      await host.setWindowMinSize(_fullMinimum.width, _fullMinimum.height);
      if (restore != null) {
        await host.setWindowRect(restore);
      }
      await host.setAlwaysOnTop(alwaysOnTop);
    } catch (error) {
      logs.addShellNote('cannot restore the full window from the dock: $error');
    }
    _dockRectValue = null;
  }

  /// Change the dock's form factor/side. Safe while the dock is closed (the next [enterDock] uses
  /// it) and while it is open (the window moves and resizes immediately). The choice is written to
  /// the portable `settings.ui.quick_dock` block through the normal save path.
  Future<void> setDockPlacement(DockPlacement placement) async {
    if (placement == _dockPlacement) {
      return;
    }
    _dockPlacement = placement;
    notifyListeners();
    final work = _dockWorkArea;
    if (_dockActive && work != null) {
      final (width, height) = _dockWindowSize;
      final rect = _dockRectFor(work);
      try {
        await host.setWindowMinSize(width, height);
        _dockRectValue = rect;
        await host.setWindowRect(rect);
      } catch (error) {
        logs.addShellNote('cannot move the quick dock: $error');
      }
    }
    await _saveDockPreference();
  }

  /// Collapse the dock to its resting edge tab/pill/island, or expand it again. The same full-window
  /// restore target is kept, so collapsing never loses the full placement.
  ///
  /// [persist] is false for the transient auto-collapse and hover expansions: they resize the window
  /// but do not rewrite the player's explicit collapsed preference.
  Future<void> setDockCollapsed(bool collapsed, {bool persist = true}) async {
    if (collapsed == _dockCollapsed) {
      return;
    }
    _dockCollapsed = collapsed;
    notifyListeners();
    await _resizeDockGeometry();
    if (persist) {
      await _saveDockPreference();
    }
  }

  Future<void> _resizeDockGeometry() async {
    if (!_dockActive) {
      return;
    }
    final work = _dockWorkArea;
    if (work == null) {
      return;
    }
    final (width, height) = _dockWindowSize;
    final rect = _dockRectFor(work);
    try {
      await host.setWindowMinSize(width, height);
      _dockRectValue = rect;
      await host.setWindowRect(rect);
    } catch (error) {
      logs.addShellNote('cannot resize the quick dock: $error');
    }
  }

  Future<void> toggleDockCollapsed() => setDockCollapsed(!_dockCollapsed);

  /// Moves the dock while the player drags its header. The content is fixed-size, so only the
  /// top-left corner changes; the snap happens on release.
  Future<void> moveDockWindow(WindowRect target) async {
    if (!_dockActive) {
      return;
    }
    final work = _dockWorkArea;
    final (width, height) = _dockWindowSize;
    final rect = _clampDock(target.left, target.top, width, height, work);
    if (rect == _dockRectValue) {
      return;
    }
    _dockRectValue = rect;
    try {
      await host.setWindowRect(rect);
    } catch (error) {
      logs.addShellNote('cannot move the quick dock: $error');
    }
  }

  /// Snaps the dragged dock to its nearest edge, like the run bar.
  Future<void> settleDockWindow() async {
    final work = _dockWorkArea;
    if (!_dockActive || work == null) {
      return;
    }
    final current = _dockRectValue;
    if (current == null) {
      return;
    }
    final snapped = _snappedRect(current, work);
    _dockRectValue = snapped;
    try {
      await host.setWindowRect(snapped);
    } catch (error) {
      logs.addShellNote('cannot settle the quick dock: $error');
    }
  }

  /// The dock's "Open full window" action.
  Future<void> showFullWindowFromDock() => exitDock();

  /// The finished run bar's "Dock" action: hand the compact window back to the launcher.
  Future<void> openDockFromMini() async {
    if (!_miniActive) {
      await enterDock();
      return;
    }
    if (_disposed) {
      return;
    }
    _dockRestoreRect = _miniRestoreRect;
    _miniRestoreRect = null;
    _miniActive = false;
    _miniTool = null;
    _miniReturnToDock = false;
    // Match enterDock: Keep open must also come back expanded from the finished run bar.
    if (_dockBehavior == DockBehavior.alwaysOpen) {
      _dockCollapsed = false;
    }
    _dockActive = true;
    _dockWorkArea ??= _miniWorkArea;
    notifyListeners();
    try {
      final work = _dockWorkArea ?? await host.primaryWorkArea();
      _dockWorkArea = work;
      final (width, height) = _dockWindowSize;
      await host.setWindowMinSize(width, height);
      final rect = _dockRectFor(work);
      _dockRectValue = rect;
      await host.setWindowRect(rect);
      await host.setAlwaysOnTop(true);
    } catch (error) {
      logs.addShellNote('cannot open the quick dock from the run bar: $error');
      await exitDock();
    }
  }

  /// Dock → run bar when a foreground tool starts. The full-window restore target is carried over, so
  /// "Full window" from the run bar still returns exactly where the shell came from.
  Future<void> _dockToMini(ToolId tool) async {
    if (!_dockActive) {
      await enterMiniMode(tool);
      return;
    }
    _miniRestoreRect = _dockRestoreRect ?? _miniRestoreRect;
    _dockRestoreRect = null;
    _dockActive = false;
    _miniActive = true;
    _miniTool = tool;
    _miniReturnToDock = true;
    _miniWorkArea ??= _dockWorkArea;
    notifyListeners();
    try {
      await host.setWindowMinSize(_miniWidth, _miniHeight);
      final work = _miniWorkArea;
      final rect = work == null
          ? (_miniRect ?? _clampDock(16, 16, _miniWidth, _miniHeight, null))
          : _miniStartRect(work);
      _miniRect = rect;
      await host.setWindowRect(rect);
      await host.setAlwaysOnTop(true);
    } catch (error) {
      logs.addShellNote('cannot collapse the dock to the run bar: $error');
      await exitMiniMode();
    }
  }

  WindowRect _dockRectFor(WindowRect work) {
    final (width, height) = _dockWindowSize;
    const margin = 16;
    final int left;
    final int top;
    switch (_dockPlacement) {
      case DockPlacement.rightRail:
        left = work.left + work.width - width - margin;
        top = work.top + (work.height - height) ~/ 2;
      case DockPlacement.leftRail:
        left = work.left + margin;
        top = work.top + (work.height - height) ~/ 2;
      case DockPlacement.bottomRight:
        left = work.left + work.width - width - margin;
        top = work.top + work.height - height - margin;
      case DockPlacement.bottomLeft:
        left = work.left + margin;
        top = work.top + work.height - height - margin;
      case DockPlacement.topCentre:
        left = work.left + (work.width - width) ~/ 2;
        top = work.top + margin;
    }
    return _clampDock(left, top, width, height, work);
  }

  WindowRect _clampDock(
    int left,
    int top,
    int width,
    int height,
    WindowRect? work,
  ) {
    final area =
        work ?? const WindowRect(left: 0, top: 0, width: 1920, height: 1080);
    return WindowRect(
      left: _clampInt(left, area.left, area.left + area.width - width),
      top: _clampInt(top, area.top, area.top + area.height - height),
      width: width,
      height: height,
    );
  }

  /// The bar's initial rect: the last dragged position when it still fits, else the default dock —
  /// right edge, 16 px margin, about a fifth down (below a typical minimap).
  WindowRect _miniStartRect(WindowRect work) {
    final saved = _miniBarRect;
    if (saved != null) {
      return _snappedRect(saved.clampedTo(work), work);
    }
    final left = work.left + work.width - _miniWidth - 16;
    final top = work.top + (work.height * 18 ~/ 100);
    return _snappedRect(
      WindowRect(
        left: left,
        top: top,
        width: _miniWidth,
        height: _miniHeight,
      ).clampedTo(work),
      work,
    );
  }

  /// Clamps [rect] into [work] and snaps it to an edge when it is within 28 px of one.
  WindowRect _snappedRect(WindowRect rect, WindowRect work) {
    const margin = 16;
    const threshold = 28;
    var left = rect.left;
    var top = rect.top;
    final rightGap = work.left + work.width - (left + rect.width);
    final leftGap = left - work.left;
    if (rightGap <= threshold) {
      left = work.left + work.width - rect.width - margin;
    } else if (leftGap <= threshold) {
      left = work.left + margin;
    }
    final bottomGap = work.top + work.height - (top + rect.height);
    final topGap = top - work.top;
    if (bottomGap <= threshold) {
      top = work.top + work.height - rect.height - margin;
    } else if (topGap <= threshold) {
      top = work.top + margin;
    }
    return WindowRect(
      left: _clampInt(left, work.left, work.left + work.width - rect.width),
      top: _clampInt(top, work.top, work.top + work.height - rect.height),
      width: rect.width,
      height: rect.height,
    );
  }

  static int _clampInt(int value, int lower, int upper) {
    if (upper < lower) return lower;
    if (value < lower) return lower;
    if (value > upper) return upper;
    return value;
  }

  void _onSubmodelChanged() {
    if (_disposed) {
      return;
    }
    _syncMiniMode();
    notifyListeners();
  }

  /// Tools/config can change the mini/dock transition, but a status event must not rebuild the
  /// whole shell (SHELL-F5). Notify only when the mode flags actually changed.
  void _onRunAffectingChanged() {
    if (_disposed) {
      return;
    }
    final mini = _miniActive;
    final dock = _dockActive;
    _syncMiniMode();
    if (mini != _miniActive || dock != _dockActive) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _windowSaveTimer?.cancel();
    unawaited(_windowMoveSubscription?.cancel());
    connection.removeListener(_onSubmodelChanged);
    tools.removeListener(_onRunAffectingChanged);
    config.removeListener(_onSubmodelChanged);
    unawaited(_eventSubscription?.cancel());
    unawaited(_logSubscription?.cancel());
    _setupRequest.dispose();
    super.dispose();
  }
}

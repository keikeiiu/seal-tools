/// The tools sub-model: the eight cards' core-published facts and the shell-side command bookkeeping.
///
/// Everything shown about a tool's RUN comes from a `state` event's [ToolSnapshot] or a
/// `tool.finished` event ([PROTOCOL.md] §5) — the card never infers a phase. Two kinds of state here
/// are shell-side convenience and are marked as such:
///
/// - [pending] tool ids: a command is in flight; the card disables its buttons. The core has not
///   answered yet, so this is NOT state.
/// - [refusal]: the core's own line for the last refused start/stop of that tool ([PROTOCOL.md] §6,
///   `tool_busy` etc.). The line is stored verbatim — the shell never rewrites it.
library;

import 'package:flutter/foundation.dart';

import '../protocol/protocol.dart';

class ToolsModel extends ChangeNotifier {
  final Map<ToolId, ToolSnapshot> _snapshots = {};
  final Map<ToolId, ToolFinished> _finished = {};
  final Map<ToolId, String> _refusals = {};
  final Set<ToolId> _pending = {};
  ToolFinished? _lastFinished;
  CoreStopping? _stopping;

  /// The eight tool ids in protocol order ([PROTOCOL.md] §9) — the card grid's order.
  List<ToolId> get toolIds => ToolId.values;

  /// The core's last published snapshot for [tool], or null before its first `state` event.
  ToolSnapshot? snapshot(ToolId tool) => _snapshots[tool];

  /// The last `tool.finished` for [tool], or null.
  ToolFinished? finished(ToolId tool) => _finished[tool];

  /// The most recent finish across tools, or null.
  ToolFinished? get lastFinished => _lastFinished;

  /// The core's line for the last refused command on [tool], verbatim; null when nothing was refused.
  String? refusal(ToolId tool) => _refusals[tool];

  /// The shell-convenience "command in flight" flag for [tool].
  bool isPending(ToolId tool) => _pending.contains(tool);

  /// Every tool with a published snapshot (so a test can tell "no state yet" from "idle").
  Map<ToolId, ToolSnapshot> get snapshots => Map.unmodifiable(_snapshots);

  /// The core's last `core.stopping` reason, if it announced one before exiting.
  CoreStopping? get stopping => _stopping;

  /// Applies one core event. Returns whether anything the cards render changed.
  bool applyEvent(ProtocolEvent event) {
    switch (event) {
      case StateEvent(:final snapshot):
        _snapshots[snapshot.tool] = snapshot;
        // A tool that has just published is no longer waiting on our last refusal; clearing here is a
        // UI convenience (the refusal line was about the previous attempt).
        _refusals.remove(snapshot.tool);
        notifyListeners();
        return true;
      case ToolFinishedEvent(:final finished):
        _finished[finished.tool] = finished;
        _lastFinished = finished;
        notifyListeners();
        return true;
      case LogEvent():
      case ConfigChangedEvent():
      case UnknownEvent():
        return false;
      case CoreStoppingEvent(:final stopping):
        _stopping = stopping;
        notifyListeners();
        return true;
    }
  }

  // ── mutations: ShellModel only ────────────────────────────────────────────────────────────

  /// Records that a start/stop for [tool] is in flight (shell convenience).
  void beginCommand(ToolId tool) {
    _pending.add(tool);
    notifyListeners();
  }

  /// The command finished (whatever its outcome).
  void endCommand(ToolId tool) {
    _pending.remove(tool);
    notifyListeners();
  }

  /// Stores the core's refusal line for [tool] verbatim.
  void refuse(ToolId tool, String line) {
    _refusals[tool] = line;
    notifyListeners();
  }

  /// Forgets the refusal line — when a new attempt starts.
  void clearRefusal(ToolId tool) {
    if (_refusals.remove(tool) != null) {
      notifyListeners();
    }
  }
}

/// Typed argument objects for the commands the shell sends today ([PROTOCOL.md] §4).
///
/// The `args` map stays on [Request] for the wire; these types are what the client's convenience
/// methods take, so a wrong argument is a compile error instead of a silently ignored key. Commands
/// the shell does not send yet (the board, capture, calibration and OCR surfaces) have no typed
/// args here — their request fixtures still decode through [Request].
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'envelope.dart';
import 'json.dart';
import 'types.dart';

/// One term of a config merge request: block name to opaque values. The shell copies blocks it does
/// not own, so a value here is [JsonMap] rather than a model.
typedef ConfigSections = JsonMap;

/// `hello`'s arguments ([PROTOCOL.md] §4): the protocol the shell speaks and its own version.
class HelloArgs implements WirePayload {
  const HelloArgs({required this.protocol, required this.client});

  final int protocol;
  final String client;

  @override
  JsonMap toJson() => {'protocol': protocol, 'client': client};
}

/// `config.validate`'s arguments: the blocks a Save would send.
class ConfigValidateArgs implements WirePayload {
  const ConfigValidateArgs({required this.sections});

  final ConfigSections sections;

  @override
  JsonMap toJson() => {'sections': sections};
}

/// `config.apply`'s arguments: the revision the shell last saw, plus the blocks it owns.
///
/// `sections` may include the portable half as `"settings"` ([PROTOCOL.md] §4's ruling); the shell
/// does not distinguish it here — the core decides what is portable.
class ConfigApplyArgs implements WirePayload {
  const ConfigApplyArgs({required this.rev, required this.sections});

  final int rev;
  final ConfigSections sections;

  @override
  JsonMap toJson() => {'rev': rev, 'sections': sections};
}

/// `config.reload`'s arguments: none.
class ConfigReloadArgs implements WirePayload {
  const ConfigReloadArgs();

  @override
  JsonMap toJson() => const {};
}

/// `tool.start`'s arguments.
class ToolStartArgs implements WirePayload {
  const ToolStartArgs({required this.tool, this.test, this.dryRun});

  final ToolId tool;

  /// QUES T only: run the same validated plan for exactly one master loop (G21).
  final bool? test;

  /// BUY/SELL only: validate the plan and move the cursor, but never click (G20).
  final bool? dryRun;

  @override
  JsonMap toJson() => {
    'tool': tool.wire,
    if (test != null) 'test': test,
    if (dryRun != null) 'dry_run': dryRun,
  };
}

/// `tool.stop`'s arguments: a tool, or none to stop whatever runs ([PROTOCOL.md] §4).
class ToolStopArgs implements WirePayload {
  const ToolStopArgs({this.tool});

  final ToolId? tool;

  @override
  JsonMap toJson() => tool == null ? const {} : {'tool': tool!.wire};
}

/// `ui.window`'s arguments ([PROTOCOL.md] §4). The numbers pass through untyped on purpose: the
/// protocol does not pin their space (they are the shell's own window, not game coordinates), and
/// the core stores them as v2's `ui:` block fields.
class UiWindowArgs implements WirePayload {
  const UiWindowArgs({
    required this.left,
    required this.top,
    required this.width,
    required this.height,
    required this.pinned,
  });

  final num left;
  final num top;
  final num width;
  final num height;
  final bool pinned;

  @override
  JsonMap toJson() => {
    'left': left,
    'top': top,
    'width': width,
    'height': height,
    'pinned': pinned,
  };
}

/// `capture`'s arguments: the physical region to grab ([PROTOCOL.md] §4).
class CaptureArgs implements WirePayload {
  const CaptureArgs({required this.region});

  final Rect region;

  @override
  JsonMap toJson() => {'region': region.toJson()};
}

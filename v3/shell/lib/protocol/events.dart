/// The event half of the protocol ([PROTOCOL.md] §5), sealed so a handler must consider every kind.
///
/// An event is NOT a response: it has no id, it is never awaited, and it arrives on its own stream
/// (see `client.dart`). The sealed hierarchy is what keeps that distinction honest at the call site.
///
/// Unknown event names decode to [UnknownEvent] rather than throwing — a newer core may add an event
/// the old shell does not know, and the shell should log it, not die.
///
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'json.dart';
import 'types.dart';

/// One unsolicited event from the core.
sealed class ProtocolEvent {
  const ProtocolEvent(this.name);

  /// The wire name, exactly as `PROTOCOL.md` §5 spells it.
  final String name;

  /// The event's `data` object, as JSON — the drift test compares it against the fixtures.
  JsonMap get data;
}

/// `state`: the whole card surface, pushed whenever the published snapshot changes.
final class StateEvent extends ProtocolEvent {
  const StateEvent(this.snapshot) : super('state');

  final ToolSnapshot snapshot;

  @override
  JsonMap get data => snapshot.toJson();
}

/// `tool.finished`: a run ended; `message` is the line the card shows.
final class ToolFinishedEvent extends ProtocolEvent {
  const ToolFinishedEvent(this.finished) : super('tool.finished');

  final ToolFinished finished;

  @override
  JsonMap get data => finished.toJson();
}

/// `log`: a line the shell may show in its status/log surface.
final class LogEvent extends ProtocolEvent {
  const LogEvent(this.log) : super('log');

  final LogLine log;

  @override
  JsonMap get data => log.toJson();
}

/// `config.changed`: the core wrote a block the UI displays. Specified; the core has no emitter yet.
final class ConfigChangedEvent extends ProtocolEvent {
  const ConfigChangedEvent(this.changed) : super('config.changed');

  final ConfigChanged changed;

  @override
  JsonMap get data => changed.toJson();
}

/// `core.stopping`: the last event before the process exits.
final class CoreStoppingEvent extends ProtocolEvent {
  const CoreStoppingEvent(this.stopping) : super('core.stopping');

  final CoreStopping stopping;

  @override
  JsonMap get data => stopping.toJson();
}

/// An event name this shell does not know. Kept so a newer core's addition is visible, not fatal.
final class UnknownEvent extends ProtocolEvent {
  const UnknownEvent(super.name, this._data);

  final JsonMap _data;

  @override
  JsonMap get data => _data;
}

/// Decodes the `event`/`data` pair of an envelope ([PROTOCOL.md] §2/§5).
///
/// Throws [ProtocolDecodeException] when a KNOWN event's payload is malformed; an unknown event name
/// returns [UnknownEvent] instead.
ProtocolEvent decodeEvent(JsonMap message) {
  final name = decodeString(message['event'], 'event');
  final data = decodeNullableMap(message['data'], 'data') ?? const {};
  return switch (name) {
    'state' => StateEvent(ToolSnapshot.fromJson(data)),
    'tool.finished' => ToolFinishedEvent(ToolFinished.fromJson(data)),
    'log' => LogEvent(LogLine.fromJson(data)),
    'config.changed' => ConfigChangedEvent(ConfigChanged.fromJson(data)),
    'core.stopping' => CoreStoppingEvent(CoreStopping.fromJson(data)),
    _ => UnknownEvent(name, data),
  };
}

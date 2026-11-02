/// Labels and formatting for the card grid — pure Dart (no widgets, except the icon lookup), so this
/// file's rules are testable and the card itself stays a rendering of them.
///
/// The card ORDER is PLAN-V3-SHELL §6's seven tools, with **Hold Space eighth** ([PLAN-V3-UI-UX.md]
/// D2: it is its own card now, at the same level as the other tools, and has no config screen).
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
/// [PLAN-V3-UI-UX.md]: ../../../docs/plans/PLAN-V3-UI-UX.md
/// [PROTOCOL.md]: ../../../docs/spec/PROTOCOL.md
library;

import 'package:flutter/material.dart';

import '../protocol/protocol.dart';

/// The eight cards, in the plan's order (Hold Space last).
const List<ToolId> cardToolIds = [
  ToolId.spammer,
  ToolId.tuner,
  ToolId.buy,
  ToolId.sell,
  ToolId.gem,
  ToolId.quest,
  ToolId.pet,
  ToolId.holdspace,
];

/// The player-facing name of a tool. The core names ids, the shell names cards.
String toolDisplayName(ToolId tool) => switch (tool) {
  ToolId.spammer => 'Skill Spammer',
  ToolId.holdspace => 'Hold Space',
  ToolId.buy => 'Buy',
  ToolId.sell => 'Sell',
  ToolId.tuner => 'Magic Tuner',
  ToolId.gem => 'Gem Composer',
  ToolId.quest => 'Quest Hand-in',
  ToolId.pet => 'Pet Feeder',
};

/// The icon a card/rail row shows for a tool (presentation only; the core names ids).
IconData toolIcon(ToolId tool) => switch (tool) {
  ToolId.spammer => Icons.bolt_outlined,
  ToolId.holdspace => Icons.space_bar,
  ToolId.buy => Icons.shopping_cart_outlined,
  ToolId.sell => Icons.sell_outlined,
  ToolId.tuner => Icons.tune,
  ToolId.gem => Icons.diamond_outlined,
  ToolId.quest => Icons.assignment_turned_in_outlined,
  ToolId.pet => Icons.pets_outlined,
};

/// The lifecycle word the card shows; a tool with no published snapshot is idle (PLAN §6's
/// "not known yet" does not exist for phases — rows are the only "not known" case, PROTOCOL §5).
String phaseWord(ToolSnapshot? snapshot) =>
    snapshot?.phase.wire ?? ToolPhase.idle.wire;

/// Renders the `next_action_in_seconds` value the core pinned (PROTOCOL §5): seconds from the
/// publish until the row needs attention. The shell shows the published value; a live tick is a
/// later UI convenience, not a reinterpretation.
String formatNextAction(double? seconds) {
  if (seconds == null) {
    return '';
  }
  if (seconds <= 0) {
    return 'now';
  }
  final whole = seconds.round();
  if (whole < 60) {
    return 'in $whole s';
  }
  final minutes = whole ~/ 60;
  final rest = whole % 60;
  return rest == 0 ? 'in $minutes m' : 'in $minutes m $rest s';
}

/// The `countdown_s` value, or null when the core did not publish one. [elapsedSeconds] is the
/// display-only time since the snapshot arrived (the grid's one ticker), so the line ticks instead of
/// freezing until the next core event.
String? countdownLabel(ToolSnapshot? snapshot, {int elapsedSeconds = 0}) {
  final seconds = snapshot?.countdownSeconds;
  if (seconds == null) {
    return null;
  }
  final formatted = formatNextAction(seconds - elapsedSeconds);
  return formatted == 'now' ? 'due now' : 'next action $formatted';
}

/// The row's next moment in the core's own word (PROTOCOL §5 `rows[].next_action`) plus the published
/// seconds, e.g. `feed in 30 m` / `reload now`. Without a word this is just the time, as before.
String toolNextActionLabel(ToolRow row, {int elapsedSeconds = 0}) {
  final seconds = row.nextActionInSeconds;
  if (seconds == null) {
    return '';
  }
  final time = formatNextAction(seconds - elapsedSeconds);
  final word = row.nextAction?.wire;
  if (word == null) {
    return time;
  }
  return time == 'now' ? '$word now' : '$word $time';
}

/// The core's UTC ISO-8601 `ended_at` as a local clock time; `yyyy-MM-dd HH:mm` when it is not today.
/// Presentation only — the value stays the core's instant. Null when absent/unparseable.
String? formatFinishedStamp(String? iso, {DateTime? now}) {
  if (iso == null) {
    return null;
  }
  final parsed = DateTime.tryParse(iso);
  if (parsed == null) {
    return null;
  }
  final local = parsed.toLocal();
  final today = now ?? DateTime.now();
  String two(int value) => value.toString().padLeft(2, '0');
  final time = '${two(local.hour)}:${two(local.minute)}';
  final sameDay =
      local.year == today.year &&
      local.month == today.month &&
      local.day == today.day;
  return sameDay
      ? time
      : '${local.year}-${two(local.month)}-${two(local.day)} $time';
}

/// A one-line Hold Space report for the Spammer card (TOOL-FLOWS: its reporting lives there).
///
/// Null when there is nothing to say. The refusal is the core's line VERBATIM — the shell does not
/// rewrite it; the phase part is the core's own pinned word.
String? holdSpaceReport(ToolSnapshot? snapshot, String? refusal) {
  if (refusal != null) {
    return refusal;
  }
  if (snapshot == null || snapshot.phase == ToolPhase.idle) {
    return null;
  }
  final message = snapshot.message.trim();
  if (message.isEmpty) {
    return phaseWord(snapshot);
  }
  return '${phaseWord(snapshot)} — $message';
}

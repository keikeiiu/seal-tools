/// The card grid: one card per PLAN-V3-SHELL §6 tool (seven — Hold Space has no card), in plan order.
///
/// Every card's facts come from the model; this widget only projects them. Hold Space's report is
/// attached to the Spammer card because TOOL-FLOWS gives holdspace no card of its own.
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../app/app.dart';
import '../protocol/protocol.dart';
import 'theme.dart';
import 'tool_card.dart';
import 'tool_labels.dart';

class CardGrid extends StatelessWidget {
  const CardGrid({super.key, required this.model});

  final ShellModel model;

  @override
  Widget build(BuildContext context) {
    return GridView.builder(
      padding: const EdgeInsets.all(ShellSpace.lg),
      gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
        maxCrossAxisExtent: 460,
        mainAxisExtent: 320,
        crossAxisSpacing: ShellSpace.md,
        mainAxisSpacing: ShellSpace.md,
      ),
      itemCount: cardToolIds.length,
      itemBuilder: (context, index) {
        final tool = cardToolIds[index];
        return ToolCard(
          tool: tool,
          snapshot: model.tools.snapshot(tool),
          finished: model.tools.finished(tool),
          refusal: model.tools.refusal(tool),
          pending: model.tools.isPending(tool),
          ready: model.isReady,
          holdSpaceLine: tool == ToolId.spammer
              ? holdSpaceReport(
                  model.tools.snapshot(ToolId.holdspace),
                  model.tools.refusal(ToolId.holdspace),
                )
              : null,
          onStart: () => unawaited(model.startTool(tool)),
          onStop: () => unawaited(model.stopTool(tool)),
        );
      },
    );
  }
}

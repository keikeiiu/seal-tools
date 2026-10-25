/// Mini mode — deliberately a documented stub in this increment.
///
/// [PLAN-V3-SHELL.md] §6: a running foreground tool collapses the window to that tool's card ("mini
/// mode"), and Pet never does. Doing it properly needs the real window geometry (resize the native
/// window to the card, remember/restore the normal size), which is the Windows runner's C++ plus the
/// `ui.window` round-trip. This increment ships the full grid only; the hook is named here so the
/// later task has one place to land and no widget pretends the feature exists.
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'package:flutter/material.dart';

/// TODO(task: mini mode): replace with the collapsed-card window driven by the running card and the
/// runner's window geometry. Not wired anywhere yet on purpose.
class MiniModePlaceholder extends StatelessWidget {
  const MiniModePlaceholder({super.key});

  @override
  Widget build(BuildContext context) => const SizedBox.shrink();
}

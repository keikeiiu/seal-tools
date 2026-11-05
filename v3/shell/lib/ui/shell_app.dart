/// The app root: one [MaterialApp], one [ShellModel], one page. No routing, no state library
/// ([PLAN-V3-SHELL.md] §3/§5).
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'dart:ui' show AppExitResponse;

import 'package:flutter/material.dart';

import '../app/app.dart';
import 'shell_page.dart';
import 'theme.dart';

class ShellApp extends StatelessWidget {
  const ShellApp({super.key, required this.model});

  final ShellModel model;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: model,
      builder: (context, _) {
        final mode = switch (model.themeMode) {
          ShellThemeMode.system => ThemeMode.system,
          ShellThemeMode.light => ThemeMode.light,
          ShellThemeMode.dark => ThemeMode.dark,
        };
        return MaterialApp(
          title: 'Seal Tools',
          debugShowCheckedModeBanner: false,
          theme: buildShellTheme(brightness: Brightness.light),
          darkTheme: buildShellTheme(brightness: Brightness.dark),
          themeMode: mode,
          home: _ExitGuard(
            model: model,
            child: ShellPage(model: model),
          ),
        );
      },
    );
  }
}

/// SHELL-F6: the window's close request goes through Flutter's exit request, so a dirty config tab
/// gets a prompt instead of silently losing the edit, and a clean exit sends `shutdown` before EOF.
class _ExitGuard extends StatefulWidget {
  const _ExitGuard({required this.model, required this.child});

  final ShellModel model;
  final Widget child;

  @override
  State<_ExitGuard> createState() => _ExitGuardState();
}

class _ExitGuardState extends State<_ExitGuard> {
  late final AppLifecycleListener _listener;

  @override
  void initState() {
    super.initState();
    _listener = AppLifecycleListener(onExitRequested: _onExitRequested);
  }

  @override
  void dispose() {
    _listener.dispose();
    super.dispose();
  }

  Future<AppExitResponse> _onExitRequested() async {
    if (!mounted) {
      return AppExitResponse.exit;
    }
    if (widget.model.hasDirtyDrafts) {
      final leave = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('Discard unsaved changes?'),
          content: const Text(
            'One or more tabs have edits that were not saved. Closing now '
            'discards them.',
          ),
          actions: [
            TextButton(
              key: const ValueKey('exit-stay'),
              onPressed: () => Navigator.of(context).pop(false),
              child: const Text('Keep editing'),
            ),
            FilledButton(
              key: const ValueKey('exit-discard'),
              onPressed: () => Navigator.of(context).pop(true),
              child: const Text('Close without saving'),
            ),
          ],
        ),
      );
      if (leave != true) {
        return AppExitResponse.cancel;
      }
    }
    await widget.model.shutdown();
    return AppExitResponse.exit;
  }

  @override
  Widget build(BuildContext context) => widget.child;
}

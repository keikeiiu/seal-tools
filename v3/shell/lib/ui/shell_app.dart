/// The app root: one [MaterialApp], one [ShellModel], one page. No routing, no state library
/// ([PLAN-V3-SHELL.md] §3/§5).
///
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
library;

import 'package:flutter/material.dart';

import '../app/app.dart';
import 'shell_page.dart';
import 'theme.dart';

class ShellApp extends StatelessWidget {
  const ShellApp({super.key, required this.model});

  final ShellModel model;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Seal Tools',
      debugShowCheckedModeBanner: false,
      theme: buildShellTheme(),
      home: ShellPage(model: model),
    );
  }
}

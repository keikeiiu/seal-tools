/// The v3 shell's entry point: one model, one app, and the core started after the first frame.
///
/// It resolves what only `main` can know — the release-vs-development core and root paths
/// ([PLAN-V3-SHELL.md] §8/§9) — then hands the [ShellModel] a spawn function and the protocol client
/// version (`app/version.dart`). The window paints its connection surface first and `connect()` runs
/// post-frame, so a slow spawn can never delay the first frame ([PLAN-V3-SHELL.md] §2).
///
/// [PLAN-V3-SHELL.md]: ../../docs/plans/PLAN-V3-SHELL.md
library;

import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart' show kReleaseMode;
import 'package:flutter/material.dart';

import 'app/app.dart';
import 'protocol/protocol.dart';
import 'ui/shell_app.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  final model = ShellModel(
    transportFactory: spawnCore,
    shellVersion: protocolClientVersion,
  );
  runApp(ShellApp(model: model));
  WidgetsBinding.instance.addPostFrameCallback(
    (_) => unawaited(model.connect()),
  );
}

/// Spawns `seal-core.exe` exactly as [PROTOCOL.md] §1 describes: a child process, no shell, one
/// `--root` argument so the core does not have to guess where the config lives.
///
/// Paths:
/// - release: the core beside the shell's executable, root = the install folder ([PLAN-V3-SHELL.md] §9).
/// - development: the checkout's `v3/core/target/debug/seal-core.exe`, root = `<repo>/v3` (the
///   directory containing `config/` — [DESIGN.md] §3).
/// - `SEAL_CORE` / `SEAL_CORE_ROOT` dart-defines override either, for a dev pointing at a build or a
///   fixture root ([PLAN-V3-SHELL.md] §8 mode 2).
///
/// A missing path throws [CoreExecutableNotFoundException]/[CoreRootNotFoundException]; the model
/// turns that into the crash view with the searched paths, not a blank window.
///
/// [PROTOCOL.md]: ../../docs/spec/PROTOCOL.md
/// [DESIGN.md]: ../../docs/spec/DESIGN.md
Future<Transport> spawnCore() async {
  const executableOverride = String.fromEnvironment('SEAL_CORE');
  const rootOverride = String.fromEnvironment('SEAL_CORE_ROOT');
  final core = locateCoreExecutable(
    isRelease: kReleaseMode,
    executablePath: Platform.resolvedExecutable,
    workingDirectory: Directory.current.path,
    override: executableOverride,
  );
  final root = locateCoreRoot(
    isRelease: kReleaseMode,
    executablePath: Platform.resolvedExecutable,
    workingDirectory: Directory.current.path,
    override: rootOverride,
  );
  return CoreProcessTransport.start(
    executable: core.path,
    arguments: ['--root', root.path],
    workingDirectory: File(core.path).parent.path,
  );
}

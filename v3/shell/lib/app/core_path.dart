/// Where the shell finds `seal-core.exe` — the one rule PLAN-V3-SHELL §4/§8/§9 states.
///
/// The release finds the core **beside the shell's executable** (PLAN §9's folder is the unit that
/// moves), never on `PATH`. A development build has no such neighbour: it resolves the checkout's
/// `v3/core/target/debug/seal-core.exe` by walking up from the working directory and from the
/// executable, and `SEAL_CORE` (`--dart-define`) overrides both so a dev can point at any build.
///
/// This file is plain `dart:io` on purpose: no Flutter imports, so the rule is testable headless.
library;

import 'dart:io';

/// The core executable could not be found where the shell expects it.
class CoreExecutableNotFoundException implements Exception {
  CoreExecutableNotFoundException(this.message, {this.searched = const []});

  final String message;

  /// The paths the resolver looked at, so the crash view can show where it looked.
  final List<String> searched;

  @override
  String toString() {
    if (searched.isEmpty) {
      return 'CoreExecutableNotFoundException: $message';
    }
    return 'CoreExecutableNotFoundException: $message\nlooked at:\n'
        '${searched.map((path) => '  $path').join('\n')}';
  }
}

/// Resolution result: the chosen path plus the candidates that were considered.
class CoreExecutableLocation {
  const CoreExecutableLocation({required this.path, required this.searched});

  final String path;
  final List<String> searched;
}

/// Resolves the core executable for the current build.
///
/// - [override] (the `SEAL_CORE` dart-define) wins when non-empty; it is not existence-checked so a
///   dev typo surfaces as the spawn error for that exact path.
/// - [isRelease] chooses beside-the-shell (`seal-core.exe` next to [executablePath]) or the dev
///   checkout walk (`v3/core/target/debug/seal-core.exe` above [workingDirectory] or the executable).
/// - [executablePath] is `Platform.resolvedExecutable`; [workingDirectory] is `Directory.current`.
///
/// Throws [CoreExecutableNotFoundException] when no candidate exists; the candidate list travels with
/// the exception so the crash view can be actionable.
CoreExecutableLocation locateCoreExecutable({
  required bool isRelease,
  required String executablePath,
  required String workingDirectory,
  String override = '',
}) {
  if (override.trim().isNotEmpty) {
    return CoreExecutableLocation(path: override, searched: [override]);
  }

  final searched = <String>[];
  if (isRelease) {
    final besideShell = _join(_parentOf(executablePath), 'seal-core.exe');
    searched.add(besideShell);
    if (File(besideShell).existsSync()) {
      return CoreExecutableLocation(path: besideShell, searched: searched);
    }
    throw CoreExecutableNotFoundException(
      'seal-core.exe is not beside the shell (${_parentOf(executablePath)}) — '
      'the release folder must keep them together',
      searched: searched,
    );
  }

  final starts = <String>{workingDirectory, _parentOf(executablePath)};
  for (final start in starts) {
    for (final ancestor in _ancestors(start)) {
      // From the repository root: <root>/v3/core/target/debug/seal-core.exe.
      final fromRoot = _join(
        ancestor,
        'v3',
        'core',
        'target',
        'debug',
        'seal-core.exe',
      );
      // From the v3 directory itself: <v3>/core/target/debug/seal-core.exe — this is what a shell
      // whose cwd is v3/shell walks past first.
      final fromV3 = _join(
        ancestor,
        'core',
        'target',
        'debug',
        'seal-core.exe',
      );
      for (final candidate in [fromV3, fromRoot]) {
        if (searched.contains(candidate)) {
          continue;
        }
        searched.add(candidate);
        if (File(candidate).existsSync()) {
          return CoreExecutableLocation(path: candidate, searched: searched);
        }
      }
    }
  }

  throw CoreExecutableNotFoundException(
    'no development core build found — run `cargo build -p seal-core` in v3/core, '
    'or pass --dart-define=SEAL_CORE=<path to seal-core.exe>',
    searched: searched,
  );
}

/// The core's ROOT could not be found where the shell expects it.
class CoreRootNotFoundException implements Exception {
  CoreRootNotFoundException(this.message, {this.searched = const []});

  final String message;

  /// The directories the resolver looked in, for the crash view.
  final List<String> searched;

  @override
  String toString() {
    if (searched.isEmpty) {
      return 'CoreRootNotFoundException: $message';
    }
    return 'CoreRootNotFoundException: $message\nlooked in:\n'
        '${searched.map((path) => '  $path').join('\n')}';
  }
}

/// A resolved root directory: the directory CONTAINING `config/`.
class CoreRootLocation {
  const CoreRootLocation({required this.path, required this.searched});

  final String path;
  final List<String> searched;
}

/// Resolves the core's application root — the directory containing `config/`
/// ([DESIGN.md] §3: the core finds it by walking up for `config/defaults.yaml(.example)`).
///
/// - [override] (`SEAL_CORE_ROOT`) wins.
/// - Release: the directory beside the shell's executable ([PLAN-V3-SHELL.md] §9's install folder;
///   its `config/` is the user's state).
/// - Development: walk up for a directory with `config/defaults.yaml(.example)` — in this checkout
///   that is `<repo>/v3` — and also check `<ancestor>/v3` so a shell whose cwd is `v3/shell` or the
///   repository root finds the same root.
///
/// Throws [CoreRootNotFoundException] with the searched roots; `main.dart` turns that into the crash
/// view instead of letting the core boot-refuse with stderr only.
///
/// [DESIGN.md]: ../../../docs/spec/DESIGN.md
/// [PLAN-V3-SHELL.md]: ../../../docs/plans/PLAN-V3-SHELL.md
CoreRootLocation locateCoreRoot({
  required bool isRelease,
  required String executablePath,
  required String workingDirectory,
  String override = '',
}) {
  if (override.trim().isNotEmpty) {
    return CoreRootLocation(path: override, searched: [override]);
  }

  final searched = <String>[];
  if (isRelease) {
    final installDirectory = _parentOf(executablePath);
    searched.add(installDirectory);
    if (_hasConfigDirectory(installDirectory)) {
      return CoreRootLocation(path: installDirectory, searched: searched);
    }
    throw CoreRootNotFoundException(
      'the install folder beside the shell ($installDirectory) has no config/ — '
      'the release folder must contain config/defaults.yaml(.example)',
      searched: searched,
    );
  }

  for (final start in {workingDirectory, _parentOf(executablePath)}) {
    for (final ancestor in _ancestors(start)) {
      for (final candidate in [ancestor, _join(ancestor, 'v3')]) {
        if (searched.contains(candidate)) {
          continue;
        }
        searched.add(candidate);
        if (_hasConfigDirectory(candidate)) {
          return CoreRootLocation(path: candidate, searched: searched);
        }
      }
    }
  }

  throw CoreRootNotFoundException(
    'no v3 config root found — expected a directory with config/defaults.yaml(.example), '
    'usually <repo>/v3; run from the checkout or pass --dart-define=SEAL_CORE_ROOT=<root>',
    searched: searched,
  );
}

bool _hasConfigDirectory(String directory) =>
    File(_join(directory, 'config', 'defaults.yaml')).existsSync() ||
    File(_join(directory, 'config', 'defaults.yaml.example')).existsSync();

/// The directory part of a path, without pulling in the `path` package (no new dependencies).
String _parentOf(String path) {
  final normalized = path.replaceAll('/', Platform.pathSeparator);
  final trimmed = normalized.endsWith(Platform.pathSeparator)
      ? normalized.substring(0, normalized.length - 1)
      : normalized;
  final index = trimmed.lastIndexOf(Platform.pathSeparator);
  if (index <= 0) {
    return trimmed;
  }
  return trimmed.substring(0, index);
}

String _join(
  String first, [
  String? second,
  String? third,
  String? fourth,
  String? fifth,
  String? sixth,
]) {
  final parts = <String>[first, ?second, ?third, ?fourth, ?fifth, ?sixth];
  return parts
      .map(
        (part) => part.endsWith(Platform.pathSeparator)
            ? part.substring(0, part.length - 1)
            : part,
      )
      .join(Platform.pathSeparator);
}

Iterable<String> _ancestors(String start) sync* {
  var directory = Directory(start).absolute.path;
  while (true) {
    yield directory;
    final parent = _parentOf(directory);
    if (parent == directory) {
      return;
    }
    directory = parent;
  }
}

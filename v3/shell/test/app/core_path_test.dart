/// The path rules `main.dart` depends on: the core beside the shell in a release, the checkout's
/// debug build in development, and the root = the directory containing `config/`.
///
/// Uses a synthetic checkout under `.dart_tool/` so the walk-up rule is tested without depending on
/// this machine's real build directory.
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:seal_shell/app/app.dart';

String _normalized(String path) => path
    .replaceAll('/', Platform.pathSeparator)
    .replaceAll('\\', Platform.pathSeparator);

String _resolved(String path) =>
    _normalized(Directory(path).resolveSymbolicLinksSync());

void main() {
  late Directory sandbox;

  setUp(() {
    sandbox = Directory('.dart_tool/test-core-path').absolute;
    if (sandbox.existsSync()) {
      sandbox.deleteSync(recursive: true);
    }
    sandbox.createSync(recursive: true);
  });

  tearDown(() {
    if (sandbox.existsSync()) {
      sandbox.deleteSync(recursive: true);
    }
  });

  test(
    'development finds v3/core/target/debug/seal-core.exe by walking up',
    () {
      final repo = Directory('${sandbox.path}/repo');
      final debugDir = Directory('${repo.path}/v3/core/target/debug')
        ..createSync(recursive: true);
      final core = File('${debugDir.path}/seal-core.exe')
        ..writeAsStringSync('');
      final shellDir = Directory('${repo.path}/v3/shell')
        ..createSync(recursive: true);
      final shellExe = File('${shellDir.path}/SealTools.exe')
        ..writeAsStringSync('');

      final located = locateCoreExecutable(
        isRelease: false,
        executablePath: shellExe.path,
        workingDirectory: shellDir.path,
      );

      expect(_resolved(located.path), _resolved(core.path));
      expect(located.searched, isNotEmpty);
    },
  );

  test('release finds the core beside the shell; the dart-define wins', () {
    final install = Directory('${sandbox.path}/install')
      ..createSync(recursive: true);
    final shellExe = File('${install.path}/SealTools.exe')
      ..writeAsStringSync('');
    final core = File('${install.path}/seal-core.exe')..writeAsStringSync('');

    final located = locateCoreExecutable(
      isRelease: true,
      executablePath: shellExe.path,
      workingDirectory: Directory.current.path,
    );
    expect(_resolved(located.path), _resolved(core.path));

    final overridden = locateCoreExecutable(
      isRelease: true,
      executablePath: shellExe.path,
      workingDirectory: Directory.current.path,
      override: r'D:\custom\seal-core.exe',
    );
    expect(overridden.path, r'D:\custom\seal-core.exe');

    core.deleteSync();
    expect(
      () => locateCoreExecutable(
        isRelease: true,
        executablePath: shellExe.path,
        workingDirectory: Directory.current.path,
      ),
      throwsA(isA<CoreExecutableNotFoundException>()),
    );
  });

  test('development root is the directory containing config/', () {
    final repo = Directory('${sandbox.path}/repo');
    Directory('${repo.path}/v3/config').createSync(recursive: true);
    File('${repo.path}/v3/config/defaults.yaml').writeAsStringSync('{}');
    final shellDir = Directory('${repo.path}/v3/shell')
      ..createSync(recursive: true);
    final shellExe = File('${shellDir.path}/SealTools.exe')
      ..writeAsStringSync('');

    final located = locateCoreRoot(
      isRelease: false,
      executablePath: shellExe.path,
      workingDirectory: shellDir.path,
    );
    expect(_resolved(located.path), _resolved('${repo.path}/v3'));

    // And the real checkout: running `flutter test` from v3/shell, the root is <repo>/v3.
    final real = locateCoreRoot(
      isRelease: false,
      executablePath: Platform.resolvedExecutable,
      workingDirectory: Directory.current.path,
    );
    expect(_resolved(real.path), _resolved('..'));
    expect(
      File('${real.path}/config/defaults.yaml').existsSync(),
      isTrue,
      reason: 'the committed dev root must carry config/defaults.yaml',
    );
  });

  test('release root is the install folder, and a missing config throws', () {
    final install = Directory('${sandbox.path}/install')
      ..createSync(recursive: true);
    Directory('${install.path}/config').createSync(recursive: true);
    File('${install.path}/config/defaults.yaml').writeAsStringSync('{}');
    final shellExe = File('${install.path}/SealTools.exe')
      ..writeAsStringSync('');

    final located = locateCoreRoot(
      isRelease: true,
      executablePath: shellExe.path,
      workingDirectory: Directory.current.path,
    );
    expect(_resolved(located.path), _resolved(install.path));

    File('${install.path}/config/defaults.yaml').deleteSync();
    expect(
      () => locateCoreRoot(
        isRelease: true,
        executablePath: shellExe.path,
        workingDirectory: Directory.current.path,
      ),
      throwsA(isA<CoreRootNotFoundException>()),
    );
  });
}

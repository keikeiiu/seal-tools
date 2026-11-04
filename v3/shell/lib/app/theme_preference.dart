/// The player's app-appearance choice, persisted as the portable `settings.ui.theme` field.
///
/// The app layer deliberately owns no Flutter type: the shell maps this enum to `ThemeMode` and to the
/// runner's title-bar mode at the UI/host boundary. `dark` is the default so an install that predates
/// this setting keeps the original dark shell instead of changing appearance on first launch.
library;

enum ShellThemeMode {
  system('system', 'Follow system'),
  light('light', 'Light'),
  dark('dark', 'Dark');

  const ShellThemeMode(this.wire, this.label);

  final String wire;
  final String label;

  static ShellThemeMode fromWire(String? wire) => values.firstWhere(
    (mode) => mode.wire == wire,
    orElse: () => ShellThemeMode.dark,
  );
}

/// The shell's state layer: one [ShellModel] with the connection, tools, config and log sub-models.
///
/// Nothing here imports a widget. The UI renders these models; the models speak the protocol
/// ([lib/protocol](../../../protocol/README.md) is the seam). Where a decision is a UI convenience
/// rather than protocol bookkeeping, the code says so at the point of judgement.
library;

export 'config.dart';
export 'connection.dart';
export 'core_path.dart';
export 'logs.dart';
export 'shell_model.dart';
export 'tools.dart';

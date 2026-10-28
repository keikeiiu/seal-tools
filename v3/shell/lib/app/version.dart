/// The two version numbers the shell has, and the difference between them.
///
/// The app bar shows the **product** version, the one a player talks about and a release is named
/// after. The **protocol client** version is the identity the shell sends in `hello` ([PROTOCOL.md]
/// §1/§4); it versions the shell↔core conversation, not the product, and CUTOVER.md decision 2
/// deliberately keeps it at `0.1.0`. They do not move together.
library;

/// The product version shown in the app bar and named by a release. Keep it in step with
/// `pubspec.yaml`'s `version:` (which also feeds `SealTools.exe`'s Windows resource version).
const String productVersion = '3.0.3';

/// The shell's PROTOCOL CLIENT identity, sent in `hello`'s `client` field. This is an engineering
/// value — do not change it when the product version changes.
const String protocolClientVersion = '0.1.0';
